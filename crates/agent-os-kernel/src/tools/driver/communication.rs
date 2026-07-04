use super::{
    current_agent, message_output, optional_string, send_control_message, string_array,
    ControlMessageRequest,
};
use crate::util::{parse_payload, required_string};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(in crate::tools) fn run_report_supervisor(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let acb = current_agent(kernel, syscall)?;
    let message = send_control_message(
        kernel,
        syscall,
        &acb,
        ControlMessageRequest {
            route: MessageRoute::Supervisor,
            message_type: optional_string(input, "message_type")
                .unwrap_or_else(|| "StatusUpdate".to_string()),
            payload: json!({"message": bounded_required_string(input, "message")?}),
            channel_id: None,
            artifact_refs: string_array(input, "artifact_refs")?,
            evidence_refs: string_array(input, "evidence_refs")?,
        },
    )?;
    message_output(descriptor, input, &message)
}

pub(in crate::tools) fn run_post_blackboard(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let acb = current_agent(kernel, syscall)?;
    let scope: CommunicationScope = input
        .get("scope")
        .map(parse_payload)
        .transpose()?
        .unwrap_or(CommunicationScope::Task);
    let section: BlackboardSection =
        parse_payload(input.get("section").ok_or_else(|| {
            AgentOsError::Validation("missing required field section".to_string())
        })?)?;
    let channel_id = required_string(input, "channel_id")?;
    let content = input
        .get("content")
        .cloned()
        .ok_or_else(|| AgentOsError::Validation("missing required field content".to_string()))?;
    let source_evidence_ids = string_array(input, "source_evidence_ids")?;
    let entry = kernel.post_blackboard_with_cause(
        PostBlackboardInput {
            source_agent_id: syscall.agent_id.clone(),
            source_thread_id: acb.thread_id.clone(),
            channel_id: Some(channel_id.clone()),
            goal_id: acb.task.goal_id.clone(),
            task_id: (scope == CommunicationScope::Task).then_some(syscall.task_id.clone()),
            scope,
            section,
            content,
            confidence: input.get("confidence").and_then(Value::as_f64),
            source_evidence_ids,
        },
        Some(syscall.syscall_id.clone()),
    )?;
    let message = send_control_message(
        kernel,
        syscall,
        &acb,
        ControlMessageRequest {
            route: MessageRoute::Blackboard,
            message_type: "BlackboardPost".to_string(),
            payload: json!({
                "scope": scope,
                "entry_type": crate::blackboard::blackboard_section_key(entry.section),
                "content": entry.content.clone(),
            }),
            channel_id: Some(channel_id),
            artifact_refs: Vec::new(),
            evidence_refs: entry.source_evidence_ids.clone(),
        },
    )?;
    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "ok",
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "entry_id": entry.entry_id,
        "section": crate::blackboard::blackboard_section_key(entry.section),
        "message_id": message.message_id,
        "delivery_status": message.delivery_status,
    }))
}

pub(in crate::tools) fn run_ask_human(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let acb = current_agent(kernel, syscall)?;
    let payload = json!({
        "question": bounded_required_string(input, "question")?,
        "context": input.get("context").cloned().unwrap_or_else(|| json!({})),
    });
    let message = send_control_message(
        kernel,
        syscall,
        &acb,
        ControlMessageRequest {
            route: MessageRoute::Human,
            message_type: optional_string(input, "message_type")
                .unwrap_or_else(|| "HumanQuestion".to_string()),
            payload,
            channel_id: None,
            artifact_refs: string_array(input, "artifact_refs")?,
            evidence_refs: string_array(input, "evidence_refs")?,
        },
    )?;
    message_output(descriptor, input, &message)
}

fn bounded_required_string(input: &Value, field: &str) -> AgentOsResult<String> {
    let value = required_string(input, field)?;
    if value.len() > 8_000 {
        return Err(AgentOsError::Validation(format!(
            "{field} must be 8000 bytes or less"
        )));
    }
    Ok(value)
}
