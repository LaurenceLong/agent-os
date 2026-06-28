use super::scope::{requested_resource_scopes, requested_scope_key, scope_list_allows};
use crate::util::rfc3339_is_past;
use agent_os_sys::*;
use serde_json::Value;

pub(super) fn validate_capability_approval(
    approval: &Approval,
    syscalls: &[String],
    risk_ceiling: u8,
    goal_id: &str,
    task_id: &str,
) -> AgentOsResult<()> {
    if !approval_is_active(approval)? {
        return Err(AgentOsError::ApprovalRequired(
            "approval is not active".to_string(),
        ));
    }
    if approval.scope.risk_ceiling < risk_ceiling {
        return Err(AgentOsError::ApprovalRequired(
            "approval risk ceiling is lower than capability risk ceiling".to_string(),
        ));
    }
    if approval.scope.goal_id != goal_id {
        return Err(AgentOsError::ApprovalRequired(
            "approval goal scope does not match capability".to_string(),
        ));
    }
    if !approval_task_allows(approval, task_id) {
        return Err(AgentOsError::ApprovalRequired(
            "approval task scope does not match capability".to_string(),
        ));
    }
    for syscall in syscalls {
        if !approval.scope.syscall_types.contains(syscall) {
            return Err(AgentOsError::ApprovalRequired(format!(
                "approval does not cover syscall {syscall}"
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_syscall_approval(
    approval: &Approval,
    syscall: &SyscallEnvelope,
    goal_id: &str,
) -> AgentOsResult<()> {
    if !approval_is_active(approval)? {
        return Err(AgentOsError::ApprovalRequired(
            "approval is not active".to_string(),
        ));
    }
    if !approval.scope.syscall_types.contains(&syscall.syscall_type) {
        return Err(AgentOsError::ApprovalRequired(
            "approval does not cover requested syscall".to_string(),
        ));
    }
    if approval.scope.risk_ceiling < syscall.risk_level {
        return Err(AgentOsError::ApprovalRequired(
            "approval risk ceiling is lower than syscall risk".to_string(),
        ));
    }
    if approval.scope.goal_id != goal_id {
        return Err(AgentOsError::ApprovalRequired(
            "approval goal scope does not match syscall".to_string(),
        ));
    }
    if !approval_task_allows(approval, &syscall.task_id) {
        return Err(AgentOsError::ApprovalRequired(
            "approval task scope does not match syscall".to_string(),
        ));
    }
    if !approval_resource_allows(approval, &syscall.resource_scope) {
        return Err(AgentOsError::ApprovalRequired(
            "approval resource scope does not match syscall".to_string(),
        ));
    }
    Ok(())
}

fn approval_is_active(approval: &Approval) -> AgentOsResult<bool> {
    if approval.status != ApprovalStatus::Approved {
        return Ok(false);
    }
    if let Some(expires_at) = &approval.expires_at {
        return Ok(!rfc3339_is_past(expires_at)?);
    }
    Ok(true)
}

fn approval_task_allows(approval: &Approval, task_id: &str) -> bool {
    approval
        .scope
        .task_id
        .as_ref()
        .is_none_or(|approved_task_id| approved_task_id == task_id)
}

fn approval_resource_allows(approval: &Approval, requested: &Value) -> bool {
    let requested = match requested_resource_scopes(requested) {
        Ok(scopes) => scopes,
        Err(_) => return false,
    };
    if requested.is_empty() {
        return true;
    }
    let approved: Vec<String> = approval
        .scope
        .resource_scopes
        .iter()
        .filter_map(|scope| requested_scope_key(scope).ok())
        .flatten()
        .collect();
    !approved.is_empty()
        && requested
            .iter()
            .all(|scope| scope_list_allows(&approved, scope))
}
