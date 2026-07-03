use crate::args::CodeOptions;
use crate::support::{
    default_state_db_for_workspace, ensure_safe_relative_workspace_path, io_result,
    write_task_bundle_from_app_response, StdioHostAppClient, StdioHostConfig,
};
use agent_os_distro::{SoftwareExactEdit, SoftwareWorkflowPrompt, SoftwareWorkflowRequest};
use agent_os_sys::*;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::time::Duration;

const CODE_RUNTIME_POLL_ATTEMPTS: usize = 480;
const CODE_RUNTIME_POLL_INTERVAL: Duration = Duration::from_millis(250);

pub(crate) fn run_code_task(options: &CodeOptions) -> AgentOsResult<Value> {
    if let Some(file) = &options.file {
        ensure_safe_relative_workspace_path(file, "--file")?;
    }
    if let Some(bundle_output) = &options.bundle_output {
        ensure_safe_relative_workspace_path(bundle_output, "--bundle-output")?;
    }
    if options.old.is_some() != options.new.is_some() {
        return Err(AgentOsError::Validation(
            "--old and --new must be provided together".to_string(),
        ));
    }
    if options.old.is_some() && options.file.is_none() {
        return Err(AgentOsError::Validation(
            "--file is required when --old and --new are provided".to_string(),
        ));
    }
    io_result(
        fs::create_dir_all(&options.workspace),
        "create workspace directory",
    )?;
    if let (Some(file), Some(_), Some(_)) = (&options.file, &options.old, &options.new) {
        let target_path = options.workspace.join(file);
        if !target_path.exists() {
            return Err(AgentOsError::NotFound(format!(
                "target file {}",
                target_path.to_string_lossy()
            )));
        }
    }

    let model_command = options.model_command.as_ref().ok_or_else(|| {
        AgentOsError::Validation(
            "--model-command is required by code app-server projection path".to_string(),
        )
    })?;
    let state_db = options
        .state_db
        .clone()
        .map(Ok)
        .unwrap_or_else(|| default_state_db_for_workspace(&options.workspace))?;
    let mut config = StdioHostConfig::state_db(state_db.clone());
    config.model_command = Some(model_command.clone());
    config.model_args = options.model_args.clone();
    let mut app_client = StdioHostAppClient::open(&config)?;
    let task_prompt = build_code_task_prompt(options)?;
    let mut output = run_code_from_app_client(&mut app_client, options, task_prompt, &state_db)?;
    output["model_command"] = json!(model_command.to_string_lossy());
    output["model_args"] = json!(&options.model_args);
    Ok(output)
}

trait CodeAppClient {
    fn request(&mut self, request: AppRequest) -> AgentOsResult<Value>;
}

impl CodeAppClient for StdioHostAppClient {
    fn request(&mut self, request: AppRequest) -> AgentOsResult<Value> {
        StdioHostAppClient::request(self, request)
    }
}

fn build_code_task_prompt(options: &CodeOptions) -> AgentOsResult<String> {
    let request = SoftwareWorkflowRequest {
        workspace_root: options.workspace.clone(),
        task: options.task.clone(),
        target_file: options.file.clone(),
        exact_edit: options
            .old
            .as_ref()
            .zip(options.new.as_ref())
            .map(|(old, new)| SoftwareExactEdit {
                old: old.clone(),
                new: new.clone(),
            }),
        test_program: options.test_program.clone(),
        test_args: options.test_args.clone(),
        edit_plan_source: None,
    };
    Ok(SoftwareWorkflowPrompt::from_request(&request)?.prompt)
}

