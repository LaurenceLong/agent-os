use crate::{empty_object, new_id, now_rfc3339, ABI_VERSION};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyscallEnvelope {
    pub abi_version: String,
    pub syscall_id: String,
    #[serde(rename = "type")]
    pub syscall_type: String,
    pub agent_id: String,
    pub task_id: String,
    pub session_id: String,
    pub capability_token: Option<String>,
    #[serde(default = "empty_object")]
    pub resource_scope: Value,
    pub risk_level: u8,
    pub idempotency_key: String,
    #[serde(default = "empty_object")]
    pub payload: Value,
    pub created_at: String,
}

impl SyscallEnvelope {
    pub fn new(
        syscall_type: impl Into<String>,
        agent_id: impl Into<String>,
        task_id: impl Into<String>,
        session_id: impl Into<String>,
        capability_token: Option<String>,
        risk_level: u8,
        payload: Value,
    ) -> Self {
        Self {
            abi_version: ABI_VERSION.to_string(),
            syscall_id: new_id("sys_"),
            syscall_type: syscall_type.into(),
            agent_id: agent_id.into(),
            task_id: task_id.into(),
            session_id: session_id.into(),
            capability_token,
            resource_scope: json!({}),
            risk_level,
            idempotency_key: new_id("idem_"),
            payload,
            created_at: now_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyscallResult {
    pub syscall_id: String,
    pub accepted: bool,
    pub event_ids: Vec<String>,
    pub output: Value,
    pub error: Option<String>,
}

impl SyscallResult {
    pub fn accepted(syscall_id: impl Into<String>, event_ids: Vec<String>, output: Value) -> Self {
        Self {
            syscall_id: syscall_id.into(),
            accepted: true,
            event_ids,
            output,
            error: None,
        }
    }

    pub fn rejected(syscall_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            syscall_id: syscall_id.into(),
            accepted: false,
            event_ids: Vec::new(),
            output: json!({}),
            error: Some(error.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub event_id: String,
    pub event_type: String,
    pub abi_version: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub agent_id: Option<String>,
    pub task_id: Option<String>,
    pub causation_id: Option<String>,
    pub correlation_id: Option<String>,
    #[serde(default = "empty_object")]
    pub payload: Value,
    pub created_at: String,
}

impl EventEnvelope {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        event_type: impl Into<String>,
        aggregate_type: impl Into<String>,
        aggregate_id: impl Into<String>,
        agent_id: Option<String>,
        task_id: Option<String>,
        causation_id: Option<String>,
        correlation_id: Option<String>,
        payload: Value,
    ) -> Self {
        Self {
            event_id: new_id("evt_"),
            event_type: event_type.into(),
            abi_version: ABI_VERSION.to_string(),
            aggregate_type: aggregate_type.into(),
            aggregate_id: aggregate_id.into(),
            agent_id,
            task_id,
            causation_id,
            correlation_id,
            payload,
            created_at: now_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOp {
    pub abi_version: String,
    pub op_id: String,
    pub thread_id: String,
    #[serde(rename = "type")]
    pub op_type: String,
    pub expected_turn_id: Option<String>,
    pub idempotency_key: String,
    pub causation_id: Option<String>,
    pub submitted_by: String,
    pub created_at: String,
    #[serde(default = "empty_object")]
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub abi_version: String,
    pub event_id: String,
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub sequence: u64,
    pub event_type: String,
    pub causation_id: Option<String>,
    pub correlation_id: Option<String>,
    pub created_at: String,
    #[serde(default = "empty_object")]
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "item_type", content = "payload")]
pub enum AgentItem {
    UserMessage(Value),
    SystemDirective(Value),
    DeveloperDirective(Value),
    ContextReference(Value),
    ContextSummary(Value),
    CommunicationMessage(Value),
    BlackboardPost(Value),
    HumanMessageRequest(Value),
    MementoFragment(Value),
    Plan(Value),
    ReasoningSummary(Value),
    AssistantMessage(Value),
    ToolCall(Value),
    ToolResult(Value),
    CommandExecution(Value),
    FileChange(Value),
    Patch(Value),
    Snapshot(Value),
    ArtifactReference(Value),
    EvidenceReference(Value),
    ReviewFinding(Value),
    VerificationResult(Value),
    SubagentMessage(Value),
    PermissionRequest(Value),
    PermissionDecision(Value),
    ContextCompaction(Value),
    FinalDraft(Value),
    FinalSubmission(Value),
    Error(Value),
}
