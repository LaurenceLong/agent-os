use crate::*;
use agent_os_sys::*;
use std::collections::BTreeSet;

impl Kernel {
    pub(crate) fn effective_permission_set(
        &self,
        thread: &AgentControlBlock,
    ) -> AgentOsResult<PermissionSet> {
        let state = self.read_state()?;
        Ok(effective_permission_set_for_thread(&state, thread))
    }

    pub(crate) fn require_control_plane_security_level(
        &self,
        thread: &AgentControlBlock,
        tool_name: &str,
    ) -> AgentOsResult<()> {
        if thread.security_level.allows_control_plane() {
            return Ok(());
        }
        Err(AgentOsError::PermissionDenied(format!(
            "{tool_name} requires security_level <= 1"
        )))
    }

    pub(crate) fn require_tool_authority(
        &self,
        thread: &AgentControlBlock,
        descriptor: &ToolDescriptor,
        risk_level: u8,
    ) -> AgentOsResult<()> {
        if matches!(descriptor.name.as_str(), "agent_control" | "set_goal") {
            self.require_control_plane_security_level(thread, &descriptor.name)?;
        }
        let effective = self.effective_permission_set(thread)?;
        require_tool_permission(&effective, descriptor, risk_level)
    }

    pub(crate) fn child_permission_snapshot(
        &self,
        parent: Option<&AgentControlBlock>,
        role_permission: &PermissionSet,
        explicit_permissions: Option<PermissionSet>,
    ) -> AgentOsResult<PermissionSet> {
        let parent_permissions = match parent {
            Some(parent) => self.effective_permission_set(parent)?,
            None => role_permission.clone(),
        };
        let requested = explicit_permissions.unwrap_or_else(|| role_permission.clone());
        if !permission_set_is_subset(&requested, &parent_permissions) {
            return Err(AgentOsError::PermissionDenied(
                "child permissions must be a subset of parent effective permissions".to_string(),
            ));
        }
        Ok(intersect_permission_sets(&requested, role_permission))
    }

    pub(crate) fn request_permissions_with_cause(
        &self,
        requester_agent_id: &str,
        reason: String,
        requested_permissions: PermissionSet,
        scope: PermissionGrantScope,
        causation_id: Option<String>,
    ) -> AgentOsResult<PermissionRequest> {
        if requested_permissions_is_empty(&requested_permissions) {
            return Err(AgentOsError::Validation(
                "request_permissions requires at least one permission field".to_string(),
            ));
        }
        let state = self.read_state()?;
        let requester = state
            .threads
            .values()
            .find(|thread| thread.agent_id == requester_agent_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("agent {requester_agent_id}")))?;
        let parent_thread_id = requester.parent_thread_id.as_ref().ok_or_else(|| {
            AgentOsError::PermissionDenied(
                "permission requests require a direct parent approver".to_string(),
            )
        })?;
        let approver = state
            .threads
            .get(parent_thread_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {parent_thread_id}")))?;
        let turn_id = match scope {
            PermissionGrantScope::Turn => {
                Some(requester.active_turn.turn_id.clone().ok_or_else(|| {
                    AgentOsError::Validation(
                        "turn-scoped permission requests require an active turn".to_string(),
                    )
                })?)
            }
            PermissionGrantScope::Session => None,
        };
        drop(state);

        let now = now_rfc3339();
        let request = PermissionRequest {
            permission_request_id: new_id("permreq_"),
            requester_agent_id: requester.agent_id.clone(),
            requester_thread_id: requester.thread_id.clone(),
            approver_agent_id: Some(approver.agent_id.clone()),
            approver_thread_id: Some(approver.thread_id.clone()),
            task_id: requester.task.task_id.clone(),
            goal_id: requester.task.goal_id.clone(),
            session_id: requester.session_id.clone(),
            turn_id,
            requested_permissions,
            granted_permissions: None,
            scope,
            reason,
            status: PermissionRequestStatus::Pending,
            decision_reason: None,
            created_at: now.clone(),
            updated_at: now,
        };
        self.emit(
            "PermissionRequested",
            "permission_request",
            &request.permission_request_id,
            Some(request.requester_agent_id.clone()),
            Some(request.task_id.clone()),
            causation_id,
            Some(request.goal_id.clone()),
            &request,
        )?;
        Ok(request)
    }

