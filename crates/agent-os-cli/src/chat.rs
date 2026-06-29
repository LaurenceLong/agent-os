use crate::args::ChatOptions;
use crate::provider_config::GlobalProviderConfig;
use crate::support::{io_result, open_kernel, write_task_bundle_if_requested};
use agent_os_kernel::{
    Kernel, RegisterGoalInput, SpawnAgentInput, SpawnTaskInput, UpdateTaskInput,
};
use agent_os_sys::*;
use agent_os_thread::{
    expand_command_template, import_workspace_ecosystem, OpenAiModelClient, RuntimeConfig,
    RuntimeRunReport, ThreadRuntime,
};
use serde_json::{json, Value};
use std::fs;
use std::io::{self, BufRead, Write};

pub(crate) fn run_chat(options: &ChatOptions) -> AgentOsResult<Value> {
    let provider_config = GlobalProviderConfig::load()?;
    let provider = provider_config.resolve(options.provider.as_deref())?;
    let model = provider.model.clone();

    let mut client_builder = OpenAiModelClient::new(provider.api_key.clone(), model.clone())
        .with_api_base(provider.base_url.clone())
        .with_api_style(provider.api_style);
    if let Some(max_tokens) = options.max_tokens {
        client_builder = client_builder.with_max_tokens(max_tokens);
    }
    if let Some(temp) = options.temperature {
        client_builder = client_builder.with_temperature(temp);
    }

    io_result(
        fs::create_dir_all(&options.workspace),
        "create workspace directory",
    )?;

    let kernel = open_kernel(&options.state_db)?;
    kernel.register_model_alias(
        &model,
        "external",
        &model,
        json!({
            "streaming": true,
            "tool_calling": true,
            "reasoning": true,
            "image_input": false,
            "structured_output": true
        }),
        "prov_default",
    )?;
    let mut session = ChatSession::new(kernel, options.workspace.clone(), provider.name, model);

    session.print_welcome();

    if let Some(initial_task) = &options.task {
        let _ = writeln!(io::stdout(), "\n> {initial_task}");
        let _ = io::stdout().flush();
        session.process_task(initial_task, client_builder.clone(), options)?;
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
        match session.process_task(trimmed, client_builder.clone(), options) {
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

struct ChatSession {
    kernel: Kernel,
    workspace: std::path::PathBuf,
    provider: String,
    model: String,
    task_count: usize,
    total_events: usize,
    last_thread_id: Option<String>,
    last_task_id: Option<String>,
}

impl ChatSession {
    fn new(kernel: Kernel, workspace: std::path::PathBuf, provider: String, model: String) -> Self {
        Self {
            kernel,
            workspace,
            provider,
            model,
            task_count: 0,
            total_events: 0,
            last_thread_id: None,
            last_task_id: None,
        }
    }

    fn process_task(
        &mut self,
        task_text: &str,
        client: OpenAiModelClient,
        options: &ChatOptions,
    ) -> AgentOsResult<()> {
        self.task_count += 1;
        let task_text = self.resolve_task_text(task_text)?;
        let goal = self.kernel.register_goal(RegisterGoalInput {
            namespace: "chat".to_string(),
            created_by: "agent-os-cli".to_string(),
            title: task_text.clone(),
            description: task_text.clone(),
            acceptance_criteria: vec!["agent completed the task with evidence".to_string()],
            constraints: Vec::new(),
            risk_level: 4,
            deadline: None,
        })?;
        let task = self.kernel.spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id.clone(),
            parent_task_id: None,
            title: task_text.to_string(),
            description: task_text.to_string(),
            depends_on: Vec::new(),
            required_artifact_types: vec![ArtifactType::Patch],
            required_evidence_types: vec![EvidenceType::DiffRef],
            priority: 10,
            risk_level: 4,
        })?;
        let agent = self.kernel.spawn_agent(SpawnAgentInput {
            task_id: task.task_id.clone(),
            role_profile_id: "role_worker".to_string(),
            owner: "agent-os-cli".to_string(),
            goal: task_text.clone(),
            success_criteria: vec!["task is complete and verified".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![self.workspace.to_string_lossy().to_string()],
        })?;
        self.kernel.update_task(UpdateTaskInput {
            task_id: task.task_id.clone(),
            status: Some(TaskStatus::Running),
            blocked_reason: None,
            owner_agent_id: Some(agent.agent_id.clone()),
            title: None,
            description: None,
            checklist: None,
        })?;

        self.last_thread_id = Some(agent.thread_id.clone());
        self.last_task_id = Some(task.task_id.clone());

        let max_steps = options.max_steps;
        let mut runtime = ThreadRuntime::new(self.kernel.clone(), agent.thread_id.clone(), client);
        let report = runtime.run_to_completion(RuntimeConfig {
            workspace_root: self.workspace.clone(),
            attach_mode: AttachMode::WorkspaceWrite,
            max_steps,
            requested_model_alias: Some(self.model.clone()),
            tool_risk_ceiling: 4,
            auto_commit_patch_artifacts: true,
            fail_on_process_nonzero: false,
        })?;

        self.print_report(&report);

        let bundle_path = write_task_bundle_if_requested(
            &self.kernel,
            &task.task_id,
            &self.workspace,
            &options.bundle_output,
        )?;
        if let Some(path) = bundle_path {
            let _ = writeln!(io::stdout(), "  Bundle: {path}");
        }

        let events = self.kernel.events()?.len();
        self.total_events = events;
        Ok(())
    }

    fn resolve_task_text(&self, task_text: &str) -> AgentOsResult<String> {
        let Some(invocation) = parse_command_invocation(task_text) else {
            return Ok(task_text.to_string());
        };
        import_workspace_ecosystem(&self.kernel, &self.workspace)?;
        let state = self.kernel.state_snapshot()?;
        let command = state
            .command_definitions
            .get(invocation.name)
            .ok_or_else(|| AgentOsError::NotFound(format!("command /{}", invocation.name)))?;
        Ok(expand_command_template(
            &command.template,
            &invocation.args,
            invocation.raw_arguments,
        ))
    }

    fn print_report(&self, report: &RuntimeRunReport) {
        let _ = writeln!(io::stdout());
        for result in &report.tool_results {
            let icon = match result.status {
                ToolCallStatus::Completed => "[ok]",
                ToolCallStatus::Failed => "[fail]",
                _ => "[...]",
            };
            let detail = format_tool_summary(result);
            let _ = writeln!(io::stdout(), "  {icon} {}", detail);
        }
        if report.final_submitted {
            let _ = writeln!(
                io::stdout(),
                "\n  Done — {} tool call(s), {} artifact(s), {} event(s)",
                report.tool_results.len(),
                report.artifacts.len(),
                report.events,
            );
        }
    }

    fn print_welcome(&self) {
        let _ = writeln!(io::stdout(), "\nAgent-OS v0.1 — Interactive Coding Agent");
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
            "provider": self.provider,
            "model": self.model,
        })
    }
}

