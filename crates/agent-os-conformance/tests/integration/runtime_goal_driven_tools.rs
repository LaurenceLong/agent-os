use crate::common::*;
use agent_os_thread::{RuntimeConfig, RuntimeRunOverrides, ThreadRuntime, ToolAction};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[test]
fn goal_driven_runtime_integration_covers_tools_and_agent_control_actions() {
    let fx = runtime_fixture("agent-os-runtime-integration-all-tools");
    let workspace_root = fx.workspace.to_string_lossy().to_string();
    fs::write(fx.workspace.join("read.txt"), "read me\n").unwrap();
    fs::write(fx.workspace.join("edit.txt"), "alpha old beta\n").unwrap();
    fs::write(fx.workspace.join("delete.txt"), "remove me\n").unwrap();

    let script = DeterministicModelClient::new(vec![
        tool(
            "read_file",
            json!({"workspace_root": workspace_root.clone(), "path": "read.txt"}),
            1,
        ),
        tool(
            "write_file",
            json!({
                "workspace_root": workspace_root.clone(),
                "path": "created.txt",
                "content": "created through goal-driven integration\n"
            }),
            4,
        ),
        tool(
            "replace_text",
            json!({
                "workspace_root": workspace_root.clone(),
                "path": "edit.txt",
                "old": "old",
                "new": "new"
            }),
            4,
        ),
        tool(
            "delete_file",
            json!({"workspace_root": workspace_root.clone(), "path": "delete.txt"}),
            4,
        ),
        tool(
            "run_command",
            json!({
                "program": std::env::current_exe().unwrap().to_string_lossy(),
                "args": ["--help"],
                "cwd": workspace_root.clone()
            }),
            4,
        ),
        tool(
            "set_goal",
            json!({"goal": "complete every model-visible tool in a runtime goal"}),
            2,
        ),
        tool(
            "update_checklist",
            json!({"items": [
                {"text": "exercise every model-visible tool", "status": "completed"}
            ]}),
            2,
        ),
        tool(
            "record_evidence",
            json!({
                "evidence_type": "external_reference",
                "claim": "goal-driven integration recorded explicit evidence",
                "blob_ref": "blob://goal-integration",
                "content_hash": "goal-integration-hash"
            }),
            2,
        ),
        tool(
            "report_supervisor",
            json!({"message": "goal-driven integration exercised status reporting"}),
            1,
        ),
        tool(
            "post_blackboard",
            json!({
                "channel_id": "test-results",
                "scope": "goal",
                "section": "test_result",
                "content": {"result": "goal-driven all-tool integration is running"}
            }),
            2,
        ),
        tool(
            "ask_human",
            json!({
                "question": "Confirm goal-driven integration human route wiring?",
                "context": {"test": "goal_driven_runtime_integration"}
            }),
            3,
        ),
        agent_control(
            "start",
            json!({
                "payload": {
                    "goal": "inspect child task from goal-driven integration",
                    "success_criteria": ["child was spawned"]
                }
            }),
            4,
        ),
        agent_control(
            "set_hook",
            json!({
                "thread_id": fx.resume_thread_id,
                "payload": {
                    "prompt": "Report one concise integration status sentence.",
                    "interval_seconds": 30,
                    "max_response_chars": 120
                }
            }),
            4,
        ),
        agent_control(
            "send",
            json!({
                "thread_id": fx.resume_thread_id,
                "payload": {"message": "continue the integration target task"}
            }),
            4,
        ),
        agent_control(
            "set_timeout",
            json!({
                "thread_id": fx.resume_thread_id,
                "payload": {"timeout_seconds": 90}
            }),
            4,
        ),
        agent_control(
            "delete_session",
            json!({"thread_id": fx.resume_thread_id}),
            6,
        ),
        agent_control("status", json!({"thread_id": fx.resume_thread_id}), 1),
        agent_control("output", json!({"thread_id": fx.resume_thread_id}), 1),
        agent_control("export_trace", json!({"thread_id": fx.resume_thread_id}), 1),
        agent_control("resume", json!({"thread_id": fx.resume_thread_id}), 4),
        agent_control("stop", json!({"thread_id": fx.stop_thread_id}), 4),
        agent_control("kill", json!({"thread_id": fx.kill_thread_id}), 6),
        agent_control("purge_state", json!({"thread_id": fx.purge_thread_id}), 6),
        tool(
            "accomplish_goal",
            json!({"summary": "Goal-driven runtime local goal accomplished."}),
            2,
        ),
        DeterministicStep::Final {
            summary: "Goal-driven runtime covered all model-visible tools.".to_string(),
            known_risks: Vec::new(),
            tests_run: vec![
                "goal_driven_runtime_integration_covers_tools_and_agent_control_actions"
                    .to_string(),
            ],
            tests_not_run: Vec::new(),
        },
    ]);

    let mut runtime =
        ThreadRuntime::new(fx.kernel.clone(), fx.supervisor_thread_id.clone(), script);
    let mut config = RuntimeConfig::workspace_write(&fx.workspace);
    config.max_steps = 32;
    config.tool_risk_ceiling = 6;
    config.auto_commit_patch_artifacts = false;
    let overrides = RuntimeRunOverrides {
        sandbox_profile_id: Some("sbox_workspace_write".to_string()),
        tool_approval_id: Some(fx.tool_approval_id.clone()),
    };
    let report = match runtime.run_to_completion_with_overrides(config, overrides) {
        Ok(report) => report,
        Err(error) => {
            let invocations = fx.kernel.state_snapshot().unwrap().tool_invocations;
            panic!("runtime failed: {error:?}; tool_invocations={invocations:#?}");
        }
    };

    assert_eq!(report.status, ThreadStatus::Completed);
    assert!(report.final_submitted);
    assert_eq!(report.tool_results.len(), 24);
    assert_eq!(
        fs::read_to_string(fx.workspace.join("created.txt")).unwrap(),
        "created through goal-driven integration\n"
    );
    assert_eq!(
        fs::read_to_string(fx.workspace.join("edit.txt")).unwrap(),
        "alpha new beta\n"
    );
    assert!(!fx.workspace.join("delete.txt").exists());

    let observed_tools = report
        .tool_results
        .iter()
        .map(|record| record.tool_name.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        observed_tools,
        BTreeSet::from([
            "agent_control",
            "ask_human",
            "delete_file",
            "post_blackboard",
            "read_file",
            "record_evidence",
            "replace_text",
            "report_supervisor",
            "run_command",
            "set_goal",
            "accomplish_goal",
            "update_checklist",
            "write_file",
        ])
    );

    let observed_agent_actions = report
        .tool_results
        .iter()
        .filter(|record| record.tool_name == "agent_control")
        .filter_map(|record| record.input.as_ref())
        .filter_map(|input| input.get("action"))
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        observed_agent_actions,
        BTreeSet::from([
            "export_trace",
            "delete_session",
            "kill",
            "output",
            "purge_state",
            "resume",
            "send",
            "set_hook",
            "set_timeout",
            "start",
            "status",
            "stop",
        ])
    );

    let state = fx.kernel.state_snapshot().unwrap();
    let final_submission = state.final_submissions.get(&fx.task_id).unwrap();
    assert_eq!(
        final_submission.summary,
        "Goal-driven runtime covered all model-visible tools."
    );
    assert!(final_submission.evidence_map.len() >= 5);
    assert_eq!(
        state.threads.get(&fx.resume_thread_id).unwrap().status,
        ThreadStatus::Ready
    );
    assert_eq!(
        state
            .threads
            .get(&fx.resume_thread_id)
            .unwrap()
            .budgets
            .wall_time_budget_ms,
        Some(90_000)
    );
    assert_eq!(
        state.threads.get(&fx.stop_thread_id).unwrap().status,
        ThreadStatus::Terminated
    );
    assert_eq!(
        state.threads.get(&fx.kill_thread_id).unwrap().status,
        ThreadStatus::Terminated
    );
    assert_eq!(
        state.threads.get(&fx.purge_thread_id).unwrap().status,
        ThreadStatus::Terminated
    );
    assert!(state.agent_control_commands.values().any(|command| {
        command.action == AgentControlAction::DeleteSession
            && command.status == AgentControlCommandStatus::Applied
    }));
    assert!(state.agent_control_commands.values().any(|command| {
        command.action == AgentControlAction::PurgeState
            && command.status == AgentControlCommandStatus::Applied
    }));

    write_audit_log(
        "goal-driven-all-tools-integration.jsonl",
        &[
            json!({"type": "goal", "task_id": fx.task_id, "workspace": workspace_root}),
            json!({"type": "runtime_report", "report": report}),
            json!({"type": "final_submission", "submission": final_submission}),
            json!({"type": "agent_control_actions", "actions": observed_agent_actions}),
        ],
    );
    let _ = fs::remove_dir_all(fx.workspace);
}

