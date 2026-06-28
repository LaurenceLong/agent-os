use super::support::*;
use super::*;

#[test]
fn build_messages_includes_system_and_user() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-test-{}", new_id("t_")));
    let request = make_request(&tmp);
    let messages = build_messages(&request, tmp.to_str().unwrap(), &None);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["role"], "system");
    assert!(messages[0]["content"]
        .as_str()
        .unwrap()
        .contains("Agent-OS"));
    assert_eq!(messages[1]["role"], "user");
    assert!(messages[1]["content"]
        .as_str()
        .unwrap()
        .contains("Write hello world"));
}

#[test]
fn default_system_prompt_generates_tool_contract() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-prompt-{}", new_id("t_")));
    let request = make_request(&tmp);
    let prompt = default_system_prompt(&request, tmp.to_str().unwrap());

    assert!(prompt.contains("## Available tools"));
    assert!(prompt.contains("read_file(path)"));
    assert!(prompt.contains("write_file(path, content)"));
    assert!(prompt.contains("replace_text(path, old, new)"));
    assert!(prompt.contains("delete_file(path)"));
    assert!(prompt.contains("run_command(program, args)"));
    assert!(prompt.contains("set_objective(objective)"));
    assert!(prompt.contains("update_checklist(items)"));
    assert!(prompt.contains("record_evidence(evidence_type, claim)"));
    assert!(prompt.contains("report_supervisor(message)"));
    assert!(prompt.contains("post_blackboard(channel_id, section, content)"));
    assert!(prompt.contains("ask_human(question)"));
    assert!(prompt.contains("agent_control(action, agent_id, thread_id, payload)"));
    assert!(prompt.contains("Host OS tools"));
    assert!(prompt.contains("Work State tools"));
    assert!(prompt.contains("Communication tools"));
    assert!(prompt.contains("Session Lifecycle"));
    assert!(prompt.contains("For agent_control, use one action per call"));
    assert!(prompt.contains("Paths are relative to the workspace root"));
    assert!(!prompt.contains("workspace.read_file"));
    assert!(!prompt.contains("process.run"));
}

#[test]
fn build_messages_includes_tool_results() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-tr-{}", new_id("t_")));
    let request = ModelTurnRequest {
        thread: make_request(&tmp).thread,
        workspace_root: tmp.clone(),
        step_index: 1,
        tool_results: vec![ToolExecutionRecord {
            call_id: "call_001".to_string(),
            tool_name: "read_file".to_string(),
            status: ToolCallStatus::Completed,
            input: None,
            output: Some(json!({
                "tool": "read_file",
                "status": "ok",
                "input": {"workspace_root": tmp.to_string_lossy(), "path": "README.md"},
                "content": "# Hello",
                "bytes_read": 7,
                "path": "README.md",
            })),
            evidence_ids: vec!["evi_1".to_string()],
            evidence_claim: Some("read README.md".to_string()),
        }],
        artifacts: Vec::new(),
    };
    let messages = build_messages(&request, tmp.to_str().unwrap(), &None);
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[2]["role"], "assistant");
    assert_eq!(
        messages[2]["tool_calls"][0]["function"]["name"],
        "read_file"
    );
    assert_eq!(messages[3]["role"], "tool");
    assert_eq!(messages[3]["tool_call_id"], "call_001");
}

#[test]
fn parse_response_extracts_tool_calls() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-pr-{}", new_id("t_")));
    let request = make_request(&tmp);
    let body = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_abc",
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "arguments": "{\"path\": \"src/main.rs\"}"
                    }
                }]
            }
        }],
        "usage": {"prompt_tokens": 100, "completion_tokens": 20}
    });
    let response = parse_response(&body, &request).unwrap();
    assert_eq!(response.usage.input_tokens, 100);
    assert_eq!(response.actions.len(), 1);
    match &response.actions[0] {
        ModelAction::ToolCall(action) => {
            assert_eq!(action.tool_name, "read_file");
            assert_eq!(action.input["path"], "src/main.rs");
            assert_eq!(
                action.input["workspace_root"],
                tmp.to_string_lossy().to_string()
            );
        }
        _ => panic!("expected ToolCall"),
    }
}

#[test]
fn parse_response_accepts_object_arguments_for_compatibility() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-obj-{}", new_id("t_")));
    let request = make_request(&tmp);
    let body = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_obj",
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "arguments": {"path": "src/lib.rs"}
                    }
                }]
            }
        }]
    });
    let response = parse_response(&body, &request).unwrap();
    match &response.actions[0] {
        ModelAction::ToolCall(action) => assert_eq!(action.input["path"], "src/lib.rs"),
        _ => panic!("expected ToolCall"),
    }
}

