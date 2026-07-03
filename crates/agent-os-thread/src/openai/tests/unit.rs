use super::support::*;
use super::*;
use crate::openai::tools::{
    anthropic_tool_definitions_for_request, anthropic_tool_definitions_for_thread,
    tool_definitions_for_request, tool_definitions_for_thread,
};

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

    assert!(prompt.contains("## Visible Tool Summary"));
    assert!(prompt.contains("Producer responsibility:"));
    assert!(prompt.contains("- glob_files:"));
    assert!(prompt.contains("- grep_files:"));
    assert!(prompt.contains("- read_file:"));
    assert!(prompt.contains("- read_image:"));
    assert!(prompt.contains("- apply_patch:"));
    assert!(prompt.contains("run_command"));
    assert!(prompt.contains(&format!("Host OS: {}", std::env::consts::OS)));
    assert!(prompt.contains("Gather context with the most appropriate visible tool"));
    assert!(prompt.contains("Run a shell command in the workspace by default"));
    assert!(!prompt.contains(r#"program "cat" with args ["file.txt"]"#));
    assert!(!prompt.contains("set_goal(goal, target_thread_id, target_agent_id)"));
    assert!(prompt.contains("- accomplish_goal:"));
    assert!(prompt.contains("- update_checklist:"));
    assert!(prompt.contains("- record_evidence:"));
    assert!(prompt.contains("- report_supervisor:"));
    assert!(prompt.contains("- post_blackboard:"));
    assert!(!prompt.contains("ask_human(question)"));
    assert!(prompt.contains("- request_permissions:"));
    assert!(!prompt.contains("agent_control(action, agent_id, thread_id, payload)"));
    assert!(prompt.contains("coordinate with or escalate to the Supervisor"));
    assert!(!prompt.contains("supervise and escalate"));
    assert!(!prompt.contains("When answering a child permission request"));
    assert!(prompt.contains("Paths are relative to the workspace root"));
    assert!(!prompt.contains("write_file(path, content)"));
    assert!(!prompt.contains("replace_text(path, old, new)"));
    assert!(!prompt.contains("delete_file(path)"));
    assert!(!prompt.contains("workspace.read_file"));
    assert!(!prompt.contains("process.run"));
}

#[test]
fn default_system_prompt_projects_supervisor_control_plane_tools() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-supervisor-prompt-{}", new_id("t_")));
    let request = make_kernel_request_for_role(
        &tmp,
        "role_supervisor",
        "Coordinate child work",
        vec!["child work is assigned".to_string()],
    )
    .1;
    let prompt = default_system_prompt(&request, tmp.to_str().unwrap());

    assert!(prompt.contains("set_goal"));
    assert!(prompt.contains("agent_control"));
    assert!(prompt.contains("Supervisor responsibility:"));
    assert!(prompt.contains("For agent_control, use one action per call"));
    assert!(prompt.contains("When answering a child permission request"));
    assert!(!prompt.contains("If agent_control or set_goal is not visible"));
}

#[test]
fn default_system_prompt_projects_reviewer_responsibility() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-reviewer-prompt-{}", new_id("t_")));
    let request = make_kernel_request_for_role(
        &tmp,
        "role_reviewer",
        "Review the proposed patch",
        vec!["review findings cite evidence".to_string()],
    )
    .1;
    let prompt = default_system_prompt(&request, tmp.to_str().unwrap());

    assert!(prompt.contains("Reviewer responsibility:"));
    assert!(prompt.contains("producer-equivalent baseline capability"));
    assert!(prompt.contains("must not mutate the artifact under review"));
    assert!(prompt.contains("coordinate with or escalate to the Supervisor"));
    assert!(!prompt.contains("When answering a child permission request"));
}

#[test]
fn default_system_prompt_projects_ecosystem_without_inlining_skill_body() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-ecosystem-{}", new_id("t_")));
    let mut request = make_request(&tmp);
    request
        .context
        .instruction_documents
        .push(InstructionDocument {
            instruction_id: "inst_1".to_string(),
            source: EcosystemSource {
                source_kind: EcosystemSourceKind::AgentOs,
                source_scope: EcosystemSourceScope::Project,
                source_path: "AGENTS.md".to_string(),
            },
            precedence_rank: 0,
            content: "Project rule: load matching skills on demand.".to_string(),
            content_hash: "hash_inst".to_string(),
            created_at: now_rfc3339(),
        });
    request.context.skill_definitions.push(SkillDefinition {
        skill_id: "skill_1".to_string(),
        name: "review-skill".to_string(),
        description: "Review code with local criteria.".to_string(),
        root_path: ".agent-os/skills/review-skill".to_string(),
        skill_file_path: ".agent-os/skills/review-skill/SKILL.md".to_string(),
        source: EcosystemSource {
            source_kind: EcosystemSourceKind::AgentOs,
            source_scope: EcosystemSourceScope::Project,
            source_path: ".agent-os/skills/review-skill/SKILL.md".to_string(),
        },
        content: "SECRET_SKILL_BODY".to_string(),
        metadata: std::collections::BTreeMap::new(),
        content_hash: "hash_skill".to_string(),
        created_at: now_rfc3339(),
    });

    let prompt = default_system_prompt(&request, tmp.to_str().unwrap());
    assert!(prompt.contains("## Imported Instructions"));
    assert!(prompt.contains("Project rule: load matching skills on demand."));
    assert!(prompt.contains("## Available Skills"));
    assert!(prompt.contains("review-skill: Review code with local criteria."));
    assert!(prompt.contains("Use load_skill(name) before following a skill"));
    assert!(!prompt.contains("SECRET_SKILL_BODY"));
}

