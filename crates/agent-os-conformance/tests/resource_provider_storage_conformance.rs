mod common;
use agent_os_store_sqlite::SqliteStore;
use common::*;
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
            role: "WorkerAgent".to_string(),
            task_id: fx.task.task_id.clone(),
            reasoning_profile: None,
            tool_visibility_profile: None,
            output_schema: None,
        })
        .unwrap();
    assert_eq!(decision.selected_model_alias, "coding-primary");
    assert_eq!(decision.provider_id, "mock-provider");
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
            role: "WorkerAgent".to_string(),
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

    let replayed = Kernel::from_events(&events).unwrap();
    let replayed_session = replayed
        .state_snapshot()
        .unwrap()
        .provider_stream_sessions
        .get(&opened.session_id)
        .unwrap()
        .clone();
    assert_eq!(replayed_session.status, ProviderStreamStatus::Completed);
    assert_eq!(replayed_session.usage.input_tokens, 12);
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
            role: "WorkerAgent".to_string(),
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
        .any(|event| event.event_type == "ProviderFallbackApplied"));
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
            role: "WorkerAgent".to_string(),
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
fn provider_fallback_emits_durable_event() {
    let fx = fixture();
    let opened = fx
        .kernel
        .open_stream_session(StreamRequest {
            thread_id: fx.worker.thread_id.clone(),
            turn_id: Some("turn_1".to_string()),
            provider_profile_id: "prov_default".to_string(),
            model_routing_policy_id: "route_default".to_string(),
            requested_model_alias: Some("text-only".to_string()),
            role: "WorkerAgent".to_string(),
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
        .unwrap();
    assert!(opened.route_decision.fallback_applied);
    assert_eq!(opened.route_decision.selected_model_alias, "mock-model");
    assert_eq!(
        opened.route_decision.fallback_from_model_alias.as_deref(),
        Some("text-only")
    );
    assert!(opened
        .stream_events
        .iter()
        .any(|event| event.event_type == ProviderStreamEventType::ProviderFallback));

    let events = fx.kernel.events().unwrap();
    assert!(events
        .iter()
        .any(|event| event.event_type == "ProviderFallbackApplied"));
    assert!(events
        .iter()
        .any(|event| event.event_type == "ProviderStreamSessionOpened"));
}

#[test]
fn sqlite_store_persists_events_for_kernel_replay() {
    let sqlite = SqliteStore::in_memory().unwrap();
    assert_eq!(sqlite.migration_version().unwrap(), 1);
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
