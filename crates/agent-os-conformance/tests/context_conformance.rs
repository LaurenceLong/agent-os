mod common;
use common::*;

#[test]
fn context_load_creates_immutable_snapshots_and_replays_staleness() {
    let fx = fixture();
    let explorer = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_worker".to_string(),
            owner: "tester".to_string(),
            local_goal: "inspect scoped context".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: Vec::new(),
        })
        .unwrap();
    let cap = fx
        .kernel
        .grant_capability(
            &explorer.agent_id,
            &fx.task.task_id,
            vec!["context.load".to_string()],
            vec!["read:*".to_string()],
            1,
            None,
        )
        .unwrap();

    let first = SyscallEnvelope::new(
        "context.load",
        explorer.agent_id.clone(),
        fx.task.task_id.clone(),
        explorer.session_id.clone(),
        Some(cap.capability_id.clone()),
        1,
        serde_json::to_value(LoadContextInput {
            agent_id: explorer.agent_id.clone(),
            task_id: fx.task.task_id.clone(),
            loaded_refs: vec!["docs/10-kernel-design/kernel-data-model.md".to_string()],
            summary_artifact_id: None,
            freshness: ContextFreshness::Fresh,
            pollution_score: 0.0,
            token_estimate: 1024,
        })
        .unwrap(),
    );
    let first_result = fx.kernel.handle_syscall(first).unwrap();
    assert!(first_result.accepted);

    let second = SyscallEnvelope::new(
        "context.load",
        explorer.agent_id.clone(),
        fx.task.task_id.clone(),
        explorer.session_id,
        Some(cap.capability_id),
        1,
        serde_json::to_value(LoadContextInput {
            agent_id: explorer.agent_id,
            task_id: fx.task.task_id.clone(),
            loaded_refs: vec!["docs/10-kernel-design/kernel-data-model.md".to_string()],
            summary_artifact_id: None,
            freshness: ContextFreshness::Stale,
            pollution_score: 0.2,
            token_estimate: 512,
        })
        .unwrap(),
    );
    let second_result = fx.kernel.handle_syscall(second).unwrap();
    assert!(second_result.accepted);

    let state = fx.kernel.state_snapshot().unwrap();
    assert_eq!(state.context_snapshots.len(), 2);
    assert!(state
        .context_snapshots
        .values()
        .any(|snapshot| snapshot.freshness == ContextFreshness::Fresh));
    assert!(state
        .context_snapshots
        .values()
        .any(|snapshot| snapshot.freshness == ContextFreshness::Stale));

    let replayed = Kernel::from_events(&fx.kernel.events().unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert_eq!(replayed_state.context_snapshots.len(), 2);
    assert!(replayed_state
        .context_snapshots
        .values()
        .any(|snapshot| snapshot.freshness == ContextFreshness::Stale));
}

#[test]
fn context_load_requires_agent_task_scope_and_context_material() {
    let fx = fixture();
    let cap = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["context.load".to_string()],
            vec!["read:*".to_string()],
            1,
            None,
        )
        .unwrap();
    let empty = SyscallEnvelope::new(
        "context.load",
        fx.worker.agent_id.clone(),
        fx.task.task_id.clone(),
        fx.worker.session_id.clone(),
        Some(cap.capability_id.clone()),
        1,
        serde_json::to_value(LoadContextInput {
            agent_id: fx.worker.agent_id.clone(),
            task_id: fx.task.task_id.clone(),
            loaded_refs: Vec::new(),
            summary_artifact_id: None,
            freshness: ContextFreshness::Unknown,
            pollution_score: 0.0,
            token_estimate: 0,
        })
        .unwrap(),
    );
    let empty_err = fx.kernel.handle_syscall(empty).unwrap_err();
    assert!(matches!(empty_err, AgentOsError::Validation(_)));

    let wrong_agent = SyscallEnvelope::new(
        "context.load",
        fx.worker.agent_id.clone(),
        fx.task.task_id.clone(),
        fx.worker.session_id.clone(),
        Some(cap.capability_id),
        1,
        serde_json::to_value(LoadContextInput {
            agent_id: "agt_wrong".to_string(),
            task_id: fx.task.task_id,
            loaded_refs: vec!["docs/README.md".to_string()],
            summary_artifact_id: None,
            freshness: ContextFreshness::Fresh,
            pollution_score: 0.0,
            token_estimate: 128,
        })
        .unwrap(),
    );
    let wrong_agent_err = fx.kernel.handle_syscall(wrong_agent).unwrap_err();
    assert!(matches!(wrong_agent_err, AgentOsError::NotFound(_)));
}
