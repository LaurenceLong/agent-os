use crate::common::*;
use agent_os_store::{EventStore, IdempotencyStore};
use agent_os_store_sqlite::SqliteStore;
use std::{env, fs};

#[test]
fn resource_conflicts_are_deterministic_and_auditable() {
    let fx = fixture();
    fx.kernel
        .request_resource_lease(
            ResourceType::File,
            "src/lib.rs",
            fx.worker.agent_id.clone(),
            fx.worker.thread_id.clone(),
            fx.goal.goal_id.clone(),
            fx.task.task_id.clone(),
            LeaseMode::Exclusive,
            Some("edit".to_string()),
        )
        .unwrap();
    let err = fx
        .kernel
        .request_resource_lease(
            ResourceType::File,
            "src/lib.rs",
            fx.worker.agent_id.clone(),
            fx.worker.thread_id.clone(),
            fx.goal.goal_id.clone(),
            fx.task.task_id.clone(),
            LeaseMode::Shared,
            Some("read while edit".to_string()),
        )
        .unwrap_err();
    assert!(matches!(err, AgentOsError::ResourceConflict(_)));
    assert!(fx
        .kernel
        .state_snapshot()
        .unwrap()
        .resource_leases
        .values()
        .any(|lease| lease.status == ResourceLeaseStatus::Denied));
}

#[test]
fn resource_release_reopens_conflict_domain_and_replays() {
    let fx = fixture();
    let lease = fx
        .kernel
        .request_resource_lease(
            ResourceType::File,
            "src/lib.rs",
            fx.worker.agent_id.clone(),
            fx.worker.thread_id.clone(),
            fx.goal.goal_id.clone(),
            fx.task.task_id.clone(),
            LeaseMode::Exclusive,
            Some("edit".to_string()),
        )
        .unwrap();
    fx.kernel
        .release_resource_lease(&lease.resource_lease_id)
        .unwrap();
    let shared = fx
        .kernel
        .request_resource_lease(
            ResourceType::File,
            "src/lib.rs",
            fx.worker.agent_id.clone(),
            fx.worker.thread_id.clone(),
            fx.goal.goal_id.clone(),
            fx.task.task_id.clone(),
            LeaseMode::Shared,
            Some("read after release".to_string()),
        )
        .unwrap();
    assert_eq!(shared.status, ResourceLeaseStatus::Granted);
    assert!(fx
        .kernel
        .events()
        .unwrap()
        .iter()
        .any(|event| event.event_type == "ResourceLeaseReleased"));

    let replayed = Kernel::from_events(&fx.kernel.events().unwrap()).unwrap();
    assert_eq!(
        replayed
            .state_snapshot()
            .unwrap()
            .resource_leases
            .get(&lease.resource_lease_id)
            .unwrap()
            .status,
        ResourceLeaseStatus::Released
    );
}

#[test]
fn environment_attach_and_release_emit_durable_events() {
    let fx = fixture();
    let env = fx
        .kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            "rust-workspace",
            "sbox_workspace_write",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    let lease = fx
        .kernel
        .attach_environment(
            &env.environment_id,
            &fx.worker.agent_id,
            &fx.worker.thread_id,
            &fx.task.task_id,
            AttachMode::WorkspaceWrite,
        )
        .unwrap();
    let released = fx
        .kernel
        .release_environment_lease(&lease.environment_lease_id)
        .unwrap();
    assert_eq!(released.status, EnvironmentLeaseStatus::Released);

    let events = fx.kernel.events().unwrap();
    assert!(events
        .iter()
        .any(|event| event.event_type == "EnvironmentLeaseGranted"));
    assert!(events
        .iter()
        .any(|event| event.event_type == "EnvironmentLeaseReleased"));
    let replayed = Kernel::from_events(&events).unwrap();
    assert_eq!(
        replayed
            .state_snapshot()
            .unwrap()
            .environment_leases
            .get(&lease.environment_lease_id)
            .unwrap()
            .status,
        EnvironmentLeaseStatus::Released
    );
}

