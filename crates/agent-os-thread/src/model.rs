use agent_os_sys::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

pub trait ModelClient {
    fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTurnRequest {
    pub thread: AgentControlBlock,
    pub workspace_root: PathBuf,
    pub step_index: u32,
    pub model_capabilities: ModelCapabilities,
    pub context: ModelContextProjection,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelContextProjection {
    pub tool_results: Vec<ToolExecutionRecord>,
    pub artifacts: Vec<ArtifactRecord>,
    pub context_snapshots: Vec<ContextSnapshot>,
    pub memory_records: Vec<MemoryRecord>,
    pub context_compactions: Vec<ContextCompaction>,
    pub mementos: Vec<MementoFragment>,
    pub tool_plan: ToolPlan,
    pub tool_descriptors: Vec<ToolDescriptor>,
    pub instruction_documents: Vec<InstructionDocument>,
    pub skill_definitions: Vec<SkillDefinition>,
    pub command_definitions: Vec<CommandDefinition>,
    pub mcp_tools: Vec<McpToolDefinition>,
    pub mcp_resources: Vec<McpResourceDefinition>,
    pub mcp_resource_templates: Vec<McpResourceTemplateDefinition>,
    pub imported_agent_profiles: Vec<ImportedAgentProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTurnResponse {
    pub actions: Vec<ModelAction>,
    pub usage: ProviderUsage,
}

impl ModelTurnResponse {
    pub fn single(action: ModelAction) -> Self {
        Self {
            actions: vec![action],
            usage: ProviderUsage::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelAction {
    OutputText { text: String },
    ToolCall(ToolAction),
    Final { submission: FinalSubmission },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAction {
    pub tool_name: String,
    pub input: Value,
    pub risk_level: u8,
    pub evidence_claim: Option<String>,
}

impl ToolAction {
    pub fn new(
        tool_name: impl Into<String>,
        input: Value,
        risk_level: u8,
        evidence_claim: Option<String>,
    ) -> Self {
        Self {
            tool_name: tool_name.into(),
            input,
            risk_level,
            evidence_claim,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionRecord {
    pub call_id: String,
    pub tool_name: String,
    pub status: ToolCallStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    pub output: Option<Value>,
    pub evidence_ids: Vec<String>,
    pub evidence_claim: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub artifact_id: String,
    pub artifact_type: ArtifactType,
    pub blob_ref: Option<String>,
    pub evidence_ids: Vec<String>,
}
