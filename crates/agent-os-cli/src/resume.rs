use crate::args::ResumeOptions;
use crate::support::{
    default_state_db_for_workspace, ensure_safe_relative_workspace_path, io_result,
    write_task_bundle_from_app_response, StdioHostAppClient, StdioHostConfig,
};
use agent_os_sys::*;
use serde_json::{json, Value};
use std::fs;
use std::time::Duration;

const RESUME_RUNTIME_POLL_ATTEMPTS: usize = 480;
const RESUME_RUNTIME_POLL_INTERVAL: Duration = Duration::from_millis(250);

pub(crate) fn run_resume(options: &ResumeOptions) -> AgentOsResult<Value> {
    if let Some(bundle_output) = &options.bundle_output {
        ensure_safe_relative_workspace_path(bundle_output, "--bundle-output")?;
    }
    io_result(
        fs::create_dir_all(&options.workspace),
        "create workspace directory",
    )?;
    let state_db = options
        .state_db
        .clone()
        .map(Ok)
        .unwrap_or_else(|| default_state_db_for_workspace(&options.workspace))?;
    let mut config = StdioHostConfig::state_db(state_db.clone());
    config.model_command = Some(options.model_command.clone());
    config.model_args = options.model_args.clone();
    let mut app_client = StdioHostAppClient::open(&config)?;
    resume_from_app_client(&mut app_client, options, &state_db)
}

trait ResumeAppClient {
    fn request(&mut self, request: AppRequest) -> AgentOsResult<Value>;
}

impl ResumeAppClient for StdioHostAppClient {
    fn request(&mut self, request: AppRequest) -> AgentOsResult<Value> {
        StdioHostAppClient::request(self, request)
    }
}

fn resume_from_app_client(
    app_client: &mut impl ResumeAppClient,
    options: &ResumeOptions,
    state_db: &std::path::Path,
) -> AgentOsResult<Value> {
    app_client.request(AppRequest::Initialize)?;
    let resumed = app_client.request(AppRequest::ThreadResume {
        client_thread_id: options.thread_id.clone(),
    })?;
    let previous_thread_status = resumed["previous_thread_status"].clone();
    let reconciliation = resumed["reconciliation"].clone();
    let turn = app_client.request(AppRequest::TurnStart {
        client_thread_id: options.thread_id.clone(),
        input: "resume requested".to_string(),
    })?;
    let runtime_job_id = required_json_string(&turn["runtime_job"], "runtime_job_id")?;
    let thread = wait_for_runtime_job(app_client, &options.thread_id, &runtime_job_id)?;
    let stats = app_client.request(AppRequest::StatsRead {
        query: StatsQuery::default(),
    })?["snapshot"]
        .clone();
    let runtime_job = runtime_job_by_id(&thread, &runtime_job_id)?;
    let bundle_path = if options.bundle_output.is_some() {
        let exported = app_client.request(AppRequest::TaskBundleExport {
            client_thread_id: options.thread_id.clone(),
        })?;
        write_task_bundle_from_app_response(
            &options.workspace,
            &options.bundle_output,
            &exported["bundle"],
        )?
    } else {
        None
    };
    Ok(json!({
        "status": "completed",
        "state_db": state_db.to_string_lossy(),
        "thread_id": options.thread_id,
        "task_id": thread["thread"]["task_id"],
        "previous_thread_status": previous_thread_status,
        "runtime_status": thread["thread"]["status"],
        "runtime_job_status": runtime_job["status"],
        "bundle_path": bundle_path,
        "thread": thread["thread"],
        "turns": thread["turns"],
        "timeline": thread["timeline"],
        "runtime_jobs": thread["runtime_jobs"],
        "stats": stats,
        "reconciliation": reconciliation
    }))
}

fn wait_for_runtime_job(
    app_client: &mut impl ResumeAppClient,
    thread_id: &str,
    runtime_job_id: &str,
) -> AgentOsResult<Value> {
    for attempt in 0..RESUME_RUNTIME_POLL_ATTEMPTS {
        let thread = app_client.request(AppRequest::ThreadRead {
            client_thread_id: thread_id.to_string(),
        })?;
        let job = runtime_job_by_id(&thread, runtime_job_id)?;
        match job["status"].as_str() {
            Some("completed") => return Ok(thread),
            Some("failed") => {
                return Err(AgentOsError::Validation(format!(
                    "runtime job {runtime_job_id} failed: {}",
                    job["last_error"].as_str().unwrap_or("unknown error")
                )))
            }
            Some("blocked") => {
                return Err(AgentOsError::Validation(format!(
                    "runtime job {runtime_job_id} blocked: {}",
                    job["last_error"].as_str().unwrap_or("unknown reason")
                )))
            }
            Some("interrupted" | "cancelled") => {
                return Err(AgentOsError::InvalidTransition(format!(
                    "runtime job {runtime_job_id} ended as {}",
                    job["status"].as_str().unwrap_or("unknown")
                )))
            }
            Some("queued" | "running") => {}
            Some(status) => {
                return Err(AgentOsError::Validation(format!(
                    "runtime job {runtime_job_id} has unknown status {status}"
                )))
            }
            None => {
                return Err(AgentOsError::Validation(format!(
                    "runtime job {runtime_job_id} omitted status"
                )))
            }
        }
        if attempt + 1 < RESUME_RUNTIME_POLL_ATTEMPTS {
            std::thread::sleep(RESUME_RUNTIME_POLL_INTERVAL);
        }
    }
    Err(AgentOsError::Validation(format!(
        "runtime job {runtime_job_id} did not complete before resume timeout"
    )))
}

