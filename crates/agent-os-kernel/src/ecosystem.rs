use crate::schema::validate_json_schema;
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

impl Kernel {
    pub fn import_instruction_document(
        &self,
        document: InstructionDocument,
    ) -> AgentOsResult<InstructionDocument> {
        if document.instruction_id.is_empty()
            || document.source.source_path.is_empty()
            || document.content_hash.is_empty()
        {
            return Err(AgentOsError::Validation(
                "instruction document requires id, source path, and content hash".to_string(),
            ));
        }
        self.emit(
            "InstructionDocumentImported",
            "instruction_document",
            &document.instruction_id,
            None,
            None,
            None,
            None,
            &document,
        )?;
        Ok(document)
    }

    pub fn import_skill_definition(
        &self,
        skill: SkillDefinition,
    ) -> AgentOsResult<SkillDefinition> {
        validate_skill_definition(&skill)?;
        if let Some(existing) = self.read_state()?.skill_definitions.get(&skill.name) {
            if existing.content_hash != skill.content_hash
                || existing.description != skill.description
            {
                return Err(AgentOsError::Validation(format!(
                    "duplicate skill name {} from {} and {}",
                    skill.name, existing.skill_file_path, skill.skill_file_path
                )));
            }
            return Ok(existing.clone());
        }
        self.emit(
            "SkillDefinitionImported",
            "skill_definition",
            &skill.skill_id,
            None,
            None,
            None,
            None,
            &skill,
        )?;
        Ok(skill)
    }

    pub fn import_command_definition(
        &self,
        command: CommandDefinition,
    ) -> AgentOsResult<CommandDefinition> {
        validate_command_definition(&command)?;
        if let Some(existing) = self.read_state()?.command_definitions.get(&command.name) {
            if existing.command_id != command.command_id {
                return Err(AgentOsError::Validation(format!(
                    "duplicate command name {} from {} and {}",
                    command.name, existing.source.source_path, command.source.source_path
                )));
            }
        }
        self.emit(
            "CommandDefinitionImported",
            "command_definition",
            &command.command_id,
            None,
            None,
            None,
            None,
            &command,
        )?;
        Ok(command)
    }

    pub fn register_mcp_server_spec(&self, server: McpServerSpec) -> AgentOsResult<McpServerSpec> {
        validate_mcp_server_spec(&server)?;
        if let Some(existing) = self.read_state()?.mcp_servers.get(&server.name) {
            if existing.server_id != server.server_id {
                return Err(AgentOsError::Validation(format!(
                    "duplicate MCP server name {} from {} and {}",
                    server.name, existing.source.source_path, server.source.source_path
                )));
            }
        }
        self.emit(
            "McpServerRegistered",
            "mcp_server",
            &server.server_id,
            None,
            None,
            None,
            None,
            &server,
        )?;
        Ok(server)
    }

    pub fn register_mcp_tool_definition(
        &self,
        tool: McpToolDefinition,
    ) -> AgentOsResult<McpToolDefinition> {
        validate_mcp_tool_definition(&tool)?;
        if !self
            .read_state()?
            .mcp_servers
            .contains_key(&tool.server_name)
        {
            return Err(AgentOsError::NotFound(format!(
                "MCP server {}",
                tool.server_name
            )));
        }
        if let Some(existing) = self.read_state()?.mcp_tools.get(&tool.model_tool_name) {
            if existing.mcp_tool_id != tool.mcp_tool_id {
                return Err(AgentOsError::Validation(format!(
                    "duplicate MCP model tool name {}",
                    tool.model_tool_name
                )));
            }
        }
        self.register_tool_descriptor(tool.tool_descriptor.clone())?;
        self.emit(
            "McpToolRegistered",
            "mcp_tool",
            &tool.mcp_tool_id,
            None,
            None,
            None,
            None,
            &tool,
        )?;
        Ok(tool)
    }

    pub fn register_imported_agent_profile(
        &self,
        profile: ImportedAgentProfile,
    ) -> AgentOsResult<ImportedAgentProfile> {
        validate_imported_agent_profile(&profile)?;
        if let Some(existing) = self
            .read_state()?
            .imported_agent_profiles
            .get(&profile.name)
        {
            if existing.imported_agent_profile_id != profile.imported_agent_profile_id {
                return Err(AgentOsError::Validation(format!(
                    "duplicate imported agent profile {} from {} and {}",
                    profile.name, existing.source.source_path, profile.source.source_path
                )));
            }
        }
        self.emit(
            "ImportedAgentProfileRegistered",
            "imported_agent_profile",
            &profile.imported_agent_profile_id,
            None,
            None,
            None,
            None,
            &profile,
        )?;
        Ok(profile)
    }
}

