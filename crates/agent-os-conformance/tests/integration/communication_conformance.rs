use crate::common::*;

#[test]
fn communication_profile_blocks_forbidden_routes() {
    let fx = fixture();
    let human = fx
        .kernel
        .send_message(SendMessageInput {
            message_type: "HumanQuestion".to_string(),
            route: MessageRoute::Human,
            source_agent_id: fx.worker.agent_id.clone(),
            source_thread_id: fx.worker.thread_id.clone(),
            target_agent_id: None,
            target_thread_id: None,
            channel_id: None,
            goal_id: fx.goal.goal_id.clone(),
            task_id: fx.task.task_id.clone(),
            risk_level: 1,
            payload: json!({"body": "help?"}),
            artifact_refs: Vec::new(),
            evidence_refs: Vec::new(),
        })
        .unwrap();
    assert_eq!(human.delivery_status, MessageDeliveryStatus::Rejected);

    let global_post = fx
        .kernel
        .send_message(SendMessageInput {
            message_type: "BlackboardPost".to_string(),
            route: MessageRoute::Blackboard,
            source_agent_id: fx.worker.agent_id.clone(),
            source_thread_id: fx.worker.thread_id.clone(),
            target_agent_id: None,
            target_thread_id: None,
            channel_id: Some("facts".to_string()),
            goal_id: fx.goal.goal_id.clone(),
            task_id: fx.task.task_id.clone(),
            risk_level: 1,
            payload: json!({"scope": "global", "entry_type": "KnownFactCandidate"}),
            artifact_refs: Vec::new(),
            evidence_refs: Vec::new(),
        })
        .unwrap();
    assert_eq!(global_post.delivery_status, MessageDeliveryStatus::Rejected);
}

#[test]
fn blackboard_post_publishes_typed_entry_and_replays() {
    let fx = fixture();
    let evidence = fx
        .kernel
        .attach_evidence(evidence_input(&fx, EvidenceType::SourceRef))
        .unwrap();
    let cap = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["blackboard.post".to_string()],
            vec!["blackboard:facts".to_string()],
            1,
            None,
        )
        .unwrap();
    let syscall = SyscallEnvelope::new(
        "blackboard.post",
        fx.worker.agent_id.clone(),
        fx.task.task_id.clone(),
        fx.worker.session_id.clone(),
        Some(cap.capability_id),
        1,
        serde_json::to_value(PostBlackboardInput {
            source_agent_id: fx.worker.agent_id.clone(),
            source_thread_id: fx.worker.thread_id.clone(),
            channel_id: Some("facts".to_string()),
            goal_id: fx.goal.goal_id.clone(),
            task_id: Some(fx.task.task_id.clone()),
            scope: CommunicationScope::Goal,
            section: BlackboardSection::KnownFact,
            content: json!({"fact": "source was inspected"}),
            confidence: Some(0.9),
            source_evidence_ids: vec![evidence.evidence_id.clone()],
        })
        .unwrap(),
    );
    let result = fx.kernel.handle_syscall(syscall).unwrap();
    assert!(result.accepted);
    assert_eq!(result.event_ids.len(), 2);

    let state = fx.kernel.state_snapshot().unwrap();
    let entry = state.blackboard_entries.values().next().unwrap();
    assert_eq!(entry.section, BlackboardSection::KnownFact);
    assert_eq!(entry.source_evidence_ids, vec![evidence.evidence_id]);

    let replayed = Kernel::from_events(&fx.kernel.events().unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert_eq!(replayed_state.blackboard_entries.len(), 1);
    assert!(fx
        .kernel
        .events()
        .unwrap()
        .iter()
        .any(|event| event.event_type == "BlackboardPostPublished"));
}

