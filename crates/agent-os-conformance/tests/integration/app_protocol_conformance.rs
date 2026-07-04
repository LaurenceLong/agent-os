use agent_os_app_server::AppServer;
use agent_os_config::AGENT_OS_HOME_ENV;
use agent_os_host::AgentOsHost;
use agent_os_sys::{
    app_protocol_json_schema, app_protocol_spec, app_protocol_typescript, app_protocol_version,
    AgentOsResult, AppMethodLifecycle, AppNotification, AppNotificationEnvelope, AppRequest,
    AppRequestEnvelope, AppResponse, AppResponseEnvelope, AutomationScheduleKind, ClientConnection,
    ClientKind, EvidenceMapEntry, FinalSubmission, ProcessLifecycleState, ProjectionCursor,
    ProviderUsage, SecurityLevel, StatsQuery, StatsSnapshot, ThreadStatus,
};
use agent_os_thread::{ModelAction, ModelClient, ModelTurnRequest, ModelTurnResponse, ToolAction};
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

#[test]
fn app_server_process_stop_and_kill_cleanup_running_processes_through_kernel() {
    let stopped = app_cleanup_running_process("process-stop", ProcessCleanupAction::Stop);
    assert_eq!(stopped["state"], "interrupted");
    assert_eq!(stopped["error"], "conformance stop");

    let killed = app_cleanup_running_process("process-kill", ProcessCleanupAction::Kill);
    assert_eq!(killed["state"], "terminated");
    assert_eq!(killed["error"], "conformance kill");
}

