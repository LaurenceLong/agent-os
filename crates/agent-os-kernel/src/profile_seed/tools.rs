use agent_os_sys::*;
use serde_json::json;

pub(super) fn core_tool_descriptors(now: &str) -> Vec<ToolDescriptor> {
    vec![
        ToolDescriptor {
            tool_id: "tool_write_file".to_string(),
            name: "write_file".to_string(),
            version: "0.1.0".to_string(),
            driver_class: ToolDriverClass::Filesystem,
            risk_level: 4,
            input_schema: json!({
                "type": "object",
                "required": ["workspace_root", "path", "content"],
                "properties": {
                    "workspace_root": {"type": "string"},
                    "path": {"type": "string"},
                    "content": {"type": "string"}
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "required": ["tool", "status", "input", "driver_class", "written_path", "bytes_written"],
                "properties": {
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "written_path": {"type": "string"},
                    "bytes_written": {"type": "integer"}
                },
                "additionalProperties": false
            }),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::DiffRef),
            created_at: now.to_string(),
        },
        ToolDescriptor {
            tool_id: "tool_read_file".to_string(),
            name: "read_file".to_string(),
            version: "0.1.0".to_string(),
            driver_class: ToolDriverClass::Filesystem,
            risk_level: 1,
            input_schema: json!({
                "type": "object",
                "required": ["workspace_root", "path"],
                "properties": {
                    "workspace_root": {"type": "string"},
                    "path": {"type": "string"}
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "required": ["tool", "status", "input", "driver_class", "path", "content", "bytes_read"],
                "properties": {
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "path": {"type": "string"},
                    "content": {"type": "string"},
                    "bytes_read": {"type": "integer"}
                },
                "additionalProperties": false
            }),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::SourceRef),
            created_at: now.to_string(),
        },
        ToolDescriptor {
            tool_id: "tool_delete_file".to_string(),
            name: "delete_file".to_string(),
            version: "0.1.0".to_string(),
            driver_class: ToolDriverClass::Filesystem,
            risk_level: 4,
            input_schema: json!({
                "type": "object",
                "required": ["workspace_root", "path"],
                "properties": {
                    "workspace_root": {"type": "string"},
                    "path": {"type": "string"}
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "required": ["tool", "status", "input", "driver_class", "deleted_path", "deleted_bytes"],
                "properties": {
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "deleted_path": {"type": "string"},
                    "deleted_bytes": {"type": "integer"}
                },
                "additionalProperties": false
            }),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::DiffRef),
            created_at: now.to_string(),
        },
        ToolDescriptor {
            tool_id: "tool_replace_text".to_string(),
            name: "replace_text".to_string(),
            version: "0.1.0".to_string(),
            driver_class: ToolDriverClass::Filesystem,
            risk_level: 4,
            input_schema: json!({
                "type": "object",
                "required": ["workspace_root", "path", "old", "new"],
                "properties": {
                    "workspace_root": {"type": "string"},
                    "path": {"type": "string"},
                    "old": {"type": "string"},
                    "new": {"type": "string"}
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "required": ["tool", "status", "input", "driver_class", "changed_path", "replacements", "before", "after"],
                "properties": {
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "changed_path": {"type": "string"},
                    "replacements": {"type": "integer"},
                    "before": {"type": "string"},
                    "after": {"type": "string"}
                },
                "additionalProperties": false
            }),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::DiffRef),
            created_at: now.to_string(),
        },
        ToolDescriptor {
            tool_id: "tool_run_command".to_string(),
            name: "run_command".to_string(),
            version: "0.1.0".to_string(),
            driver_class: ToolDriverClass::Shell,
            risk_level: 4,
            input_schema: json!({
                "type": "object",
                "required": ["program", "args", "cwd"],
                "properties": {
                    "program": {"type": "string"},
                    "args": {"type": "array", "items": {"type": "string"}},
                    "cwd": {"type": "string"}
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "required": ["tool", "status", "input", "driver_class", "exit_code", "stdout", "stderr"],
                "properties": {
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "exit_code": {"type": "integer"},
                    "stdout": {"type": "string"},
                    "stderr": {"type": "string"}
                },
                "additionalProperties": false
            }),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::CommandLog),
            created_at: now.to_string(),
        },
        ToolDescriptor {
            tool_id: "tool_set_objective".to_string(),
            name: "set_objective".to_string(),
            version: "0.1.0".to_string(),
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 2,
            input_schema: json!({
                "type": "object",
                "required": ["objective"],
                "properties": {
                    "objective": {"type": "string"},
                    "title": {"type": "string"},
                    "task_id": {"type": "string"}
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "required": ["tool", "status", "input", "driver_class", "task_id", "objective"],
                "properties": {
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "task_id": {"type": "string"},
                    "objective": {"type": "string"}
                },
                "additionalProperties": false
            }),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: None,
            created_at: now.to_string(),
        },
        ToolDescriptor {
            tool_id: "tool_update_checklist".to_string(),
            name: "update_checklist".to_string(),
            version: "0.1.0".to_string(),
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 2,
            input_schema: json!({
                "type": "object",
                "required": ["items"],
                "properties": {
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
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "required": ["tool", "status", "input", "driver_class", "task_id", "items"],
                "properties": {
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "task_id": {"type": "string"},
                    "items": {"type": "array"}
                },
                "additionalProperties": false
            }),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: None,
            created_at: now.to_string(),
        },
        ToolDescriptor {
            tool_id: "tool_record_evidence".to_string(),
            name: "record_evidence".to_string(),
            version: "0.1.0".to_string(),
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 2,
            input_schema: json!({
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
                    "claim": {"type": "string"},
                    "task_id": {"type": "string"},
                    "artifact_id": {"type": "string"},
                    "blob_ref": {"type": "string"},
                    "content_hash": {"type": "string"},
                    "inline_content": {"type": "string"},
                    "metadata": {"type": "object"}
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "required": ["tool", "status", "input", "driver_class", "evidence_id", "evidence_type"],
                "properties": {
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "evidence_id": {"type": "string"},
                    "evidence_type": {"type": "string"},
                    "claim": {"type": "string"}
                },
                "additionalProperties": false
            }),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: None,
            created_at: now.to_string(),
        },
        ToolDescriptor {
            tool_id: "tool_report_supervisor".to_string(),
            name: "report_supervisor".to_string(),
            version: "0.1.0".to_string(),
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 1,
            input_schema: json!({
                "type": "object",
                "required": ["message"],
                "properties": {
                    "message": {"type": "string"},
                    "message_type": {"enum": ["StatusUpdate", "BlockerReport", "RiskReport", "CompletionReport"]},
                    "artifact_refs": {"type": "array", "items": {"type": "string"}},
                    "evidence_refs": {"type": "array", "items": {"type": "string"}}
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "required": ["tool", "status", "input", "driver_class", "message_id", "delivery_status"],
                "properties": {
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "message_id": {"type": "string"},
                    "delivery_status": {"type": "string"}
                },
                "additionalProperties": true
            }),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: None,
            created_at: now.to_string(),
        },
        ToolDescriptor {
            tool_id: "tool_post_blackboard".to_string(),
            name: "post_blackboard".to_string(),
            version: "0.1.0".to_string(),
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 2,
            input_schema: json!({
                "type": "object",
                "required": ["channel_id", "section", "content"],
                "properties": {
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
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "required": ["tool", "status", "input", "driver_class", "entry_id", "section"],
                "properties": {
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "entry_id": {"type": "string"},
                    "section": {"type": "string"},
                    "message_id": {"type": "string"}
                },
                "additionalProperties": true
            }),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: None,
            created_at: now.to_string(),
        },
        ToolDescriptor {
            tool_id: "tool_ask_human".to_string(),
            name: "ask_human".to_string(),
            version: "0.1.0".to_string(),
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 3,
            input_schema: json!({
                "type": "object",
                "required": ["question"],
                "properties": {
                    "question": {"type": "string"},
                    "message_type": {"enum": ["HumanQuestion", "HumanEscalation", "ApprovalRequest"]},
                    "context": {"type": "object"},
                    "artifact_refs": {"type": "array", "items": {"type": "string"}},
                    "evidence_refs": {"type": "array", "items": {"type": "string"}}
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "required": ["tool", "status", "input", "driver_class", "message_id", "delivery_status"],
                "properties": {
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "message_id": {"type": "string"},
                    "delivery_status": {"type": "string"}
                },
                "additionalProperties": true
            }),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: None,
            created_at: now.to_string(),
        },
        ToolDescriptor {
            tool_id: "tool_submit_final".to_string(),
            name: "submit_final".to_string(),
            version: "0.1.0".to_string(),
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 2,
            input_schema: json!({
                "type": "object",
                "required": ["summary", "evidence_map"],
                "properties": {
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
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "required": [
                    "tool",
                    "status",
                    "input",
                    "driver_class",
                    "task_id",
                    "final_submitted",
                    "summary",
                    "evidence_map_entries"
                ],
                "properties": {
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "task_id": {"type": "string"},
                    "final_submitted": {"type": "boolean"},
                    "summary": {"type": "string"},
                    "evidence_map_entries": {"type": "integer"}
                },
                "additionalProperties": false
            }),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: None,
            created_at: now.to_string(),
        },
        ToolDescriptor {
            tool_id: "tool_agent_control".to_string(),
            name: "agent_control".to_string(),
            version: "0.1.0".to_string(),
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 6,
            input_schema: json!({
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
                    "agent_id": {"type": "string"},
                    "thread_id": {"type": "string"},
                    "idempotency_key": {"type": "string"},
                    "payload": {"type": "object"}
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "required": ["tool", "status", "action", "driver_class"],
                "properties": {
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "action": {"type": "string"},
                    "driver_class": {"type": "string"}
                },
                "additionalProperties": true
            }),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: None,
            created_at: now.to_string(),
        },
    ]
}
