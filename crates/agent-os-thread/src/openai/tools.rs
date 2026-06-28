use serde_json::{json, Value};

use agent_os_sys::AgentControlBlock;

#[cfg(test)]
pub(crate) fn tool_definitions() -> Vec<Value> {
    tool_definitions_with_privileged_actions(true)
}

pub(crate) fn tool_definitions_for_thread(thread: &AgentControlBlock) -> Vec<Value> {
    tool_definitions_with_privileged_actions(thread.role == "SupervisorAgent")
}

fn tool_definitions_with_privileged_actions(include_privileged_agent_control: bool) -> Vec<Value> {
    let mut tools = vec![
        json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a workspace file and return its exact contents plus read evidence. Use before any edit, before citing local code, and when recovering from a failed edit. Paths are workspace-relative.",
                "parameters": {
                    "type": "object",
                    "required": ["path"],
                    "properties": {
                        "path": {"type": "string", "description": "Workspace-relative path to the file to read. Do not use absolute paths or '..'."}
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Create or fully replace one workspace file. Use for new files or intentional full-file rewrites after reading existing content when it exists. Prefer replace_text for small edits.",
                "parameters": {
                    "type": "object",
                    "required": ["path", "content"],
                    "properties": {
                        "path": {"type": "string", "description": "Workspace-relative target path. Parent directories are created when needed."},
                        "content": {"type": "string", "description": "Complete final file content, not a diff or partial patch."}
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "replace_text",
                "description": "Replace exactly one occurrence of text in a workspace file. Use for surgical edits after read_file confirms the surrounding context. Fails if old text is empty or does not match exactly once.",
                "parameters": {
                    "type": "object",
                    "required": ["path", "old", "new"],
                    "properties": {
                        "path": {"type": "string", "description": "Workspace-relative file path to edit."},
                        "old": {"type": "string", "description": "Exact existing text to replace. Include enough context to make it unique."},
                        "new": {"type": "string", "description": "Replacement text to write in place of old."}
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "delete_file",
                "description": "Delete one workspace file and record deletion evidence. Use only when the task requires removal or when cleaning up a file created by this task. Does not delete directories.",
                "parameters": {
                    "type": "object",
                    "required": ["path"],
                    "properties": {
                        "path": {"type": "string", "description": "Workspace-relative file path to delete. Do not use absolute paths or '..'."}
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "run_command",
            "description": "Run a command-line program in the workspace with captured stdout, stderr, and exit code. Use for tests, builds, linters, git inspection, and local smoke checks. Pass the executable and args separately. On Windows, run shell builtins and .cmd/.bat scripts with program cmd and args [\"/C\", \"...\"]; do not use dir or a .cmd file as the program directly.",
                "parameters": {
                    "type": "object",
                    "required": ["program", "args"],
                    "properties": {
                        "program": {"type": "string", "description": "Executable name or path, for example cargo, npm, git, python, or powershell."},
                        "args": {"type": "array", "items": {"type": "string"}, "description": "Command-line arguments as separate strings. Use [] when no arguments are needed."}
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "set_objective",
                "description": "Update the current task objective in Agent-OS durable work state. Use when the goal or working objective needs to be clarified before continuing.",
                "parameters": {
                    "type": "object",
                    "required": ["objective"],
                    "properties": {
                        "objective": {"type": "string", "description": "The precise current objective for this task."},
                        "title": {"type": "string", "description": "Optional short task title."},
                        "task_id": {"type": "string", "description": "Optional current task id. Omit unless explicitly known."}
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "update_checklist",
                "description": "Replace the current task checklist with explicit progress state. Use for compact planning and progress tracking in Agent-OS, not for final reporting.",
                "parameters": {
                    "type": "object",
                    "required": ["items"],
                    "properties": {
                        "task_id": {"type": "string", "description": "Optional current task id. Omit unless explicitly known."},
                        "items": {
                            "type": "array",
                            "description": "Ordered checklist items for the current task.",
                            "items": {
                                "type": "object",
                                "required": ["text"],
                                "properties": {
                                    "text": {"type": "string"},
                                    "status": {"enum": ["pending", "in_progress", "completed", "blocked"]}
                                },
                                "additionalProperties": false
                            }
                        }
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "record_evidence",
                "description": "Record evidence in Agent-OS for the current task. Use for source references, test results, command summaries, external references, screenshots, runtime traces, and other durable proof.",
                "parameters": {
                    "type": "object",
                    "required": ["evidence_type", "claim"],
                    "properties": {
                        "evidence_type": {
                            "enum": [
                                "source_ref",
                                "diff_ref",
                                "command_log",
                                "test_result",
                                "benchmark_result",
                                "review_finding",
                                "approval_record",
                                "runtime_trace",
                                "screenshot",
                                "external_reference"
                            ]
                        },
                        "claim": {"type": "string", "description": "What this evidence supports."},
                        "task_id": {"type": "string", "description": "Optional current task id. Omit unless explicitly known."},
                        "blob_ref": {"type": "string"},
                        "content_hash": {"type": "string"},
                        "inline_content": {"type": "string", "description": "Small inline evidence payload when no blob_ref exists."},
                        "metadata": {"type": "object"}
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "report_supervisor",
                "description": "Send a bounded report to the Supervisor route. Use for progress, blockers, risk, or completion signals that should be visible to the parent Supervisor.",
                "parameters": {
                    "type": "object",
                    "required": ["message"],
                    "properties": {
                        "message": {"type": "string", "description": "Concise report body."},
                        "message_type": {"enum": ["StatusUpdate", "BlockerReport", "RiskReport", "CompletionReport"]},
                        "artifact_refs": {"type": "array", "items": {"type": "string"}},
                        "evidence_refs": {"type": "array", "items": {"type": "string"}}
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "post_blackboard",
                "description": "Publish a typed blackboard entry to shared Agent-OS state. Use for scoped known facts, hypotheses, risks, open questions, test results, and review results.",
                "parameters": {
                    "type": "object",
                    "required": ["channel_id", "section", "content"],
                    "properties": {
                        "channel_id": {"type": "string", "description": "Allowed blackboard channel such as facts, risks, blockers, evidence, test-results, or review-results."},
                        "scope": {"enum": ["task", "goal", "global"], "description": "Defaults to task."},
                        "section": {"enum": ["known_fact", "hypothesis", "risk", "open_question", "test_result", "review_result"]},
                        "content": {"type": "object", "description": "Structured entry content."},
                        "confidence": {"type": "number"},
                        "source_evidence_ids": {"type": "array", "items": {"type": "string"}, "description": "Required by policy for known facts."}
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "ask_human",
                "description": "Ask for human input through the Agent-OS Human route. This records delivery of the question; do not repeat it or wait for an answer unless the task explicitly requires a blocking human reply. Worker roles may be denied by policy.",
                "parameters": {
                    "type": "object",
                    "required": ["question"],
                    "properties": {
                        "question": {"type": "string", "description": "Specific question for the human."},
                        "message_type": {"enum": ["HumanQuestion", "HumanEscalation", "ApprovalRequest"]},
                        "context": {"type": "object"},
                        "artifact_refs": {"type": "array", "items": {"type": "string"}},
                        "evidence_refs": {"type": "array", "items": {"type": "string"}}
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "agent_control",
                "description": "Supervise child agents through one CLI-like control tool. Use for delegation, progress hooks, status, output reads, follow-up instructions, resume, graceful stop, and privileged lifecycle actions. The runtime records invocation edges and child session state.",
                "parameters": {
                    "type": "object",
                    "required": ["action"],
                    "properties": {
                        "action": {
                            "enum": [
                                "start",
                                "status",
                                "output",
                                "set_hook",
                                "send",
                                "resume",
                                "stop",
                                "set_timeout",
                                "export_trace",
                                "kill",
                                "delete_session",
                                "purge_state"
                            ]
                        },
                        "agent_id": {"type": "string", "description": "Target agent id for non-start actions. Use either agent_id or thread_id."},
                        "thread_id": {"type": "string", "description": "Target thread id for non-start actions. Use either thread_id or agent_id."},
                        "idempotency_key": {"type": "string", "description": "Optional caller-provided idempotency key for retry-safe control actions."},
                        "payload": {"type": "object", "description": "Action-specific payload. For start, include assignment and optional role_profile_id, workdir, workspace_roots, timeout_seconds, output_policy, success_criteria, and hooks. For set_hook, include interval_seconds and prompt."}
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "submit_final",
                "description": "Submit the final task result to Agent-OS. Call immediately once requested work and verification are complete, or when a blocker is evidence-backed. Do not repeat successful tool calls before this. The runtime attaches changed artifacts and tool evidence from prior calls.",
                "parameters": {
                    "type": "object",
                    "required": ["summary"],
                    "properties": {
                        "summary": {"type": "string", "description": "Brief factual summary of what was accomplished or why execution is blocked."},
                        "tests_run": {"type": "array", "items": {"type": "string"}, "description": "Verification commands that were actually run."},
                        "known_risks": {"type": "array", "items": {"type": "string"}, "description": "Known limitations, skipped verification, or residual risks."}
                    },
                    "additionalProperties": false
                }
            }
        }),
    ];
    redact_privileged_agent_control_actions(&mut tools, include_privileged_agent_control);
    tools
}

#[cfg(test)]
pub(crate) fn anthropic_tool_definitions() -> Vec<Value> {
    openai_tools_to_anthropic(tool_definitions())
}

pub(crate) fn anthropic_tool_definitions_for_thread(thread: &AgentControlBlock) -> Vec<Value> {
    openai_tools_to_anthropic(tool_definitions_for_thread(thread))
}

fn openai_tools_to_anthropic(tools: Vec<Value>) -> Vec<Value> {
    tools
        .into_iter()
        .filter_map(|tool| {
            let function = tool.get("function")?;
            Some(json!({
                "name": function.get("name")?.clone(),
                "description": function.get("description")?.clone(),
                "input_schema": function.get("parameters")?.clone()
            }))
        })
        .collect()
}

fn redact_privileged_agent_control_actions(tools: &mut [Value], include: bool) {
    if include {
        return;
    }
    for tool in tools {
        let name = tool
            .get("function")
            .and_then(|function| function.get("name"))
            .and_then(Value::as_str);
        if name != Some("agent_control") {
            continue;
        }
        if let Some(actions) = tool
            .pointer_mut("/function/parameters/properties/action/enum")
            .and_then(Value::as_array_mut)
        {
            actions.retain(|action| {
                action.as_str().is_none_or(|action| {
                    !matches!(action, "kill" | "delete_session" | "purge_state")
                })
            });
        }
    }
}