#[test]
fn app_server_automation_schedule_and_run_projection_round_trips_through_store() {
    let host = AgentOsHost::in_memory();
    let mut server = AppServer::new(host.clone());
    let initialized = jsonl_request(&mut server, "req_init", AppRequest::Initialize);
    assert!(matches!(initialized.response, AppResponse::Accepted(_)));
    let root = isolated_temp_dir("automation-conformance");
    fs::create_dir_all(&root).unwrap();

    let started = accepted_body(jsonl_request(
        &mut server,
        "req_thread_start",
        AppRequest::ThreadStart {
            goal: "automation conformance target".to_string(),
            workspace: Some(root.to_string_lossy().to_string()),
        },
    ));
    let thread_id = started["thread"]["client_thread_id"]
        .as_str()
        .unwrap()
        .to_string();

    let created = accepted_body(jsonl_request(
        &mut server,
        "req_automation_create",
        AppRequest::AutomationScheduleCreate {
            name: "conformance wakeup".to_string(),
            kind: AutomationScheduleKind::ThreadWakeup,
            target_thread_id: Some(thread_id.clone()),
            workspace: None,
            prompt: "continue conformance automation".to_string(),
            next_run_at: Some("2026-06-30T00:00:00Z".to_string()),
            interval_seconds: Some(60),
            payload: json!({"source": "conformance"}),
        },
    ));
    let schedule_id = created["automation_schedule"]["schedule_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        created["automation_schedule"]["created_by_client_id"],
        "human_1"
    );
    assert_eq!(created["automation_schedule"]["status"], "active");

    let listed = accepted_body(jsonl_request(
        &mut server,
        "req_automation_schedule_list",
        AppRequest::AutomationScheduleList,
    ));
    assert!(listed["automation_schedules"]
        .as_array()
        .unwrap()
        .iter()
        .any(|schedule| schedule["schedule_id"] == schedule_id));

    let runs = host.run_due_automations_at("2026-06-30T00:00:01Z").unwrap();
    assert_eq!(runs.len(), 1);
    let run_id = runs[0].run_id.clone();
    assert_eq!(runs[0].schedule_id, schedule_id);
    assert_eq!(
        runs[0].target_thread_id.as_deref(),
        Some(thread_id.as_str())
    );

    let run_list = accepted_body(jsonl_request(
        &mut server,
        "req_automation_run_list",
        AppRequest::AutomationRunList {
            schedule_id: Some(schedule_id.clone()),
        },
    ));
    assert_eq!(run_list["automation_runs"][0]["run_id"], run_id);
    assert_eq!(run_list["automation_runs"][0]["status"], "queued");
    assert_eq!(
        run_list["automation_runs"][0]["payload"]["source"],
        "conformance"
    );

    let read = accepted_body(jsonl_request(
        &mut server,
        "req_thread_read_after_automation",
        AppRequest::ThreadRead {
            client_thread_id: thread_id.clone(),
        },
    ));
    assert!(read["automation_runs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|run| run["run_id"] == run_id));
    assert!(read["runtime_jobs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|job| job["job"]["client_thread_id"] == thread_id && job["status"] == "queued"));
    remove_dir_all_retry(&root);
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

#[derive(Clone, Copy)]
enum ProcessCleanupAction {
    Stop,
    Kill,
}

fn app_cleanup_running_process(label: &str, action: ProcessCleanupAction) -> serde_json::Value {
    let root = isolated_temp_dir(label);
    fs::create_dir_all(&root).unwrap();
    let host = AgentOsHost::in_memory();
    let mut server = AppServer::new(host.clone());
    let initialized = jsonl_request(&mut server, "req_init", AppRequest::Initialize);
    assert!(matches!(initialized.response, AppResponse::Accepted(_)));

    let started = accepted_body(jsonl_request(
        &mut server,
        "req_thread_start",
        AppRequest::ThreadStart {
            goal: format!("start {label} process"),
            workspace: Some(root.to_string_lossy().to_string()),
        },
    ));
    let thread_id = started["thread"]["client_thread_id"]
        .as_str()
        .unwrap()
        .to_string();
    jsonl_request(
        &mut server,
        "req_turn_start",
        AppRequest::TurnStart {
            client_thread_id: thread_id.clone(),
            input: "start a long-running process".to_string(),
        },
    );

    let report = host
        .run_next_runtime_job(LongRunningCommandModel {
            workspace: root.clone(),
            used: false,
        })
        .unwrap()
        .expect("queued runtime job");
    assert_eq!(report.status, ThreadStatus::WaitingTool);

    let read = accepted_body(jsonl_request(
        &mut server,
        "req_thread_read_running_process",
        AppRequest::ThreadRead {
            client_thread_id: thread_id,
        },
    ));
    let process_id = read["process_sessions"][0]["process_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(read["process_sessions"][0]["state"], "running");

    let running = accepted_body(jsonl_request(
        &mut server,
        "req_process_list_running",
        AppRequest::ProcessList {
            state: Some(ProcessLifecycleState::Running),
        },
    ));
    assert_eq!(running["process_sessions"][0]["process_id"], process_id);

    let cleaned = match action {
        ProcessCleanupAction::Stop => accepted_body(jsonl_request(
            &mut server,
            "req_process_stop",
            AppRequest::ProcessStop {
                process_id: process_id.clone(),
                reason: Some("conformance stop".to_string()),
            },
        )),
        ProcessCleanupAction::Kill => accepted_body(jsonl_request(
            &mut server,
            "req_process_kill",
            AppRequest::ProcessKill {
                process_id: process_id.clone(),
                reason: Some("conformance kill".to_string()),
            },
        )),
    };
    let process_session = cleaned["process_session"].clone();

    let after = accepted_body(jsonl_request(
        &mut server,
        "req_process_list_running_after_cleanup",
        AppRequest::ProcessList {
            state: Some(ProcessLifecycleState::Running),
        },
    ));
    assert!(after["process_sessions"].as_array().unwrap().is_empty());

    remove_dir_all_retry(&root);
    process_session
}

#[derive(Debug, Clone)]
struct LongRunningCommandModel {
    workspace: PathBuf,
    used: bool,
}

impl ModelClient for LongRunningCommandModel {
    fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
        if self.used {
            let evidence_map = request
                .context
                .tool_results
                .iter()
                .filter(|result| !result.evidence_ids.is_empty())
                .filter_map(|result| {
                    result
                        .evidence_claim
                        .as_ref()
                        .map(|claim| EvidenceMapEntry {
                            claim: claim.clone(),
                            evidence_refs: result.evidence_ids.clone(),
                        })
                })
                .collect();
            return Ok(ModelTurnResponse::single(ModelAction::Final {
                submission: FinalSubmission {
                    summary: "long process cleanup complete".to_string(),
                    changed_artifacts: Vec::new(),
                    evidence_map,
                    unverified_claims: Vec::new(),
                    known_risks: Vec::new(),
                    tests_run: Vec::new(),
                    tests_not_run: Vec::new(),
                    approvals: Vec::new(),
                },
            }));
        }
        self.used = true;
        let (command, args) = long_running_command();
        Ok(ModelTurnResponse {
            actions: vec![ModelAction::ToolCall(ToolAction::new(
                "run_command",
                json!({
                    "command": command,
                    "mode": "exec",
                    "args": args,
                    "cwd": self.workspace.to_string_lossy(),
                }),
                4,
                Some("long-running process started".to_string()),
            ))],
            usage: ProviderUsage::default(),
        })
    }
}

fn long_running_command() -> (String, Vec<String>) {
    if cfg!(windows) {
        (
            "powershell".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "Start-Sleep -Seconds 30".to_string(),
            ],
        )
    } else {
        (
            "sh".to_string(),
            vec!["-c".to_string(), "sleep 30".to_string()],
        )
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

fn remove_dir_all_retry(path: &std::path::Path) {
    for attempt in 0..20 {
        match fs::remove_dir_all(path) {
            Ok(()) => return,
            Err(error) if attempt < 19 => {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if !path.exists() {
                    return;
                }
                if error.kind() == std::io::ErrorKind::NotFound {
                    return;
                }
            }
            Err(error) => panic!("remove temp dir {}: {error}", path.display()),
        }
    }
}

fn restore_agent_os_home(previous_home: Option<std::ffi::OsString>) {
    match previous_home {
        Some(value) => env::set_var(AGENT_OS_HOME_ENV, value),
        None => env::remove_var(AGENT_OS_HOME_ENV),
    }
}
