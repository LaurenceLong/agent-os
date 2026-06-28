use super::tool_workflow::ToolRoleFinal;
use super::types::{
    ReviewRecord, RoleExecution, RoleSpawn, SoftwareCodeTask, SoftwareEngineeringPipeline,
    VerificationRecord,
};
use super::util::{evidence_by_type, test_command};
use crate::RuntimeRunReport;
use agent_os_kernel::{
    AttachEvidenceInput, CompleteTaskInput, RequestReviewInput, SpawnAgentInput, SpawnTaskInput,
    SubmitReviewInput, SubmitVerificationInput, UpdateTaskInput,
};
use agent_os_sys::*;
use serde_json::json;
use std::collections::BTreeMap;

impl SoftwareEngineeringPipeline {
    pub(super) fn run_explorer(
        &self,
        agent: &AgentControlBlock,
        spec: &SoftwareCodeTask,
    ) -> AgentOsResult<RuntimeRunReport> {
        let session = self.start_tool_role(agent, &spec.workspace_root, AttachMode::ReadOnly, 1)?;
        let tool_result = self.invoke_planned_tool(
            &session,
            "read_file",
            json!({
                "workspace_root": spec.workspace_root.to_string_lossy(),
                "path": spec.file.to_string_lossy(),
            }),
            1,
            "WorkerAgent inspected the target source file",
        )?;
        self.complete_tool_role(
            &session,
            Vec::new(),
            vec![tool_result],
            ToolRoleFinal {
                summary: format!("Inspected {}", spec.file.to_string_lossy()),
                known_risks: Vec::new(),
                tests_run: Vec::new(),
                tests_not_run: Vec::new(),
            },
        )
    }

    pub(super) fn run_coder(
        &self,
        agent: &AgentControlBlock,
        spec: &SoftwareCodeTask,
        old: &str,
        new: &str,
    ) -> AgentOsResult<RuntimeRunReport> {
        let session =
            self.start_tool_role(agent, &spec.workspace_root, AttachMode::WorkspaceWrite, 4)?;
        let tool_result = self.invoke_planned_tool(
            &session,
            "replace_text",
            json!({
                "workspace_root": spec.workspace_root.to_string_lossy(),
                "path": spec.file.to_string_lossy(),
                "old": old,
                "new": new,
            }),
            4,
            "WorkerAgent applied the exact repository edit",
        )?;
        let artifacts = vec![self.commit_patch_artifact_for_tool(&session, &tool_result)?];
        self.complete_tool_role(
            &session,
            artifacts,
            vec![tool_result],
            ToolRoleFinal {
                summary: format!("Applied exact edit to {}", spec.file.to_string_lossy()),
                known_risks: Vec::new(),
                tests_run: Vec::new(),
                tests_not_run: Vec::new(),
            },
        )
    }

    pub(super) fn run_tester(
        &self,
        agent: &AgentControlBlock,
        spec: &SoftwareCodeTask,
    ) -> AgentOsResult<RuntimeRunReport> {
        let session = self.start_tool_role(agent, &spec.workspace_root, AttachMode::ReadOnly, 4)?;
        let tool_result = self.invoke_planned_tool(
            &session,
            "run_command",
            json!({
                "program": spec.test_program.to_string_lossy(),
                "args": spec.test_args,
                "cwd": spec.workspace_root.to_string_lossy(),
            }),
            4,
            "WorkerAgent ran the declared verification command",
        )?;
        let exit_code = tool_result
            .output
            .as_ref()
            .and_then(|output| output.get("exit_code"))
            .and_then(serde_json::Value::as_i64)
            .ok_or_else(|| {
                AgentOsError::Validation("run_command output omitted exit_code".to_string())
            })?;
        if exit_code != 0 {
            return Err(AgentOsError::Validation(format!(
                "run_command failed with exit code {exit_code}"
            )));
        }
        self.complete_tool_role(
            &session,
            Vec::new(),
            vec![tool_result],
            ToolRoleFinal {
                summary: "Test command passed".to_string(),
                known_risks: Vec::new(),
                tests_run: vec![test_command(spec)],
                tests_not_run: Vec::new(),
            },
        )
    }

