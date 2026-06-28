use super::types::{
    RoleSpawn, SoftwareCodeTask, SoftwareEngineeringPipeline, SoftwarePipelineReport,
    SoftwareReplaySummary,
};
use super::util::{artifact_ids, collect_evidence_ids, latest_artifact_id, process_exit_code};
use agent_os_kernel::{Kernel, RegisterGoalInput, ReviewFindingInput};
use agent_os_sys::*;
use serde_json::json;

impl SoftwareEngineeringPipeline {
    pub fn new(kernel: Kernel) -> AgentOsResult<Self> {
        Ok(Self {
            kernel,
            distro: super::distro::SoftwareEngineeringDistro::load_default()?,
        })
    }

    pub fn kernel(&self) -> Kernel {
        self.kernel.clone()
    }

    pub fn run_code_task(&self, spec: SoftwareCodeTask) -> AgentOsResult<SoftwarePipelineReport> {
        let review_policy_name = self.distro.review_policy["policy_name"]
            .as_str()
            .ok_or_else(|| {
                AgentOsError::Validation(
                    "software distro review policy requires policy_name".to_string(),
                )
            })?;
        let goal = self.kernel.register_goal(RegisterGoalInput {
            namespace: self.distro.manifest.package_name.clone(),
            created_by: format!("distro:{}", self.distro.manifest.package_name),
            title: spec.task.clone(),
            description: spec.task.clone(),
            acceptance_criteria: self.distro.final_answer_policy["acceptance_criteria"]
                .as_array()
                .ok_or_else(|| {
                    AgentOsError::Validation(
                        "software distro final-answer policy requires acceptance_criteria"
                            .to_string(),
                    )
                })?
                .iter()
                .map(|item| {
                    item.as_str().map(str::to_string).ok_or_else(|| {
                        AgentOsError::Validation(
                            "software distro acceptance criteria must be strings".to_string(),
                        )
                    })
                })
                .collect::<AgentOsResult<Vec<_>>>()?,
            constraints: vec![format!("review policy: {review_policy_name}")],
            risk_level: 4,
            deadline: None,
        })?;
        let supervisor = self.spawn_supervisor(&goal.goal_id, &spec)?;
        let explorer = self.spawn_explorer(&goal.goal_id, &supervisor)?;
        let explorer_report = self.run_explorer(&explorer.agent, &spec)?;

        let coder = self.spawn_coder(&goal.goal_id, &supervisor, vec![explorer.task.task_id])?;
        let coder_report = self.run_coder(&coder.agent, &spec, &spec.old, &spec.new)?;
        let mut coder_reports = vec![coder_report];

        let tester = self.spawn_tester(&goal.goal_id, &supervisor, vec![coder.task.task_id])?;
        let mut tester_report = self.run_tester(&tester.agent, &spec)?;

        let reviewer =
            self.spawn_reviewer(&goal.goal_id, &supervisor, vec![tester.task.task_id])?;
        let mut review_records = Vec::new();
        let mut review_finding_count = 0;
        let first_artifact_id = latest_artifact_id(&coder_reports)?;
        if let Some(revision) = &spec.review_revision {
            let review_record = self.run_reviewer(
                &reviewer.agent,
                &reviewer.task.task_id,
                &first_artifact_id,
                ReviewVerdict::NeedsRevision,
                vec![ReviewFindingInput {
                    severity: FindingSeverity::P2,
                    title: revision.finding_title.clone(),
                    body: revision.finding_body.clone(),
                    location: Some(json!({ "file": spec.file.to_string_lossy() })),
                    evidence_ids: Vec::new(),
                }],
            )?;
            review_finding_count += 1;
            review_records.push(review_record);

            let revision_coder =
                self.spawn_revision_coder(&goal.goal_id, &supervisor, &reviewer.task.task_id)?;
            let revision_report =
                self.run_coder(&revision_coder.agent, &spec, &revision.old, &revision.new)?;
            coder_reports.push(revision_report);

            let revision_tester = self.spawn_revision_tester(
                &goal.goal_id,
                &supervisor,
                &revision_coder.task.task_id,
            )?;
            tester_report = self.run_tester(&revision_tester.agent, &spec)?;
        }

        let latest_artifact_id = latest_artifact_id(&coder_reports)?;
        let accept_reviewer = if spec.review_revision.is_some() {
            self.spawn_accept_reviewer(&goal.goal_id, &supervisor, &reviewer.task.task_id)?
        } else {
            reviewer
        };
        let accept_review = self.run_reviewer(
            &accept_reviewer.agent,
            &accept_reviewer.task.task_id,
            &latest_artifact_id,
            ReviewVerdict::Accept,
            Vec::new(),
        )?;
        review_records.push(accept_review);

        let verifier =
            self.spawn_verifier(&goal.goal_id, &supervisor, &accept_reviewer.task.task_id)?;
        let all_artifact_ids = artifact_ids(&coder_reports);
        let all_evidence_ids = collect_evidence_ids(
            [&explorer_report, &tester_report]
                .into_iter()
                .chain(coder_reports.iter()),
            &review_records,
            &[],
        );
        let verification_record = self.run_verifier(
            &verifier.agent,
            &verifier.task.task_id,
            &latest_artifact_id,
            &all_evidence_ids,
        )?;
        let all_evidence_ids = collect_evidence_ids(
            [&explorer_report, &tester_report]
                .into_iter()
                .chain(coder_reports.iter()),
            &review_records,
            std::slice::from_ref(&verification_record.evidence_id),
        );
        self.run_supervisor(
            &supervisor.agent,
            &supervisor.task.task_id,
            &spec,
            &all_artifact_ids,
            &all_evidence_ids,
            &latest_artifact_id,
            process_exit_code(&tester_report.tool_results)?,
        )?;

        let replayed = Kernel::from_events(&self.kernel.events()?)?;
        let replayed_state = replayed.state_snapshot()?;
        Ok(SoftwarePipelineReport {
            status: ThreadStatus::Completed,
            goal_id: goal.goal_id.clone(),
            role_thread_ids: self.role_thread_ids(&goal.goal_id)?,
            artifact_ids: all_artifact_ids,
            latest_artifact_id,
            evidence_ids: all_evidence_ids,
            test_exit_code: process_exit_code(&tester_report.tool_results)?,
            edit_plan_source: spec.edit_plan_source,
            planned_file: spec.file,
            review_verdicts: review_records.iter().map(|record| record.verdict).collect(),
            review_finding_count,
            verification_verdict: verification_record.verdict,
            supervisor_final_task_id: supervisor.task.task_id,
            replay: SoftwareReplaySummary {
                tasks: replayed_state.tasks.len(),
                threads: replayed_state.threads.len(),
                artifacts: replayed_state.artifacts.len(),
                evidence: replayed_state.evidence.len(),
                reviews: replayed_state.reviews.len(),
                review_findings: replayed_state.review_findings.len(),
                verifications: replayed_state.verifications.len(),
                final_submissions: replayed_state.final_submissions.len(),
            },
            events: self.kernel.events()?.len(),
        })
    }

