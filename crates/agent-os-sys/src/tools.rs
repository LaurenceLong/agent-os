use crate::EvidenceType;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

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
pub struct ToolExample {
    pub description: String,
    pub parameters: Value,
    pub expected_result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub tool_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub version: String,
    pub driver_class: ToolDriverClass,
    pub risk_level: u8,
    pub input_schema: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_input_schema: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<ToolExample>,
    pub output_schema: Value,
    #[serde(default)]
    pub runtime_input_policy: ToolRuntimeInputPolicy,
    #[serde(default = "crate::empty_object")]
    pub driver_config: Value,
    pub idempotency: IdempotencyMode,
    pub evidence_type: Option<EvidenceType>,
    pub created_at: String,
}

impl Default for ToolDescriptor {
    fn default() -> Self {
        Self {
            tool_id: String::new(),
            name: String::new(),
            description: String::new(),
            version: String::new(),
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 0,
            input_schema: crate::empty_object(),
            model_input_schema: None,
            examples: Vec::new(),
            output_schema: crate::empty_object(),
            runtime_input_policy: ToolRuntimeInputPolicy::default(),
            driver_config: crate::empty_object(),
            idempotency: IdempotencyMode::None,
            evidence_type: None,
            created_at: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolRuntimeInputPolicy {
    #[serde(default)]
    pub injected_fields: BTreeMap<String, String>,
    #[serde(default)]
    pub required_resource_scopes: Vec<String>,
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
