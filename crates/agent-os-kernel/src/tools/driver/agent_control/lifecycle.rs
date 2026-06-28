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
        AgentControlAction::Output => output_for_target(kernel, target),
        AgentControlAction::Send => Ok(AgentControlActionResult {
            thread_status: target.status,
            output: json!({
                "sent": true,
                "payload": payload,
            }),
        }),
        AgentControlAction::Resume => {
            let acb = kernel.transition_thread(
                &target.thread_id,
                ThreadStatus::Ready,
                Some("agent_control resume".to_string()),
            )?;
            Ok(AgentControlActionResult {
                thread_status: acb.status,
                output: json!({"resumed": true}),
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
        AgentControlAction::ExportTrace => export_trace_for_target(kernel, target),
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
        AgentControlAction::Start
        | AgentControlAction::Status
        | AgentControlAction::SetHook
        | AgentControlAction::DeleteSession
        | AgentControlAction::PurgeState => Err(AgentOsError::Validation(format!(
            "invalid lifecycle action dispatch: {action:?}"
        ))),
    }
}

fn output_for_target(
    kernel: &Kernel,
    target: &AgentControlBlock,
) -> AgentOsResult<AgentControlActionResult> {
    let state = kernel.read_state()?;
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
    Ok(AgentControlActionResult {
        thread_status: target.status,
        output: json!(output),
    })
}

fn export_trace_for_target(
    kernel: &Kernel,
    target: &AgentControlBlock,
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
    let preview_event_limit = 5usize;
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
