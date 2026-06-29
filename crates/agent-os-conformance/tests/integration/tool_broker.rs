use crate::common::*;
use serde_json::Value;
use std::{collections::BTreeSet, fs, path::PathBuf};

#[test]
fn tool_broker_integration_runs_all_model_visible_tool_families() {
    let fx = fixture();
    let workspace = temp_workspace("agent-os-integration-tool-broker");
    fs::create_dir_all(&workspace).unwrap();
    fs::write(workspace.join("read.txt"), "read me\n").unwrap();
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
    tools.invoke(
        4,
        "write_file",
        json!({
            "workspace_root": workspace.to_string_lossy(),
            "path": "created.txt",
            "content": "created through integration broker\n"
        }),
        Some("new file was written"),
    );
    tools.invoke(
        4,
        "replace_text",
        json!({
            "workspace_root": workspace.to_string_lossy(),
            "path": "edit.txt",
            "old": "old",
            "new": "new"
        }),
        Some("workspace file was edited"),
    );
    tools.invoke(
        4,
        "delete_file",
        json!({
            "workspace_root": workspace.to_string_lossy(),
            "path": "delete.txt"
        }),
        Some("workspace file was deleted"),
    );
    tools.invoke(
        4,
        "run_command",
        json!({
            "program": std::env::current_exe().unwrap().to_string_lossy(),
            "args": ["--help"],
            "cwd": workspace.to_string_lossy()
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
        3,
        "ask_human",
        json!({
            "question": "Confirm integration broker wiring?",
            "context": {"test": "tool_broker_integration"}
        }),
        None,
    );
    tools.invoke(
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
            "ask_human",
            "delete_file",
            "post_blackboard",
            "read_file",
            "record_evidence",
            "replace_text",
            "request_permissions",
            "report_supervisor",
            "run_command",
            "set_goal",
            "accomplish_goal",
            "submit_final",
            "update_checklist",
            "write_file",
        ])
    );
    assert!(state.threads.contains_key(
        child.output.as_ref().unwrap()["thread_id"]
            .as_str()
            .unwrap()
    ));
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
