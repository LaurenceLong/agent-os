use agent_os_sys::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SoftwareCodeTask {
    pub workspace_root: PathBuf,
    pub task: String,
    pub file: PathBuf,
    pub old: String,
    pub new: String,
    pub test_program: PathBuf,
    pub test_args: Vec<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareExactEdit {
    pub old: String,
    pub new: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareWorkflowRequest {
    pub workspace_root: PathBuf,
    pub task: String,
    pub target_file: Option<PathBuf>,
    pub exact_edit: Option<SoftwareExactEdit>,
    pub test_program: PathBuf,
    pub test_args: Vec<String>,
    pub edit_plan_source: Option<SoftwareEditPlanSource>,
}

impl SoftwareWorkflowRequest {
    pub fn from_code_task(spec: &SoftwareCodeTask) -> Self {
        Self {
            workspace_root: spec.workspace_root.clone(),
            task: spec.task.clone(),
            target_file: Some(spec.file.clone()),
            exact_edit: Some(SoftwareExactEdit {
                old: spec.old.clone(),
                new: spec.new.clone(),
            }),
            test_program: spec.test_program.clone(),
            test_args: spec.test_args.clone(),
            edit_plan_source: Some(spec.edit_plan_source),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareWorkflowStep {
    pub label: String,
    pub core_role: String,
    pub objective: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareWorkflowPrompt {
    pub package_name: String,
    pub prompt: String,
    pub workflow_steps: Vec<SoftwareWorkflowStep>,
    pub acceptance_criteria: Vec<String>,
    pub review_policy_name: String,
    pub final_answer_policy_name: String,
    pub required_evidence_types: Vec<String>,
}
