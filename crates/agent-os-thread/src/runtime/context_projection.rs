use crate::ToolExecutionRecord;
use serde_json::Value;

use super::feedback::RUNTIME_FEEDBACK_TOOL;

const MAX_PROJECTED_TOOL_RESULTS: usize = 8;
const MAX_PROJECTED_OLDER_TOOL_STRING_CHARS: usize = 2000;

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