#[test]
fn build_messages_includes_tool_results() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-tr-{}", new_id("t_")));
    let request = ModelTurnRequest {
        thread: make_request(&tmp).thread,
        workspace_root: tmp.clone(),
        step_index: 1,
        model_capabilities: image_capable_model(),
        context: ModelContextProjection {
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
            ..ModelContextProjection::default()
        },
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
fn build_messages_projects_read_image_as_openai_image_part() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-image-{}", new_id("t_")));
    let request = ModelTurnRequest {
        thread: make_request(&tmp).thread,
        workspace_root: tmp.clone(),
        step_index: 1,
        model_capabilities: image_capable_model(),
        context: ModelContextProjection {
            tool_results: vec![ToolExecutionRecord {
                call_id: "call_image".to_string(),
                tool_name: "read_image".to_string(),
                status: ToolCallStatus::Completed,
                input: Some(json!({"workspace_root": tmp.to_string_lossy(), "path": "shot.png"})),
                output: Some(json!({
                    "tool": "read_image",
                    "status": "ok",
                    "input": {"workspace_root": tmp.to_string_lossy(), "path": "shot.png"},
                    "path": "shot.png",
                    "mime_type": "image/png",
                    "encoding": "base64",
                    "data_url": "data:image/png;base64,AA==",
                    "bytes_read": 1
                })),
                evidence_ids: Vec::new(),
                evidence_claim: None,
            }],
            ..ModelContextProjection::default()
        },
    };

    let messages = build_messages(&request, tmp.to_str().unwrap(), &None);

    assert_eq!(messages.len(), 5);
    assert_eq!(
        messages[2]["tool_calls"][0]["function"]["name"],
        "read_image"
    );
    assert_eq!(messages[3]["role"], "tool");
    assert!(!messages[3]["content"]
        .as_str()
        .unwrap()
        .contains("data:image/png"));
    assert_eq!(messages[4]["role"], "user");
    assert_eq!(messages[4]["content"][1]["type"], "image_url");
    assert_eq!(
        messages[4]["content"][1]["image_url"]["url"],
        "data:image/png;base64,AA=="
    );
}

#[test]
fn build_messages_reports_read_image_error_for_text_only_model() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-image-text-{}", new_id("t_")));
    let mut capabilities = image_capable_model();
    capabilities.image_input = false;
    let request = ModelTurnRequest {
        thread: make_request(&tmp).thread,
        workspace_root: tmp.clone(),
        step_index: 1,
        model_capabilities: capabilities,
        context: ModelContextProjection {
            tool_results: vec![ToolExecutionRecord {
                call_id: "call_image".to_string(),
                tool_name: "read_image".to_string(),
                status: ToolCallStatus::Completed,
                input: Some(json!({"workspace_root": tmp.to_string_lossy(), "path": "shot.png"})),
                output: Some(json!({
                    "path": "shot.png",
                    "mime_type": "image/png",
                    "encoding": "base64",
                    "data_url": "data:image/png;base64,AA==",
                    "bytes_read": 1
                })),
                evidence_ids: Vec::new(),
                evidence_claim: None,
            }],
            ..ModelContextProjection::default()
        },
    };

    let messages = build_messages(&request, tmp.to_str().unwrap(), &None);

    assert_eq!(messages.len(), 5);
    assert!(messages[4]["content"]
        .as_str()
        .unwrap()
        .contains("does not support image input"));
}

