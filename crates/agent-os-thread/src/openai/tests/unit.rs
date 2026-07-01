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

    assert!(prompt.contains("## Available tools"));
    assert!(prompt.contains("read_file(path)"));
    assert!(prompt.contains("apply_patch(patch)"));
    assert!(prompt.contains("run_command(program, args, env?)"));
    assert!(prompt.contains("set_goal(goal, target_thread_id, target_agent_id)"));
    assert!(prompt.contains("accomplish_goal(summary)"));
    assert!(prompt.contains("update_checklist(items)"));
    assert!(prompt.contains("record_evidence(evidence_type, claim)"));
    assert!(prompt.contains("report_supervisor(message)"));
    assert!(prompt.contains("post_blackboard(channel_id, section, content)"));
    assert!(prompt.contains("ask_human(question)"));
    assert!(prompt.contains("request_permissions(reason, scope, permissions)"));
    assert!(prompt.contains("agent_control(action, agent_id, thread_id, payload)"));
    assert!(prompt.contains("Host OS tools"));
    assert!(prompt.contains("Work State tools"));
    assert!(prompt.contains("Communication tools"));
    assert!(prompt.contains("Session Lifecycle"));
    assert!(prompt.contains("For agent_control, use one action per call"));
    assert!(prompt.contains("Paths are relative to the workspace root"));
    assert!(!prompt.contains("write_file(path, content)"));
    assert!(!prompt.contains("replace_text(path, old, new)"));
    assert!(!prompt.contains("delete_file(path)"));
    assert!(!prompt.contains("workspace.read_file"));
    assert!(!prompt.contains("process.run"));
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
fn build_messages_projects_runtime_feedback_as_user_text() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-feedback-{}", new_id("t_")));
    let request = ModelTurnRequest {
        thread: make_request(&tmp).thread,
        workspace_root: tmp.clone(),
        step_index: 2,
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
fn parse_response_extracts_submit_final() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-sf-{}", new_id("t_")));
    let base_request = make_request(&tmp);
    let request = ModelTurnRequest {
        thread: base_request.thread,
        workspace_root: tmp,
        step_index: 3,
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
    assert_eq!(names.len(), 15);
    assert!(names.contains(&"apply_patch"));
    assert!(names.contains(&"read_file"));
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
    assert!(names.contains(&"agent_control"));
    assert!(names.contains(&"submit_final"));
    assert!(!names.contains(&"write_file"));
    assert!(!names.contains(&"replace_text"));
    assert!(!names.contains(&"delete_file"));
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
fn worker_tool_view_hides_privileged_agent_control_actions() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-worker-tools-{}", new_id("t_")));
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
fn request_tool_view_includes_permitted_dynamic_mcp_tools() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-mcp-tools-{}", new_id("t_")));
    let mut request = make_request(&tmp);
    attach_mcp_echo_tool(&mut request);

    let tools = tool_definitions_for_request(&request);
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|tool| tool.pointer("/function/name").and_then(Value::as_str))
        .collect();
    assert!(names.contains(&"mcp__echo__echo"));
    let mcp_tool = tools
        .iter()
        .find(|tool| {
            tool.pointer("/function/name").and_then(Value::as_str) == Some("mcp__echo__echo")
        })
        .unwrap();
    assert_eq!(
        mcp_tool.pointer("/function/parameters/required"),
        Some(&json!(["text"]))
    );

    let anthropic_names: Vec<String> = anthropic_tool_definitions_for_request(&request)
        .into_iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_string))
        .collect();
    assert!(anthropic_names.contains(&"mcp__echo__echo".to_string()));
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
    let request = make_request(&tmp);
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
fn api_style_parses_explicit_and_base_url_values() {
    assert_eq!(
        LlmApiStyle::from_value("openai-compatible").unwrap(),
        LlmApiStyle::OpenAiCompatible
    );
    assert_eq!(
        LlmApiStyle::from_value("anthropic").unwrap(),
        LlmApiStyle::AnthropicCompatible
    );
    assert_eq!(
        LlmApiStyle::from_base_url("https://provider.example/anthropic"),
        LlmApiStyle::AnthropicCompatible
    );
    assert_eq!(
        LlmApiStyle::from_base_url("https://provider.example/v1"),
        LlmApiStyle::OpenAiCompatible
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
            source_path: "agent-os.json".to_string(),
        },
        tool_descriptor: ToolDescriptor {
            tool_id: "tool_mcp__echo__echo".to_string(),
            name: "mcp__echo__echo".to_string(),
            description: "Echo one text field.".to_string(),
            version: "0.2.0".to_string(),
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
    request.context.mcp_tools.push(tool);
}
