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
fn emit_updates_store_projections_without_rebuild() {
    let (kernel, _, _, agent, _) = fixture();

    let started = kernel.start_turn(&agent.thread_id).unwrap();
    let store = kernel.store();

    let threads = store.thread_summaries().unwrap();
    assert!(threads
        .iter()
        .any(|thread| thread.client_thread_id == agent.thread_id));

    let turns = store.turn_summaries().unwrap();
    assert_eq!(turns.len(), 1);
    assert_eq!(
        turns[0].turn_id,
        started.active_turn.turn_id.expect("started turn id")
    );

    let timeline = store.timeline_items(Some(&agent.thread_id)).unwrap();
    assert!(timeline
        .iter()
        .any(|item| item.item_type == TimelineItemType::TurnStarted));
}

#[test]
fn resource_session_lifecycle_projects_and_replays() {
    let (kernel, _, _, agent, _) = fixture();

    let session = kernel
        .open_resource_session(OpenResourceSessionInput {
            resource_type: ResourceSessionType::Terminal,
            client_thread_id: Some(agent.thread_id.clone()),
            owner_agent_id: Some(agent.agent_id.clone()),
            lease_expires_at: None,
            payload: json!({"cwd": "workspace", "shell": "powershell"}),
        })
        .unwrap();

    assert_eq!(session.status, ResourceSessionStatus::Active);
    assert_eq!(
        kernel
            .state_snapshot()
            .unwrap()
            .resource_sessions
            .get(&session.session_id)
            .unwrap()
            .resource_type,
        ResourceSessionType::Terminal
    );
    let projected = kernel.store().resource_sessions().unwrap();
    assert!(projected.iter().any(|resource| {
        resource.session_id == session.session_id
            && resource.resource_type == "terminal"
            && resource.status == "active"
            && resource.client_thread_id.as_deref() == Some(agent.thread_id.as_str())
    }));

    let closed = kernel.close_resource_session(&session.session_id).unwrap();
    assert_eq!(closed.status, ResourceSessionStatus::Closed);
    let projected = kernel.store().resource_sessions().unwrap();
    assert!(projected.iter().any(|resource| {
        resource.session_id == session.session_id && resource.status == "closed"
    }));

    let replayed = Kernel::from_events(&kernel.events().unwrap()).unwrap();
    let replayed_session = replayed
        .state_snapshot()
        .unwrap()
        .resource_sessions
        .get(&session.session_id)
        .cloned()
        .unwrap();
    assert_eq!(replayed_session.status, ResourceSessionStatus::Closed);
    assert_eq!(replayed.store().resource_sessions().unwrap(), projected);
}

#[test]
fn automation_schedule_run_projects_and_replays() {
    let (kernel, _, _, agent, _) = fixture();

    let schedule = kernel
        .create_automation_schedule(CreateAutomationScheduleInput {
            name: "wake thread".to_string(),
            kind: AutomationScheduleKind::ThreadWakeup,
            target_thread_id: Some(agent.thread_id.clone()),
            workspace: None,
            prompt: "continue scheduled work".to_string(),
            next_run_at: Some("2026-06-30T00:00:00Z".to_string()),
            interval_seconds: None,
            created_by_client_id: "human_1".to_string(),
            payload: json!({"source": "unit"}),
        })
        .unwrap();
    let run = kernel
        .queue_automation_run(&schedule.schedule_id, "2026-06-30T00:00:00Z")
        .unwrap();

    let state = kernel.state_snapshot().unwrap();
    assert!(state
        .automation_schedules
        .get(&schedule.schedule_id)
        .unwrap()
        .next_run_at
        .is_none());
    assert_eq!(
        state.automation_runs.get(&run.run_id).unwrap().status,
        AutomationRunStatus::Queued
    );
    assert!(kernel
        .store()
        .automation_runs()
        .unwrap()
        .iter()
        .any(|projected| projected.run_id == run.run_id
            && projected.target_thread_id.as_deref() == Some(agent.thread_id.as_str())));

    let replayed = Kernel::from_events(&kernel.events().unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert!(replayed_state
        .automation_schedules
        .get(&schedule.schedule_id)
        .unwrap()
        .next_run_at
        .is_none());
    assert!(replayed
        .store()
        .automation_runs()
        .unwrap()
        .iter()
        .any(|projected| projected.run_id == run.run_id));
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
