use crate::common::*;
use std::{fs, path::PathBuf};

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
fn reviewer_and_producer_have_equivalent_baseline_permissions() {
    let fx = fixture();
    let producer = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_producer".to_string(),
            owner: "tester".to_string(),
            goal: "produce".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: Vec::new(),
        })
        .unwrap();
    let reviewer = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_reviewer".to_string(),
            owner: "tester".to_string(),
            goal: "review".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: Vec::new(),
        })
        .unwrap();
    assert_eq!(
        producer.effective_permissions_snapshot,
        reviewer.effective_permissions_snapshot
    );
    assert!(reviewer
        .effective_permissions_snapshot
        .allowed_tool_names
        .contains(&"apply_patch".to_string()));
    assert!(reviewer
        .effective_permissions_snapshot
        .allowed_tool_names
        .contains(&"run_command".to_string()));
    assert!(!reviewer
        .effective_permissions_snapshot
        .allowed_tool_names
        .contains(&"agent_control".to_string()));
    assert!(!reviewer
        .effective_permissions_snapshot
        .allowed_tool_names
        .contains(&"set_goal".to_string()));
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
                    "mode": "exec",
                    "command": "cargo",
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
            goal: "orchestrate".to_string(),
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
            role_profile_id: "role_producer".to_string(),
            owner: "tester".to_string(),
            goal: "inspect".to_string(),
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

#[test]
fn s_level_gate_denies_control_plane_tools_to_nested_supervisor() {
    let fx = fixture();
    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "tester".to_string(),
            goal: "root supervisor".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: Vec::new(),
        })
        .unwrap();
    let nested = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: supervisor.agent_id.clone(),
            goal: "nested supervisor".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: Some(supervisor.thread_id.clone()),
            workspace_roots: Vec::new(),
        })
        .unwrap();
    assert_eq!(nested.security_level, SecurityLevel(2));
    let cap = fx
        .kernel
        .grant_capability(
            &nested.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap();

    let set_goal = fx
        .kernel
        .invoke_tool(
            &nested.agent_id,
            &fx.task.task_id,
            &nested.session_id,
            cap.capability_id.clone(),
            2,
            ToolInvokeInput {
                tool_name: "set_goal".to_string(),
                input: json!({"goal": "try nested retarget"}),
                evidence_claim: None,
            },
        )
        .unwrap_err();
    assert!(matches!(set_goal, AgentOsError::PermissionDenied(_)));

    let agent_control = fx
        .kernel
        .invoke_tool(
            &nested.agent_id,
            &fx.task.task_id,
            &nested.session_id,
            cap.capability_id,
            1,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: json!({"action": "status", "thread_id": nested.thread_id}),
                evidence_claim: None,
            },
        )
        .unwrap_err();
    assert!(matches!(agent_control, AgentOsError::PermissionDenied(_)));
}