#[test]
fn budget_exhaustion_changes_admission_state() {
    let fx = fixture();
    let ledger = fx
        .kernel
        .create_budget_ledger(
            BudgetScope::Task,
            fx.task.task_id.clone(),
            Some(10),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    let err = fx
        .kernel
        .debit_budget(
            &ledger.budget_ledger_id,
            BudgetDebit {
                tokens: 11,
                tool_calls: 0,
                wall_time_ms: 0,
                cost: 0.0,
                human_interrupts: 0,
                model_requests: 0,
            },
        )
        .unwrap_err();
    assert!(matches!(err, AgentOsError::BudgetExhausted(_)));
    let state = fx.kernel.state_snapshot().unwrap();
    assert_eq!(
        state
            .budget_ledgers
            .get(&ledger.budget_ledger_id)
            .unwrap()
            .status,
        BudgetStatus::Exhausted
    );
}

#[test]
fn provider_routing_uses_role_policy_and_model_aliases() {
    let fx = fixture();
    let decision = fx
        .kernel
        .resolve_provider_route(StreamRequest {
            thread_id: fx.worker.thread_id.clone(),
            turn_id: None,
            provider_profile_id: "prov_default".to_string(),
            model_routing_policy_id: "route_default".to_string(),
            requested_model_alias: None,
            role: "ProducerAgent".to_string(),
            task_id: fx.task.task_id.clone(),
            reasoning_profile: None,
            tool_visibility_profile: None,
            output_schema: None,
        })
        .unwrap();
    assert_eq!(decision.selected_model_alias, "coding-primary");
    assert_eq!(decision.provider_id, "primary-provider");
    assert!(decision.model_capabilities.image_input);
}

#[test]
fn provider_stream_session_records_usage_and_replays() {
    let fx = fixture();
    let opened = fx
        .kernel
        .open_stream_session(StreamRequest {
            thread_id: fx.worker.thread_id.clone(),
            turn_id: Some("turn_1".to_string()),
            provider_profile_id: "prov_default".to_string(),
            model_routing_policy_id: "route_default".to_string(),
            requested_model_alias: None,
            role: "ProducerAgent".to_string(),
            task_id: fx.task.task_id.clone(),
            reasoning_profile: None,
            tool_visibility_profile: None,
            output_schema: None,
        })
        .unwrap();
    assert_eq!(opened.status, ProviderStreamStatus::Open);
    assert!(opened
        .stream_events
        .iter()
        .any(|event| event.event_type == ProviderStreamEventType::StreamStarted));

    let metered = fx
        .kernel
        .record_provider_usage(
            &opened.session_id,
            ProviderUsage {
                input_tokens: 12,
                output_tokens: 7,
                cost: 0.19,
            },
        )
        .unwrap();
    assert_eq!(metered.usage.input_tokens, 12);
    assert_eq!(metered.usage.output_tokens, 7);
    assert!(metered
        .stream_events
        .iter()
        .any(|event| event.event_type == ProviderStreamEventType::UsageUpdated));

    let completed = fx
        .kernel
        .complete_stream_session(&opened.session_id)
        .unwrap();
    assert_eq!(completed.status, ProviderStreamStatus::Completed);
    assert!(completed.completed_at.is_some());
    assert!(completed
        .stream_events
        .iter()
        .any(|event| event.event_type == ProviderStreamEventType::StreamCompleted));

    let events = fx.kernel.events().unwrap();
    assert!(events
        .iter()
        .any(|event| event.event_type == "ProviderStreamSessionOpened"));
    assert!(events
        .iter()
        .any(|event| event.event_type == "ProviderUsageRecorded"));
    assert!(events
        .iter()
        .any(|event| event.event_type == "ProviderStreamCompleted"));
    assert!(events
        .iter()
        .any(|event| event.event_type == "ResourceLeaseGranted"));
    assert!(events
        .iter()
        .any(|event| event.event_type == "ResourceLeaseReleased"));

    let replayed = Kernel::from_events(&events).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    let replayed_session = replayed_state
        .provider_stream_sessions
        .get(&opened.session_id)
        .unwrap()
        .clone();
    assert_eq!(replayed_session.status, ProviderStreamStatus::Completed);
    assert_eq!(replayed_session.usage.input_tokens, 12);
    assert_eq!(
        replayed_state
            .resource_leases
            .get(&opened.provider_slot_lease_id)
            .unwrap()
            .status,
        ResourceLeaseStatus::Released
    );
    assert_eq!(
        replayed_session.route_decision.credential_ref_id,
        "cred_default_llm"
    );
}

#[test]
fn scheduler_rejects_turn_when_provider_profile_budget_is_exhausted() {
    let fx = fixture();
    let ledger = fx
        .kernel
        .create_budget_ledger(
            BudgetScope::ProviderProfile,
            "prov_default",
            None,
            None,
            None,
            None,
            None,
            Some(0),
        )
        .unwrap();
    let _ = fx.kernel.debit_budget(
        &ledger.budget_ledger_id,
        BudgetDebit {
            tokens: 0,
            tool_calls: 0,
            wall_time_ms: 0,
            cost: 0.0,
            human_interrupts: 0,
            model_requests: 1,
        },
    );
    let err = fx.kernel.start_turn(&fx.worker.thread_id).unwrap_err();
    assert!(matches!(err, AgentOsError::BudgetExhausted(_)));
}

#[test]
fn scheduler_rejects_turn_when_human_attention_budget_is_exhausted() {
    let fx = fixture();
    let ledger = fx
        .kernel
        .create_budget_ledger(
            BudgetScope::HumanAttention,
            fx.goal.goal_id.clone(),
            None,
            None,
            None,
            None,
            Some(0),
            None,
        )
        .unwrap();
    let _ = fx.kernel.debit_budget(
        &ledger.budget_ledger_id,
        BudgetDebit {
            tokens: 0,
            tool_calls: 0,
            wall_time_ms: 0,
            cost: 0.0,
            human_interrupts: 1,
            model_requests: 0,
        },
    );
    let err = fx.kernel.start_turn(&fx.worker.thread_id).unwrap_err();
    assert!(matches!(err, AgentOsError::BudgetExhausted(_)));
}

#[test]
fn scheduler_rejects_turn_when_provider_slot_is_unavailable() {
    let fx = fixture();
    fx.kernel
        .request_resource_lease(
            ResourceType::ProviderSlot,
            "primary-provider",
            "other-agent",
            "other-thread",
            fx.goal.goal_id.clone(),
            fx.task.task_id.clone(),
            LeaseMode::Exclusive,
            Some("other model call".to_string()),
        )
        .unwrap();
    let err = fx.kernel.start_turn(&fx.worker.thread_id).unwrap_err();
    assert!(matches!(err, AgentOsError::ResourceConflict(_)));
}

#[test]
fn ready_queue_orders_ready_threads_by_task_priority() {
    let fx = fixture();
    let high = fx
        .kernel
        .spawn_task(SpawnTaskInput {
            goal_id: fx.goal.goal_id.clone(),
            parent_task_id: None,
            title: "High priority".to_string(),
            description: "High priority".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: Vec::new(),
            priority: 100,
            risk_level: 1,
        })
        .unwrap();
    let high_agent = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: high.task_id,
            role_profile_id: "role_producer".to_string(),
            owner: "tester".to_string(),
            goal: "high".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![".".to_string()],
        })
        .unwrap();
    fx.kernel
        .transition_thread(&fx.worker.thread_id, ThreadStatus::Ready, None)
        .unwrap();
    fx.kernel
        .transition_thread(&high_agent.thread_id, ThreadStatus::Ready, None)
        .unwrap();
    assert_eq!(
        fx.kernel.ready_queue_snapshot().unwrap(),
        vec![high_agent.thread_id, fx.worker.thread_id]
    );
}

