use super::types::SoftwareEngineeringPipeline;
use crate::{ArtifactRecord, RuntimeRunReport, ToolExecutionRecord};
use agent_os_kernel::{CommitArtifactInput, CompleteTaskInput, ToolInvokeInput, UpdateTaskInput};
use agent_os_sys::*;
use serde_json::json;
use std::path::Path;

impl SoftwareEngineeringPipeline {
    pub(super) fn start_tool_role(
        &self,
        agent: &AgentControlBlock,
        workspace_root: &Path,
        attach_mode: AttachMode,
        tool_risk_ceiling: u8,
    ) -> AgentOsResult<ToolRoleSession> {
        self.kernel.update_task(UpdateTaskInput {
            task_id: agent.task.task_id.clone(),
            status: Some(TaskStatus::Running),
            blocked_reason: None,
            owner_agent_id: Some(agent.agent_id.clone()),
            title: None,
            description: None,
            checklist: None,
        })?;
        let acb = self.kernel.start_turn(&agent.thread_id)?;
        let env = self.kernel.create_environment(
            BackendType::IsolatedWorktree,
            workspace_root.to_string_lossy(),
            acb.config_snapshot.sandbox_profile_id.clone(),
            ReusePolicy::TaskScoped,
        )?;
        let environment_lease = self.kernel.attach_environment(
            &env.environment_id,
            &acb.agent_id,
            &acb.thread_id,
            &acb.task.task_id,
            attach_mode,
        )?;
        let capability = self.kernel.grant_capability(
            &acb.agent_id,
            &acb.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            tool_risk_ceiling,
            None,
        )?;
        Ok(ToolRoleSession {
            acb,
            environment_id: env.environment_id,
            environment_lease_id: environment_lease.environment_lease_id,
            capability_id: capability.capability_id,
        })
    }

    pub(super) fn invoke_planned_tool(
        &self,
        session: &ToolRoleSession,
        tool_name: &str,
        input: serde_json::Value,
        risk_level: u8,
        evidence_claim: &str,
    ) -> AgentOsResult<ToolExecutionRecord> {
        let tool_input = ToolInvokeInput {
            tool_name: tool_name.to_string(),
            input,
            evidence_claim: Some(evidence_claim.to_string()),
        };
        self.kernel.record_tool_proposal(
            &session.acb.agent_id,
            &session.acb.task.task_id,
            tool_input.clone(),
            risk_level,
        )?;
        let invocation = self.kernel.invoke_tool(
            &session.acb.agent_id,
            &session.acb.task.task_id,
            &session.acb.session_id,
            session.capability_id.clone(),
            risk_level,
            tool_input.clone(),
        )?;
        Ok(ToolExecutionRecord {
            call_id: invocation.call_id,
            tool_name: invocation.tool_name,
            status: invocation.status,
            input: Some(invocation.input),
            output: invocation.output,
            evidence_ids: invocation.evidence_ids,
            evidence_claim: tool_input.evidence_claim,
        })
    }