#[test]
fn goal_driven_runtime_integration_rejects_understated_privileged_agent_control_risk() {
    let case = RejectionCase {
        action: "kill",
        risk_level: 4,
    };
    let fx = runtime_fixture(&format!(
        "agent-os-runtime-integration-reject-{}",
        case.action
    ));
    let script = DeterministicModelClient::new(vec![agent_control(
        case.action,
        json!({"thread_id": fx.kill_thread_id}),
        case.risk_level,
    )]);
    let mut runtime =
        ThreadRuntime::new(fx.kernel.clone(), fx.supervisor_thread_id.clone(), script);
    let mut config = RuntimeConfig::workspace_write(&fx.workspace);
    config.max_steps = 2;
    config.tool_risk_ceiling = 6;
    let overrides = RuntimeRunOverrides {
        sandbox_profile_id: None,
        tool_approval_id: Some(fx.tool_approval_id.clone()),
    };
    let err = runtime
        .run_to_completion_with_overrides(config, overrides)
        .unwrap_err();
    assert!(matches!(err, AgentOsError::PermissionDenied(_)), "{err:?}");

    let state = fx.kernel.state_snapshot().unwrap();
    assert!(state.tool_invocations.values().any(|invocation| {
        invocation.tool_name == "agent_control"
            && invocation.status == ToolCallStatus::Failed
            && invocation.input.get("action").and_then(Value::as_str) == Some(case.action)
    }));
    write_audit_log(
        &format!(
            "goal-driven-agent-control-{}-rejection-integration.jsonl",
            case.action
        ),
        &[
            json!({"type": "rejection_case", "action": case.action, "risk_level": case.risk_level}),
            json!({"type": "error", "error": err.to_string()}),
            json!({"type": "tool_invocations", "invocations": state.tool_invocations}),
            json!({"type": "agent_control_commands", "commands": state.agent_control_commands}),
        ],
    );
    let _ = fs::remove_dir_all(fx.workspace);
}

