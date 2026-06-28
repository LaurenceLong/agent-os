use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    Plan,
    Patch,
    TestLog,
    BenchmarkResult,
    ReviewReport,
    AnalysisNote,
    FinalAnswer,
    MemoryProposal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ArtifactStatus {
    Draft,
    Submitted,
    UnderReview,
    ReviewFailed,
    NeedsRevision,
    Verified,
    Accepted,
    Rejected,
    Superseded,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceType {
    SourceRef,
    DiffRef,
    CommandLog,
    TestResult,
    BenchmarkResult,
    ReviewFinding,
    ApprovalRecord,
    RuntimeTrace,
    Screenshot,
    ExternalReference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum EvidenceStatus {
    Active,
    Invalidated,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ReviewStatus {
    Requested,
    InProgress,
    Submitted,
    Accepted,
    Rejected,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewVerdict {
    Accept,
    Reject,
    NeedsRevision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FindingSeverity {
    P0,
    P1,
    P2,
    P3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum FindingStatus {
    Open,
    Accepted,
    Rejected,
    Resolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum VerificationStatus {
    Requested,
    Submitted,
    Failed,
    Passed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationVerdict {
    Pass,
    Fail,
    Inconclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalType {
    Human,
    Policy,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ApprovalStatus {
    Requested,
    Approved,
    Denied,
    Expired,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub artifact_id: String,
    pub goal_id: String,
    pub task_id: String,
    pub owner_agent_id: String,
    pub artifact_type: ArtifactType,
    pub version: u32,
    pub status: ArtifactStatus,
    pub blob_ref: Option<String>,
    pub content_hash: Option<String>,
    pub metadata: Value,
    pub created_at: String,
    pub updated_at: String,
    pub supersedes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub evidence_id: String,
    pub goal_id: String,
    pub task_id: Option<String>,
    pub artifact_id: Option<String>,
    pub evidence_type: EvidenceType,
    pub producer_agent_id: Option<String>,
    pub claim: Option<String>,
    pub blob_ref: Option<String>,
    pub content_hash: Option<String>,
    pub metadata: Value,
    pub status: EvidenceStatus,
    pub created_at: String,
    pub invalidated_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub review_id: String,
    pub artifact_id: String,
    pub artifact_version: u32,
    pub reviewer_agent_id: String,
    pub status: ReviewStatus,
    pub focus: Vec<String>,
    pub verdict: ReviewVerdict,
    pub evidence_ids: Vec<String>,
    pub created_at: String,
    pub submitted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFinding {
    pub finding_id: String,
    pub review_id: String,
    pub severity: FindingSeverity,
    pub title: String,
    pub body: String,
    pub location: Option<Value>,
    pub evidence_ids: Vec<String>,
    pub status: FindingStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verification {
    pub verification_id: String,
    pub artifact_id: Option<String>,
    pub final_artifact_id: Option<String>,
    pub verifier_agent_id: String,
    pub status: VerificationStatus,
    pub checked_claims: Vec<Value>,
    pub unsupported_claims: Vec<String>,
    pub stale_evidence_ids: Vec<String>,
    pub verdict: VerificationVerdict,
    pub created_at: String,
    pub submitted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalScope {
    pub syscall_types: Vec<String>,
    pub resource_scopes: Vec<Value>,
    pub risk_ceiling: u8,
    pub goal_id: String,
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approval {
    pub approval_id: String,
    pub goal_id: String,
    pub task_id: Option<String>,
    pub requested_by_agent_id: String,
    pub approval_type: ApprovalType,
    pub scope: ApprovalScope,
    pub risk_level: u8,
    pub status: ApprovalStatus,
    pub decision_by: Option<String>,
    pub decision_reason: Option<String>,
    pub created_at: String,
    pub decided_at: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceMapEntry {
    pub claim: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalSubmission {
    pub summary: String,
    pub changed_artifacts: Vec<String>,
    pub evidence_map: Vec<EvidenceMapEntry>,
    pub unverified_claims: Vec<String>,
    pub known_risks: Vec<String>,
    pub tests_run: Vec<String>,
    pub tests_not_run: Vec<String>,
    pub approvals: Vec<String>,
}
