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
use crate::util::required_string;
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
    match action {
        AgentControlAction::Start => {
            let assignment = required_string(&payload, "assignment")?;
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
            let child = kernel.spawn_agent_with_cause(
                SpawnAgentInput {
                    task_id,
                    role_profile_id,
                    owner: syscall.agent_id.clone(),
                    local_goal: assignment,
                    success_criteria: string_array(&payload, "success_criteria")?,
                    failure_criteria: string_array(&payload, "failure_criteria")?,
                    parent_thread_id: Some(requester.thread_id.clone()),
                    workspace_roots,
                },
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
                "supervisor_level": child.supervisor_level,
                "thread_status": child.status,
                "session_id": child.session_id,
                "output_handle": format!("thread:{}", child.thread_id),
                "hooks": hooks,
            }))
        }
        AgentControlAction::Status => {
            let target = target::resolve_agent_control_target(kernel, input, &payload)?;
            let hooks = hooks::agent_hooks_for(kernel, &target.agent_id)?;
            Ok(json!({
                "tool": descriptor.name.clone(),
                "status": "ok",
                "action": action_text,
                "driver_class": descriptor.driver_class,
                "agent_id": target.agent_id,
                "thread_id": target.thread_id,
                "invocation_id": target.invocation_id,
                "supervisor_level": target.supervisor_level,
                "thread_status": target.status,
                "session_id": target.session_id,
                "hooks": hooks,
            }))
        }
        AgentControlAction::SetHook => {
            let target = target::resolve_agent_control_target(kernel, input, &payload)?;
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
        | AgentControlAction::Kill => {
            let target = target::resolve_agent_control_target(kernel, input, &payload)?;
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
        AgentControlAction::DeleteSession | AgentControlAction::PurgeState => {
            let target = target::resolve_agent_control_target(kernel, input, &payload)?;
            command::record_agent_control_command(
                kernel,
                syscall,
                &requester,
                Some(&target),
                action,
                payload,
                AgentControlCommandStatus::Rejected,
            )?;
            Err(AgentOsError::Validation(format!(
                "agent_control action {action_text} is rejected by the append-only v0.1 store"
            )))
        }
    }
}

/// Result of a stateful lifecycle action: the new thread status plus any
/// structured output payload surfaced back to the model.
struct AgentControlActionResult {
    thread_status: ThreadStatus,
    output: Value,
}
