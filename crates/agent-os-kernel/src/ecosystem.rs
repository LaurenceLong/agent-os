use crate::schema::validate_json_schema;
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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
        examples: Vec::new(),
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

pub fn discover_mcp_tool_definitions(
    server: &McpServerSpec,
    source: EcosystemSource,
) -> AgentOsResult<Vec<McpToolDefinition>> {
    validate_mcp_server_spec(server)?;
    let listed = mcp_list_tools(server)?;
    let now = now_rfc3339();
    let mut tools = Vec::new();
    for item in listed {
        let name = required_json_string(&item, "name", "MCP tool")?;
        let description = item
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let input_schema = item
            .get("inputSchema")
            .or_else(|| item.get("input_schema"))
            .cloned()
            .unwrap_or_else(|| json!({"type": "object", "additionalProperties": true}));
        let output_schema = mcp_output_schema();
        let descriptor = mcp_tool_descriptor(
            server,
            &name,
            &description,
            input_schema.clone(),
            output_schema.clone(),
            &now,
        )?;
        tools.push(McpToolDefinition {
            mcp_tool_id: stable_mcp_tool_id(&server.name, &name),
            server_name: server.name.clone(),
            tool_name: name.clone(),
            model_tool_name: mcp_model_tool_name(&server.name, &name),
            description,
            input_schema,
            output_schema,
            source: source.clone(),
            tool_descriptor: descriptor,
            created_at: now.clone(),
        });
    }
    Ok(tools)
}

fn mcp_list_tools(server: &McpServerSpec) -> AgentOsResult<Vec<Value>> {
    let (program, args) = server
        .command
        .split_first()
        .ok_or_else(|| AgentOsError::Validation("MCP command must not be empty".to_string()))?;
    let mut child = Command::new(program)
        .args(args)
        .envs(server.environment.clone())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| AgentOsError::Validation(format!("spawn MCP server: {error}")))?;
    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| AgentOsError::Validation("MCP stdin unavailable".to_string()))?;
        writeln!(stdin, "{}", json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"agent-os","version":ABI_VERSION}}}))
            .and_then(|_| writeln!(stdin, "{}", json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}})))
            .and_then(|_| writeln!(stdin, "{}", json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}})))
            .map_err(|error| AgentOsError::Validation(format!("write MCP tools/list: {error}")))?;
    }
    let output = wait_mcp_child(child, server.timeout_ms, "tools/list")?;
    if !output.status.success() {
        return Err(AgentOsError::Validation(format!(
            "MCP tools/list exited with status {}: {}",
            output.status,
            bounded_stderr(&output.stderr)
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let result = parse_json_rpc_result(&stdout, 2)?;
    result
        .get("tools")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| {
            AgentOsError::Validation("MCP tools/list response missing tools".to_string())
        })
}

fn wait_mcp_child(mut child: Child, timeout_ms: u64, operation: &str) -> AgentOsResult<Output> {
    let timeout = Duration::from_millis(timeout_ms);
    let started = Instant::now();
    loop {
        if child
            .try_wait()
            .map_err(|error| {
                AgentOsError::Validation(format!("wait for MCP {operation}: {error}"))
            })?
            .is_some()
        {
            return child.wait_with_output().map_err(|error| {
                AgentOsError::Validation(format!("wait for MCP {operation}: {error}"))
            });
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let output = child.wait_with_output().map_err(|error| {
                AgentOsError::Validation(format!("wait for timed-out MCP {operation}: {error}"))
            })?;
            return Err(AgentOsError::Validation(format!(
                "MCP {operation} timed out after {timeout_ms}ms: {}",
                bounded_stderr(&output.stderr)
            )));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn parse_json_rpc_result(stdout: &str, id: u64) -> AgentOsResult<Value> {
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(trimmed)?;
        if value.get("id").and_then(Value::as_u64) == Some(id) {
            return value.get("result").cloned().ok_or_else(|| {
                AgentOsError::Validation("JSON-RPC response missing result".to_string())
            });
        }
    }
    Err(AgentOsError::Validation(
        "JSON-RPC response id was not found".to_string(),
    ))
}

fn mcp_output_schema() -> Value {
    json!({
        "type": "object",
        "required": ["tool", "status", "input", "driver_class", "server_name", "tool_name", "raw_result", "stderr"],
        "properties": {
            "tool": {"type": "string"},
            "status": {"enum": ["ok"]},
            "input": {"type": "object"},
            "driver_class": {"type": "string"},
            "server_name": {"type": "string"},
            "tool_name": {"type": "string"},
            "raw_result": {"type": "object"},
            "stderr": {"type": "string"}
        },
        "additionalProperties": false
    })
}

fn required_json_string(value: &Value, field: &str, label: &str) -> AgentOsResult<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| AgentOsError::Validation(format!("{label} missing string field {field}")))
}

fn stable_mcp_tool_id(server_name: &str, tool_name: &str) -> String {
    let digest = Sha256::digest(format!("{server_name}\n{tool_name}").as_bytes());
    let hash: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("mcptool_{}", &hash[..16])
}

fn bounded_stderr(stderr: &[u8]) -> String {
    const LIMIT: usize = 4096;
    let text = String::from_utf8_lossy(stderr);
    let mut bounded: String = text.chars().take(LIMIT).collect();
    if text.chars().count() > LIMIT {
        bounded.push_str("...");
    }
    bounded
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn mcp_discovery_rejects_empty_command_before_spawn() {
        let server = McpServerSpec {
            server_id: "mcp_empty".to_string(),
            name: "empty".to_string(),
            transport: McpTransportKind::LocalStdio,
            command: Vec::new(),
            environment: BTreeMap::new(),
            enabled: true,
            timeout_ms: 1000,
            source: source(),
            created_at: now_rfc3339(),
        };

        let err = discover_mcp_tool_definitions(&server, source()).unwrap_err();

        assert!(err.to_string().contains("command"));
    }

    #[test]
    fn mcp_discovery_reports_missing_tools_result() {
        let server = McpServerSpec {
            server_id: "mcp_invalid".to_string(),
            name: "invalid".to_string(),
            transport: McpTransportKind::LocalStdio,
            command: vec![
                std::env::current_exe()
                    .unwrap()
                    .to_string_lossy()
                    .to_string(),
                "--ignored-by-test-binary".to_string(),
            ],
            environment: BTreeMap::new(),
            enabled: true,
            timeout_ms: 1000,
            source: source(),
            created_at: now_rfc3339(),
        };

        let err = discover_mcp_tool_definitions(&server, source()).unwrap_err();

        assert!(
            err.to_string().contains("JSON-RPC response")
                || err.to_string().contains("exited with status")
        );
    }

    fn source() -> EcosystemSource {
        EcosystemSource {
            source_kind: EcosystemSourceKind::AgentOs,
            source_scope: EcosystemSourceScope::Config,
            source_path: "agent-os.json".to_string(),
        }
    }
}
