use agent_os_kernel::*;
use agent_os_sys::*;
use serde_json::json;

fn fixture() -> (Kernel, Goal, Task, AgentControlBlock, CapabilityToken) {
    let kernel = Kernel::new();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "test".to_string(),
            created_by: "user".to_string(),
            title: "Build".to_string(),
            description: "Build a thing".to_string(),
            acceptance_criteria: vec!["works".to_string()],
            constraints: Vec::new(),
            risk_level: 3,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id.clone(),
            parent_task_id: None,
            title: "Patch".to_string(),
            description: "Patch files".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: Vec::new(),
            priority: 10,
            risk_level: 3,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id.clone(),
            role_profile_id: "role_worker".to_string(),
            owner: "user".to_string(),
            goal: "patch".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: Vec::new(),
        })
        .unwrap();
    let cap = kernel
        .grant_capability(
            &agent.agent_id,
            &task.task_id,
            vec!["evidence.attach".to_string(), "artifact.commit".to_string()],
            vec!["artifact:*".to_string()],
            3,
            None,
        )
        .unwrap();
    (kernel, goal, task, agent, cap)
}

#[test]
fn replay_rebuilds_projection() {
    let (kernel, _, _, _, _) = fixture();
    let events = kernel.events().unwrap();
    let replayed = Kernel::from_events(&events).unwrap();
    assert_eq!(
        replayed.state_snapshot().unwrap().threads.len(),
        kernel.state_snapshot().unwrap().threads.len()
    );
}

#[test]
fn invalid_transition_is_rejected() {
    let (kernel, _, _, agent, _) = fixture();
    kernel
        .transition_thread(&agent.thread_id, ThreadStatus::Ready, None)
        .unwrap();
    kernel
        .transition_thread(&agent.thread_id, ThreadStatus::Running, None)
        .unwrap();
    kernel
        .transition_thread(&agent.thread_id, ThreadStatus::Completed, None)
        .unwrap();
    let err = kernel
        .transition_thread(&agent.thread_id, ThreadStatus::Running, None)
        .unwrap_err();
    assert!(matches!(err, AgentOsError::InvalidTransition(_)));
}

#[test]
fn syscall_without_capability_is_rejected() {
    let (kernel, _, task, agent, _) = fixture();
    let syscall = SyscallEnvelope::new(
        "evidence.attach",
        agent.agent_id,
        task.task_id,
        agent.session_id,
        None,
        1,
        json!({}),
    );
    let err = kernel.handle_syscall(syscall).unwrap_err();
    assert!(matches!(err, AgentOsError::PermissionDenied(_)));
}