#[test]
fn parse_anthropic_response_extracts_tool_use() {
    let tmp = std::env::temp_dir().join(format!("aos-anthropic-tu-{}", new_id("t_")));
    let request = make_request(&tmp);
    let body = json!({
        "content": [{
            "type": "tool_use",
            "id": "toolu_1",
            "name": "write_file",
            "input": {"path": "out.txt", "content": "hello\n"}
        }],
        "usage": {"input_tokens": 8, "output_tokens": 5}
    });
    let response = parse_anthropic_response(&body, &request).unwrap();
    assert_eq!(response.usage.output_tokens, 5);
    match &response.actions[0] {
        ModelAction::ToolCall(action) => {
            assert_eq!(action.tool_name, "write_file");
            assert_eq!(action.input["path"], "out.txt");
            assert_eq!(
                action.input["workspace_root"],
                tmp.to_string_lossy().to_string()
            );
        }
        _ => panic!("expected ToolCall"),
    }
}

#[test]
fn parse_response_extracts_submit_final() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-sf-{}", new_id("t_")));
    let request = ModelTurnRequest {
        thread: make_request(&tmp).thread,
        workspace_root: tmp,
        step_index: 3,
        tool_results: vec![ToolExecutionRecord {
            call_id: "call_1".to_string(),
            tool_name: "write_file".to_string(),
            status: ToolCallStatus::Completed,
            input: None,
            output: Some(json!({"input": {"path": "out.txt"}})),
            evidence_ids: vec!["evi_final".to_string()],
            evidence_claim: Some("wrote output".to_string()),
        }],
        artifacts: vec![ArtifactRecord {
            artifact_id: "art_1".to_string(),
            artifact_type: ArtifactType::Patch,
            blob_ref: None,
            evidence_ids: vec![],
        }],
    };
    let body = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "Done!",
                "tool_calls": [{
                    "id": "call_done",
                    "type": "function",
                    "function": {
                        "name": "submit_final",
                        "arguments": "{\"summary\": \"Task completed\", \"tests_run\": [\"cargo test\"]}"
                    }
                }]
            }
        }],
        "usage": {"prompt_tokens": 50, "completion_tokens": 10}
    });
    let response = parse_response(&body, &request).unwrap();
    assert_eq!(response.actions.len(), 2);
    match &response.actions[0] {
        ModelAction::OutputText { text } => assert_eq!(text, "Done!"),
        _ => panic!("expected OutputText"),
    }
    match &response.actions[1] {
        ModelAction::Final { submission } => {
            assert_eq!(submission.summary, "Task completed");
            assert_eq!(submission.tests_run, vec!["cargo test"]);
            assert_eq!(submission.changed_artifacts, vec!["art_1"]);
            assert_eq!(submission.evidence_map.len(), 1);
            assert_eq!(submission.evidence_map[0].evidence_refs, vec!["evi_final"]);
        }
        _ => panic!("expected Final"),
    }
}

#[test]
fn parse_anthropic_response_extracts_submit_final() {
    let tmp = std::env::temp_dir().join(format!("aos-anthropic-sf-{}", new_id("t_")));
    let request = ModelTurnRequest {
        thread: make_request(&tmp).thread,
        workspace_root: tmp,
        step_index: 2,
        tool_results: vec![ToolExecutionRecord {
            call_id: "call_1".to_string(),
            tool_name: "write_file".to_string(),
            status: ToolCallStatus::Completed,
            input: Some(json!({"path": "out.txt"})),
            output: Some(json!({"written_path": "out.txt"})),
            evidence_ids: vec!["evi_anthropic".to_string()],
            evidence_claim: Some("wrote output".to_string()),
        }],
        artifacts: vec![ArtifactRecord {
            artifact_id: "art_1".to_string(),
            artifact_type: ArtifactType::Patch,
            blob_ref: None,
            evidence_ids: vec![],
        }],
    };
    let body = json!({
        "content": [{
            "type": "tool_use",
            "id": "toolu_final",
            "name": "submit_final",
            "input": {
                "summary": "Anthropic path complete",
                "tests_run": ["cargo test -p agent-os-thread openai::tests"]
            }
        }],
        "usage": {"input_tokens": 20, "output_tokens": 6}
    });
    let response = parse_anthropic_response(&body, &request).unwrap();
    match &response.actions[0] {
        ModelAction::Final { submission } => {
            assert_eq!(submission.summary, "Anthropic path complete");
            assert_eq!(submission.changed_artifacts, vec!["art_1"]);
            assert_eq!(
                submission.evidence_map[0].evidence_refs,
                vec!["evi_anthropic"]
            );
        }
        _ => panic!("expected Final"),
    }
}

