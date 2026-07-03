use agent_os_sys::{
    app_protocol_json_schema, app_protocol_spec, app_protocol_typescript, app_protocol_version,
    AppMethodLifecycle, AppNotification, AppNotificationEnvelope, AppRequest, AppRequestEnvelope,
    AppResponse, AppResponseEnvelope, ClientConnection, ClientKind, ProjectionCursor,
    SecurityLevel, StatsQuery, StatsSnapshot,
};
use serde_json::json;
use std::collections::HashSet;

#[test]
fn app_protocol_envelopes_are_explicitly_versioned() {
    let request = AppRequestEnvelope {
        protocol: app_protocol_version(),
        request_id: "req_protocol_1".to_string(),
        client: human_client(),
        request: AppRequest::StatsRead {
            query: StatsQuery::default(),
        },
    };
    let response = AppResponseEnvelope {
        protocol: app_protocol_version(),
        request_id: "req_protocol_1".to_string(),
        response: AppResponse::Accepted(json!({"ok": true})),
    };
    let notification = AppNotificationEnvelope {
        protocol: app_protocol_version(),
        subscription_id: Some("sub_1".to_string()),
        cursor: ProjectionCursor {
            last_event_ordinal: 1,
        },
        notification: AppNotification::StatsUpdated(StatsSnapshot {
            provider_calls: 1,
            ..StatsSnapshot::default()
        }),
    };

    assert_eq!(
        serde_json::to_value(request).unwrap()["protocol"],
        "agent-os.app.v1"
    );
    assert_eq!(
        serde_json::to_value(response).unwrap()["protocol"],
        "agent-os.app.v1"
    );
    assert_eq!(
        serde_json::to_value(notification).unwrap()["protocol"],
        "agent-os.app.v1"
    );
}

#[test]
fn app_protocol_export_freezes_current_agent_os_method_families() {
    let spec = app_protocol_spec();
    let methods = spec
        .request_methods
        .iter()
        .map(|method| method.method.as_str())
        .collect::<HashSet<_>>();

    assert_eq!(spec.version, "agent-os.app.v1");
    for implemented in [
        "initialize",
        "thread/start",
        "thread/read",
        "thread/turns/read",
        "thread/items/read",
        "thread/fork",
        "thread/rollback",
        "thread/compact",
        "thread/list",
        "thread/search",
        "thread/archive",
        "thread/unarchive",
        "thread/delete",
        "thread/name/set",
        "task/bundle/export",
        "turn/start",
        "turn/steer",
        "turn/interrupt",
        "approval/respond",
        "resource/session/open",
        "resource/session/close",
        "process/stop",
        "process/kill",
        "automation/schedule/create",
        "automation/schedule/list",
        "automation/run/list",
        "stats/read",
        "config/read",
        "model/list",
        "provider/capabilities/read",
        "provider/usage/read",
        "permission_profile/list",
        "subscribe",
        "unsubscribe",
    ] {
        assert!(
            spec.request_methods.iter().any(|method| {
                method.method == implemented && method.lifecycle == AppMethodLifecycle::Implemented
            }),
            "missing implemented method {implemented}"
        );
    }
    for absent in [
        "config/write",
        "config/batch",
        "skills/list",
        "plugin/install",
        "mcp/resource/read",
        "fs/watch",
        "command/exec",
        "thread/metadata/update",
        "thread/settings/update",
        "provider/status/read",
    ] {
        assert!(
            !methods.contains(absent),
            "premature method {absent} must not be exported"
        );
    }
}

#[test]
fn app_protocol_schema_and_typescript_exports_match_versioned_contract() {
    let schema = app_protocol_json_schema();
    let request_methods = schema["properties"]["request"]["properties"]["method"]["enum"]
        .as_array()
        .expect("request method enum");
    let notification_types = schema["properties"]["notification"]["properties"]["notification"]
        ["properties"]["type"]["enum"]
        .as_array()
        .expect("notification type enum");
    let typescript = app_protocol_typescript();

    assert_eq!(schema["properties"]["protocol"]["const"], "agent-os.app.v1");
    assert!(request_methods
        .iter()
        .any(|method| method == "thread/start"));
    assert!(request_methods
        .iter()
        .any(|method| method == "provider/capabilities/read"));
    assert!(request_methods
        .iter()
        .any(|method| method == "process/kill"));
    assert!(notification_types
        .iter()
        .any(|kind| kind == "stats_updated"));
    assert!(notification_types
        .iter()
        .any(|kind| kind == "resource_updated"));
    assert!(typescript.contains("export const APP_PROTOCOL_VERSION"));
    assert!(typescript.contains("\"agent-os.app.v1\""));
    assert!(typescript.contains("\"provider/capabilities/read\""));
}

fn human_client() -> ClientConnection {
    ClientConnection {
        client_id: "human_1".to_string(),
        client_name: "Conformance".to_string(),
        client_kind: ClientKind::TerminalUi,
        authority: SecurityLevel::HUMAN_ROOT,
        connected_at: "2026-07-03T00:00:00Z".to_string(),
    }
}
