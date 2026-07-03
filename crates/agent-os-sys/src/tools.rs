use crate::EvidenceType;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const DEFAULT_TOOL_FOREGROUND_TIMEOUT_MS: u64 = 15_000;
pub const TOOL_OUTPUT_DEFAULT_NEW_LINES: u64 = 200;
pub const TOOL_OUTPUT_DEFAULT_PAGE_LINES: u64 = 200;
pub const TOOL_OUTPUT_MAX_LINES: u64 = 1000;
pub const TOOL_OUTPUT_MAX_WINDOW_BYTES: u64 = 8_000;

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolBackgroundExecution {
    #[default]
    KernelWorker,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolOutputDefaultMode {
    #[default]
    New,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolOutputManagementMode {
    #[default]
    ManagedTextFields,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolOutputManagementPolicy {
    pub mode: ToolOutputManagementMode,
    pub default_mode: ToolOutputDefaultMode,
    pub default_new_lines: u64,
    pub default_page_lines: u64,
    pub max_lines: u64,
    pub max_window_bytes: u64,
}

impl Default for ToolOutputManagementPolicy {
    fn default() -> Self {
        Self {
            mode: ToolOutputManagementMode::ManagedTextFields,
            default_mode: ToolOutputDefaultMode::New,
            default_new_lines: TOOL_OUTPUT_DEFAULT_NEW_LINES,
            default_page_lines: TOOL_OUTPUT_DEFAULT_PAGE_LINES,
            max_lines: TOOL_OUTPUT_MAX_LINES,
            max_window_bytes: TOOL_OUTPUT_MAX_WINDOW_BYTES,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolRecoveryPolicy {
    #[default]
    CancelOrphanRunning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolLifecyclePolicy {
    pub foreground_timeout_ms: u64,
    pub background_execution: ToolBackgroundExecution,
    pub output_management: ToolOutputManagementPolicy,
    pub recovery: ToolRecoveryPolicy,
}

impl Default for ToolLifecyclePolicy {
    fn default() -> Self {
        Self {
            foreground_timeout_ms: DEFAULT_TOOL_FOREGROUND_TIMEOUT_MS,
            background_execution: ToolBackgroundExecution::KernelWorker,
            output_management: ToolOutputManagementPolicy::default(),
            recovery: ToolRecoveryPolicy::CancelOrphanRunning,
        }
    }
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
    #[serde(default)]
    pub lifecycle: ToolLifecyclePolicy,
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
            lifecycle: ToolLifecyclePolicy::default(),
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
