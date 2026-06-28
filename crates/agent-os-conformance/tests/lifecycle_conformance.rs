mod common;
use agent_os_thread::{mock_turn_start_op, AgentThreadHandle};
use common::*;

#[test]
fn invalid_lifecycle_transition_is_rejected() {
    let fx = fixture();
    fx.kernel
        .transition_thread(&fx.worker.thread_id, ThreadStatus::Ready, None)
        .unwrap();
    fx.kernel
        .transition_thread(&fx.worker.thread_id, ThreadStatus::Running, None)
        .unwrap();
    fx.kernel
        .transition_thread(&fx.worker.thread_id, ThreadStatus::Completed, None)
        .unwrap();
    let err = fx
        .kernel
        .transition_thread(&fx.worker.thread_id, ThreadStatus::Running, None)
        .unwrap_err();
    assert!(matches!(err, AgentOsError::InvalidTransition(_)));
}

#[test]
fn state_replay_rebuilds_kernel_projection() {
    let fx = fixture();
    let evidence = fx
        .kernel
        .attach_evidence(evidence_input(&fx, EvidenceType::DiffRef))
        .unwrap();
    attach_writable_environment(&fx);
    fx.kernel
        .commit_artifact(CommitArtifactInput {
            goal_id: fx.goal.goal_id.clone(),
            task_id: fx.task.task_id.clone(),
            owner_agent_id: fx.worker.agent_id.clone(),
            artifact_type: ArtifactType::Patch,
            blob_ref: Some("blob://patch".to_string()),
            content_hash: Some("patch-hash".to_string()),
            inline_bytes: None,
            metadata: json!({}),
            evidence_ids: vec![evidence.evidence_id],
            supersedes: None,
        })
        .unwrap();

    let replayed = Kernel::from_events(&fx.kernel.events().unwrap()).unwrap();
    let original = fx.kernel.state_snapshot().unwrap();
    let rebuilt = replayed.state_snapshot().unwrap();
    assert_eq!(original.artifacts.len(), rebuilt.artifacts.len());
    assert_eq!(original.evidence.len(), rebuilt.evidence.len());
    assert_eq!(original.threads.len(), rebuilt.threads.len());
}

#[test]
fn mock_runtime_admits_turns_and_rejects_stale_steering() {
    let fx = fixture();
    let handle = AgentThreadHandle::new(fx.kernel.clone(), fx.worker.thread_id.clone());
    let ack = handle.try_start_turn(mock_turn_start_op(fx.worker.thread_id.clone()));
    assert!(ack.is_ok());

    let stale = AgentOp {
        abi_version: ABI_VERSION.to_string(),
        op_id: new_id("op_"),
        thread_id: fx.worker.thread_id.clone(),
        op_type: "turn.steer".to_string(),
        expected_turn_id: Some("turn_stale".to_string()),
        idempotency_key: new_id("idem_"),
        causation_id: None,
        submitted_by: "tester".to_string(),
        created_at: now_rfc3339(),
        payload: json!({}),
    };
    let result = handle.steer_turn(stale).unwrap();
    assert!(!result.accepted);
}

#[test]
fn supervisor_hierarchy_and_invocation_edges_replay() {
    let fx = fixture();
    let s0 = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "tester".to_string(),
            local_goal: "root supervision".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: Vec::new(),
        })
        .unwrap();
    let s1 = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "tester".to_string(),
            local_goal: "delegated supervision".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: Some(s0.thread_id.clone()),
            workspace_roots: Vec::new(),
        })
        .unwrap();
    let worker = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_worker".to_string(),
            owner: "tester".to_string(),
            local_goal: "assigned work".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: Some(s1.thread_id.clone()),
            workspace_roots: Vec::new(),
        })
        .unwrap();

    assert_eq!(s0.supervisor_level, Some(0));
    assert_eq!(s1.supervisor_level, Some(1));
    assert_eq!(worker.supervisor_level, None);

    let state = fx.kernel.state_snapshot().unwrap();
    let root_invocation = state.agent_invocations.get(&s0.invocation_id).unwrap();
    assert_eq!(
        root_invocation.relationship,
        AgentInvocationRelationship::RootSupervisor
    );
    let delegated_invocation = state.agent_invocations.get(&s1.invocation_id).unwrap();
    assert_eq!(delegated_invocation.caller_thread_id, Some(s0.thread_id));
    assert_eq!(delegated_invocation.caller_supervisor_level, Some(0));
    assert_eq!(delegated_invocation.callee_supervisor_level, Some(1));
    assert_eq!(
        delegated_invocation.relationship,
        AgentInvocationRelationship::SupervisorDelegation
    );
    let worker_invocation = state.agent_invocations.get(&worker.invocation_id).unwrap();
    assert_eq!(worker_invocation.caller_thread_id, Some(s1.thread_id));
    assert_eq!(worker_invocation.caller_supervisor_level, Some(1));
    assert_eq!(
        worker_invocation.relationship,
        AgentInvocationRelationship::WorkerAssignment
    );

    let replayed = Kernel::from_events(&fx.kernel.events().unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert_eq!(
        replayed_state
            .threads
            .get(&worker.thread_id)
            .unwrap()
            .invocation_id,
        worker.invocation_id.clone()
    );
    assert!(replayed_state
        .agent_invocations
        .contains_key(&worker.invocation_id));
}