fn run_code_from_app_client(
    app_client: &mut impl CodeAppClient,
    options: &CodeOptions,
    task_prompt: String,
    state_db: &Path,
) -> AgentOsResult<Value> {
    app_client.request(AppRequest::Initialize)?;
    let started = app_client.request(AppRequest::ThreadStart {
        goal: task_prompt.clone(),
        workspace: Some(options.workspace.to_string_lossy().to_string()),
    })?;
    let thread_id = required_json_string(&started["thread"], "client_thread_id")?;
    let task_id = required_json_string(&started["thread"], "task_id")?;
    let goal_id = required_json_string(&started["thread"], "goal_id")?;

    let turn = app_client.request(AppRequest::TurnStart {
        client_thread_id: thread_id.clone(),
        input: task_prompt,
    })?;
    let runtime_job_id = required_json_string(&turn["runtime_job"], "runtime_job_id")?;
    let thread = wait_for_code_runtime_job(app_client, &thread_id, &runtime_job_id)?;
    let stats = app_client.request(AppRequest::StatsRead {
        query: StatsQuery::default(),
    })?["snapshot"]
        .clone();
    let runtime_job = runtime_job_by_id(&thread, &runtime_job_id)?;
    let artifact_ids = json_field_strings(&thread["artifacts"], "artifact_id");
    let evidence_ids = json_field_strings(&thread["evidence"], "evidence_id");
    let tool_results = tool_results_from_timeline(&thread["timeline"]);
    let artifacts = projection_payloads(&thread["artifacts"]);
    let evidence = projection_payloads(&thread["evidence"]);
    let bundle_path = if options.bundle_output.is_some() {
        let exported = app_client.request(AppRequest::TaskBundleExport {
            client_thread_id: thread_id.clone(),
        })?;
        write_task_bundle_from_app_response(
            &options.workspace,
            &options.bundle_output,
            &exported["bundle"],
        )?
    } else {
        None
    };
    let changed_path = options
        .file
        .as_ref()
        .map(|file| options.workspace.join(file).to_string_lossy().to_string());
    let latest_artifact_id = artifact_ids.last().cloned();
    let edit_plan_source = if options.old.is_some() {
        "exact_prompt"
    } else {
        "external_model"
    };

    Ok(json!({
        "status": "completed",
        "goal_id": goal_id,
        "task_id": task_id,
        "thread_id": thread_id,
        "changed_path": changed_path,
        "state_db": state_db.to_string_lossy(),
        "bundle_path": bundle_path,
        "planned_file": options.file.as_ref().map(|file| file.to_string_lossy().to_string()),
        "edit_plan_source": edit_plan_source,
        "latest_artifact_id": latest_artifact_id,
        "artifact_ids": artifact_ids,
        "evidence_ids": evidence_ids,
        "test_exit_code": null,
        "runtime_status": thread["thread"]["status"],
        "runtime_job_status": runtime_job["status"],
        "tool_results": tool_results,
        "artifacts": artifacts,
        "evidence": evidence,
        "stats": stats
    }))
}