fn runtime_job_by_id<'a>(thread: &'a Value, runtime_job_id: &str) -> AgentOsResult<&'a Value> {
    thread["runtime_jobs"]
        .as_array()
        .and_then(|jobs| {
            jobs.iter()
                .find(|job| job["runtime_job_id"].as_str() == Some(runtime_job_id))
        })
        .ok_or_else(|| AgentOsError::NotFound(format!("runtime job {runtime_job_id}")))
}

#[cfg(test)]
fn resume_thread_recovery_from_host(
    state_db: &std::path::Path,
    thread_id: &str,
) -> AgentOsResult<Value> {
    use agent_os_host::{AgentOsHost, AppServer};

    let host = AgentOsHost::open_sqlite(state_db)?;
    let mut server = AppServer::new(host);
    let client = ClientConnection {
        client_id: "agent-os-cli-test".to_string(),
        client_name: "Agent-OS CLI Test".to_string(),
        client_kind: ClientKind::TerminalUi,
        authority: SecurityLevel::HUMAN_ROOT,
        connected_at: now_rfc3339(),
    };
    let response = server.handle_envelope(AppRequestEnvelope {
        request_id: new_id("req_"),
        client: client.clone(),
        request: AppRequest::Initialize,
    });
    assert!(matches!(response.response, AppResponse::Accepted(_)));
    let response = server.handle_envelope(AppRequestEnvelope {
        request_id: new_id("req_"),
        client,
        request: AppRequest::ThreadResume {
            client_thread_id: thread_id.to_string(),
        },
    });
    match response.response {
        AppResponse::Accepted(body) => Ok(body),
        AppResponse::Rejected { code, message } => Err(AgentOsError::Validation(format!(
            "app-server {code}: {message}"
        ))),
    }
}

