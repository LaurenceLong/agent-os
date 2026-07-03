use crate::{ModelContextProjection, ToolExecutionRecord};
use agent_os_sys::ModelLimit;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::feedback::RUNTIME_FEEDBACK_TOOL;

const MAX_PROJECTED_TOOL_RESULTS: usize = 8;
const MAX_PROJECTED_OLDER_TOOL_STRING_CHARS: usize = 2000;
const MIN_CONTEXT_RESERVE_TOKENS: u64 = 2048;
const RECENT_TOOL_RESULT_PROTECTION: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ContextBudgetReport {
    pub before_tokens: u64,
    pub after_tokens: u64,
    pub usable_input_tokens: u64,
    pub pruned_refs: Vec<String>,
    pub over_budget_after_prune: bool,
}

impl ContextBudgetReport {
    pub(super) fn pruned(&self) -> bool {
        !self.pruned_refs.is_empty()
    }
}

pub(super) fn project_tool_results(
    tool_results: &[ToolExecutionRecord],
) -> Vec<ToolExecutionRecord> {
    let recent_start = tool_results
        .len()
        .saturating_sub(MAX_PROJECTED_TOOL_RESULTS);
    tool_results
        .iter()
        .enumerate()
        .filter(|(index, result)| {
            *index >= recent_start
                || !result.evidence_ids.is_empty()
                || is_persistent_runtime_feedback(result)
        })
        .map(|(index, result)| {
            if index >= recent_start {
                result.clone()
            } else {
                compact_tool_result(result)
            }
        })
        .collect()
}

pub(super) fn prune_context_for_model_limit(
    context: &mut ModelContextProjection,
    limit: &ModelLimit,
) -> ContextBudgetReport {
    let usable_input_tokens = usable_input_tokens(limit);
    let before_tokens = estimate_context_tokens(context);
    let mut pruned_refs = Vec::new();
    if before_tokens <= usable_input_tokens {
        return ContextBudgetReport {
            before_tokens,
            after_tokens: before_tokens,
            usable_input_tokens,
            pruned_refs,
            over_budget_after_prune: false,
        };
    }

    let protected_start = context
        .tool_results
        .len()
        .saturating_sub(RECENT_TOOL_RESULT_PROTECTION);
    for index in 0..context.tool_results.len() {
        if estimate_context_tokens(context) <= usable_input_tokens {
            break;
        }
        let result = &mut context.tool_results[index];
        if index >= protected_start
            || !result.evidence_ids.is_empty()
            || is_persistent_runtime_feedback(result)
        {
            continue;
        }
        if mark_tool_result_pruned(result) {
            pruned_refs.push(format!("tool_result:{}", result.call_id));
        }
    }

    while estimate_context_tokens(context) > usable_input_tokens
        && context.context_snapshots.len() > 1
    {
        let removed = context.context_snapshots.remove(0);
        pruned_refs.push(format!("context_snapshot:{}", removed.context_id));
    }

    while estimate_context_tokens(context) > usable_input_tokens && context.memory_records.len() > 1
    {
        let removed = context.memory_records.remove(0);
        pruned_refs.push(format!("memory_record:{}", removed.memory_id));
    }

    let after_tokens = estimate_context_tokens(context);
    ContextBudgetReport {
        before_tokens,
        after_tokens,
        usable_input_tokens,
        pruned_refs,
        over_budget_after_prune: after_tokens > usable_input_tokens,
    }
}

fn usable_input_tokens(limit: &ModelLimit) -> u64 {
    let raw = limit
        .input
        .unwrap_or_else(|| limit.context.saturating_sub(limit.output));
    let reserve = limit.output.max(MIN_CONTEXT_RESERVE_TOKENS).min(raw / 2);
    raw.saturating_sub(reserve).max(1)
}

fn estimate_context_tokens(context: &ModelContextProjection) -> u64 {
    let bytes = serde_json::to_vec(context)
        .map(|bytes| bytes.len() as u64)
        .unwrap_or(u64::MAX / 8);
    bytes.saturating_add(3) / 4
}

fn is_persistent_runtime_feedback(result: &ToolExecutionRecord) -> bool {
    if result.tool_name != RUNTIME_FEEDBACK_TOOL {
        return false;
    }
    result
        .output
        .as_ref()
        .and_then(|output| output.get("message"))
        .and_then(Value::as_str)
        .is_some_and(|message| {
            message.contains("pre-patch investigation budget is nearly exhausted")
                || message.contains("patch plus command evidence already exist")
        })
}

fn compact_tool_result(result: &ToolExecutionRecord) -> ToolExecutionRecord {
    let mut compacted = result.clone();
    if let Some(output) = &result.output {
        let (mut value, truncated) =
            compact_json_value(output, MAX_PROJECTED_OLDER_TOOL_STRING_CHARS);
        if truncated {
            if let Value::Object(map) = &mut value {
                map.insert("projection_truncated".to_string(), Value::Bool(true));
                map.insert(
                    "projection_note".to_string(),
                    Value::String(
                        "Older evidence output was truncated for projection; rerun a narrower command if exact omitted content is needed."
                            .to_string(),
                    ),
                );
            }
        }
        compacted.output = Some(value);
    }
    compacted
}

fn mark_tool_result_pruned(result: &mut ToolExecutionRecord) -> bool {
    if result
        .output
        .as_ref()
        .and_then(|output| output.get("projection_pruned"))
        .and_then(Value::as_bool)
        == Some(true)
    {
        return false;
    }
    result.output = Some(serde_json::json!({
        "projection_pruned": true,
        "reason": "context_pressure",
        "tool_name": result.tool_name,
        "call_id": result.call_id,
        "status": result.status,
        "note": "Older non-evidence tool output was pruned before the provider call; rerun a narrower tool call if exact omitted content is needed."
    }));
    true
}

fn compact_json_value(value: &Value, max_string_chars: usize) -> (Value, bool) {
    match value {
        Value::String(text) if text.chars().count() > max_string_chars => {
            let prefix = text.chars().take(max_string_chars).collect::<String>();
            let omitted = text.chars().count().saturating_sub(max_string_chars);
            (
                Value::String(format!(
                    "{prefix}\n...[truncated for projection: {omitted} chars omitted]"
                )),
                true,
            )
        }
        Value::String(_) | Value::Null | Value::Bool(_) | Value::Number(_) => {
            (value.clone(), false)
        }
        Value::Array(items) => {
            let mut truncated = false;
            let values = items
                .iter()
                .map(|item| {
                    let (value, item_truncated) = compact_json_value(item, max_string_chars);
                    truncated |= item_truncated;
                    value
                })
                .collect();
            (Value::Array(values), truncated)
        }
        Value::Object(map) => {
            let mut truncated = false;
            let values = map
                .iter()
                .map(|(key, item)| {
                    let (value, item_truncated) = compact_json_value(item, max_string_chars);
                    truncated |= item_truncated;
                    (key.clone(), value)
                })
                .collect();
            (Value::Object(values), truncated)
        }
    }
}
