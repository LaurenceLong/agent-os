use agent_os_kernel::Kernel;
use agent_os_sys::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SoftwareCodeTask {
    pub workspace_root: PathBuf,
    pub task: String,
    pub file: PathBuf,
    pub old: String,
    pub new: String,
    pub test_program: PathBuf,
    pub test_args: Vec<String>,
    pub review_revision: Option<ReviewRevision>,
    pub edit_plan_source: SoftwareEditPlanSource,
}

impl SoftwareCodeTask {
    pub fn exact_edit(
        workspace_root: impl Into<PathBuf>,
        task: impl Into<String>,
        file: impl Into<PathBuf>,
        old: impl Into<String>,
        new: impl Into<String>,
        test_program: impl Into<PathBuf>,
        test_args: Vec<String>,
    ) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            task: task.into(),
            file: file.into(),
            old: old.into(),
            new: new.into(),
            test_program: test_program.into(),
            test_args,
            review_revision: None,
            edit_plan_source: SoftwareEditPlanSource::Exact,
        }
    }

    pub fn plan_from_task(
        workspace_root: impl Into<PathBuf>,
        task: impl Into<String>,
        scoped_file: Option<PathBuf>,
        test_program: impl Into<PathBuf>,
        test_args: Vec<String>,
    ) -> AgentOsResult<Self> {
        super::planner::plan_from_task(workspace_root, task, scoped_file, test_program, test_args)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoftwareEditPlanSource {
    Exact,
    Inferred,
}

#[derive(Debug, Clone)]
pub struct ReviewRevision {
    pub finding_title: String,
    pub finding_body: String,
    pub old: String,
    pub new: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwarePipelineReport {
    pub status: ThreadStatus,
    pub goal_id: String,
    pub role_thread_ids: BTreeMap<String, String>,
    pub artifact_ids: Vec<String>,
    pub latest_artifact_id: String,
    pub evidence_ids: Vec<String>,
    pub test_exit_code: i64,
    pub edit_plan_source: SoftwareEditPlanSource,
    pub planned_file: PathBuf,
    pub review_verdicts: Vec<ReviewVerdict>,
    pub review_finding_count: usize,
    pub verification_verdict: VerificationVerdict,
    pub supervisor_final_task_id: String,
    pub replay: SoftwareReplaySummary,
    pub events: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareReplaySummary {
    pub tasks: usize,
    pub threads: usize,
    pub artifacts: usize,
    pub evidence: usize,
    pub reviews: usize,
    pub review_findings: usize,
    pub verifications: usize,
    pub final_submissions: usize,
}

#[derive(Debug, Clone)]
pub struct SoftwareEngineeringPipeline {
    pub(super) kernel: Kernel,
    pub(super) distro: super::distro::SoftwareEngineeringDistro,
}

#[derive(Debug)]
pub(super) struct RoleSpawn<'a> {
    pub(super) goal_id: &'a str,
    pub(super) parent_task_id: Option<&'a str>,
    pub(super) depends_on: Vec<String>,
    pub(super) role_profile_id: &'a str,
    pub(super) title: &'a str,
    pub(super) description: &'a str,
    pub(super) required_artifact_types: Vec<ArtifactType>,
    pub(super) required_evidence_types: Vec<EvidenceType>,
    pub(super) parent_thread_id: Option<&'a str>,
    pub(super) workspace_root: &'a Path,
}

#[derive(Debug)]
pub(super) struct RoleExecution {
    pub(super) task: Task,
    pub(super) agent: AgentControlBlock,
}

#[derive(Debug)]
pub(super) struct ReviewRecord {
    pub(super) verdict: ReviewVerdict,
    pub(super) evidence_id: String,
}

#[derive(Debug)]
pub(super) struct VerificationRecord {
    pub(super) verdict: VerificationVerdict,
    pub(super) evidence_id: String,
}
