use agent_os_sys::{ToolDescriptor, ToolRuntimeInputPolicy};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(super) fn apply_core_model_metadata(descriptors: &mut [ToolDescriptor]) {
    for descriptor in descriptors {
        match descriptor.name.as_str() {
            "read_file" => apply(
                descriptor,
                "Read a workspace file and return its exact contents plus read evidence.",
                schema(
                    &["path"],
                    json!({
                        "path": {"type": "string", "description": "Workspace-relative path to read. Do not use absolute paths or '..'."}
                    }),
                ),
                injected_workspace_root("workspace_root"),
            ),
            "write_file" => apply(
                descriptor,
                "Create or fully replace one workspace file.",
                schema(
                    &["path", "content"],
                    json!({
                        "path": {"type": "string", "description": "Workspace-relative target path."},
                        "content": {"type": "string", "description": "Complete final file content."}
                    }),
                ),
                injected_workspace_root("workspace_root"),
            ),
            "replace_text" => apply(
                descriptor,
                "Replace exactly one occurrence of text in a workspace file.",
                schema(
                    &["path", "old", "new"],
                    json!({
                        "path": {"type": "string"},
                        "old": {"type": "string", "description": "Exact text that must appear once."},
                        "new": {"type": "string", "description": "Replacement text."}
                    }),
                ),
                injected_workspace_root("workspace_root"),
            ),
            "delete_file" => apply(
                descriptor,
                "Delete one workspace file.",
                schema(
                    &["path"],
                    json!({
                        "path": {"type": "string", "description": "Workspace-relative path to delete."}
                    }),
                ),
                injected_workspace_root("workspace_root"),
            ),
            "run_command" => apply(
                descriptor,
                "Run an allowlisted command in the workspace and capture stdout, stderr, and exit code.",
                schema(
                    &["program", "args"],
                    json!({
                        "program": {"type": "string"},
                        "args": {"type": "array", "items": {"type": "string"}}
                    }),
                ),
                injected_workspace_root("cwd"),
            ),
            "set_goal" => apply(
                descriptor,
                "Supervisor-only goal setting and direct-child retargeting.",
                schema(
                    &["goal"],
                    json!({
                        "goal": {"type": "string"},
                        "target_thread_id": {"type": "string"},
                        "target_agent_id": {"type": "string"},
                        "title": {"type": "string"},
                        "success_criteria": {"type": "array", "items": {"type": "string"}},
                        "failure_criteria": {"type": "array", "items": {"type": "string"}}
                    }),
                ),
                ToolRuntimeInputPolicy::default(),
            ),
            "accomplish_goal" => apply(
                descriptor,
                "Mark this agent's local goal accomplished before final submission.",
                schema(
                    &["summary"],
                    json!({
                        "summary": {"type": "string"},
                        "evidence_refs": {"type": "array", "items": {"type": "string"}},
                        "artifact_refs": {"type": "array", "items": {"type": "string"}},
                        "known_risks": {"type": "array", "items": {"type": "string"}}
                    }),
                ),
                ToolRuntimeInputPolicy::default(),
            ),
            "update_checklist" => apply(
                descriptor,
                "Replace the current task checklist with structured item states.",
                schema(
                    &["items"],
                    json!({
                        "task_id": {"type": "string"},
                        "items": {
                            "type": "array",
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
                    }),
                ),
                ToolRuntimeInputPolicy::default(),
            ),
            "record_evidence" => apply(
                descriptor,
                "Record evidence for a task, claim, artifact, or runtime event.",
                schema(
                    &["evidence_type", "claim"],
                    json!({
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
                        "claim": {"type": "string"},
                        "task_id": {"type": "string"},
                        "artifact_id": {"type": "string"},
                        "blob_ref": {"type": "string"},
                        "content_hash": {"type": "string"},
                        "inline_content": {"type": "string"},
                        "metadata": {"type": "object"}
                    }),
                ),
                ToolRuntimeInputPolicy::default(),
            ),
            "report_supervisor" => apply(
                descriptor,
                "Send a concise progress, blocker, risk, or completion report to the supervisor.",
                schema(
                    &["message"],
                    json!({
                        "message": {"type": "string"},
                        "message_type": {"enum": ["StatusUpdate", "BlockerReport", "RiskReport", "CompletionReport"]},
                        "artifact_refs": {"type": "array", "items": {"type": "string"}},
                        "evidence_refs": {"type": "array", "items": {"type": "string"}}
                    }),
                ),
                ToolRuntimeInputPolicy::default(),
            ),
            "post_blackboard" => apply(
                descriptor,
                "Post a scoped blackboard entry through the kernel communication plane.",
                schema(
                    &["channel_id", "section", "content"],
                    json!({
                        "channel_id": {"type": "string"},
                        "scope": {"enum": ["task", "goal", "global"]},
                        "section": {
                            "enum": [
                                "known_fact",
                                "hypothesis",
                                "risk",
                                "open_question",
                                "test_result",
                                "review_result"
                            ]
                        },
                        "content": {"type": "object"},
                        "confidence": {"type": "number"},
                        "source_evidence_ids": {"type": "array", "items": {"type": "string"}}
                    }),
                ),
                ToolRuntimeInputPolicy::default(),
            ),
            "ask_human" => apply(
                descriptor,
                "Ask the human for clarification, escalation, or approval through the kernel route.",
                schema(
                    &["question"],
                    json!({
                        "question": {"type": "string"},
                        "message_type": {"enum": ["HumanQuestion", "HumanEscalation", "ApprovalRequest"]},
                        "context": {"type": "object"},
                        "artifact_refs": {"type": "array", "items": {"type": "string"}},
                        "evidence_refs": {"type": "array", "items": {"type": "string"}}
                    }),
                ),
                ToolRuntimeInputPolicy::default(),
            ),
            "request_permissions" => apply(
                descriptor,
                "Ask the parent agent for a turn- or session-scoped subset permission grant.",
                schema(
                    &["reason", "permissions", "scope"],
                    json!({
                        "reason": {"type": "string"},
                        "scope": {"enum": ["turn", "session"]},
                        "permissions": permission_set_schema()
                    }),
                ),
                ToolRuntimeInputPolicy::default(),
            ),
            "load_skill" => apply(
                descriptor,
                "Load the full SKILL.md content for one imported skill by name.",
                schema(
                    &["name"],
                    json!({
                        "name": {"type": "string", "description": "Imported skill name."}
                    }),
                ),
                required_scopes(&["skill:*"]),
            ),
            "read_skill_resource" => apply(
                descriptor,
                "Read a file under one imported skill root without allowing path escape.",
                schema(
                    &["name", "path"],
                    json!({
                        "name": {"type": "string", "description": "Imported skill name."},
                        "path": {"type": "string", "description": "Skill-root-relative resource path."}
                    }),
                ),
                required_scopes(&["skill:*", "skill_file:*"]),
            ),
            "submit_final" => apply(
                descriptor,
                "Submit the structured final answer. This must be the last tool call in a session.",
                schema(
                    &["summary", "evidence_map"],
                    json!({
                        "summary": {"type": "string"},
                        "changed_artifacts": {"type": "array", "items": {"type": "string"}},
                        "evidence_map": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "required": ["claim", "evidence_refs"],
                                "properties": {
                                    "claim": {"type": "string"},
                                    "evidence_refs": {"type": "array", "items": {"type": "string"}}
                                },
                                "additionalProperties": false
                            }
                        },
                        "unverified_claims": {"type": "array", "items": {"type": "string"}},
                        "known_risks": {"type": "array", "items": {"type": "string"}},
                        "tests_run": {"type": "array", "items": {"type": "string"}},
                        "tests_not_run": {"type": "array", "items": {"type": "string"}},
                        "approvals": {"type": "array", "items": {"type": "string"}}
                    }),
                ),
                ToolRuntimeInputPolicy::default(),
            ),
            "agent_control" => apply(
                descriptor,
                "Supervisor control for child agent lifecycle, status, output, hooks, permission decisions, and privileged state actions.",
                schema(
                    &["action"],
                    json!({
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
                                "purge_state",
                                "approve_permission",
                                "deny_permission"
                            ]
                        },
                        "agent_id": {"type": "string"},
                        "thread_id": {"type": "string"},
                        "idempotency_key": {"type": "string"},
                        "payload": {
                            "type": "object",
                            "description": "Action-specific payload."
                        }
                    }),
                ),
                ToolRuntimeInputPolicy::default(),
            ),
            _ => {}
        }
    }
}

