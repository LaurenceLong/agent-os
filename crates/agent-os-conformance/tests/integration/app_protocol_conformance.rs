use agent_os_app_server::AppServer;
use agent_os_config::AGENT_OS_HOME_ENV;
use agent_os_host::AgentOsHost;
use agent_os_sys::{
    app_protocol_json_schema, app_protocol_spec, app_protocol_typescript, app_protocol_version,
    AppMethodLifecycle, AppNotification, AppNotificationEnvelope, AppRequest, AppRequestEnvelope,
    AppResponse, AppResponseEnvelope, ClientConnection, ClientKind, ProjectionCursor,
    SecurityLevel, StatsQuery, StatsSnapshot,
};
use serde_json::json;
use std::{
    collections::HashSet,
    env, fs,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

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
        "process/list",
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
        .any(|method| method == "process/list"));
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
    assert!(typescript.contains("\"process/list\""));
}

#[test]
fn app_server_jsonl_host_path_updates_kernel_projection_and_notifications() {
    let host = AgentOsHost::in_memory();
    let mut server = AppServer::new(host.clone());

    let initialized = jsonl_request(&mut server, "req_init", AppRequest::Initialize);
    assert_eq!(accepted_body(initialized)["client_id"], "human_1");

    let subscribed = jsonl_request(
        &mut server,
        "req_subscribe",
        AppRequest::Subscribe {
            cursor: Some(ProjectionCursor {
                last_event_ordinal: 0,
            }),
        },
    );
    let subscription_id = accepted_body(subscribed)["subscription_id"]
        .as_str()
        .unwrap()
        .to_string();

    let started = jsonl_request(
        &mut server,
        "req_thread_start",
        AppRequest::ThreadStart {
            goal: "exercise app server host conformance".to_string(),
            workspace: Some("D:/work/app-server-conformance".to_string()),
        },
    );
    let thread_id = accepted_body(started)["thread"]["client_thread_id"]
        .as_str()
        .unwrap()
        .to_string();

    let turn_started = jsonl_request(
        &mut server,
        "req_turn_start",
        AppRequest::TurnStart {
            client_thread_id: thread_id.clone(),
            input: "start from the app protocol".to_string(),
        },
    );
    let turn_id = accepted_body(turn_started)["turn"]["turn_id"]
        .as_str()
        .unwrap()
        .to_string();

    let read = jsonl_request(
        &mut server,
        "req_thread_read",
        AppRequest::ThreadRead {
            client_thread_id: thread_id.clone(),
        },
    );
    let body = accepted_body(read);
    assert_eq!(body["thread"]["client_thread_id"], thread_id);
    assert!(body["turns"]
        .as_array()
        .unwrap()
        .iter()
        .any(|turn| turn["turn_id"] == turn_id));
    assert_eq!(body["runtime_jobs"].as_array().unwrap().len(), 1);

    let listed = jsonl_request(
        &mut server,
        "req_thread_list",
        AppRequest::ThreadList {
            archived: Some(false),
        },
    );
    assert!(accepted_body(listed)["threads"]
        .as_array()
        .unwrap()
        .iter()
        .any(|thread| thread["client_thread_id"] == thread_id));

    let notifications = server.drain_notifications().unwrap();
    assert!(notifications.iter().all(|notification| {
        notification.subscription_id.as_deref() == Some(subscription_id.as_str())
    }));
    assert!(notifications.iter().any(|notification| matches!(
        &notification.notification,
        AppNotification::ThreadChanged(thread) if thread.client_thread_id == thread_id
    )));
    assert!(notifications.iter().any(|notification| matches!(
        &notification.notification,
        AppNotification::TurnStarted(turn) if turn.turn_id == turn_id
    )));

    let state = host.kernel().state_snapshot().unwrap();
    assert!(state.threads.contains_key(&thread_id));
    assert!(host
        .kernel()
        .store()
        .turn_summaries()
        .unwrap()
        .iter()
        .any(|turn| turn.turn_id == turn_id));
}

