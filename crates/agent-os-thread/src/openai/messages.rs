use super::audit::truncate;
use super::prompt::default_system_prompt;
use crate::{ModelTurnRequest, ToolExecutionRecord};
use serde_json::{json, Value};

pub(crate) fn build_messages(
    request: &ModelTurnRequest,
    workspace_root: &str,
    system_prompt_override: &Option<String>,
) -> Vec<Value> {
    let system_content = system_prompt_override
        .clone()
        .unwrap_or_else(|| default_system_prompt(request, workspace_root));

    let mut messages = vec![
        json!({"role": "system", "content": system_content}),
        json!({
            "role": "user",
            "content": format_user_task_message(request, workspace_root)
        }),
    ];

    for result in &request.tool_results {
        inject_tool_result_messages(&mut messages, result, workspace_root);
    }

    messages
}

pub(crate) fn build_anthropic_messages(
    request: &ModelTurnRequest,
    workspace_root: &str,
) -> Vec<Value> {
    let mut messages = vec![json!({
        "role": "user",
        "content": format_user_task_message(request, workspace_root)
    })];

    for result in &request.tool_results {
        inject_anthropic_tool_result_messages(&mut messages, result);
    }

    messages
}

fn format_user_task_message(request: &ModelTurnRequest, workspace_root: &str) -> String {
    format!(
        "Task: {}\n\n{}\n\nWorkspace: {}",
        request.thread.task.local_goal,
        if request.thread.task.success_criteria.is_empty() {
            String::new()
        } else {
            format!(
                "Success criteria:\n{}",
                request
                    .thread
                    .task
                    .success_criteria
                    .iter()
                    .map(|c| format!("- {c}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        },
        workspace_root
    )
}

fn inject_tool_result_messages(
    messages: &mut Vec<Value>,
    result: &ToolExecutionRecord,
    _workspace_root: &str,
) {
    let (func_name, func_input) = reconstruct_call(result);
    let call_id = &result.call_id;

    messages.push(json!({
        "role": "assistant",
        "content": null,
        "tool_calls": [{
            "id": call_id,
            "type": "function",
            "function": {
                "name": func_name,
                "arguments": serde_json::to_string(&func_input).unwrap_or_else(|_| "{}".to_string())
            }
        }]
    }));

    let result_content = match &result.output {
        Some(output) => {
            let trimmed = trim_tool_output(output);
            serde_json::to_string(&trimmed).unwrap_or_else(|_| "{}".to_string())
        }
        None => format!(
            "{{\"status\": \"{}\"}}",
            format!("{:?}", result.status).to_lowercase()
        ),
    };

    messages.push(json!({
        "role": "tool",
        "tool_call_id": call_id,
        "content": truncate(&result_content, 8000),
    }));
}

fn inject_anthropic_tool_result_messages(messages: &mut Vec<Value>, result: &ToolExecutionRecord) {
    let (tool_name, input) = reconstruct_call(result);
    messages.push(json!({
        "role": "assistant",
        "content": [{
            "type": "tool_use",
            "id": result.call_id,
            "name": tool_name,
            "input": input
        }]
    }));

    let result_content = match &result.output {
        Some(output) => {
            serde_json::to_string(&trim_tool_output(output)).unwrap_or_else(|_| "{}".to_string())
        }
        None => format!(
            "{{\"status\": \"{}\"}}",
            format!("{:?}", result.status).to_lowercase()
        ),
    };

    messages.push(json!({
        "role": "user",
        "content": [{
            "type": "tool_result",
            "tool_use_id": result.call_id,
            "content": truncate(&result_content, 8000)
        }]
    }));
}

fn reconstruct_call(result: &ToolExecutionRecord) -> (&'static str, Value) {
    let input = result.input.clone().unwrap_or_else(|| {
        result
            .output
            .as_ref()
            .and_then(|o| o.get("input"))
            .cloned()
            .unwrap_or_else(|| json!({}))
    });

    match result.tool_name.as_str() {
        "read_file" => ("read_file", strip_workspace_root(&input)),
        "write_file" => ("write_file", strip_workspace_root(&input)),
        "delete_file" => ("delete_file", strip_workspace_root(&input)),
        "replace_text" => ("replace_text", strip_workspace_root(&input)),
        "run_command" => ("run_command", strip_cwd(&input)),
        "set_objective" => ("set_objective", input),
        "update_checklist" => ("update_checklist", input),
        "record_evidence" => ("record_evidence", input),
        "report_supervisor" => ("report_supervisor", input),
        "post_blackboard" => ("post_blackboard", input),
        "ask_human" => ("ask_human", input),
        "agent_control" => ("agent_control", input),
        _ => ("unknown", input),
    }
}

fn strip_workspace_root(input: &Value) -> Value {
    if let Value::Object(mut map) = input.clone() {
        map.remove("workspace_root");
        Value::Object(map)
    } else {
        input.clone()
    }
}

fn strip_cwd(input: &Value) -> Value {
    if let Value::Object(mut map) = input.clone() {
        map.remove("cwd");
        Value::Object(map)
    } else {
        input.clone()
    }
}

fn trim_tool_output(output: &Value) -> Value {
    let mut trimmed = output.clone();
    if let Value::Object(map) = &mut trimmed {
        for key in ["input", "driver_class", "tool", "status"] {
            if map.len() > 3 {
                map.remove(key);
            }
        }
        if let Some(content_len) = map.get("content").and_then(Value::as_str).map(str::len) {
            if content_len > 4000 {
                if let Some(Value::String(s)) = map.get_mut("content") {
                    *s = format!("{}(truncated)", truncate(s, 4000));
                }
            }
        }
        if let Some(stdout_len) = map.get("stdout").and_then(Value::as_str).map(str::len) {
            if stdout_len > 4000 {
                if let Some(Value::String(s)) = map.get_mut("stdout") {
                    *s = format!("{}(truncated)", truncate(s, 4000));
                }
            }
        }
        if let Some(stderr_len) = map.get("stderr").and_then(Value::as_str).map(str::len) {
            if stderr_len > 2000 {
                if let Some(Value::String(s)) = map.get_mut("stderr") {
                    *s = format!("{}(truncated)", truncate(s, 2000));
                }
            }
        }
    }
    trimmed
}
