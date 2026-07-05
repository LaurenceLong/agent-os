use crate::args::ChatOptions;
use crate::support::{
    default_state_db_for_workspace, ensure_safe_relative_workspace_path, io_result,
    write_task_bundle_from_app_response, StdioHostAppClient, StdioHostConfig,
};
use agent_os_config::ResolvedAgentOsConfig;
use agent_os_sys::*;
use serde_json::{json, Value};
use std::fs;
use std::io::{self, BufRead, Write};
use std::time::{Duration, Instant};

const CHAT_RUNTIME_POLL_INTERVAL: Duration = Duration::from_millis(250);

pub(crate) fn run_chat(options: &ChatOptions) -> AgentOsResult<Value> {
    if let Some(bundle_output) = &options.bundle_output {
        ensure_safe_relative_workspace_path(bundle_output, "--bundle-output")?;
    }
    let provider_config = ResolvedAgentOsConfig::load(Some(&options.workspace))?;
    let model = provider_config
        .providers
        .resolve(options.model.as_deref())?;

    io_result(
        fs::create_dir_all(&options.workspace),
        "create workspace directory",
    )?;

    let state_db = options
        .state_db
        .clone()
        .map(Ok)
        .unwrap_or_else(|| default_state_db_for_workspace(&options.workspace))?;
    let mut config = StdioHostConfig::state_db(state_db);
    config.model = Some(model.id.clone());
    config.max_steps = Some(options.max_steps);
    config.max_tokens = options.max_tokens;
    config.temperature = options.temperature.map(|value| value.to_string());
    let app_client = StdioHostAppClient::open(&config)?;
    let mut session = ChatSession::new_for_app_client(
        Box::new(app_client),
        options.workspace.clone(),
        model.provider_id,
        model.id,
    )?;

    session.print_welcome();

    let initial_task = resolve_initial_task(options)?;
    if let Some(initial_task) = &initial_task {
        let _ = writeln!(io::stdout(), "\n> {initial_task}");
        let _ = io::stdout().flush();
        session.process_task(initial_task, options)?;
        if exits_after_initial_task(options) {
            session.print_farewell();
            return Ok(session.summary());
        }
    }

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    loop {
        let _ = write!(io::stdout(), "\n> ");
        let _ = io::stdout().flush();
        let line = match lines.next() {
            Some(Ok(input)) => input,
            Some(Err(_)) | None => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match trimmed {
            "exit" | "quit" | ":q" => break,
            "help" | ":h" => {
                print_help();
                continue;
            }
            "status" | ":s" => {
                session.print_status();
                continue;
            }
            _ => {}
        }
        match session.process_task(trimmed, options) {
            Ok(()) => {}
            Err(AgentOsError::Validation(msg)) if msg.contains("max_steps") => {
                let _ = writeln!(
                    io::stdout(),
                    "\n[agent-os] Task did not complete within the step limit. \
                     Try breaking it into smaller steps."
                );
            }
            Err(e) => {
                let _ = writeln!(io::stdout(), "\n[agent-os] Error: {e}");
            }
        }
    }

    session.print_farewell();
    Ok(session.summary())
}

trait ChatAppClient {
    fn request(&mut self, request: AppRequest) -> AgentOsResult<Value>;
}

impl ChatAppClient for StdioHostAppClient {
    fn request(&mut self, request: AppRequest) -> AgentOsResult<Value> {
        StdioHostAppClient::request(self, request)
    }
}

fn exits_after_initial_task(options: &ChatOptions) -> bool {
    options.task_text().is_some() || options.task_file.is_some()
}

fn resolve_initial_task(options: &ChatOptions) -> AgentOsResult<Option<String>> {
    if let Some(task) = options.task_text() {
        return Ok(Some(task));
    }
    let Some(task_file) = &options.task_file else {
        return Ok(None);
    };
    let task = fs::read_to_string(task_file).map_err(|error| {
        AgentOsError::Validation(format!("read task file {}: {error}", task_file.display()))
    })?;
    Ok(Some(task))
}

fn required_json_string(object: &Value, field: &str) -> AgentOsResult<String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| AgentOsError::Validation(format!("app-server response omitted {field}")))
}

