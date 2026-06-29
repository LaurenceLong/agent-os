use agent_os_kernel::KernelState;
use agent_os_sys::{
    CommandDefinition, ImportedAgentProfile, InstructionDocument, McpToolDefinition,
    SkillDefinition, ToolDescriptor,
};

pub(super) struct EcosystemProjection {
    pub tool_descriptors: Vec<ToolDescriptor>,
    pub instruction_documents: Vec<InstructionDocument>,
    pub skill_definitions: Vec<SkillDefinition>,
    pub command_definitions: Vec<CommandDefinition>,
    pub mcp_tools: Vec<McpToolDefinition>,
    pub imported_agent_profiles: Vec<ImportedAgentProfile>,
}

pub(super) fn from_state(state: &KernelState) -> EcosystemProjection {
    let mut tool_descriptors: Vec<_> = state.tool_descriptors.values().cloned().collect();
    tool_descriptors.sort_by(|left, right| left.name.cmp(&right.name));

    let mut instruction_documents: Vec<_> = state.instruction_documents.values().cloned().collect();
    instruction_documents.sort_by(|left, right| {
        left.precedence_rank
            .cmp(&right.precedence_rank)
            .then_with(|| left.source.source_path.cmp(&right.source.source_path))
            .then_with(|| left.instruction_id.cmp(&right.instruction_id))
    });

    let mut skill_definitions: Vec<_> = state.skill_definitions.values().cloned().collect();
    skill_definitions.sort_by(|left, right| left.name.cmp(&right.name));

    let mut command_definitions: Vec<_> = state.command_definitions.values().cloned().collect();
    command_definitions.sort_by(|left, right| left.name.cmp(&right.name));

    let mut mcp_tools: Vec<_> = state.mcp_tools.values().cloned().collect();
    mcp_tools.sort_by(|left, right| {
        left.server_name
            .cmp(&right.server_name)
            .then_with(|| left.tool_name.cmp(&right.tool_name))
    });

    let mut imported_agent_profiles: Vec<_> =
        state.imported_agent_profiles.values().cloned().collect();
    imported_agent_profiles.sort_by(|left, right| left.name.cmp(&right.name));

    EcosystemProjection {
        tool_descriptors,
        instruction_documents,
        skill_definitions,
        command_definitions,
        mcp_tools,
        imported_agent_profiles,
    }
}