    fn spawn_supervisor(
        &self,
        goal_id: &str,
        spec: &SoftwareCodeTask,
    ) -> AgentOsResult<super::types::RoleExecution> {
        self.spawn_role(RoleSpawn {
            goal_id,
            parent_task_id: None,
            depends_on: Vec::new(),
            role_profile_id: "role_supervisor",
            title: "Supervise software task",
            description: &self.distro.supervisor_prompt,
            required_artifact_types: vec![ArtifactType::Patch],
            required_evidence_types: vec![
                EvidenceType::DiffRef,
                EvidenceType::CommandLog,
                EvidenceType::ReviewFinding,
                EvidenceType::RuntimeTrace,
            ],
            parent_thread_id: None,
            workspace_root: &spec.workspace_root,
        })
    }

    fn spawn_explorer(
        &self,
        goal_id: &str,
        supervisor: &super::types::RoleExecution,
    ) -> AgentOsResult<super::types::RoleExecution> {
        self.spawn_role(RoleSpawn {
            goal_id,
            parent_task_id: Some(&supervisor.task.task_id),
            depends_on: Vec::new(),
            role_profile_id: "role_worker",
            title: "Explore target file",
            description: &self.distro.worker_prompt,
            required_artifact_types: Vec::new(),
            required_evidence_types: vec![EvidenceType::SourceRef],
            parent_thread_id: Some(&supervisor.agent.thread_id),
            workspace_root: first_workspace(&supervisor.agent),
        })
    }

    fn spawn_coder(
        &self,
        goal_id: &str,
        supervisor: &super::types::RoleExecution,
        depends_on: Vec<String>,
    ) -> AgentOsResult<super::types::RoleExecution> {
        self.spawn_role(RoleSpawn {
            goal_id,
            parent_task_id: Some(&supervisor.task.task_id),
            depends_on,
            role_profile_id: "role_worker",
            title: "Apply exact patch",
            description: &self.distro.worker_prompt,
            required_artifact_types: vec![ArtifactType::Patch],
            required_evidence_types: vec![EvidenceType::DiffRef],
            parent_thread_id: Some(&supervisor.agent.thread_id),
            workspace_root: first_workspace(&supervisor.agent),
        })
    }

