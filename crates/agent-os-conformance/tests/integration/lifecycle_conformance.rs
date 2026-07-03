use crate::common::*;
use agent_os_thread::{turn_start_op, AgentThreadHandle};

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
fn agent_thread_handle_admits_turns_and_rejects_stale_steering() {
    let fx = fixture();
    let handle = AgentThreadHandle::new(fx.kernel.clone(), fx.worker.thread_id.clone());
    let ack = handle.try_start_turn(turn_start_op(fx.worker.thread_id.clone()));
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
            goal: "root supervision".to_string(),
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
            goal: "delegated supervision".to_string(),
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
            role_profile_id: "role_producer".to_string(),
            owner: "tester".to_string(),
            goal: "assigned work".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: Some(s1.thread_id.clone()),
            workspace_roots: Vec::new(),
        })
        .unwrap();

    assert_eq!(s0.security_level, SecurityLevel::ROOT_AGENT);
    assert_eq!(s1.security_level, SecurityLevel(2));
    assert_eq!(worker.security_level, SecurityLevel(3));

    let state = fx.kernel.state_snapshot().unwrap();
    let root_invocation = state.agent_invocations.get(&s0.invocation_id).unwrap();
    assert_eq!(
        root_invocation.relationship,
        AgentInvocationRelationship::RootSupervisor
    );
    let delegated_invocation = state.agent_invocations.get(&s1.invocation_id).unwrap();
    assert_eq!(delegated_invocation.caller_thread_id, Some(s0.thread_id));
    assert_eq!(
        delegated_invocation.caller_security_level,
        Some(SecurityLevel::ROOT_AGENT)
    );
    assert_eq!(delegated_invocation.callee_security_level, SecurityLevel(2));
    assert_eq!(
        delegated_invocation.relationship,
        AgentInvocationRelationship::SupervisorDelegation
    );
    let worker_invocation = state.agent_invocations.get(&worker.invocation_id).unwrap();
    assert_eq!(worker_invocation.caller_thread_id, Some(s1.thread_id));
    assert_eq!(
        worker_invocation.caller_security_level,
        Some(SecurityLevel(2))
    );
    assert_eq!(
        worker_invocation.relationship,
        AgentInvocationRelationship::ProducerAssignment
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
            goal: "supervise demo".to_string(),
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
                        "goal": "complete the demo task",
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
    assert_eq!(child.task.goal, "complete the demo task");
    let invocation = state.agent_invocations.get(&child.invocation_id).unwrap();
    assert_eq!(
        invocation.relationship,
        AgentInvocationRelationship::ProducerAssignment
    );
    assert_eq!(
        invocation.caller_thread_id,
        Some(supervisor.thread_id.clone())
    );
    assert_eq!(invocation.goal, child.task.goal);
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
fn supervisor_set_goal_retargets_direct_child_and_worker_is_denied() {
    let fx = fixture();
    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "tester".to_string(),
            goal: "supervise retargeting".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![".".to_string()],
        })
        .unwrap();
    let child = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_producer".to_string(),
            owner: supervisor.agent_id.clone(),
            goal: "initial child goal".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: Some(supervisor.thread_id.clone()),
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
            2,
            None,
        )
        .unwrap();
    let worker_cap = fx
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

    fx.kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            supervisor_cap.capability_id,
            2,
            ToolInvokeInput {
                tool_name: "set_goal".to_string(),
                input: json!({
                    "target_thread_id": child.thread_id.clone(),
                    "goal": "retargeted child goal",
                    "success_criteria": ["retarget is durable"]
                }),
                evidence_claim: None,
            },
        )
        .unwrap();

    let denied = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            worker_cap.capability_id,
            2,
            ToolInvokeInput {
                tool_name: "set_goal".to_string(),
                input: json!({"target_agent_id": child.agent_id.clone(), "goal": "illegal worker retarget"}),
                evidence_claim: None,
            },
        )
        .unwrap_err();
    assert!(matches!(denied, AgentOsError::PermissionDenied(_)));

    let state = fx.kernel.state_snapshot().unwrap();
    let retargeted = state.threads.get(&child.thread_id).unwrap();
    assert_eq!(retargeted.task.goal, "retargeted child goal");
    assert_eq!(retargeted.task.goal_revision, 2);
    assert_eq!(
        state
            .agent_invocations
            .get(&retargeted.invocation_id)
            .unwrap()
            .goal,
        "retargeted child goal"
    );
}

