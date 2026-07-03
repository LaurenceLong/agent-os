use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const APP_PROTOCOL_VERSION: &str = "agent-os.app.v1";

pub fn app_protocol_version() -> String {
    APP_PROTOCOL_VERSION.to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppProtocolSpec {
    pub version: String,
    pub transport: AppProtocolTransport,
    pub request_methods: Vec<AppMethodSpec>,
    pub notification_types: Vec<AppNotificationSpec>,
    pub json_schema: Value,
    pub typescript: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppProtocolTransport {
    pub name: String,
    pub framing: String,
    pub request_envelope: String,
    pub response_envelope: String,
    pub notification_envelope: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppMethodSpec {
    pub method: String,
    pub family: AppProtocolFamily,
    pub authority: AppProtocolAuthority,
    pub lifecycle: AppMethodLifecycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppMethodLifecycle {
    Implemented,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppNotificationSpec {
    pub notification_type: String,
    pub family: AppProtocolFamily,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppProtocolAuthority {
    HumanRoot,
    ClientSession,
    KernelProjection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppProtocolFamily {
    Core,
    Thread,
    Task,
    Turn,
    Approval,
    Resource,
    Automation,
    Stats,
    Config,
    Model,
    Provider,
    Permission,
    Subscription,
    Artifact,
    Evidence,
}

pub fn app_protocol_spec() -> AppProtocolSpec {
    AppProtocolSpec {
        version: app_protocol_version(),
        transport: AppProtocolTransport {
            name: "stdio-jsonl".to_string(),
            framing: "one JSON object per line".to_string(),
            request_envelope: "AppRequestEnvelope".to_string(),
            response_envelope: "AppResponseEnvelope".to_string(),
            notification_envelope: "AppNotificationEnvelope".to_string(),
        },
        request_methods: app_protocol_request_methods(),
        notification_types: app_protocol_notification_types(),
        json_schema: app_protocol_json_schema(),
        typescript: app_protocol_typescript(),
    }
}

pub fn app_protocol_request_methods() -> Vec<AppMethodSpec> {
    use AppMethodLifecycle::Implemented;
    use AppProtocolAuthority::{ClientSession, HumanRoot};
    use AppProtocolFamily::{
        Approval, Automation, Config, Core, Model, Permission, Provider, Resource, Stats,
        Subscription, Task, Thread, Turn,
    };

    vec![
        method("initialize", Core, HumanRoot, Implemented),
        method("thread/start", Thread, ClientSession, Implemented),
        method("thread/resume", Thread, ClientSession, Implemented),
        method("thread/read", Thread, ClientSession, Implemented),
        method("thread/turns/read", Thread, ClientSession, Implemented),
        method("thread/items/read", Thread, ClientSession, Implemented),
        method("thread/fork", Thread, ClientSession, Implemented),
        method("thread/rollback", Thread, ClientSession, Implemented),
        method("thread/compact", Thread, ClientSession, Implemented),
        method("thread/list", Thread, ClientSession, Implemented),
        method("thread/search", Thread, ClientSession, Implemented),
        method("thread/archive", Thread, ClientSession, Implemented),
        method("thread/unarchive", Thread, ClientSession, Implemented),
        method("thread/delete", Thread, ClientSession, Implemented),
        method("thread/name/set", Thread, ClientSession, Implemented),
        method("task/bundle/export", Task, ClientSession, Implemented),
        method("turn/start", Turn, ClientSession, Implemented),
        method("turn/steer", Turn, ClientSession, Implemented),
        method("turn/interrupt", Turn, ClientSession, Implemented),
        method("approval/respond", Approval, ClientSession, Implemented),
        method(
            "resource/session/open",
            Resource,
            ClientSession,
            Implemented,
        ),
        method(
            "resource/session/close",
            Resource,
            ClientSession,
            Implemented,
        ),
        method(
            "automation/schedule/create",
            Automation,
            ClientSession,
            Implemented,
        ),
        method(
            "automation/schedule/list",
            Automation,
            ClientSession,
            Implemented,
        ),
        method(
            "automation/run/list",
            Automation,
            ClientSession,
            Implemented,
        ),
        method("stats/read", Stats, ClientSession, Implemented),
        method("config/read", Config, ClientSession, Implemented),
        method("model/list", Model, ClientSession, Implemented),
        method(
            "provider/capabilities/read",
            Provider,
            ClientSession,
            Implemented,
        ),
        method("provider/usage/read", Provider, ClientSession, Implemented),
        method(
            "permission_profile/list",
            Permission,
            ClientSession,
            Implemented,
        ),
        method("subscribe", Subscription, ClientSession, Implemented),
        method("unsubscribe", Subscription, ClientSession, Implemented),
    ]
}

pub fn app_protocol_notification_types() -> Vec<AppNotificationSpec> {
    use AppProtocolFamily::{Approval, Artifact, Evidence, Resource, Stats, Thread, Turn};

    vec![
        notification("thread_changed", Thread),
        notification("turn_started", Turn),
        notification("turn_completed", Turn),
        notification("item_started", Turn),
        notification("item_completed", Turn),
        notification("agent_message_delta", Turn),
        notification("tool_update", Turn),
        notification("approval_requested", Approval),
        notification("approval_resolved", Approval),
        notification("stats_updated", Stats),
        notification("artifact_indexed", Artifact),
        notification("evidence_indexed", Evidence),
        notification("resource_updated", Resource),
    ]
}

pub fn app_protocol_json_schema() -> Value {
    let request_methods = app_protocol_request_methods()
        .into_iter()
        .map(|spec| spec.method)
        .collect::<Vec<_>>();
    let notification_types = app_protocol_notification_types()
        .into_iter()
        .map(|spec| spec.notification_type)
        .collect::<Vec<_>>();

    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://agent-os.dev/schemas/app-protocol-v1.json",
        "title": "Agent-OS App Protocol v1",
        "type": "object",
        "required": ["protocol"],
        "properties": {
            "protocol": {
                "const": APP_PROTOCOL_VERSION
            },
            "request": {
                "type": "object",
                "required": ["protocol", "request_id", "client", "method"],
                "properties": {
                    "protocol": { "const": APP_PROTOCOL_VERSION },
                    "request_id": { "type": "string", "minLength": 1 },
                    "client": { "type": "object" },
                    "method": { "type": "string", "enum": request_methods },
                    "params": { "type": "object" }
                },
                "additionalProperties": true
            },
            "response": {
                "type": "object",
                "required": ["protocol", "request_id", "response"],
                "properties": {
                    "protocol": { "const": APP_PROTOCOL_VERSION },
                    "request_id": { "type": "string" },
                    "response": { "type": "object" }
                },
                "additionalProperties": false
            },
            "notification": {
                "type": "object",
                "required": ["protocol", "cursor", "notification"],
                "properties": {
                    "protocol": { "const": APP_PROTOCOL_VERSION },
                    "subscription_id": { "type": ["string", "null"] },
                    "cursor": { "type": "object" },
                    "notification": {
                        "type": "object",
                        "required": ["type"],
                        "properties": {
                            "type": { "type": "string", "enum": notification_types },
                            "payload": {}
                        },
                        "additionalProperties": true
                    }
                },
                "additionalProperties": false
            }
        },
        "additionalProperties": false
    })
}

pub fn app_protocol_typescript() -> String {
    let methods = app_protocol_request_methods()
        .into_iter()
        .map(|spec| format!("  | {:?}", spec.method))
        .collect::<Vec<_>>()
        .join("\n");
    let notification_types = app_protocol_notification_types()
        .into_iter()
        .map(|spec| format!("  | {:?}", spec.notification_type))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "export const APP_PROTOCOL_VERSION = {:?};\n\n\
         export type AppMethod =\n{};\n\n\
         export type AppNotificationType =\n{};\n\n\
         export interface AppRequestEnvelope {{\n\
           protocol: typeof APP_PROTOCOL_VERSION;\n\
           request_id: string;\n\
           client: unknown;\n\
           method: AppMethod;\n\
           params?: Record<string, unknown>;\n\
         }}\n\n\
         export interface AppResponseEnvelope {{\n\
           protocol: typeof APP_PROTOCOL_VERSION;\n\
           request_id: string;\n\
           response: unknown;\n\
         }}\n\n\
         export interface AppNotificationEnvelope {{\n\
           protocol: typeof APP_PROTOCOL_VERSION;\n\
           subscription_id?: string | null;\n\
           cursor: unknown;\n\
           notification: {{ type: AppNotificationType; payload?: unknown }};\n\
         }}\n",
        APP_PROTOCOL_VERSION, methods, notification_types
    )
}

fn method(
    method: &str,
    family: AppProtocolFamily,
    authority: AppProtocolAuthority,
    lifecycle: AppMethodLifecycle,
) -> AppMethodSpec {
    AppMethodSpec {
        method: method.to_string(),
        family,
        authority,
        lifecycle,
    }
}

fn notification(notification_type: &str, family: AppProtocolFamily) -> AppNotificationSpec {
    AppNotificationSpec {
        notification_type: notification_type.to_string(),
        family,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn request_method_catalog_has_no_duplicates() {
        let methods = app_protocol_request_methods();
        let unique = methods
            .iter()
            .map(|spec| spec.method.as_str())
            .collect::<HashSet<_>>();

        assert_eq!(methods.len(), unique.len());
    }

    #[test]
    fn protocol_export_carries_current_agent_os_methods() {
        let spec = app_protocol_spec();
        let methods = spec
            .request_methods
            .iter()
            .map(|method| method.method.as_str())
            .collect::<HashSet<_>>();

        for method in [
            "thread/start",
            "model/list",
            "provider/capabilities/read",
            "permission_profile/list",
            "subscribe",
        ] {
            assert!(methods.contains(method), "missing method {method}");
        }
    }

    #[test]
    fn typescript_export_uses_protocol_literal() {
        let typescript = app_protocol_typescript();

        assert!(typescript.contains("agent-os.app.v1"));
        assert!(typescript.contains("export type AppMethod"));
        assert!(typescript.contains("\"thread/start\""));
        assert!(typescript.contains("\"provider/capabilities/read\""));
    }
}
