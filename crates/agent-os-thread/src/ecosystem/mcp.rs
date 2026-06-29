use super::stable_id;
use agent_os_kernel::mcp_tool_descriptor;
use agent_os_sys::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub(super) fn discover_mcp(
    config_path: &Path,
    config: &Value,
) -> AgentOsResult<Vec<(McpServerSpec, Vec<McpToolDefinition>)>> {
    let mut servers = Vec::new();
    let Some(local_stdio) = config
        .pointer("/mcp/local_stdio")
        .and_then(Value::as_object)
    else {
        return Ok(servers);
    };
    for (name, item) in local_stdio {
        let command = string_array_field(item, "command")?;
        let environment = string_map_field(item, "environment")?;
        let enabled = item.get("enabled").and_then(Value::as_bool).unwrap_or(true);
        let source = EcosystemSource {
            source_kind: EcosystemSourceKind::AgentOs,
            source_scope: EcosystemSourceScope::Config,
            source_path: config_path.to_string_lossy().to_string(),
        };
        let server = McpServerSpec {
            server_id: stable_id("mcp", config_path, name),
            name: name.clone(),
            transport: McpTransportKind::LocalStdio,
            command,
            environment,
            enabled,
            timeout_ms: item
                .get("timeout_ms")
                .and_then(Value::as_u64)
                .unwrap_or(30_000),
            source: source.clone(),
            created_at: now_rfc3339(),
        };
        let tools = if enabled {
            discover_mcp_tools(&server, source)?
        } else {
            Vec::new()
        };
        servers.push((server, tools));
    }
    Ok(servers)
}

fn discover_mcp_tools(
    server: &McpServerSpec,
    source: EcosystemSource,
) -> AgentOsResult<Vec<McpToolDefinition>> {
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
            mcp_tool_id: stable_id("mcptool", Path::new(&server.name), &name),
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
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| AgentOsError::Validation("MCP stdin unavailable".to_string()))?;
        writeln!(stdin, "{}", json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"agent-os","version":ABI_VERSION}}}))
            .and_then(|_| writeln!(stdin, "{}", json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}})))
            .and_then(|_| writeln!(stdin, "{}", json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}})))
            .map_err(|error| AgentOsError::Validation(format!("write MCP tools/list: {error}")))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| AgentOsError::Validation(format!("wait for MCP tools/list: {error}")))?;
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

fn string_array_field(value: &Value, field: &str) -> AgentOsResult<Vec<String>> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| AgentOsError::Validation(format!("{field} must be an array")))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| AgentOsError::Validation(format!("{field} entries must be strings")))
        })
        .collect()
}

fn string_map_field(value: &Value, field: &str) -> AgentOsResult<BTreeMap<String, String>> {
    let Some(map) = value.get(field) else {
        return Ok(BTreeMap::new());
    };
    map.as_object()
        .ok_or_else(|| AgentOsError::Validation(format!("{field} must be an object")))?
        .iter()
        .map(|(key, value)| {
            value
                .as_str()
                .map(|value| (key.clone(), value.to_string()))
                .ok_or_else(|| AgentOsError::Validation(format!("{field} values must be strings")))
        })
        .collect()
}
