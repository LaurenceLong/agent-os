use super::stable_id;
use agent_os_kernel::discover_mcp_tool_definitions;
use agent_os_sys::*;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

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
            discover_mcp_tool_definitions(&server, source)?
        } else {
            Vec::new()
        };
        servers.push((server, tools));
    }
    Ok(servers)
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