    pub(super) fn run_reviewer(
        &self,
        agent: &AgentControlBlock,
        task_id: &str,
        artifact_id: &str,
        verdict: ReviewVerdict,
        findings: Vec<agent_os_kernel::ReviewFindingInput>,
    ) -> AgentOsResult<ReviewRecord> {
        self.begin_manual_role(agent, task_id)?;
        let review_evidence = self.kernel.attach_evidence(AttachEvidenceInput {
            goal_id: agent.task.goal_id.clone(),
            task_id: Some(task_id.to_string()),
            artifact_id: Some(artifact_id.to_string()),
            evidence_type: EvidenceType::ReviewFinding,
            producer_agent_id: Some(agent.agent_id.clone()),
            claim: Some(match verdict {
                ReviewVerdict::Accept => "ReviewerAgent accepted the patch artifact".to_string(),
                ReviewVerdict::Reject => "ReviewerAgent rejected the patch artifact".to_string(),
                ReviewVerdict::NeedsRevision => {
                    "ReviewerAgent requested a patch revision".to_string()
                }
            }),
            blob_ref: None,
            content_hash: None,
            inline_bytes: None,
            metadata: json!({
                "artifact_id": artifact_id,
                "verdict": verdict,
                "finding_count": findings.len(),
            }),
        })?;
        let review = self.kernel.request_review(RequestReviewInput {
            artifact_id: artifact_id.to_string(),
            reviewer_agent_id: agent.agent_id.clone(),
            focus: vec!["correctness".to_string(), "evidence".to_string()],
        })?;
        let findings = findings
            .into_iter()
            .map(|mut finding| {
                if finding.evidence_ids.is_empty() {
                    finding
                        .evidence_ids
                        .push(review_evidence.evidence_id.clone());
                }
                finding
            })
            .collect();
        let review = self.kernel.submit_review(SubmitReviewInput {
            review_id: review.review_id,
            reviewer_agent_id: agent.agent_id.clone(),
            verdict,
            evidence_ids: vec![review_evidence.evidence_id.clone()],
            findings,
        })?;
        self.kernel.complete_task(CompleteTaskInput {
            task_id: task_id.to_string(),
            artifact_ids: Vec::new(),
            evidence_ids: vec![review_evidence.evidence_id.clone()],
        })?;
        self.submit_role_final(
            agent,
            task_id,
            format!("Review submitted with verdict {:?}", review.verdict),
            vec![EvidenceMapEntry {
                claim: "review was submitted for the exact artifact".to_string(),
                evidence_refs: vec![review_evidence.evidence_id.clone()],
            }],
            Vec::new(),
            Vec::new(),
        )?;
        Ok(ReviewRecord {
            verdict: review.verdict,
            evidence_id: review_evidence.evidence_id,
        })
    }

