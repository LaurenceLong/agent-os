use agent_os_sys::{
    new_id, now_rfc3339, AgentControlBlock, AgentOsError, AgentOsResult, TurnStatus,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeJob {
    pub client_thread_id: String,
    pub turn_id: String,
    pub agent_thread_id: String,
    pub workspace: String,
    pub provider_profile: String,
    pub model: String,
}

impl RuntimeJob {
    pub fn from_active_turn(thread: &AgentControlBlock) -> AgentOsResult<Self> {
        let turn_id = thread.active_turn.turn_id.clone().ok_or_else(|| {
            AgentOsError::Validation(format!(
                "thread {} has no active turn for RuntimeJob",
                thread.thread_id
            ))
        })?;
        if thread.active_turn.status != Some(TurnStatus::InProgress) {
            return Err(AgentOsError::InvalidTransition(format!(
                "thread {} active turn is not InProgress",
                thread.thread_id
            )));
        }
        let workspace = thread
            .config_snapshot
            .workspace_roots
            .first()
            .cloned()
            .ok_or_else(|| {
                AgentOsError::Validation(format!(
                    "thread {} has no workspace root for RuntimeJob",
                    thread.thread_id
                ))
            })?;
        Ok(Self {
            client_thread_id: thread.thread_id.clone(),
            turn_id,
            agent_thread_id: thread.thread_id.clone(),
            workspace,
            provider_profile: thread.config_snapshot.provider_profile_id.clone(),
            model: thread.config_snapshot.model_id.clone(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeJobStatus {
    Queued,
    Running,
    Completed,
    Blocked,
    Failed,
    Interrupted,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeJobRecord {
    pub runtime_job_id: String,
    pub job: RuntimeJob,
    pub status: RuntimeJobStatus,
    pub created_at: String,
    pub updated_at: String,
    pub last_error: Option<String>,
}

impl RuntimeJobRecord {
    pub fn queued(job: RuntimeJob) -> Self {
        let now = now_rfc3339();
        Self {
            runtime_job_id: new_id("rtjob_"),
            job,
            status: RuntimeJobStatus::Queued,
            created_at: now.clone(),
            updated_at: now,
            last_error: None,
        }
    }

    pub fn interrupt(&mut self) {
        self.status = RuntimeJobStatus::Interrupted;
        self.updated_at = now_rfc3339();
    }

    pub fn start(&mut self) {
        self.status = RuntimeJobStatus::Running;
        self.updated_at = now_rfc3339();
        self.last_error = None;
    }

    pub fn requeue(&mut self) {
        self.status = RuntimeJobStatus::Queued;
        self.updated_at = now_rfc3339();
        self.last_error = None;
    }

    pub fn complete(&mut self) {
        self.status = RuntimeJobStatus::Completed;
        self.updated_at = now_rfc3339();
        self.last_error = None;
    }

    pub fn block(&mut self, reason: String) {
        self.status = RuntimeJobStatus::Blocked;
        self.updated_at = now_rfc3339();
        self.last_error = Some(reason);
    }

    pub fn fail(&mut self, error: String) {
        self.status = RuntimeJobStatus::Failed;
        self.updated_at = now_rfc3339();
        self.last_error = Some(error);
    }
}
