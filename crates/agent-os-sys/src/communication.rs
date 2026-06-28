use crate::ProfileStatus;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRoute {
    Supervisor,
    Blackboard,
    Human,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum MessageDeliveryStatus {
    Pending,
    Delivered,
    Rejected,
    Deferred,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CommunicationScope {
    None,
    Task,
    Goal,
    Global,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DirectRoutePolicy {
    pub enabled: bool,
    pub allowed_message_types: Vec<String>,
    pub trigger_turn: bool,
    pub rate_limit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackboardRoutePolicy {
    pub enabled: bool,
    pub allowed_scopes: CommunicationScope,
    pub allowed_channels: Vec<String>,
    pub allowed_entry_types: Vec<String>,
    pub broadcast: bool,
    pub requires_review: bool,
}

impl Default for BlackboardRoutePolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            allowed_scopes: CommunicationScope::None,
            allowed_channels: Vec::new(),
            allowed_entry_types: Vec::new(),
            broadcast: false,
            requires_review: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HumanRoutePolicy {
    pub enabled: bool,
    pub allowed_message_types: Vec<String>,
    pub requires_supervisor_approval: bool,
    pub attention_budget: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CompletionRoutePolicy {
    pub required_report: bool,
    pub allowed_artifact_refs: bool,
    pub allowed_evidence_refs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationProfile {
    pub communication_profile_id: String,
    pub agent_id: String,
    pub thread_id: String,
    pub status: ProfileStatus,
    pub supervisor: DirectRoutePolicy,
    pub blackboard: BlackboardRoutePolicy,
    pub human: HumanRoutePolicy,
    pub completion: CompletionRoutePolicy,
    pub created_at: String,
    pub updated_at: String,
    pub superseded_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub message_id: String,
    pub message_type: String,
    pub route: MessageRoute,
    pub source_agent_id: String,
    pub source_thread_id: String,
    pub target_agent_id: Option<String>,
    pub target_thread_id: Option<String>,
    pub channel_id: Option<String>,
    pub goal_id: String,
    pub task_id: String,
    pub risk_level: u8,
    pub trigger_turn: bool,
    pub requires_review: bool,
    pub payload: Value,
    pub artifact_refs: Vec<String>,
    pub evidence_refs: Vec<String>,
    pub delivery_status: MessageDeliveryStatus,
    pub rejected_reason: Option<String>,
    pub created_at: String,
    pub delivered_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackboardChannel {
    pub channel_id: String,
    pub scope: CommunicationScope,
    pub name: String,
    pub allowed_entry_types: Vec<String>,
    pub subscriber_agent_ids: Vec<String>,
    pub requires_review: bool,
    pub created_at: String,
    pub archived_at: Option<String>,
}