#[test]
fn agent_control_starts_child_and_records_hook_state() {
    let fx = fixture();
    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "tester".to_string(),
            local_goal: "supervise demo".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![".".to_string()],
        })
        .unwrap();
    let cap = fx
        .kernel
        .grant_capability(
            &supervisor.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap();

    let start = fx
        .kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            cap.capability_id.clone(),
            4,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: json!({
                    "action": "start",
                    "payload": {
                        "assignment": "complete the demo task",
                        "success_criteria": ["demo task is complete"],
                        "hooks": [{
                            "interval_seconds": 60,
                            "prompt": "Report one concise progress sentence.",
                            "max_response_chars": 160
                        }]
                    }
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    assert_eq!(start.status, ToolCallStatus::Completed);
    assert_eq!(start.risk_level, 4);
    let output = start.output.as_ref().unwrap();
    assert_eq!(output["tool"], "agent_control");
    assert_eq!(output["action"], "start");
    let child_agent_id = output["agent_id"].as_str().unwrap().to_string();
    let child_thread_id = output["thread_id"].as_str().unwrap().to_string();

    let status = fx
        .kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            cap.capability_id,
            1,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: json!({
                    "action": "status",
                    "agent_id": child_agent_id
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    assert_eq!(status.status, ToolCallStatus::Completed);
    assert_eq!(status.risk_level, 1);

    let state = fx.kernel.state_snapshot().unwrap();
    let child = state.threads.get(&child_thread_id).unwrap();
    assert_eq!(child.parent_thread_id, Some(supervisor.thread_id.clone()));
    let invocation = state.agent_invocations.get(&child.invocation_id).unwrap();
    assert_eq!(
        invocation.relationship,
        AgentInvocationRelationship::WorkerAssignment
    );
    assert_eq!(
        invocation.caller_thread_id,
        Some(supervisor.thread_id.clone())
    );
    assert_eq!(state.agent_hooks.len(), 1);
    let hook = state.agent_hooks.values().next().unwrap();
    assert_eq!(hook.agent_id, child.agent_id);
    assert_eq!(hook.interval_seconds, 60);
    assert_eq!(hook.response_route, MessageRoute::Supervisor);
    assert!(state
        .agent_control_commands
        .values()
        .any(|command| command.action == AgentControlAction::Start
            && command.target_thread_id.as_deref() == Some(&child_thread_id)));

    let bundle = fx.kernel.export_task_bundle(&fx.task.task_id).unwrap();
    assert_eq!(bundle.projection_snapshot.agent_hooks.len(), 1);
    assert_eq!(bundle.projection_snapshot.agent_control_commands.len(), 1);

    let replayed = Kernel::from_events(&fx.kernel.events().unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert!(replayed_state.agent_hooks.contains_key(&hook.hook_id));
    assert!(replayed_state
        .agent_control_commands
        .values()
        .any(|command| command.action == AgentControlAction::Start));
}

#[test]
fn task_ready_requires_completed_dependencies_and_completion_requires_evidence() {
    let fx = fixture();
    let dependent = fx
        .kernel
        .spawn_task(SpawnTaskInput {
            goal_id: fx.goal.goal_id.clone(),
            parent_task_id: None,
            title: "Dependent".to_string(),
            description: "Waits on patch".to_string(),
            depends_on: vec![fx.task.task_id.clone()],
            required_artifact_types: Vec::new(),
            required_evidence_types: Vec::new(),
            priority: 10,
            risk_level: 1,
        })
        .unwrap();
    let ready_err = fx
        .kernel
        .update_task(UpdateTaskInput {
            task_id: dependent.task_id.clone(),
            status: Some(TaskStatus::Ready),
            blocked_reason: None,
            owner_agent_id: None,
            title: None,
            description: None,
            checklist: None,
        })
        .unwrap_err();
    assert!(matches!(ready_err, AgentOsError::InvalidTransition(_)));

    let complete_err = fx
        .kernel
        .complete_task(CompleteTaskInput {
            task_id: fx.task.task_id.clone(),
            artifact_ids: Vec::new(),
            evidence_ids: Vec::new(),
        })
        .unwrap_err();
    assert!(matches!(complete_err, AgentOsError::Validation(_)));

    let evidence = fx
        .kernel
        .attach_evidence(evidence_input(&fx, EvidenceType::DiffRef))
        .unwrap();
    attach_writable_environment(&fx);
    let artifact = fx
        .kernel
        .commit_artifact(CommitArtifactInput {
            goal_id: fx.goal.goal_id.clone(),
            task_id: fx.task.task_id.clone(),
            owner_agent_id: fx.worker.agent_id.clone(),
            artifact_type: ArtifactType::Patch,
            blob_ref: Some("blob://patch".to_string()),
            content_hash: Some("patch-hash".to_string()),
            inline_bytes: None,
            metadata: json!({}),
            evidence_ids: vec![evidence.evidence_id.clone()],
            supersedes: None,
        })
        .unwrap();
    fx.kernel
        .complete_task(CompleteTaskInput {
            task_id: fx.task.task_id.clone(),
            artifact_ids: vec![artifact.artifact_id],
            evidence_ids: vec![evidence.evidence_id],
        })
        .unwrap();
    fx.kernel
        .update_task(UpdateTaskInput {
            task_id: dependent.task_id,
            status: Some(TaskStatus::Ready),
            blocked_reason: None,
            owner_agent_id: None,
            title: None,
            description: None,
            checklist: None,
        })
        .unwrap();
}
