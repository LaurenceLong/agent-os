use agent_os_sys::{AgentOsError, AgentOsResult, ProcessLifecycleState};
use serde_json::{json, Value};
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(crate) struct RunOptions {
    pub(crate) workspace: PathBuf,
    pub(crate) task: String,
    pub(crate) output: PathBuf,
    pub(crate) bundle_output: Option<PathBuf>,
    pub(crate) state_db: Option<PathBuf>,
    pub(crate) model_command: Option<PathBuf>,
    pub(crate) model_args: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct CodeOptions {
    pub(crate) workspace: PathBuf,
    pub(crate) task: String,
    pub(crate) file: Option<PathBuf>,
    pub(crate) old: Option<String>,
    pub(crate) new: Option<String>,
    pub(crate) test_program: PathBuf,
    pub(crate) test_args: Vec<String>,
    pub(crate) bundle_output: Option<PathBuf>,
    pub(crate) state_db: Option<PathBuf>,
    pub(crate) model_command: Option<PathBuf>,
    pub(crate) model_args: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct StatusOptions {
    pub(crate) state_db: Option<PathBuf>,
    pub(crate) thread_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcessAction {
    List,
    Stop,
    Kill,
}

#[derive(Debug, Clone)]
pub(crate) struct ProcessOptions {
    pub(crate) action: ProcessAction,
    pub(crate) state_db: Option<PathBuf>,
    pub(crate) process_id: Option<String>,
    pub(crate) state: Option<ProcessLifecycleState>,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ResumeOptions {
    pub(crate) state_db: Option<PathBuf>,
    pub(crate) thread_id: String,
    pub(crate) workspace: PathBuf,
    pub(crate) bundle_output: Option<PathBuf>,
    pub(crate) model_command: PathBuf,
    pub(crate) model_args: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ChatOptions {
    pub(crate) workspace: PathBuf,
    pub(crate) task: Option<String>,
    pub(crate) task_file: Option<PathBuf>,
    pub(crate) model: Option<String>,
    pub(crate) max_steps: u32,
    pub(crate) runtime_timeout_seconds: u64,
    pub(crate) max_tokens: Option<u64>,
    pub(crate) temperature: Option<f64>,
    pub(crate) state_db: Option<PathBuf>,
    pub(crate) bundle_output: Option<PathBuf>,
}

impl RunOptions {
    pub(crate) fn parse(args: &[String]) -> AgentOsResult<Self> {
        let mut workspace = PathBuf::from(".");
        let mut task = None;
        let mut output = PathBuf::from("agent-os-task-result.md");
        let mut bundle_output = None;
        let mut state_db = None;
        let mut model_command = None;
        let mut model_args = Vec::new();
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--workspace" => {
                    index += 1;
                    workspace = PathBuf::from(required_arg(args, index, "--workspace")?);
                }
                "--task" => {
                    index += 1;
                    let (value, consumed_index) = collect_task_arg(args, index, "--task")?;
                    task = Some(value);
                    index = consumed_index;
                }
                "--output" => {
                    index += 1;
                    output = PathBuf::from(required_arg(args, index, "--output")?);
                }
                "--bundle-output" => {
                    index += 1;
                    bundle_output =
                        Some(PathBuf::from(required_arg(args, index, "--bundle-output")?));
                }
                "--state-db" => {
                    index += 1;
                    state_db = Some(PathBuf::from(required_arg(args, index, "--state-db")?));
                }
                "--model-command" => {
                    index += 1;
                    model_command =
                        Some(PathBuf::from(required_arg(args, index, "--model-command")?));
                }
                "--model-arg" => {
                    index += 1;
                    model_args.push(required_arg(args, index, "--model-arg")?.to_string());
                }
                "--help" | "-h" => {
                    return Err(AgentOsError::Validation(
                        serde_json::to_string(&usage_json()).unwrap_or_default(),
                    ));
                }
                other => {
                    return Err(AgentOsError::Validation(format!(
                        "unknown run option {other}"
                    )));
                }
            }
            index += 1;
        }
        Ok(Self {
            workspace,
            task: task.unwrap_or_else(|| "Create an Agent-OS task result".to_string()),
            output,
            bundle_output,
            state_db,
            model_command,
            model_args,
        })
    }
}

impl CodeOptions {
    pub(crate) fn parse(args: &[String]) -> AgentOsResult<Self> {
        let mut workspace = PathBuf::from(".");
        let mut task = None;
        let mut file = None;
        let mut old = None;
        let mut new = None;
        let mut test_program = None;
        let mut test_args = Vec::new();
        let mut bundle_output = None;
        let mut state_db = None;
        let mut model_command = None;
        let mut model_args = Vec::new();
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--workspace" => {
                    index += 1;
                    workspace = PathBuf::from(required_arg(args, index, "--workspace")?);
                }
                "--task" => {
                    index += 1;
                    let (value, consumed_index) = collect_task_arg(args, index, "--task")?;
                    task = Some(value);
                    index = consumed_index;
                }
                "--file" => {
                    index += 1;
                    file = Some(PathBuf::from(required_arg(args, index, "--file")?));
                }
                "--old" => {
                    index += 1;
                    old = Some(required_arg(args, index, "--old")?.to_string());
                }
                "--new" => {
                    index += 1;
                    new = Some(required_arg(args, index, "--new")?.to_string());
                }
                "--test-program" => {
                    index += 1;
                    test_program =
                        Some(PathBuf::from(required_arg(args, index, "--test-program")?));
                }
                "--test-arg" => {
                    index += 1;
                    test_args.push(required_arg(args, index, "--test-arg")?.to_string());
                }
                "--bundle-output" => {
                    index += 1;
                    bundle_output =
                        Some(PathBuf::from(required_arg(args, index, "--bundle-output")?));
                }
                "--state-db" => {
                    index += 1;
                    state_db = Some(PathBuf::from(required_arg(args, index, "--state-db")?));
                }
                "--model-command" => {
                    index += 1;
                    model_command =
                        Some(PathBuf::from(required_arg(args, index, "--model-command")?));
                }
                "--model-arg" => {
                    index += 1;
                    model_args.push(required_arg(args, index, "--model-arg")?.to_string());
                }
                "--help" | "-h" => {
                    return Err(AgentOsError::Validation(
                        serde_json::to_string(&usage_json()).unwrap_or_default(),
                    ));
                }
                other => {
                    return Err(AgentOsError::Validation(format!(
                        "unknown code option {other}"
                    )));
                }
            }
            index += 1;
        }
        let test_program = match test_program {
            Some(program) => program,
            None => env::current_exe().map_err(|error| {
                AgentOsError::Validation(format!("resolve default test executable: {error}"))
            })?,
        };
        if test_args.is_empty() {
            test_args.push("--help".to_string());
        }
        if old.is_some() != new.is_some() {
            return Err(AgentOsError::Validation(
                "--old and --new must be provided together".to_string(),
            ));
        }
        if old.is_some() && file.is_none() {
            return Err(AgentOsError::Validation(
                "--file is required when --old and --new are provided".to_string(),
            ));
        }
        Ok(Self {
            workspace,
            task: task.unwrap_or_else(|| "Apply exact repository edit".to_string()),
            file,
            old,
            new,
            test_program,
            test_args,
            bundle_output,
            state_db,
            model_command,
            model_args,
        })
    }
}