#[test]
fn forbidden_provider_override_is_rejected() {
    let fx = fixture();
    let err = fx
        .kernel
        .open_stream_session(StreamRequest {
            thread_id: fx.worker.thread_id.clone(),
            turn_id: Some("turn_1".to_string()),
            provider_profile_id: "prov_default".to_string(),
            model_routing_policy_id: "route_default".to_string(),
            requested_model_alias: Some("external-escape".to_string()),
            role: "ProducerAgent".to_string(),
            task_id: fx.task.task_id.clone(),
            reasoning_profile: None,
            tool_visibility_profile: None,
            output_schema: None,
        })
        .unwrap_err();
    assert!(matches!(err, AgentOsError::PermissionDenied(_)));
    let events = fx.kernel.events().unwrap();
    assert!(!events
        .iter()
        .any(|event| event.event_type == "ProviderStreamSessionOpened"));
}

#[test]
fn provider_capability_mismatch_is_rejected_before_stream_open() {
    let fx = fixture();
    let err = fx
        .kernel
        .open_stream_session(StreamRequest {
            thread_id: fx.worker.thread_id.clone(),
            turn_id: Some("turn_1".to_string()),
            provider_profile_id: "prov_strict_text".to_string(),
            model_routing_policy_id: "route_default".to_string(),
            requested_model_alias: Some("text-only".to_string()),
            role: "ProducerAgent".to_string(),
            task_id: fx.task.task_id.clone(),
            reasoning_profile: None,
            tool_visibility_profile: None,
            output_schema: Some(json!({
                "type": "object",
                "properties": {
                    "summary": {"type": "string"}
                }
            })),
        })
        .unwrap_err();
    assert!(matches!(err, AgentOsError::PermissionDenied(_)));
    assert!(!fx
        .kernel
        .events()
        .unwrap()
        .iter()
        .any(|event| event.event_type == "ProviderStreamSessionOpened"));
}

