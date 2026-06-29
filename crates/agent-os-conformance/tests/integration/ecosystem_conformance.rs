use crate::common::*;
use agent_os_thread::{expand_command_template, import_workspace_ecosystem, ModelTurnRequest};
use std::fs;
use std::process::Command;
use std::sync::{Arc, Mutex};

#[test]
fn ecosystem_imports_project_sources_and_replays_kernel_events() {
    let workspace = temp_workspace("agent-os-ecosystem-import");
    fs::create_dir_all(workspace.join(".agent-os/skills/review-skill/resources")).unwrap();
    fs::create_dir_all(workspace.join(".agent-os/commands/code")).unwrap();
    fs::create_dir_all(workspace.join(".agent-os/agents")).unwrap();
    fs::write(workspace.join("AGENTS.md"), "Project rule: read first.\n").unwrap();
    fs::write(
        workspace.join(".agent-os/skills/review-skill/SKILL.md"),
        "---\nname: review-skill\ndescription: Review code with local criteria.\n---\nRead resources/checklist.md before reporting.\n",
    )
    .unwrap();
    fs::write(
        workspace.join(".agent-os/skills/review-skill/resources/checklist.md"),
        "Check risk, tests, and scope.\n",
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

    let kernel = Kernel::new();
    let report = import_workspace_ecosystem(&kernel, &workspace).unwrap();
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

    let replayed = Kernel::from_events(&kernel.events().unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
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
fn ecosystem_rejects_duplicate_skill_names_from_different_sources() {
    let workspace = temp_workspace("agent-os-ecosystem-duplicate-skill");
    for root in [".agent-os", ".opencode"] {
        fs::create_dir_all(workspace.join(root).join("skills/dupe")).unwrap();
        fs::write(
            workspace.join(root).join("skills/dupe/SKILL.md"),
            format!(
                "---\nname: dupe\ndescription: {root} skill.\n---\nDifferent content from {root}.\n"
            ),
        )
        .unwrap();
    }

    let error = import_workspace_ecosystem(&Kernel::new(), &workspace).unwrap_err();
    assert!(
        matches!(&error, AgentOsError::Validation(message) if message.contains("duplicate skill name dupe")),
        "{error:?}"
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn ecosystem_rejects_command_shell_interpolation() {
    let workspace = temp_workspace("agent-os-ecosystem-command-reject");
    fs::create_dir_all(workspace.join(".opencode/commands")).unwrap();
    fs::write(
        workspace.join(".opencode/commands/unsafe.md"),
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
        "---\nname: review-skill\ndescription: Review code with local criteria.\n---\nUse resources/checklist.md.\n",
    )
    .unwrap();
    fs::write(
        workspace.join(".agent-os/skills/review-skill/resources/checklist.md"),
        "Check risk, tests, and scope.\n",
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
                input: json!({"name": "review-skill"}),
                evidence_claim: Some("skill loaded".to_string()),
            },
        )
        .unwrap();
    assert_eq!(loaded.output.unwrap()["name"], json!("review-skill"));

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
                input: json!({"name": "review-skill", "path": "resources/checklist.md"}),
                evidence_claim: Some("resource loaded".to_string()),
            },
        )
        .unwrap();
    assert_eq!(resource.output.unwrap()["bytes_read"], json!(30));

    let denied = fx.kernel.invoke_tool(
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
    );
    assert!(matches!(denied, Err(AgentOsError::Validation(_))));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn local_stdio_mcp_registers_and_executes_with_kernel_permissions() {
    let workspace = temp_workspace("agent-os-ecosystem-mcp");
    fs::create_dir_all(&workspace).unwrap();
    let server = compile_mcp_fixture(&workspace);
    fs::write(
        workspace.join("agent-os.json"),
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
    let state = fx.kernel.state_snapshot().unwrap();
    let tool = state.mcp_tools.get("mcp__echo__echo").unwrap();
    assert_eq!(tool.server_name, "echo");
    assert!(state.tool_descriptors.contains_key("mcp__echo__echo"));

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
            role_profile_id: "role_worker".to_string(),
            owner: "conformance".to_string(),
            goal: "Inspect projected ecosystem".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    let seen = Arc::new(Mutex::new(None));
    let model = CapturingModel {
        seen: Arc::clone(&seen),
    };
    let mut runtime = agent_os_thread::ThreadRuntime::new(kernel, worker.thread_id, model);
    let mut config = agent_os_thread::RuntimeConfig::workspace_write(&workspace);
    config.max_steps = 1;
    let err = runtime.run_to_completion(config).unwrap_err();
    assert!(matches!(err, AgentOsError::Validation(_)));
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