impl StatusOptions {
    pub(crate) fn parse(args: &[String]) -> AgentOsResult<Self> {
        let mut state_db = None;
        let mut thread_id = None;
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--state-db" => {
                    index += 1;
                    state_db = Some(PathBuf::from(required_arg(args, index, "--state-db")?));
                }
                "--thread-id" => {
                    index += 1;
                    thread_id = Some(required_arg(args, index, "--thread-id")?.to_string());
                }
                "--help" | "-h" => {
                    return Err(AgentOsError::Validation(
                        serde_json::to_string(&usage_json()).unwrap_or_default(),
                    ));
                }
                other => {
                    return Err(AgentOsError::Validation(format!(
                        "unknown status option {other}"
                    )));
                }
            }
            index += 1;
        }
        Ok(Self {
            state_db,
            thread_id,
        })
    }
}

impl ProcessOptions {
    pub(crate) fn parse(args: &[String]) -> AgentOsResult<Self> {
        let action = match args.first().map(String::as_str) {
            Some("list") => ProcessAction::List,
            Some("stop") => ProcessAction::Stop,
            Some("kill") => ProcessAction::Kill,
            Some("--help") | Some("-h") | None => {
                return Err(AgentOsError::Validation(
                    serde_json::to_string(&usage_json()).unwrap_or_default(),
                ));
            }
            Some(other) => {
                return Err(AgentOsError::Validation(format!(
                    "unknown process action {other}"
                )));
            }
        };
        let mut state_db = None;
        let mut process_id = None;
        let mut state = None;
        let mut reason = None;
        let mut index = 1;
        while index < args.len() {
            match args[index].as_str() {
                "--state-db" => {
                    index += 1;
                    state_db = Some(PathBuf::from(required_arg(args, index, "--state-db")?));
                }
                "--process-id" => {
                    index += 1;
                    process_id = Some(required_arg(args, index, "--process-id")?.to_string());
                }
                "--reason" => {
                    index += 1;
                    reason = Some(required_arg(args, index, "--reason")?.to_string());
                }
                "--state" => {
                    index += 1;
                    state = Some(parse_process_state(required_arg(args, index, "--state")?)?);
                }
                "--help" | "-h" => {
                    return Err(AgentOsError::Validation(
                        serde_json::to_string(&usage_json()).unwrap_or_default(),
                    ));
                }
                other => {
                    return Err(AgentOsError::Validation(format!(
                        "unknown process option {other}"
                    )));
                }
            }
            index += 1;
        }
        match action {
            ProcessAction::List => {
                if process_id.is_some() || reason.is_some() {
                    return Err(AgentOsError::Validation(
                        "process list accepts --state but not --process-id or --reason".to_string(),
                    ));
                }
            }
            ProcessAction::Stop | ProcessAction::Kill => {
                if state.is_some() {
                    return Err(AgentOsError::Validation(
                        "--state is only valid for process list".to_string(),
                    ));
                }
                if process_id.is_none() {
                    return Err(AgentOsError::Validation(
                        "--process-id is required for process cleanup".to_string(),
                    ));
                }
            }
        }
        Ok(Self {
            action,
            state_db,
            process_id,
            state,
            reason,
        })
    }
}

