use agent_os_sys::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterGoalInput {
    pub namespace: String,
    pub created_by: String,
    pub title: String,
    pub description: String,
    pub acceptance_criteria: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub risk_level: u8,
    pub deadline: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnTaskInput {
    pub goal_id: String,
    pub parent_task_id: Option<String>,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub required_artifact_types: Vec<ArtifactType>,
    #[serde(default)]
    pub required_evidence_types: Vec<EvidenceType>,
    #[serde(default = "default_priority")]
    pub priority: i32,
    #[serde(default)]
    pub risk_level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTaskInput {
    pub task_id: String,
    pub status: Option<TaskStatus>,
    pub blocked_reason: Option<String>,
    pub owner_agent_id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub checklist: Option<Vec<ChecklistItem>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteTaskInput {
    pub task_id: String,
    #[serde(default)]
    pub artifact_ids: Vec<String>,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnAgentInput {
    pub task_id: String,
    pub role_profile_id: String,
    pub owner: String,
    pub local_goal: String,
    #[serde(default)]
    pub success_criteria: Vec<String>,
    #[serde(default)]
    pub failure_criteria: Vec<String>,
    pub parent_thread_id: Option<String>,
    #[serde(default)]
    pub workspace_roots: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachEvidenceInput {
    pub goal_id: String,
    pub task_id: Option<String>,
    pub artifact_id: Option<String>,
    pub evidence_type: EvidenceType,
    pub producer_agent_id: Option<String>,
    pub claim: Option<String>,
    pub blob_ref: Option<String>,
    pub content_hash: Option<String>,
    pub inline_bytes: Option<Vec<u8>>,
    #[serde(default = "empty_object")]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitArtifactInput {
    pub goal_id: String,
    pub task_id: String,
    pub owner_agent_id: String,
    pub artifact_type: ArtifactType,
    pub blob_ref: Option<String>,
    pub content_hash: Option<String>,
    pub inline_bytes: Option<Vec<u8>>,
    #[serde(default = "empty_object")]
    pub metadata: Value,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
    pub supersedes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageInput {
    pub message_type: String,
    pub route: MessageRoute,
    pub source_agent_id: String,
    pub source_thread_id: String,
    pub target_agent_id: Option<String>,
    pub target_thread_id: Option<String>,
    pub channel_id: Option<String>,
    pub goal_id: String,
    pub task_id: String,
    #[serde(default)]
    pub risk_level: u8,
    #[serde(default = "empty_object")]
    pub payload: Value,
    #[serde(default)]
    pub artifact_refs: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostBlackboardInput {
    pub source_agent_id: String,
    pub source_thread_id: String,
    pub channel_id: Option<String>,
    pub goal_id: String,
    pub task_id: Option<String>,
    pub scope: CommunicationScope,
    pub section: BlackboardSection,
    #[serde(default = "empty_object")]
    pub content: Value,
    pub confidence: Option<f64>,
    #[serde(default)]
    pub source_evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadContextInput {
    pub agent_id: String,
    pub task_id: String,
    #[serde(default)]
    pub loaded_refs: Vec<String>,
    pub summary_artifact_id: Option<String>,
    pub freshness: ContextFreshness,
    #[serde(default)]
    pub pollution_score: f64,
    #[serde(default)]
    pub token_estimate: u64,
}

/// Propose a durable memory write. Proposed memory is not authoritative until
/// committed (`docs/10-kernel-design/kernel-data-model.md:849-850`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposeMemoryWriteInput {
    pub namespace: String,
    pub content: Value,
    pub created_by_agent_id: String,
    #[serde(default)]
    pub source_evidence_ids: Vec<String>,
}

/// Commit (activate) a previously proposed memory record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitMemoryWriteInput {
    pub memory_id: String,
    pub approved_by: String,
}

/// Compact a thread's context window, replacing older context entries with a
/// summary and recording replacement provenance
/// (`docs/10-kernel-design/agent-thread-core-module.md:542-543, 833`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactContextInput {
    pub thread_id: String,
    pub agent_id: String,
    pub task_id: String,
    pub summary_artifact_id: Option<String>,
    /// Opaque refs to the context entries being superseded by this compaction.
    #[serde(default)]
    pub superseded_refs: Vec<String>,
    #[serde(default)]
    pub token_estimate: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInvokeInput {
    pub tool_name: String,
    #[serde(default = "empty_object")]
    pub input: Value,
    pub evidence_claim: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMementoInput {
    pub owner_agent_id: String,
    pub owner_thread_id: String,
    pub goal_id: String,
    pub task_id: String,
    pub anchor: MementoAnchor,
    pub content: MementoContent,
    pub projection: MementoProjection,
    #[serde(default)]
    pub links: MementoLinks,
    pub supersedes: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestReviewInput {
    pub artifact_id: String,
    pub reviewer_agent_id: String,
    #[serde(default)]
    pub focus: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitReviewInput {
    pub review_id: String,
    pub reviewer_agent_id: String,
    pub verdict: ReviewVerdict,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
    #[serde(default)]
    pub findings: Vec<ReviewFindingInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFindingInput {
    pub severity: FindingSeverity,
    pub title: String,
    pub body: String,
    pub location: Option<Value>,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitVerificationInput {
    pub artifact_id: Option<String>,
    pub final_artifact_id: Option<String>,
    pub verifier_agent_id: String,
    #[serde(default)]
    pub checked_claims: Vec<Value>,
    #[serde(default)]
    pub unsupported_claims: Vec<String>,
    #[serde(default)]
    pub stale_evidence_ids: Vec<String>,
    pub verdict: VerificationVerdict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestApprovalInput {
    pub goal_id: String,
    pub task_id: Option<String>,
    pub requested_by_agent_id: String,
    pub approval_type: ApprovalType,
    pub scope: ApprovalScope,
    pub risk_level: u8,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordApprovalInput {
    pub approval_id: String,
    pub status: ApprovalStatus,
    pub decision_by: String,
    pub decision_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetDebit {
    #[serde(default)]
    pub tokens: u64,
    #[serde(default)]
    pub tool_calls: u64,
    #[serde(default)]
    pub wall_time_ms: u64,
    #[serde(default)]
    pub cost: f64,
    #[serde(default)]
    pub human_interrupts: u64,
    #[serde(default)]
    pub model_requests: u64,
}

fn default_priority() -> i32 {
    10
}