#[test]
fn parent_approved_session_permission_enables_child_tool_call() {
    let fx = fixture();
    let workspace = temp_workspace("agent-os-session-permission");
    fs::create_dir_all(&workspace).unwrap();
    let (supervisor, child) = supervisor_and_reviewer_child(&fx, &workspace);
    let child_low_cap = fx
        .kernel
        .grant_capability(
            &child.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            1,
            None,
        )
        .unwrap();
    let request = fx
        .kernel
        .invoke_tool(
            &child.agent_id,
            &fx.task.task_id,
            &child.session_id,
            child_low_cap.capability_id,
            1,
            ToolInvokeInput {
                tool_name: "request_permissions".to_string(),
                input: json!({
                    "reason": "write the reviewed file after parent approval",
                    "scope": "session",
                    "permissions": apply_patch_permission()
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    let request_id = request.output.as_ref().unwrap()["permission_request_id"]
        .as_str()
        .unwrap();

    approve_permission_request(&fx, &supervisor, request_id, apply_patch_permission());
    let elevated_cap = fx
        .kernel
        .grant_capability(
            &child.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap();
    attach_workspace_for_agent(&fx.kernel, &child, &fx.task.task_id, &workspace);
    fx.kernel
        .invoke_tool(
            &child.agent_id,
            &fx.task.task_id,
            &child.session_id,
            elevated_cap.capability_id,
            4,
            ToolInvokeInput {
                tool_name: "apply_patch".to_string(),
                input: json!({
                    "workspace_root": workspace.to_string_lossy(),
                    "patch": "*** Begin Patch\n*** Add File: approved.txt\n+approved\n*** End Patch\n"
                }),
                evidence_claim: Some("approved child apply_patch succeeded".to_string()),
            },
        )
        .unwrap();
    assert_eq!(
        fs::read_to_string(workspace.join("approved.txt")).unwrap(),
        "approved\n"
    );
    let replayed = Kernel::from_events(&fx.kernel.events().unwrap()).unwrap();
    assert_eq!(
        replayed.state_snapshot().unwrap().permission_grants.len(),
        1
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn denied_permission_request_does_not_change_child_authority() {
    let fx = fixture();
    let workspace = temp_workspace("agent-os-denied-permission");
    fs::create_dir_all(&workspace).unwrap();
    let (supervisor, child) = supervisor_and_reviewer_child(&fx, &workspace);
    let child_low_cap = fx
        .kernel
        .grant_capability(
            &child.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            1,
            None,
        )
        .unwrap();
    let request = fx
        .kernel
        .invoke_tool(
            &child.agent_id,
            &fx.task.task_id,
            &child.session_id,
            child_low_cap.capability_id,
            1,
            ToolInvokeInput {
                tool_name: "request_permissions".to_string(),
                input: json!({
                    "reason": "try to write without approval",
                    "scope": "session",
                    "permissions": apply_patch_permission()
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    let request_id = request.output.as_ref().unwrap()["permission_request_id"]
        .as_str()
        .unwrap();
    deny_permission_request(&fx, &supervisor, request_id);

    let denied = fx
        .kernel
        .grant_capability(
            &child.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap_err();
    assert!(matches!(denied, AgentOsError::PermissionDenied(_)));
    assert!(fx
        .kernel
        .state_snapshot()
        .unwrap()
        .permission_grants
        .is_empty());
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn turn_scoped_permission_grant_expires_after_turn_completes() {
    let fx = fixture();
    let workspace = temp_workspace("agent-os-turn-permission");
    fs::create_dir_all(&workspace).unwrap();
    let (supervisor, child) = supervisor_and_reviewer_child(&fx, &workspace);
    fx.kernel.start_turn(&child.thread_id).unwrap();
    let child_low_cap = fx
        .kernel
        .grant_capability(
            &child.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            1,
            None,
        )
        .unwrap();
    let request = fx
        .kernel
        .invoke_tool(
            &child.agent_id,
            &fx.task.task_id,
            &child.session_id,
            child_low_cap.capability_id,
            1,
            ToolInvokeInput {
                tool_name: "request_permissions".to_string(),
                input: json!({
                    "reason": "temporary write during this turn",
                    "scope": "turn",
                    "permissions": apply_patch_permission()
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    let request_id = request.output.as_ref().unwrap()["permission_request_id"]
        .as_str()
        .unwrap();
    approve_permission_request(&fx, &supervisor, request_id, apply_patch_permission());
    fx.kernel
        .grant_capability(
            &child.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap();
    fx.kernel
        .transition_thread(&child.thread_id, ThreadStatus::Ready, None)
        .unwrap();
    let expired = fx
        .kernel
        .grant_capability(
            &child.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap_err();
    assert!(matches!(expired, AgentOsError::PermissionDenied(_)));
    let _ = fs::remove_dir_all(workspace);
}

fn supervisor_and_reviewer_child(
    fx: &Fixture,
    workspace: &std::path::Path,
) -> (AgentControlBlock, AgentControlBlock) {
    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "tester".to_string(),
            goal: "approve child permissions".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
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
    let child_start = fx
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
                        "goal": "request write permission",
                        "role_profile_id": "role_reviewer",
                        "workspace_roots": [workspace.to_string_lossy()],
                        "permissions": restricted_permission_requester_permission()
                    }
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    let child_thread_id = child_start.output.as_ref().unwrap()["thread_id"]
        .as_str()
        .unwrap();
    let child = fx
        .kernel
        .state_snapshot()
        .unwrap()
        .threads
        .get(child_thread_id)
        .cloned()
        .unwrap();
    (supervisor, child)
}

fn approve_permission_request(
    fx: &Fixture,
    supervisor: &AgentControlBlock,
    permission_request_id: &str,
    permissions: serde_json::Value,
) {
    let parent_cap = fx
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
    fx.kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            parent_cap.capability_id,
            4,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: json!({
                    "action": "approve_permission",
                    "payload": {
                        "permission_request_id": permission_request_id,
                        "permissions": permissions
                    }
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
}

fn deny_permission_request(
    fx: &Fixture,
    supervisor: &AgentControlBlock,
    permission_request_id: &str,
) {
    let parent_cap = fx
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
    fx.kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            parent_cap.capability_id,
            4,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: json!({
                    "action": "deny_permission",
                    "payload": {"permission_request_id": permission_request_id}
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
}

fn attach_workspace_for_agent(
    kernel: &Kernel,
    agent: &AgentControlBlock,
    task_id: &str,
    workspace: &std::path::Path,
) {
    let env = kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            workspace.to_string_lossy(),
            "sbox_workspace_write",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    kernel
        .attach_environment(
            &env.environment_id,
            &agent.agent_id,
            &agent.thread_id,
            task_id,
            AttachMode::WorkspaceWrite,
        )
        .unwrap();
}

fn apply_patch_permission() -> serde_json::Value {
    json!({
        "max_risk_level": 4,
        "allowed_syscalls": ["tool.invoke"],
        "resource_scopes": ["tool:*"],
        "allowed_tool_names": ["apply_patch"],
        "allowed_tool_driver_classes": ["filesystem"],
        "approval_required_above": 4,
        "requires_evidence_for": []
    })
}

fn restricted_permission_requester_permission() -> serde_json::Value {
    json!({
        "max_risk_level": 1,
        "allowed_syscalls": ["tool.invoke"],
        "resource_scopes": ["tool:*"],
        "allowed_tool_names": ["request_permissions"],
        "allowed_tool_driver_classes": ["kernel_builtin"],
        "approval_required_above": 1,
        "requires_evidence_for": []
    })
}

fn temp_workspace(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        new_id("case_")
    ))
}
