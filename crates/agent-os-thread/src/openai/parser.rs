use crate::{ModelAction, ModelTurnRequest, ModelTurnResponse, ToolAction};
use agent_os_sys::*;
use serde_json::{json, Value};

pub(crate) fn parse_response(
    body: &Value,
    request: &ModelTurnRequest,
) -> AgentOsResult<ModelTurnResponse> {
    let choice = body
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .ok_or_else(|| {
            AgentOsError::Validation("OpenAI response missing choices array".to_string())
        })?;

    let message = choice
        .get("message")
        .ok_or_else(|| AgentOsError::Validation("OpenAI response missing message".to_string()))?;

    let mut actions = Vec::new();

    if let Some(content) = message.get("content").and_then(Value::as_str) {
        if !content.trim().is_empty() {
            actions.push(ModelAction::OutputText {
                text: content.to_string(),
            });
        }
    }

    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        for tc in tool_calls {
            let function = tc.get("function").ok_or_else(|| {
                AgentOsError::Validation("tool_call missing function field".to_string())
            })?;
            let name = function
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    AgentOsError::Validation("tool_call missing function.name".to_string())
                })?;
            let arguments = function
                .get("arguments")
                .map(parse_tool_arguments)
                .unwrap_or_else(|| json!({}));

            if name == "submit_final" {
                let submission = build_final_submission(&arguments, request);
                actions.push(ModelAction::Final { submission });
            } else {
                let (tool_name, input, risk_level) = map_function_call(name, arguments, request);
                let claim = evidence_claim_for_tool(&tool_name);
                actions.push(ModelAction::ToolCall(ToolAction::new(
                    tool_name,
                    input,
                    risk_level,
                    Some(claim),
                )));
            }
        }
    }

    let usage = body
        .get("usage")
        .map(|u| ProviderUsage {
            input_tokens: u.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0),
            output_tokens: u
                .get("completion_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            cost: 0.0,
        })
        .unwrap_or_default();

    if actions.is_empty() {
        actions.push(ModelAction::OutputText {
            text: "(no action from model)".to_string(),
        });
    }

    Ok(ModelTurnResponse { actions, usage })
}

fn parse_tool_arguments(value: &Value) -> Value {
    match value {
        Value::String(arguments) => serde_json::from_str(arguments).unwrap_or_else(|_| json!({})),
        Value::Object(_) => value.clone(),
        _ => json!({}),
    }
}

pub(crate) fn parse_anthropic_response(
    body: &Value,
    request: &ModelTurnRequest,
) -> AgentOsResult<ModelTurnResponse> {
    let content = body
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            AgentOsError::Validation(
                "Anthropic-compatible response missing content array".to_string(),
            )
        })?;

    let mut actions = Vec::new();
    for block in content {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    if !text.trim().is_empty() {
                        actions.push(ModelAction::OutputText {
                            text: text.to_string(),
                        });
                    }
                }
            }
            Some("tool_use") => {
                let name = block
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| AgentOsError::Validation("tool_use missing name".to_string()))?;
                let input = block.get("input").cloned().unwrap_or_else(|| json!({}));
                if name == "submit_final" {
                    let submission = build_final_submission(&input, request);
                    actions.push(ModelAction::Final { submission });
                } else {
                    let (tool_name, input, risk_level) = map_function_call(name, input, request);
                    let claim = evidence_claim_for_tool(&tool_name);
                    actions.push(ModelAction::ToolCall(ToolAction::new(
                        tool_name,
                        input,
                        risk_level,
                        Some(claim),
                    )));
                }
            }
            _ => {}
        }
    }

    let usage = body
        .get("usage")
        .map(|u| ProviderUsage {
            input_tokens: u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0),
            output_tokens: u.get("output_tokens").and_then(Value::as_u64).unwrap_or(0),
            cost: 0.0,
        })
        .unwrap_or_default();

    if actions.is_empty() {
        actions.push(ModelAction::OutputText {
            text: "(no action from model)".to_string(),
        });
    }

    Ok(ModelTurnResponse { actions, usage })
}

