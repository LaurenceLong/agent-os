use crate::common::*;

#[test]
fn syscall_without_capability_is_rejected() {
    let fx = fixture();
    let syscall = SyscallEnvelope::new(
        "evidence.attach",
        fx.worker.agent_id.clone(),
        fx.task.task_id.clone(),
        fx.worker.session_id.clone(),
        None,
        1,
        serde_json::to_value(evidence_input(&fx, EvidenceType::SourceRef)).unwrap(),
    );
    let err = fx.kernel.handle_syscall(syscall).unwrap_err();
    assert!(matches!(err, AgentOsError::PermissionDenied(_)));
}

#[test]
fn capability_cannot_exceed_permission_profile() {
    let fx = fixture();
    let err = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["verify.submit".to_string()],
            vec!["verify:*".to_string()],
            1,
            None,
        )
        .unwrap_err();
    assert!(matches!(err, AgentOsError::PermissionDenied(_)));
}

#[test]
fn reviewer_cannot_write_workspace_files() {
    let fx = fixture();
    let reviewer = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_reviewer".to_string(),
            owner: "tester".to_string(),
            local_goal: "review".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: Vec::new(),
        })
        .unwrap();
    let err = fx
        .kernel
        .grant_capability(
            &reviewer.agent_id,
            &fx.task.task_id,
            vec!["artifact.commit".to_string()],
            vec!["workspace:*".to_string()],
            3,
            None,
        )
        .unwrap_err();
    assert!(matches!(err, AgentOsError::PermissionDenied(_)));
}

#[test]
fn syscall_resource_scope_must_be_covered_by_capability_and_permission() {
    let fx = fixture();
    let cap = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.discover".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap();
    let mut denied = SyscallEnvelope::new(
        "tool.discover",
        fx.worker.agent_id.clone(),
        fx.task.task_id.clone(),
        fx.worker.session_id.clone(),
        Some(cap.capability_id.clone()),
        4,
        json!({}),
    );
    denied.resource_scope = json!({"scope": "workspace:src/lib.rs"});
    let err = fx.kernel.handle_syscall(denied).unwrap_err();
    assert!(matches!(err, AgentOsError::PermissionDenied(_)));

    let mut allowed = SyscallEnvelope::new(
        "tool.discover",
        fx.worker.agent_id.clone(),
        fx.task.task_id.clone(),
        fx.worker.session_id.clone(),
        Some(cap.capability_id),
        4,
        json!({}),
    );
    allowed.resource_scope = json!({"scope": "tool:discover"});
    let result = fx.kernel.handle_syscall(allowed).unwrap();
    assert!(result.accepted);
}

#[test]
fn denied_tool_call_is_audited_and_replayable() {
    let fx = fixture();
    let cap = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            1,
            None,
        )
        .unwrap();
    let err = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            cap.capability_id,
            4,
            ToolInvokeInput {
                tool_name: "run_command".to_string(),
                input: json!({
                    "program": "cargo",
                    "args": ["test"],
                    "cwd": "."
                }),
                evidence_claim: Some("tests were run".to_string()),
            },
        )
        .unwrap_err();
    assert!(matches!(err, AgentOsError::PermissionDenied(_)));

    let state = fx.kernel.state_snapshot().unwrap();
    assert!(state
        .tool_invocations
        .values()
        .any(|invocation| invocation.tool_name == "run_command"
            && invocation.status == ToolCallStatus::Denied));
    assert!(state.audit_events.values().any(|audit| {
        audit.action == "tool.invoke"
            && audit.resource_type == "tool_invocation"
            && audit.result == AuditResult::Deny
    }));

    let replayed = Kernel::from_events(&fx.kernel.events().unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert!(replayed_state
        .tool_invocations
        .values()
        .any(|invocation| invocation.tool_name == "run_command"
            && invocation.status == ToolCallStatus::Denied));
}

