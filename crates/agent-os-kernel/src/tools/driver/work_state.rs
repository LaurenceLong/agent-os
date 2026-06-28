use super::{current_agent, current_task_id, optional_string};
use crate::util::{parse_payload, required_string};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(super) fn run_set_objective(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let objective = required_string(input, "objective")?;
    let task_id = current_task_id(input, syscall)?;
    let task = kernel.update_task_with_cause(
        UpdateTaskInput {
            task_id: task_id.clone(),
            status: None,
            blocked_reason: None,
            owner_agent_id: None,
            title: optional_string(input, "title"),
            description: Some(objective.clone()),
            checklist: None,
        },
        Some(syscall.syscall_id.clone()),
    )?;
    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "ok",
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "task_id": task.task_id,
        "objective": task.description,
    }))
}

pub(super) fn run_update_checklist(
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

pub(super) fn run_record_evidence(
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
    let inline_bytes = optional_string(input, "inline_content").map(String::into_bytes);
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
