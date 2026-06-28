use crate::EvidenceType;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ToolCallStatus {
    Proposed,
    Validated,
    PendingApproval,
    Denied,
    Running,
    Completed,
    Failed,
    Cancelled,
    TimedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolDriverClass {
    KernelBuiltin,
    Filesystem,
    Shell,
    Git,
    Mcp,
    Browser,
    ExternalApi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdempotencyMode {
    None,
    KernelDeduplicated,
    ToolNative,
    ManualCompensationRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityToken {
    pub capability_id: String,
    pub agent_id: String,
    pub task_id: String,
    pub role: String,
    pub syscalls: Vec<String>,
    pub resource_scopes: Vec<String>,
    pub risk_ceiling: u8,
    pub expires_at: Option<String>,
    pub approval_id: Option<String>,
    pub created_at: String,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub tool_id: String,
    pub name: String,
    pub version: String,
    pub driver_class: ToolDriverClass,
    pub risk_level: u8,
    pub input_schema: Value,
    pub output_schema: Value,
    pub idempotency: IdempotencyMode,
    pub evidence_type: Option<EvidenceType>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInvocation {
    pub call_id: String,
    pub tool_id: String,
    pub tool_name: String,
    pub agent_id: String,
    pub task_id: String,
    pub status: ToolCallStatus,
    pub risk_level: u8,
    pub input: Value,
    pub output: Option<Value>,
    pub evidence_ids: Vec<String>,
    pub audit_refs: Vec<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}
