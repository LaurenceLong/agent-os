use crate::common::*;
use agent_os_config::AgentOsPaths;
use agent_os_ecosystem::{
    discover_ecosystem, expand_command_template, EcosystemCatalog, EcosystemDiscoverOptions,
    EcosystemImportReport,
};
use agent_os_thread::ModelTurnRequest;
use std::fs;
use std::process::Command;
use std::sync::{Arc, Mutex};

#[test]
fn ecosystem_imports_project_sources_and_replays_kernel_events() {
    let workspace = temp_workspace("agent-os-ecosystem-import");
    fs::create_dir_all(workspace.join(".agent-os/skills/review-skill/resources")).unwrap();
    fs::create_dir_all(workspace.join(".agent-os/commands/code")).unwrap();
    fs::create_dir_all(workspace.join(".agent-os/agents")).unwrap();
    fs::create_dir_all(workspace.join(".agent-os/prompts")).unwrap();
    fs::write(workspace.join("AGENTS.md"), "Project rule: read first.\n").unwrap();
    fs::write(
        workspace.join(".agent-os/skills/review-skill/SKILL.md"),
        "---\nname: review-skill\ndescription: Review code with local criteria.\n---\nRead resources/checklist.md before reporting.\n",
    )
    .unwrap();
    fs::write(
        workspace.join(".agent-os/skills/review-skill/resources/checklist.md"),
        "Check risk\nCheck tests\nCheck scope\n",
    )
    .unwrap();
    fs::write(
        workspace.join(".agent-os/commands/code/review.md"),
        "---\ndescription: Review one target file.\n---\nReview $1 with $ARGUMENTS.\n",
    )
    .unwrap();
    fs::write(
        workspace.join(".agent-os/agents/reviewer.md"),
        "---\ndescription: Focused reviewer.\nmode: subagent\n---\nAct as a focused reviewer.\n",
    )
    .unwrap();
    fs::write(
        workspace.join(".agent-os/prompts/supervisor.md"),
        "Supervisor prompt\n",
    )
    .unwrap();
    fs::write(
        workspace.join(".agent-os/manifest.json"),
        r#"{
  "manifest_version": "0.1",
  "package_name": "project-ecosystem-package",
  "package_type": "agent",
  "version": "0.1.0",
  "entrypoint": "prompts/supervisor.md",
  "required_kernel_version": "0.3",
  "capabilities_requested": ["tool.invoke"],
  "roles_provided": ["ReviewerAgent"],
  "tools_provided": [],
  "schemas": [],
  "signature": null
}
"#,
    )
    .unwrap();

    let kernel = Kernel::new();
    let report = import_workspace_ecosystem(&kernel, &workspace).unwrap();
    assert_eq!(report.packages, 1);
    assert!(report.instructions >= 1);
    assert!(report.skills >= 1);
    assert!(report.commands >= 1);
    assert!(report.agents >= 1);

    let state = kernel.state_snapshot().unwrap();
    assert!(state
        .instruction_documents
        .values()
        .any(|document| document.content == "Project rule: read first.\n"));
    assert!(state.skill_definitions.contains_key("review-skill"));
    assert!(state.command_definitions.contains_key("code/review"));
    assert!(state.imported_agent_profiles.contains_key("reviewer"));
    let package = state
        .package_installs
        .get("project-ecosystem-package")
        .expect("package manifest installed during import");
    assert_eq!(package.status, PackageInstallStatus::Enabled);
    assert_eq!(
        package.install_provenance.installed_by,
        "agent-os-ecosystem"
    );
    let contributions = state
        .package_contributions
        .values()
        .map(|contribution| {
            (
                contribution.contribution_kind,
                contribution.contribution_name.as_str(),
            )
        })
        .collect::<Vec<_>>();
    assert!(contributions.contains(&(PackageContributionKind::SkillDefinition, "review-skill")));
    assert!(contributions.contains(&(PackageContributionKind::CommandDefinition, "code/review")));
    assert!(contributions.contains(&(PackageContributionKind::ImportedAgentProfile, "reviewer")));

    let replayed = Kernel::from_events(&kernel.events().unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert!(replayed_state
        .package_installs
        .contains_key("project-ecosystem-package"));
    assert_eq!(
        replayed_state.package_contributions.len(),
        state.package_contributions.len()
    );
    assert_eq!(
        replayed_state.skill_definitions["review-skill"].content,
        state.skill_definitions["review-skill"].content
    );
    assert_eq!(
        replayed_state.command_definitions["code/review"].argument_hints,
        vec!["$ARGUMENTS".to_string(), "$1".to_string()]
    );
    assert_eq!(
        expand_command_template(
            &replayed_state.command_definitions["code/review"].template,
            &["src/lib.rs".to_string(), "extra".to_string()],
            "src/lib.rs extra",
        ),
        "Review src/lib.rs with src/lib.rs extra."
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn ecosystem_imports_instruction_precedence_through_kernel_events() {
    let root = temp_workspace("agent-os-instruction-precedence");
    let project = root.join("repo");
    let workspace = project.join("nested").join("crate");
    let paths = test_paths(&workspace);
    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir_all(&paths.config_dir).unwrap();
    fs::write(paths.config_dir.join("AGENTS.md"), "global agents\n").unwrap();
    fs::write(project.join("CLAUDE.md"), "parent claude\n").unwrap();
    fs::write(project.join("AGENTS.md"), "parent agents\n").unwrap();
    fs::write(workspace.join("CLAUDE.md"), "nearest claude\n").unwrap();
    fs::write(workspace.join("AGENTS.md"), "nearest agents\n").unwrap();

    let catalog = discover_ecosystem(&EcosystemDiscoverOptions {
        workspace_root: workspace.clone(),
        paths,
    })
    .unwrap();
    assert_eq!(catalog.instruction_documents.len(), 5);

    let kernel = Kernel::new();
    import_catalog(&kernel, &catalog).unwrap();
    let imported_events = kernel
        .events()
        .unwrap()
        .into_iter()
        .filter(|event| event.event_type == "InstructionDocumentImported")
        .collect::<Vec<_>>();
    assert_eq!(imported_events.len(), 5);

    let mut instructions = kernel
        .state_snapshot()
        .unwrap()
        .instruction_documents
        .values()
        .map(|document| (document.precedence_rank, document.content.clone()))
        .collect::<Vec<_>>();
    instructions.sort_by_key(|(precedence, _)| *precedence);
    assert_eq!(
        instructions,
        vec![
            (0, "global agents\n".to_string()),
            (1, "parent claude\n".to_string()),
            (2, "parent agents\n".to_string()),
            (3, "nearest claude\n".to_string()),
            (4, "nearest agents\n".to_string()),
        ]
    );

    let replayed = Kernel::from_events(&kernel.events().unwrap()).unwrap();
    let mut replayed_instructions = replayed
        .state_snapshot()
        .unwrap()
        .instruction_documents
        .values()
        .map(|document| (document.precedence_rank, document.content.clone()))
        .collect::<Vec<_>>();
    replayed_instructions.sort_by_key(|(precedence, _)| *precedence);
    assert_eq!(replayed_instructions, instructions);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn package_install_enable_disable_state_replays_from_kernel_events() {
    let workspace = temp_workspace("agent-os-package-install-state");
    fs::create_dir_all(workspace.join(".agent-os/skills/pkg-skill")).unwrap();
    fs::create_dir_all(workspace.join(".agent-os/prompts")).unwrap();
    fs::create_dir_all(workspace.join(".agent-os/policy")).unwrap();
    fs::write(
        workspace.join(".agent-os/skills/pkg-skill/SKILL.md"),
        "---\nname: pkg-skill\ndescription: Package skill.\n---\nUse the package skill.\n",
    )
    .unwrap();
    fs::write(
        workspace.join(".agent-os/prompts/supervisor.md"),
        "Prompt\n",
    )
    .unwrap();
    fs::write(workspace.join(".agent-os/config.json"), "{}\n").unwrap();
    fs::write(workspace.join(".agent-os/policy/review.json"), "{}\n").unwrap();
    fs::write(
        workspace.join(".agent-os/manifest.json"),
        r#"{
  "manifest_version": "0.1",
  "package_name": "governed-package",
  "package_type": "agent",
  "version": "0.1.0",
  "entrypoint": "prompts/supervisor.md",
  "required_kernel_version": "0.3",
  "capabilities_requested": ["tool.invoke"],
  "roles_provided": ["ProducerAgent"],
  "tools_provided": [],
  "schemas": ["policy/review.json"],
  "signature": null
}
"#,
    )
    .unwrap();
    let catalog = discover_ecosystem(&EcosystemDiscoverOptions {
        workspace_root: workspace.clone(),
        paths: test_paths(&workspace),
    })
    .unwrap();
    let package = catalog.package_manifests[0].clone();
    let skill = catalog
        .skill_definitions
        .iter()
        .find(|skill| skill.name == "pkg-skill")
        .unwrap()
        .clone();

    let kernel = Kernel::new();
    let install = kernel
        .install_package_manifest(
            package.clone(),
            PackageInstallProvenance {
                installed_by: "conformance".to_string(),
                reason: Some("test install".to_string()),
            },
        )
        .unwrap();
    assert_eq!(install.manifest.package_name, "governed-package");
    assert_eq!(install.status, PackageInstallStatus::Enabled);
    assert_eq!(
        kernel
            .state_snapshot()
            .unwrap()
            .package_installs
            .get("governed-package")
            .unwrap()
            .content_hash,
        package.content_hash
    );

    let duplicate = kernel
        .install_package_manifest(
            package,
            PackageInstallProvenance {
                installed_by: "conformance".to_string(),
                reason: None,
            },
        )
        .unwrap_err();
    assert!(
        matches!(duplicate, AgentOsError::Validation(ref message) if message.contains("already installed")),
        "{duplicate:?}"
    );
    let skill_contribution = kernel
        .register_package_contribution(
            "governed-package",
            PackageContributionKind::SkillDefinition,
            skill.skill_id.clone(),
            skill.name.clone(),
            skill.source.clone(),
            Some(skill.content_hash.clone()),
        )
        .unwrap();
    assert_eq!(
        skill_contribution.contribution_kind,
        PackageContributionKind::SkillDefinition
    );
    assert_eq!(skill_contribution.contribution_name, "pkg-skill");
    let package_config_path = std::path::PathBuf::from(&install.root_path).join("config.json");

    let disabled = kernel
        .disable_package("governed-package", "disabled by policy")
        .unwrap();
    assert_eq!(disabled.status, PackageInstallStatus::Disabled);
    assert_eq!(
        disabled.disabled_reason.as_deref(),
        Some("disabled by policy")
    );
    let disabled_contribution = kernel
        .register_package_contribution(
            "governed-package",
            PackageContributionKind::McpServer,
            "mcp_server_disabled",
            "disabled-server",
            EcosystemSource {
                source_kind: EcosystemSourceKind::AgentOs,
                source_scope: EcosystemSourceScope::Project,
                source_path: package_config_path.to_string_lossy().to_string(),
            },
            None,
        )
        .unwrap_err();
    assert!(
        matches!(disabled_contribution, AgentOsError::Validation(ref message) if message.contains("is disabled")),
        "{disabled_contribution:?}"
    );
    let enabled = kernel.enable_package("governed-package").unwrap();
    assert_eq!(enabled.status, PackageInstallStatus::Enabled);
    assert_eq!(enabled.disabled_reason, None);
    let mcp_contribution = kernel
        .register_package_contribution(
            "governed-package",
            PackageContributionKind::McpServer,
            "mcp_server_echo",
            "echo",
            EcosystemSource {
                source_kind: EcosystemSourceKind::AgentOs,
                source_scope: EcosystemSourceScope::Project,
                source_path: package_config_path.to_string_lossy().to_string(),
            },
            None,
        )
        .unwrap();
    assert_eq!(
        mcp_contribution.contribution_kind,
        PackageContributionKind::McpServer
    );
    let outside_contribution = kernel
        .register_package_contribution(
            "governed-package",
            PackageContributionKind::CommandDefinition,
            "command_outside",
            "outside",
            EcosystemSource {
                source_kind: EcosystemSourceKind::AgentOs,
                source_scope: EcosystemSourceScope::Project,
                source_path: workspace.join("outside.md").to_string_lossy().to_string(),
            },
            None,
        )
        .unwrap_err();
    assert!(
        matches!(outside_contribution, AgentOsError::Validation(ref message) if message.contains("outside package root")),
        "{outside_contribution:?}"
    );

    let events = kernel.events().unwrap();
    assert!(events
        .iter()
        .any(|event| event.event_type == "PackageInstalled"));
    assert!(events
        .iter()
        .any(|event| event.event_type == "PackageDisabled"));
    assert!(events
        .iter()
        .any(|event| event.event_type == "PackageEnabled"));
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "PackageContributionRegistered")
            .count(),
        2
    );

    let replayed = Kernel::from_events(&events).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    let replayed_package = replayed_state
        .package_installs
        .get("governed-package")
        .unwrap();
    assert_eq!(replayed_package.status, PackageInstallStatus::Enabled);
    assert_eq!(
        replayed_package.manifest.entrypoint,
        "prompts/supervisor.md"
    );
    assert_eq!(
        replayed_package.install_provenance.installed_by,
        "conformance"
    );
    let replayed_contribution_names = replayed_state
        .package_contributions
        .values()
        .map(|contribution| contribution.contribution_name.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        replayed_contribution_names,
        std::collections::BTreeSet::from(["echo", "pkg-skill"])
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn ecosystem_project_agent_os_overrides_agents_and_claude_skill_names() {
    let workspace = temp_workspace("agent-os-ecosystem-duplicate-skill");
    for (root, marker) in [
        (".claude", "claude"),
        (".agents", "agents"),
        (".agent-os", "agent-os"),
    ] {
        fs::create_dir_all(workspace.join(root).join("skills/dupe")).unwrap();
        fs::write(
            workspace.join(root).join("skills/dupe/SKILL.md"),
            format!("---\nname: dupe\ndescription: {marker} skill.\n---\nDifferent content from {marker}.\n"),
        )
        .unwrap();
    }

    let kernel = Kernel::new();
    import_workspace_ecosystem(&kernel, &workspace).unwrap();
    let state = kernel.state_snapshot().unwrap();

    assert_eq!(
        state.skill_definitions["dupe"].description,
        "agent-os skill."
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn ecosystem_rejects_command_shell_interpolation() {
    let workspace = temp_workspace("agent-os-ecosystem-command-reject");
    fs::create_dir_all(workspace.join(".agent-os/commands")).unwrap();
    fs::write(
        workspace.join(".agent-os/commands/unsafe.md"),
        "---\ndescription: Unsafe command.\n---\nRun !`rm -rf .`.\n",
    )
    .unwrap();

    let error = import_workspace_ecosystem(&Kernel::new(), &workspace).unwrap_err();
    assert!(
        matches!(&error, AgentOsError::Validation(message) if message.contains("unsupported shell interpolation")),
        "{error:?}"
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn skill_tools_enforce_scope_and_skill_root_resource_bounds() {
    let workspace = temp_workspace("agent-os-ecosystem-skill-tools");
    fs::create_dir_all(workspace.join(".agent-os/skills/review-skill/resources")).unwrap();
    fs::write(
        workspace.join(".agent-os/skills/review-skill/SKILL.md"),
        "---\nname: review-skill\ndescription: Review code with local criteria.\n---\nUse resources/checklist.md.\nSecond skill line.\nThird skill line.\n",
    )
    .unwrap();
    fs::write(
        workspace.join(".agent-os/skills/review-skill/resources/checklist.md"),
        "Check risk\nCheck tests\nCheck scope\n",
    )
    .unwrap();
    let fx = fixture();
    import_workspace_ecosystem(&fx.kernel, &workspace).unwrap();
    let _lease = attach_writable_environment(&fx);
    let allowed = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec![
                "tool:*".to_string(),
                "skill:review-skill".to_string(),
                "skill_file:review-skill:*".to_string(),
            ],
            1,
            None,
        )
        .unwrap();

    let loaded = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            allowed.capability_id.clone(),
            1,
            ToolInvokeInput {
                tool_name: "load_skill".to_string(),
                input: json!({"name": "review-skill", "offset": 1, "limit": 1}),
                evidence_claim: Some("skill loaded".to_string()),
            },
        )
        .unwrap();
    let loaded_output = loaded.output.unwrap();
    assert_eq!(loaded_output["name"], json!("review-skill"));
    assert_eq!(loaded_output["offset"], json!(1));
    assert_eq!(loaded_output["limit"], json!(1));
    assert_eq!(
        loaded_output["content"],
        json!("Use resources/checklist.md.\n")
    );
    assert_eq!(loaded_output["total_lines"], json!(3));
    assert_eq!(loaded_output["returned_lines"], json!(1));
    assert_eq!(loaded_output["next_offset"], json!(2));
    assert_eq!(loaded_output["truncated"], json!(true));

    let resource = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            allowed.capability_id.clone(),
            1,
            ToolInvokeInput {
                tool_name: "read_skill_resource".to_string(),
                input: json!({
                    "name": "review-skill",
                    "path": "resources/checklist.md",
                    "offset": 1,
                    "limit": 1
                }),
                evidence_claim: Some("resource loaded".to_string()),
            },
        )
        .unwrap();
    let resource_output = resource.output.unwrap();
    assert_eq!(resource_output["content"], json!("Check risk\n"));
    assert_eq!(resource_output["bytes_read"], json!(11));
    assert_eq!(resource_output["offset"], json!(1));
    assert_eq!(resource_output["limit"], json!(1));
    assert_eq!(resource_output["total_lines"], json!(3));
    assert_eq!(resource_output["returned_lines"], json!(1));
    assert_eq!(resource_output["next_offset"], json!(2));
    assert_eq!(resource_output["truncated"], json!(true));
    assert_eq!(resource_output["omitted_lines"], json!(2));

    let denied = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            allowed.capability_id,
            1,
            ToolInvokeInput {
                tool_name: "read_skill_resource".to_string(),
                input: json!({"name": "review-skill", "path": "../SKILL.md"}),
                evidence_claim: None,
            },
        )
        .unwrap();
    assert_eq!(denied.status, ToolCallStatus::Failed);
    let error = denied
        .output
        .as_ref()
        .and_then(|output| output.get("error"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    assert!(error.contains("skill resource path") && error.contains("skill root"));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn local_stdio_mcp_registers_and_executes_with_kernel_permissions() {
    let workspace = temp_workspace("agent-os-ecosystem-mcp");
    fs::create_dir_all(&workspace).unwrap();
    let server = compile_mcp_fixture(&workspace);
    fs::create_dir_all(workspace.join(".agent-os/prompts")).unwrap();
    fs::write(
        workspace.join(".agent-os/prompts/supervisor.md"),
        "MCP package prompt\n",
    )
    .unwrap();
    fs::write(
        workspace.join(".agent-os/manifest.json"),
        r#"{
  "manifest_version": "0.1",
  "package_name": "mcp-fixture-package",
  "package_type": "agent",
  "version": "0.1.0",
  "entrypoint": "prompts/supervisor.md",
  "required_kernel_version": "0.3",
  "capabilities_requested": ["tool.invoke"],
  "roles_provided": ["ProducerAgent"],
  "tools_provided": ["mcp__echo__echo"],
  "schemas": [],
  "signature": null
}
"#,
    )
    .unwrap();
    fs::write(
        workspace.join(".agent-os/config.json"),
        json!({
            "mcp": {
                "local_stdio": {
                    "echo": {
                        "command": [server.to_string_lossy()]
                    }
                }
            }
        })
        .to_string(),
    )
    .unwrap();

    let fx = fixture();
    let report = import_workspace_ecosystem(&fx.kernel, &workspace).unwrap();
    assert_eq!(report.mcp_servers, 1);
    assert_eq!(report.mcp_tools, 1);
    assert_eq!(report.mcp_resources, 1);
    assert_eq!(report.mcp_resource_templates, 1);
    let state = fx.kernel.state_snapshot().unwrap();
    let tool = state.mcp_tools.get("mcp__echo__echo").unwrap();
    assert_eq!(tool.server_name, "echo");
    let resource = state.mcp_resources.get("fixture://status").unwrap();
    assert_eq!(resource.server_name, "echo");
    assert_eq!(resource.name.as_deref(), Some("Fixture Status"));
    let template = state
        .mcp_resource_templates
        .get("fixture://items/{id}")
        .unwrap();
    assert_eq!(template.server_name, "echo");
    assert_eq!(template.name.as_deref(), Some("Fixture Item"));
    let contribution_kinds = state
        .package_contributions
        .values()
        .map(|contribution| {
            (
                contribution.contribution_kind,
                contribution.contribution_name.as_str(),
            )
        })
        .collect::<Vec<_>>();
    assert!(contribution_kinds.contains(&(PackageContributionKind::McpServer, "echo")));
    assert!(contribution_kinds.contains(&(PackageContributionKind::McpTool, "mcp__echo__echo")));
    assert!(
        contribution_kinds.contains(&(PackageContributionKind::McpResource, "fixture://status"))
    );
    assert!(contribution_kinds.contains(&(
        PackageContributionKind::McpResourceTemplate,
        "fixture://items/{id}"
    )));
    let replayed = Kernel::from_events(&fx.kernel.events().unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert!(replayed_state
        .mcp_resources
        .contains_key("fixture://status"));
    assert!(replayed_state
        .mcp_resource_templates
        .contains_key("fixture://items/{id}"));
    let descriptor = state.tool_descriptors.get("mcp__echo__echo").unwrap();
    assert_eq!(descriptor.driver_class, ToolDriverClass::Mcp);
    assert_eq!(
        descriptor.lifecycle.foreground_timeout_ms,
        DEFAULT_TOOL_FOREGROUND_TIMEOUT_MS
    );
    assert_eq!(
        descriptor.lifecycle.background_execution,
        ToolBackgroundExecution::KernelWorker
    );
    assert_eq!(
        descriptor.lifecycle.output_management.mode,
        ToolOutputManagementMode::ManagedTextFields
    );
    assert_eq!(
        descriptor.lifecycle.output_management.max_window_bytes,
        TOOL_OUTPUT_MAX_WINDOW_BYTES
    );

    let _lease = attach_writable_environment(&fx);
    let allowed = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string(), "mcp:echo:echo".to_string()],
            3,
            None,
        )
        .unwrap();
    let invocation = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            allowed.capability_id.clone(),
            3,
            ToolInvokeInput {
                tool_name: "mcp__echo__echo".to_string(),
                input: json!({"text": "hello"}),
                evidence_claim: Some("MCP echo executed".to_string()),
            },
        )
        .unwrap();
    assert_eq!(
        invocation.output.unwrap()["raw_result"]["content"][0]["text"],
        json!("hello")
    );

    let denied = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            3,
            None,
        )
        .unwrap();
    let denied_result = fx.kernel.invoke_tool(
        &fx.worker.agent_id,
        &fx.task.task_id,
        &fx.worker.session_id,
        denied.capability_id,
        3,
        ToolInvokeInput {
            tool_name: "mcp__echo__echo".to_string(),
            input: json!({"text": "blocked"}),
            evidence_claim: None,
        },
    );
    assert!(
        matches!(&denied_result, Err(AgentOsError::PermissionDenied(message)) if message.contains("resource scope mcp:echo:echo")),
        "{denied_result:?}"
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_projects_ecosystem_into_model_context() {
    let workspace = temp_workspace("agent-os-ecosystem-runtime-projection");
    fs::create_dir_all(workspace.join(".agent-os/skills/review-skill")).unwrap();
    fs::write(
        workspace.join("AGENTS.md"),
        "Project rule: use skills on demand.\n",
    )
    .unwrap();
    fs::write(
        workspace.join(".agent-os/skills/review-skill/SKILL.md"),
        "---\nname: review-skill\ndescription: Review code with local criteria.\n---\nSECRET_SKILL_BODY\n",
    )
    .unwrap();
    let kernel = Kernel::new();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "ecosystem-runtime".to_string(),
            created_by: "conformance".to_string(),
            title: "Project ecosystem".to_string(),
            description: "Project ecosystem into runtime context".to_string(),
            acceptance_criteria: vec!["ecosystem is visible to the model".to_string()],
            constraints: Vec::new(),
            risk_level: 1,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Finish".to_string(),
            description: "Finish".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: Vec::new(),
            priority: 1,
            risk_level: 1,
        })
        .unwrap();
    let worker = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id,
            role_profile_id: "role_producer".to_string(),
            owner: "conformance".to_string(),
            goal: "Inspect projected ecosystem".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    import_workspace_ecosystem(&kernel, &workspace).unwrap();
    let seen = Arc::new(Mutex::new(None));
    let model = CapturingModel {
        seen: Arc::clone(&seen),
    };
    let mut runtime = agent_os_thread::ThreadRuntime::new(kernel, worker.thread_id, model);
    let mut config = agent_os_thread::RuntimeConfig::workspace_write(&workspace);
    config.max_steps = 1;
    let report = runtime.run_to_completion(config).unwrap();
    assert_eq!(report.status, ThreadStatus::Blocked);
    assert!(!report.final_submitted);
    let request = seen.lock().unwrap().clone().unwrap();
    assert!(request
        .context
        .instruction_documents
        .iter()
        .any(|document| document.content == "Project rule: use skills on demand.\n"));
    let skill = request
        .context
        .skill_definitions
        .iter()
        .find(|skill| skill.name == "review-skill")
        .unwrap();
    assert_eq!(skill.content, "SECRET_SKILL_BODY");
    let _ = fs::remove_dir_all(workspace);
}

#[derive(Clone)]
struct CapturingModel {
    seen: Arc<Mutex<Option<ModelTurnRequest>>>,
}

impl agent_os_thread::ModelClient for CapturingModel {
    fn next(
        &mut self,
        request: &ModelTurnRequest,
    ) -> AgentOsResult<agent_os_thread::ModelTurnResponse> {
        *self.seen.lock().unwrap() = Some(request.clone());
        Ok(agent_os_thread::ModelTurnResponse::single(
            agent_os_thread::ModelAction::OutputText {
                text: "observed".to_string(),
            },
        ))
    }
}

fn compile_mcp_fixture(workspace: &std::path::Path) -> std::path::PathBuf {
    let source = workspace.join("mcp_fixture.rs");
    let binary = workspace.join(format!("mcp_fixture{}", std::env::consts::EXE_SUFFIX));
    fs::write(
        &source,
        r##"
use std::io::{self, BufRead};

fn main() {
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        if line.contains("\"method\":\"tools/list\"") {
            println!("{}", r#"{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"echo","description":"Echo one text field.","inputSchema":{"type":"object","required":["text"],"properties":{"text":{"type":"string"}},"additionalProperties":false}}]}}"#);
        } else if line.contains("\"method\":\"resources/list\"") {
            println!("{}", r#"{"jsonrpc":"2.0","id":2,"result":{"resources":[{"uri":"fixture://status","name":"Fixture Status","description":"Fixture status resource.","mimeType":"text/plain"}]}}"#);
        } else if line.contains("\"method\":\"resources/templates/list\"") {
            println!("{}", r#"{"jsonrpc":"2.0","id":2,"result":{"resourceTemplates":[{"uriTemplate":"fixture://items/{id}","name":"Fixture Item","description":"Fixture item resource.","mimeType":"application/json"}]}}"#);
        } else if line.contains("\"method\":\"tools/call\"") {
            let text = line.split("\"text\":\"").nth(1).and_then(|rest| rest.split('"').next()).unwrap_or("");
            println!(r#"{{"jsonrpc":"2.0","id":2,"result":{{"content":[{{"type":"text","text":"{}"}}]}}}}"#, text);
        } else if line.contains("\"method\":\"initialize\"") {
            println!("{}", r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"fixture","version":"0.0.1"}}}"#);
        }
    }
}
"##,
    )
    .unwrap();
    let output = Command::new("rustc")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "rustc failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    binary
}

fn temp_workspace(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        new_id("case_")
    ))
}