#[test]
fn blackboard_post_rejects_unallowed_type_and_unproven_fact() {
    let fx = fixture();
    let cap = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["blackboard.post".to_string()],
            vec!["blackboard:facts".to_string()],
            1,
            None,
        )
        .unwrap();
    let unallowed = SyscallEnvelope::new(
        "blackboard.post",
        fx.worker.agent_id.clone(),
        fx.task.task_id.clone(),
        fx.worker.session_id.clone(),
        Some(cap.capability_id.clone()),
        1,
        serde_json::to_value(PostBlackboardInput {
            source_agent_id: fx.worker.agent_id.clone(),
            source_thread_id: fx.worker.thread_id.clone(),
            channel_id: Some("facts".to_string()),
            goal_id: fx.goal.goal_id.clone(),
            task_id: Some(fx.task.task_id.clone()),
            scope: CommunicationScope::Goal,
            section: BlackboardSection::AcceptanceCriterion,
            content: json!({"criterion": "ship it"}),
            confidence: None,
            source_evidence_ids: Vec::new(),
        })
        .unwrap(),
    );
    let unallowed_err = fx.kernel.handle_syscall(unallowed).unwrap_err();
    assert!(matches!(unallowed_err, AgentOsError::PermissionDenied(_)));

    let unproven_fact = SyscallEnvelope::new(
        "blackboard.post",
        fx.worker.agent_id.clone(),
        fx.task.task_id.clone(),
        fx.worker.session_id.clone(),
        Some(cap.capability_id),
        1,
        serde_json::to_value(PostBlackboardInput {
            source_agent_id: fx.worker.agent_id.clone(),
            source_thread_id: fx.worker.thread_id.clone(),
            channel_id: Some("facts".to_string()),
            goal_id: fx.goal.goal_id.clone(),
            task_id: Some(fx.task.task_id.clone()),
            scope: CommunicationScope::Goal,
            section: BlackboardSection::KnownFact,
            content: json!({"fact": "unsupported"}),
            confidence: Some(0.5),
            source_evidence_ids: Vec::new(),
        })
        .unwrap(),
    );
    let unproven_err = fx.kernel.handle_syscall(unproven_fact).unwrap_err();
    assert!(matches!(unproven_err, AgentOsError::Validation(_)));
}

#[test]
fn post_blackboard_reports_unproven_fact_failure_through_broker() {
    let fx = fixture();
    let cap = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            2,
            None,
        )
        .unwrap();

    let invocation = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            cap.capability_id,
            2,
            ToolInvokeInput {
                tool_name: "post_blackboard".to_string(),
                input: json!({
                    "channel_id": "facts",
                    "scope": "goal",
                    "section": "known_fact",
                    "content": {"fact": "unsupported broker fact"},
                    "confidence": 0.5
                }),
                evidence_claim: Some(
                    "unproven blackboard fact failure was model-visible".to_string(),
                ),
            },
        )
        .unwrap();

    assert_eq!(invocation.status, ToolCallStatus::Failed);
    assert!(invocation.evidence_ids.is_empty());
    let output = invocation.output.as_ref().unwrap();
    assert_eq!(output["status"], "failed");
    assert_eq!(output["stage"], "driver");
    assert!(output["error"]
        .as_str()
        .unwrap()
        .contains("facts and decisions require evidence provenance"));

    let state = fx.kernel.state_snapshot().unwrap();
    assert!(state.blackboard_entries.is_empty());
    assert!(state
        .messages
        .values()
        .all(|message| message.route != MessageRoute::Blackboard));
}

#[test]
fn control_plane_tools_execute_through_tool_broker() {
    let fx = fixture();
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

    let invoke = |tool_name: &str, input: serde_json::Value, risk_level: u8| {
        fx.kernel
            .invoke_tool(
                &fx.worker.agent_id,
                &fx.task.task_id,
                &fx.worker.session_id,
                cap.capability_id.clone(),
                risk_level,
                ToolInvokeInput {
                    tool_name: tool_name.to_string(),
                    input,
                    evidence_claim: Some(format!("{tool_name} conformance")),
                },
            )
            .unwrap()
    };

    let checklist = invoke(
        "update_checklist",
        json!({"items": [{"text": "record state", "status": "completed"}]}),
        2,
    );
    assert_eq!(
        checklist.output.as_ref().unwrap()["items"][0]["status"],
        "completed"
    );

    let evidence = invoke(
        "record_evidence",
        json!({
            "evidence_type": "external_reference",
            "claim": "control-plane tools ran through broker",
            "blob_ref": "blob://control-plane",
            "content_hash": "control-plane-hash"
        }),
        2,
    );
    let evidence_id = evidence.output.as_ref().unwrap()["evidence_id"]
        .as_str()
        .unwrap()
        .to_string();

    let report = invoke(
        "report_supervisor",
        json!({"message": "control-plane tools are under test"}),
        1,
    );
    assert_eq!(
        report.output.as_ref().unwrap()["delivery_status"],
        "Delivered"
    );

    let blackboard = invoke(
        "post_blackboard",
        json!({
            "channel_id": "risks",
            "scope": "goal",
            "section": "risk",
            "content": {"risk": "control-plane conformance risk"}
        }),
        2,
    );
    assert_eq!(blackboard.output.as_ref().unwrap()["section"], "risk");

    let goal_completion = invoke(
        "accomplish_goal",
        json!({"summary": "complete control-plane tool conformance"}),
        2,
    );
    assert_eq!(
        goal_completion.output.as_ref().unwrap()["goal_accomplished"],
        true
    );

    let state = fx.kernel.state_snapshot().unwrap();
    let worker = state.threads.get(&fx.worker.thread_id).unwrap();
    let task = state.tasks.get(&fx.task.task_id).unwrap();
    assert_eq!(worker.task.goal_status, AgentGoalStatus::Accomplished);
    assert_eq!(task.checklist[0].text, "record state");
    assert!(state.evidence.contains_key(&evidence_id));
    assert_eq!(state.blackboard_entries.len(), 1);
    assert!(state
        .messages
        .values()
        .any(|message| message.route == MessageRoute::Supervisor));

    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "tester".to_string(),
            goal: "ask human".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![".".to_string()],
        })
        .unwrap();
    let supervisor_cap = fx
        .kernel
        .grant_capability(
            &supervisor.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            3,
            None,
        )
        .unwrap();
    let human = fx
        .kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            supervisor_cap.capability_id,
            2,
            ToolInvokeInput {
                tool_name: "ask_human".to_string(),
                input: json!({"question": "confirm control-plane conformance?"}),
                evidence_claim: Some("human route conformance".to_string()),
            },
        )
        .unwrap();
    assert_eq!(
        human.output.as_ref().unwrap()["delivery_status"],
        "Delivered"
    );

    let replayed = Kernel::from_events(&fx.kernel.events().unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert_eq!(
        replayed_state
            .tasks
            .get(&fx.task.task_id)
            .unwrap()
            .checklist[0]
            .text,
        "record state"
    );
    assert_eq!(replayed_state.blackboard_entries.len(), 1);
}

