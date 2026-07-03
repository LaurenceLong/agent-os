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

    pub fn register_mcp_resource_definition(
        &self,
        resource: McpResourceDefinition,
    ) -> AgentOsResult<McpResourceDefinition> {
        validate_mcp_resource_definition(&resource)?;
        if !self
            .read_state()?
            .mcp_servers
            .contains_key(&resource.server_name)
        {
            return Err(AgentOsError::NotFound(format!(
                "MCP server {}",
                resource.server_name
            )));
        }
        self.emit(
            "McpResourceRegistered",
            "mcp_resource",
            &resource.mcp_resource_id,
            None,
            None,
            None,
            None,
            &resource,
        )?;
        Ok(resource)
    }

    pub fn register_mcp_resource_template_definition(
        &self,
        template: McpResourceTemplateDefinition,
    ) -> AgentOsResult<McpResourceTemplateDefinition> {
        validate_mcp_resource_template_definition(&template)?;
        if !self
            .read_state()?
            .mcp_servers
            .contains_key(&template.server_name)
        {
            return Err(AgentOsError::NotFound(format!(
                "MCP server {}",
                template.server_name
            )));
        }
        self.emit(
            "McpResourceTemplateRegistered",
            "mcp_resource_template",
            &template.mcp_resource_template_id,
            None,
            None,
            None,
            None,
            &template,
        )?;
        Ok(template)
    }

    pub fn register_imported_agent_profile(
        &self,
        profile: ImportedAgentProfile,
    ) -> AgentOsResult<ImportedAgentProfile> {
        validate_imported_agent_profile(&profile)?;
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
        version: "0.3.0".to_string(),
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
        lifecycle: ToolLifecyclePolicy::default(),
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
    let listed = mcp_list_capability(server, "tools/list", "tools")?;
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

pub fn discover_mcp_resource_definitions(
    server: &McpServerSpec,
    source: EcosystemSource,
) -> AgentOsResult<Vec<McpResourceDefinition>> {
    validate_mcp_server_spec(server)?;
    let listed = mcp_list_capability(server, "resources/list", "resources")?;
    let now = now_rfc3339();
    let mut resources = Vec::new();
    for item in listed {
        let uri = required_json_string(&item, "uri", "MCP resource")?;
        resources.push(McpResourceDefinition {
            mcp_resource_id: stable_mcp_resource_id(&server.name, &uri),
            server_name: server.name.clone(),
            uri,
            name: optional_json_string(&item, "name", "MCP resource")?,
            description: optional_json_string(&item, "description", "MCP resource")?,
            mime_type: optional_json_string(&item, "mimeType", "MCP resource")?
                .or(optional_json_string(&item, "mime_type", "MCP resource")?),
            source: source.clone(),
            created_at: now.clone(),
        });
    }
    Ok(resources)
}

pub fn discover_mcp_resource_template_definitions(
    server: &McpServerSpec,
    source: EcosystemSource,
) -> AgentOsResult<Vec<McpResourceTemplateDefinition>> {
    validate_mcp_server_spec(server)?;
    let listed = mcp_list_capability(server, "resources/templates/list", "resourceTemplates")?;
    let now = now_rfc3339();
    let mut templates = Vec::new();
    for item in listed {
        let uri_template = required_json_string(&item, "uriTemplate", "MCP resource template")
            .or_else(|_| required_json_string(&item, "uri_template", "MCP resource template"))?;
        templates.push(McpResourceTemplateDefinition {
            mcp_resource_template_id: stable_mcp_resource_template_id(&server.name, &uri_template),
            server_name: server.name.clone(),
            uri_template,
            name: optional_json_string(&item, "name", "MCP resource template")?,
            description: optional_json_string(&item, "description", "MCP resource template")?,
            mime_type: optional_json_string(&item, "mimeType", "MCP resource template")?.or(
                optional_json_string(&item, "mime_type", "MCP resource template")?,
            ),
            source: source.clone(),
            created_at: now.clone(),
        });
    }
    Ok(templates)
}

fn mcp_list_capability(
    server: &McpServerSpec,
    method: &str,
    result_field: &str,
) -> AgentOsResult<Vec<Value>> {
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
            .and_then(|_| writeln!(stdin, "{}", json!({"jsonrpc":"2.0","id":2,"method":method,"params":{}})))
            .map_err(|error| AgentOsError::Validation(format!("write MCP {method}: {error}")))?;
    }
    let output = wait_mcp_child(child, server.timeout_ms, method)?;
    if !output.status.success() {
        return Err(AgentOsError::Validation(format!(
            "MCP {method} exited with status {}: {}",
            output.status,
            bounded_stderr(&output.stderr)
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let result = parse_json_rpc_result(&stdout, 2)?;
    result
        .get(result_field)
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| {
            AgentOsError::Validation(format!("MCP {method} response missing {result_field}"))
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

fn optional_json_string(value: &Value, field: &str, label: &str) -> AgentOsResult<Option<String>> {
    let Some(item) = value.get(field) else {
        return Ok(None);
    };
    item.as_str()
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| AgentOsError::Validation(format!("{label} field {field} must be a string")))
}

fn stable_mcp_tool_id(server_name: &str, tool_name: &str) -> String {
    let digest = Sha256::digest(format!("{server_name}\n{tool_name}").as_bytes());
    let hash: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("mcptool_{}", &hash[..16])
}

fn stable_mcp_resource_id(server_name: &str, uri: &str) -> String {
    let digest = Sha256::digest(format!("{server_name}\n{uri}").as_bytes());
    let hash: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("mcpres_{}", &hash[..16])
}

fn stable_mcp_resource_template_id(server_name: &str, uri_template: &str) -> String {
    let digest = Sha256::digest(format!("{server_name}\n{uri_template}").as_bytes());
    let hash: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("mcprestpl_{}", &hash[..16])
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

fn validate_mcp_resource_definition(resource: &McpResourceDefinition) -> AgentOsResult<()> {
    if resource.mcp_resource_id.is_empty()
        || resource.server_name.is_empty()
        || resource.uri.trim().is_empty()
    {
        return Err(AgentOsError::Validation(
            "MCP resource requires id, server, and uri".to_string(),
        ));
    }
    Ok(())
}

fn validate_mcp_resource_template_definition(
    template: &McpResourceTemplateDefinition,
) -> AgentOsResult<()> {
    if template.mcp_resource_template_id.is_empty()
        || template.server_name.is_empty()
        || template.uri_template.trim().is_empty()
    {
        return Err(AgentOsError::Validation(
            "MCP resource template requires id, server, and uri_template".to_string(),
        ));
    }
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
            source_path: ".agent-os/config.json".to_string(),
        }
    }
}