fn import_workspace_ecosystem(
    kernel: &Kernel,
    workspace: &std::path::Path,
) -> AgentOsResult<EcosystemImportReport> {
    let catalog = discover_ecosystem(&EcosystemDiscoverOptions {
        workspace_root: workspace.to_path_buf(),
        paths: test_paths(workspace),
    })?;
    import_catalog(kernel, &catalog)
}

fn import_catalog(
    kernel: &Kernel,
    catalog: &EcosystemCatalog,
) -> AgentOsResult<EcosystemImportReport> {
    for package in &catalog.package_manifests {
        if !kernel
            .state_snapshot()?
            .package_installs
            .contains_key(&package.manifest.package_name)
        {
            kernel.install_package_manifest(
                package.clone(),
                PackageInstallProvenance {
                    installed_by: "agent-os-ecosystem".to_string(),
                    reason: Some("workspace ecosystem import".to_string()),
                },
            )?;
        }
    }
    for document in &catalog.instruction_documents {
        kernel.import_instruction_document(document.clone())?;
        register_catalog_contribution(
            kernel,
            &catalog.package_manifests,
            PackageContributionKind::InstructionDocument,
            &document.instruction_id,
            &document.source.source_path,
            &document.source,
            Some(document.content_hash.as_str()),
        )?;
    }
    for skill in &catalog.skill_definitions {
        kernel.import_skill_definition(skill.clone())?;
        register_catalog_contribution(
            kernel,
            &catalog.package_manifests,
            PackageContributionKind::SkillDefinition,
            &skill.skill_id,
            &skill.name,
            &skill.source,
            Some(skill.content_hash.as_str()),
        )?;
    }
    for command in &catalog.command_definitions {
        kernel.import_command_definition(command.clone())?;
        register_catalog_contribution(
            kernel,
            &catalog.package_manifests,
            PackageContributionKind::CommandDefinition,
            &command.command_id,
            &command.name,
            &command.source,
            Some(command.content_hash.as_str()),
        )?;
    }
    for profile in &catalog.imported_agent_profiles {
        kernel.register_imported_agent_profile(profile.clone())?;
        register_catalog_contribution(
            kernel,
            &catalog.package_manifests,
            PackageContributionKind::ImportedAgentProfile,
            &profile.imported_agent_profile_id,
            &profile.name,
            &profile.source,
            Some(profile.content_hash.as_str()),
        )?;
    }
    for server in &catalog.mcp_servers {
        kernel.register_mcp_server_spec(server.clone())?;
        register_catalog_contribution(
            kernel,
            &catalog.package_manifests,
            PackageContributionKind::McpServer,
            &server.server_id,
            &server.name,
            &server.source,
            None,
        )?;
    }
    for tool in &catalog.mcp_tools {
        kernel.register_mcp_tool_definition(tool.clone())?;
        register_catalog_contribution(
            kernel,
            &catalog.package_manifests,
            PackageContributionKind::McpTool,
            &tool.mcp_tool_id,
            &tool.model_tool_name,
            &tool.source,
            None,
        )?;
    }
    for resource in &catalog.mcp_resources {
        kernel.register_mcp_resource_definition(resource.clone())?;
        register_catalog_contribution(
            kernel,
            &catalog.package_manifests,
            PackageContributionKind::McpResource,
            &resource.mcp_resource_id,
            &resource.uri,
            &resource.source,
            None,
        )?;
    }
    for template in &catalog.mcp_resource_templates {
        kernel.register_mcp_resource_template_definition(template.clone())?;
        register_catalog_contribution(
            kernel,
            &catalog.package_manifests,
            PackageContributionKind::McpResourceTemplate,
            &template.mcp_resource_template_id,
            &template.uri_template,
            &template.source,
            None,
        )?;
    }
    Ok(catalog.import_report())
}

