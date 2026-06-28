use crate::common::*;

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

#[test]
fn memory_write_policy_requires_provenance_and_gates_proposed_to_active() {
    let fx = fixture();
    // Provenance is required: a proposed memory without source evidence is rejected.
    let no_provenance = fx
        .kernel
        .propose_memory_write(ProposeMemoryWriteInput {
            namespace: "decisions".to_string(),
            content: json!({"note": "no provenance"}),
            created_by_agent_id: fx.worker.agent_id.clone(),
            source_evidence_ids: Vec::new(),
        })
        .unwrap_err();
    assert!(matches!(no_provenance, AgentOsError::Validation(_)));
    let missing_provenance = fx
        .kernel
        .propose_memory_write(ProposeMemoryWriteInput {
            namespace: "decisions".to_string(),
            content: json!({"note": "missing provenance record"}),
            created_by_agent_id: fx.worker.agent_id.clone(),
            source_evidence_ids: vec!["evd_missing".to_string()],
        })
        .unwrap_err();
    assert!(matches!(missing_provenance, AgentOsError::NotFound(_)));

    // Attach evidence to serve as provenance, then propose.
    let evidence = fx
        .kernel
        .attach_evidence(evidence_input(&fx, EvidenceType::SourceRef))
        .unwrap();
    let proposed = fx
        .kernel
        .propose_memory_write(ProposeMemoryWriteInput {
            namespace: "decisions".to_string(),
            content: json!({"decision": "adopt typed agent items"}),
            created_by_agent_id: fx.worker.agent_id.clone(),
            source_evidence_ids: vec![evidence.evidence_id.clone()],
        })
        .unwrap();
    assert_eq!(proposed.status, MemoryStatus::Proposed);

    // Proposed memory is not yet authoritative: it is not active.
    let state = fx.kernel.state_snapshot().unwrap();
    assert!(state
        .memory_records
        .get(&proposed.memory_id)
        .is_some_and(
            |record| record.status == MemoryStatus::Proposed && record.activated_at.is_none()
        ));

    // Commit activates it.
    let activated = fx
        .kernel
        .commit_memory_write(CommitMemoryWriteInput {
            memory_id: proposed.memory_id.clone(),
            approved_by: "tester".to_string(),
        })
        .unwrap();
    assert_eq!(activated.status, MemoryStatus::Active);
    assert!(activated.activated_at.is_some());

    // Replay preserves the proposed->active provenance.
    let replayed = Kernel::from_events(&fx.kernel.events().unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    let replayed_record = replayed_state
        .memory_records
        .get(&proposed.memory_id)
        .unwrap();
    assert_eq!(replayed_record.status, MemoryStatus::Active);
    assert_eq!(
        replayed_record.source_evidence_ids,
        vec![evidence.evidence_id]
    );

    // Invalidation keeps the record auditable but not authoritative.
    let invalidated = fx.kernel.invalidate_memory(&proposed.memory_id).unwrap();
    assert_eq!(invalidated.status, MemoryStatus::Invalidated);
}

#[test]
fn context_compaction_records_replacement_provenance() {
    let fx = fixture();
    let compaction = fx
        .kernel
        .compact_context(CompactContextInput {
            thread_id: fx.worker.thread_id.clone(),
            agent_id: fx.worker.agent_id.clone(),
            task_id: fx.task.task_id.clone(),
            summary_artifact_id: None,
            superseded_refs: vec!["ctx_old_1".to_string(), "ctx_old_2".to_string()],
            token_estimate: 2048,
        })
        .unwrap();
    assert_eq!(compaction.superseded_refs.len(), 2);
    assert_eq!(compaction.token_estimate, 2048);

    let state = fx.kernel.state_snapshot().unwrap();
    assert_eq!(state.context_compactions.len(), 1);
    let recorded = state
        .context_compactions
        .get(&compaction.compaction_id)
        .unwrap();
    assert_eq!(recorded.superseded_refs, compaction.superseded_refs);

    // Replay preserves the compaction provenance.
    let replayed = Kernel::from_events(&fx.kernel.events().unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert!(replayed_state
        .context_compactions
        .contains_key(&compaction.compaction_id));
}

#[test]
fn context_commit_summary_syscall_records_replacement_provenance() {
    let fx = fixture();
    let cap = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["context.commit_summary".to_string()],
            vec!["context:*".to_string()],
            1,
            None,
        )
        .unwrap();
    let syscall = SyscallEnvelope::new(
        "context.commit_summary",
        fx.worker.agent_id.clone(),
        fx.task.task_id.clone(),
        fx.worker.session_id.clone(),
        Some(cap.capability_id),
        1,
        serde_json::to_value(CompactContextInput {
            thread_id: fx.worker.thread_id.clone(),
            agent_id: fx.worker.agent_id.clone(),
            task_id: fx.task.task_id.clone(),
            summary_artifact_id: None,
            superseded_refs: vec!["ctx_a".to_string(), "ctx_b".to_string()],
            token_estimate: 512,
        })
        .unwrap(),
    );
    let result = fx.kernel.handle_syscall(syscall).unwrap();
    assert!(result.accepted);
    assert_eq!(
        fx.kernel
            .state_snapshot()
            .unwrap()
            .context_compactions
            .len(),
        1
    );
}

#[test]
fn context_invalidate_marks_stale_without_silent_reuse() {
    let fx = fixture();
    let snapshot = fx
        .kernel
        .load_context(LoadContextInput {
            agent_id: fx.worker.agent_id.clone(),
            task_id: fx.task.task_id.clone(),
            loaded_refs: vec!["docs/README.md".to_string()],
            summary_artifact_id: None,
            freshness: ContextFreshness::Fresh,
            pollution_score: 0.0,
            token_estimate: 256,
        })
        .unwrap();
    let invalidated = fx.kernel.invalidate_context(&snapshot.context_id).unwrap();
    assert_eq!(invalidated.freshness, ContextFreshness::Stale);
    assert!(invalidated.invalidated_at.is_some());
}
