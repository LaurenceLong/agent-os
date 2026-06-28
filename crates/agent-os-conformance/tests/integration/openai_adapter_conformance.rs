use std::env;
use std::fs;

use crate::common;

#[test]
fn openai_adapter_pattern_drives_full_integration_task() {
    let workspace = env::temp_dir().join(format!(
        "aos-openai-integration-{}-{}",
        std::process::id(),
        common::new_id("case_")
    ));
    fs::create_dir_all(workspace.join("src")).unwrap();
    fs::write(
        workspace.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .unwrap();

    let kernel = common::Kernel::new();
    let goal = kernel
        .register_goal(common::RegisterGoalInput {
            namespace: "openai-integration".to_string(),
            created_by: "conformance".to_string(),
            title: "Change add to multiply".to_string(),
            description: "Change the add function to multiply".to_string(),
            acceptance_criteria: vec!["function returns product".to_string()],
            constraints: Vec::new(),
            risk_level: 4,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(common::SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Edit multiply".to_string(),
            description: "Change add to multiply".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: vec![common::ArtifactType::Patch],
            required_evidence_types: vec![
                common::EvidenceType::SourceRef,
                common::EvidenceType::DiffRef,
            ],
            priority: 10,
            risk_level: 4,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(common::SpawnAgentInput {
            task_id: task.task_id,
            role_profile_id: "role_worker".to_string(),
            owner: "conformance".to_string(),
            local_goal: "Change the add function to multiply".to_string(),
            success_criteria: vec!["function returns product".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();

    let script = common::DeterministicModelClient::new([
        common::DeterministicStep::ToolCall(agent_os_thread::ToolAction::new(
            "read_file",
            common::json!({
                "workspace_root": workspace.to_string_lossy(),
                "path": "src/lib.rs"
            }),
            1,
            Some("target file was inspected before edit".to_string()),
        )),
        common::DeterministicStep::ToolCall(agent_os_thread::ToolAction::new(
            "replace_text",
            common::json!({
                "workspace_root": workspace.to_string_lossy(),
                "path": "src/lib.rs",
                "old": "a + b",
                "new": "a * b"
            }),
            4,
            Some("exact edit applied: add changed to multiply".to_string()),
        )),
        common::DeterministicStep::Final {
            summary: "Changed add to multiply in src/lib.rs".to_string(),
            known_risks: Vec::new(),
            tests_run: vec!["verified file contents".to_string()],
            tests_not_run: vec!["no test suite present".to_string()],
        },
    ]);

    let mut runtime = agent_os_thread::ThreadRuntime::new(kernel.clone(), agent.thread_id, script);
    let report = runtime
        .run_to_completion(agent_os_thread::RuntimeConfig::workspace_write(&workspace))
        .unwrap();

    assert_eq!(report.status, common::ThreadStatus::Completed);
    assert!(report.final_submitted);
    assert_eq!(report.tool_results.len(), 2);
    assert_eq!(report.artifacts.len(), 1);

    let content = fs::read_to_string(workspace.join("src/lib.rs")).unwrap();
    assert!(content.contains("a * b"));
    assert!(!content.contains("a + b"));

    let replayed = common::Kernel::from_events(&kernel.events().unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert_eq!(replayed_state.final_submissions.len(), 1);
    assert_eq!(replayed_state.artifacts.len(), 1);
    assert_eq!(replayed_state.evidence.len(), 2);

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn openai_adapter_workspace_root_injection_pattern() {
    let workspace = env::temp_dir().join(format!(
        "aos-openai-inject-{}-{}",
        std::process::id(),
        common::new_id("case_")
    ));
    fs::create_dir_all(&workspace).unwrap();

    let kernel = common::Kernel::new();
    let goal = kernel
        .register_goal(common::RegisterGoalInput {
            namespace: "openai-inject".to_string(),
            created_by: "conformance".to_string(),
            title: "Write file".to_string(),
            description: "Write output file".to_string(),
            acceptance_criteria: vec!["file exists".to_string()],
            constraints: Vec::new(),
            risk_level: 4,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(common::SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Write".to_string(),
            description: "Write file".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: vec![common::ArtifactType::Patch],
            required_evidence_types: vec![common::EvidenceType::DiffRef],
            priority: 10,
            risk_level: 4,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(common::SpawnAgentInput {
            task_id: task.task_id,
            role_profile_id: "role_worker".to_string(),
            owner: "conformance".to_string(),
            local_goal: "Write hello.txt".to_string(),
            success_criteria: vec!["hello.txt exists".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();

    let ws = workspace.to_string_lossy().to_string();
    let script = common::DeterministicModelClient::new([
        common::DeterministicStep::ToolCall(agent_os_thread::ToolAction::new(
            "write_file",
            common::json!({
                "workspace_root": ws,
                "path": "hello.txt",
                "content": "Hello from agent-os!\n"
            }),
            4,
            Some("output file was created".to_string()),
        )),
        common::DeterministicStep::Final {
            summary: "Wrote hello.txt".to_string(),
            known_risks: Vec::new(),
            tests_run: vec!["verified file write".to_string()],
            tests_not_run: Vec::new(),
        },
    ]);

    let mut runtime = agent_os_thread::ThreadRuntime::new(kernel.clone(), agent.thread_id, script);
    let report = runtime
        .run_to_completion(agent_os_thread::RuntimeConfig::workspace_write(&workspace))
        .unwrap();

    assert_eq!(report.status, common::ThreadStatus::Completed);
    assert_eq!(
        fs::read_to_string(workspace.join("hello.txt")).unwrap(),
        "Hello from agent-os!\n"
    );
    assert_eq!(report.artifacts.len(), 1);

    let state = kernel.state_snapshot().unwrap();
    let invocation = state
        .tool_invocations
        .values()
        .find(|inv| {
            inv.tool_name == "write_file" && inv.status == common::ToolCallStatus::Completed
        })
        .unwrap();
    assert_eq!(invocation.status, common::ToolCallStatus::Completed);
    assert!(!invocation.evidence_ids.is_empty());

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn openai_adapter_command_execution_and_evidence() {
    let workspace = env::temp_dir().join(format!(
        "aos-openai-cmd-{}-{}",
        std::process::id(),
        common::new_id("case_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    fs::write(workspace.join("data.txt"), "value: 42\n").unwrap();

    let kernel = common::Kernel::new();
    let goal = kernel
        .register_goal(common::RegisterGoalInput {
            namespace: "openai-cmd".to_string(),
            created_by: "conformance".to_string(),
            title: "Verify data".to_string(),
            description: "Read and verify data file".to_string(),
            acceptance_criteria: vec!["command executed and evidence attached".to_string()],
            constraints: Vec::new(),
            risk_level: 4,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(common::SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Read and verify".to_string(),
            description: "Run command to verify".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: vec![common::ArtifactType::Patch],
            required_evidence_types: vec![
                common::EvidenceType::SourceRef,
                common::EvidenceType::CommandLog,
            ],
            priority: 10,
            risk_level: 4,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(common::SpawnAgentInput {
            task_id: task.task_id,
            role_profile_id: "role_worker".to_string(),
            owner: "conformance".to_string(),
            local_goal: "Read data.txt and run echo to verify".to_string(),
            success_criteria: vec!["command executed".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();

    let current_exe = env::current_exe().unwrap();
    let ws = workspace.to_string_lossy().to_string();
    let script = common::DeterministicModelClient::new([
        common::DeterministicStep::ToolCall(agent_os_thread::ToolAction::new(
            "read_file",
            common::json!({
                "workspace_root": ws,
                "path": "data.txt"
            }),
            1,
            Some("data file was read".to_string()),
        )),
        common::DeterministicStep::ToolCall(agent_os_thread::ToolAction::new(
            "write_file",
            common::json!({
                "workspace_root": ws,
                "path": "verified.txt",
                "content": "verified: 42\n"
            }),
            4,
            Some("verification result was written".to_string()),
        )),
        common::DeterministicStep::ToolCall(agent_os_thread::ToolAction::new(
            "run_command",
            common::json!({
                "program": current_exe.to_string_lossy(),
                "args": ["--help"],
                "cwd": ws
            }),
            4,
            Some("verification command was executed".to_string()),
        )),
        common::DeterministicStep::Final {
            summary: "Read data file, wrote verification result, and ran verification command"
                .to_string(),
            known_risks: Vec::new(),
            tests_run: vec!["echo verification".to_string()],
            tests_not_run: Vec::new(),
        },
    ]);

    let mut runtime = agent_os_thread::ThreadRuntime::new(kernel.clone(), agent.thread_id, script);
    let report = runtime
        .run_to_completion(agent_os_thread::RuntimeConfig {
            fail_on_process_nonzero: false,
            ..agent_os_thread::RuntimeConfig::workspace_write(&workspace)
        })
        .unwrap();

    assert_eq!(report.status, common::ThreadStatus::Completed);
    assert_eq!(report.tool_results.len(), 3);

    let state = kernel.state_snapshot().unwrap();
    let evidence_types: Vec<_> = report
        .tool_results
        .iter()
        .flat_map(|r| r.evidence_ids.iter())
        .filter_map(|eid| state.evidence.get(eid))
        .map(|e| e.evidence_type)
        .collect();
    assert!(evidence_types.contains(&common::EvidenceType::SourceRef));
    assert!(evidence_types.contains(&common::EvidenceType::CommandLog));

    let _ = fs::remove_dir_all(workspace);
}