fn parse_process_state(value: &str) -> AgentOsResult<ProcessLifecycleState> {
    match value {
        "starting" => Ok(ProcessLifecycleState::Starting),
        "running" => Ok(ProcessLifecycleState::Running),
        "exited" => Ok(ProcessLifecycleState::Exited),
        "failed" => Ok(ProcessLifecycleState::Failed),
        "interrupted" => Ok(ProcessLifecycleState::Interrupted),
        "terminated" => Ok(ProcessLifecycleState::Terminated),
        "timed_out" => Ok(ProcessLifecycleState::TimedOut),
        "orphaned" => Ok(ProcessLifecycleState::Orphaned),
        _ => Err(AgentOsError::Validation(format!(
            "unknown process state {value}"
        ))),
    }
}

impl ResumeOptions {
    pub(crate) fn parse(args: &[String]) -> AgentOsResult<Self> {
        let mut state_db = None;
        let mut thread_id = None;
        let mut workspace = PathBuf::from(".");
        let mut bundle_output = None;
        let mut model_command = None;
        let mut model_args = Vec::new();
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--state-db" => {
                    index += 1;
                    state_db = Some(PathBuf::from(required_arg(args, index, "--state-db")?));
                }
                "--thread-id" => {
                    index += 1;
                    thread_id = Some(required_arg(args, index, "--thread-id")?.to_string());
                }
                "--workspace" => {
                    index += 1;
                    workspace = PathBuf::from(required_arg(args, index, "--workspace")?);
                }
                "--bundle-output" => {
                    index += 1;
                    bundle_output =
                        Some(PathBuf::from(required_arg(args, index, "--bundle-output")?));
                }
                "--model-command" => {
                    index += 1;
                    model_command =
                        Some(PathBuf::from(required_arg(args, index, "--model-command")?));
                }
                "--model-arg" => {
                    index += 1;
                    model_args.push(required_arg(args, index, "--model-arg")?.to_string());
                }
                "--help" | "-h" => {
                    return Err(AgentOsError::Validation(
                        serde_json::to_string(&usage_json()).unwrap_or_default(),
                    ));
                }
                other => {
                    return Err(AgentOsError::Validation(format!(
                        "unknown resume option {other}"
                    )));
                }
            }
            index += 1;
        }
        Ok(Self {
            state_db,
            thread_id: thread_id.ok_or_else(|| {
                AgentOsError::Validation("--thread-id is required for resume".to_string())
            })?,
            workspace,
            bundle_output,
            model_command: model_command.ok_or_else(|| {
                AgentOsError::Validation("--model-command is required for resume".to_string())
            })?,
            model_args,
        })
    }
}