pub(super) fn permission_set_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "max_risk_level",
            "allowed_syscalls",
            "resource_scopes",
            "allowed_tool_names",
            "allowed_tool_driver_classes",
            "approval_required_above",
            "requires_evidence_for"
        ],
        "properties": {
            "max_risk_level": {"type": "integer", "minimum": 0, "maximum": 6},
            "allowed_syscalls": {"type": "array", "items": {"type": "string"}},
            "resource_scopes": {"type": "array", "items": {"type": "string"}},
            "allowed_tool_names": {"type": "array", "items": {"type": "string"}},
            "allowed_tool_driver_classes": {
                "type": "array",
                "items": {
                    "enum": [
                        "kernel_builtin",
                        "filesystem",
                        "shell",
                        "git",
                        "mcp",
                        "browser",
                        "external_api"
                    ]
                }
            },
            "approval_required_above": {"type": "integer", "minimum": 0, "maximum": 6},
            "requires_evidence_for": {"type": "array", "items": {"type": "string"}}
        },
        "additionalProperties": false
    })
}

fn apply(
    descriptor: &mut ToolDescriptor,
    description: &str,
    model_input_schema: Value,
    runtime_input_policy: ToolRuntimeInputPolicy,
) {
    descriptor.description = description.to_string();
    descriptor.model_input_schema = Some(model_input_schema);
    descriptor.runtime_input_policy = runtime_input_policy;
}

fn schema(required: &[&str], properties: Value) -> Value {
    json!({
        "type": "object",
        "required": required,
        "properties": properties,
        "additionalProperties": false
    })
}

fn injected_workspace_root(field_name: &str) -> ToolRuntimeInputPolicy {
    let mut injected_fields = BTreeMap::new();
    injected_fields.insert(field_name.to_string(), "workspace_root".to_string());
    ToolRuntimeInputPolicy {
        injected_fields,
        ..ToolRuntimeInputPolicy::default()
    }
}

fn required_scopes(scopes: &[&str]) -> ToolRuntimeInputPolicy {
    ToolRuntimeInputPolicy {
        required_resource_scopes: scopes.iter().map(|scope| (*scope).to_string()).collect(),
        ..ToolRuntimeInputPolicy::default()
    }
}
