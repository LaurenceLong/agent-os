use super::*;
use agent_os_kernel::Kernel;
use agent_os_store_sqlite::SqliteStore;
use agent_os_sys::new_id;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn run_from_app_client_polls_projection_until_runtime_completed() {
    let workspace = PathBuf::from("workspace");
    let mut client = FakeRunClient::default();

    let output = run_from_app_client(
        &mut client,
        &RunOptions {
            workspace,
            task: "Write result.md".to_string(),
            output: PathBuf::from("result.md"),
            bundle_output: None,
            state_db: Some(PathBuf::from("state.sqlite")),
            model_command: Some(PathBuf::from("model.exe")),
            model_args: Vec::new(),
        },
        "Write result.md\nRequested workspace output path: result.md".to_string(),
        &PathBuf::from("state.sqlite"),
    )
    .unwrap();

    assert_eq!(output["status"], json!("completed"));
    assert_eq!(output["runtime_job_status"], json!("completed"));
    assert_eq!(output["runtime_status"], json!("Completed"));
    assert_eq!(output["artifact_ids"], json!(["art_1"]));
    assert_eq!(output["evidence_ids"], json!(["evd_1"]));
    assert_eq!(
        client.requests,
        vec![
            "initialize",
            "thread/start",
            "turn/start",
            "thread/read",
            "stats/read",
        ]
    );
}