#[derive(Clone, Copy)]
struct RejectionCase {
    action: &'static str,
    risk_level: u8,
}

struct RuntimeFixture {
    kernel: Kernel,
    task_id: String,
    supervisor_thread_id: String,
    resume_thread_id: String,
    stop_thread_id: String,
    kill_thread_id: String,
    purge_thread_id: String,
    tool_approval_id: String,
    workspace: PathBuf,
}

fn runtime_fixture(prefix: &str) -> RuntimeFixture {
    let workspace = temp_workspace(prefix);
    fs::create_dir_all(&workspace).unwrap();
    let kernel = Kernel::new();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "integration".to_string(),
            created_by: "agent-os-conformance".to_string(),
            title: "Goal-driven tool coverage".to_string(),
            description: "Exercise the full model-visible tool surface".to_string(),
            acceptance_criteria: vec![
                "all tools run through the runtime loop".to_string(),
                "final submission includes evidence".to_string(),
            ],
            constraints: Vec::new(),
            risk_level: 6,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Exercise tools".to_string(),
            description: "Exercise every model-visible tool".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: vec![
                EvidenceType::SourceRef,
                EvidenceType::DiffRef,
                EvidenceType::CommandLog,
            ],
            priority: 10,
            risk_level: 6,
        })
        .unwrap();
    let supervisor = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "agent-os-conformance".to_string(),
            goal: "Use every model-visible tool to complete the coverage goal".to_string(),
            success_criteria: vec!["all tool actions are observable".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    let approval = kernel
        .request_approval(RequestApprovalInput {
            goal_id: task.goal_id.clone(),
            task_id: Some(task.task_id.clone()),
            requested_by_agent_id: supervisor.agent_id.clone(),
            approval_type: ApprovalType::Human,
            scope: ApprovalScope {
                syscall_types: vec!["tool.invoke".to_string()],
                resource_scopes: vec![
                    json!("tool:*"),
                    json!("instruction:*"),
                    json!("skill:*"),
                    json!("skill_file:*"),
                    json!("mcp:*"),
                ],
                risk_ceiling: 6,
                goal_id: task.goal_id.clone(),
                task_id: Some(task.task_id.clone()),
            },
            risk_level: 6,
            expires_at: None,
        })
        .unwrap();
    kernel
        .record_approval(RecordApprovalInput {
            approval_id: approval.approval_id.clone(),
            status: ApprovalStatus::Approved,
            decision_by: "agent-os-conformance".to_string(),
            decision_reason: Some("approve bounded integration tool coverage".to_string()),
        })
        .unwrap();
    let resume_target = child_agent(
        &kernel,
        &task.task_id,
        &supervisor,
        "resume target",
        &workspace,
    );
    kernel
        .transition_thread(&resume_target.thread_id, ThreadStatus::Ready, None)
        .unwrap();
    kernel
        .transition_thread(&resume_target.thread_id, ThreadStatus::Suspended, None)
        .unwrap();
    let stop_target = child_agent(
        &kernel,
        &task.task_id,
        &supervisor,
        "stop target",
        &workspace,
    );
    let kill_target = child_agent(
        &kernel,
        &task.task_id,
        &supervisor,
        "kill target",
        &workspace,
    );
    kernel
        .transition_thread(&kill_target.thread_id, ThreadStatus::Running, None)
        .unwrap();
    let purge_target = child_agent(
        &kernel,
        &task.task_id,
        &supervisor,
        "purge target",
        &workspace,
    );

    RuntimeFixture {
        kernel,
        task_id: task.task_id,
        supervisor_thread_id: supervisor.thread_id,
        resume_thread_id: resume_target.thread_id,
        stop_thread_id: stop_target.thread_id,
        kill_thread_id: kill_target.thread_id,
        purge_thread_id: purge_target.thread_id,
        tool_approval_id: approval.approval_id,
        workspace,
    }
}