fn required_json_string(object: &Value, field: &str) -> AgentOsResult<String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| AgentOsError::Validation(format!("app-server response omitted {field}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_os_kernel::{Kernel, RegisterGoalInput, SpawnAgentInput, SpawnTaskInput};
    use agent_os_store_sqlite::SqliteStore;
    use std::env;
    use std::path::PathBuf;

    #[test]
    fn resume_from_app_client_polls_projection_until_runtime_completed() {
        let workspace = PathBuf::from("workspace");
        let mut client = FakeResumeClient::default();

        let output = resume_from_app_client(
            &mut client,
            &ResumeOptions {
                state_db: Some(PathBuf::from("state.sqlite")),
                thread_id: "thread_1".to_string(),
                workspace,
                bundle_output: None,
                model_command: PathBuf::from("model.exe"),
                model_args: Vec::new(),
            },
            &PathBuf::from("state.sqlite"),
        )
        .unwrap();

        assert_eq!(output["status"], json!("completed"));
        assert_eq!(output["runtime_job_status"], json!("completed"));
        assert_eq!(output["runtime_status"], json!("Completed"));
        assert_eq!(
            client.requests,
            vec![
                "initialize",
                "thread/resume",
                "turn/start",
                "thread/read",
                "stats/read",
            ]
        );
    }

    #[test]
    fn resume_from_app_client_exports_bundle_when_requested() {
        let workspace = env::temp_dir().join(format!(
            "agent-os-cli-resume-bundle-{}-{}",
            std::process::id(),
            new_id("case_")
        ));
        fs::create_dir_all(&workspace).unwrap();
        let mut client = FakeResumeClient::default();

        let output = resume_from_app_client(
            &mut client,
            &ResumeOptions {
                state_db: Some(PathBuf::from("state.sqlite")),
                thread_id: "thread_1".to_string(),
                workspace: workspace.clone(),
                bundle_output: Some(PathBuf::from("bundle/resume.json")),
                model_command: PathBuf::from("model.exe"),
                model_args: Vec::new(),
            },
            &PathBuf::from("state.sqlite"),
        )
        .unwrap();

        let bundle_path = workspace.join("bundle/resume.json");
        assert_eq!(output["bundle_path"], json!(bundle_path.to_string_lossy()));
        let bundle: Value = serde_json::from_slice(&fs::read(&bundle_path).unwrap()).unwrap();
        assert_eq!(bundle["root_task_id"], "task_1");
        assert_eq!(
            client.requests,
            vec![
                "initialize",
                "thread/resume",
                "turn/start",
                "thread/read",
                "stats/read",
                "task/bundle/export",
            ]
        );
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn run_resume_rejects_bundle_output_path_escape() {
        let error = run_resume(&ResumeOptions {
            state_db: Some(PathBuf::from("state.sqlite")),
            thread_id: "thread_1".to_string(),
            workspace: PathBuf::from("."),
            bundle_output: Some(PathBuf::from("../resume-bundle.json")),
            model_command: PathBuf::from("model.exe"),
            model_args: Vec::new(),
        })
        .unwrap_err();

        assert!(error.to_string().contains("--bundle-output"));
    }

    #[test]
    fn prepare_thread_for_resume_recovers_running_thread_after_replay() {
        let state_db = env::temp_dir().join(format!(
            "agent-os-cli-resume-{}-{}.sqlite",
            std::process::id(),
            new_id("case_")
        ));
        let kernel = Kernel::with_replayed_store(SqliteStore::open(&state_db).unwrap()).unwrap();
        let goal = kernel
            .register_goal(RegisterGoalInput {
                namespace: "resume-test".to_string(),
                created_by: "agent-os-cli-test".to_string(),
                title: "Resume".to_string(),
                description: "Resume".to_string(),
                acceptance_criteria: vec!["thread resumes".to_string()],
                constraints: Vec::new(),
                risk_level: 1,
                deadline: None,
            })
            .unwrap();
        let task = kernel
            .spawn_task(SpawnTaskInput {
                goal_id: goal.goal_id,
                parent_task_id: None,
                title: "Task".to_string(),
                description: "Task".to_string(),
                depends_on: Vec::new(),
                required_artifact_types: Vec::new(),
                required_evidence_types: Vec::new(),
                priority: 1,
                risk_level: 1,
            })
            .unwrap();
        let agent = kernel
            .spawn_agent(SpawnAgentInput {
                task_id: task.task_id,
                role_profile_id: "role_producer".to_string(),
                owner: "agent-os-cli-test".to_string(),
                goal: "Resume".to_string(),
                success_criteria: Vec::new(),
                failure_criteria: Vec::new(),
                parent_thread_id: None,
                workspace_roots: vec![".".to_string()],
            })
            .unwrap();
        kernel.start_turn(&agent.thread_id).unwrap();

        let body = resume_thread_recovery_from_host(&state_db, &agent.thread_id).unwrap();
        assert_eq!(body["previous_thread_status"], json!("Running"));
        assert_eq!(body["thread"]["status"], json!("Ready"));
        let _ = std::fs::remove_file(state_db);
    }

    #[derive(Default)]
    struct FakeResumeClient {
        requests: Vec<&'static str>,
    }

    impl ResumeAppClient for FakeResumeClient {
        fn request(&mut self, request: AppRequest) -> AgentOsResult<Value> {
            match request {
                AppRequest::Initialize => {
                    self.requests.push("initialize");
                    Ok(json!({"initialized": true}))
                }
                AppRequest::ThreadResume { client_thread_id } => {
                    self.requests.push("thread/resume");
                    assert_eq!(client_thread_id, "thread_1");
                    Ok(json!({
                        "thread": {
                            "client_thread_id": "thread_1",
                            "task_id": "task_1",
                            "status": "Ready"
                        },
                        "previous_thread_status": "Running",
                        "reconciliation": {"reconciliation_id": "rec_1"}
                    }))
                }
                AppRequest::TurnStart {
                    client_thread_id,
                    input,
                } => {
                    self.requests.push("turn/start");
                    assert_eq!(client_thread_id, "thread_1");
                    assert_eq!(input, "resume requested");
                    Ok(json!({
                        "runtime_job": {
                            "runtime_job_id": "rtjob_1",
                            "status": "queued"
                        }
                    }))
                }
                AppRequest::ThreadRead { client_thread_id } => {
                    self.requests.push("thread/read");
                    assert_eq!(client_thread_id, "thread_1");
                    Ok(json!({
                        "thread": {
                            "client_thread_id": "thread_1",
                            "task_id": "task_1",
                            "status": "Completed"
                        },
                        "turns": [],
                        "timeline": [],
                        "runtime_jobs": [{
                            "runtime_job_id": "rtjob_1",
                            "status": "completed",
                            "job": {"client_thread_id": "thread_1"}
                        }],
                        "resources": [],
                        "automation_runs": []
                    }))
                }
                AppRequest::StatsRead { .. } => {
                    self.requests.push("stats/read");
                    Ok(json!({"snapshot": {"tool_calls": 1}}))
                }
                AppRequest::TaskBundleExport { client_thread_id } => {
                    self.requests.push("task/bundle/export");
                    assert_eq!(client_thread_id, "thread_1");
                    Ok(json!({
                        "bundle": {
                            "abi_version": "0.3.0",
                            "bundle_kind": "task",
                            "exported_at": "2026-06-30T00:00:00Z",
                            "root_task_id": "task_1",
                            "goal_id": "goal_1",
                            "task_ids": ["task_1"],
                            "profile_snapshot": {},
                            "projection_snapshot": {},
                            "events": [],
                            "replay_summary": {
                                "event_count": 1,
                                "task_count": 1,
                                "thread_count": 1,
                                "artifact_count": 0,
                                "evidence_count": 0,
                                "final_submission_count": 0
                            }
                        }
                    }))
                }
                other => panic!("unexpected resume request: {other:?}"),
            }
        }
    }
}