struct ChatSession {
    app_client: Box<dyn ChatAppClient>,
    workspace: std::path::PathBuf,
    provider: String,
    model: String,
    task_count: usize,
    total_events: usize,
    last_thread_id: Option<String>,
    last_task_id: Option<String>,
    last_bundle_path: Option<String>,
}

impl ChatSession {
    fn new_for_app_client(
        mut app_client: Box<dyn ChatAppClient>,
        workspace: std::path::PathBuf,
        provider: String,
        model: String,
    ) -> AgentOsResult<Self> {
        app_client.request(AppRequest::Initialize)?;
        Ok(Self {
            app_client,
            workspace,
            provider,
            model,
            task_count: 0,
            total_events: 0,
            last_thread_id: None,
            last_task_id: None,
            last_bundle_path: None,
        })
    }

    fn process_task(&mut self, task_text: &str, options: &ChatOptions) -> AgentOsResult<()> {
        if let Some(bundle_output) = &options.bundle_output {
            ensure_safe_relative_workspace_path(bundle_output, "--bundle-output")?;
        }
        self.task_count += 1;
        let task_text = self.resolve_task_text(task_text)?;
        let started = self.app_client.request(AppRequest::ThreadStart {
            goal: task_text.clone(),
            workspace: Some(self.workspace.to_string_lossy().to_string()),
        })?;
        let thread_id = required_json_string(&started["thread"], "client_thread_id")?;
        let task_id = required_json_string(&started["thread"], "task_id")?;
        let turn = self.app_client.request(AppRequest::TurnStart {
            client_thread_id: thread_id.clone(),
            input: task_text,
        })?;
        let runtime_job_id = required_json_string(&turn["runtime_job"], "runtime_job_id")?;

        self.last_thread_id = Some(thread_id.clone());
        self.last_task_id = Some(task_id);

        let thread = wait_for_runtime_job(
            &mut *self.app_client,
            &thread_id,
            &runtime_job_id,
            Duration::from_secs(options.runtime_timeout_seconds),
        )?;
        self.print_projection_report(&thread);
        self.total_events += thread["timeline"]
            .as_array()
            .map(Vec::len)
            .unwrap_or_default();
        if options.bundle_output.is_some() {
            let exported = self.app_client.request(AppRequest::TaskBundleExport {
                client_thread_id: thread_id,
            })?;
            self.last_bundle_path = write_task_bundle_from_app_response(
                &self.workspace,
                &options.bundle_output,
                &exported["bundle"],
            )?;
        }
        Ok(())
    }

    fn resolve_task_text(&self, task_text: &str) -> AgentOsResult<String> {
        if is_command_invocation(task_text) {
            return Err(AgentOsError::Validation(
                "workspace command expansion is not supported by chat app-server projection path"
                    .to_string(),
            ));
        }
        Ok(task_text.to_string())
    }

    fn print_projection_report(&self, thread: &Value) {
        let tool_results = tool_result_values_from_timeline(&thread["timeline"]);
        let _ = writeln!(io::stdout());
        for result in &tool_results {
            let icon = match result["status"].as_str() {
                Some("Completed") | Some("completed") => "[ok]",
                Some("Failed") | Some("failed") => "[fail]",
                _ => "[...]",
            };
            let detail = format_tool_summary_value(result);
            let _ = writeln!(io::stdout(), "  {icon} {}", detail);
        }
        if thread["runtime_jobs"]
            .as_array()
            .is_some_and(|jobs| jobs.iter().any(|job| job["status"] == "completed"))
        {
            let artifact_count = thread["artifacts"]
                .as_array()
                .map(Vec::len)
                .unwrap_or_default();
            let event_count = thread["timeline"]
                .as_array()
                .map(Vec::len)
                .unwrap_or_default();
            let _ = writeln!(
                io::stdout(),
                "\n  Done - {} tool call(s), {} artifact(s), {} event(s)",
                tool_results.len(),
                artifact_count,
                event_count,
            );
        }
    }