    pub(crate) fn respond_permission_request_with_cause(
        &self,
        approver_agent_id: &str,
        permission_request_id: &str,
        granted_permissions: Option<PermissionSet>,
        decision_reason: Option<String>,
        causation_id: Option<String>,
    ) -> AgentOsResult<(PermissionRequest, Option<PermissionGrant>)> {
        let state = self.read_state()?;
        let approver = state
            .threads
            .values()
            .find(|thread| thread.agent_id == approver_agent_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("agent {approver_agent_id}")))?;
        self.require_control_plane_security_level(&approver, "agent_control approve_permission")?;
        let mut request = state
            .permission_requests
            .get(permission_request_id)
            .cloned()
            .ok_or_else(|| {
                AgentOsError::NotFound(format!("permission request {permission_request_id}"))
            })?;
        if request.approver_thread_id.as_deref() != Some(&approver.thread_id) {
            return Err(AgentOsError::PermissionDenied(
                "permission request can only be answered by the direct parent".to_string(),
            ));
        }
        if request.status != PermissionRequestStatus::Pending {
            if request.status == PermissionRequestStatus::Denied && granted_permissions.is_none() {
                return Ok((request, None));
            }
            return Err(AgentOsError::InvalidTransition(format!(
                "permission request {:?} -> response",
                request.status
            )));
        }
        let requester = state
            .threads
            .get(&request.requester_thread_id)
            .cloned()
            .ok_or_else(|| {
                AgentOsError::NotFound(format!("thread {}", request.requester_thread_id))
            })?;
        let approver_permissions = effective_permission_set_for_thread(&state, &approver);
        drop(state);

        let now = now_rfc3339();
        let mut grant = None;
        if let Some(candidate) = granted_permissions {
            if !permission_set_is_subset(&candidate, &request.requested_permissions) {
                return Err(AgentOsError::PermissionDenied(
                    "granted permissions must be a subset of requested permissions".to_string(),
                ));
            }
            let normalized = intersect_permission_sets(&candidate, &approver_permissions);
            if !requested_permissions_is_empty(&normalized) {
                request.status = PermissionRequestStatus::Approved;
                request.granted_permissions = Some(normalized.clone());
                grant = Some(PermissionGrant {
                    permission_grant_id: new_id("permgrant_"),
                    permission_request_id: request.permission_request_id.clone(),
                    agent_id: requester.agent_id.clone(),
                    thread_id: requester.thread_id.clone(),
                    task_id: requester.task.task_id.clone(),
                    goal_id: requester.task.goal_id.clone(),
                    granted_by_agent_id: approver.agent_id.clone(),
                    granted_by_thread_id: approver.thread_id.clone(),
                    permissions: normalized,
                    scope: request.scope,
                    session_id: match request.scope {
                        PermissionGrantScope::Session => Some(request.session_id.clone()),
                        PermissionGrantScope::Turn => None,
                    },
                    turn_id: match request.scope {
                        PermissionGrantScope::Turn => request.turn_id.clone(),
                        PermissionGrantScope::Session => None,
                    },
                    created_at: now.clone(),
                });
            } else {
                request.status = PermissionRequestStatus::Denied;
                request.granted_permissions = None;
            }
        } else {
            request.status = PermissionRequestStatus::Denied;
            request.granted_permissions = None;
        }
        request.decision_reason = decision_reason;
        request.updated_at = now;
        self.emit(
            "PermissionRequestResolved",
            "permission_request",
            &request.permission_request_id,
            Some(request.requester_agent_id.clone()),
            Some(request.task_id.clone()),
            causation_id.clone(),
            Some(request.goal_id.clone()),
            &request,
        )?;
        if let Some(grant) = &grant {
            self.emit(
                "PermissionGranted",
                "permission_grant",
                &grant.permission_grant_id,
                Some(grant.agent_id.clone()),
                Some(grant.task_id.clone()),
                causation_id,
                Some(grant.goal_id.clone()),
                grant,
            )?;
        }
        Ok((request, grant))
    }
}

pub(crate) fn effective_permission_set_for_thread(
    state: &KernelState,
    thread: &AgentControlBlock,
) -> PermissionSet {
    let mut effective = thread.effective_permissions_snapshot.clone();
    for grant in state.permission_grants.values() {
        if grant.thread_id != thread.thread_id {
            continue;
        }
        let active = match grant.scope {
            PermissionGrantScope::Session => {
                grant.session_id.as_deref() == Some(&thread.session_id)
            }
            PermissionGrantScope::Turn => {
                grant.turn_id.as_deref() == thread.active_turn.turn_id.as_deref()
                    && matches!(
                        thread.active_turn.status,
                        Some(
                            TurnStatus::InProgress
                                | TurnStatus::AwaitingTool
                                | TurnStatus::AwaitingPermission
                                | TurnStatus::AwaitingUser
                        )
                    )
            }
        };
        if active {
            effective = merge_permission_sets(&effective, &grant.permissions);
        }
    }
    effective
}

