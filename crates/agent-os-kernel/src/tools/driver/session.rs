use super::{current_agent, string_array};
use crate::util::{parse_payload, required_string};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(in crate::tools) fn run_submit_final(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let acb = current_agent(kernel, syscall)?;
    let evidence_map = input
        .get("evidence_map")
        .map(parse_payload::<Vec<EvidenceMapEntry>>)
        .transpose()?
        .ok_or_else(|| {
            AgentOsError::Validation("submit_final requires evidence_map".to_string())
        })?;
    let submission = FinalSubmission {
        summary: required_string(input, "summary")?,
        changed_artifacts: string_array(input, "changed_artifacts")?,
        evidence_map,
        unverified_claims: string_array(input, "unverified_claims")?,
        known_risks: string_array(input, "known_risks")?,
        tests_run: string_array(input, "tests_run")?,
        tests_not_run: string_array(input, "tests_not_run")?,
        approvals: string_array(input, "approvals")?,
    };
    kernel.submit_final_with_cause(
        &syscall.agent_id,
        &syscall.task_id,
        submission.clone(),
        Some(syscall.syscall_id.clone()),
    )?;
    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "ok",
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "task_id": acb.task.task_id,
        "final_submitted": true,
        "summary": submission.summary,
        "evidence_map_entries": submission.evidence_map.len(),
    }))
}
