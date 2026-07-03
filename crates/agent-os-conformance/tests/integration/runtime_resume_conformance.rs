use crate::common::*;
use agent_os_store_sqlite::SqliteStore;
use agent_os_thread::{RuntimeConfig, ThreadRuntime, ToolAction};
use std::{env, fs};

#[test]
fn agent_thread_resumes_after_process_restart_with_persisted_tool_state() {
    let workspace = env::temp_dir().join(format!(
        "agent-os-conformance-runtime-resume-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    let db_path = workspace.join("agent-os.sqlite");
    fs::create_dir_all(&workspace).unwrap();

    let first_kernel = Kernel::with_replayed_store(SqliteStore::open(&db_path).unwrap()).unwrap();
    let goal = first_kernel
        .register_goal(RegisterGoalInput {
            namespace: "runtime-resume".to_string(),
            created_by: "conformance".to_string(),
            title: "Resume runtime".to_string(),
            description: "Resume runtime".to_string(),
            acceptance_criteria: vec!["thread resumes from durable events".to_string()],
            constraints: Vec::new(),
            risk_level: 4,
            deadline: None,
        })
        .unwrap();
    let task = first_kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Write result".to_string(),
            description: "Write result".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: vec![ArtifactType::Patch],
            required_evidence_types: vec![EvidenceType::DiffRef],
            priority: 10,
            risk_level: 4,
        })
        .unwrap();
    let agent = first_kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id.clone(),
            role_profile_id: "role_producer".to_string(),
            owner: "conformance".to_string(),
            goal: "write result".to_string(),
            success_criteria: vec!["result is written".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    let first_script = DeterministicModelClient::new(vec![DeterministicStep::ToolCall(
        ToolAction::new(
            "apply_patch",
            json!({
                "workspace_root": workspace.to_string_lossy(),
                "patch": "*** Begin Patch\n*** Add File: result.md\n+written before restart\n*** End Patch\n"
            }),
            4,
            Some("result was written before restart".to_string()),
        ),
    )]);
    let mut first_runtime =
        ThreadRuntime::new(first_kernel.clone(), agent.thread_id.clone(), first_script);
    assert!(first_runtime
        .run_to_completion(RuntimeConfig::workspace_write(&workspace))
        .is_err());
    assert!(workspace.join("result.md").exists());
    drop(first_runtime);
    drop(first_kernel);

    let resumed_kernel = Kernel::with_replayed_store(SqliteStore::open(&db_path).unwrap()).unwrap();
    assert_eq!(
        resumed_kernel
            .state_snapshot()
            .unwrap()
            .threads
            .get(&agent.thread_id)
            .unwrap()
            .status,
        ThreadStatus::Running
    );
    let reconciliation = resumed_kernel
        .reconcile_thread_recovery(&agent.thread_id)
        .unwrap();
    assert_eq!(reconciliation.workspace_diff_refs.len(), 1);
    resumed_kernel
        .transition_thread(
            &agent.thread_id,
            ThreadStatus::Interrupted,
            Some("process restart recovery".to_string()),
        )
        .unwrap();
    resumed_kernel
        .transition_thread(
            &agent.thread_id,
            ThreadStatus::Ready,
            Some("resume after restart".to_string()),
        )
        .unwrap();

    let final_script = DeterministicModelClient::new(vec![DeterministicStep::Final {
        summary: "Resumed from durable state.".to_string(),
        known_risks: Vec::new(),
        tests_run: Vec::new(),
        tests_not_run: Vec::new(),
    }]);
    let mut resumed_runtime = ThreadRuntime::new(
        resumed_kernel.clone(),
        agent.thread_id.clone(),
        final_script,
    );
    let report = resumed_runtime
        .run_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap();
    assert_eq!(report.status, ThreadStatus::Completed);
    assert_eq!(report.artifacts.len(), 1);
    assert_eq!(report.tool_results.len(), 1);

    let replayed_kernel =
        Kernel::with_replayed_store(SqliteStore::open(&db_path).unwrap()).unwrap();
    let replayed_state = replayed_kernel.state_snapshot().unwrap();
    assert!(replayed_state.final_submissions.contains_key(&task.task_id));
    assert_eq!(
        replayed_state.threads.get(&agent.thread_id).unwrap().status,
        ThreadStatus::Completed
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn thread_recovery_orphans_running_process_sessions_after_replay() {
    let workspace = env::temp_dir().join(format!(
        "agent-os-conformance-process-orphan-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    let fx = fixture();
    let env = fx
        .kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            workspace.to_string_lossy(),
            "sbox_workspace_write",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    fx.kernel
        .attach_environment(
            &env.environment_id,
            &fx.worker.agent_id,
            &fx.worker.thread_id,
            &fx.task.task_id,
            AttachMode::WorkspaceWrite,
        )
        .unwrap();
    let cap = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap();
    let slow_command = if cfg!(windows) {
        "Write-Output before-orphan; Start-Sleep -Seconds 30; Write-Output after-orphan"
    } else {
        "echo before-orphan; sleep 30; echo after-orphan"
    };
    let command = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            cap.capability_id,
            4,
            ToolInvokeInput {
                tool_name: "run_command".to_string(),
                input: json!({
                    "command": slow_command,
                    "cwd": workspace.to_string_lossy()
                }),
                evidence_claim: Some("background command started".to_string()),
            },
        )
        .unwrap();
    assert_eq!(command.status, ToolCallStatus::Running);
    let process_id = command
        .output
        .as_ref()
        .and_then(|output| output.get("process_id"))
        .and_then(serde_json::Value::as_str)
        .unwrap()
        .to_string();
    wait_process_state(&fx.kernel, &process_id, ProcessLifecycleState::Running);

    let replayed = Kernel::from_events(&fx.kernel.events().unwrap()).unwrap();
    let reconciliation = replayed
        .reconcile_thread_recovery(&fx.worker.thread_id)
        .unwrap();
    assert_eq!(reconciliation.orphan_tool_call_ids, vec![command.call_id]);
    assert_eq!(reconciliation.orphan_process_ids, vec![process_id.clone()]);
    let state = replayed.state_snapshot().unwrap();
    let session = state.process_sessions.get(&process_id).unwrap();
    assert_eq!(session.state, ProcessLifecycleState::Orphaned);
    assert_eq!(
        session.error.as_deref(),
        Some("process session orphaned during recovery")
    );
    assert!(session.completed_at.is_some());
    assert_eq!(
        state
            .tool_invocations
            .get(&reconciliation.orphan_tool_call_ids[0])
            .unwrap()
            .status,
        ToolCallStatus::Cancelled
    );

    let replayed_after_recovery = Kernel::from_events(&replayed.events().unwrap()).unwrap();
    let recovered_state = replayed_after_recovery.state_snapshot().unwrap();
    assert_eq!(
        recovered_state
            .process_sessions
            .get(&process_id)
            .unwrap()
            .state,
        ProcessLifecycleState::Orphaned
    );
    assert_eq!(
        recovered_state
            .reconciliation_reports
            .get(&reconciliation.reconciliation_id)
            .unwrap()
            .orphan_process_ids,
        vec![process_id.clone()]
    );

    let _ = fx
        .kernel
        .terminate_process_session(&process_id, "test cleanup");
    let _ = fs::remove_dir_all(workspace);
}

fn wait_process_state(
    kernel: &Kernel,
    process_id: &str,
    expected: ProcessLifecycleState,
) -> ProcessSession {
    let started = std::time::Instant::now();
    loop {
        let session = kernel
            .state_snapshot()
            .unwrap()
            .process_sessions
            .get(process_id)
            .unwrap()
            .clone();
        if session.state == expected {
            return session;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "process {process_id} remained in {:?}, expected {:?}",
            session.state,
            expected
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
