use agent_os_sys::ProcessLifecycleState;
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
#[command(name = "agent-os", version, about = "Agent-OS command line and TUI")]
pub(crate) struct Cli {
    #[arg(value_name = "WORKSPACE")]
    pub(crate) workspace: Option<PathBuf>,

    #[command(subcommand)]
    pub(crate) command: Option<CliCommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum CliCommand {
    Tui(TuiOptions),
    Chat(ChatOptions),
    Run(RunOptions),
    Code(CodeOptions),
    Status(StatusOptions),
    Process {
        #[command(subcommand)]
        command: ProcessCommand,
    },
    Resume(ResumeOptions),
    Host {
        #[command(subcommand)]
        command: HostCommand,
    },
}

#[derive(Debug, Clone, Args)]
pub(crate) struct TuiOptions {
    #[arg(value_name = "WORKSPACE")]
    pub(crate) workspace: Option<PathBuf>,
    #[arg(long)]
    pub(crate) thread: Option<String>,
    #[arg(long)]
    pub(crate) resume: Option<String>,
    #[arg(long, short = 'm')]
    pub(crate) model: Option<String>,
    #[arg(long)]
    pub(crate) profile: Option<String>,
    #[arg(long)]
    pub(crate) state_db: Option<PathBuf>,
    #[arg(long)]
    pub(crate) max_steps: Option<u32>,
    #[arg(long)]
    pub(crate) max_tokens: Option<u64>,
    #[arg(long)]
    pub(crate) temperature: Option<f64>,
    #[arg(long)]
    pub(crate) no_alt_screen: bool,
}

impl TuiOptions {
    pub(crate) fn default_workspace(workspace: Option<PathBuf>) -> Self {
        Self {
            workspace,
            thread: None,
            resume: None,
            model: None,
            profile: None,
            state_db: None,
            max_steps: None,
            max_tokens: None,
            temperature: None,
            no_alt_screen: false,
        }
    }
}

impl From<TuiOptions> for agent_os_tui::TuiOptions {
    fn from(options: TuiOptions) -> Self {
        Self {
            workspace: options.workspace,
            thread: options.thread,
            resume: options.resume,
            model: options.model,
            profile: options.profile,
            state_db: options.state_db,
            max_steps: options.max_steps,
            max_tokens: options.max_tokens,
            temperature: options.temperature,
            no_alt_screen: options.no_alt_screen,
        }
    }
}

#[derive(Debug, Clone, Args)]
pub(crate) struct RunOptions {
    #[arg(long, default_value = ".")]
    pub(crate) workspace: PathBuf,
    #[arg(value_name = "PROMPT", num_args = 0..)]
    pub(crate) prompt: Vec<String>,
    #[arg(long, default_value = "agent-os-task-result.md")]
    pub(crate) output: PathBuf,
    #[arg(long)]
    pub(crate) bundle_output: Option<PathBuf>,
    #[arg(long)]
    pub(crate) state_db: Option<PathBuf>,
    #[arg(long)]
    pub(crate) model_command: Option<PathBuf>,
    #[arg(long = "model-arg", allow_hyphen_values = true)]
    pub(crate) model_args: Vec<String>,
}

impl RunOptions {
    pub(crate) fn task_text(&self) -> String {
        joined_or_default(&self.prompt, "Create an Agent-OS task result")
    }
}

#[derive(Debug, Clone, Args)]
pub(crate) struct CodeOptions {
    #[arg(long, default_value = ".")]
    pub(crate) workspace: PathBuf,
    #[arg(long, num_args = 1..)]
    pub(crate) task: Vec<String>,
    #[arg(long)]
    pub(crate) file: Option<PathBuf>,
    #[arg(long)]
    pub(crate) old: Option<String>,
    #[arg(long)]
    pub(crate) new: Option<String>,
    #[arg(long)]
    pub(crate) test_program: Option<PathBuf>,
    #[arg(long = "test-arg", allow_hyphen_values = true)]
    pub(crate) test_args: Vec<String>,
    #[arg(long)]
    pub(crate) bundle_output: Option<PathBuf>,
    #[arg(long)]
    pub(crate) state_db: Option<PathBuf>,
    #[arg(long)]
    pub(crate) model_command: Option<PathBuf>,
    #[arg(long = "model-arg", allow_hyphen_values = true)]
    pub(crate) model_args: Vec<String>,
}

impl CodeOptions {
    pub(crate) fn task_text(&self) -> String {
        joined_or_default(&self.task, "Apply exact repository edit")
    }