#[test]
fn provider_capability_mismatch_is_strictly_rejected() {
    let fx = fixture();
    let err = fx
        .kernel
        .open_stream_session(StreamRequest {
            thread_id: fx.worker.thread_id.clone(),
            turn_id: Some("turn_1".to_string()),
            provider_profile_id: "prov_default".to_string(),
            model_routing_policy_id: "route_default".to_string(),
            requested_model_alias: Some("text-only".to_string()),
            role: "ProducerAgent".to_string(),
            task_id: fx.task.task_id.clone(),
            reasoning_profile: None,
            tool_visibility_profile: None,
            output_schema: Some(json!({
                "type": "object",
                "properties": {
                    "summary": {"type": "string"}
                }
            })),
        })
        .unwrap_err();
    assert!(matches!(err, AgentOsError::PermissionDenied(_)));

    let events = fx.kernel.events().unwrap();
    assert!(!events
        .iter()
        .any(|event| event.event_type == "ProviderStreamSessionOpened"));
    assert!(!events.iter().any(|event| {
        event.event_type == "ResourceLeaseGranted"
            && event
                .payload
                .get("resource_type")
                .and_then(serde_json::Value::as_str)
                == Some("provider_slot")
    }));
}

#[test]
fn sqlite_store_persists_events_for_kernel_replay() {
    let sqlite = SqliteStore::in_memory().unwrap();
    assert_eq!(sqlite.migration_version().unwrap(), 3);
    let fx = fixture_with_kernel(Kernel::with_store(sqlite));
    let evidence = fx
        .kernel
        .attach_evidence(evidence_input(&fx, EvidenceType::SourceRef))
        .unwrap();
    fx.kernel
        .submit_final(
            &fx.worker.agent_id,
            &fx.task.task_id,
            FinalSubmission {
                summary: "done".to_string(),
                changed_artifacts: Vec::new(),
                evidence_map: vec![EvidenceMapEntry {
                    claim: "source was inspected".to_string(),
                    evidence_refs: vec![evidence.evidence_id],
                }],
                unverified_claims: Vec::new(),
                known_risks: Vec::new(),
                tests_run: Vec::new(),
                tests_not_run: vec!["none".to_string()],
                approvals: Vec::new(),
            },
        )
        .unwrap();

    let persisted_events = fx.kernel.events().unwrap();
    assert!(persisted_events
        .iter()
        .any(|event| event.event_type == "FinalSubmitted"));
    let replayed = Kernel::from_events(&persisted_events).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert_eq!(replayed_state.final_submissions.len(), 1);
    assert_eq!(replayed_state.evidence.len(), 1);
    assert!(fx
        .kernel
        .store()
        .projection_checkpoint("evidence_index")
        .unwrap()
        .is_some());
}