#[test]
fn build_anthropic_messages_projects_read_image_as_image_block() {
    let tmp = std::env::temp_dir().join(format!("aos-anthropic-image-{}", new_id("t_")));
    let request = ModelTurnRequest {
        thread: make_request(&tmp).thread,
        workspace_root: tmp.clone(),
        step_index: 1,
        model_capabilities: image_capable_model(),
        context: ModelContextProjection {
            tool_results: vec![ToolExecutionRecord {
                call_id: "call_image".to_string(),
                tool_name: "read_image".to_string(),
                status: ToolCallStatus::Completed,
                input: Some(json!({"workspace_root": tmp.to_string_lossy(), "path": "shot.png"})),
                output: Some(json!({
                    "path": "shot.png",
                    "mime_type": "image/png",
                    "encoding": "base64",
                    "data_url": "data:image/png;base64,AA==",
                    "bytes_read": 1
                })),
                evidence_ids: vec!["evi_image".to_string()],
                evidence_claim: Some("read image".to_string()),
            }],
            ..ModelContextProjection::default()
        },
    };

    let messages = build_anthropic_messages(&request, tmp.to_str().unwrap());

    assert_eq!(messages.len(), 3);
    assert_eq!(messages[2]["content"][0]["type"], "tool_result");
    let text = messages[2]["content"][0]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(text.contains("evi_image"));
    assert!(!text.contains("data:image/png"));
    assert_eq!(messages[2]["content"][0]["content"][1]["type"], "image");
    assert_eq!(
        messages[2]["content"][0]["content"][1]["source"]["media_type"],
        "image/png"
    );
    assert_eq!(
        messages[2]["content"][0]["content"][1]["source"]["data"],
        "AA=="
    );
}

#[test]
fn build_anthropic_messages_reports_read_image_error_for_text_only_model() {
    let tmp = std::env::temp_dir().join(format!("aos-anthropic-image-text-{}", new_id("t_")));
    let mut capabilities = image_capable_model();
    capabilities.image_input = false;
    let request = ModelTurnRequest {
        thread: make_request(&tmp).thread,
        workspace_root: tmp.clone(),
        step_index: 1,
        model_capabilities: capabilities,
        context: ModelContextProjection {
            tool_results: vec![ToolExecutionRecord {
                call_id: "call_image".to_string(),
                tool_name: "read_image".to_string(),
                status: ToolCallStatus::Completed,
                input: Some(json!({"workspace_root": tmp.to_string_lossy(), "path": "shot.png"})),
                output: Some(json!({
                    "path": "shot.png",
                    "mime_type": "image/png",
                    "encoding": "base64",
                    "data_url": "data:image/png;base64,AA==",
                    "bytes_read": 1
                })),
                evidence_ids: Vec::new(),
                evidence_claim: None,
            }],
            ..ModelContextProjection::default()
        },
    };

    let messages = build_anthropic_messages(&request, tmp.to_str().unwrap());

    assert_eq!(messages.len(), 3);
    assert_eq!(messages[2]["content"][0]["type"], "tool_result");
    assert!(messages[2]["content"][0]["content"]
        .as_str()
        .unwrap()
        .contains("does not support image input"));
}

#[test]
fn build_messages_projects_runtime_feedback_as_user_text() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-feedback-{}", new_id("t_")));
    let request = ModelTurnRequest {
        thread: make_request(&tmp).thread,
        workspace_root: tmp.clone(),
        step_index: 2,
        model_capabilities: image_capable_model(),
        context: ModelContextProjection {
            tool_results: vec![ToolExecutionRecord {
                call_id: "feedback_001".to_string(),
                tool_name: "runtime_feedback".to_string(),
                status: ToolCallStatus::Failed,
                input: Some(json!({"step_index": 1})),
                output: Some(json!({
                    "message": "The previous model response had no tool call or final submission.",
                    "text_excerpt": "I will inspect the repository."
                })),
                evidence_ids: Vec::new(),
                evidence_claim: None,
            }],
            ..ModelContextProjection::default()
        },
    };

    let messages = build_messages(&request, tmp.to_str().unwrap(), &None);

    assert_eq!(messages.len(), 3);
    assert_eq!(messages[2]["role"], "user");
    assert!(messages[2]["content"]
        .as_str()
        .unwrap()
        .contains("Runtime feedback"));
    assert!(messages[2].get("tool_calls").is_none());
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
fn parse_response_accepts_object_arguments_from_provider() {
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
            "name": "apply_patch",
            "input": {"patch": "*** Begin Patch\n*** Add File: out.txt\n+hello\n*** End Patch\n"}
        }],
        "usage": {"input_tokens": 8, "output_tokens": 5}
    });
    let response = parse_anthropic_response(&body, &request).unwrap();
    assert_eq!(response.usage.output_tokens, 5);
    match &response.actions[0] {
        ModelAction::ToolCall(action) => {
            assert_eq!(action.tool_name, "apply_patch");
            assert!(action.input["patch"].as_str().unwrap().contains("out.txt"));
            assert_eq!(
                action.input["workspace_root"],
                tmp.to_string_lossy().to_string()
            );
        }
        _ => panic!("expected ToolCall"),
    }
}