#[test]
fn memento_is_owner_scoped_and_triggered_by_child_completion() {
    let fx = fixture();
    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "tester".to_string(),
            goal: "supervise child completion".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: Vec::new(),
        })
        .unwrap();
    let child = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_producer".to_string(),
            owner: "tester".to_string(),
            goal: "test".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: Some(supervisor.thread_id.clone()),
            workspace_roots: Vec::new(),
        })
        .unwrap();
    let draft = fx
        .kernel
        .create_memento(CreateMementoInput {
            owner_agent_id: supervisor.agent_id.clone(),
            owner_thread_id: supervisor.thread_id.clone(),
            goal_id: fx.goal.goal_id.clone(),
            task_id: fx.task.task_id.clone(),
            anchor: MementoAnchor {
                anchor_type: MementoAnchorType::ChildThreadCompleted,
                anchor_ref: Some(child.thread_id.clone()),
                condition: None,
            },
            content: MementoContent {
                title: "After tester".to_string(),
                body: "Check evidence first".to_string(),
                checklist: vec!["inspect logs".to_string()],
                structured: None,
            },
            projection: MementoProjection {
                mode: MementoProjectionMode::OwnerNextTurn,
                priority: MementoPriority::High,
                max_projection_count: Some(1),
            },
            links: MementoLinks::default(),
            supersedes: None,
            expires_at: None,
        })
        .unwrap();
    assert!(fx
        .kernel
        .visible_mementos_for_thread(&supervisor.thread_id, &supervisor.thread_id)
        .unwrap()
        .is_empty());
    fx.kernel
        .arm_memento(&supervisor.agent_id, &draft.memento_id)
        .unwrap();
    let armed_mementos = fx
        .kernel
        .visible_mementos_for_thread(&supervisor.thread_id, &supervisor.thread_id)
        .unwrap();
    assert!(armed_mementos
        .iter()
        .any(|m| m.memento_id == draft.memento_id && m.status == MementoStatus::Armed));
    assert!(fx
        .kernel
        .visible_mementos_for_thread(&child.thread_id, &supervisor.thread_id)
        .is_err());

    fx.kernel
        .transition_thread(&child.thread_id, ThreadStatus::Ready, None)
        .unwrap();
    fx.kernel
        .transition_thread(&child.thread_id, ThreadStatus::Running, None)
        .unwrap();
    let cap = fx
        .kernel
        .grant_capability(
            &child.agent_id,
            &fx.task.task_id,
            vec!["agent.complete".to_string()],
            vec!["agent:*".to_string()],
            1,
            None,
        )
        .unwrap();
    let syscall = SyscallEnvelope::new(
        "agent.complete",
        child.agent_id.clone(),
        fx.task.task_id.clone(),
        child.session_id.clone(),
        Some(cap.capability_id),
        1,
        json!({}),
    );
    fx.kernel.handle_syscall(syscall).unwrap();
    let mementos = fx
        .kernel
        .visible_mementos_for_thread(&supervisor.thread_id, &supervisor.thread_id)
        .unwrap();
    assert!(mementos
        .iter()
        .any(|m| m.memento_id == draft.memento_id && m.status == MementoStatus::Triggered));
    fx.kernel
        .consume_memento(&supervisor.agent_id, &draft.memento_id)
        .unwrap();
    assert!(fx
        .kernel
        .visible_mementos_for_thread(&supervisor.thread_id, &supervisor.thread_id)
        .unwrap()
        .is_empty());
}