#[test]
fn sqlite_store_replays_existing_events_and_continues_after_restart() {
    let db_path = env::temp_dir().join(format!(
        "agent-os-conformance-restart-{}-{}.sqlite",
        std::process::id(),
        new_id("case_")
    ));
    let first = Kernel::with_replayed_store(SqliteStore::open(&db_path).unwrap()).unwrap();
    let fx = fixture_with_kernel(first);
    let first_goal_id = fx.goal.goal_id.clone();
    let first_task_id = fx.task.task_id.clone();
    drop(fx);

    let restarted = Kernel::with_replayed_store(SqliteStore::open(&db_path).unwrap()).unwrap();
    let restarted_state = restarted.state_snapshot().unwrap();
    assert!(restarted_state.goals.contains_key(&first_goal_id));
    assert!(restarted_state.tasks.contains_key(&first_task_id));

    let second_goal = restarted
        .register_goal(RegisterGoalInput {
            namespace: "conformance".to_string(),
            created_by: "restart-test".to_string(),
            title: "Continue after restart".to_string(),
            description: "Ensure durable store can append after replay".to_string(),
            acceptance_criteria: vec!["new event appends without id collision".to_string()],
            constraints: Vec::new(),
            risk_level: 1,
            deadline: None,
        })
        .unwrap();
    assert_ne!(second_goal.goal_id, first_goal_id);

    let replayed_again = Kernel::with_replayed_store(SqliteStore::open(&db_path).unwrap()).unwrap();
    let replayed_state = replayed_again.state_snapshot().unwrap();
    assert!(replayed_state.goals.contains_key(&first_goal_id));
    assert!(replayed_state.goals.contains_key(&second_goal.goal_id));
    let _ = fs::remove_file(db_path);
}