#[test]
fn accomplish_goal_completes_local_goal_hooks_and_invocation_before_final_submission() {
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
            supervisor_cap.capability_id,
            4,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: json!({
                    "action": "start",
                    "payload": {
                        "goal": "finish local child goal",
                        "hooks": [{
                            "interval_seconds": 30,
                            "prompt": "Report progress."
                        }]
                    }
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    let child_thread_id = start.output.as_ref().unwrap()["thread_id"]
        .as_str()
        .unwrap()
        .to_string();
    let child = fx
        .kernel
        .state_snapshot()
        .unwrap()
        .threads
        .get(&child_thread_id)
        .cloned()
        .unwrap();
    let child_cap = fx
        .kernel
        .grant_capability(
            &child.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            2,
            None,
        )
        .unwrap();
    fx.kernel
        .invoke_tool(
            &child.agent_id,
            &fx.task.task_id,
            &child.session_id,
            child_cap.capability_id,
            2,
            ToolInvokeInput {
                tool_name: "accomplish_goal".to_string(),
                input: json!({"summary": "child local goal complete"}),
                evidence_claim: None,
            },
        )
        .unwrap();

    let state = fx.kernel.state_snapshot().unwrap();
    let child = state.threads.get(&child_thread_id).unwrap();
    assert_eq!(child.status, ThreadStatus::Completing);
    assert_eq!(child.task.goal_status, AgentGoalStatus::Accomplished);
    assert_eq!(
        state
            .agent_invocations
            .get(&child.invocation_id)
            .unwrap()
            .status,
        AgentInvocationStatus::Completed
    );
    assert!(
        state
            .agent_hooks
            .values()
            .any(|hook| hook.thread_id == child_thread_id
                && hook.status == AgentHookStatus::Completed)
    );
    assert_ne!(
        state.tasks.get(&fx.task.task_id).unwrap().status,
        TaskStatus::Completed
    );
}

#[test]
fn agent_control_lifecycle_actions_update_state_and_trace() {
    let fx = fixture();
    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "tester".to_string(),
            goal: "supervise lifecycle".to_string(),
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

    let child = fx
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
                        "goal": "child lifecycle target"
                    }
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    let child_thread_id = child.output.as_ref().unwrap()["thread_id"]
        .as_str()
        .unwrap()
        .to_string();

    let timeout = fx
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
                    "action": "set_timeout",
                    "thread_id": child_thread_id,
                    "payload": {"timeout_seconds": 45}
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    assert_eq!(
        timeout.output.as_ref().unwrap()["output"]["timeout_ms"],
        json!(45000)
    );

    fx.kernel
        .transition_thread(&child_thread_id, ThreadStatus::Ready, None)
        .unwrap();
    fx.kernel
        .transition_thread(&child_thread_id, ThreadStatus::Suspended, None)
        .unwrap();
    let resumed = fx
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
                    "action": "resume",
                    "thread_id": child_thread_id
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    assert_eq!(
        resumed.output.as_ref().unwrap()["thread_status"],
        json!("Ready")
    );

    let stream = fx
        .kernel
        .open_stream_session(StreamRequest {
            thread_id: child_thread_id.clone(),
            turn_id: Some("turn_lifecycle".to_string()),
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
    fx.kernel
        .record_provider_stream_event(
            &stream.session_id,
            ProviderStreamEventType::OutputTextDelta,
            json!({"text": "first"}),
        )
        .unwrap();
    fx.kernel
        .record_provider_stream_event(
            &stream.session_id,
            ProviderStreamEventType::OutputTextDelta,
            json!({"text": "second"}),
        )
        .unwrap();
    let paged_output = fx
        .kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            cap.capability_id.clone(),
            1,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: json!({
                    "action": "output",
                    "thread_id": child_thread_id.clone(),
                    "payload": {"cursor": 1, "limit": 1}
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    let output_page = &paged_output.output.as_ref().unwrap()["output"];
    assert_eq!(output_page["cursor"], json!(1));
    assert_eq!(output_page["limit"], json!(1));
    assert_eq!(output_page["items"].as_array().unwrap().len(), 1);
    assert!(output_page["total_items"].as_u64().unwrap() >= 3);
    assert_eq!(output_page["next_cursor"], json!(2));
    assert_eq!(output_page["truncated"], json!(true));

    let trace = fx
        .kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            cap.capability_id.clone(),
            1,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: json!({
                    "action": "export_trace",
                    "thread_id": child_thread_id
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    let trace_output = &trace.output.as_ref().unwrap()["output"];
    assert!(trace_output["event_count"].as_u64().unwrap() > 0);
    assert!(trace_output["events"].is_null());
    assert!(
        trace_output["preview_events"].as_array().unwrap().len()
            <= trace_output["preview_event_limit"].as_u64().unwrap() as usize
    );
    assert!(trace_output["event_types"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event_type| event_type == "ThreadConfigured"));

    let stopped = fx
        .kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            cap.capability_id,
            4,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: json!({
                    "action": "stop",
                    "thread_id": child_thread_id
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    assert_eq!(
        stopped.output.as_ref().unwrap()["thread_status"],
        json!("Terminated")
    );
    assert_eq!(
        fx.kernel
            .state_snapshot()
            .unwrap()
            .threads
            .get(&child_thread_id)
            .unwrap()
            .budgets
            .wall_time_budget_ms,
        Some(45000)
    );
}

#[test]
fn privileged_agent_control_actions_require_privileged_risk() {
    let fx = fixture();
    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "tester".to_string(),
            goal: "supervise privileged lifecycle".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![".".to_string()],
        })
        .unwrap();
    let child = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_producer".to_string(),
            owner: supervisor.agent_id.clone(),
            goal: "target".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: Some(supervisor.thread_id.clone()),
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
    let result = fx
        .kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            cap.capability_id,
            4,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: json!({
                    "action": "kill",
                    "thread_id": child.thread_id
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    assert_eq!(result.status, ToolCallStatus::Failed);
    let error = result
        .output
        .as_ref()
        .and_then(|output| output.get("error"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    assert!(error.contains("agent_control action requires risk level 6"));
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