    pub(crate) fn test_program(&self) -> std::io::Result<PathBuf> {
        self.test_program
            .clone()
            .map(Ok)
            .unwrap_or_else(std::env::current_exe)
    }

    pub(crate) fn test_args(&self) -> Vec<String> {
        if self.test_args.is_empty() {
            vec!["--help".to_string()]
        } else {
            self.test_args.clone()
        }
    }
}

#[derive(Debug, Clone, Args)]
pub(crate) struct StatusOptions {
    #[arg(long)]
    pub(crate) state_db: Option<PathBuf>,
    #[arg(value_name = "THREAD_ID")]
    pub(crate) thread_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcessAction {
    List,
    Stop,
    Kill,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum ProcessCommand {
    List(ProcessListOptions),
    Stop(ProcessCleanupOptions),
    Kill(ProcessCleanupOptions),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ProcessListOptions {
    #[arg(long)]
    pub(crate) state_db: Option<PathBuf>,
    #[arg(long, value_enum)]
    pub(crate) state: Option<ProcessStateArg>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ProcessCleanupOptions {
    #[arg(long)]
    pub(crate) state_db: Option<PathBuf>,
    #[arg(value_name = "PROCESS_ID")]
    pub(crate) process_id: String,
    #[arg(long)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProcessOptions {
    pub(crate) action: ProcessAction,
    pub(crate) state_db: Option<PathBuf>,
    pub(crate) process_id: Option<String>,
    pub(crate) state: Option<ProcessLifecycleState>,
    pub(crate) reason: Option<String>,
}

impl From<ProcessCommand> for ProcessOptions {
    fn from(command: ProcessCommand) -> Self {
        match command {
            ProcessCommand::List(options) => Self {
                action: ProcessAction::List,
                state_db: options.state_db,
                process_id: None,
                state: options.state.map(Into::into),
                reason: None,
            },
            ProcessCommand::Stop(options) => Self {
                action: ProcessAction::Stop,
                state_db: options.state_db,
                process_id: Some(options.process_id),
                state: None,
                reason: options.reason,
            },
            ProcessCommand::Kill(options) => Self {
                action: ProcessAction::Kill,
                state_db: options.state_db,
                process_id: Some(options.process_id),
                state: None,
                reason: options.reason,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ProcessStateArg {
    Starting,
    Running,
    Exited,
    Failed,
    Interrupted,
    Terminated,
    TimedOut,
    Orphaned,
}

impl From<ProcessStateArg> for ProcessLifecycleState {
    fn from(state: ProcessStateArg) -> Self {
        match state {
            ProcessStateArg::Starting => ProcessLifecycleState::Starting,
            ProcessStateArg::Running => ProcessLifecycleState::Running,
            ProcessStateArg::Exited => ProcessLifecycleState::Exited,
            ProcessStateArg::Failed => ProcessLifecycleState::Failed,
            ProcessStateArg::Interrupted => ProcessLifecycleState::Interrupted,
            ProcessStateArg::Terminated => ProcessLifecycleState::Terminated,
            ProcessStateArg::TimedOut => ProcessLifecycleState::TimedOut,
            ProcessStateArg::Orphaned => ProcessLifecycleState::Orphaned,
        }
    }
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ResumeOptions {
    #[arg(long)]
    pub(crate) state_db: Option<PathBuf>,
    #[arg(value_name = "THREAD_ID")]
    pub(crate) thread_id: String,
    #[arg(long, default_value = ".")]
    pub(crate) workspace: PathBuf,
    #[arg(long)]
    pub(crate) bundle_output: Option<PathBuf>,
    #[arg(long)]
    pub(crate) model_command: PathBuf,
    #[arg(long = "model-arg", allow_hyphen_values = true)]
    pub(crate) model_args: Vec<String>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ChatOptions {
    #[arg(long, short = 'w', default_value = ".")]
    pub(crate) workspace: PathBuf,
    #[arg(long, short = 't', conflicts_with = "task_file")]
    pub(crate) task: Option<String>,
    #[arg(long, conflicts_with = "task")]
    pub(crate) task_file: Option<PathBuf>,
    #[arg(long, short = 'm')]
    pub(crate) model: Option<String>,
    #[arg(long, default_value_t = 32)]
    pub(crate) max_steps: u32,
    #[arg(long, default_value_t = 120)]
    pub(crate) runtime_timeout_seconds: u64,
    #[arg(long)]
    pub(crate) max_tokens: Option<u64>,
    #[arg(long)]
    pub(crate) temperature: Option<f64>,
    #[arg(long)]
    pub(crate) state_db: Option<PathBuf>,
    #[arg(long)]
    pub(crate) bundle_output: Option<PathBuf>,
    #[arg(value_name = "TASK", num_args = 0..)]
    pub(crate) positional_task: Vec<String>,
}

impl ChatOptions {
    pub(crate) fn task_text(&self) -> Option<String> {
        self.task
            .clone()
            .or_else(|| (!self.positional_task.is_empty()).then(|| self.positional_task.join(" ")))
    }
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum HostCommand {
    Serve(HostServeOptions),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct HostServeOptions {
    #[arg(long)]
    pub(crate) stdio: bool,
    #[arg(long)]
    pub(crate) state_db: Option<PathBuf>,
    #[arg(long)]
    pub(crate) model: Option<String>,
    #[arg(long)]
    pub(crate) provider_config: Option<PathBuf>,
    #[arg(long)]
    pub(crate) max_steps: Option<u32>,
    #[arg(long)]
    pub(crate) max_tokens: Option<u64>,
    #[arg(long)]
    pub(crate) temperature: Option<String>,
}

fn joined_or_default(parts: &[String], default: &str) -> String {
    if parts.is_empty() {
        default.to_string()
    } else {
        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn parse(parts: &[&str]) -> Cli {
        Cli::parse_from(std::iter::once("agent-os").chain(parts.iter().copied()))
    }

    #[test]
    fn run_collects_positional_prompt() {
        let cli = parse(&["run", "Write", "task", "report", "--output", "result.md"]);

        let Some(CliCommand::Run(options)) = cli.command else {
            panic!("expected run command");
        };
        assert_eq!(options.task_text(), "Write task report");
        assert_eq!(options.output, PathBuf::from("result.md"));
    }

    #[test]
    fn chat_task_file_sets_initial_task_path() {
        let cli = parse(&["chat", "--task-file", "SWE_BENCH_TASK.md"]);

        let Some(CliCommand::Chat(options)) = cli.command else {
            panic!("expected chat command");
        };
        assert_eq!(options.task_file, Some(PathBuf::from("SWE_BENCH_TASK.md")));
        assert_eq!(options.task_text(), None);
    }

    #[test]
    fn chat_positional_task_collects_words() {
        let cli = parse(&["chat", "Backend", "smoke", "task", "--model", "mify/coder"]);

        let Some(CliCommand::Chat(options)) = cli.command else {
            panic!("expected chat command");
        };
        assert_eq!(options.task_text().as_deref(), Some("Backend smoke task"));
        assert_eq!(options.model.as_deref(), Some("mify/coder"));
    }

    #[test]
    fn process_list_options_parse_state_filter() {
        let cli = parse(&[
            "process",
            "list",
            "--state",
            "running",
            "--state-db",
            "state.sqlite",
        ]);

        let Some(CliCommand::Process { command }) = cli.command else {
            panic!("expected process command");
        };
        let options = ProcessOptions::from(command);
        assert_eq!(options.action, ProcessAction::List);
        assert_eq!(options.state, Some(ProcessLifecycleState::Running));
        assert_eq!(options.state_db, Some(PathBuf::from("state.sqlite")));
    }

    #[test]
    fn process_kill_options_parse_cleanup_request() {
        let cli = parse(&[
            "process",
            "kill",
            "proc_1",
            "--reason",
            "test cleanup",
            "--state-db",
            "state.sqlite",
        ]);

        let Some(CliCommand::Process { command }) = cli.command else {
            panic!("expected process command");
        };
        let options = ProcessOptions::from(command);
        assert_eq!(options.action, ProcessAction::Kill);
        assert_eq!(options.process_id.as_deref(), Some("proc_1"));
        assert_eq!(options.reason.as_deref(), Some("test cleanup"));
        assert_eq!(options.state_db, Some(PathBuf::from("state.sqlite")));
    }

    #[test]
    fn code_task_flag_collects_adjacent_words() {
        let cli = parse(&[
            "code",
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
        ]);

        let Some(CliCommand::Code(options)) = cli.command else {
            panic!("expected code command");
        };
        assert_eq!(options.task_text(), "Change one snippet");
        assert_eq!(options.file, Some(PathBuf::from("README.md")));
    }
}
