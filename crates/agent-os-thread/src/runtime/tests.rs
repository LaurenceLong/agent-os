use super::*;
use crate::{ScriptedModelClient, ScriptedStep};
use agent_os_kernel::{RegisterGoalInput, SpawnAgentInput, SpawnTaskInput};
use std::{env, fs};

#[test]
fn scripted_runtime_finishes_code_task_through_tool_loop() {
    let workspace = env::temp_dir().join(format!(
        "agent-os-thread-runtime-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    fs::create_dir_all(workspace.join("src")).unwrap();
    fs::write(
        workspace.join("src/lib.rs"),
        "pub fn answer() -> i32 { 1 }\n",
    )
    .unwrap();
    let kernel = Kernel::new();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "runtime-test".to_string(),
            created_by: "agent-os-thread-test".to_string(),
            title: "Change answer".to_string(),
            description: "Change answer from one to two".to_string(),
            acceptance_criteria: vec!["test command passes".to_string()],
            constraints: Vec::new(),
            risk_level: 4,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Apply code edit".to_string(),
            description: "Change answer from one to two".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: vec![ArtifactType::Patch],
            required_evidence_types: vec![
                EvidenceType::SourceRef,
                EvidenceType::DiffRef,
                EvidenceType::CommandLog,
            ],
            priority: 10,
            risk_level: 4,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id,
            role_profile_id: "role_worker".to_string(),
            owner: "agent-os-thread-test".to_string(),
            local_goal: "Change answer from one to two".to_string(),
            success_criteria: vec!["test command passes".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    let current_exe = env::current_exe().unwrap();
    let script = ScriptedModelClient::new(vec![
        ScriptedStep::ToolCall(ToolAction::new(
            "read_file",
            json!({
                "workspace_root": workspace.to_string_lossy(),
                "path": "src/lib.rs"
            }),
            1,
            Some("target file was inspected before edit".to_string()),
        )),
        ScriptedStep::ToolCall(ToolAction::new(
            "replace_text",
            json!({
                "workspace_root": workspace.to_string_lossy(),
                "path": "src/lib.rs",
                "old": "1",
                "new": "2"
            }),
            4,
            Some("exact repository edit was applied".to_string()),
        )),
        ScriptedStep::ToolCall(ToolAction::new(
            "run_command",
            json!({
                "program": current_exe.to_string_lossy(),
                "args": ["--help"],
                "cwd": workspace.to_string_lossy()
            }),
            4,
            Some("test command was run after edit".to_string()),
        )),
        ScriptedStep::Final {
            summary: "Applied exact edit and verified it.".to_string(),
            known_risks: Vec::new(),
            tests_run: vec!["test binary --help".to_string()],
            tests_not_run: Vec::new(),
        },
    ]);
    let mut runtime = ThreadRuntime::new(kernel.clone(), agent.thread_id, script);
    let report = runtime
        .run_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap();
    assert_eq!(report.status, ThreadStatus::Completed);
    assert!(report.final_submitted);
    assert_eq!(report.artifacts.len(), 1);
    assert_eq!(report.tool_results.len(), 3);
    assert_eq!(
        fs::read_to_string(workspace.join("src/lib.rs")).unwrap(),
        "pub fn answer() -> i32 { 2 }\n"
    );
    let replayed = Kernel::from_events(&kernel.events().unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert_eq!(replayed_state.final_submissions.len(), 1);
    assert_eq!(replayed_state.artifacts.len(), 1);
    assert!(replayed_state
        .tool_invocations
        .values()
        .any(|invocation| invocation.status == ToolCallStatus::Proposed));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_resumes_with_persisted_tool_results_and_artifacts() {
    let workspace = env::temp_dir().join(format!(
        "agent-os-thread-runtime-resume-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    let kernel = Kernel::new();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "runtime-resume-test".to_string(),
            created_by: "agent-os-thread-test".to_string(),
            title: "Resume write".to_string(),
            description: "Write file before restart, final after restart".to_string(),
            acceptance_criteria: vec!["final can use persisted evidence".to_string()],
            constraints: Vec::new(),
            risk_level: 4,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Write file".to_string(),
            description: "Write file".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: vec![ArtifactType::Patch],
            required_evidence_types: vec![EvidenceType::DiffRef],
            priority: 10,
            risk_level: 4,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id.clone(),
            role_profile_id: "role_worker".to_string(),
            owner: "agent-os-thread-test".to_string(),
            local_goal: "Write result.md".to_string(),
            success_criteria: vec!["result.md exists".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();

    let first_script = ScriptedModelClient::new(vec![ScriptedStep::ToolCall(ToolAction::new(
        "write_file",
        json!({
            "workspace_root": workspace.to_string_lossy(),
            "path": "result.md",
            "content": "persisted before restart\n"
        }),
        4,
        Some("result file was written before restart".to_string()),
    ))]);
    let mut first_runtime =
        ThreadRuntime::new(kernel.clone(), agent.thread_id.clone(), first_script);
    let err = first_runtime
        .run_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap_err();
    assert!(matches!(err, AgentOsError::Validation(_)));
    assert!(workspace.join("result.md").exists());
    assert_eq!(kernel.state_snapshot().unwrap().artifacts.len(), 1);

    kernel
        .transition_thread(
            &agent.thread_id,
            ThreadStatus::Interrupted,
            Some("simulated process restart".to_string()),
        )
        .unwrap();
    kernel
        .transition_thread(
            &agent.thread_id,
            ThreadStatus::Ready,
            Some("resume after restart".to_string()),
        )
        .unwrap();

    let second_script = ScriptedModelClient::new(vec![ScriptedStep::Final {
        summary: "Resumed and submitted final.".to_string(),
        known_risks: Vec::new(),
        tests_run: Vec::new(),
        tests_not_run: Vec::new(),
    }]);
    let mut second_runtime =
        ThreadRuntime::new(kernel.clone(), agent.thread_id.clone(), second_script);
    let report = second_runtime
        .run_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap();
    assert_eq!(report.status, ThreadStatus::Completed);
    assert_eq!(report.tool_results.len(), 1);
    assert_eq!(report.artifacts.len(), 1);
    let state = kernel.state_snapshot().unwrap();
    assert_eq!(state.final_submissions.len(), 1);
    let final_submission = state.final_submissions.get(&task.task_id).unwrap();
    assert_eq!(final_submission.changed_artifacts.len(), 1);
    assert_eq!(final_submission.evidence_map.len(), 1);
    let _ = fs::remove_dir_all(workspace);
}
