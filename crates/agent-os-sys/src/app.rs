use crate::{
    AutomationScheduleKind, CredentialSource, LlmApiStyle, ModelCapabilities, ModelLimit,
    ResourceSessionType, SecurityLevel, ThreadStatus, TurnStatus,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionCursor {
    pub last_event_ordinal: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionCheckpoint {
    pub projection_name: String,
    pub last_event_ordinal: u64,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientConnection {
    pub client_id: String,
    pub client_name: String,
    pub client_kind: ClientKind,
    pub authority: SecurityLevel,
    pub connected_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientKind {
    Human,
    Automation,
    DesktopApp,
    TerminalUi,
    Ide,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientThread {
    pub client_thread_id: String,
    pub agent_thread_id: String,
    pub task_id: Option<String>,
    pub goal_id: Option<String>,
    pub title: String,
    pub status: ThreadStatus,
    pub active_turn_id: Option<String>,
    pub archived: bool,
    pub deleted: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnRecord {
    pub turn_id: String,
    pub client_thread_id: Option<String>,
    pub agent_thread_id: String,
    pub task_id: Option<String>,
    pub goal_id: Option<String>,
    pub status: TurnStatus,
    pub started_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnInputKind {
    Start,
    Steer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnInputRecord {
    pub input_id: String,
    pub client_thread_id: String,
    pub turn_id: String,
    pub submitted_by_client_id: String,
    pub kind: TurnInputKind,
    pub input: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimelineItemType {
    ThreadChanged,
    TurnStarted,
    TurnCompleted,
    ItemStarted,
    ItemCompleted,
    AgentMessageDelta,
    ToolUpdated,
    ApprovalRequested,
    ApprovalResolved,
    StatsUpdated,
    ArtifactIndexed,
    EvidenceIndexed,
    ResourceUpdated,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineItem {
    pub item_id: String,
    pub event_id: String,
    pub item_type: TimelineItemType,
    pub client_thread_id: Option<String>,
    pub agent_id: Option<String>,
    pub task_id: Option<String>,
    pub turn_id: Option<String>,
    pub summary: String,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsQuery {
    pub client_thread_id: Option<String>,
    pub agent_id: Option<String>,
    pub task_id: Option<String>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub tool_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StatsSnapshot {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub cost: f64,
    pub provider_calls: u64,
    pub provider_errors: u64,
    pub tool_calls: u64,
    pub tool_successes: u64,
    pub tool_failures: u64,
    pub tool_denials: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub approvals_requested: u64,
    pub approvals_resolved: u64,
    pub budget_debits: u64,
    pub latency_ms_total: u64,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApprovalQueueProjection {
    pub approval_id: String,
    pub client_thread_id: Option<String>,
    pub agent_id: String,
    pub task_id: String,
    pub status: String,
    pub requested_at: String,
    pub resolved_at: Option<String>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceSessionProjection {
    pub session_id: String,
    pub resource_type: String,
    pub client_thread_id: Option<String>,
    pub owner_agent_id: Option<String>,
    pub status: String,
    pub lease_expires_at: Option<String>,
    pub updated_at: String,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactIndexProjection {
    pub artifact_id: String,
    pub client_thread_id: Option<String>,
    pub agent_id: String,
    pub task_id: String,
    pub artifact_type: String,
    pub created_at: String,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceIndexProjection {
    pub evidence_id: String,
    pub client_thread_id: Option<String>,
    pub agent_id: String,
    pub task_id: String,
    pub evidence_type: String,
    pub created_at: String,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfigProjection {
    pub config_path: String,
    pub data_dir: String,
    pub state_dir: String,
    pub cache_dir: String,
    pub log_dir: String,
    pub project: Option<AppProjectProjection>,
    pub model: String,
    pub small_model: Option<String>,
    pub providers: Vec<AppProviderProjection>,
    pub global_config_recovery: Option<AppConfigRecoveryProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppProjectProjection {
    pub canonical_root: String,
    pub slug: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfigRecoveryProjection {
    pub primary_path: String,
    pub backup_path: String,
    pub primary_error: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppProviderProjection {
    pub provider_id: String,
    pub endpoint: LlmApiStyle,
    pub base_url: String,
    pub timeout_ms: Option<u64>,
    pub credential: AppCredentialProjection,
    pub models: Vec<AppModelProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppCredentialProjection {
    pub source: CredentialSource,
    pub name: String,
    pub redacted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppModelProjection {
    pub id: String,
    pub provider_id: String,
    pub model_id: String,
    pub provider_model_name: String,
    pub endpoint: LlmApiStyle,
    pub base_url: String,
    pub timeout_ms: Option<u64>,
    pub capabilities: ModelCapabilities,
    pub limit: ModelLimit,
    pub options: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppProviderCapabilitiesProjection {
    pub provider_id: String,
    pub models: Vec<AppModelProjection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppProviderUsageProjection {
    pub query: StatsQuery,
    pub snapshot: StatsSnapshot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppRequestEnvelope {
    pub protocol: String,
    pub request_id: String,
    pub client: ClientConnection,
    #[serde(flatten)]
    pub request: AppRequest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppResponseEnvelope {
    pub protocol: String,
    pub request_id: String,
    pub response: AppResponse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppNotificationEnvelope {
    pub protocol: String,
    pub subscription_id: Option<String>,
    pub cursor: ProjectionCursor,
    pub notification: AppNotification,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum AppRequest {
    #[serde(rename = "initialize")]
    Initialize,
    #[serde(rename = "thread/start")]
    ThreadStart {
        goal: String,
        workspace: Option<String>,
    },
    #[serde(rename = "thread/resume")]
    ThreadResume { client_thread_id: String },
    #[serde(rename = "thread/read")]
    ThreadRead { client_thread_id: String },
    #[serde(rename = "thread/list")]
    ThreadList { archived: Option<bool> },
    #[serde(rename = "thread/search")]
    ThreadSearch { query: String },
    #[serde(rename = "thread/archive")]
    ThreadArchive { client_thread_id: String },
    #[serde(rename = "thread/unarchive")]
    ThreadUnarchive { client_thread_id: String },
    #[serde(rename = "thread/delete")]
    ThreadDelete { client_thread_id: String },
    #[serde(rename = "thread/name/set")]
    ThreadNameSet {
        client_thread_id: String,
        title: String,
    },
    #[serde(rename = "task/bundle/export")]
    TaskBundleExport { client_thread_id: String },
    #[serde(rename = "turn/start")]
    TurnStart {
        client_thread_id: String,
        input: String,
    },
    #[serde(rename = "turn/steer")]
    TurnSteer { turn_id: String, input: String },
    #[serde(rename = "turn/interrupt")]
    TurnInterrupt { turn_id: String },
    #[serde(rename = "approval/respond")]
    ApprovalRespond { approval_id: String, approved: bool },
    #[serde(rename = "resource/session/open")]
    ResourceSessionOpen {
        resource_type: ResourceSessionType,
        client_thread_id: Option<String>,
        lease_expires_at: Option<String>,
        payload: Value,
    },
    #[serde(rename = "resource/session/close")]
    ResourceSessionClose { session_id: String },
    #[serde(rename = "automation/schedule/create")]
    AutomationScheduleCreate {
        name: String,
        kind: AutomationScheduleKind,
        target_thread_id: Option<String>,
        workspace: Option<String>,
        prompt: String,
        next_run_at: Option<String>,
        interval_seconds: Option<u64>,
        payload: Value,
    },
    #[serde(rename = "automation/schedule/list")]
    AutomationScheduleList,
    #[serde(rename = "automation/run/list")]
    AutomationRunList { schedule_id: Option<String> },
    #[serde(rename = "stats/read")]
    StatsRead { query: StatsQuery },
    #[serde(rename = "config/read")]
    ConfigRead { workspace: Option<String> },
    #[serde(rename = "model/list")]
    ModelList { workspace: Option<String> },
    #[serde(rename = "provider/capabilities/read")]
    ProviderCapabilitiesRead {
        workspace: Option<String>,
        provider_id: Option<String>,
    },
    #[serde(rename = "provider/usage/read")]
    ProviderUsageRead { query: StatsQuery },
    #[serde(rename = "permission_profile/list")]
    PermissionProfileList,
    #[serde(rename = "subscribe")]
    Subscribe { cursor: Option<ProjectionCursor> },
    #[serde(rename = "unsubscribe")]
    Unsubscribe { subscription_id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", content = "body")]
pub enum AppResponse {
    Accepted(Value),
    Rejected { code: String, message: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum AppNotification {
    #[serde(rename = "thread_changed")]
    ThreadChanged(ClientThread),
    #[serde(rename = "turn_started")]
    TurnStarted(TurnRecord),
    #[serde(rename = "turn_completed")]
    TurnCompleted(TurnRecord),
    #[serde(rename = "item_started")]
    ItemStarted(TimelineItem),
    #[serde(rename = "item_completed")]
    ItemCompleted(TimelineItem),
    #[serde(rename = "agent_message_delta")]
    AgentMessageDelta(TimelineItem),
    #[serde(rename = "tool_update")]
    ToolUpdate(TimelineItem),
    #[serde(rename = "approval_requested")]
    ApprovalRequested(ApprovalQueueProjection),
    #[serde(rename = "approval_resolved")]
    ApprovalResolved(ApprovalQueueProjection),
    #[serde(rename = "stats_updated")]
    StatsUpdated(StatsSnapshot),
    #[serde(rename = "artifact_indexed")]
    ArtifactIndexed(ArtifactIndexProjection),
    #[serde(rename = "evidence_indexed")]
    EvidenceIndexed(EvidenceIndexProjection),
    #[serde(rename = "resource_updated")]
    ResourceUpdated(ResourceSessionProjection),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_protocol_version;
    use crate::SecurityLevel;
    use serde_json::json;

    #[test]
    fn app_request_envelope_uses_protocol_method_names_and_client_identity() {
        let envelope = AppRequestEnvelope {
            protocol: app_protocol_version(),
            request_id: "req_1".to_string(),
            client: human_client(),
            request: AppRequest::StatsRead {
                query: StatsQuery {
                    client_thread_id: Some("thread_1".to_string()),
                    ..StatsQuery::default()
                },
            },
        };

        let encoded = serde_json::to_value(&envelope).unwrap();

        assert_eq!(encoded["request_id"], "req_1");
        assert_eq!(encoded["protocol"], "agent-os.app.v1");
        assert_eq!(encoded["client"]["client_id"], "human_1");
        assert_eq!(encoded["method"], "stats/read");
        assert_eq!(
            encoded["params"]["query"]["client_thread_id"],
            json!("thread_1")
        );
    }

    #[test]
    fn automation_schedule_request_uses_protocol_method_name() {
        let envelope = AppRequestEnvelope {
            protocol: app_protocol_version(),
            request_id: "req_auto_1".to_string(),
            client: human_client(),
            request: AppRequest::AutomationScheduleCreate {
                name: "wake thread".to_string(),
                kind: AutomationScheduleKind::ThreadWakeup,
                target_thread_id: Some("thread_1".to_string()),
                workspace: None,
                prompt: "continue".to_string(),
                next_run_at: Some("2026-06-30T00:00:00Z".to_string()),
                interval_seconds: None,
                payload: json!({"source": "test"}),
            },
        };

        let encoded = serde_json::to_value(&envelope).unwrap();

        assert_eq!(encoded["method"], "automation/schedule/create");
        assert_eq!(encoded["params"]["kind"], "thread_wakeup");
        assert_eq!(encoded["params"]["target_thread_id"], "thread_1");
    }

    #[test]
    fn task_bundle_export_request_uses_protocol_method_name() {
        let envelope = AppRequestEnvelope {
            protocol: app_protocol_version(),
            request_id: "req_bundle_1".to_string(),
            client: human_client(),
            request: AppRequest::TaskBundleExport {
                client_thread_id: "thread_1".to_string(),
            },
        };

        let encoded = serde_json::to_value(&envelope).unwrap();

        assert_eq!(encoded["method"], "task/bundle/export");
        assert_eq!(encoded["params"]["client_thread_id"], "thread_1");
    }

    #[test]
    fn initialize_request_takes_client_identity_from_envelope() {
        let decoded: AppRequestEnvelope = serde_json::from_value(json!({
            "protocol": "agent-os.app.v1",
            "request_id": "req_init",
            "client": human_client(),
            "method": "initialize"
        }))
        .unwrap();

        assert!(matches!(decoded.request, AppRequest::Initialize));
        assert_eq!(decoded.client.authority, SecurityLevel::HUMAN_ROOT);
    }

    #[test]
    fn notification_envelope_uses_stable_type_names_and_cursor() {
        let envelope = AppNotificationEnvelope {
            protocol: app_protocol_version(),
            subscription_id: Some("sub_1".to_string()),
            cursor: ProjectionCursor {
                last_event_ordinal: 7,
            },
            notification: AppNotification::StatsUpdated(StatsSnapshot {
                provider_calls: 2,
                ..StatsSnapshot::default()
            }),
        };

        let encoded = serde_json::to_value(&envelope).unwrap();

        assert_eq!(encoded["protocol"], "agent-os.app.v1");
        assert_eq!(encoded["subscription_id"], "sub_1");
        assert_eq!(encoded["cursor"]["last_event_ordinal"], 7);
        assert_eq!(encoded["notification"]["type"], "stats_updated");
        assert_eq!(encoded["notification"]["payload"]["provider_calls"], 2);
    }

    fn human_client() -> ClientConnection {
        ClientConnection {
            client_id: "human_1".to_string(),
            client_name: "Terminal".to_string(),
            client_kind: ClientKind::TerminalUi,
            authority: SecurityLevel::HUMAN_ROOT,
            connected_at: "2026-06-30T00:00:00Z".to_string(),
        }
    }
}
