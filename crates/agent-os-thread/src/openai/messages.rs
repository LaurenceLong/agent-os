use super::audit::truncate;
use super::prompt::default_system_prompt;
use crate::{ModelTurnRequest, ToolExecutionRecord};
use agent_os_sys::ToolCallStatus;
use serde_json::{json, Value};

const RUNTIME_FEEDBACK_TOOL: &str = "runtime_feedback";

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

    for result in &request.context.tool_results {
        inject_tool_result_messages(&mut messages, result, request, workspace_root);
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

    for result in &request.context.tool_results {
        inject_anthropic_tool_result_messages(&mut messages, result, request);
    }

    messages
}

fn format_user_task_message(request: &ModelTurnRequest, workspace_root: &str) -> String {
    format!(
        "Task: {}\n\n{}\n\nWorkspace: {}",
        request.thread.task.goal,
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
    request: &ModelTurnRequest,
    _workspace_root: &str,
) {
    if result.tool_name == RUNTIME_FEEDBACK_TOOL {
        messages.push(json!({
            "role": "user",
            "content": format_runtime_feedback(result),
        }));
        return;
    }
    if result.tool_name == "read_image" {
        inject_openai_read_image_result_messages(
            messages,
            result,
            request.model_capabilities.image_input,
        );
        return;
    }

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
            let mut trimmed = trim_tool_output(output);
            attach_evidence_ids(&mut trimmed, result);
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

fn attach_evidence_ids(value: &mut Value, result: &ToolExecutionRecord) {
    if result.evidence_ids.is_empty() {
        return;
    }
    match value {
        Value::Object(map) => {
            map.insert("evidence_ids".to_string(), json!(result.evidence_ids));
        }
        other => {
            *other = json!({
                "output": other.clone(),
                "evidence_ids": result.evidence_ids,
            });
        }
    }
}

fn inject_openai_read_image_result_messages(
    messages: &mut Vec<Value>,
    result: &ToolExecutionRecord,
    image_input_supported: bool,
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

    messages.push(json!({
        "role": "tool",
        "tool_call_id": call_id,
        "content": truncate(&read_image_result_text(result), 8000),
    }));

    let Some(image) = read_image_payload(result) else {
        return;
    };
    if !image_input_supported {
        messages.push(json!({
            "role": "user",
            "content": unsupported_image_input_text(&image.path),
        }));
        return;
    }
    messages.push(json!({
        "role": "user",
        "content": [
            {
                "type": "text",
                "text": format!("Image loaded from read_image: {}", image.path)
            },
            {
                "type": "image_url",
                "image_url": {
                    "url": image.data_url
                }
            }
        ]
    }));
}

fn inject_anthropic_tool_result_messages(
    messages: &mut Vec<Value>,
    result: &ToolExecutionRecord,
    request: &ModelTurnRequest,
) {
    if result.tool_name == RUNTIME_FEEDBACK_TOOL {
        messages.push(json!({
            "role": "user",
            "content": format_runtime_feedback(result),
        }));
        return;
    }
    if result.tool_name == "read_image" {
        inject_anthropic_read_image_result_messages(
            messages,
            result,
            request.model_capabilities.image_input,
        );
        return;
    }

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
            let mut trimmed = trim_tool_output(output);
            attach_evidence_ids(&mut trimmed, result);
            serde_json::to_string(&trimmed).unwrap_or_else(|_| "{}".to_string())
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

fn inject_anthropic_read_image_result_messages(
    messages: &mut Vec<Value>,
    result: &ToolExecutionRecord,
    image_input_supported: bool,
) {
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

    let Some(image) = read_image_payload(result) else {
        messages.push(json!({
            "role": "user",
            "content": [{
                "type": "tool_result",
                "tool_use_id": result.call_id,
                "content": truncate(&read_image_result_text(result), 8000)
            }]
        }));
        return;
    };
    if !image_input_supported {
        messages.push(json!({
            "role": "user",
            "content": [{
                "type": "tool_result",
                "tool_use_id": result.call_id,
                "content": unsupported_image_input_text(&image.path)
            }]
        }));
        return;
    }
    messages.push(json!({
        "role": "user",
        "content": [{
            "type": "tool_result",
            "tool_use_id": result.call_id,
            "content": [
                {
                    "type": "text",
                    "text": format!(
                        "Image loaded from read_image: {}\n{}",
                        image.path,
                        read_image_result_text(result)
                    )
                },
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": image.mime_type,
                        "data": image.base64_data
                    }
                }
            ]
        }]
    }));
}

fn format_runtime_feedback(result: &ToolExecutionRecord) -> String {
    let Some(output) = &result.output else {
        return "Runtime feedback: previous response did not include a tool call or final submission.".to_string();
    };
    let message = output
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("Previous response did not include a tool call or final submission.");
    let excerpt = output
        .get("text_excerpt")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if excerpt.trim().is_empty() {
        format!("Runtime feedback: {message}")
    } else {
        format!("Runtime feedback: {message}\nPrevious text excerpt:\n{excerpt}")
    }
}

fn reconstruct_call(result: &ToolExecutionRecord) -> (String, Value) {
    let input = result.input.clone().unwrap_or_else(|| {
        result
            .output
            .as_ref()
            .and_then(|o| o.get("input"))
            .cloned()
            .unwrap_or_else(|| json!({}))
    });

    match result.tool_name.as_str() {
        "read_file" => ("read_file".to_string(), strip_workspace_root(&input)),
        "read_image" => ("read_image".to_string(), strip_workspace_root(&input)),
        "apply_patch" => ("apply_patch".to_string(), strip_workspace_root(&input)),
        "run_command" => ("run_command".to_string(), strip_cwd(&input)),
        known @ ("set_goal"
        | "accomplish_goal"
        | "update_checklist"
        | "record_evidence"
        | "report_supervisor"
        | "post_blackboard"
        | "ask_human"
        | "request_permissions"
        | "load_skill"
        | "read_skill_resource"
        | "agent_control") => (known.to_string(), input),
        name if name.starts_with("mcp__") => (name.to_string(), input),
        _ => (result.tool_name.clone(), input),
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
        if map.len() > 3 {
            map.remove("data_url");
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

struct ReadImagePayload {
    path: String,
    mime_type: String,
    data_url: String,
    base64_data: String,
}

fn read_image_payload(result: &ToolExecutionRecord) -> Option<ReadImagePayload> {
    if result.status != ToolCallStatus::Completed {
        return None;
    }
    let output = result.output.as_ref()?;
    let path = output.get("path")?.as_str()?.to_string();
    let mime_type = output.get("mime_type")?.as_str()?.to_string();
    let data_url = output.get("data_url")?.as_str()?.to_string();
    let prefix = format!("data:{mime_type};base64,");
    let base64_data = data_url.strip_prefix(&prefix)?.to_string();
    if base64_data.is_empty() {
        return None;
    }
    Some(ReadImagePayload {
        path,
        mime_type,
        data_url,
        base64_data,
    })
}

fn read_image_result_text(result: &ToolExecutionRecord) -> String {
    let Some(output) = &result.output else {
        return format!(
            "{{\"status\": \"{}\"}}",
            format!("{:?}", result.status).to_lowercase()
        );
    };
    let mut trimmed = trim_tool_output(output);
    attach_evidence_ids(&mut trimmed, result);
    serde_json::to_string(&trimmed).unwrap_or_else(|_| "{}".to_string())
}

fn unsupported_image_input_text(path: &str) -> String {
    format!(
        "ERROR: Cannot read image \"{path}\" (this model does not support image input). Inform the user."
    )
}