struct CommandInvocation<'a> {
    name: &'a str,
    raw_arguments: &'a str,
    args: Vec<String>,
}

fn parse_command_invocation(input: &str) -> Option<CommandInvocation<'_>> {
    let input = input.trim();
    let rest = input.strip_prefix('/')?;
    if rest.is_empty() {
        return None;
    }
    let (name, raw_arguments) = rest
        .split_once(char::is_whitespace)
        .map(|(name, args)| (name, args.trim()))
        .unwrap_or((rest, ""));
    if name.is_empty() {
        return None;
    }
    Some(CommandInvocation {
        name,
        raw_arguments,
        args: raw_arguments
            .split_whitespace()
            .map(str::to_string)
            .collect(),
    })
}

fn format_tool_summary(result: &agent_os_thread::ToolExecutionRecord) -> String {
    let name = friendly_tool_name(&result.tool_name);
    match result.tool_name.as_str() {
        "read_file" => {
            let path = result
                .output
                .as_ref()
                .and_then(|o| o.get("path"))
                .and_then(Value::as_str)
                .unwrap_or("?");
            format!("{name} {path}")
        }
        "write_file" => {
            let path = result
                .output
                .as_ref()
                .and_then(|o| o.get("written_path"))
                .or_else(|| result.output.as_ref().and_then(|o| o.get("path")))
                .and_then(Value::as_str)
                .unwrap_or("?");
            format!("{name} {path}")
        }
        "replace_text" => {
            let path = result
                .output
                .as_ref()
                .and_then(|o| o.get("changed_path"))
                .and_then(Value::as_str)
                .unwrap_or("?");
            let count = result
                .output
                .as_ref()
                .and_then(|o| o.get("replacements"))
                .and_then(Value::as_i64)
                .unwrap_or(1);
            format!("{name} {path} ({count} replacement(s))")
        }
        "delete_file" => {
            let path = result
                .output
                .as_ref()
                .and_then(|o| o.get("deleted_path"))
                .and_then(Value::as_str)
                .unwrap_or("?");
            format!("{name} {path}")
        }
        "run_command" => {
            let exit = result
                .output
                .as_ref()
                .and_then(|o| o.get("exit_code"))
                .and_then(Value::as_i64)
                .unwrap_or(-1);
            let program = result
                .output
                .as_ref()
                .and_then(|o| o.get("input"))
                .and_then(|i| i.get("program"))
                .and_then(Value::as_str)
                .unwrap_or("?");
            format!("{name} {program} (exit {exit})")
        }
        _ => name.to_string(),
    }
}

fn friendly_tool_name(tool_name: &str) -> &str {
    match tool_name {
        "read_file" => "read",
        "write_file" => "write",
        "replace_text" => "edit",
        "delete_file" => "delete",
        "run_command" => "run",
        _ => tool_name,
    }
}

fn print_help() {
    let _ = writeln!(
        io::stdout(),
        "Commands:\n  \
         <task>   — Give the agent a coding task\n  \
         status   — Show session statistics\n  \
         help     — Show this message\n  \
         exit     — End the session"
    );
}

#[cfg(test)]
mod tests;