#[test]
fn parse_openai_responses_response_extracts_function_call() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-resp-fn-{}", new_id("t_")));
    let request = make_request(&tmp);
    let body = json!({
        "output": [
            {
                "type": "message",
                "content": [{"type": "output_text", "text": "I will inspect the file."}]
            },
            {
                "type": "function_call",
                "call_id": "call_read",
                "name": "read_file",
                "arguments": "{\"path\":\"src/lib.rs\"}"
            }
        ],
        "usage": {"input_tokens": 30, "output_tokens": 7}
    });

    let response = parse_openai_responses_response(&body, &request).unwrap();

    assert_eq!(response.usage.input_tokens, 30);
    assert_eq!(response.actions.len(), 2);
    match &response.actions[0] {
        ModelAction::OutputText { text } => assert!(text.contains("inspect")),
        _ => panic!("expected OutputText"),
    }
    match &response.actions[1] {
        ModelAction::ToolCall(action) => {
            assert_eq!(action.tool_name, "read_file");
            assert_eq!(action.input["path"], "src/lib.rs");
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
    let base_request = make_request(&tmp);
    let request = ModelTurnRequest {
        thread: base_request.thread,
        workspace_root: tmp,
        step_index: 3,
        model_capabilities: base_request.model_capabilities,
        context: ModelContextProjection {
            tool_results: vec![ToolExecutionRecord {
                call_id: "call_1".to_string(),
                tool_name: "apply_patch".to_string(),
                status: ToolCallStatus::Completed,
                input: None,
                output: Some(
                    json!({"input": {"patch": "*** Begin Patch\n*** Add File: out.txt\n+done\n*** End Patch\n"}}),
                ),
                evidence_ids: vec!["evi_final".to_string()],
                evidence_claim: Some("wrote output".to_string()),
            }],
            artifacts: vec![ArtifactRecord {
                artifact_id: "art_1".to_string(),
                artifact_type: ArtifactType::Patch,
                blob_ref: None,
                evidence_ids: vec![],
            }],
            tool_descriptors: base_request.context.tool_descriptors,
            ..ModelContextProjection::default()
        },
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
        ModelAction::ToolCall(action) => {
            assert_eq!(action.tool_name, "submit_final");
            assert_eq!(action.risk_level, 2);
            assert_eq!(action.input["summary"], "Task completed");
            assert_eq!(action.input["tests_run"], json!(["cargo test"]));
        }
        _ => panic!("expected submit_final ToolCall"),
    }
}

#[test]
fn parse_anthropic_response_extracts_submit_final() {
    let tmp = std::env::temp_dir().join(format!("aos-anthropic-sf-{}", new_id("t_")));
    let base_request = make_request(&tmp);
    let request = ModelTurnRequest {
        thread: base_request.thread,
        workspace_root: tmp,
        step_index: 2,
        model_capabilities: base_request.model_capabilities,
        context: ModelContextProjection {
            tool_results: vec![ToolExecutionRecord {
                call_id: "call_1".to_string(),
                tool_name: "apply_patch".to_string(),
                status: ToolCallStatus::Completed,
                input: Some(
                    json!({"patch": "*** Begin Patch\n*** Add File: out.txt\n+done\n*** End Patch\n"}),
                ),
                output: Some(json!({"path": "out.txt"})),
                evidence_ids: vec!["evi_anthropic".to_string()],
                evidence_claim: Some("wrote output".to_string()),
            }],
            artifacts: vec![ArtifactRecord {
                artifact_id: "art_1".to_string(),
                artifact_type: ArtifactType::Patch,
                blob_ref: None,
                evidence_ids: vec![],
            }],
            tool_descriptors: base_request.context.tool_descriptors,
            ..ModelContextProjection::default()
        },
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
        ModelAction::ToolCall(action) => {
            assert_eq!(action.tool_name, "submit_final");
            assert_eq!(action.risk_level, 2);
            assert_eq!(action.input["summary"], "Anthropic path complete");
            assert_eq!(
                action.input["tests_run"],
                json!(["cargo test -p agent-os-thread openai::tests"])
            );
        }
        _ => panic!("expected submit_final ToolCall"),
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
    assert_eq!(names.len(), 19);
    assert!(names.contains(&"apply_patch"));
    assert!(names.contains(&"glob_files"));
    assert!(names.contains(&"grep_files"));
    assert!(names.contains(&"read_file"));
    assert!(names.contains(&"read_image"));
    assert!(names.contains(&"run_command"));
    assert!(names.contains(&"set_goal"));
    assert!(names.contains(&"accomplish_goal"));
    assert!(names.contains(&"update_checklist"));
    assert!(names.contains(&"record_evidence"));
    assert!(names.contains(&"report_supervisor"));
    assert!(names.contains(&"post_blackboard"));
    assert!(names.contains(&"ask_human"));
    assert!(names.contains(&"request_permissions"));
    assert!(names.contains(&"load_skill"));
    assert!(names.contains(&"read_skill_resource"));
    assert!(names.contains(&"tool_search"));
    assert!(names.contains(&"agent_control"));
    assert!(names.contains(&"submit_final"));
    assert!(!names.contains(&"write_file"));
    assert!(!names.contains(&"replace_text"));
    assert!(!names.contains(&"delete_file"));
    assert!(!names.contains(&"todo"));
    assert!(!names.contains(&"glob"));
    assert!(!names.contains(&"lsp"));
    assert!(!names.contains(&"webfetch"));
    for tool in &tools {
        let name = tool
            .pointer("/function/name")
            .and_then(Value::as_str)
            .unwrap();
        let description = tool
            .pointer("/function/description")
            .and_then(Value::as_str)
            .unwrap();
        assert!(description.contains("Examples:"), "{name}: {description}");
        assert!(description.contains("parameters:"), "{name}: {description}");
        assert!(
            description.contains("expected_result:"),
            "{name}: {description}"
        );
    }
    let run_command = tools
        .iter()
        .find(|tool| tool.pointer("/function/name").and_then(Value::as_str) == Some("run_command"))
        .unwrap();
    assert_eq!(
        run_command.pointer("/function/parameters/required"),
        Some(&json!(["command"]))
    );
    assert_eq!(
        run_command.pointer("/function/parameters/properties/command/type"),
        Some(&json!("string"))
    );
    assert_eq!(
        run_command.pointer("/function/parameters/properties/env/type"),
        Some(&json!("object"))
    );
}

#[test]
fn apply_patch_tool_schema_projects_exact_patch_markers() {
    let tools = tool_definitions();
    let apply_patch = tools
        .iter()
        .find(|tool| tool.pointer("/function/name").and_then(Value::as_str) == Some("apply_patch"))
        .unwrap();
    let description = apply_patch
        .pointer("/function/description")
        .and_then(Value::as_str)
        .unwrap();
    let patch_description = apply_patch
        .pointer("/function/parameters/properties/patch/description")
        .and_then(Value::as_str)
        .unwrap();

    for text in [description, patch_description] {
        assert!(text.contains("*** Begin Patch"), "{text}");
        assert!(text.contains("*** Add File:"), "{text}");
        assert!(text.contains("*** Update File:"), "{text}");
        assert!(text.contains("*** Delete File:"), "{text}");
        assert!(text.contains("*** End Patch"), "{text}");
    }
    assert!(description.contains("Examples:"), "{description}");
    assert!(description.contains("Add a new file"), "{description}");
    assert!(
        description.contains("*** Add File: docs/example.md"),
        "{description}"
    );
    assert!(
        description.contains("Update an existing file"),
        "{description}"
    );
    assert!(
        description.contains("*** Update File: src/lib.rs"),
        "{description}"
    );
    assert!(
        description.contains("Delete an obsolete file"),
        "{description}"
    );
    assert!(
        description.contains("*** Delete File: tmp/obsolete.txt"),
        "{description}"
    );
    assert!(
        patch_description.contains("plain lines")
            || patch_description.contains("plain context lines"),
        "{patch_description}"
    );
}

#[test]
fn agent_control_tool_schema_warns_against_invented_agent_ids() {
    let tools = tool_definitions();
    let agent_control = tools
        .iter()
        .find(|tool| {
            tool.pointer("/function/name").and_then(Value::as_str) == Some("agent_control")
        })
        .unwrap();
    let description = agent_control
        .pointer("/function/description")
        .and_then(Value::as_str)
        .unwrap();
    let agent_id_description = agent_control
        .pointer("/function/parameters/properties/agent_id/description")
        .and_then(Value::as_str)
        .unwrap();
    let thread_id_description = agent_control
        .pointer("/function/parameters/properties/thread_id/description")
        .and_then(Value::as_str)
        .unwrap();

    for text in [description, agent_id_description, thread_id_description] {
        assert!(text.contains("Do not invent"), "{text}");
        assert!(text.contains("agent_id"), "{text}");
        assert!(text.contains("thread_id"), "{text}");
    }
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
    assert!(anthropic_tools.iter().all(|tool| tool
        .get("description")
        .and_then(Value::as_str)
        .is_some_and(|description| description.contains("Examples:"))));
}

#[test]
fn producer_tool_view_hides_privileged_agent_control_actions() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-producer-tools-{}", new_id("t_")));
    let request = make_request(&tmp);
    let tools = tool_definitions_for_thread(&request.thread);
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|tool| tool.pointer("/function/name").and_then(Value::as_str))
        .collect();
    assert!(!names.contains(&"set_goal"));
    assert!(names.contains(&"accomplish_goal"));
    assert!(names.contains(&"request_permissions"));
    assert!(!names.contains(&"agent_control"));
    let actions = agent_control_actions(&tools);
    assert!(actions.is_empty());
    assert!(!actions.contains(&"kill".to_string()));
    assert!(!actions.contains(&"delete_session".to_string()));
    assert!(!actions.contains(&"purge_state".to_string()));
}

