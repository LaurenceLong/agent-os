use super::support::*;
use super::*;

#[test]
fn mock_tool_call_strings_run_local_tools_and_build_llm_tool_results() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-local-tool-{}", new_id("t_")));
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("input.txt"), "hello from local tool\n").unwrap();
    let (kernel, request) = make_kernel_request(&tmp);
    let agent = request.thread.clone();
    let task_id = agent.task.task_id.clone();
    let env = kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            tmp.to_string_lossy(),
            "sbox_workspace_write",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    kernel
        .attach_environment(
            &env.environment_id,
            &agent.agent_id,
            &agent.thread_id,
            &task_id,
            AttachMode::WorkspaceWrite,
        )
        .unwrap();
    let cap = kernel
        .grant_capability(
            &agent.agent_id,
            &task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap();
    let mock_tool_call_response = r#"{
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [
                        {
                            "id": "call_mock_read",
                            "type": "function",
                            "function": {
                                "name": "read_file",
                                "arguments": "{\"path\":\"input.txt\"}"
                            }
                        },
                        {
                            "id": "call_mock_write",
                            "type": "function",
                            "function": {
                                "name": "write_file",
                                "arguments": "{\"path\":\"output.txt\",\"content\":\"written by mock call\\n\"}"
                            }
                        }
                    ]
                }
            }],
            "usage": {"prompt_tokens": 12, "completion_tokens": 8}
        }"#;
    let body: Value = serde_json::from_str(mock_tool_call_response).unwrap();
    let response = parse_response(&body, &request).unwrap();
    assert_eq!(response.actions.len(), 2);
    let parsed_actions = response.actions.clone();

    let mut records = Vec::new();
    for action in response.actions {
        let ModelAction::ToolCall(action) = action else {
            panic!("expected ToolCall");
        };
        let invocation = kernel
            .invoke_tool(
                &agent.agent_id,
                &task_id,
                &agent.session_id,
                cap.capability_id.clone(),
                action.risk_level,
                ToolInvokeInput {
                    tool_name: action.tool_name,
                    input: action.input,
                    evidence_claim: action.evidence_claim.clone(),
                },
            )
            .unwrap();
        records.push(ToolExecutionRecord {
            call_id: invocation.call_id,
            tool_name: invocation.tool_name,
            status: invocation.status,
            input: Some(invocation.input),
            output: invocation.output,
            evidence_ids: invocation.evidence_ids,
            evidence_claim: action.evidence_claim,
        });
    }
    assert_eq!(
        std::fs::read_to_string(tmp.join("output.txt")).unwrap(),
        "written by mock call\n"
    );

    let next_request = ModelTurnRequest {
        thread: agent,
        workspace_root: tmp.clone(),
        step_index: 1,
        context: ModelContextProjection {
            tool_results: records,
            ..ModelContextProjection::default()
        },
    };
    let messages = build_messages(&next_request, tmp.to_str().unwrap(), &None);
    assert_eq!(messages.len(), 6);
    let audit_log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit/mock-tool-call-audit.jsonl");
    std::fs::create_dir_all(audit_log_path.parent().unwrap()).unwrap();
    let state = kernel.state_snapshot().unwrap();
    let mut tool_invocations: Vec<_> = state.tool_invocations.values().cloned().collect();
    tool_invocations.sort_by(|left, right| left.call_id.cmp(&right.call_id));
    let mut audit_events: Vec<_> = state.audit_events.values().cloned().collect();
    audit_events.sort_by(|left, right| left.audit_id.cmp(&right.audit_id));
    let entries = [
        json!({
            "type": "system_prompt",
            "content": default_system_prompt(&request, tmp.to_str().unwrap())
        }),
        json!({
            "type": "mock_llm_tool_call_response",
            "body": body
        }),
        json!({
            "type": "parsed_model_actions",
            "actions": parsed_actions
        }),
        json!({
            "type": "tool_execution_records",
            "records": next_request.context.tool_results.clone()
        }),
        json!({
            "type": "llm_messages_after_tools",
            "messages": messages.clone()
        }),
        json!({
            "type": "tool_invocations",
            "tool_invocations": tool_invocations
        }),
        json!({
            "type": "audit_events",
            "audit_events": audit_events
        }),
        json!({
            "type": "kernel_events",
            "events": kernel.events().unwrap()
        }),
    ];
    let mut audit_log = std::fs::File::create(&audit_log_path).unwrap();
    for entry in entries {
        use std::io::Write;
        writeln!(audit_log, "{}", serde_json::to_string(&entry).unwrap()).unwrap();
    }
    println!("audit_log={}", audit_log_path.display());

    assert_eq!(
        messages[2]["tool_calls"][0]["function"]["name"],
        "read_file"
    );
    let read_args: Value = serde_json::from_str(
        messages[2]["tool_calls"][0]["function"]["arguments"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(read_args["path"], "input.txt");
    assert!(read_args.get("workspace_root").is_none());
    assert_eq!(messages[3]["role"], "tool");
    let read_result: Value =
        serde_json::from_str(messages[3]["content"].as_str().unwrap()).unwrap();
    assert_eq!(read_result["content"], "hello from local tool\n");
    assert!(read_result.get("input").is_none());

    assert_eq!(
        messages[4]["tool_calls"][0]["function"]["name"],
        "write_file"
    );
    let write_args: Value = serde_json::from_str(
        messages[4]["tool_calls"][0]["function"]["arguments"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(write_args["path"], "output.txt");
    assert_eq!(write_args["content"], "written by mock call\n");
    assert!(write_args.get("workspace_root").is_none());
    assert_eq!(messages[5]["role"], "tool");
    let write_result: Value =
        serde_json::from_str(messages[5]["content"].as_str().unwrap()).unwrap();
    assert_eq!(write_result["bytes_written"], 21);
    assert!(write_result["written_path"]
        .as_str()
        .unwrap()
        .ends_with("output.txt"));

    let _ = std::fs::remove_dir_all(tmp);
}

#[test]
fn openai_compatible_mock_adapter_runs_every_core_tool() {
    let tmp = std::env::temp_dir().join(format!("aos-openai-all-tools-{}", new_id("t_")));
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("read.txt"), "read me\n").unwrap();
    std::fs::write(tmp.join("edit.txt"), "alpha old beta\n").unwrap();
    std::fs::write(tmp.join("delete.txt"), "remove me\n").unwrap();
    let (kernel, request) = make_kernel_request_for_role(
        &tmp,
        "role_supervisor",
        "Supervise provider-neutral mock adapter coverage",
        vec!["all core tools return structured results".to_string()],
    );
    let capability = attach_workspace_and_grant(&kernel, &request, 4);
    let current_exe = std::env::current_exe().unwrap();
    let initial_messages = build_messages(&request, tmp.to_str().unwrap(), &None);
    let openai_tools = tool_definitions();

    let tool_calls = vec![
        ("call_read", "read_file", json!({"path": "read.txt"})),
        (
            "call_write",
            "write_file",
            json!({"path": "created.txt", "content": "created by provider mock\n"}),
        ),
        (
            "call_replace",
            "replace_text",
            json!({"path": "edit.txt", "old": "old", "new": "new"}),
        ),
        ("call_delete", "delete_file", json!({"path": "delete.txt"})),
        (
            "call_run",
            "run_command",
            json!({"program": current_exe.to_string_lossy(), "args": ["--help"]}),
        ),
        (
            "call_goal",
            "set_goal",
            json!({"goal": "complete provider-neutral all-tool mock adapter coverage"}),
        ),
        (
            "call_accomplish_goal",
            "accomplish_goal",
            json!({"summary": "provider-neutral mock adapter local goal complete"}),
        ),
        (
            "call_checklist",
            "update_checklist",
            json!({"items": [
                {"text": "exercise every model-visible tool", "status": "completed"}
            ]}),
        ),
        (
            "call_evidence",
            "record_evidence",
            json!({
                "evidence_type": "external_reference",
                "claim": "provider mock executed control-plane tools",
                "blob_ref": "blob://mock-evidence",
                "content_hash": "mock-hash"
            }),
        ),
        (
            "call_report",
            "report_supervisor",
            json!({"message": "provider mock all-tool coverage is progressing"}),
        ),
        (
            "call_blackboard",
            "post_blackboard",
            json!({
                "channel_id": "risks",
                "scope": "goal",
                "section": "risk",
                "content": {"risk": "mock adapter risk entry"}
            }),
        ),
        (
            "call_human",
            "ask_human",
            json!({"question": "Confirm mock adapter human route wiring?"}),
        ),
        (
            "call_agent",
            "agent_control",
            json!({
                "action": "start",
                "payload": {
                    "goal": "inspect provider-neutral mock adapter coverage",
                    "success_criteria": ["report status"]
                }
            }),
        ),
    ];
    let body = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": tool_calls
                    .into_iter()
                    .map(|(id, name, arguments)| json!({
                        "id": id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": serde_json::to_string(&arguments).unwrap()
                        }
                    }))
                    .collect::<Vec<_>>()
            }
        }],
        "usage": {"prompt_tokens": 42, "completion_tokens": 24}
    });
    let response = parse_response(&body, &request).unwrap();
    let parsed_actions = response.actions.clone();
    let records = execute_tool_actions(&kernel, &request, &capability, response.actions);
    assert_core_tool_mock_effects(&tmp, &records);

    let next_request = ModelTurnRequest {
        thread: request.thread,
        workspace_root: tmp.clone(),
        step_index: 1,
        context: ModelContextProjection {
            tool_results: records,
            ..ModelContextProjection::default()
        },
    };
    let messages = build_messages(&next_request, tmp.to_str().unwrap(), &None);
    write_mock_interaction_log(
        "openai-compatible-mock-adapter-interaction.jsonl",
        &[
            json!({
                "type": "provider_request",
                "provider": "openai-compatible",
                "endpoint": "/chat/completions",
                "body": {
                    "model": "mock-model",
                    "messages": initial_messages,
                    "tools": openai_tools,
                    "tool_choice": "auto"
                }
            }),
            json!({
                "type": "mock_llm_response",
                "provider": "openai-compatible",
                "body": body
            }),
            json!({
                "type": "parsed_model_actions",
                "provider": "openai-compatible",
                "actions": parsed_actions
            }),
            json!({
                "type": "tool_execution_records",
                "provider": "openai-compatible",
                "records": next_request.context.tool_results.clone()
            }),
            json!({
                "type": "provider_followup_request",
                "provider": "openai-compatible",
                "endpoint": "/chat/completions",
                "body": {
                    "model": "mock-model",
                    "messages": messages,
                    "tools": tool_definitions(),
                    "tool_choice": "auto"
                }
            }),
        ],
    );
    assert_eq!(messages.iter().filter(|m| m["role"] == "tool").count(), 13);
    let first_args: Value = serde_json::from_str(
        messages[2]["tool_calls"][0]["function"]["arguments"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(first_args["path"], "read.txt");
    assert!(first_args.get("workspace_root").is_none());
    let _ = std::fs::remove_dir_all(tmp);
}

#[test]
fn anthropic_compatible_mock_adapter_runs_every_core_tool() {
    let tmp = std::env::temp_dir().join(format!("aos-anthropic-all-tools-{}", new_id("t_")));
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("read.txt"), "read me\n").unwrap();
    std::fs::write(tmp.join("edit.txt"), "alpha old beta\n").unwrap();
    std::fs::write(tmp.join("delete.txt"), "remove me\n").unwrap();
    let (kernel, request) = make_kernel_request_for_role(
        &tmp,
        "role_supervisor",
        "Supervise provider-neutral mock adapter coverage",
        vec!["all core tools return structured results".to_string()],
    );
    let capability = attach_workspace_and_grant(&kernel, &request, 4);
    let current_exe = std::env::current_exe().unwrap();
    let anthropic_system = default_system_prompt(&request, tmp.to_str().unwrap());
    let initial_messages = build_anthropic_messages(&request, tmp.to_str().unwrap());
    let anthropic_tools = anthropic_tool_definitions();

    let body = json!({
        "id": "msg_mock",
        "type": "message",
        "role": "assistant",
        "content": [
            {"type": "tool_use", "id": "toolu_read", "name": "read_file", "input": {"path": "read.txt"}},
            {"type": "tool_use", "id": "toolu_write", "name": "write_file", "input": {"path": "created.txt", "content": "created by provider mock\n"}},
            {"type": "tool_use", "id": "toolu_replace", "name": "replace_text", "input": {"path": "edit.txt", "old": "old", "new": "new"}},
            {"type": "tool_use", "id": "toolu_delete", "name": "delete_file", "input": {"path": "delete.txt"}},
            {"type": "tool_use", "id": "toolu_run", "name": "run_command", "input": {"program": current_exe.to_string_lossy(), "args": ["--help"]}},
            {"type": "tool_use", "id": "toolu_goal", "name": "set_goal", "input": {"goal": "complete provider-neutral all-tool mock adapter coverage"}},
            {"type": "tool_use", "id": "toolu_accomplish_goal", "name": "accomplish_goal", "input": {"summary": "provider-neutral mock adapter local goal complete"}},
            {"type": "tool_use", "id": "toolu_checklist", "name": "update_checklist", "input": {"items": [
                {"text": "exercise every model-visible tool", "status": "completed"}
            ]}},
            {"type": "tool_use", "id": "toolu_evidence", "name": "record_evidence", "input": {
                "evidence_type": "external_reference",
                "claim": "provider mock executed control-plane tools",
                "blob_ref": "blob://mock-evidence",
                "content_hash": "mock-hash"
            }},
            {"type": "tool_use", "id": "toolu_report", "name": "report_supervisor", "input": {"message": "provider mock all-tool coverage is progressing"}},
            {"type": "tool_use", "id": "toolu_blackboard", "name": "post_blackboard", "input": {
                "channel_id": "risks",
                "scope": "goal",
                "section": "risk",
                "content": {"risk": "mock adapter risk entry"}
            }},
            {"type": "tool_use", "id": "toolu_human", "name": "ask_human", "input": {"question": "Confirm mock adapter human route wiring?"}},
            {"type": "tool_use", "id": "toolu_agent", "name": "agent_control", "input": {
                "action": "start",
                "payload": {
                    "goal": "inspect provider-neutral mock adapter coverage",
                    "success_criteria": ["report status"]
                }
            }}
        ],
        "usage": {"input_tokens": 40, "output_tokens": 22}
    });
    let response = parse_anthropic_response(&body, &request).unwrap();
    assert_eq!(response.usage.input_tokens, 40);
    let parsed_actions = response.actions.clone();
    let records = execute_tool_actions(&kernel, &request, &capability, response.actions);
    assert_core_tool_mock_effects(&tmp, &records);

    let next_request = ModelTurnRequest {
        thread: request.thread,
        workspace_root: tmp.clone(),
        step_index: 1,
        context: ModelContextProjection {
            tool_results: records,
            ..ModelContextProjection::default()
        },
    };
    let messages = build_anthropic_messages(&next_request, tmp.to_str().unwrap());
    write_mock_interaction_log(
        "anthropic-compatible-mock-adapter-interaction.jsonl",
        &[
            json!({
                "type": "provider_request",
                "provider": "anthropic-compatible",
                "endpoint": "/v1/messages",
                "body": {
                    "model": "mock-model",
                    "system": anthropic_system,
                    "messages": initial_messages,
                    "tools": anthropic_tools,
                    "tool_choice": {"type": "auto"},
                    "max_tokens": 4096,
                    "temperature": 0.0
                }
            }),
            json!({
                "type": "mock_llm_response",
                "provider": "anthropic-compatible",
                "body": body
            }),
            json!({
                "type": "parsed_model_actions",
                "provider": "anthropic-compatible",
                "actions": parsed_actions
            }),
            json!({
                "type": "tool_execution_records",
                "provider": "anthropic-compatible",
                "records": next_request.context.tool_results.clone()
            }),
            json!({
                "type": "provider_followup_request",
                "provider": "anthropic-compatible",
                "endpoint": "/v1/messages",
                "body": {
                    "model": "mock-model",
                    "system": default_system_prompt(&next_request, tmp.to_str().unwrap()),
                    "messages": messages,
                    "tools": anthropic_tool_definitions(),
                    "tool_choice": {"type": "auto"},
                    "max_tokens": 4096,
                    "temperature": 0.0
                }
            }),
        ],
    );
    assert_eq!(
        messages
            .iter()
            .filter(|m| {
                m.get("content")
                    .and_then(Value::as_array)
                    .is_some_and(|content| content.iter().any(|part| part["type"] == "tool_result"))
            })
            .count(),
        13
    );
    assert_eq!(messages[1]["content"][0]["type"], "tool_use");
    assert_eq!(messages[2]["content"][0]["type"], "tool_result");
    assert_eq!(messages[1]["content"][0]["input"]["path"], "read.txt");
    assert!(messages[1]["content"][0]["input"]
        .get("workspace_root")
        .is_none());
    let _ = std::fs::remove_dir_all(tmp);
}