    pub(super) fn commit_patch_artifact_for_tool(
        &self,
        session: &ToolRoleSession,
        record: &ToolExecutionRecord,
    ) -> AgentOsResult<ArtifactRecord> {
        if record.status != ToolCallStatus::Completed {
            return Err(AgentOsError::Validation(format!(
                "tool {} did not complete",
                record.tool_name
            )));
        }
        if record.evidence_ids.is_empty() {
            return Err(AgentOsError::Validation(format!(
                "tool {} did not produce evidence",
                record.tool_name
            )));
        }
        let output = record.output.as_ref().ok_or_else(|| {
            AgentOsError::Validation(format!("tool {} omitted output", record.tool_name))
        })?;
        let path_key = match record.tool_name.as_str() {
            "write_file" => "written_path",
            "replace_text" => "changed_path",
            "delete_file" => "deleted_path",
            other => {
                return Err(AgentOsError::Validation(format!(
                    "tool {other} does not produce a patch artifact"
                )));
            }
        };
        let path = output
            .get(path_key)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                AgentOsError::Validation(format!("tool {} omitted {}", record.tool_name, path_key))
            })?;
        let artifact = self.kernel.commit_artifact(CommitArtifactInput {
            goal_id: session.acb.task.goal_id.clone(),
            task_id: session.acb.task.task_id.clone(),
            owner_agent_id: session.acb.agent_id.clone(),
            artifact_type: ArtifactType::Patch,
            blob_ref: Some(path.to_string()),
            content_hash: None,
            inline_bytes: None,
            metadata: json!({
                "tool_call_id": record.call_id,
                "tool_name": record.tool_name,
                "environment_id": session.environment_id,
                "environment_lease_id": session.environment_lease_id,
                "output": output,
            }),
            evidence_ids: record.evidence_ids.clone(),
            supersedes: None,
        })?;
        Ok(ArtifactRecord {
            artifact_id: artifact.artifact_id,
            artifact_type: artifact.artifact_type,
            blob_ref: artifact.blob_ref,
            evidence_ids: record.evidence_ids.clone(),
        })
    }

    pub(super) fn complete_tool_role(
        &self,
        session: &ToolRoleSession,
        artifacts: Vec<ArtifactRecord>,
        tool_results: Vec<ToolExecutionRecord>,
        final_payload: ToolRoleFinal,
    ) -> AgentOsResult<RuntimeRunReport> {
        let mut evidence_map = Vec::new();
        for result in &tool_results {
            if result.evidence_ids.is_empty() {
                return Err(AgentOsError::Validation(format!(
                    "tool {} did not produce evidence for final submission",
                    result.tool_name
                )));
            }
            let claim = result.evidence_claim.clone().ok_or_else(|| {
                AgentOsError::Validation(format!(
                    "tool {} omitted evidence claim",
                    result.tool_name
                ))
            })?;
            evidence_map.push(EvidenceMapEntry {
                claim,
                evidence_refs: result.evidence_ids.clone(),
            });
        }
        self.kernel.complete_task(CompleteTaskInput {
            task_id: session.acb.task.task_id.clone(),
            artifact_ids: artifacts
                .iter()
                .map(|artifact| artifact.artifact_id.clone())
                .collect(),
            evidence_ids: tool_results
                .iter()
                .flat_map(|result| result.evidence_ids.clone())
                .collect(),
        })?;
        self.kernel.submit_final(
            &session.acb.agent_id,
            &session.acb.task.task_id,
            FinalSubmission {
                summary: final_payload.summary,
                changed_artifacts: artifacts
                    .iter()
                    .map(|artifact| artifact.artifact_id.clone())
                    .collect(),
                evidence_map,
                unverified_claims: Vec::new(),
                known_risks: final_payload.known_risks,
                tests_run: final_payload.tests_run,
                tests_not_run: final_payload.tests_not_run,
                approvals: Vec::new(),
            },
        )?;
        self.kernel.transition_thread(
            &session.acb.thread_id,
            ThreadStatus::Completed,
            Some("role submitted final answer".to_string()),
        )?;
        self.kernel
            .record_checkpoint(&session.acb.thread_id, new_id("ckpt_"))?;
        Ok(RuntimeRunReport {
            thread_id: session.acb.thread_id.clone(),
            task_id: session.acb.task.task_id.clone(),
            status: ThreadStatus::Completed,
            provider_stream_session_ids: Vec::new(),
            tool_results,
            artifacts,
            final_submitted: true,
            events: self.kernel.events()?.len(),
        })
    }
}

pub(super) struct ToolRoleFinal {
    pub(super) summary: String,
    pub(super) known_risks: Vec<String>,
    pub(super) tests_run: Vec<String>,
    pub(super) tests_not_run: Vec<String>,
}

pub(super) struct ToolRoleSession {
    acb: AgentControlBlock,
    environment_id: String,
    environment_lease_id: String,
    capability_id: String,
}
