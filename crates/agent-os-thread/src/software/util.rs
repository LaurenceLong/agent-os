use super::types::{ReviewRecord, SoftwareCodeTask};
use crate::{RuntimeRunReport, ToolExecutionRecord};
use agent_os_kernel::Kernel;
use agent_os_sys::*;

pub(super) fn latest_artifact_id(reports: &[RuntimeRunReport]) -> AgentOsResult<String> {
    reports
        .iter()
        .rev()
        .find_map(|report| report.artifacts.last())
        .map(|artifact| artifact.artifact_id.clone())
        .ok_or_else(|| AgentOsError::Validation("pipeline produced no patch artifact".to_string()))
}

pub(super) fn artifact_ids(reports: &[RuntimeRunReport]) -> Vec<String> {
    reports
        .iter()
        .flat_map(|report| report.artifacts.iter())
        .map(|artifact| artifact.artifact_id.clone())
        .collect()
}

pub(super) fn collect_evidence_ids<'a>(
    reports: impl Iterator<Item = &'a RuntimeRunReport>,
    review_records: &[ReviewRecord],
    extra_evidence_ids: &[String],
) -> Vec<String> {
    let mut ids: Vec<String> = reports
        .flat_map(|report| report.tool_results.iter())
        .flat_map(|result| result.evidence_ids.clone())
        .collect();
    ids.extend(
        review_records
            .iter()
            .map(|record| record.evidence_id.clone()),
    );
    ids.extend(extra_evidence_ids.iter().cloned());
    ids.sort();
    ids.dedup();
    ids
}

pub(super) fn evidence_by_type(
    kernel: &Kernel,
    evidence_ids: &[String],
    evidence_type: EvidenceType,
) -> AgentOsResult<Vec<String>> {
    let state = kernel.state_snapshot()?;
    let ids: Vec<String> = evidence_ids
        .iter()
        .filter(|id| {
            state
                .evidence
                .get(*id)
                .is_some_and(|evidence| evidence.evidence_type == evidence_type)
        })
        .cloned()
        .collect();
    if ids.is_empty() {
        return Err(AgentOsError::Validation(format!(
            "missing evidence type {:?}",
            evidence_type
        )));
    }
    Ok(ids)
}

pub(super) fn process_exit_code(results: &[ToolExecutionRecord]) -> AgentOsResult<i64> {
    results
        .iter()
        .find(|result| result.tool_name == "run_command")
        .and_then(|result| result.output.as_ref())
        .and_then(|output| output.get("exit_code"))
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| AgentOsError::Validation("run_command output omitted exit_code".to_string()))
}

pub(super) fn test_command(spec: &SoftwareCodeTask) -> String {
    format!(
        "{} {}",
        spec.test_program.to_string_lossy(),
        spec.test_args.join(" ")
    )
}
