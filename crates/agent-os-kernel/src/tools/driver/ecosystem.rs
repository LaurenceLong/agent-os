use crate::util::required_string;
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

pub(in crate::tools) fn run_load_skill(
    kernel: &Kernel,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let name = required_string(input, "name")?;
    let skill = kernel
        .read_state()?
        .skill_definitions
        .get(&name)
        .cloned()
        .ok_or_else(|| AgentOsError::NotFound(format!("skill {name}")))?;
    let (offset, limit) = super::super::builtin::read_file::parse_paging(input)?;
    let page = super::super::builtin::read_file::paginate_text(&skill.content, offset, limit);
    Ok(json!({
        "tool": descriptor.name,
        "status": "ok",
        "input": input,
        "driver_class": descriptor.driver_class,
        "skill_id": skill.skill_id,
        "name": skill.name,
        "description": skill.description,
        "content": page.content,
        "root_path": skill.root_path,
        "skill_file_path": skill.skill_file_path,
        "offset": offset,
        "limit": limit,
        "total_lines": page.total_lines,
        "returned_lines": page.returned_lines,
        "next_offset": page.next_offset,
        "truncated": page.truncated,
        "omitted_lines": page.omitted_lines
    }))
}

pub(in crate::tools) fn run_read_skill_resource(
    kernel: &Kernel,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let name = required_string(input, "name")?;
    let relative = PathBuf::from(required_string(input, "path")?);
    ensure_safe_relative_path(&relative)?;
    let skill = kernel
        .read_state()?
        .skill_definitions
        .get(&name)
        .cloned()
        .ok_or_else(|| AgentOsError::NotFound(format!("skill {name}")))?;
    let root = PathBuf::from(&skill.root_path)
        .canonicalize()
        .map_err(|error| AgentOsError::Validation(format!("canonicalize skill root: {error}")))?;
    let target = root.join(&relative);
    let canonical = target.canonicalize().map_err(|error| {
        AgentOsError::Validation(format!("canonicalize skill resource: {error}"))
    })?;
    if !canonical.starts_with(&root) {
        return Err(AgentOsError::PermissionDenied(
            "skill resource path escapes skill root".to_string(),
        ));
    }
    let content = std::fs::read_to_string(&canonical)
        .map_err(|error| AgentOsError::Validation(format!("read skill resource: {error}")))?;
    let (offset, limit) = super::super::builtin::read_file::parse_paging(input)?;
    let page = super::super::builtin::read_file::paginate_text(&content, offset, limit);
    let bytes_read = page.content.len();
    Ok(json!({
        "tool": descriptor.name,
        "status": "ok",
        "input": input,
        "driver_class": descriptor.driver_class,
        "skill_id": skill.skill_id,
        "name": skill.name,
        "path": relative.to_string_lossy(),
        "content": page.content,
        "bytes_read": bytes_read,
        "offset": offset,
        "limit": limit,
        "total_lines": page.total_lines,
        "returned_lines": page.returned_lines,
        "next_offset": page.next_offset,
        "truncated": page.truncated,
        "omitted_lines": page.omitted_lines
    }))
}

pub(super) fn run_mcp_tool(descriptor: &ToolDescriptor, input: &Value) -> AgentOsResult<Value> {
    let server_name = required_string(&descriptor.driver_config, "server_name")?;
    let tool_name = required_string(&descriptor.driver_config, "tool_name")?;
    let command = descriptor
        .driver_config
        .get("command")
        .and_then(Value::as_array)
        .ok_or_else(|| AgentOsError::Validation("MCP driver command must be an array".to_string()))?
        .iter()
        .map(|item| {
            item.as_str().map(str::to_string).ok_or_else(|| {
                AgentOsError::Validation("MCP command entries must be strings".to_string())
            })
        })
        .collect::<AgentOsResult<Vec<_>>>()?;
    let (program, args) = command
        .split_first()
        .ok_or_else(|| AgentOsError::Validation("MCP command must not be empty".to_string()))?;
    let mut child = Command::new(program)
        .args(args)
        .envs(mcp_environment(&descriptor.driver_config)?)
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
        let initialize = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "agent-os", "version": ABI_VERSION}
            }
        });
        let initialized = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        });
        let call = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": input
            }
        });
        writeln!(stdin, "{initialize}")
            .and_then(|_| writeln!(stdin, "{initialized}"))
            .and_then(|_| writeln!(stdin, "{call}"))
            .map_err(|error| AgentOsError::Validation(format!("write MCP request: {error}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| AgentOsError::Validation(format!("wait for MCP server: {error}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let raw_result = bounded_json(parse_mcp_response(&stdout, 2)?);
    let (stderr, _, _) = super::super::builtin::run_command::bounded_text(&output.stderr);
    Ok(json!({
        "tool": descriptor.name,
        "status": "ok",
        "input": input,
        "driver_class": descriptor.driver_class,
        "server_name": server_name,
        "tool_name": tool_name,
        "raw_result": raw_result,
        "stderr": stderr
    }))
}

fn bounded_json(value: Value) -> Value {
    match value {
        Value::String(text) => {
            let bytes = text.as_bytes();
            let (bounded, truncated, total) =
                super::super::builtin::run_command::bounded_text(bytes);
            if truncated {
                json!({
                    "content": bounded,
                    "truncated": true,
                    "original_bytes": total
                })
            } else {
                Value::String(bounded)
            }
        }
        Value::Array(items) => Value::Array(items.into_iter().map(bounded_json).collect()),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, bounded_json(value)))
                .collect(),
        ),
        other => other,
    }
}

fn ensure_safe_relative_path(path: &Path) -> AgentOsResult<()> {
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(AgentOsError::Validation(
            "skill resource path must stay inside the skill root".to_string(),
        ));
    }
    Ok(())
}

fn mcp_environment(config: &Value) -> AgentOsResult<Vec<(String, String)>> {
    let Some(env) = config.get("environment") else {
        return Ok(Vec::new());
    };
    let env = env
        .as_object()
        .ok_or_else(|| AgentOsError::Validation("MCP environment must be an object".to_string()))?;
    env.iter()
        .map(|(key, value)| {
            value
                .as_str()
                .map(|value| (key.clone(), value.to_string()))
                .ok_or_else(|| {
                    AgentOsError::Validation("MCP environment values must be strings".to_string())
                })
        })
        .collect()
}

fn parse_mcp_response(stdout: &str, id: u64) -> AgentOsResult<Value> {
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(trimmed)?;
        if value.get("id").and_then(Value::as_u64) != Some(id) {
            continue;
        }
        if let Some(error) = value.get("error") {
            return Err(AgentOsError::Validation(format!(
                "MCP tool failed: {error}"
            )));
        }
        return value
            .get("result")
            .cloned()
            .ok_or_else(|| AgentOsError::Validation("MCP response missing result".to_string()));
    }
    Err(AgentOsError::Validation(
        "MCP response for tools/call was not found".to_string(),
    ))
}