fn child_agent(
    kernel: &Kernel,
    task_id: &str,
    supervisor: &AgentControlBlock,
    goal: &str,
    workspace: &Path,
) -> AgentControlBlock {
    kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task_id.to_string(),
            role_profile_id: "role_worker".to_string(),
            owner: supervisor.agent_id.clone(),
            goal: goal.to_string(),
            success_criteria: vec!["target action is observable".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: Some(supervisor.thread_id.clone()),
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap()
}

fn tool(tool_name: &str, input: Value, risk_level: u8) -> DeterministicStep {
    DeterministicStep::ToolCall(ToolAction::new(
        tool_name,
        input,
        risk_level,
        Some(format!("{tool_name} completed in goal-driven integration")),
    ))
}

fn agent_control(action: &str, mut input: Value, risk_level: u8) -> DeterministicStep {
    input
        .as_object_mut()
        .unwrap()
        .insert("action".to_string(), Value::String(action.to_string()));
    tool("agent_control", input, risk_level)
}

fn temp_workspace(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        new_id("case_")
    ))
}

fn write_audit_log(file_name: &str, entries: &[Value]) {
    let audit_log_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit")
        .join(file_name);
    fs::create_dir_all(audit_log_path.parent().unwrap()).unwrap();
    let mut file = fs::File::create(&audit_log_path).unwrap();
    for entry in entries {
        writeln!(file, "{}", serde_json::to_string(entry).unwrap()).unwrap();
    }
    println!("integration_audit_log={}", audit_log_path.display());
}