#[test]
fn sqlite_store_replays_context_memory_and_memento_after_restart() {
    let db_path = env::temp_dir().join(format!(
        "agent-os-conformance-context-restart-{}-{}.sqlite",
        std::process::id(),
        new_id("case_")
    ));
    let first = Kernel::with_replayed_store(SqliteStore::open(&db_path).unwrap()).unwrap();
    let fx = fixture_with_kernel(first.clone());
    let evidence = first
        .attach_evidence(evidence_input(&fx, EvidenceType::SourceRef))
        .unwrap();
    let context = first
        .load_context(LoadContextInput {
            agent_id: fx.worker.agent_id.clone(),
            task_id: fx.task.task_id.clone(),
            loaded_refs: vec![evidence.evidence_id.clone()],
            summary_artifact_id: None,
            freshness: ContextFreshness::Fresh,
            pollution_score: 0.0,
            token_estimate: 128,
        })
        .unwrap();
    let proposed_memory = first
        .propose_memory_write(ProposeMemoryWriteInput {
            namespace: "decisions".to_string(),
            content: json!({"decision": "restart keeps context-affecting state"}),
            created_by_agent_id: fx.worker.agent_id.clone(),
            source_evidence_ids: vec![evidence.evidence_id.clone()],
        })
        .unwrap();
    let active_memory = first
        .commit_memory_write(CommitMemoryWriteInput {
            memory_id: proposed_memory.memory_id.clone(),
            approved_by: "conformance".to_string(),
        })
        .unwrap();
    let memento = first
        .create_memento(CreateMementoInput {
            owner_agent_id: fx.worker.agent_id.clone(),
            owner_thread_id: fx.worker.thread_id.clone(),
            goal_id: fx.goal.goal_id.clone(),
            task_id: fx.task.task_id.clone(),
            anchor: MementoAnchor {
                anchor_type: MementoAnchorType::Manual,
                anchor_ref: Some("sqlite-context-restart".to_string()),
                condition: None,
            },
            content: MementoContent {
                title: "Restart context reminder".to_string(),
                body: "SQLite replay must preserve owner context state.".to_string(),
                checklist: Vec::new(),
                structured: None,
            },
            projection: MementoProjection {
                mode: MementoProjectionMode::OwnerContext,
                priority: MementoPriority::High,
                max_projection_count: Some(1),
            },
            links: MementoLinks::default(),
            supersedes: None,
            expires_at: None,
        })
        .unwrap();
    let armed_memento = first
        .arm_memento(&fx.worker.agent_id, &memento.memento_id)
        .unwrap();
    drop(fx);
    drop(first);

    let restarted = Kernel::with_replayed_store(SqliteStore::open(&db_path).unwrap()).unwrap();
    let state = restarted.state_snapshot().unwrap();
    assert!(state.context_snapshots.contains_key(&context.context_id));
    let restarted_memory = state.memory_records.get(&active_memory.memory_id).unwrap();
    assert_eq!(restarted_memory.status, MemoryStatus::Active);
    assert_eq!(
        restarted_memory.source_evidence_ids,
        vec![evidence.evidence_id.clone()]
    );
    let restarted_memento = state.mementos.get(&armed_memento.memento_id).unwrap();
    assert_eq!(restarted_memento.status, MementoStatus::Armed);
    assert_eq!(
        restarted_memento.owner_thread_id,
        armed_memento.owner_thread_id
    );
    assert!(state.evidence.contains_key(&evidence.evidence_id));
    let visible = restarted
        .visible_mementos_for_thread(
            &armed_memento.owner_thread_id,
            &armed_memento.owner_thread_id,
        )
        .unwrap();
    assert!(visible
        .iter()
        .any(|candidate| candidate.memento_id == armed_memento.memento_id));
    let _ = fs::remove_file(db_path);
}

#[test]
fn sqlite_idempotency_results_persist_across_restart_without_polluting_event_log() {
    let db_path = env::temp_dir().join(format!(
        "agent-os-conformance-idempotency-{}-{}.sqlite",
        std::process::id(),
        new_id("case_")
    ));
    let store = SqliteStore::open(&db_path).unwrap();
    let accepted = SyscallResult::accepted(
        "syscall_accept_1",
        vec!["event_accept_1".to_string()],
        json!({"status": "accepted"}),
    );
    let rejected = SyscallResult::rejected("syscall_reject_1", "risk ceiling exceeded");

    store
        .put_syscall_result("idem-accept".to_string(), accepted.clone())
        .unwrap();
    store
        .put_syscall_result("idem-reject".to_string(), rejected.clone())
        .unwrap();
    assert!(matches!(
        store.put_syscall_result("idem-accept".to_string(), accepted.clone()),
        Err(AgentOsError::IdempotencyConflict(_))
    ));
    assert!(store.all_events().unwrap().is_empty());
    drop(store);

    let reopened = SqliteStore::open(&db_path).unwrap();
    let accepted_after_restart = reopened.get_syscall_result("idem-accept").unwrap().unwrap();
    assert_eq!(accepted_after_restart.syscall_id, accepted.syscall_id);
    assert!(accepted_after_restart.accepted);
    assert_eq!(accepted_after_restart.event_ids, accepted.event_ids);
    assert_eq!(accepted_after_restart.output["status"], "accepted");

    let rejected_after_restart = reopened.get_syscall_result("idem-reject").unwrap().unwrap();
    assert_eq!(rejected_after_restart.syscall_id, rejected.syscall_id);
    assert!(!rejected_after_restart.accepted);
    assert_eq!(
        rejected_after_restart.error.as_deref(),
        Some("risk ceiling exceeded")
    );
    assert!(reopened
        .get_syscall_result("idem-missing")
        .unwrap()
        .is_none());
    assert!(reopened.all_events().unwrap().is_empty());

    drop(reopened);
    let _ = fs::remove_file(db_path);
}
