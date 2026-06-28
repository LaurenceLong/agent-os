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
    let action = parse_agent_control_action(&action_text)?;
    require_agent_control_action_risk(action, syscall.risk_level)?;
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
            let workspace_roots = agent_control_workspace_roots(
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
            let hooks = configure_start_hooks(kernel, syscall, &child, &payload)?;
            record_agent_control_command(
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
            let target = resolve_agent_control_target(kernel, input, &payload)?;
            let hooks = agent_hooks_for(kernel, &target.agent_id)?;
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
            let target = resolve_agent_control_target(kernel, input, &payload)?;
            let hook = configure_agent_hook(kernel, syscall, &target, &payload)?;
            record_agent_control_command(
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
            let target = resolve_agent_control_target(kernel, input, &payload)?;
            let action_result = apply_lifecycle_action(kernel, syscall, action, &target, &payload)?;
            let command = record_agent_control_command(
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
            let target = resolve_agent_control_target(kernel, input, &payload)?;
            record_agent_control_command(
                kernel,
                syscall,
                &requester,
                Some(&target),
                action,
                payload,
                AgentControlCommandStatus::Rejected,
            )?;
            Err(AgentOsError::Unsupported(format!(
                "agent_control action {action_text} is not available in the append-only v0.1 store"
            )))
        }
    }
}

struct AgentControlActionResult {
    thread_status: ThreadStatus,
    output: Value,
}

fn apply_lifecycle_action(
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
        | AgentControlAction::PurgeState => Err(AgentOsError::Unsupported(
            "unsupported lifecycle helper action".to_string(),
        )),
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
    let events: Vec<Value> = kernel
        .events()?
        .into_iter()
        .filter(|event| {
            event.aggregate_id == target.thread_id
                || event.agent_id.as_deref() == Some(&target.agent_id)
                || event.task_id.as_deref() == Some(&target.task.task_id)
        })
        .map(|event| serde_json::to_value(event).map_err(AgentOsError::from))
        .collect::<AgentOsResult<Vec<_>>>()?;
    Ok(AgentControlActionResult {
        thread_status: target.status,
        output: json!({
            "events": events,
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

fn require_agent_control_action_risk(
    action: AgentControlAction,
    risk_level: u8,
) -> AgentOsResult<()> {
    let required = match action {
        AgentControlAction::Kill
        | AgentControlAction::DeleteSession
        | AgentControlAction::PurgeState => 6,
        AgentControlAction::Start
        | AgentControlAction::SetHook
        | AgentControlAction::Send
        | AgentControlAction::Resume
        | AgentControlAction::Stop
        | AgentControlAction::SetTimeout => 4,
        AgentControlAction::Status
        | AgentControlAction::Output
        | AgentControlAction::ExportTrace => 1,
    };
    if risk_level < required {
        return Err(AgentOsError::PermissionDenied(format!(
            "agent_control action requires risk level {required}"
        )));
    }
    Ok(())
}

fn parse_agent_control_action(value: &str) -> AgentOsResult<AgentControlAction> {
    match value {
        "start" => Ok(AgentControlAction::Start),
        "status" => Ok(AgentControlAction::Status),
        "output" => Ok(AgentControlAction::Output),
        "set_hook" => Ok(AgentControlAction::SetHook),
        "send" => Ok(AgentControlAction::Send),
        "resume" => Ok(AgentControlAction::Resume),
        "stop" => Ok(AgentControlAction::Stop),
        "set_timeout" => Ok(AgentControlAction::SetTimeout),
        "export_trace" => Ok(AgentControlAction::ExportTrace),
        "kill" => Ok(AgentControlAction::Kill),
        "delete_session" => Ok(AgentControlAction::DeleteSession),
        "purge_state" => Ok(AgentControlAction::PurgeState),
        _ => Err(AgentOsError::Validation(format!(
            "unknown agent_control action {value}"
        ))),
    }
}

fn resolve_agent_control_target(
    kernel: &Kernel,
    input: &Value,
    payload: &Value,
) -> AgentOsResult<AgentControlBlock> {
    let agent_id = input
        .get("agent_id")
        .or_else(|| payload.get("agent_id"))
        .and_then(Value::as_str);
    let thread_id = input
        .get("thread_id")
        .or_else(|| payload.get("thread_id"))
        .and_then(Value::as_str);
    let state = kernel.read_state()?;
    if let Some(agent_id) = agent_id {
        return state
            .threads
            .values()
            .find(|thread| thread.agent_id == agent_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("agent {agent_id}")));
    }
    if let Some(thread_id) = thread_id {
        return state
            .threads
            .get(thread_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {thread_id}")));
    }
    Err(AgentOsError::Validation(
        "agent_control action requires agent_id or thread_id".to_string(),
    ))
}

fn configure_start_hooks(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    target: &AgentControlBlock,
    payload: &Value,
) -> AgentOsResult<Vec<AgentHook>> {
    let Some(hooks) = payload.get("hooks").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    hooks
        .iter()
        .map(|hook_payload| configure_agent_hook(kernel, syscall, target, hook_payload))
        .collect()
}

fn configure_agent_hook(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    target: &AgentControlBlock,
    payload: &Value,
) -> AgentOsResult<AgentHook> {
    let now = now_rfc3339();
    let hook = AgentHook {
        hook_id: new_id("hook_"),
        agent_id: target.agent_id.clone(),
        thread_id: target.thread_id.clone(),
        hook_type: payload
            .get("hook_type")
            .and_then(Value::as_str)
            .unwrap_or("progress_report")
            .to_string(),
        interval_seconds: payload
            .get("interval_seconds")
            .and_then(Value::as_u64)
            .unwrap_or(120),
        prompt: required_string(payload, "prompt")?,
        response_route: MessageRoute::Supervisor,
        max_response_chars: payload
            .get("max_response_chars")
            .and_then(Value::as_u64)
            .unwrap_or(200),
        stop_when: payload
            .get("stop_when")
            .and_then(Value::as_str)
            .unwrap_or("terminal")
            .to_string(),
        on_missed_reports: payload
            .get("on_missed_reports")
            .and_then(Value::as_str)
            .unwrap_or("report")
            .to_string(),
        status: AgentHookStatus::Active,
        created_at: now.clone(),
        updated_at: now,
    };
    kernel.emit(
        "AgentHookConfigured",
        "agent_hook",
        &hook.hook_id,
        Some(syscall.agent_id.clone()),
        Some(syscall.task_id.clone()),
        Some(syscall.syscall_id.clone()),
        Some(target.task.goal_id.clone()),
        &hook,
    )?;
    Ok(hook)
}

fn record_agent_control_command(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    requester: &AgentControlBlock,
    target: Option<&AgentControlBlock>,
    action: AgentControlAction,
    payload: Value,
    status: AgentControlCommandStatus,
) -> AgentOsResult<AgentControlCommand> {
    let command = AgentControlCommand {
        command_id: new_id("actl_"),
        action,
        requested_by_agent_id: requester.agent_id.clone(),
        requested_by_thread_id: requester.thread_id.clone(),
        target_agent_id: target.map(|thread| thread.agent_id.clone()),
        target_thread_id: target.map(|thread| thread.thread_id.clone()),
        task_id: syscall.task_id.clone(),
        goal_id: requester.task.goal_id.clone(),
        payload,
        status,
        created_at: now_rfc3339(),
    };
    kernel.emit(
        "AgentControlCommandRecorded",
        "agent_control_command",
        &command.command_id,
        Some(syscall.agent_id.clone()),
        Some(syscall.task_id.clone()),
        Some(syscall.syscall_id.clone()),
        Some(requester.task.goal_id.clone()),
        &command,
    )?;
    Ok(command)
}

fn agent_hooks_for(kernel: &Kernel, agent_id: &str) -> AgentOsResult<Vec<AgentHook>> {
    Ok(kernel
        .read_state()?
        .agent_hooks
        .values()
        .filter(|hook| hook.agent_id == agent_id)
        .cloned()
        .collect())
}

fn agent_control_workspace_roots(
    payload: &Value,
    fallback: &[String],
) -> AgentOsResult<Vec<String>> {
    if let Some(roots) = payload.get("workspace_roots") {
        return roots
            .as_array()
            .ok_or_else(|| {
                AgentOsError::Validation("workspace_roots must be an array".to_string())
            })?
            .iter()
            .map(|root| {
                root.as_str().map(str::to_string).ok_or_else(|| {
                    AgentOsError::Validation("workspace_roots entries must be strings".to_string())
                })
            })
            .collect();
    }
    if let Some(workdir) = payload.get("workdir").and_then(Value::as_str) {
        return Ok(vec![workdir.to_string()]);
    }
    Ok(fallback.to_vec())
}