pub(crate) fn permission_set_is_subset(child: &PermissionSet, parent: &PermissionSet) -> bool {
    child.max_risk_level <= parent.max_risk_level
        && child.approval_required_above <= parent.approval_required_above
        && child
            .allowed_syscalls
            .iter()
            .all(|syscall| string_list_allows(&parent.allowed_syscalls, syscall))
        && child
            .resource_scopes
            .iter()
            .all(|scope| scope_list_allows(&parent.resource_scopes, scope))
        && child
            .allowed_tool_names
            .iter()
            .all(|tool| string_list_allows(&parent.allowed_tool_names, tool))
        && child
            .allowed_tool_driver_classes
            .iter()
            .all(|class| parent.allowed_tool_driver_classes.contains(class))
}

pub(crate) fn intersect_permission_sets(
    requested: &PermissionSet,
    parent: &PermissionSet,
) -> PermissionSet {
    PermissionSet {
        max_risk_level: requested.max_risk_level.min(parent.max_risk_level),
        allowed_syscalls: intersect_strings(
            &requested.allowed_syscalls,
            &parent.allowed_syscalls,
            string_list_allows,
        ),
        resource_scopes: intersect_strings(
            &requested.resource_scopes,
            &parent.resource_scopes,
            scope_list_allows,
        ),
        allowed_tool_names: intersect_strings(
            &requested.allowed_tool_names,
            &parent.allowed_tool_names,
            string_list_allows,
        ),
        allowed_tool_driver_classes: requested
            .allowed_tool_driver_classes
            .iter()
            .filter(|class| parent.allowed_tool_driver_classes.contains(class))
            .copied()
            .collect(),
        approval_required_above: requested
            .approval_required_above
            .min(parent.approval_required_above),
        requires_evidence_for: union_strings(
            &requested.requires_evidence_for,
            &parent.requires_evidence_for,
        ),
    }
}

fn require_tool_permission(
    permissions: &PermissionSet,
    descriptor: &ToolDescriptor,
    risk_level: u8,
) -> AgentOsResult<()> {
    if risk_level > permissions.max_risk_level || descriptor.risk_level > permissions.max_risk_level
    {
        return Err(AgentOsError::PermissionDenied(
            "tool risk exceeds effective permission ceiling".to_string(),
        ));
    }
    if !string_list_allows(&permissions.allowed_tool_names, &descriptor.name)
        && descriptor.driver_class != ToolDriverClass::Mcp
    {
        return Err(AgentOsError::PermissionDenied(format!(
            "effective permissions do not allow tool {}",
            descriptor.name
        )));
    }
    if !permissions
        .allowed_tool_driver_classes
        .contains(&descriptor.driver_class)
    {
        return Err(AgentOsError::PermissionDenied(format!(
            "effective permissions do not allow tool driver class {:?}",
            descriptor.driver_class
        )));
    }
    Ok(())
}

fn merge_permission_sets(left: &PermissionSet, right: &PermissionSet) -> PermissionSet {
    PermissionSet {
        max_risk_level: left.max_risk_level.max(right.max_risk_level),
        allowed_syscalls: union_strings(&left.allowed_syscalls, &right.allowed_syscalls),
        resource_scopes: union_strings(&left.resource_scopes, &right.resource_scopes),
        allowed_tool_names: union_strings(&left.allowed_tool_names, &right.allowed_tool_names),
        allowed_tool_driver_classes: union_tool_driver_classes(
            &left.allowed_tool_driver_classes,
            &right.allowed_tool_driver_classes,
        ),
        approval_required_above: left
            .approval_required_above
            .max(right.approval_required_above),
        requires_evidence_for: union_strings(
            &left.requires_evidence_for,
            &right.requires_evidence_for,
        ),
    }
}

fn requested_permissions_is_empty(permissions: &PermissionSet) -> bool {
    permissions.max_risk_level == 0
        && permissions.allowed_syscalls.is_empty()
        && permissions.resource_scopes.is_empty()
        && permissions.allowed_tool_names.is_empty()
        && permissions.allowed_tool_driver_classes.is_empty()
}

