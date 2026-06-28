mod audit;
mod client;
mod messages;
mod parser;
mod prompt;
mod tools;

pub use client::{LlmApiStyle, OpenAiModelClient};

#[cfg(test)]
pub(super) use audit::append_jsonl;
#[cfg(test)]
pub(super) use messages::{build_anthropic_messages, build_messages};
#[cfg(test)]
pub(super) use parser::{map_function_call, parse_anthropic_response, parse_response};
#[cfg(test)]
pub(super) use prompt::default_system_prompt;
#[cfg(test)]
pub(super) use tools::{anthropic_tool_definitions, tool_definitions};
#[cfg(test)]
pub(super) use {
    crate::{ModelAction, ModelTurnRequest, ToolExecutionRecord},
    agent_os_sys::*,
    serde_json::{json, Value},
    std::path::Path,
};

#[cfg(test)]
#[path = "openai/tests.rs"]
mod tests;