#[test]
fn app_server_config_model_and_provider_projection_uses_config_crate_contract() {
    let _guard = config_env_lock().lock().unwrap();
    let previous_home = env::var_os(AGENT_OS_HOME_ENV);
    let root = isolated_temp_dir("app-config-provider");
    let home = root.join("home");
    let workspace = root.join("workspace");
    fs::create_dir_all(home.join("config")).unwrap();
    fs::create_dir_all(workspace.join(".agent-os")).unwrap();
    fs::write(
        home.join("config").join("config.json"),
        serde_json::to_string_pretty(&json!({
            "model": "openai/global-model",
            "small_model": "openai/global-model",
            "provider": {
                "openai": {
                    "api_key": "global-secret-key",
                    "endpoint": "openai_responses",
                    "options": {
                        "base_url": "https://provider.example/v1",
                        "timeout_ms": 45000
                    },
                    "models": {
                        "global-model": {
                            "name": "gpt-global",
                            "limit": {"context": 128000, "input": 120000, "output": 4096},
                            "capabilities": {
                                "streaming": true,
                                "tool_calling": true,
                                "reasoning": false,
                                "temperature": true,
                                "image_input": false,
                                "structured_output": true
                            }
                        }
                    }
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(
        workspace.join(".agent-os").join("config.json"),
        serde_json::to_string_pretty(&json!({
            "model": "openai/project-model",
            "provider": {
                "openai": {
                    "models": {
                        "project-model": {
                            "name": "gpt-project",
                            "options": {"effort": "medium"},
                            "limit": {"context": 64000, "input": 60000, "output": 2048},
                            "capabilities": {
                                "streaming": true,
                                "tool_calling": true,
                                "reasoning": true,
                                "temperature": false,
                                "image_input": true,
                                "structured_output": true
                            }
                        }
                    }
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();
    env::set_var(AGENT_OS_HOME_ENV, &home);

    let result = std::panic::catch_unwind(|| {
        let mut server = initialized_jsonl_server();
        let workspace = workspace.to_string_lossy().to_string();

        let config = accepted_body(jsonl_request(
            &mut server,
            "req_config_read",
            AppRequest::ConfigRead {
                workspace: Some(workspace.clone()),
            },
        ));
        assert_eq!(config["config"]["model"], "openai/project-model");
        assert_eq!(config["config"]["small_model"], "openai/global-model");
        assert_eq!(
            config["config"]["providers"][0]["credential"]["redacted"],
            true
        );
        assert_eq!(
            config["config"]["providers"][0]["credential"]["name"],
            "provider/openai/api_key"
        );
        assert!(!serde_json::to_string(&config)
            .unwrap()
            .contains("global-secret-key"));
        assert_eq!(
            config["config"]["providers"][0]["base_url"],
            "https://provider.example/v1"
        );
        assert_eq!(config["config"]["project"]["slug"], "workspace");

        let models = accepted_body(jsonl_request(
            &mut server,
            "req_model_list",
            AppRequest::ModelList {
                workspace: Some(workspace.clone()),
            },
        ));
        let model_ids = models["models"]
            .as_array()
            .unwrap()
            .iter()
            .map(|model| model["id"].as_str().unwrap())
            .collect::<HashSet<_>>();
        assert!(model_ids.contains("openai/global-model"));
        assert!(model_ids.contains("openai/project-model"));

        let capabilities = accepted_body(jsonl_request(
            &mut server,
            "req_provider_capabilities",
            AppRequest::ProviderCapabilitiesRead {
                workspace: Some(workspace),
                provider_id: Some("openai".to_string()),
            },
        ));
        let project_model = capabilities["providers"][0]["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|model| model["id"] == "openai/project-model")
            .unwrap();
        assert_eq!(project_model["provider_model_name"], "gpt-project");
        assert_eq!(project_model["endpoint"], "open_ai_responses");
        assert_eq!(project_model["limit"]["context"], 64000);
        assert_eq!(project_model["options"]["effort"], "medium");
        assert_eq!(project_model["capabilities"]["reasoning"], true);
        assert_eq!(project_model["capabilities"]["image_input"], true);

        let rejected = jsonl_request(
            &mut server,
            "req_missing_provider",
            AppRequest::ProviderCapabilitiesRead {
                workspace: None,
                provider_id: Some("missing".to_string()),
            },
        );
        assert!(matches!(
            rejected.response,
            AppResponse::Rejected { code, .. } if code == "not_found"
        ));
    });

    restore_agent_os_home(previous_home);
    fs::remove_dir_all(root).unwrap();
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
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

fn initialized_jsonl_server() -> AppServer<AgentOsHost> {
    let mut server = AppServer::new(AgentOsHost::in_memory());
    let initialized = jsonl_request(&mut server, "req_init", AppRequest::Initialize);
    assert!(matches!(initialized.response, AppResponse::Accepted(_)));
    server
}

fn jsonl_request(
    server: &mut AppServer<AgentOsHost>,
    request_id: &str,
    request: AppRequest,
) -> AppResponseEnvelope {
    let line = serde_json::to_string(&AppRequestEnvelope {
        protocol: app_protocol_version(),
        request_id: request_id.to_string(),
        client: human_client(),
        request,
    })
    .unwrap();
    let response = server.handle_line(&line);
    let envelope = serde_json::from_str::<AppResponseEnvelope>(&response).unwrap();
    assert_eq!(envelope.protocol, app_protocol_version());
    assert_eq!(envelope.request_id, request_id);
    envelope
}

fn accepted_body(envelope: AppResponseEnvelope) -> serde_json::Value {
    match envelope.response {
        AppResponse::Accepted(body) => body,
        AppResponse::Rejected { code, message } => {
            panic!("app request rejected: {code}: {message}");
        }
    }
}

fn config_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn isolated_temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    env::temp_dir().join(format!(
        "agent-os-conformance-{label}-{}-{unique}",
        std::process::id()
    ))
}

fn restore_agent_os_home(previous_home: Option<std::ffi::OsString>) {
    match previous_home {
        Some(value) => env::set_var(AGENT_OS_HOME_ENV, value),
        None => env::remove_var(AGENT_OS_HOME_ENV),
    }
}
