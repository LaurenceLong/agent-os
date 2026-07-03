use crate::args::RunOptions;
use crate::support::{
    default_state_db_for_workspace, ensure_safe_relative_workspace_path, io_result,
    write_task_bundle_from_app_response, StdioHostAppClient, StdioHostConfig,
};
use agent_os_sys::*;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::time::Duration;

const RUN_RUNTIME_POLL_ATTEMPTS: usize = 480;
const RUN_RUNTIME_POLL_INTERVAL: Duration = Duration::from_millis(250);

pub(crate) fn run_e2e_task(options: &RunOptions) -> AgentOsResult<Value> {
    ensure_safe_relative_workspace_path(&options.output, "--output")?;
    if let Some(bundle_output) = &options.bundle_output {
        ensure_safe_relative_workspace_path(bundle_output, "--bundle-output")?;
    }
    io_result(
        fs::create_dir_all(&options.workspace),
        "create workspace directory",
    )?;
    let model_command = options.model_command.as_ref().ok_or_else(|| {
        AgentOsError::Validation(
            "--model-command is required by run app-server projection path".to_string(),
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
    let task_prompt = format!(
        "{}\nRequested workspace output path: {}",
        options.task,
        options.output.to_string_lossy()
    );
    let mut output = run_from_app_client(&mut app_client, options, task_prompt, &state_db)?;
    output["model_command"] = json!(model_command.to_string_lossy());
    output["model_args"] = json!(&options.model_args);
    Ok(output)
}

trait RunAppClient {
    fn request(&mut self, request: AppRequest) -> AgentOsResult<Value>;
}

impl RunAppClient for StdioHostAppClient {
    fn request(&mut self, request: AppRequest) -> AgentOsResult<Value> {
        StdioHostAppClient::request(self, request)
    }
}

fn run_from_app_client(
    app_client: &mut impl RunAppClient,
    options: &RunOptions,
    task_prompt: String,
    state_db: &Path,
) -> AgentOsResult<Value> {
    let output_path = options.workspace.join(&options.output);
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
    let thread = wait_for_runtime_job(app_client, &thread_id, &runtime_job_id)?;
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

    Ok(json!({
        "status": "completed",
        "goal_id": goal_id,
        "task_id": task_id,
        "thread_id": thread_id,
        "output_path": output_path.to_string_lossy(),
        "state_db": state_db.to_string_lossy(),
        "bundle_path": bundle_path,
        "runtime_status": thread["thread"]["status"],
        "runtime_job_status": runtime_job["status"],
        "artifact_ids": artifact_ids,
        "evidence_ids": evidence_ids,
        "provider_stream_session_ids": [],
        "tool_results": tool_results,
        "artifacts": artifacts,
        "evidence": evidence,
        "stats": stats,
        "thread": thread["thread"],
        "turns": thread["turns"],
        "timeline": thread["timeline"],
        "runtime_jobs": thread["runtime_jobs"],
        "resources": thread["resources"],
        "automation_runs": thread["automation_runs"],
    }))
}

fn wait_for_runtime_job(
    app_client: &mut impl RunAppClient,
    thread_id: &str,
    runtime_job_id: &str,
) -> AgentOsResult<Value> {
    for attempt in 0..RUN_RUNTIME_POLL_ATTEMPTS {
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
        if attempt + 1 < RUN_RUNTIME_POLL_ATTEMPTS {
            std::thread::sleep(RUN_RUNTIME_POLL_INTERVAL);
        }
    }
    Err(AgentOsError::Validation(format!(
        "runtime job {runtime_job_id} did not complete before run timeout"
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
mod tests;
