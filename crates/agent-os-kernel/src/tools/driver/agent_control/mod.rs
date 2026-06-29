//! `agent_control` tool driver.
//!
//! Dispatch lives here; the implementation is split by ownership into focused
//! submodules:
//! - [`action`] parses action names and enforces risk gating.
//! - [`target`] resolves target threads and child workspace roots.
//! - [`hooks`] configures agent hooks.
//! - [`command`] records durable control commands.
//! - [`lifecycle`] applies stateful lifecycle actions.

mod action;
mod command;
mod hooks;
mod lifecycle;
mod target;

use super::string_array;
use crate::util::{parse_payload, required_string};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(super) fn run_agent_control(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let action_text = required_string(input, "action")?;
    let action = action::parse_agent_control_action(&action_text)?;
    action::require_agent_control_action_risk(action, syscall.risk_level)?;
    let payload = input.get("payload").cloned().unwrap_or_else(|| json!({}));
    let requester = kernel
        .thread_by_agent(&syscall.agent_id)?
        .ok_or_else(|| AgentOsError::NotFound(format!("agent {}", syscall.agent_id)))?;
    kernel.require_control_plane_security_level(&requester, "agent_control")?;
    match action {
        AgentControlAction::Start => {
            let goal = required_string(&payload, "goal")?;
            let explicit_permissions = payload.get("permissions").map(parse_payload).transpose()?;
            let role_profile_id = payload
                .get("role_profile_id")
                .and_then(Value::as_str)
                .unwrap_or("role_worker")
                .to_string();
            let task_id = payload
                .get("task_id")
                .and_then(Value::as_str)
                .unwrap_or(&syscall.task_id)
                .to_string();
            let workspace_roots = target::agent_control_workspace_roots(
                &payload,
                &requester.config_snapshot.workspace_roots,
            )?;
            let child = kernel.spawn_agent_with_permissions_with_cause(
                SpawnAgentInput {
                    task_id,
                    role_profile_id,
                    owner: syscall.agent_id.clone(),
                    goal,
                    success_criteria: string_array(&payload, "success_criteria")?,
                    failure_criteria: string_array(&payload, "failure_criteria")?,
                    parent_thread_id: Some(requester.thread_id.clone()),
                    workspace_roots,
                },
                explicit_permissions,
                Some(syscall.syscall_id.clone()),
            )?;
            let hooks = hooks::configure_start_hooks(kernel, syscall, &child, &payload)?;
            command::record_agent_control_command(
                kernel,
                syscall,
                &requester,
                Some(&child),
                action,
                payload,
                AgentControlCommandStatus::Applied,
            )?;
            Ok(json!({
                "tool": descriptor.name.clone(),
                "status": "ok",
                "action": action_text,
                "driver_class": descriptor.driver_class,
                "agent_id": child.agent_id,
                "thread_id": child.thread_id,
                "invocation_id": child.invocation_id,
                "security_level": child.security_level,
                "thread_status": child.status,
                "session_id": child.session_id,
                "goal": child.task.goal,
                "output_handle": format!("thread:{}", child.thread_id),
                "hooks": hooks,
            }))
        }
        AgentControlAction::Status => {
            let target = target::resolve_agent_control_target(kernel, input, &payload)?;
            require_supervision_target(&requester, &target)?;
            let hooks = hooks::agent_hooks_for(kernel, &target.agent_id)?;
            Ok(json!({
                "tool": descriptor.name.clone(),
                "status": "ok",
                "action": action_text,
                "driver_class": descriptor.driver_class,
                "agent_id": target.agent_id,
                "thread_id": target.thread_id,
                "invocation_id": target.invocation_id,
                "security_level": target.security_level,
                "thread_status": target.status,
                "session_id": target.session_id,
                "hooks": hooks,
            }))
        }
        AgentControlAction::ApprovePermission | AgentControlAction::DenyPermission => {
            let permission_request_id = required_string(&payload, "permission_request_id")?;
            let decision_reason = payload
                .get("decision_reason")
                .and_then(Value::as_str)
                .map(str::to_string);
            let granted_permissions = if action == AgentControlAction::ApprovePermission {
                let permissions = payload.get("permissions").ok_or_else(|| {
                    AgentOsError::Validation(
                        "approve_permission requires payload.permissions".to_string(),
                    )
                })?;
                let permissions: PermissionSet = parse_payload(permissions)?;
                if permissions.max_risk_level > syscall.risk_level {
                    return Err(AgentOsError::PermissionDenied(format!(
                        "approve_permission requires risk level {}",
                        permissions.max_risk_level
                    )));
                }
                Some(permissions)
            } else {
                None
            };
            let (request, grant) = kernel.respond_permission_request_with_cause(
                &syscall.agent_id,
                &permission_request_id,
                granted_permissions,
                decision_reason,
                Some(syscall.syscall_id.clone()),
            )?;
            let target = kernel
                .read_state()?
                .threads
                .get(&request.requester_thread_id)
                .cloned();
            let command = command::record_agent_control_command(
                kernel,
                syscall,
                &requester,
                target.as_ref(),
                action,
                payload,
                AgentControlCommandStatus::Applied,
            )?;
            Ok(json!({
                "tool": descriptor.name.clone(),
                "status": "ok",
                "action": action_text,
                "driver_class": descriptor.driver_class,
                "command_id": command.command_id,
                "permission_request_id": request.permission_request_id,
                "request_status": request.status,
                "permission_grant_id": grant.as_ref().map(|grant| grant.permission_grant_id.as_str()),
                "scope": request.scope,
                "target_agent_id": request.requester_agent_id,
                "target_thread_id": request.requester_thread_id,
            }))
        }
        AgentControlAction::SetHook => {
            let target = target::resolve_agent_control_target(kernel, input, &payload)?;
            require_supervision_target(&requester, &target)?;
            let hook = hooks::configure_agent_hook(kernel, syscall, &target, &payload)?;
            command::record_agent_control_command(
                kernel,
                syscall,
                &requester,
                Some(&target),
                action,
                payload,
                AgentControlCommandStatus::Applied,
            )?;
            Ok(json!({
                "tool": descriptor.name.clone(),
                "status": "ok",
                "action": action_text,
                "driver_class": descriptor.driver_class,
                "agent_id": target.agent_id,
                "thread_id": target.thread_id,
                "hook": hook,
            }))
        }
        AgentControlAction::Output
        | AgentControlAction::Send
        | AgentControlAction::Resume
        | AgentControlAction::Stop
        | AgentControlAction::SetTimeout
        | AgentControlAction::ExportTrace
        | AgentControlAction::Kill
        | AgentControlAction::DeleteSession
        | AgentControlAction::PurgeState => {
            let target = target::resolve_agent_control_target(kernel, input, &payload)?;
            require_supervision_target(&requester, &target)?;
            let action_result =
                lifecycle::apply_lifecycle_action(kernel, syscall, action, &target, &payload)?;
            let command = command::record_agent_control_command(
                kernel,
                syscall,
                &requester,
                Some(&target),
                action,
                payload,
                AgentControlCommandStatus::Applied,
            )?;
            Ok(json!({
                "tool": descriptor.name.clone(),
                "status": "ok",
                "action": action_text,
                "driver_class": descriptor.driver_class,
                "agent_id": target.agent_id,
                "thread_id": target.thread_id,
                "command_id": command.command_id,
                "thread_status": action_result.thread_status,
                "output": action_result.output,
                "cursor": null,
            }))
        }
    }
}

fn require_supervision_target(
    requester: &AgentControlBlock,
    target: &AgentControlBlock,
) -> AgentOsResult<()> {
    if target.thread_id == requester.thread_id
        || target.parent_thread_id.as_deref() == Some(&requester.thread_id)
    {
        return Ok(());
    }
    Err(AgentOsError::PermissionDenied(
        "agent_control can only target the requester thread or a direct child".to_string(),
    ))
}

/// Result of a stateful lifecycle action: the new thread status plus any
/// structured output payload surfaced back to the model.
struct AgentControlActionResult {
    thread_status: ThreadStatus,
    output: Value,
}