#[test]
fn read_image_tool_is_hidden_for_text_only_models() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-image-tools-{}", new_id("t_")));
    let mut request = make_request(&tmp);
    request.model_capabilities.image_input = false;

    let names = tool_definitions_for_request(&request)
        .into_iter()
        .filter_map(|tool| {
            tool.pointer("/function/name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect::<Vec<_>>();

    assert!(names.contains(&"glob_files".to_string()));
    assert!(names.contains(&"grep_files".to_string()));
    assert!(names.contains(&"read_file".to_string()));
    assert!(!names.contains(&"read_image".to_string()));
}

#[test]
fn reviewer_tool_view_matches_producer_without_control_plane() {
    let producer_tmp =
        std::env::temp_dir().join(format!("aos-openai-producer-tools-{}", new_id("t_")));
    let reviewer_tmp =
        std::env::temp_dir().join(format!("aos-openai-reviewer-tools-{}", new_id("t_")));
    let producer_request = make_request(&producer_tmp);
    let reviewer_request = make_kernel_request_for_role(
        &reviewer_tmp,
        "role_reviewer",
        "Review the proposed artifact",
        vec!["review cites evidence".to_string()],
    )
    .1;
    let producer_tools = tool_definitions_for_thread(&producer_request.thread);
    let reviewer_tools = tool_definitions_for_thread(&reviewer_request.thread);
    let producer_names: Vec<&str> = producer_tools
        .iter()
        .filter_map(|tool| tool.pointer("/function/name").and_then(Value::as_str))
        .collect();
    let reviewer_names: Vec<&str> = reviewer_tools
        .iter()
        .filter_map(|tool| tool.pointer("/function/name").and_then(Value::as_str))
        .collect();
    assert_eq!(reviewer_names, producer_names);
    assert!(reviewer_names.contains(&"run_command"));
    assert!(reviewer_names.contains(&"apply_patch"));
    assert!(reviewer_names.contains(&"submit_final"));
    assert!(!reviewer_names.contains(&"agent_control"));
    assert!(!reviewer_names.contains(&"set_goal"));
}

#[test]
fn supervisor_tool_view_includes_privileged_agent_control_actions() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-supervisor-tools-{}", new_id("t_")));
    let (_kernel, request) = make_kernel_request_for_role(
        &tmp,
        "role_supervisor",
        "Supervise privileged action visibility",
        vec!["tool view is role scoped".to_string()],
    );
    let tools = tool_definitions_for_thread(&request.thread);
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|tool| tool.pointer("/function/name").and_then(Value::as_str))
        .collect();
    assert!(names.contains(&"set_goal"));
    let actions = agent_control_actions(&tools);
    assert!(actions.contains(&"approve_permission".to_string()));
    assert!(actions.contains(&"deny_permission".to_string()));
    assert!(actions.contains(&"kill".to_string()));
    assert!(actions.contains(&"delete_session".to_string()));
    assert!(actions.contains(&"purge_state".to_string()));

    let anthropic_actions = anthropic_tool_definitions_for_thread(&request.thread)
        .into_iter()
        .find(|tool| tool.get("name").and_then(Value::as_str) == Some("agent_control"))
        .and_then(|tool| {
            tool.pointer("/input_schema/properties/action/enum")
                .and_then(Value::as_array)
                .cloned()
        })
        .unwrap();
    assert!(anthropic_actions
        .iter()
        .any(|action| action.as_str() == Some("kill")));
}

