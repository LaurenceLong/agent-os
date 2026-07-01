use super::{current_agent, current_task_id, optional_string, string_array};
use crate::util::{parse_payload, required_string};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(in crate::tools) fn run_set_goal(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let goal = required_string(input, "goal")?;
    let acb = kernel.set_agent_goal_with_cause(
        &syscall.agent_id,
        optional_string(input, "target_thread_id"),
        optional_string(input, "target_agent_id"),
        goal,
        optional_string(input, "title"),
        optional_string_array(input, "success_criteria")?,
        optional_string_array(input, "failure_criteria")?,
        Some(syscall.syscall_id.clone()),
    )?;
    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "ok",
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "thread_id": acb.thread_id,
        "agent_id": acb.agent_id,
        "task_id": acb.task.task_id,
        "goal": acb.task.goal,
        "goal_status": acb.task.goal_status,
        "goal_revision": acb.task.goal_revision,
    }))
}

pub(in crate::tools) fn run_accomplish_goal(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let summary = required_string(input, "summary")?;
    let completion = kernel.accomplish_agent_goal_with_cause(
        &syscall.agent_id,
        summary.clone(),
        string_array(input, "evidence_refs")?,
        string_array(input, "artifact_refs")?,
        string_array(input, "known_risks")?,
        Some(syscall.syscall_id.clone()),
    )?;
    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "ok",
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "thread_id": completion.thread.thread_id,
        "agent_id": completion.thread.agent_id,
        "task_id": completion.thread.task.task_id,
        "goal": completion.thread.task.goal,
        "goal_status": completion.thread.task.goal_status,
        "goal_accomplished": true,
        "summary": summary,
        "hooks_completed": completion.hooks_completed,
    }))
}

pub(in crate::tools) fn run_update_checklist(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let task_id = current_task_id(input, syscall)?;
    let items = input
        .get("items")
        .ok_or_else(|| AgentOsError::Validation("missing required field items".to_string()))?;
    let checklist: Vec<ChecklistItem> = parse_payload(items)?;
    let task = kernel.update_task_with_cause(
        UpdateTaskInput {
            task_id: task_id.clone(),
            status: None,
            blocked_reason: None,
            owner_agent_id: None,
            title: None,
            description: None,
            checklist: Some(checklist),
        },
        Some(syscall.syscall_id.clone()),
    )?;
    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "ok",
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "task_id": task.task_id,
        "items": task.checklist,
    }))
}

pub(in crate::tools) fn run_record_evidence(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let acb = current_agent(kernel, syscall)?;
    let evidence_type: EvidenceType =
        parse_payload(input.get("evidence_type").ok_or_else(|| {
            AgentOsError::Validation("missing required field evidence_type".to_string())
        })?)?;
    let claim = required_string(input, "claim")?;
    let inline_bytes = optional_string(input, "inline_content")
        .map(|content| {
            if content.len() > 8_000 {
                return Err(AgentOsError::Validation(
                    "record_evidence inline_content must be 8000 bytes or less; use blob_ref or content_hash for large evidence".to_string(),
                ));
            }
            Ok(content.into_bytes())
        })
        .transpose()?;
    let evidence = kernel.attach_evidence_with_cause(
        AttachEvidenceInput {
            goal_id: acb.task.goal_id,
            task_id: Some(current_task_id(input, syscall)?),
            artifact_id: optional_string(input, "artifact_id"),
            evidence_type,
            producer_agent_id: Some(syscall.agent_id.clone()),
            claim: Some(claim.clone()),
            blob_ref: optional_string(input, "blob_ref"),
            content_hash: optional_string(input, "content_hash"),
            inline_bytes,
            metadata: input.get("metadata").cloned().unwrap_or_else(|| json!({})),
        },
        Some(syscall.syscall_id.clone()),
    )?;
    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "ok",
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "evidence_id": evidence.evidence_id,
        "evidence_type": evidence.evidence_type,
        "claim": claim,
    }))
}

fn optional_string_array(value: &Value, field: &str) -> AgentOsResult<Option<Vec<String>>> {
    if value.get(field).is_none() {
        return Ok(None);
    }
    string_array(value, field).map(Some)
}