    fn print_welcome(&self) {
        let _ = writeln!(io::stdout(), "\nAgent-OS v0.4 - Interactive Coding Agent");
        let _ = writeln!(io::stdout(), "  Workspace: {}", self.workspace.display());
        let _ = writeln!(io::stdout(), "  Provider:  {}", self.provider);
        let _ = writeln!(io::stdout(), "  Model:     {}", self.model);
        let _ = writeln!(
            io::stdout(),
            "\n  Type a task, or 'help' for commands. 'exit' to quit."
        );
    }

    fn print_farewell(&self) {
        let _ = writeln!(
            io::stdout(),
            "\nSession: {} task(s), {} total events. Goodbye!",
            self.task_count,
            self.total_events
        );
    }

    fn print_status(&self) {
        let _ = writeln!(
            io::stdout(),
            "  Tasks: {}, Events: {}, Model: {}",
            self.task_count,
            self.total_events,
            self.model
        );
        let _ = writeln!(io::stdout(), "  Provider: {}", self.provider);
        if let Some(tid) = &self.last_thread_id {
            let _ = writeln!(io::stdout(), "  Last thread: {tid}");
        }
    }

    fn summary(&self) -> Value {
        json!({
            "status": "session_ended",
            "tasks": self.task_count,
            "total_events": self.total_events,
            "last_thread_id": self.last_thread_id,
            "last_task_id": self.last_task_id,
            "last_bundle_path": self.last_bundle_path,
            "provider": self.provider,
            "model": self.model,
        })
    }
}

fn wait_for_runtime_job(
    app_client: &mut dyn ChatAppClient,
    thread_id: &str,
    runtime_job_id: &str,
    timeout: Duration,
) -> AgentOsResult<Value> {
    let started_at = Instant::now();
    loop {
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
        let elapsed = started_at.elapsed();
        if elapsed >= timeout {
            break;
        }
        std::thread::sleep(CHAT_RUNTIME_POLL_INTERVAL.min(timeout - elapsed));
    }
    Err(AgentOsError::Validation(format!(
        "runtime job {runtime_job_id} did not complete before chat timeout"
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

fn tool_result_values_from_timeline(timeline: &Value) -> Vec<Value> {
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

fn is_command_invocation(input: &str) -> bool {
    let input = input.trim();
    let Some(rest) = input.strip_prefix('/') else {
        return false;
    };
    let (name, _) = rest
        .split_once(char::is_whitespace)
        .map(|(name, args)| (name, args.trim()))
        .unwrap_or((rest, ""));
    !name.is_empty()
}

fn format_tool_summary_value(result: &Value) -> String {
    let tool_name = result["tool_name"].as_str().unwrap_or("?");
    let name = friendly_tool_name(tool_name);
    match tool_name {
        "read_file" => {
            let path = result["output"]["path"].as_str().unwrap_or("?");
            format!("{name} {path}")
        }
        "apply_patch" => {
            let path = result["output"]["path"].as_str().unwrap_or("?");
            let operation = result["output"]["operation"].as_str().unwrap_or("patch");
            format!("{name} {operation} {path}")
        }
        "run_command" => {
            let exit = result["output"]["exit_code"].as_i64().unwrap_or(-1);
            let command = result["output"]["input"]["command"].as_str().unwrap_or("?");
            format!("{name} {command} (exit {exit})")
        }
        _ => name.to_string(),
    }
}

fn friendly_tool_name(tool_name: &str) -> &str {
    match tool_name {
        "read_file" => "read",
        "apply_patch" => "patch",
        "run_command" => "run",
        _ => tool_name,
    }
}

fn print_help() {
    let _ = writeln!(
        io::stdout(),
        "Commands:\n  \
         <task>   - Give the agent a coding task\n  \
         status   - Show session statistics\n  \
         help     - Show this message\n  \
         exit     - End the session"
    );
}

#[cfg(test)]
mod tests;
