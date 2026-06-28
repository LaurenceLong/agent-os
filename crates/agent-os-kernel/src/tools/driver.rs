mod agent_control;
mod communication;
mod session;
mod work_state;
mod workspace;

use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(super) fn run_tool_driver(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    match descriptor.name.as_str() {
        "write_file" => workspace::run_workspace_write_file(kernel, syscall, descriptor, input),
        "read_file" => workspace::run_workspace_read_file(kernel, syscall, descriptor, input),
        "delete_file" => workspace::run_workspace_delete_file(kernel, syscall, descriptor, input),
        "replace_text" => workspace::run_workspace_replace_text(kernel, syscall, descriptor, input),
        "run_command" => workspace::run_process(kernel, syscall, descriptor, input),
        "set_objective" => work_state::run_set_objective(kernel, syscall, descriptor, input),
        "update_checklist" => work_state::run_update_checklist(kernel, syscall, descriptor, input),
        "record_evidence" => work_state::run_record_evidence(kernel, syscall, descriptor, input),
        "report_supervisor" => {
            communication::run_report_supervisor(kernel, syscall, descriptor, input)
        }
        "post_blackboard" => communication::run_post_blackboard(kernel, syscall, descriptor, input),
        "ask_human" => communication::run_ask_human(kernel, syscall, descriptor, input),
        "agent_control" => agent_control::run_agent_control(kernel, syscall, descriptor, input),
        "submit_final" => session::run_submit_final(kernel, syscall, descriptor, input),
        _ => Ok(json!({
            "tool": descriptor.name.clone(),
            "status": "ok",
            "input": input.clone(),
            "driver_class": descriptor.driver_class,
        })),
    }
}

fn string_array(value: &Value, field: &str) -> AgentOsResult<Vec<String>> {
    let Some(items) = value.get(field) else {
        return Ok(Vec::new());
    };
    items
        .as_array()
        .ok_or_else(|| AgentOsError::Validation(format!("{field} must be an array")))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| AgentOsError::Validation(format!("{field} entries must be strings")))
        })
        .collect()
}

fn optional_string(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_string)
}

fn current_agent(kernel: &Kernel, syscall: &SyscallEnvelope) -> AgentOsResult<AgentControlBlock> {
    kernel
        .thread_by_agent(&syscall.agent_id)?
        .ok_or_else(|| AgentOsError::NotFound(format!("agent {}", syscall.agent_id)))
}

fn current_task_id(input: &Value, syscall: &SyscallEnvelope) -> AgentOsResult<String> {
    let task_id = optional_string(input, "task_id").unwrap_or_else(|| syscall.task_id.clone());
    if task_id != syscall.task_id {
        return Err(AgentOsError::PermissionDenied(
            "work-state tools can only update the current task".to_string(),
        ));
    }
    Ok(task_id)
}

struct ControlMessageRequest {
    route: MessageRoute,
    message_type: String,
    payload: Value,
    channel_id: Option<String>,
    artifact_refs: Vec<String>,
    evidence_refs: Vec<String>,
}

fn send_control_message(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    acb: &AgentControlBlock,
    request: ControlMessageRequest,
) -> AgentOsResult<AgentMessage> {
    let (target_agent_id, target_thread_id) = supervisor_target(kernel, acb)?;
    let message = kernel.send_message_with_cause(
        SendMessageInput {
            message_type: request.message_type,
            route: request.route,
            source_agent_id: syscall.agent_id.clone(),
            source_thread_id: acb.thread_id.clone(),
            target_agent_id,
            target_thread_id,
            channel_id: request.channel_id,
            goal_id: acb.task.goal_id.clone(),
            task_id: syscall.task_id.clone(),
            risk_level: syscall.risk_level,
            payload: request.payload,
            artifact_refs: request.artifact_refs,
            evidence_refs: request.evidence_refs,
        },
        Some(syscall.syscall_id.clone()),
    )?;
    if message.delivery_status != MessageDeliveryStatus::Delivered {
        return Err(AgentOsError::PermissionDenied(
            message
                .rejected_reason
                .clone()
                .unwrap_or_else(|| "message route was rejected".to_string()),
        ));
    }
    Ok(message)
}

fn supervisor_target(
    kernel: &Kernel,
    acb: &AgentControlBlock,
) -> AgentOsResult<(Option<String>, Option<String>)> {
    let Some(parent_thread_id) = &acb.parent_thread_id else {
        return Ok((None, None));
    };
    let state = kernel.read_state()?;
    let parent = state
        .threads
        .get(parent_thread_id)
        .ok_or_else(|| AgentOsError::NotFound(format!("thread {parent_thread_id}")))?;
    Ok((
        Some(parent.agent_id.clone()),
        Some(parent.thread_id.clone()),
    ))
}

fn message_output(
    descriptor: &ToolDescriptor,
    input: &Value,
    message: &AgentMessage,
) -> AgentOsResult<Value> {
    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "ok",
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "message_id": message.message_id,
        "delivery_status": message.delivery_status,
        "requires_review": message.requires_review,
        "trigger_turn": message.trigger_turn,
    }))
}