fn intersect_strings(
    requested: &[String],
    parent: &[String],
    allows: fn(&[String], &str) -> bool,
) -> Vec<String> {
    requested
        .iter()
        .filter(|value| allows(parent, value))
        .cloned()
        .collect()
}

fn union_strings(left: &[String], right: &[String]) -> Vec<String> {
    left.iter()
        .chain(right.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn union_tool_driver_classes(
    left: &[ToolDriverClass],
    right: &[ToolDriverClass],
) -> Vec<ToolDriverClass> {
    let mut values = left.to_vec();
    for class in right {
        if !values.contains(class) {
            values.push(*class);
        }
    }
    values
}

fn string_list_allows(list: &[String], value: &str) -> bool {
    list.iter().any(|item| item == "*" || item == value)
}

fn scope_list_allows(allowed_scopes: &[String], requested_scope: &str) -> bool {
    allowed_scopes
        .iter()
        .any(|allowed| scope_pattern_allows(allowed, requested_scope))
}

fn scope_pattern_allows(allowed: &str, requested: &str) -> bool {
    if allowed == "*" || allowed == requested {
        return true;
    }
    allowed.strip_suffix(":*").is_some_and(|prefix| {
        requested == prefix
            || requested
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with(':'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permission_set() -> PermissionSet {
        PermissionSet {
            max_risk_level: 2,
            allowed_syscalls: vec!["tool.invoke".to_string()],
            resource_scopes: vec!["tool:read_file".to_string()],
            allowed_tool_names: vec!["read_file".to_string()],
            allowed_tool_driver_classes: vec![ToolDriverClass::Filesystem],
            approval_required_above: 2,
            requires_evidence_for: vec!["read_file".to_string()],
        }
    }

    fn parent_child_fixture() -> (Kernel, AgentControlBlock, AgentControlBlock) {
        let kernel = Kernel::new();
        let goal = kernel
            .register_goal(RegisterGoalInput {
                namespace: "test".to_string(),
                created_by: "user".to_string(),
                title: "Permission".to_string(),
                description: "Permission fixture".to_string(),
                acceptance_criteria: vec!["permission response recorded".to_string()],
                constraints: Vec::new(),
                risk_level: 3,
                deadline: None,
            })
            .unwrap();
        let task = kernel
            .spawn_task(SpawnTaskInput {
                goal_id: goal.goal_id.clone(),
                parent_task_id: None,
                title: "Coordinate".to_string(),
                description: "Coordinate permissions".to_string(),
                depends_on: Vec::new(),
                required_artifact_types: Vec::new(),
                required_evidence_types: Vec::new(),
                priority: 10,
                risk_level: 3,
            })
            .unwrap();
        let parent = kernel
            .spawn_agent(SpawnAgentInput {
                task_id: task.task_id.clone(),
                role_profile_id: "role_supervisor".to_string(),
                owner: "user".to_string(),
                goal: "supervise".to_string(),
                success_criteria: Vec::new(),
                failure_criteria: Vec::new(),
                parent_thread_id: None,
                workspace_roots: Vec::new(),
            })
            .unwrap();
        let child = kernel
            .spawn_agent(SpawnAgentInput {
                task_id: task.task_id,
                role_profile_id: "role_producer".to_string(),
                owner: "user".to_string(),
                goal: "produce".to_string(),
                success_criteria: Vec::new(),
                failure_criteria: Vec::new(),
                parent_thread_id: Some(parent.thread_id.clone()),
                workspace_roots: Vec::new(),
            })
            .unwrap();
        (kernel, parent, child)
    }

    #[test]
    fn duplicate_deny_permission_response_is_idempotent() {
        let (kernel, parent, child) = parent_child_fixture();
        let request = kernel
            .request_permissions_with_cause(
                &child.agent_id,
                "Need bounded read permission".to_string(),
                permission_set(),
                PermissionGrantScope::Session,
                None,
            )
            .unwrap();

        let (first, first_grant) = kernel
            .respond_permission_request_with_cause(
                &parent.agent_id,
                &request.permission_request_id,
                None,
                Some("Denied for test".to_string()),
                None,
            )
            .unwrap();
        let (second, second_grant) = kernel
            .respond_permission_request_with_cause(
                &parent.agent_id,
                &request.permission_request_id,
                None,
                Some("Denied for test".to_string()),
                None,
            )
            .unwrap();

        assert_eq!(first.status, PermissionRequestStatus::Denied);
        assert!(first_grant.is_none());
        assert_eq!(second.status, PermissionRequestStatus::Denied);
        assert!(second_grant.is_none());
    }
}