fn wait_for_code_runtime_job(
    app_client: &mut impl CodeAppClient,
    thread_id: &str,
    runtime_job_id: &str,
) -> AgentOsResult<Value> {
    for attempt in 0..CODE_RUNTIME_POLL_ATTEMPTS {
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
        if attempt + 1 < CODE_RUNTIME_POLL_ATTEMPTS {
            std::thread::sleep(CODE_RUNTIME_POLL_INTERVAL);
        }
    }
    Err(AgentOsError::Validation(format!(
        "runtime job {runtime_job_id} did not complete before code timeout"
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

fn json_field_strings(items: &Value, field: &str) -> Vec<String> {
    items
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item[field].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn projection_payloads(items: &Value) -> Vec<Value> {
    items
        .as_array()
        .map(|items| items.iter().map(|item| item["payload"].clone()).collect())
        .unwrap_or_default()
}

fn tool_results_from_timeline(timeline: &Value) -> Vec<Value> {
    timeline
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter(|item| item["item_type"].as_str() == Some("ToolUpdated"))
                .map(|item| item["payload"].clone())
                .collect()
        })
        .unwrap_or_default()
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
    use agent_os_sys::new_id;
    use std::env;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    #[test]
    fn code_from_app_client_polls_projection_until_runtime_completed() {
        let workspace = PathBuf::from("workspace");
        let mut client = FakeCodeClient::default();

        let output = run_code_from_app_client(
            &mut client,
            &CodeOptions {
                workspace,
                task: "Change answer from one to two".to_string(),
                file: Some(PathBuf::from("src/lib.rs")),
                old: Some("1".to_string()),
                new: Some("2".to_string()),
                test_program: PathBuf::from("test.exe"),
                test_args: vec!["--help".to_string()],
                bundle_output: None,
                state_db: Some(PathBuf::from("state.sqlite")),
                model_command: Some(PathBuf::from("model.exe")),
                model_args: Vec::new(),
            },
            build_code_task_prompt(&CodeOptions {
                workspace: PathBuf::from("workspace"),
                task: "Change answer from one to two".to_string(),
                file: Some(PathBuf::from("src/lib.rs")),
                old: Some("1".to_string()),
                new: Some("2".to_string()),
                test_program: PathBuf::from("test.exe"),
                test_args: vec!["--help".to_string()],
                bundle_output: None,
                state_db: Some(PathBuf::from("state.sqlite")),
                model_command: Some(PathBuf::from("model.exe")),
                model_args: Vec::new(),
            })
            .unwrap(),
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
    fn code_from_app_client_exports_bundle_when_requested() {
        let workspace = env::temp_dir().join(format!(
            "agent-os-cli-code-bundle-{}-{}",
            std::process::id(),
            new_id("case_")
        ));
        fs::create_dir_all(&workspace).unwrap();
        let mut client = FakeCodeClient::default();
        let options = CodeOptions {
            workspace: workspace.clone(),
            task: "Change answer from one to two".to_string(),
            file: Some(PathBuf::from("src/lib.rs")),
            old: Some("1".to_string()),
            new: Some("2".to_string()),
            test_program: PathBuf::from("test.exe"),
            test_args: vec!["--help".to_string()],
            bundle_output: Some(PathBuf::from("bundle/code.json")),
            state_db: Some(PathBuf::from("state.sqlite")),
            model_command: Some(PathBuf::from("model.exe")),
            model_args: Vec::new(),
        };

        let output = run_code_from_app_client(
            &mut client,
            &options,
            build_code_task_prompt(&options).unwrap(),
            &PathBuf::from("state.sqlite"),
        )
        .unwrap();

        let bundle_path = workspace.join("bundle/code.json");
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

    #[derive(Default)]
    struct FakeCodeClient {
        requests: Vec<&'static str>,
    }

    impl CodeAppClient for FakeCodeClient {
        fn request(&mut self, request: AppRequest) -> AgentOsResult<Value> {
            match request {
                AppRequest::Initialize => {
                    self.requests.push("initialize");
                    Ok(json!({"initialized": true}))
                }
                AppRequest::ThreadStart { goal, workspace } => {
                    self.requests.push("thread/start");
                    assert!(goal.contains("Change answer from one to two"));
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
                    assert!(input.contains("Target file: src/lib.rs"));
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
                other => panic!("unexpected request {other:?}"),
            }
        }
    }

    #[test]
    fn cli_code_applies_exact_edit_and_runs_test_command() {
        let workspace = env::temp_dir().join(format!(
            "agent-os-cli-code-{}-{}",
            std::process::id(),
            new_id("case_")
        ));
        fs::create_dir_all(workspace.join("src")).unwrap();
        fs::write(
            workspace.join("src/lib.rs"),
            "pub fn answer() -> i32 { 1 }\n",
        )
        .unwrap();
        let options = CodeOptions {
            workspace: workspace.clone(),
            task: "Change answer from one to two".to_string(),
            file: Some(PathBuf::from("src/lib.rs")),
            old: Some("1".to_string()),
            new: Some("2".to_string()),
            test_program: env::current_exe().unwrap(),
            test_args: vec!["--help".to_string()],
            bundle_output: Some(PathBuf::from("bundle/code.json")),
            state_db: Some(workspace.join("agent-os.sqlite")),
            model_command: Some(compile_external_code_model(&workspace)),
            model_args: vec![env::current_exe().unwrap().to_string_lossy().to_string()],
        };
        let output = run_code_task(&options).unwrap();
        assert_eq!(output["status"], json!("completed"));
        assert_eq!(
            fs::read_to_string(workspace.join("src/lib.rs")).unwrap(),
            "pub fn answer() -> i32 { 2 }\n"
        );
        assert_eq!(output["runtime_job_status"], json!("completed"));
        assert_eq!(output["edit_plan_source"], json!("exact_prompt"));
        assert_eq!(output["artifact_ids"].as_array().unwrap().len(), 1);
        assert!(output["evidence_ids"].as_array().unwrap().len() >= 2);
        assert!(output["stats"]["tool_calls"].as_u64().unwrap() >= 2);
        let bundle_path = workspace.join("bundle/code.json");
        assert_eq!(output["bundle_path"], json!(bundle_path.to_string_lossy()));
        let bundle: Value = serde_json::from_slice(&fs::read(bundle_path).unwrap()).unwrap();
        assert_eq!(bundle["root_task_id"], output["task_id"]);
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn cli_code_can_infer_simple_edit_from_task() {
        let workspace = env::temp_dir().join(format!(
            "agent-os-cli-code-plan-{}-{}",
            std::process::id(),
            new_id("case_")
        ));
        fs::create_dir_all(workspace.join("src")).unwrap();
        fs::write(
            workspace.join("src/lib.rs"),
            "pub fn answer() -> i32 { 1 }\n",
        )
        .unwrap();
        let options = CodeOptions {
            workspace: workspace.clone(),
            task: "Change answer from one to two".to_string(),
            file: None,
            old: None,
            new: None,
            test_program: env::current_exe().unwrap(),
            test_args: vec!["--help".to_string()],
            bundle_output: None,
            state_db: Some(workspace.join("agent-os.sqlite")),
            model_command: Some(compile_external_code_model(&workspace)),
            model_args: vec![env::current_exe().unwrap().to_string_lossy().to_string()],
        };
        let output = run_code_task(&options).unwrap();
        assert_eq!(output["status"], json!("completed"));
        assert_eq!(output["edit_plan_source"], json!("external_model"));
        assert_eq!(
            fs::read_to_string(workspace.join("src/lib.rs")).unwrap(),
            "pub fn answer() -> i32 { 2 }\n"
        );
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn cli_code_rejects_bundle_output_path_escape() {
        let options = CodeOptions {
            workspace: PathBuf::from("."),
            task: "Change answer from one to two".to_string(),
            file: Some(PathBuf::from("src/lib.rs")),
            old: Some("1".to_string()),
            new: Some("2".to_string()),
            test_program: PathBuf::from("test.exe"),
            test_args: vec!["--help".to_string()],
            bundle_output: Some(PathBuf::from("../bundle.json")),
            state_db: None,
            model_command: Some(PathBuf::from("model.exe")),
            model_args: Vec::new(),
        };
        let error = run_code_task(&options).unwrap_err();
        assert!(error.to_string().contains("--bundle-output"));
    }

    fn compile_external_code_model(workspace: &Path) -> PathBuf {
        let source_path = workspace.join("external_code_model.rs");
        let model_program = workspace.join(format!(
            "external_code_model{}",
            std::env::consts::EXE_SUFFIX
        ));
        fs::write(
            &source_path,
            r##"
use std::env;
use std::io::{self, Read};

fn main() {
    let test_program = env::args().nth(1).expect("test program arg");
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    match step_index(&input) {
        0 => {
            let workspace_root = json_string(&input, "workspace_root");
            let patch = "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-pub fn answer() -> i32 { 1 }\n+pub fn answer() -> i32 { 2 }\n*** End Patch\n";
            print!(
                "{{\"actions\":[{{\"type\":\"tool_call\",\"tool_name\":\"apply_patch\",\"input\":{{\"workspace_root\":\"{}\",\"patch\":\"{}\"}},\"risk_level\":4,\"evidence_claim\":\"code model updated src/lib.rs through apply_patch\"}},{{\"type\":\"tool_call\",\"tool_name\":\"run_command\",\"input\":{{\"mode\":\"exec\",\"command\":\"{}\",\"args\":[\"--help\"],\"cwd\":\"{}\"}},\"risk_level\":4,\"evidence_claim\":\"validation command ran\"}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":1,\"cost\":0.0}}}}",
                workspace_root,
                json_escape(patch),
                json_escape(&test_program),
                workspace_root
            );
        }
        _ => {
            let evidence_id = first_evidence_id(&input);
            print!(
                "{{\"actions\":[{{\"type\":\"final\",\"submission\":{{\"summary\":\"Code task completed through app-server runtime worker.\",\"changed_artifacts\":[],\"evidence_map\":[{{\"claim\":\"workspace edit and validation were captured\",\"evidence_refs\":[\"{}\"]}}],\"unverified_claims\":[],\"known_risks\":[],\"tests_run\":[\"test --help\"],\"tests_not_run\":[],\"approvals\":[]}}}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":1,\"cost\":0.0}}}}",
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

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped
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
}
