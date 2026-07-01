use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

use super::AgentControlActionResult;

/// Apply a stateful `agent_control` lifecycle action to the target thread.
///
/// Returns the resulting thread status and any structured output payload.
pub(super) fn apply_lifecycle_action(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    action: AgentControlAction,
    target: &AgentControlBlock,
    payload: &Value,
) -> AgentOsResult<AgentControlActionResult> {
    match action {
        AgentControlAction::Output => output_for_target(kernel, target, payload),
        AgentControlAction::Send => Ok(AgentControlActionResult {
            thread_status: target.status,
            output: json!({
                "sent": true,
                "payload": payload,
            }),
        }),
        AgentControlAction::Resume => {
            let acb = resume_target(kernel, syscall, target)?;
            Ok(AgentControlActionResult {
                thread_status: acb.status,
                output: json!({
                    "resumed": true,
                    "session_id": acb.session_id,
                }),
            })
        }
        AgentControlAction::Stop => {
            let acb = terminate_target(
                kernel,
                target,
                ThreadStatus::Terminated,
                "agent_control stop",
            )?;
            Ok(AgentControlActionResult {
                thread_status: acb.status,
                output: json!({"stopped": true}),
            })
        }
        AgentControlAction::SetTimeout => {
            let acb = set_timeout_budget(kernel, syscall, target, payload)?;
            Ok(AgentControlActionResult {
                thread_status: acb.status,
                output: json!({
                    "timeout_ms": acb.budgets.wall_time_budget_ms,
                }),
            })
        }
        AgentControlAction::ExportTrace => export_trace_for_target(kernel, target, payload),
        AgentControlAction::Kill => {
            let acb = terminate_target(
                kernel,
                target,
                ThreadStatus::Terminated,
                "agent_control kill",
            )?;
            Ok(AgentControlActionResult {
                thread_status: acb.status,
                output: json!({"killed": true}),
            })
        }
        AgentControlAction::DeleteSession => {
            let (acb, previous_session_id) = delete_session_for_target(kernel, syscall, target)?;
            Ok(AgentControlActionResult {
                thread_status: acb.status,
                output: json!({
                    "deleted_session": true,
                    "previous_session_id": previous_session_id,
                    "session_id": acb.session_id,
                }),
            })
        }
        AgentControlAction::PurgeState => {
            let acb = purge_state_for_target(kernel, syscall, target)?;
            Ok(AgentControlActionResult {
                thread_status: acb.status,
                output: json!({
                    "purged": true,
                    "tombstone_event": "AgentStatePurged",
                    "session_id": acb.session_id,
                }),
            })
        }
        AgentControlAction::Start
        | AgentControlAction::Status
        | AgentControlAction::SetHook
        | AgentControlAction::ApprovePermission
        | AgentControlAction::DenyPermission => Err(AgentOsError::Validation(format!(
            "invalid lifecycle action dispatch: {action:?}"
        ))),
    }
}

fn output_for_target(
    kernel: &Kernel,
    target: &AgentControlBlock,
    payload: &Value,
) -> AgentOsResult<AgentControlActionResult> {
    let state = kernel.read_state()?;
    if let Some(tool_call_id) = payload.get("tool_call_id").and_then(Value::as_str) {
        let invocation = state
            .tool_invocations
            .get(tool_call_id)
            .filter(|invocation| {
                invocation.agent_id == target.agent_id || invocation.task_id == target.task.task_id
            })
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("tool call {tool_call_id}")))?;
        let worker = kernel
            .tool_workers
            .lock()
            .ok()
            .and_then(|workers| workers.get(tool_call_id).cloned());
        return Ok(AgentControlActionResult {
            thread_status: target.status,
            output: super::super::super::output::query_tool_output(
                &invocation,
                worker.as_ref(),
                payload,
            )?,
        });
    }
    let cursor = payload
        .get("cursor")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0);
    let limit = payload
        .get("limit")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(20)
        .clamp(1, 100);
    let mut output: Vec<Value> = state
        .provider_stream_sessions
        .values()
        .filter(|session| session.request.thread_id == target.thread_id)
        .flat_map(|session| {
            session.stream_events.iter().map(|event| {
                json!({
                    "session_id": session.session_id,
                    "event_id": event.event_id,
                    "event_type": event.event_type,
                    "payload": event.payload,
                    "created_at": event.created_at,
                })
            })
        })
        .collect();
    output.sort_by(|left, right| {
        left["created_at"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["created_at"].as_str().unwrap_or_default())
    });
    let total = output.len();
    let items = output
        .into_iter()
        .skip(cursor)
        .take(limit)
        .collect::<Vec<_>>();
    let next_cursor = (cursor + items.len() < total).then_some(cursor + items.len());
    Ok(AgentControlActionResult {
        thread_status: target.status,
        output: json!({
            "items": items,
            "cursor": cursor,
            "limit": limit,
            "total_items": total,
            "next_cursor": next_cursor,
            "truncated": next_cursor.is_some()
        }),
    })
}