pub fn mcp_tool_descriptor(
    server: &McpServerSpec,
    tool_name: &str,
    description: &str,
    input_schema: Value,
    output_schema: Value,
    now: &str,
) -> AgentOsResult<ToolDescriptor> {
    if tool_name.trim().is_empty() {
        return Err(AgentOsError::Validation(
            "MCP tool name must not be empty".to_string(),
        ));
    }
    let model_tool_name = mcp_model_tool_name(&server.name, tool_name);
    Ok(ToolDescriptor {
        tool_id: format!("tool_{model_tool_name}"),
        name: model_tool_name,
        description: description.to_string(),
        version: "0.2.0".to_string(),
        driver_class: ToolDriverClass::Mcp,
        risk_level: 3,
        input_schema: input_schema.clone(),
        model_input_schema: Some(input_schema),
        output_schema,
        runtime_input_policy: ToolRuntimeInputPolicy {
            required_resource_scopes: vec![format!("mcp:{}:{}", server.name, tool_name)],
            ..ToolRuntimeInputPolicy::default()
        },
        driver_config: json!({
            "server_name": server.name,
            "tool_name": tool_name,
            "transport": server.transport,
            "command": server.command,
            "environment": server.environment,
            "timeout_ms": server.timeout_ms
        }),
        idempotency: IdempotencyMode::ToolNative,
        evidence_type: Some(EvidenceType::ExternalReference),
        created_at: now.to_string(),
    })
}

fn validate_skill_definition(skill: &SkillDefinition) -> AgentOsResult<()> {
    if skill.skill_id.is_empty()
        || skill.name.is_empty()
        || skill.description.is_empty()
        || skill.root_path.is_empty()
        || skill.skill_file_path.is_empty()
        || skill.content_hash.is_empty()
    {
        return Err(AgentOsError::Validation(
            "skill requires id, name, description, root path, file path, and content hash"
                .to_string(),
        ));
    }
    Ok(())
}

fn validate_command_definition(command: &CommandDefinition) -> AgentOsResult<()> {
    if command.command_id.is_empty()
        || command.name.is_empty()
        || command.template.trim().is_empty()
        || command.content_hash.is_empty()
    {
        return Err(AgentOsError::Validation(
            "command requires id, name, non-empty template, and content hash".to_string(),
        ));
    }
    if command.template.contains("!`") {
        return Err(AgentOsError::Validation(
            "command templates must not use shell interpolation".to_string(),
        ));
    }
    Ok(())
}

fn validate_mcp_server_spec(server: &McpServerSpec) -> AgentOsResult<()> {
    if server.server_id.is_empty() || server.name.is_empty() || server.command.is_empty() {
        return Err(AgentOsError::Validation(
            "local stdio MCP server requires id, name, and command".to_string(),
        ));
    }
    Ok(())
}

fn validate_mcp_tool_definition(tool: &McpToolDefinition) -> AgentOsResult<()> {
    if tool.mcp_tool_id.is_empty()
        || tool.server_name.is_empty()
        || tool.tool_name.is_empty()
        || tool.model_tool_name != mcp_model_tool_name(&tool.server_name, &tool.tool_name)
    {
        return Err(AgentOsError::Validation(
            "MCP tool requires id, server, tool name, and canonical model tool name".to_string(),
        ));
    }
    validate_json_schema(
        &json!({"type": "object"}),
        &tool.input_schema,
        "mcp.input_schema",
    )?;
    Ok(())
}

fn validate_imported_agent_profile(profile: &ImportedAgentProfile) -> AgentOsResult<()> {
    if profile.imported_agent_profile_id.is_empty()
        || profile.name.is_empty()
        || profile.prompt.trim().is_empty()
        || profile.content_hash.is_empty()
    {
        return Err(AgentOsError::Validation(
            "imported agent profile requires id, name, prompt, and content hash".to_string(),
        ));
    }
    Ok(())
}