#[test]
fn high_risk_capability_requires_active_bounded_approval() {
    let fx = fixture();
    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "tester".to_string(),
            local_goal: "orchestrate".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: Vec::new(),
        })
        .unwrap();

    let missing_approval = fx
        .kernel
        .grant_capability(
            &supervisor.agent_id,
            &fx.task.task_id,
            vec!["agent.spawn".to_string()],
            vec!["agent:*".to_string()],
            6,
            None,
        )
        .unwrap_err();
    assert!(matches!(
        missing_approval,
        AgentOsError::ApprovalRequired(_)
    ));

    let wrong_scope = fx
        .kernel
        .request_approval(RequestApprovalInput {
            goal_id: fx.goal.goal_id.clone(),
            task_id: Some(fx.task.task_id.clone()),
            requested_by_agent_id: supervisor.agent_id.clone(),
            approval_type: ApprovalType::Human,
            scope: ApprovalScope {
                syscall_types: vec!["tool.invoke".to_string()],
                resource_scopes: Vec::new(),
                risk_ceiling: 6,
                goal_id: fx.goal.goal_id.clone(),
                task_id: Some(fx.task.task_id.clone()),
            },
            risk_level: 6,
            expires_at: None,
        })
        .unwrap();
    fx.kernel
        .record_approval(RecordApprovalInput {
            approval_id: wrong_scope.approval_id.clone(),
            status: ApprovalStatus::Approved,
            decision_by: "human".to_string(),
            decision_reason: Some("approve wrong action".to_string()),
        })
        .unwrap();
    let wrong_scope_err = fx
        .kernel
        .grant_capability(
            &supervisor.agent_id,
            &fx.task.task_id,
            vec!["agent.spawn".to_string()],
            vec!["agent:*".to_string()],
            6,
            Some(wrong_scope.approval_id),
        )
        .unwrap_err();
    assert!(matches!(wrong_scope_err, AgentOsError::ApprovalRequired(_)));

    let approval = fx
        .kernel
        .request_approval(RequestApprovalInput {
            goal_id: fx.goal.goal_id.clone(),
            task_id: Some(fx.task.task_id.clone()),
            requested_by_agent_id: supervisor.agent_id.clone(),
            approval_type: ApprovalType::Human,
            scope: ApprovalScope {
                syscall_types: vec!["agent.spawn".to_string()],
                resource_scopes: Vec::new(),
                risk_ceiling: 6,
                goal_id: fx.goal.goal_id.clone(),
                task_id: Some(fx.task.task_id.clone()),
            },
            risk_level: 6,
            expires_at: None,
        })
        .unwrap();
    fx.kernel
        .record_approval(RecordApprovalInput {
            approval_id: approval.approval_id.clone(),
            status: ApprovalStatus::Approved,
            decision_by: "human".to_string(),
            decision_reason: Some("approve child spawn".to_string()),
        })
        .unwrap();
    let cap = fx
        .kernel
        .grant_capability(
            &supervisor.agent_id,
            &fx.task.task_id,
            vec!["agent.spawn".to_string()],
            vec!["agent:*".to_string()],
            6,
            Some(approval.approval_id),
        )
        .unwrap();
    let syscall = SyscallEnvelope::new(
        "agent.spawn",
        supervisor.agent_id.clone(),
        fx.task.task_id.clone(),
        supervisor.session_id.clone(),
        Some(cap.capability_id),
        6,
        serde_json::to_value(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_worker".to_string(),
            owner: "tester".to_string(),
            local_goal: "inspect".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: Some(supervisor.thread_id),
            workspace_roots: Vec::new(),
        })
        .unwrap(),
    );
    let result = fx.kernel.handle_syscall(syscall).unwrap();
    assert!(result.accepted);
}

#[test]
fn expired_or_redecided_approval_is_rejected() {
    let fx = fixture();
    let expired = fx
        .kernel
        .request_approval(RequestApprovalInput {
            goal_id: fx.goal.goal_id.clone(),
            task_id: Some(fx.task.task_id.clone()),
            requested_by_agent_id: fx.worker.agent_id.clone(),
            approval_type: ApprovalType::Human,
            scope: ApprovalScope {
                syscall_types: vec!["artifact.commit".to_string()],
                resource_scopes: Vec::new(),
                risk_ceiling: 4,
                goal_id: fx.goal.goal_id.clone(),
                task_id: Some(fx.task.task_id.clone()),
            },
            risk_level: 4,
            expires_at: Some("1970-01-01T00:00:00Z".to_string()),
        })
        .unwrap_err();
    assert!(matches!(expired, AgentOsError::InvalidTransition(_)));

    let approval = fx
        .kernel
        .request_approval(RequestApprovalInput {
            goal_id: fx.goal.goal_id.clone(),
            task_id: Some(fx.task.task_id.clone()),
            requested_by_agent_id: fx.worker.agent_id.clone(),
            approval_type: ApprovalType::Human,
            scope: ApprovalScope {
                syscall_types: vec!["artifact.commit".to_string()],
                resource_scopes: Vec::new(),
                risk_ceiling: 4,
                goal_id: fx.goal.goal_id.clone(),
                task_id: Some(fx.task.task_id.clone()),
            },
            risk_level: 4,
            expires_at: None,
        })
        .unwrap();
    fx.kernel
        .record_approval(RecordApprovalInput {
            approval_id: approval.approval_id.clone(),
            status: ApprovalStatus::Approved,
            decision_by: "human".to_string(),
            decision_reason: None,
        })
        .unwrap();
    let repeat = fx
        .kernel
        .record_approval(RecordApprovalInput {
            approval_id: approval.approval_id,
            status: ApprovalStatus::Denied,
            decision_by: "human".to_string(),
            decision_reason: Some("changed mind".to_string()),
        })
        .unwrap_err();
    assert!(matches!(repeat, AgentOsError::InvalidTransition(_)));
}