fn export_trace_for_target(
    kernel: &Kernel,
    target: &AgentControlBlock,
    payload: &Value,
) -> AgentOsResult<AgentControlActionResult> {
    let events = kernel
        .events()?
        .into_iter()
        .filter(|event| {
            event.aggregate_id == target.thread_id
                || event.agent_id.as_deref() == Some(&target.agent_id)
                || event.task_id.as_deref() == Some(&target.task.task_id)
        })
        .collect::<Vec<_>>();
    let event_count = events.len();
    let preview_event_limit = payload
        .get("limit")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(5)
        .clamp(1, 100);
    let preview_events = events
        .iter()
        .take(preview_event_limit)
        .map(|event| {
            json!({
                "event_id": event.event_id,
                "event_type": event.event_type,
                "aggregate_type": event.aggregate_type,
                "aggregate_id": event.aggregate_id,
                "agent_id": event.agent_id,
                "task_id": event.task_id,
                "created_at": event.created_at,
            })
        })
        .collect::<Vec<_>>();
    let event_types = events
        .iter()
        .map(|event| event.event_type.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    Ok(AgentControlActionResult {
        thread_status: target.status,
        output: json!({
            "event_count": event_count,
            "event_types": event_types,
            "first_event_id": events.first().map(|event| event.event_id.as_str()),
            "last_event_id": events.last().map(|event| event.event_id.as_str()),
            "preview_event_limit": preview_event_limit,
            "preview_events": preview_events,
            "events_omitted": event_count > preview_event_limit,
        }),
    })
}

fn set_timeout_budget(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    target: &AgentControlBlock,
    payload: &Value,
) -> AgentOsResult<AgentControlBlock> {
    let timeout_ms = payload
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .or_else(|| {
            payload
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .map(|seconds| seconds.saturating_mul(1000))
        })
        .ok_or_else(|| {
            AgentOsError::Validation(
                "set_timeout requires timeout_ms or timeout_seconds".to_string(),
            )
        })?;
    let mut acb = target.clone();
    acb.budgets.wall_time_budget_ms = Some(timeout_ms);
    acb.audit.updated_at = now_rfc3339();
    kernel.emit(
        "ThreadConfigured",
        "thread",
        &acb.thread_id,
        Some(acb.agent_id.clone()),
        Some(acb.task.task_id.clone()),
        Some(syscall.syscall_id.clone()),
        Some(acb.task.goal_id.clone()),
        &acb,
    )?;
    Ok(acb)
}

fn resume_target(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    target: &AgentControlBlock,
) -> AgentOsResult<AgentControlBlock> {
    if target.status == ThreadStatus::Unloaded || target.session_id.is_empty() {
        let mut acb = target.clone();
        acb.session_id = new_id("sess_");
        acb.active_turn = ActiveTurn::default();
        acb.status_reason = None;
        acb.audit.updated_at = now_rfc3339();
        kernel.emit(
            "ThreadConfigured",
            "thread",
            &acb.thread_id,
            Some(acb.agent_id.clone()),
            Some(acb.task.task_id.clone()),
            Some(syscall.syscall_id.clone()),
            Some(acb.task.goal_id.clone()),
            &acb,
        )?;
    }
    kernel.transition_thread_with_cause(
        &target.thread_id,
        ThreadStatus::Ready,
        Some("agent_control resume".to_string()),
        Some(syscall.syscall_id.clone()),
    )
}

fn delete_session_for_target(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    target: &AgentControlBlock,
) -> AgentOsResult<(AgentControlBlock, String)> {
    let previous_session_id = target.session_id.clone();
    let mut acb = target.clone();
    acb.session_id = String::new();
    acb.status = ThreadStatus::Unloaded;
    acb.status_reason = Some("agent_control delete_session".to_string());
    acb.active_turn = ActiveTurn::default();
    acb.resources = ThreadResources::default();
    acb.recovery.dirty = true;
    acb.audit.updated_at = now_rfc3339();
    kernel.emit(
        "ThreadConfigured",
        "thread",
        &acb.thread_id,
        Some(acb.agent_id.clone()),
        Some(acb.task.task_id.clone()),
        Some(syscall.syscall_id.clone()),
        Some(acb.task.goal_id.clone()),
        &acb,
    )?;
    Ok((acb, previous_session_id))
}

fn purge_state_for_target(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    target: &AgentControlBlock,
) -> AgentOsResult<AgentControlBlock> {
    kernel.close_active_hooks_for_thread_with_cause(
        &target.thread_id,
        AgentHookStatus::Cancelled,
        Some(syscall.syscall_id.clone()),
    )?;
    kernel.close_invocation_for_thread_with_cause(
        &target.thread_id,
        AgentInvocationStatus::Cancelled,
        Some(syscall.syscall_id.clone()),
    )?;
    let mut acb = target.clone();
    acb.status = ThreadStatus::Terminated;
    acb.status_reason = Some("agent_control purge_state".to_string());
    acb.active_turn = ActiveTurn::default();
    acb.resources = ThreadResources::default();
    acb.task.goal_status = AgentGoalStatus::Cancelled;
    acb.audit.updated_at = now_rfc3339();
    acb.audit.termination_reason = Some("agent_control purge_state".to_string());
    acb.recovery.dirty = true;
    kernel.emit(
        "AgentStatePurged",
        "thread",
        &acb.thread_id,
        Some(acb.agent_id.clone()),
        Some(acb.task.task_id.clone()),
        Some(syscall.syscall_id.clone()),
        Some(acb.task.goal_id.clone()),
        &acb,
    )?;
    Ok(acb)
}

/// Terminate a target thread, allowing an interrupt detour when the target is
/// currently running and cannot move directly to the terminal status.
fn terminate_target(
    kernel: &Kernel,
    target: &AgentControlBlock,
    terminal_status: ThreadStatus,
    reason: &str,
) -> AgentOsResult<AgentControlBlock> {
    match kernel.transition_thread(&target.thread_id, terminal_status, Some(reason.to_string())) {
        Ok(acb) => Ok(acb),
        Err(AgentOsError::InvalidTransition(_)) if target.status == ThreadStatus::Running => {
            kernel.transition_thread(
                &target.thread_id,
                ThreadStatus::Interrupted,
                Some(reason.to_string()),
            )?;
            kernel.transition_thread(&target.thread_id, terminal_status, Some(reason.to_string()))
        }
        Err(error) => Err(error),
    }
}