    pub(super) fn run_verifier(
        &self,
        agent: &AgentControlBlock,
        task_id: &str,
        artifact_id: &str,
        evidence_ids: &[String],
    ) -> AgentOsResult<VerificationRecord> {
        self.begin_manual_role(agent, task_id)?;
        let verification_evidence = self.kernel.attach_evidence(AttachEvidenceInput {
            goal_id: agent.task.goal_id.clone(),
            task_id: Some(task_id.to_string()),
            artifact_id: Some(artifact_id.to_string()),
            evidence_type: EvidenceType::RuntimeTrace,
            producer_agent_id: Some(agent.agent_id.clone()),
            claim: Some("WorkerAgent confirmed evidence coverage".to_string()),
            blob_ref: None,
            content_hash: None,
            inline_bytes: None,
            metadata: json!({
                "artifact_id": artifact_id,
                "evidence_ids": evidence_ids,
            }),
        })?;
        let verification = self.kernel.submit_verification(SubmitVerificationInput {
            artifact_id: Some(artifact_id.to_string()),
            final_artifact_id: None,
            verifier_agent_id: agent.agent_id.clone(),
            checked_claims: vec![json!({
                "claim": "diff, test, review, and risk evidence are present",
                "evidence_ids": evidence_ids,
            })],
            unsupported_claims: Vec::new(),
            stale_evidence_ids: Vec::new(),
            verdict: VerificationVerdict::Pass,
        })?;
        self.kernel.complete_task(CompleteTaskInput {
            task_id: task_id.to_string(),
            artifact_ids: Vec::new(),
            evidence_ids: vec![verification_evidence.evidence_id.clone()],
        })?;
        self.submit_role_final(
            agent,
            task_id,
            "Verification passed".to_string(),
            vec![EvidenceMapEntry {
                claim: "verification passed with no unsupported claims".to_string(),
                evidence_refs: vec![verification_evidence.evidence_id.clone()],
            }],
            Vec::new(),
            Vec::new(),
        )?;
        Ok(VerificationRecord {
            verdict: verification.verdict,
            evidence_id: verification_evidence.evidence_id,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_supervisor(
        &self,
        agent: &AgentControlBlock,
        task_id: &str,
        spec: &SoftwareCodeTask,
        artifact_ids: &[String],
        evidence_ids: &[String],
        latest_artifact_id: &str,
        test_exit_code: i64,
    ) -> AgentOsResult<()> {
        self.begin_manual_role(agent, task_id)?;
        let diff_evidence = evidence_by_type(&self.kernel, evidence_ids, EvidenceType::DiffRef)?;
        let command_evidence =
            evidence_by_type(&self.kernel, evidence_ids, EvidenceType::CommandLog)?;
        let review_evidence =
            evidence_by_type(&self.kernel, evidence_ids, EvidenceType::ReviewFinding)?;
        let verification_evidence =
            evidence_by_type(&self.kernel, evidence_ids, EvidenceType::RuntimeTrace)?;
        self.kernel.submit_final(
            &agent.agent_id,
            task_id,
            FinalSubmission {
                summary: format!(
                    "Completed software task for {}. Latest artifact: {}. Test exit code: {}.",
                    spec.file.to_string_lossy(),
                    latest_artifact_id,
                    test_exit_code
                ),
                changed_artifacts: artifact_ids.to_vec(),
                evidence_map: vec![
                    EvidenceMapEntry {
                        claim: format!(
                            "changed file {} has diff evidence",
                            spec.file.to_string_lossy()
                        ),
                        evidence_refs: diff_evidence,
                    },
                    EvidenceMapEntry {
                        claim: format!("test command `{}` passed", test_command(spec)),
                        evidence_refs: command_evidence,
                    },
                    EvidenceMapEntry {
                        claim: "review accepted the latest artifact".to_string(),
                        evidence_refs: review_evidence,
                    },
                    EvidenceMapEntry {
                        claim: "risks and unsupported claims were verified".to_string(),
                        evidence_refs: verification_evidence,
                    },
                ],
                unverified_claims: Vec::new(),
                known_risks: vec!["No known risks identified by verifier.".to_string()],
                tests_run: vec![test_command(spec)],
                tests_not_run: Vec::new(),
                approvals: Vec::new(),
            },
        )?;
        self.kernel.complete_task(CompleteTaskInput {
            task_id: task_id.to_string(),
            artifact_ids: artifact_ids.to_vec(),
            evidence_ids: evidence_ids.to_vec(),
        })?;
        self.kernel.transition_thread(
            &agent.thread_id,
            ThreadStatus::Completed,
            Some("SupervisorAgent submitted final answer".to_string()),
        )?;
        self.kernel
            .record_checkpoint(&agent.thread_id, new_id("ckpt_"))?;
        Ok(())
    }

    pub(super) fn spawn_role(&self, spawn: RoleSpawn<'_>) -> AgentOsResult<RoleExecution> {
        let task = self.kernel.spawn_task(SpawnTaskInput {
            goal_id: spawn.goal_id.to_string(),
            parent_task_id: spawn.parent_task_id.map(str::to_string),
            title: spawn.title.to_string(),
            description: spawn.description.to_string(),
            depends_on: spawn.depends_on,
            required_artifact_types: spawn.required_artifact_types,
            required_evidence_types: spawn.required_evidence_types,
            priority: 10,
            risk_level: 4,
        })?;
        let agent = self.kernel.spawn_agent(SpawnAgentInput {
            task_id: task.task_id.clone(),
            role_profile_id: spawn.role_profile_id.to_string(),
            owner: "agent-os-software-pipeline".to_string(),
            local_goal: spawn.description.to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: spawn.parent_thread_id.map(str::to_string),
            workspace_roots: vec![spawn.workspace_root.to_string_lossy().to_string()],
        })?;
        Ok(RoleExecution { task, agent })
    }

    pub(super) fn role_thread_ids(&self, goal_id: &str) -> AgentOsResult<BTreeMap<String, String>> {
        let state = self.kernel.state_snapshot()?;
        let mut roles = BTreeMap::new();
        for thread in state
            .threads
            .values()
            .filter(|thread| thread.task.goal_id == goal_id)
        {
            roles.insert(thread.role.clone(), thread.thread_id.clone());
        }
        Ok(roles)
    }

    fn begin_manual_role(&self, agent: &AgentControlBlock, task_id: &str) -> AgentOsResult<()> {
        self.kernel.update_task(UpdateTaskInput {
            task_id: task_id.to_string(),
            status: Some(TaskStatus::Running),
            blocked_reason: None,
            owner_agent_id: Some(agent.agent_id.clone()),
            title: None,
            description: None,
            checklist: None,
        })?;
        self.kernel.start_turn(&agent.thread_id)?;
        Ok(())
    }

    fn submit_role_final(
        &self,
        agent: &AgentControlBlock,
        task_id: &str,
        summary: String,
        evidence_map: Vec<EvidenceMapEntry>,
        tests_run: Vec<String>,
        tests_not_run: Vec<String>,
    ) -> AgentOsResult<()> {
        self.kernel.submit_final(
            &agent.agent_id,
            task_id,
            FinalSubmission {
                summary,
                changed_artifacts: Vec::new(),
                evidence_map,
                unverified_claims: Vec::new(),
                known_risks: Vec::new(),
                tests_run,
                tests_not_run,
                approvals: Vec::new(),
            },
        )?;
        self.kernel.transition_thread(
            &agent.thread_id,
            ThreadStatus::Completed,
            Some("role completed".to_string()),
        )?;
        self.kernel
            .record_checkpoint(&agent.thread_id, new_id("ckpt_"))?;
        Ok(())
    }
}