    fn spawn_tester(
        &self,
        goal_id: &str,
        supervisor: &super::types::RoleExecution,
        depends_on: Vec<String>,
    ) -> AgentOsResult<super::types::RoleExecution> {
        self.spawn_role(RoleSpawn {
            goal_id,
            parent_task_id: Some(&supervisor.task.task_id),
            depends_on,
            role_profile_id: "role_worker",
            title: "Run verification command",
            description: &self.distro.worker_prompt,
            required_artifact_types: Vec::new(),
            required_evidence_types: vec![EvidenceType::CommandLog],
            parent_thread_id: Some(&supervisor.agent.thread_id),
            workspace_root: first_workspace(&supervisor.agent),
        })
    }

    fn spawn_reviewer(
        &self,
        goal_id: &str,
        supervisor: &super::types::RoleExecution,
        depends_on: Vec<String>,
    ) -> AgentOsResult<super::types::RoleExecution> {
        self.spawn_role(RoleSpawn {
            goal_id,
            parent_task_id: Some(&supervisor.task.task_id),
            depends_on,
            role_profile_id: "role_reviewer",
            title: "Review exact artifact",
            description: &self.distro.reviewer_prompt,
            required_artifact_types: Vec::new(),
            required_evidence_types: vec![EvidenceType::ReviewFinding],
            parent_thread_id: Some(&supervisor.agent.thread_id),
            workspace_root: first_workspace(&supervisor.agent),
        })
    }

    fn spawn_revision_coder(
        &self,
        goal_id: &str,
        supervisor: &super::types::RoleExecution,
        review_task_id: &str,
    ) -> AgentOsResult<super::types::RoleExecution> {
        self.spawn_role(RoleSpawn {
            goal_id,
            parent_task_id: Some(&supervisor.task.task_id),
            depends_on: vec![review_task_id.to_string()],
            role_profile_id: "role_worker",
            title: "Revise patch from review finding",
            description: &self.distro.worker_prompt,
            required_artifact_types: vec![ArtifactType::Patch],
            required_evidence_types: vec![EvidenceType::DiffRef],
            parent_thread_id: Some(&supervisor.agent.thread_id),
            workspace_root: first_workspace(&supervisor.agent),
        })
    }

    fn spawn_revision_tester(
        &self,
        goal_id: &str,
        supervisor: &super::types::RoleExecution,
        coder_task_id: &str,
    ) -> AgentOsResult<super::types::RoleExecution> {
        self.spawn_role(RoleSpawn {
            goal_id,
            parent_task_id: Some(&supervisor.task.task_id),
            depends_on: vec![coder_task_id.to_string()],
            role_profile_id: "role_worker",
            title: "Re-run verification command",
            description: &self.distro.worker_prompt,
            required_artifact_types: Vec::new(),
            required_evidence_types: vec![EvidenceType::CommandLog],
            parent_thread_id: Some(&supervisor.agent.thread_id),
            workspace_root: first_workspace(&supervisor.agent),
        })
    }

    fn spawn_accept_reviewer(
        &self,
        goal_id: &str,
        supervisor: &super::types::RoleExecution,
        previous_review_task_id: &str,
    ) -> AgentOsResult<super::types::RoleExecution> {
        self.spawn_role(RoleSpawn {
            goal_id,
            parent_task_id: Some(&supervisor.task.task_id),
            depends_on: vec![previous_review_task_id.to_string()],
            role_profile_id: "role_reviewer",
            title: "Review revised artifact",
            description: &self.distro.reviewer_prompt,
            required_artifact_types: Vec::new(),
            required_evidence_types: vec![EvidenceType::ReviewFinding],
            parent_thread_id: Some(&supervisor.agent.thread_id),
            workspace_root: first_workspace(&supervisor.agent),
        })
    }

    fn spawn_verifier(
        &self,
        goal_id: &str,
        supervisor: &super::types::RoleExecution,
        review_task_id: &str,
    ) -> AgentOsResult<super::types::RoleExecution> {
        self.spawn_role(RoleSpawn {
            goal_id,
            parent_task_id: Some(&supervisor.task.task_id),
            depends_on: vec![review_task_id.to_string()],
            role_profile_id: "role_worker",
            title: "Verify evidence coverage",
            description: &self.distro.worker_prompt,
            required_artifact_types: Vec::new(),
            required_evidence_types: vec![EvidenceType::RuntimeTrace],
            parent_thread_id: Some(&supervisor.agent.thread_id),
            workspace_root: first_workspace(&supervisor.agent),
        })
    }
}

fn first_workspace(agent: &AgentControlBlock) -> &std::path::Path {
    std::path::Path::new(
        agent
            .config_snapshot
            .workspace_roots
            .first()
            .map(String::as_str)
            .unwrap_or("."),
    )
}
