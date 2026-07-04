use crate::common::*;
use serde_json::Value;
use std::{collections::BTreeSet, fs, path::PathBuf};

#[test]
fn tool_broker_integration_runs_all_model_visible_tool_families() {
    let fx = fixture();
    let workspace = temp_workspace("agent-os-integration-tool-broker");
    fs::create_dir_all(&workspace).unwrap();
    fs::write(workspace.join("read.txt"), "read me\n").unwrap();
    fs::write(
        workspace.join("shot.png"),
        [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a],
    )
    .unwrap();
    fs::write(workspace.join("edit.txt"), "alpha old beta\n").unwrap();
    fs::write(workspace.join("delete.txt"), "remove me\n").unwrap();

    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "integration-test".to_string(),
            goal: "Exercise model-visible tools through the kernel broker".to_string(),
            success_criteria: vec!["all tool families persist state".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    attach_workspace_for_agent(&fx.kernel, &supervisor, &fx.task.task_id, &workspace);
    let capability = fx
        .kernel
        .grant_capability(
            &supervisor.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap();
    let tools = ToolInvoker {
        kernel: &fx.kernel,
        agent: &supervisor,
        task_id: &fx.task.task_id,
        capability: &capability,
    };

    let read = tools.invoke(
        1,
        "read_file",
        json!({
            "workspace_root": workspace.to_string_lossy(),
            "path": "read.txt"
        }),
        Some("source file was inspected"),
    );
    let glob = tools.invoke(
        1,
        "glob_files",
        json!({
            "workspace_root": workspace.to_string_lossy(),
            "pattern": "read.txt",
            "limit": 10
        }),
        Some("workspace glob located source path"),
    );
    let grep = tools.invoke(
        1,
        "grep_files",
        json!({
            "workspace_root": workspace.to_string_lossy(),
            "pattern": "read me",
            "path": "read.txt",
            "limit": 10
        }),
        Some("workspace grep located source content"),
    );
    let image = tools.invoke(
        1,
        "read_image",
        json!({
            "workspace_root": workspace.to_string_lossy(),
            "path": "shot.png"
        }),
        Some("source image was inspected"),
    );
    tools.invoke(
        4,
        "apply_patch",
        json!({
            "workspace_root": workspace.to_string_lossy(),
            "patch": "*** Begin Patch\n*** Add File: created.txt\n+created through integration broker\n*** End Patch\n"
        }),
        Some("new file was created through apply_patch"),
    );
    tools.invoke(
        4,
        "apply_patch",
        json!({
            "workspace_root": workspace.to_string_lossy(),
            "patch": "*** Begin Patch\n*** Update File: edit.txt\n@@\n-alpha old beta\n+alpha new beta\n*** End Patch\n"
        }),
        Some("workspace file was updated through apply_patch"),
    );
    tools.invoke(
        4,
        "apply_patch",
        json!({
            "workspace_root": workspace.to_string_lossy(),
            "patch": "*** Begin Patch\n*** Delete File: delete.txt\n*** End Patch\n"
        }),
        Some("workspace file was deleted through apply_patch"),
    );
    let command = tools.invoke(
        4,
        "run_command",
        json!({
            "command": if cfg!(windows) {
                "Write-Output $env:AGENT_OS_RUN_COMMAND_ENV_TEST"
            } else {
                "printf %s \"$AGENT_OS_RUN_COMMAND_ENV_TEST\""
            },
            "cwd": workspace.to_string_lossy(),
            "env": {"AGENT_OS_RUN_COMMAND_ENV_TEST": "env-visible"}
        }),
        Some("test command was executed"),
    );
    tools.invoke(
        2,
        "set_goal",
        json!({"goal": "complete integration tool-broker coverage"}),
        None,
    );
    tools.invoke(
        2,
        "update_checklist",
        json!({"items": [
            {"text": "run all model-visible tool families", "status": "completed"}
        ]}),
        None,
    );
    let recorded = tools.invoke(
        2,
        "record_evidence",
        json!({
            "evidence_type": "external_reference",
            "claim": "integration broker evidence was recorded",
            "blob_ref": "blob://integration-evidence",
            "content_hash": "integration-hash"
        }),
        None,
    );
    tools.invoke(
        1,
        "report_supervisor",
        json!({"message": "integration tool broker is progressing"}),
        None,
    );
    tools.invoke(
        2,
        "post_blackboard",
        json!({
            "channel_id": "test-results",
            "scope": "goal",
            "section": "test_result",
            "content": {"result": "tool broker integration passed"},
            "source_evidence_ids": [read.evidence_ids[0]]
        }),
        None,
    );
    tools.invoke(
        2,
        "ask_human",
        json!({
            "question": "Confirm integration broker wiring?",
            "context": {"test": "tool_broker_integration"}
        }),
        None,
    );
    let child = tools.invoke(
        4,
        "agent_control",
        json!({
            "action": "start",
            "payload": {
                "goal": "inspect integration broker child",
                "success_criteria": ["child agent was spawned"]
            }
        }),
        None,
    );
    let child_thread_id = child.output.as_ref().unwrap()["thread_id"]
        .as_str()
        .unwrap();
    let child_thread = fx
        .kernel
        .state_snapshot()
        .unwrap()
        .threads
        .get(child_thread_id)
        .cloned()
        .unwrap();
    let child_capability = fx
        .kernel
        .grant_capability(
            &child_thread.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            1,
            None,
        )
        .unwrap();
    let child_tools = ToolInvoker {
        kernel: &fx.kernel,
        agent: &child_thread,
        task_id: &fx.task.task_id,
        capability: &child_capability,
    };
    child_tools.invoke(
        1,
        "request_permissions",
        json!({
            "reason": "verify permission request tool broker path",
            "scope": "session",
            "permissions": {
                "max_risk_level": 1,
                "allowed_syscalls": ["tool.invoke"],
                "resource_scopes": ["tool:*"],
                "allowed_tool_names": ["read_file"],
                "allowed_tool_driver_classes": ["filesystem"],
                "approval_required_above": 1,
                "requires_evidence_for": []
            }
        }),
        None,
    );
    tools.invoke(
        2,
        "accomplish_goal",
        json!({"summary": "integration broker local goal complete"}),
        None,
    );
    tools.invoke(
        2,
        "submit_final",
        json!({
            "summary": "integration broker covered every model-visible tool family",
            "evidence_map": [{
                "claim": "read_file inspected the source file",
                "evidence_refs": [read.evidence_ids[0]]
            }, {
                "claim": "record_evidence stored explicit external evidence",
                "evidence_refs": [recorded.output.as_ref().unwrap()["evidence_id"].as_str().unwrap()]
            }],
            "tests_run": ["tool_broker_integration_runs_all_model_visible_tool_families"]
        }),
        None,
    );

    assert_eq!(
        fs::read_to_string(workspace.join("created.txt")).unwrap(),
        "created through integration broker\n"
    );
    assert_eq!(
        fs::read_to_string(workspace.join("edit.txt")).unwrap(),
        "alpha new beta\n"
    );
    assert!(!workspace.join("delete.txt").exists());
    assert_eq!(
        command.output.as_ref().unwrap()["stdout"]
            .as_str()
            .unwrap()
            .trim(),
        "env-visible"
    );
    assert_eq!(glob.status, ToolCallStatus::Completed);
    assert_eq!(glob.output.as_ref().unwrap()["returned_matches"], 1);
    assert_eq!(
        glob.output.as_ref().unwrap()["matches"][0]["path"],
        "read.txt"
    );
    assert_eq!(grep.status, ToolCallStatus::Completed);
    assert_eq!(grep.output.as_ref().unwrap()["returned_matches"], 1);
    assert_eq!(
        grep.output.as_ref().unwrap()["matches"][0]["path"],
        "read.txt"
    );

    let state = fx.kernel.state_snapshot().unwrap();
    let completed_tools = state
        .tool_invocations
        .values()
        .filter(|invocation| invocation.status == ToolCallStatus::Completed)
        .map(|invocation| invocation.tool_name.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        completed_tools,
        BTreeSet::from([
            "agent_control",
            "apply_patch",
            "ask_human",
            "glob_files",
            "grep_files",
            "post_blackboard",
            "read_file",
            "read_image",
            "record_evidence",
            "request_permissions",
            "report_supervisor",
            "run_command",
            "set_goal",
            "accomplish_goal",
            "submit_final",
            "update_checklist",
        ])
    );
    assert!(state.threads.contains_key(
        child.output.as_ref().unwrap()["thread_id"]
            .as_str()
            .unwrap()
    ));
    assert_eq!(image.status, ToolCallStatus::Completed);
    assert_eq!(image.output.as_ref().unwrap()["mime_type"], "image/png");
    assert_eq!(state.blackboard_entries.len(), 1);
    assert_eq!(state.final_submissions.len(), 1);
    assert!(state.agent_control_commands.values().any(|command| {
        command.action == AgentControlAction::Start
            && command.status == AgentControlCommandStatus::Applied
    }));

    let replayed = Kernel::from_events(&fx.kernel.events().unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert_eq!(
        replayed_state.final_submissions.len(),
        state.final_submissions.len()
    );
    assert_eq!(
        replayed_state.tool_invocations.len(),
        state.tool_invocations.len()
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn request_permissions_requires_direct_parent_through_broker() {
    let fx = fixture();
    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "integration-test".to_string(),
            goal: "Exercise root permission request rejection".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: Vec::new(),
        })
        .unwrap();
    let capability = fx
        .kernel
        .grant_capability(
            &supervisor.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            1,
            None,
        )
        .unwrap();

    let invocation = fx
        .kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            capability.capability_id,
            1,
            ToolInvokeInput {
                tool_name: "request_permissions".to_string(),
                input: json!({
                    "reason": "root agents have no direct parent approver",
                    "scope": "session",
                    "permissions": {
                        "max_risk_level": 1,
                        "allowed_syscalls": ["tool.invoke"],
                        "resource_scopes": ["tool:*"],
                        "allowed_tool_names": ["read_file"],
                        "allowed_tool_driver_classes": ["filesystem"],
                        "approval_required_above": 1,
                        "requires_evidence_for": []
                    }
                }),
                evidence_claim: None,
            },
        )
        .unwrap();

    assert_eq!(invocation.status, ToolCallStatus::Failed);
    assert!(invocation.evidence_ids.is_empty());
    let output = invocation.output.as_ref().unwrap();
    assert_eq!(output["status"], "failed");
    assert_eq!(output["stage"], "driver");
    assert!(output["error"]
        .as_str()
        .unwrap()
        .contains("permission requests require a direct parent approver"));
    assert!(fx
        .kernel
        .state_snapshot()
        .unwrap()
        .permission_requests
        .is_empty());
}

#[test]
fn workspace_discovery_tools_cover_parameter_semantics_through_broker() {
    let fx = fixture();
    let workspace = temp_workspace("agent-os-integration-discovery-params");
    fs::create_dir_all(workspace.join("docs")).unwrap();
    fs::create_dir_all(workspace.join("notes")).unwrap();
    fs::write(workspace.join("docs/alpha.txt"), "alpha document\n").unwrap();
    fs::write(workspace.join("docs/beta.txt"), "beta document\n").unwrap();
    fs::write(workspace.join("docs/gamma.md"), "gamma document\n").unwrap();
    fs::write(workspace.join("notes/one.txt"), "Needle uppercase\n").unwrap();
    fs::write(workspace.join("notes/two.txt"), "needle lowercase\n").unwrap();
    fs::write(workspace.join("notes/skip.md"), "needle markdown\n").unwrap();

    let worker = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_producer".to_string(),
            owner: "integration-test".to_string(),
            goal: "Exercise discovery tool parameter semantics".to_string(),
            success_criteria: vec!["discovery parameters produce auditable output".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    attach_workspace_for_agent(&fx.kernel, &worker, &fx.task.task_id, &workspace);
    let capability = fx
        .kernel
        .grant_capability(
            &worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            1,
            None,
        )
        .unwrap();
    let tools = ToolInvoker {
        kernel: &fx.kernel,
        agent: &worker,
        task_id: &fx.task.task_id,
        capability: &capability,
    };

    let glob = tools.invoke(
        1,
        "glob_files",
        json!({
            "workspace_root": workspace.to_string_lossy(),
            "path": "docs",
            "pattern": "*.txt",
            "offset": 1,
            "limit": 1
        }),
        Some("scoped glob pagination was queried"),
    );
    let glob_output = glob.output.as_ref().unwrap();
    assert_eq!(glob_output["path"], json!("docs"));
    assert_eq!(glob_output["pattern"], json!("*.txt"));
    assert_eq!(glob_output["offset"], json!(1));
    assert_eq!(glob_output["limit"], json!(1));
    assert_eq!(glob_output["total_matches"], json!(2));
    assert_eq!(glob_output["returned_matches"], json!(1));
    assert_eq!(glob_output["next_offset"], serde_json::Value::Null);
    assert_eq!(glob_output["matches"][0]["path"], json!("docs/beta.txt"));

    let grep_case_insensitive = tools.invoke(
        1,
        "grep_files",
        json!({
            "workspace_root": workspace.to_string_lossy(),
            "path": "notes",
            "include": "*.txt",
            "pattern": "needle",
            "case_sensitive": false,
            "offset": 0,
            "limit": 1
        }),
        Some("case-insensitive grep pagination was queried"),
    );
    let grep_output = grep_case_insensitive.output.as_ref().unwrap();
    assert_eq!(grep_output["path"], json!("notes"));
    assert_eq!(grep_output["include"], json!("*.txt"));
    assert_eq!(grep_output["case_sensitive"], json!(false));
    assert_eq!(grep_output["total_matches"], json!(2));
    assert_eq!(grep_output["returned_matches"], json!(1));
    assert_eq!(grep_output["next_offset"], json!(1));
    assert_eq!(grep_output["matches"][0]["path"], json!("notes/one.txt"));
    assert_eq!(grep_output["matches"][0]["line_number"], json!(1));
    assert_eq!(grep_output["matches"][0]["line"], json!("Needle uppercase"));

    let grep_case_sensitive = tools.invoke(
        1,
        "grep_files",
        json!({
            "workspace_root": workspace.to_string_lossy(),
            "path": "notes",
            "include": "*.txt",
            "pattern": "needle",
            "case_sensitive": true
        }),
        Some("case-sensitive grep was queried"),
    );
    let sensitive_output = grep_case_sensitive.output.as_ref().unwrap();
    assert_eq!(sensitive_output["case_sensitive"], json!(true));
    assert_eq!(sensitive_output["total_matches"], json!(1));
    assert_eq!(
        sensitive_output["matches"][0]["path"],
        json!("notes/two.txt")
    );

    let invalid_scope = tools.invoke(
        1,
        "glob_files",
        json!({
            "workspace_root": workspace.to_string_lossy(),
            "path": "../outside",
            "pattern": "*.txt"
        }),
        Some("invalid glob scope was rejected"),
    );
    assert_eq!(invalid_scope.status, ToolCallStatus::Failed);
    let error = invalid_scope
        .output
        .as_ref()
        .and_then(|output| output.get("error"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    assert!(
        error.contains("outside workspace root") || error.contains("workspace"),
        "{error}"
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn apply_patch_rejects_multiple_file_operations() {
    let fx = fixture();
    let workspace = temp_workspace("agent-os-integration-apply-patch-one-op");
    fs::create_dir_all(&workspace).unwrap();

    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "integration-test".to_string(),
            goal: "Exercise apply_patch one-operation contract".to_string(),
            success_criteria: vec!["multi-operation patches are rejected".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    attach_workspace_for_agent(&fx.kernel, &supervisor, &fx.task.task_id, &workspace);
    let capability = fx
        .kernel
        .grant_capability(
            &supervisor.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap();

    for patch in [
        "*** Begin Patch\n*** Add File: created.txt\n+created\n*** Delete File: created.txt\n*** End Patch\n",
        "*** Begin Patch\n*** Delete File: missing.txt\n*** Add File: other.txt\n+other\n*** End Patch\n",
    ] {
        let invocation = fx
            .kernel
            .invoke_tool(
                &supervisor.agent_id,
                &fx.task.task_id,
                &supervisor.session_id,
                capability.capability_id.clone(),
                4,
                ToolInvokeInput {
                    tool_name: "apply_patch".to_string(),
                    input: json!({
                        "workspace_root": workspace.to_string_lossy(),
                        "patch": patch
                    }),
                    evidence_claim: Some("multi-operation patch was rejected".to_string()),
                },
            )
            .unwrap();
        assert_eq!(invocation.status, ToolCallStatus::Failed);
        let error = invocation.output.unwrap()["error"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(
            error.contains("apply_patch accepts exactly one file operation"),
            "{error}"
        );
    }

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn apply_patch_reports_semantic_failures_through_broker() {
    let fx = fixture();
    let workspace = temp_workspace("agent-os-integration-apply-patch-failures");
    fs::create_dir_all(workspace.join("dir")).unwrap();
    fs::write(workspace.join("existing.txt"), "original\n").unwrap();
    fs::write(workspace.join("target.txt"), "alpha\n").unwrap();

    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "integration-test".to_string(),
            goal: "Exercise apply_patch semantic failure output".to_string(),
            success_criteria: vec!["semantic patch failures are visible".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    attach_workspace_for_agent(&fx.kernel, &supervisor, &fx.task.task_id, &workspace);
    let capability = fx
        .kernel
        .grant_capability(
            &supervisor.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap();
    let tools = ToolInvoker {
        kernel: &fx.kernel,
        agent: &supervisor,
        task_id: &fx.task.task_id,
        capability: &capability,
    };

    for (patch, expected) in [
        (
            "*** Begin Patch\n*** Add File: existing.txt\n+replacement\n*** End Patch\n",
            "apply_patch add file target already exists",
        ),
        (
            "*** Begin Patch\n*** Update File: target.txt\n@@\n-beta\n+gamma\n*** End Patch\n",
            "apply_patch update hunk did not match file content",
        ),
        (
            "*** Begin Patch\n*** Delete File: dir\n*** End Patch\n",
            "apply_patch delete operation only deletes files",
        ),
    ] {
        let invocation = tools.invoke(
            4,
            "apply_patch",
            json!({
                "workspace_root": workspace.to_string_lossy(),
                "patch": patch
            }),
            Some("apply_patch semantic failure was reported"),
        );
        assert_eq!(invocation.status, ToolCallStatus::Failed);
        assert!(invocation.evidence_ids.is_empty());
        let error = invocation
            .output
            .as_ref()
            .and_then(|output| output.get("error"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(error.contains(expected), "{error}");
    }

    assert_eq!(
        fs::read_to_string(workspace.join("existing.txt")).unwrap(),
        "original\n"
    );
    assert_eq!(
        fs::read_to_string(workspace.join("target.txt")).unwrap(),
        "alpha\n"
    );
    assert!(workspace.join("dir").is_dir());

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn apply_patch_accepts_plain_context_hunks_through_broker() {
    let fx = fixture();
    let workspace = temp_workspace("agent-os-integration-apply-patch-plain-context");
    fs::create_dir_all(workspace.join("src")).unwrap();
    fs::write(
        workspace.join("src/lib.rs"),
        "fn demo() {\n    before();\n\n    after();\n}\n",
    )
    .unwrap();

    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "integration-test".to_string(),
            goal: "Exercise apply_patch plain context hunk contract".to_string(),
            success_criteria: vec!["plain context hunks are accepted".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    attach_workspace_for_agent(&fx.kernel, &supervisor, &fx.task.task_id, &workspace);
    let capability = fx
        .kernel
        .grant_capability(
            &supervisor.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap();
    let tools = ToolInvoker {
        kernel: &fx.kernel,
        agent: &supervisor,
        task_id: &fx.task.task_id,
        capability: &capability,
    };

    let invocation = tools.invoke(
        4,
        "apply_patch",
        json!({
            "workspace_root": workspace.to_string_lossy(),
            "patch": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\nfn demo() {\n    before();\n\n+    inserted();\n    after();\n}\n*** End Patch\n"
        }),
        Some("workspace file was updated through a plain-context apply_patch hunk"),
    );

    assert_eq!(invocation.status, ToolCallStatus::Completed);
    assert_eq!(
        fs::read_to_string(workspace.join("src/lib.rs")).unwrap(),
        "fn demo() {\n    before();\n\n    inserted();\n    after();\n}\n"
    );

    let _ = fs::remove_dir_all(workspace);
}

struct ToolInvoker<'a> {
    kernel: &'a Kernel,
    agent: &'a AgentControlBlock,
    task_id: &'a str,
    capability: &'a CapabilityToken,
}

impl ToolInvoker<'_> {
    fn invoke(
        &self,
        risk_level: u8,
        tool_name: &str,
        input: Value,
        evidence_claim: Option<&str>,
    ) -> ToolInvocation {
        self.kernel
            .invoke_tool(
                &self.agent.agent_id,
                self.task_id,
                &self.agent.session_id,
                self.capability.capability_id.clone(),
                risk_level,
                ToolInvokeInput {
                    tool_name: tool_name.to_string(),
                    input,
                    evidence_claim: evidence_claim.map(str::to_string),
                },
            )
            .unwrap()
    }
}

fn attach_workspace_for_agent(
    kernel: &Kernel,
    agent: &AgentControlBlock,
    task_id: &str,
    workspace: &std::path::Path,
) {
    let env = kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            workspace.to_string_lossy(),
            "sbox_workspace_write",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    kernel
        .attach_environment(
            &env.environment_id,
            &agent.agent_id,
            &agent.thread_id,
            task_id,
            AttachMode::WorkspaceWrite,
        )
        .unwrap();
}

fn temp_workspace(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        new_id("case_")
    ))
}
