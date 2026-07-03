mod args;
mod chat;
mod code;
mod process;
mod resume;
mod run;
mod status;
mod support;

use agent_os_sys::{AgentOsError, AgentOsResult};
use args::{
    usage_json, ChatOptions, CodeOptions, ProcessOptions, ResumeOptions, RunOptions, StatusOptions,
};
use serde_json::Value;
use std::env;

fn main() -> AgentOsResult<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let output = dispatch(&args)?;
    if !args.first().map(|a| a == "chat").unwrap_or(false) {
        println!("{}", serde_json::to_string_pretty(&output)?);
    }
    Ok(())
}

fn dispatch(args: &[String]) -> AgentOsResult<Value> {
    match args.first().map(String::as_str) {
        None | Some("--help") | Some("-h") | Some("help") => Ok(usage_json()),
        Some("chat") => chat::run_chat(&ChatOptions::parse(&args[1..])?),
        Some("run") => run::run_e2e_task(&RunOptions::parse(&args[1..])?),
        Some("code") => code::run_code_task(&CodeOptions::parse(&args[1..])?),
        Some("status") => status::run_status(&StatusOptions::parse(&args[1..])?),
        Some("process") => process::run_process(&ProcessOptions::parse(&args[1..])?),
        Some("resume") => resume::run_resume(&ResumeOptions::parse(&args[1..])?),
        Some(other) => Err(AgentOsError::Validation(format!(
            "unknown command {other}; use `agent-os help`"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_invocation_returns_usage_without_demo_command() {
        let output = dispatch(&[]).unwrap();

        assert!(output["commands"]["chat"].is_string());
        assert!(output["commands"]["demo"].is_null());
    }
}
