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
            role_profile_id: "role_worker".to_string(),
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