#[test]
fn parse_response_handles_text_only() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-to-{}", new_id("t_")));
    let request = make_request(&tmp);
    let body = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "I'll start by reading the file."
            }
        }],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5}
    });
    let response = parse_response(&body, &request).unwrap();
    assert_eq!(response.actions.len(), 1);
    match &response.actions[0] {
        ModelAction::OutputText { text } => {
            assert!(text.contains("reading the file"));
        }
        _ => panic!("expected OutputText"),
    }
}

#[test]
fn tool_definitions_include_all_core_tools() {
    let tools = tool_definitions();
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| {
            t.get("function")
                .and_then(|f| f.get("name"))
                .and_then(Value::as_str)
        })
        .collect();
    assert_eq!(names.len(), 13);
    assert!(names.contains(&"read_file"));
    assert!(names.contains(&"write_file"));
    assert!(names.contains(&"replace_text"));
    assert!(names.contains(&"delete_file"));
    assert!(names.contains(&"run_command"));
    assert!(names.contains(&"set_objective"));
    assert!(names.contains(&"update_checklist"));
    assert!(names.contains(&"record_evidence"));
    assert!(names.contains(&"report_supervisor"));
    assert!(names.contains(&"post_blackboard"));
    assert!(names.contains(&"ask_human"));
    assert!(names.contains(&"agent_control"));
    assert!(names.contains(&"submit_final"));
}

#[test]
fn anthropic_tool_definitions_mirror_core_tools() {
    let openai_names: Vec<String> = tool_definitions()
        .iter()
        .filter_map(|tool| {
            tool.get("function")
                .and_then(|function| function.get("name"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect();
    let anthropic_tools = anthropic_tool_definitions();
    let anthropic_names: Vec<String> = anthropic_tools
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_string))
        .collect();
    assert_eq!(anthropic_names, openai_names);
    assert!(anthropic_tools
        .iter()
        .all(|tool| tool.get("input_schema").is_some()));
}

#[test]
fn map_function_call_injects_workspace_root() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-mf-{}", new_id("t_")));
    let request = make_request(&tmp);
    let (tool_name, input, risk) = map_function_call(
        "write_file",
        json!({"path": "test.rs", "content": "fn main() {}"}),
        &request,
    );
    assert_eq!(tool_name, "write_file");
    assert_eq!(input["workspace_root"], tmp.to_string_lossy().to_string());
    assert_eq!(risk, 4);
}

#[test]
fn map_function_call_supports_delete_file() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-md-{}", new_id("t_")));
    let request = make_request(&tmp);
    let (tool_name, input, risk) =
        map_function_call("delete_file", json!({"path": "old.txt"}), &request);
    assert_eq!(tool_name, "delete_file");
    assert_eq!(input["workspace_root"], tmp.to_string_lossy().to_string());
    assert_eq!(risk, 4);
}

#[test]
fn map_function_call_supports_agent_control_actions() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-ac-{}", new_id("t_")));
    let request = make_request(&tmp);
    let (tool_name, input, risk) = map_function_call(
        "agent_control",
        json!({
            "action": "start",
            "payload": {
                "assignment": "inspect the demo",
                "hooks": [{
                    "interval_seconds": 60,
                    "prompt": "Report concise progress."
                }]
            }
        }),
        &request,
    );
    assert_eq!(tool_name, "agent_control");
    assert_eq!(input["action"], "start");
    assert_eq!(risk, 4);
}

#[test]
fn api_style_parses_explicit_and_base_url_values() {
    assert_eq!(
        LlmApiStyle::from_value("openai-compatible").unwrap(),
        LlmApiStyle::OpenAiCompatible
    );
    assert_eq!(
        LlmApiStyle::from_value("anthropic").unwrap(),
        LlmApiStyle::AnthropicCompatible
    );
    std::env::remove_var("LLM_API_STYLE");
    std::env::remove_var("AGENT_OS_API_STYLE");
    assert_eq!(
        LlmApiStyle::from_env_or_base("http://model.mify.ai.srv/anthropic").unwrap(),
        LlmApiStyle::AnthropicCompatible
    );
    assert_eq!(
        LlmApiStyle::from_env_or_base("http://model.mify.ai.srv/v1").unwrap(),
        LlmApiStyle::OpenAiCompatible
    );
}

#[test]
fn from_env_fails_without_api_key() {
    std::env::remove_var("LLM_API_KEY");
    std::env::remove_var("OPENAI_API_KEY");
    let result = OpenAiModelClient::from_env();
    assert!(result.is_err());
}