#[test]
fn run_from_app_client_exports_bundle_when_requested() {
    let workspace = env::temp_dir().join(format!(
        "agent-os-cli-run-bundle-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    let mut client = FakeRunClient::default();

    let output = run_from_app_client(
        &mut client,
        &RunOptions {
            workspace: workspace.clone(),
            task: "Write result.md".to_string(),
            output: PathBuf::from("result.md"),
            bundle_output: Some(PathBuf::from("bundle/task.json")),
            state_db: Some(PathBuf::from("state.sqlite")),
            model_command: Some(PathBuf::from("model.exe")),
            model_args: Vec::new(),
        },
        "Write result.md\nRequested workspace output path: result.md".to_string(),
        &PathBuf::from("state.sqlite"),
    )
    .unwrap();

    let bundle_path = workspace.join("bundle/task.json");
    assert_eq!(output["bundle_path"], json!(bundle_path.to_string_lossy()));
    let bundle: Value = serde_json::from_slice(&fs::read(&bundle_path).unwrap()).unwrap();
    assert_eq!(bundle["root_task_id"], "task_1");
    assert_eq!(
        client.requests,
        vec![
            "initialize",
            "thread/start",
            "turn/start",
            "thread/read",
            "stats/read",
            "task/bundle/export",
        ]
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn cli_run_rejects_bundle_output_path_escape() {
    let options = RunOptions {
        workspace: PathBuf::from("."),
        task: "Write a deterministic task report".to_string(),
        output: PathBuf::from("result.md"),
        bundle_output: Some(PathBuf::from("../bundle.json")),
        state_db: None,
        model_command: Some(PathBuf::from("model.exe")),
        model_args: Vec::new(),
    };
    let error = run_e2e_task(&options).unwrap_err();
    assert!(error.to_string().contains("--bundle-output"));
}

#[derive(Default)]
struct FakeRunClient {
    requests: Vec<&'static str>,
}

impl RunAppClient for FakeRunClient {
    fn request(&mut self, request: AppRequest) -> AgentOsResult<Value> {
        match request {
            AppRequest::Initialize => {
                self.requests.push("initialize");
                Ok(json!({"initialized": true}))
            }
            AppRequest::ThreadStart { goal, workspace } => {
                self.requests.push("thread/start");
                assert_eq!(
                    goal,
                    "Write result.md\nRequested workspace output path: result.md"
                );
                assert!(workspace.as_deref().is_some_and(|path| !path.is_empty()));
                Ok(json!({
                    "thread": {
                        "client_thread_id": "thread_1",
                        "task_id": "task_1",
                        "goal_id": "goal_1",
                        "status": "Ready"
                    }
                }))
            }
            AppRequest::TurnStart {
                client_thread_id,
                input,
            } => {
                self.requests.push("turn/start");
                assert_eq!(client_thread_id, "thread_1");
                assert_eq!(
                    input,
                    "Write result.md\nRequested workspace output path: result.md"
                );
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
                        "goal_id": "goal_1",
                        "status": "Completed"
                    },
                    "turns": [],
                    "timeline": [
                        {
                            "item_type": "ToolUpdated",
                            "payload": {
                                "tool_call_id": "tool_1",
                                "tool_name": "apply_patch",
                                "status": "Completed",
                                "evidence_ids": ["evd_1"]
                            }
                        }
                    ],
                    "runtime_jobs": [
                        {
                            "runtime_job_id": "rtjob_1",
                            "status": "completed"
                        }
                    ],
                    "artifacts": [
                        {
                            "artifact_id": "art_1",
                            "payload": {"artifact_id": "art_1"}
                        }
                    ],
                    "evidence": [
                        {
                            "evidence_id": "evd_1",
                            "payload": {"evidence_id": "evd_1"}
                        }
                    ],
                    "resources": [],
                    "automation_runs": []
                }))
            }
            AppRequest::TaskBundleExport { client_thread_id } => {
                self.requests.push("task/bundle/export");
                assert_eq!(client_thread_id, "thread_1");
                Ok(json!({
                    "bundle": {
                        "abi_version": "0.2.0",
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
            AppRequest::StatsRead { query } => {
                self.requests.push("stats/read");
                assert_eq!(query, StatsQuery::default());
                Ok(json!({
                    "snapshot": {
                        "input_tokens": 1,
                        "output_tokens": 1,
                        "cached_input_tokens": 0,
                        "cost": 0.0,
                        "provider_calls": 1,
                        "provider_errors": 0,
                        "tool_calls": 1,
                        "tool_successes": 1,
                        "tool_failures": 0,
                        "tool_denials": 0,
                        "cache_hits": 0,
                        "cache_misses": 0,
                        "approvals_requested": 0,
                        "approvals_resolved": 0,
                        "budget_debits": 0,
                        "latency_ms_total": 0,
                        "updated_at": "2026-06-30T00:00:00Z"
                    }
                }))
            }
            other => panic!("unexpected request {other:?}"),
        }
    }
}

#[test]
fn cli_run_rejects_output_path_escape() {
    let options = RunOptions {
        workspace: PathBuf::from("."),
        task: "bad output".to_string(),
        output: PathBuf::from("../escape.md"),
        bundle_output: None,
        state_db: None,
        model_command: None,
        model_args: Vec::new(),
    };
    let err = run_e2e_task(&options).unwrap_err();
    assert!(matches!(err, AgentOsError::Validation(_)));
}

#[test]
fn cli_run_persists_events_to_state_db_for_restart_replay() {
    let workspace = env::temp_dir().join(format!(
        "agent-os-cli-state-db-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    let state_db = workspace.join("agent-os.sqlite");
    fs::create_dir_all(&workspace).unwrap();
    let model_program = compile_external_run_model(&workspace);
    let options = RunOptions {
        workspace: workspace.clone(),
        task: "Write a durable task report".to_string(),
        output: PathBuf::from("result.md"),
        bundle_output: None,
        state_db: Some(state_db.clone()),
        model_command: Some(model_program),
        model_args: Vec::new(),
    };
    let output = run_e2e_task(&options).unwrap();
    assert_eq!(output["status"], json!("completed"));
    assert!(state_db.exists());

    let replayed = Kernel::with_replayed_store(SqliteStore::open(&state_db).unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    let task_id = output["task_id"].as_str().unwrap();
    assert!(replayed_state.tasks.contains_key(task_id));
    assert!(replayed_state.final_submissions.contains_key(task_id));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn cli_run_can_use_external_model_process() {
    let workspace = env::temp_dir().join(format!(
        "agent-os-cli-external-run-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    let source_path = workspace.join("external_run_model.rs");
    let model_program = workspace.join(format!(
        "external_run_model{}",
        std::env::consts::EXE_SUFFIX
    ));
    fs::write(
        &source_path,
        r##"
use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    match step_index(&input) {
        0 => {
            let workspace_root = json_string(&input, "workspace_root");
            print!(
                "{{\"actions\":[{{\"type\":\"tool_call\",\"tool_name\":\"apply_patch\",\"input\":{{\"workspace_root\":\"{}\",\"patch\":\"*** Begin Patch\\n*** Add File: result.md\\n+external model completed\\n*** End Patch\\n\"}},\"risk_level\":4,\"evidence_claim\":\"external model created result file through apply_patch\"}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":1,\"cost\":0.0}}}}",
                workspace_root
            );
        }
        _ => {
            let evidence_id = first_evidence_id(&input);
            print!(
                "{{\"actions\":[{{\"type\":\"final\",\"submission\":{{\"summary\":\"External model completed the task.\",\"changed_artifacts\":[],\"evidence_map\":[{{\"claim\":\"result file was written\",\"evidence_refs\":[\"{}\"]}}],\"unverified_claims\":[],\"known_risks\":[],\"tests_run\":[],\"tests_not_run\":[],\"approvals\":[]}}}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":1,\"cost\":0.0}}}}",
                evidence_id
            );
        }
    }
}

fn step_index(input: &str) -> u32 {
    let marker = "\"step_index\":";
    let start = input.find(marker).unwrap() + marker.len();
    let rest = &input[start..];
    let end = rest
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().unwrap()
}

fn json_string(input: &str, field: &str) -> String {
    let marker = format!("\"{}\":\"", field);
    let start = input.find(&marker).unwrap() + marker.len();
    let bytes = input.as_bytes();
    let mut index = start;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            break;
        }
        index += 1;
    }
    input[start..index].to_string()
}

fn first_evidence_id(input: &str) -> String {
    let marker = "\"evidence_ids\":[\"";
    let start = input.find(marker).unwrap() + marker.len();
    let rest = &input[start..];
    let end = rest.find('"').unwrap();
    rest[..end].to_string()
}
"##,
    )
    .unwrap();
    let rustc_output = Command::new("rustc")
        .arg(&source_path)
        .arg("-o")
        .arg(&model_program)
        .output()
        .unwrap();
    assert!(
        rustc_output.status.success(),
        "rustc failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&rustc_output.stdout),
        String::from_utf8_lossy(&rustc_output.stderr)
    );
    let options = RunOptions {
        workspace: workspace.clone(),
        task: "Write result.md with an external model".to_string(),
        output: PathBuf::from("result.md"),
        bundle_output: Some(PathBuf::from("bundle/run.json")),
        state_db: None,
        model_command: Some(model_program),
        model_args: Vec::new(),
    };
    let output = run_e2e_task(&options).unwrap();
    assert_eq!(output["status"], json!("completed"));
    assert_eq!(
        fs::read_to_string(workspace.join("result.md")).unwrap(),
        "external model completed\n"
    );
    assert_eq!(output["runtime_job_status"], json!("completed"));
    assert_eq!(output["artifact_ids"].as_array().unwrap().len(), 1);
    let bundle_path = workspace.join("bundle/run.json");
    assert_eq!(output["bundle_path"], json!(bundle_path.to_string_lossy()));
    let bundle: Value = serde_json::from_slice(&fs::read(bundle_path).unwrap()).unwrap();
    assert_eq!(bundle["root_task_id"], output["task_id"]);
    let state_db = PathBuf::from(output["state_db"].as_str().unwrap());
    assert!(state_db.exists());
    let _ = fs::remove_dir_all(workspace);
}

fn compile_external_run_model(workspace: &Path) -> PathBuf {
    let source_path = workspace.join("external_run_model_helper.rs");
    let model_program = workspace.join(format!(
        "external_run_model_helper{}",
        std::env::consts::EXE_SUFFIX
    ));
    fs::write(
        &source_path,
        r##"
use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    match step_index(&input) {
        0 => {
            let workspace_root = json_string(&input, "workspace_root");
            print!(
                "{{\"actions\":[{{\"type\":\"tool_call\",\"tool_name\":\"apply_patch\",\"input\":{{\"workspace_root\":\"{}\",\"patch\":\"*** Begin Patch\\n*** Add File: result.md\\n+external model completed\\n*** End Patch\\n\"}},\"risk_level\":4,\"evidence_claim\":\"external model created result file through apply_patch\"}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":1,\"cost\":0.0}}}}",
                workspace_root
            );
        }
        _ => {
            let evidence_id = first_evidence_id(&input);
            print!(
                "{{\"actions\":[{{\"type\":\"final\",\"submission\":{{\"summary\":\"External model completed the task.\",\"changed_artifacts\":[],\"evidence_map\":[{{\"claim\":\"result file was written\",\"evidence_refs\":[\"{}\"]}}],\"unverified_claims\":[],\"known_risks\":[],\"tests_run\":[],\"tests_not_run\":[],\"approvals\":[]}}}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":1,\"cost\":0.0}}}}",
                evidence_id
            );
        }
    }
}

fn step_index(input: &str) -> u32 {
    let marker = "\"step_index\":";
    let start = input.find(marker).unwrap() + marker.len();
    let rest = &input[start..];
    let end = rest
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().unwrap()
}

fn json_string(input: &str, field: &str) -> String {
    let marker = format!("\"{}\":\"", field);
    let start = input.find(&marker).unwrap() + marker.len();
    let bytes = input.as_bytes();
    let mut index = start;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            break;
        }
        index += 1;
    }
    input[start..index].to_string()
}

fn first_evidence_id(input: &str) -> String {
    let marker = "\"evidence_ids\":[\"";
    let start = input.find(marker).unwrap() + marker.len();
    let rest = &input[start..];
    let end = rest.find('"').unwrap();
    rest[..end].to_string()
}
"##,
    )
    .unwrap();
    let rustc_output = Command::new("rustc")
        .arg(&source_path)
        .arg("-o")
        .arg(&model_program)
        .output()
        .unwrap();
    assert!(
        rustc_output.status.success(),
        "rustc failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&rustc_output.stdout),
        String::from_utf8_lossy(&rustc_output.stderr)
    );
    model_program
}
