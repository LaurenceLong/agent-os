use crate::ProfileStatus;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialRef {
    pub credential_ref_id: String,
    pub source: CredentialSource,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialSource {
    LocalConfig,
    SecretStore,
    WorkerScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProfile {
    pub provider_profile_id: String,
    pub status: ProfileStatus,
    pub name: String,
    pub default_provider_id: Option<String>,
    pub default_model_alias: Option<String>,
    pub routing_policy_id: String,
    pub allowed_model_aliases: Vec<String>,
    pub credential_ref: CredentialRef,
    pub retry_policy: Option<Value>,
    pub transform_policy: Option<Value>,
    pub reasoning_defaults: Value,
    pub tool_visibility_profile: Option<String>,
    pub timeout_ms: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub created_at: String,
    pub updated_at: String,
    pub superseded_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelAlias {
    pub model_alias_id: String,
    pub alias: String,
    pub provider_id: String,
    pub provider_model_name: String,
    pub capabilities: Value,
    pub limits: Value,
    pub cost: Value,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingPolicy {
    pub routing_policy_id: String,
    pub status: ProfileStatus,
    pub name: String,
    pub rules: Vec<Value>,
    pub created_at: String,
    pub updated_at: String,
    pub superseded_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamRequest {
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub provider_profile_id: String,
    pub model_routing_policy_id: String,
    pub requested_model_alias: Option<String>,
    pub role: String,
    pub task_id: String,
    pub reasoning_profile: Option<String>,
    pub tool_visibility_profile: Option<String>,
    pub output_schema: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRouteDecision {
    pub provider_profile_id: String,
    pub routing_policy_id: String,
    pub requested_model_alias: Option<String>,
    pub selected_model_alias: String,
    pub provider_id: String,
    pub provider_model_name: String,
    pub credential_ref_id: String,
    pub resolved_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderStreamStatus {
    Open,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderStreamEventType {
    StreamStarted,
    ReasoningStarted,
    ReasoningDelta,
    ReasoningCompleted,
    OutputTextStarted,
    OutputTextDelta,
    OutputTextCompleted,
    ToolCallProposed,
    ToolCallCompleted,
    UsageUpdated,
    ProviderWarning,
    ProviderRetry,
    StreamCompleted,
    StreamFailed,
    StreamCancelled,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProviderUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStreamEvent {
    pub event_id: String,
    pub session_id: String,
    pub event_type: ProviderStreamEventType,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStreamSession {
    pub session_id: String,
    pub request: StreamRequest,
    pub route_decision: ProviderRouteDecision,
    pub provider_slot_lease_id: String,
    pub status: ProviderStreamStatus,
    pub stream_events: Vec<ProviderStreamEvent>,
    pub usage: ProviderUsage,
    pub created_at: String,
    pub completed_at: Option<String>,
}
