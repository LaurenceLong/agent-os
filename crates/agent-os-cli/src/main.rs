mod args;
mod chat;
mod code;
mod demo;
mod resume;
mod run;
mod status;
mod support;

use agent_os_sys::{AgentOsError, AgentOsResult};
use args::{usage_json, ChatOptions, CodeOptions, ResumeOptions, RunOptions, StatusOptions};
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
        None | Some("demo") => demo::run_demo(),
        Some("chat") => chat::run_chat(&ChatOptions::parse(&args[1..])?),
        Some("run") => run::run_e2e_task(&RunOptions::parse(&args[1..])?),
        Some("code") => code::run_code_task(&CodeOptions::parse(&args[1..])?),
        Some("status") => status::run_status(&StatusOptions::parse(&args[1..])?),
        Some("resume") => resume::run_resume(&ResumeOptions::parse(&args[1..])?),
        Some("--help") | Some("-h") | Some("help") => Ok(usage_json()),
        Some(other) => Err(AgentOsError::Validation(format!(
            "unknown command {other}; use `agent-os help`"
        ))),
    }
}
