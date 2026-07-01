use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationScheduleKind {
    ThreadWakeup,
    StandaloneRun,
    ProjectRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationScheduleStatus {
    Active,
    Paused,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationRunStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateAutomationScheduleInput {
    pub name: String,
    pub kind: AutomationScheduleKind,
    pub target_thread_id: Option<String>,
    pub workspace: Option<String>,
    pub prompt: String,
    pub next_run_at: Option<String>,
    pub interval_seconds: Option<u64>,
    pub created_by_client_id: String,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomationSchedule {
    pub schedule_id: String,
    pub name: String,
    pub kind: AutomationScheduleKind,
    pub status: AutomationScheduleStatus,
    pub target_thread_id: Option<String>,
    pub workspace: Option<String>,
    pub prompt: String,
    pub next_run_at: Option<String>,
    pub interval_seconds: Option<u64>,
    pub created_by_client_id: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_run_at: Option<String>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomationRun {
    pub run_id: String,
    pub schedule_id: String,
    pub kind: AutomationScheduleKind,
    pub status: AutomationRunStatus,
    pub target_thread_id: Option<String>,
    pub workspace: Option<String>,
    pub prompt: String,
    pub scheduled_for: String,
    pub queued_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error: Option<String>,
    pub payload: Value,
}

pub type AutomationScheduleProjection = AutomationSchedule;
pub type AutomationRunProjection = AutomationRun;