pub(crate) fn map_function_call(
    name: &str,
    mut arguments: Value,
    request: &ModelTurnRequest,
) -> (String, Value, u8) {
    let workspace_root = request.workspace_root.to_string_lossy().to_string();
    match name {
        "read_file" => {
            inject_field(&mut arguments, "workspace_root", &workspace_root);
            ("read_file".to_string(), arguments, 1)
        }
        "write_file" => {
            inject_field(&mut arguments, "workspace_root", &workspace_root);
            ("write_file".to_string(), arguments, 4)
        }
        "delete_file" => {
            inject_field(&mut arguments, "workspace_root", &workspace_root);
            ("delete_file".to_string(), arguments, 4)
        }
        "replace_text" => {
            inject_field(&mut arguments, "workspace_root", &workspace_root);
            ("replace_text".to_string(), arguments, 4)
        }
        "run_command" => {
            inject_field(&mut arguments, "cwd", &workspace_root);
            ("run_command".to_string(), arguments, 4)
        }
        "set_objective" => ("set_objective".to_string(), arguments, 2),
        "update_checklist" => ("update_checklist".to_string(), arguments, 2),
        "record_evidence" => ("record_evidence".to_string(), arguments, 2),
        "report_supervisor" => ("report_supervisor".to_string(), arguments, 1),
        "post_blackboard" => ("post_blackboard".to_string(), arguments, 2),
        "ask_human" => ("ask_human".to_string(), arguments, 3),
        "agent_control" => {
            let risk = agent_control_risk(&arguments);
            ("agent_control".to_string(), arguments, risk)
        }
        _ => (name.to_string(), arguments, 1),
    }
}

fn agent_control_risk(arguments: &Value) -> u8 {
    match arguments.get("action").and_then(Value::as_str) {
        Some("status" | "output" | "export_trace") => 1,
        Some("kill" | "delete_session" | "purge_state") => 6,
        Some("start" | "set_hook" | "send" | "resume" | "stop" | "set_timeout") => 4,
        _ => 4,
    }
}

fn inject_field(arguments: &mut Value, key: &str, value: &str) {
    if let Value::Object(map) = arguments {
        map.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn evidence_claim_for_tool(tool_name: &str) -> String {
    match tool_name {
        "read_file" => "file contents were read from the workspace".to_string(),
        "write_file" => "file was written to the workspace".to_string(),
        "delete_file" => "file was deleted from the workspace".to_string(),
        "replace_text" => "exact text replacement was applied to a workspace file".to_string(),
        "run_command" => "command was executed and output captured".to_string(),
        "set_objective" => "task objective was updated in Agent-OS work state".to_string(),
        "update_checklist" => "task checklist was updated in Agent-OS work state".to_string(),
        "record_evidence" => "evidence was recorded in Agent-OS".to_string(),
        "report_supervisor" => "status report was sent to the Supervisor route".to_string(),
        "post_blackboard" => "blackboard entry was published with scoped provenance".to_string(),
        "ask_human" => "human question was routed through Agent-OS".to_string(),
        "agent_control" => "agent supervision action was recorded".to_string(),
        _ => "tool was executed through the kernel tool broker".to_string(),
    }
}

fn build_final_submission(arguments: &Value, request: &ModelTurnRequest) -> FinalSubmission {
    let summary = arguments
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("Task completed")
        .to_string();
    let tests_run = arguments
        .get("tests_run")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let known_risks = arguments
        .get("known_risks")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    let evidence_map: Vec<EvidenceMapEntry> = request
        .tool_results
        .iter()
        .filter(|r| !r.evidence_ids.is_empty())
        .map(|r| EvidenceMapEntry {
            claim: r
                .evidence_claim
                .clone()
                .unwrap_or_else(|| format!("tool {} completed with evidence", r.tool_name)),
            evidence_refs: r.evidence_ids.clone(),
        })
        .collect();

    let changed_artifacts: Vec<String> = request
        .artifacts
        .iter()
        .map(|a| a.artifact_id.clone())
        .collect();

    FinalSubmission {
        summary,
        changed_artifacts,
        evidence_map,
        unverified_claims: Vec::new(),
        known_risks,
        tests_run,
        tests_not_run: Vec::new(),
        approvals: Vec::new(),
    }
}