#[test]
fn request_tool_view_projects_tool_search_for_deferred_mcp_tools() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-mcp-tools-{}", new_id("t_")));
    let (kernel, mut request) = make_kernel_request(&tmp);
    let mcp_tool = mcp_echo_tool();
    kernel
        .register_tool_descriptor(mcp_tool.tool_descriptor.clone())
        .unwrap();
    refresh_tool_descriptors(&kernel, &mut request);

    let tools = tool_definitions_for_request(&request);
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|tool| tool.pointer("/function/name").and_then(Value::as_str))
        .collect();
    assert!(names.contains(&"tool_search"));
    assert!(!names.contains(&"mcp__echo__echo"));
    let tool_search = tools
        .iter()
        .find(|tool| tool.pointer("/function/name").and_then(Value::as_str) == Some("tool_search"))
        .unwrap();
    assert_eq!(
        tool_search.pointer("/function/parameters/required"),
        Some(&json!(["query"]))
    );
    let deferred_mcp = request
        .context
        .tool_plan
        .entries
        .iter()
        .find(|entry| entry.descriptor.name == "mcp__echo__echo")
        .unwrap();
    assert_eq!(deferred_mcp.exposure, ToolExposure::Deferred);

    let anthropic_names: Vec<String> = anthropic_tool_definitions_for_request(&request)
        .into_iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_string))
        .collect();
    assert!(anthropic_names.contains(&"tool_search".to_string()));
    assert!(!anthropic_names.contains(&"mcp__echo__echo".to_string()));
}

