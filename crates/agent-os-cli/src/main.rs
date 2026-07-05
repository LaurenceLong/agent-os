mod args;
mod chat;
mod code;
mod process;
mod resume;
mod run;
mod status;
mod support;

use agent_os_sys::{AgentOsError, AgentOsResult};
use args::{Cli, CliCommand, ProcessOptions};
use clap::Parser;
use serde_json::{json, Value};
use std::io::{self, BufReader};

fn main() -> AgentOsResult<()> {
    let cli = Cli::parse();
    let print_output = should_print_output(&cli.command);
    let output = dispatch_cli(cli)?;
    if print_output {
        println!("{}", serde_json::to_string_pretty(&output)?);
    }
    Ok(())
}

fn dispatch_cli(cli: Cli) -> AgentOsResult<Value> {
    match cli.command {
        None => run_tui(args::TuiOptions::default_workspace(cli.workspace)),
        Some(CliCommand::Tui(options)) => run_tui(options),
        Some(CliCommand::Chat(options)) => chat::run_chat(&options),
        Some(CliCommand::Run(options)) => run::run_e2e_task(&options),
        Some(CliCommand::Code(options)) => code::run_code_task(&options),
        Some(CliCommand::Status(options)) => status::run_status(&options),
        Some(CliCommand::Process { command }) => {
            process::run_process(&ProcessOptions::from(command))
        }
        Some(CliCommand::Resume(options)) => resume::run_resume(&options),
        Some(CliCommand::Host { command }) => run_host(command),
    }
}

fn should_print_output(command: &Option<CliCommand>) -> bool {
    !matches!(
        command,
        Some(CliCommand::Chat(_)) | Some(CliCommand::Tui(_)) | Some(CliCommand::Host { .. }) | None
    )
}

fn run_tui(options: args::TuiOptions) -> AgentOsResult<Value> {
    let report = agent_os_tui::run_tui(options.into())?;
    Ok(json!({
        "last_thread_id": report.last_thread_id,
        "submitted_turns": report.submitted_turns,
        "final_status": report.final_status,
    }))
}

fn run_host(command: args::HostCommand) -> AgentOsResult<Value> {
    match command {
        args::HostCommand::Serve(options) => {
            if !options.stdio {
                return Err(AgentOsError::Validation(
                    "agent-os host serve requires --stdio".to_string(),
                ));
            }
            let state_db = options.state_db.ok_or_else(|| {
                AgentOsError::Validation("agent-os host serve requires --state-db".to_string())
            })?;
            let runtime_model_config = if options.model.is_some()
                || options.provider_config.is_some()
                || options.max_tokens.is_some()
                || options.temperature.is_some()
            {
                Some(agent_os_host::HostRuntimeModelConfig::Provider(
                    agent_os_host::ProviderRuntimeModelConfig {
                        model: options.model,
                        config_path: options.provider_config,
                        max_steps: options.max_steps.unwrap_or(16),
                        max_tokens: options.max_tokens,
                        temperature: options.temperature,
                    },
                ))
            } else {
                None
            };
            let host_args = agent_os_host::HostArgs {
                state_db,
                runtime_model_config,
            };
            let stdin = io::stdin();
            let stdout = io::stdout();
            agent_os_host::serve_stdio_host(
                host_args,
                BufReader::new(stdin.lock()),
                stdout.lock(),
            )?;
            Ok(json!({"status": "completed"}))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clap_treats_unknown_bare_argument_as_default_workspace() {
        let cli = Cli::parse_from(["agent-os", "workspace"]);

        assert_eq!(cli.workspace, Some(std::path::PathBuf::from("workspace")));
        assert!(cli.command.is_none());
    }

    #[test]
    fn host_stdio_command_does_not_print_protocol_trailer() {
        let cli = Cli::parse_from([
            "agent-os",
            "host",
            "serve",
            "--stdio",
            "--state-db",
            "state.sqlite",
        ]);

        assert!(!should_print_output(&cli.command));
    }
}
