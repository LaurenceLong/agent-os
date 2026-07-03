use crate::{empty_object, ToolDescriptor};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EcosystemSourceKind {
    AgentOs,
    Claude,
    Agents,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EcosystemSourceScope {
    Project,
    Global,
    Config,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTransportKind {
    LocalStdio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportedAgentMode {
    Primary,
    Subagent,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemSource {
    pub source_kind: EcosystemSourceKind,
    pub source_scope: EcosystemSourceScope,
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionDocument {
    pub instruction_id: String,
    pub source: EcosystemSource,
    pub precedence_rank: u32,
    pub content: String,
    pub content_hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDefinition {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    pub root_path: String,
    pub skill_file_path: String,
    pub source: EcosystemSource,
    pub content: String,
    pub metadata: BTreeMap<String, String>,
    pub content_hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDefinition {
    pub command_id: String,
    pub name: String,
    pub description: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub template: String,
    pub argument_hints: Vec<String>,
    pub source: EcosystemSource,
    pub content_hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerSpec {
    pub server_id: String,
    pub name: String,
    pub transport: McpTransportKind,
    pub command: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub enabled: bool,
    pub timeout_ms: u64,
    pub source: EcosystemSource,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    pub mcp_tool_id: String,
    pub server_name: String,
    pub tool_name: String,
    pub model_tool_name: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub source: EcosystemSource,
    pub tool_descriptor: ToolDescriptor,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedAgentProfile {
    pub imported_agent_profile_id: String,
    pub name: String,
    pub description: Option<String>,
    pub mode: ImportedAgentMode,
    pub prompt: String,
    pub model: Option<String>,
    pub role_profile_id: Option<String>,
    pub permission_profile_id: Option<String>,
    pub source: EcosystemSource,
    pub content_hash: String,
    #[serde(default = "empty_object")]
    pub metadata: Value,
    pub created_at: String,
}

pub fn mcp_model_tool_name(server_name: &str, tool_name: &str) -> String {
    format!(
        "mcp__{}__{}",
        sanitize_ecosystem_identifier(server_name),
        sanitize_ecosystem_identifier(tool_name)
    )
}

pub fn sanitize_ecosystem_identifier(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_underscore = false;
    for ch in value.chars() {
        let next = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '_'
        };
        if next == '_' {
            if !last_was_underscore && !out.is_empty() {
                out.push(next);
            }
            last_was_underscore = true;
        } else {
            out.push(next);
            last_was_underscore = false;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        "unnamed".to_string()
    } else {
        out
    }
}