#[test]
fn request_tool_view_projects_core_tools_from_kernel_descriptors() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-core-tools-{}", new_id("t_")));
    let (kernel, mut request) = make_kernel_request(&tmp);
    let mut read_descriptor = kernel
        .state_snapshot()
        .unwrap()
        .tool_descriptors
        .get("read_file")
        .cloned()
        .unwrap();
    read_descriptor.description = "Kernel-owned read descriptor projection.".to_string();
    read_descriptor.model_input_schema = Some(json!({
        "type": "object",
        "required": ["path", "read_mode"],
        "properties": {
            "path": {"type": "string"},
            "read_mode": {"enum": ["exact"]}
        },
        "additionalProperties": false
    }));
    kernel.register_tool_descriptor(read_descriptor).unwrap();
    refresh_tool_descriptors(&kernel, &mut request);

    let tools = tool_definitions_for_request(&request);
    let read_file = tools
        .iter()
        .find(|tool| tool.pointer("/function/name").and_then(Value::as_str) == Some("read_file"))
        .unwrap();
    let openai_description = read_file
        .pointer("/function/description")
        .and_then(Value::as_str)
        .unwrap();
    assert!(openai_description.starts_with("Kernel-owned read descriptor projection."));
    assert!(openai_description.contains("Examples:"));
    assert_eq!(
        read_file.pointer("/function/parameters/required"),
        Some(&json!(["path", "read_mode"]))
    );

    let anthropic_read = anthropic_tool_definitions_for_request(&request)
        .into_iter()
        .find(|tool| tool.get("name").and_then(Value::as_str) == Some("read_file"))
        .unwrap();
    let anthropic_description = anthropic_read
        .get("description")
        .and_then(Value::as_str)
        .unwrap();
    assert!(anthropic_description.starts_with("Kernel-owned read descriptor projection."));
    assert!(anthropic_description.contains("Examples:"));
    assert_eq!(
        anthropic_read.pointer("/input_schema/required"),
        Some(&json!(["path", "read_mode"]))
    );
}

fn agent_control_actions(tools: &[Value]) -> Vec<String> {
    tools
        .iter()
        .find(|tool| {
            tool.get("function")
                .and_then(|function| function.get("name"))
                .and_then(Value::as_str)
                == Some("agent_control")
        })
        .and_then(|tool| {
            tool.pointer("/function/parameters/properties/action/enum")
                .and_then(Value::as_array)
        })
        .into_iter()
        .flatten()
        .filter_map(|action| action.as_str().map(str::to_string))
        .collect()
}

#[test]
fn map_function_call_injects_workspace_root() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-mf-{}", new_id("t_")));
    let request = make_request(&tmp);
    let (tool_name, input, risk) = map_function_call(
        "apply_patch",
        json!({"patch": "*** Begin Patch\n*** Add File: test.rs\n+fn main() {}\n*** End Patch\n"}),
        &request,
    );
    assert_eq!(tool_name, "apply_patch");
    assert_eq!(input["workspace_root"], tmp.to_string_lossy().to_string());
    assert_eq!(risk, 4);
}

#[test]
fn map_function_call_keeps_apply_patch_delete_operation() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-md-{}", new_id("t_")));
    let request = make_request(&tmp);
    let (tool_name, input, risk) = map_function_call(
        "apply_patch",
        json!({"patch": "*** Begin Patch\n*** Delete File: old.txt\n*** End Patch\n"}),
        &request,
    );
    assert_eq!(tool_name, "apply_patch");
    assert_eq!(input["workspace_root"], tmp.to_string_lossy().to_string());
    assert_eq!(risk, 4);
}