fn required_arg<'a>(args: &'a [String], index: usize, flag: &str) -> AgentOsResult<&'a str> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| AgentOsError::Validation(format!("{flag} requires a value")))
}

fn collect_task_arg(
    args: &[String],
    start_index: usize,
    flag: &str,
) -> AgentOsResult<(String, usize)> {
    let first = required_arg(args, start_index, flag)?;
    if first.starts_with('-') {
        return Err(AgentOsError::Validation(format!(
            "{flag} requires a task value"
        )));
    }

    let mut end_index = start_index;
    while end_index + 1 < args.len() && !args[end_index + 1].starts_with('-') {
        end_index += 1;
    }
    Ok((args[start_index..=end_index].join(" "), end_index))
}

impl ChatOptions {
    pub(crate) fn parse(args: &[String]) -> AgentOsResult<Self> {
        let mut workspace = PathBuf::from(".");
        let mut task = None;
        let mut task_file = None;
        let mut model = None;
        let mut max_steps = 32u32;
        let mut runtime_timeout_seconds = 120u64;
        let mut max_tokens = None;
        let mut temperature = None;
        let mut state_db = None;
        let mut bundle_output = None;
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--workspace" | "-w" => {
                    index += 1;
                    workspace = PathBuf::from(required_arg(args, index, "--workspace")?);
                }
                "--task" | "-t" => {
                    index += 1;
                    let (value, consumed_index) = collect_task_arg(args, index, "--task")?;
                    task = Some(value);
                    index = consumed_index;
                }
                "--task-file" => {
                    index += 1;
                    task_file = Some(PathBuf::from(required_arg(args, index, "--task-file")?));
                }
                "--model" | "-m" => {
                    index += 1;
                    model = Some(required_arg(args, index, "--model")?.to_string());
                }
                "--max-steps" => {
                    index += 1;
                    max_steps =
                        required_arg(args, index, "--max-steps")?
                            .parse()
                            .map_err(|_| {
                                AgentOsError::Validation("--max-steps must be a number".to_string())
                            })?;
                }
                "--runtime-timeout-seconds" => {
                    index += 1;
                    runtime_timeout_seconds =
                        required_arg(args, index, "--runtime-timeout-seconds")?
                            .parse()
                            .map_err(|_| {
                                AgentOsError::Validation(
                                    "--runtime-timeout-seconds must be a number".to_string(),
                                )
                            })?;
                }
                "--max-tokens" => {
                    index += 1;
                    max_tokens = Some(required_arg(args, index, "--max-tokens")?.parse().map_err(
                        |_| AgentOsError::Validation("--max-tokens must be a number".to_string()),
                    )?);
                }
                "--temperature" => {
                    index += 1;
                    temperature = Some(
                        required_arg(args, index, "--temperature")?
                            .parse()
                            .map_err(|_| {
                                AgentOsError::Validation(
                                    "--temperature must be a number".to_string(),
                                )
                            })?,
                    );
                }
                "--state-db" => {
                    index += 1;
                    state_db = Some(PathBuf::from(required_arg(args, index, "--state-db")?));
                }
                "--bundle-output" => {
                    index += 1;
                    bundle_output =
                        Some(PathBuf::from(required_arg(args, index, "--bundle-output")?));
                }
                "--help" | "-h" => {
                    return Err(AgentOsError::Validation(
                        serde_json::to_string(&usage_json()).unwrap_or_default(),
                    ));
                }
                other if !other.starts_with('-') && task.is_none() => {
                    let (value, consumed_index) = collect_task_arg(args, index, "<task>")?;
                    task = Some(value);
                    index = consumed_index;
                }
                other => {
                    return Err(AgentOsError::Validation(format!(
                        "unknown chat option {other}"
                    )));
                }
            }
            index += 1;
        }
        if task.is_some() && task_file.is_some() {
            return Err(AgentOsError::Validation(
                "--task and --task-file cannot be used together".to_string(),
            ));
        }
        Ok(Self {
            workspace,
            task,
            task_file,
            model,
            max_steps,
            runtime_timeout_seconds,
            max_tokens,
            temperature,
            state_db,
            bundle_output,
        })
    }
}

