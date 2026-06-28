use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlackboardSection {
    Goal,
    Constraint,
    KnownFact,
    Hypothesis,
    Decision,
    OpenQuestion,
    Risk,
    TestResult,
    ReviewResult,
    AcceptanceCriterion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum BlackboardStatus {
    Active,
    Superseded,
    Invalidated,
    Resolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ContextFreshness {
    Fresh,
    Stale,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum MemoryStatus {
    Proposed,
    Active,
    Superseded,
    Invalidated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackboardEntry {
    pub entry_id: String,
    pub goal_id: String,
    pub task_id: Option<String>,
    pub section: BlackboardSection,
    pub status: BlackboardStatus,
    pub content: Value,
    pub confidence: Option<f64>,
    pub source_evidence_ids: Vec<String>,
    pub created_by_agent_id: Option<String>,
    pub created_at: String,
    pub superseded_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub context_id: String,
    pub agent_id: String,
    pub task_id: String,
    pub loaded_refs: Vec<String>,
    pub summary_artifact_id: Option<String>,
    pub freshness: ContextFreshness,
    pub pollution_score: f64,
    pub token_estimate: u64,
    pub created_at: String,
    pub invalidated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub memory_id: String,
    pub namespace: String,
    pub status: MemoryStatus,
    pub content: Value,
    pub source_evidence_ids: Vec<String>,
    pub created_by_agent_id: Option<String>,
    pub approved_by: Option<String>,
    pub created_at: String,
    pub activated_at: Option<String>,
    pub superseded_by: Option<String>,
}

/// A durable record of a context compaction, carrying the replacement
/// provenance required by
/// `docs/10-kernel-design/agent-thread-core-module.md:542-543`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextCompaction {
    pub compaction_id: String,
    pub thread_id: String,
    pub agent_id: String,
    pub task_id: String,
    /// Artifact that summarizes the superseded context, if any.
    pub summary_artifact_id: Option<String>,
    /// Refs to the context entries replaced by this compaction. The original
    /// history remains linked so replay can reconstruct the full window.
    pub superseded_refs: Vec<String>,
    pub token_estimate: u64,
    pub created_at: String,
}