#[test]
fn map_function_call_supports_agent_control_actions() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-ac-{}", new_id("t_")));
    let (_kernel, request) = make_kernel_request_for_role(
        &tmp,
        "role_supervisor",
        "inspect agent control actions",
        vec!["agent control is available".to_string()],
    );
    let (tool_name, input, risk) = map_function_call(
        "agent_control",
        json!({
            "action": "start",
            "payload": {
                "goal": "inspect the demo",
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
fn map_function_call_supports_skill_and_mcp_tools() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-ecosystem-map-{}", new_id("t_")));
    let mut request = make_request(&tmp);
    attach_mcp_echo_tool(&mut request);
    let (tool_name, input, risk) =
        map_function_call("load_skill", json!({"name": "review-skill"}), &request);
    assert_eq!(tool_name, "load_skill");
    assert_eq!(input["name"], "review-skill");
    assert_eq!(risk, 1);

    let (tool_name, input, risk) =
        map_function_call("mcp__echo__echo", json!({"text": "hello"}), &request);
    assert_eq!(tool_name, "mcp__echo__echo");
    assert_eq!(input["text"], "hello");
    assert_eq!(risk, 3);
}

#[test]
fn endpoint_parses_only_canonical_values() {
    assert_eq!(
        LlmApiStyle::from_value("openai_chat_completions").unwrap(),
        LlmApiStyle::OpenAiChatCompletions
    );
    assert_eq!(
        LlmApiStyle::from_value("openai_responses").unwrap(),
        LlmApiStyle::OpenAiResponses
    );
    assert_eq!(
        LlmApiStyle::from_value("anthropic_messages").unwrap(),
        LlmApiStyle::AnthropicMessages
    );
    assert!(LlmApiStyle::from_value("openai-compatible").is_err());
    assert!(LlmApiStyle::from_value("openai-chat-completions").is_err());
    assert!(LlmApiStyle::from_value("responses").is_err());
}

#[test]
fn openai_responses_transform_uses_endpoint_specific_wire_shape() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-responses-{}", new_id("t_")));
    let request = make_request(&tmp);
    let provider_request = build_provider_request(
        ProviderRequestConfig {
            endpoint: LlmApiStyle::OpenAiResponses,
            api_base: "https://api.example.test/v1",
            api_key: "test-key",
            model: "gpt-responses",
            max_tokens: 777,
            temperature: Some(0.2),
            model_options: &std::collections::BTreeMap::from([(
                "reasoning".to_string(),
                json!({"effort": "medium"}),
            )]),
            system_prompt_override: &None,
        },
        &request,
    )
    .unwrap();

    assert_eq!(provider_request.provider_label, "openai_responses");
    assert_eq!(provider_request.endpoint_path, "/responses");
    assert_eq!(
        provider_request.url,
        "https://api.example.test/v1/responses"
    );
    assert_eq!(provider_request.body["model"], "gpt-responses");
    assert_eq!(provider_request.body["max_output_tokens"], 777);
    assert!(provider_request.body.get("messages").is_none());
    assert!(provider_request.body["input"].as_array().unwrap().len() >= 2);
    assert_eq!(provider_request.body["tools"][0]["type"], "function");
    assert_eq!(provider_request.body["tools"][0]["strict"], false);
    assert_eq!(
        provider_request.body["reasoning"],
        json!({"effort": "medium"})
    );
}

fn mcp_echo_tool() -> McpToolDefinition {
    let schema = json!({
        "type": "object",
        "required": ["text"],
        "properties": {"text": {"type": "string"}},
        "additionalProperties": false
    });
    McpToolDefinition {
        mcp_tool_id: "mcptool_echo".to_string(),
        server_name: "echo".to_string(),
        tool_name: "echo".to_string(),
        model_tool_name: "mcp__echo__echo".to_string(),
        description: "Echo one text field.".to_string(),
        input_schema: schema.clone(),
        output_schema: json!({"type": "object"}),
        source: EcosystemSource {
            source_kind: EcosystemSourceKind::AgentOs,
            source_scope: EcosystemSourceScope::Config,
            source_path: ".agent-os/config.json".to_string(),
        },
        tool_descriptor: ToolDescriptor {
            tool_id: "tool_mcp__echo__echo".to_string(),
            name: "mcp__echo__echo".to_string(),
            description: "Echo one text field.".to_string(),
            version: "0.3.0".to_string(),
            driver_class: ToolDriverClass::Mcp,
            risk_level: 3,
            input_schema: schema.clone(),
            model_input_schema: Some(schema),
            output_schema: json!({"type": "object"}),
            runtime_input_policy: ToolRuntimeInputPolicy {
                required_resource_scopes: vec!["mcp:echo:echo".to_string()],
                ..ToolRuntimeInputPolicy::default()
            },
            idempotency: IdempotencyMode::ToolNative,
            evidence_type: Some(EvidenceType::ExternalReference),
            created_at: now_rfc3339(),
            ..ToolDescriptor::default()
        },
        created_at: now_rfc3339(),
    }
}

fn attach_mcp_echo_tool(request: &mut ModelTurnRequest) {
    let tool = mcp_echo_tool();
    request
        .context
        .tool_descriptors
        .push(tool.tool_descriptor.clone());
    request.context.tool_plan.entries.push(ToolPlanEntry {
        descriptor: tool.tool_descriptor.clone(),
        exposure: ToolExposure::Direct,
        reason: None,
    });
    request.context.mcp_tools.push(tool);
}
