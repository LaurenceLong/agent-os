use crate::{ArtifactRecord, ToolExecutionRecord};
use agent_os_sys::{AttachMode, ThreadStatus};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub workspace_root: PathBuf,
    pub attach_mode: AttachMode,
    pub max_steps: u32,
    pub requested_model_alias: Option<String>,
    pub tool_risk_ceiling: u8,
    pub auto_commit_patch_artifacts: bool,
    pub fail_on_process_nonzero: bool,
}

impl RuntimeConfig {
    pub fn workspace_write(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            attach_mode: AttachMode::WorkspaceWrite,
            max_steps: 16,
            requested_model_alias: None,
            tool_risk_ceiling: 4,
            auto_commit_patch_artifacts: true,
            fail_on_process_nonzero: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeRunOverrides {
    pub sandbox_profile_id: Option<String>,
    pub tool_approval_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeRunReport {
    pub thread_id: String,
    pub task_id: String,
    pub status: ThreadStatus,
    pub provider_stream_session_ids: Vec<String>,
    pub tool_results: Vec<ToolExecutionRecord>,
    pub artifacts: Vec<ArtifactRecord>,
    pub final_submitted: bool,
    pub events: usize,
}
