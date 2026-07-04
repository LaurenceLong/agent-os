use super::{schema, BuiltinTool};
use crate::*;
use agent_os_sys::*;
use serde::Serialize;
use serde_json::{json, Value};

pub(super) const DEFAULT_LIMIT: usize = 8;
pub(super) const MAX_LIMIT: usize = 25;

#[derive(Debug, Clone, Serialize)]
struct DeferredToolSearchMatch {
    name: String,
    description: String,
    driver_class: ToolDriverClass,
    risk_level: u8,
    reason: Option<String>,
}

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "tool_search",
        descriptor,
        execute,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_tool_search",
            name: "tool_search",
            description: "Search deferred model tools that are discoverable for the current turn. Use it when the needed capability may be provided by MCP, package, or plugin contributions that are not directly listed.",
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 1,
            input_schema: schema::object(
                &["query"],
                json!({
                    "query": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": MAX_LIMIT}
                }),
            ),
            model_input_schema: schema::object(
                &["query"],
                json!({
                    "query": {"type": "string", "description": "Capability, tool name, server name, or task keyword to search among deferred tools."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": MAX_LIMIT, "description": "Maximum deferred tools to return. Defaults to 8 and is capped at 25."}
                }),
            ),
            examples: vec![
                schema::example(
                    "Find an MCP echo capability.",
                    json!({"query": "echo", "limit": 5}),
                    "Returns matching deferred tool summaries without executing them.",
                ),
            ],
            output_schema: schema::object(
                &[
                    "tool",
                    "status",
                    "input",
                    "driver_class",
                    "query",
                    "total_matches",
                    "returned_matches",
                    "matches",
                ],
                json!({
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "query": {"type": "string"},
                    "total_matches": {"type": "integer"},
                    "returned_matches": {"type": "integer"},
                    "matches": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["name", "description", "driver_class", "risk_level"],
                            "properties": {
                                "name": {"type": "string"},
                                "description": {"type": "string"},
                                "driver_class": {"type": "string"},
                                "risk_level": {"type": "integer"},
                                "reason": {"type": ["string", "null"]}
                            },
                            "additionalProperties": false
                        }
                    }
                }),
            ),
            runtime_input_policy: ToolRuntimeInputPolicy::default(),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::RuntimeTrace),
        },
    )
}

fn execute(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    _tool_call_id: &str,
    input: &Value,
) -> AgentOsResult<Value> {
    let query = crate::util::required_string(input, "query")?;
    let limit = input
        .get("limit")
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| {
                    AgentOsError::Validation("tool_search limit must be an integer".to_string())
                })
                .and_then(|value| {
                    usize::try_from(value).map_err(|_| {
                        AgentOsError::Validation("tool_search limit is too large".to_string())
                    })
                })
        })
        .transpose()?
        .unwrap_or(DEFAULT_LIMIT);
    if limit == 0 || limit > MAX_LIMIT {
        return Err(AgentOsError::Validation(format!(
            "tool_search limit must be between 1 and {MAX_LIMIT}"
        )));
    }
    let thread = kernel
        .thread_by_agent(&syscall.agent_id)?
        .ok_or_else(|| AgentOsError::NotFound(format!("agent {}", syscall.agent_id)))?;
    let plan = kernel.plan_tools_for_turn(
        &thread,
        ModelCapabilities {
            tool_calling: true,
            image_input: true,
            ..ModelCapabilities::default()
        },
        ToolPlanningMode::Normal,
    )?;
    let normalized_query = normalize_tool_search_text(&query);
    let mut matches = plan
        .entries
        .into_iter()
        .filter(|entry| entry.exposure == ToolExposure::Deferred)
        .filter(|entry| deferred_tool_matches(&entry.descriptor, &normalized_query))
        .map(|entry| DeferredToolSearchMatch {
            name: entry.descriptor.name,
            description: entry.descriptor.description,
            driver_class: entry.descriptor.driver_class,
            risk_level: entry.descriptor.risk_level,
            reason: entry.reason,
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| left.name.cmp(&right.name));
    let total_matches = matches.len();
    let returned = matches.into_iter().take(limit).collect::<Vec<_>>();

    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "ok",
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "query": query,
        "total_matches": total_matches,
        "returned_matches": returned.len(),
        "matches": returned,
    }))
}

fn deferred_tool_matches(descriptor: &ToolDescriptor, normalized_query: &str) -> bool {
    if normalized_query.is_empty() {
        return true;
    }
    let searchable = normalize_tool_search_text(&format!(
        "{} {} {}",
        descriptor.name, descriptor.description, descriptor.driver_config
    ));
    searchable.contains(normalized_query)
        || normalized_query
            .split_whitespace()
            .all(|term| searchable.contains(term))
}

fn normalize_tool_search_text(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_exposes_search_query_and_limit() {
        let descriptor = descriptor("now");
        let model_schema = descriptor.model_input_schema.as_ref().unwrap();

        assert!(model_schema.pointer("/properties/query").is_some());
        assert!(model_schema.pointer("/properties/limit").is_some());
        assert_eq!(model_schema.pointer("/required"), Some(&json!(["query"])));
        assert!(descriptor
            .examples
            .iter()
            .any(|example| example.parameters == json!({"query": "echo", "limit": 5})));
    }

    #[test]
    fn search_matches_multi_word_query_across_tool_name_separators() {
        let descriptor = ToolDescriptor {
            name: "mcp__live_echo__echo".to_string(),
            description: "Echo one text field.".to_string(),
            ..ToolDescriptor::default()
        };

        assert!(deferred_tool_matches(&descriptor, "live echo"));
    }
}
