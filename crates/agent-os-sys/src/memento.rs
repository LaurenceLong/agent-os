use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum MementoStatus {
    Draft,
    Armed,
    Triggered,
    Projected,
    Consumed,
    Superseded,
    Expired,
    Invalidated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MementoAnchorType {
    ChildThreadCompleted,
    ToolCompleted,
    ApprovalResolved,
    ReviewSubmitted,
    VerificationSubmitted,
    TurnResumed,
    CompactionCompleted,
    TimeReached,
    ArtifactStatusChanged,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MementoProjectionMode {
    OwnerContext,
    OwnerInterrupt,
    OwnerNextTurn,
    SupervisorReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum MementoPriority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MementoAnchor {
    pub anchor_type: MementoAnchorType,
    pub anchor_ref: Option<String>,
    pub condition: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MementoContent {
    pub title: String,
    pub body: String,
    pub checklist: Vec<String>,
    pub structured: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MementoProjection {
    pub mode: MementoProjectionMode,
    pub priority: MementoPriority,
    pub max_projection_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MementoImmutability {
    pub content_hash: String,
    pub committed_at: Option<String>,
    pub committed_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MementoVisibility {
    pub owner_only: bool,
    pub child_visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MementoLinks {
    pub related_child_thread_ids: Vec<String>,
    pub related_tool_call_ids: Vec<String>,
    pub related_artifact_ids: Vec<String>,
    pub related_evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MementoSupersession {
    pub supersedes: Option<String>,
    pub superseded_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MementoFragment {
    pub memento_id: String,
    pub owner_agent_id: String,
    pub owner_thread_id: String,
    pub goal_id: String,
    pub task_id: String,
    pub status: MementoStatus,
    pub anchor: MementoAnchor,
    pub content: MementoContent,
    pub projection: MementoProjection,
    pub immutability: MementoImmutability,
    pub visibility: MementoVisibility,
    pub links: MementoLinks,
    pub supersession: MementoSupersession,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: Option<String>,
}