pub(crate) fn usage_json() -> Value {
    json!({
        "commands": {
            "chat": "Interactive coding agent powered by a configured provider model.",
            "run": "Run a deterministic end-to-end Agent-OS task.",
            "code": "Apply an exact repository edit and run a test command through Agent-OS.",
            "status": "Inspect a SQLite-backed Agent-OS state database.",
            "process": "List, stop, or kill kernel-owned Agent-OS process sessions.",
            "resume": "Resume an existing Agent Thread from a SQLite-backed state database."
        },
        "chat_options": {
            "--workspace, -w": "Workspace directory. Default: .",
            "--task, -t": "Initial batch task to run before exiting.",
            "--task-file": "Read initial batch task text from a UTF-8 file before exiting.",
            "--model, -m": "Model id in provider/model form. Defaults to the global/project Agent-OS config model.",
            "--max-steps": "Maximum agent steps per task. Default: 32",
            "--runtime-timeout-seconds": "Maximum wall-clock seconds to wait for the runtime job. Default: 120",
            "--max-tokens": "Maximum output tokens per model call.",
            "--temperature": "Model temperature (0.0 = deterministic). Default: 0.0",
            "--state-db": "Optional SQLite event store path for durable replay.",
            "--bundle-output": "Optional relative path for selected task bundle JSON inside workspace.",
            "<task>": "Positional: initial batch task text."
        },
        "run_options": {
            "--workspace": "Workspace directory. Default: .",
            "--task": "Task description. Default: Create an Agent-OS task result",
            "--output": "Relative output path inside workspace. Default: agent-os-task-result.md",
            "--bundle-output": "Optional relative path for selected task bundle JSON inside workspace.",
            "--state-db": "Optional SQLite event store path for durable replay across process restarts.",
            "--model-command": "External model action process. It receives ModelTurnRequest JSON on stdin and emits ModelTurnResponse JSON on stdout.",
            "--model-arg": "One argument for the external model action process. Repeatable."
        },
        "code_options": {
            "--workspace": "Workspace directory. Default: .",
            "--task": "Task description. Default: Apply exact repository edit",
            "--file": "Optional relative target file path. Required only for exact edit mode.",
            "--old": "Exact text to replace. Requires --file and --new.",
            "--new": "Replacement text. Requires --file and --old.",
            "--test-program": "Test executable. Default: current agent-os executable.",
            "--test-arg": "One argument for the test executable. Repeatable. Default: --help",
            "--bundle-output": "Optional relative path for selected task bundle JSON inside workspace.",
            "--state-db": "Optional SQLite event store path for durable replay across process restarts.",
            "--model-command": "Required external model action process. It receives ModelTurnRequest JSON on stdin and emits ModelTurnResponse JSON on stdout.",
            "--model-arg": "One argument for the external model action process. Repeatable."
        },
        "status_options": {
            "--state-db": "Optional SQLite event store path. Defaults to the global Agent-OS state store.",
            "--thread-id": "Optional thread id to inspect."
        },
        "process_options": {
            "list": "List process sessions, optionally filtered by state.",
            "stop": "Interrupt a running process session.",
            "kill": "Terminate a running process session.",
            "--state-db": "Optional SQLite event store path. Defaults to the global Agent-OS state store.",
            "--process-id": "Required process session id for stop and kill.",
            "--state": "Optional process list state filter: starting, running, exited, failed, interrupted, terminated, timed_out, or orphaned.",
            "--reason": "Optional cleanup reason recorded on the process session."
        },
        "resume_options": {
            "--state-db": "Optional SQLite event store path. Defaults to the global Agent-OS state store.",
            "--thread-id": "Required thread id to resume.",
            "--workspace": "Workspace directory for resumed tool execution. Default: .",
            "--bundle-output": "Optional relative path for selected task bundle JSON inside workspace.",
            "--model-command": "Required external model action process.",
            "--model-arg": "One argument for the external model action process. Repeatable."
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| part.to_string()).collect()
    }

    #[test]
    fn chat_task_flag_collects_adjacent_words_until_next_option() {
        let options = ChatOptions::parse(&argv(&[
            "--task",
            "Frontend",
            "smoke",
            "task",
            "--max-steps",
            "8",
        ]))
        .unwrap();

        assert_eq!(options.task.as_deref(), Some("Frontend smoke task"));
        assert_eq!(options.max_steps, 8);
    }

    #[test]
    fn chat_positional_task_collects_adjacent_words_until_next_option() {
        let options = ChatOptions::parse(&argv(&[
            "Backend",
            "smoke",
            "task",
            "--model",
            "mify/coder",
        ]))
        .unwrap();

        assert_eq!(options.task.as_deref(), Some("Backend smoke task"));
        assert_eq!(options.model.as_deref(), Some("mify/coder"));
    }

    #[test]
    fn chat_task_file_sets_initial_task_path() {
        let options = ChatOptions::parse(&argv(&["--task-file", "SWE_BENCH_TASK.md"])).unwrap();

        assert_eq!(options.task_file, Some(PathBuf::from("SWE_BENCH_TASK.md")));
        assert_eq!(options.task, None);
    }

    #[test]
    fn chat_runtime_timeout_seconds_is_configurable() {
        let options = ChatOptions::parse(&argv(&["--runtime-timeout-seconds", "3600"])).unwrap();

        assert_eq!(options.runtime_timeout_seconds, 3600);
    }

    #[test]
    fn process_stop_requires_process_id() {
        let error = ProcessOptions::parse(&argv(&["stop"])).unwrap_err();

        assert!(error.to_string().contains("--process-id"));
    }

    #[test]
    fn process_list_options_parse_state_filter() {
        let options = ProcessOptions::parse(&argv(&[
            "list",
            "--state",
            "running",
            "--state-db",
            "state.sqlite",
        ]))
        .unwrap();

        assert_eq!(options.action, ProcessAction::List);
        assert_eq!(options.state, Some(ProcessLifecycleState::Running));
        assert_eq!(options.process_id, None);
        assert_eq!(options.state_db, Some(PathBuf::from("state.sqlite")));
    }

    #[test]
    fn process_kill_options_parse_cleanup_request() {
        let options = ProcessOptions::parse(&argv(&[
            "kill",
            "--process-id",
            "proc_1",
            "--reason",
            "test cleanup",
            "--state-db",
            "state.sqlite",
        ]))
        .unwrap();

        assert_eq!(options.action, ProcessAction::Kill);
        assert_eq!(options.process_id.as_deref(), Some("proc_1"));
        assert_eq!(options.reason.as_deref(), Some("test cleanup"));
        assert_eq!(options.state_db, Some(PathBuf::from("state.sqlite")));
    }

    #[test]
    fn chat_rejects_task_text_and_task_file_together() {
        let error = ChatOptions::parse(&argv(&[
            "--task",
            "inline",
            "task",
            "--task-file",
            "SWE_BENCH_TASK.md",
        ]))
        .unwrap_err();

        assert!(error.to_string().contains("--task and --task-file"));
    }

    #[test]
    fn run_task_flag_collects_adjacent_words_until_next_option() {
        let options = RunOptions::parse(&argv(&[
            "--task",
            "Write",
            "task",
            "report",
            "--output",
            "result.md",
        ]))
        .unwrap();

        assert_eq!(options.task, "Write task report");
        assert_eq!(options.output, PathBuf::from("result.md"));
    }

    #[test]
    fn code_task_flag_collects_adjacent_words_until_next_option() {
        let options = CodeOptions::parse(&argv(&[
            "--task",
            "Change",
            "one",
            "snippet",
            "--file",
            "README.md",
            "--old",
            "old",
            "--new",
            "new",
        ]))
        .unwrap();

        assert_eq!(options.task, "Change one snippet");
        assert_eq!(options.file, Some(PathBuf::from("README.md")));
    }
}