fn register_catalog_contribution(
    kernel: &Kernel,
    packages: &[PackageManifestRecord],
    kind: PackageContributionKind,
    contribution_id: &str,
    contribution_name: &str,
    source: &EcosystemSource,
    content_hash: Option<&str>,
) -> AgentOsResult<()> {
    let Some(package) = package_for_source(packages, source) else {
        return Ok(());
    };
    if kernel
        .state_snapshot()?
        .package_contributions
        .values()
        .any(|contribution| {
            contribution.package_name == package.manifest.package_name
                && contribution.contribution_kind == kind
                && contribution.contribution_id == contribution_id
                && contribution.content_hash.as_deref() == content_hash
        })
    {
        return Ok(());
    }
    kernel.register_package_contribution(
        &package.manifest.package_name,
        kind,
        contribution_id.to_string(),
        contribution_name.to_string(),
        source.clone(),
        content_hash.map(str::to_string),
    )?;
    Ok(())
}

fn package_for_source<'a>(
    packages: &'a [PackageManifestRecord],
    source: &EcosystemSource,
) -> Option<&'a PackageManifestRecord> {
    let source_path = std::path::Path::new(&source.source_path);
    packages
        .iter()
        .find(|package| source_path.starts_with(std::path::Path::new(&package.root_path)))
}

fn test_paths(workspace: &std::path::Path) -> AgentOsPaths {
    let home = workspace.join("__agent_os_home");
    AgentOsPaths {
        home: home.clone(),
        config_dir: home.join("config"),
        data_dir: home.join("data"),
        state_dir: home.join("state"),
        cache_dir: home.join("cache"),
        log_dir: home.join("log"),
        bin_dir: home.join("cache/bin"),
    }
}
