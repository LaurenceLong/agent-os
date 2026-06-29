use super::tool_schemas::{apply_core_model_metadata, permission_set_schema};
use agent_os_sys::*;
use serde_json::json;

mod filesystem;

pub(super) fn core_tool_descriptors(now: &str) -> Vec<ToolDescriptor> {
    let mut descriptors = filesystem::descriptors(now);
    descriptors.extend(vec![
        ToolDescriptor {
            tool_id: "tool_set_goal".to_string(),
            name: "set_goal".to_string(),
            version: "0.2.0".to_string(),
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 2,
            input_schema: json!({
                "type": "object",
                "required": ["goal"],
                "properties": {
                    "goal": {"type": "string"},
                    "target_thread_id": {"type": "string"},
                    "target_agent_id": {"type": "string"},
                    "title": {"type": "string"},
                    "success_criteria": {"type": "array", "items": {"type": "string"}},
                    "failure_criteria": {"type": "array", "items": {"type": "string"}}
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "required": ["tool", "status", "input", "driver_class", "thread_id", "agent_id", "task_id", "goal", "goal_status", "goal_revision"],
                "properties": {
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "thread_id": {"type": "string"},
                    "agent_id": {"type": "string"},
                    "task_id": {"type": "string"},
                    "goal": {"type": "string"},
                    "goal_status": {"type": "string"},
                    "goal_revision": {"type": "integer"}
                },
                "additionalProperties": false
            }),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: None,
            created_at: now.to_string(),
            ..ToolDescriptor::default()
        },
        ToolDescriptor {
            tool_id: "tool_accomplish_goal".to_string(),
            name: "accomplish_goal".to_string(),
            version: "0.2.0".to_string(),
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 2,
            input_schema: json!({
                "type": "object",
                "required": ["summary"],
                "properties": {
                    "summary": {"type": "string"},
                    "evidence_refs": {"type": "array", "items": {"type": "string"}},
                    "artifact_refs": {"type": "array", "items": {"type": "string"}},
                    "known_risks": {"type": "array", "items": {"type": "string"}}
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
                    "thread_id",
                    "agent_id",
                    "task_id",
                    "goal",
                    "goal_status",
                    "goal_accomplished",
                    "summary",
                    "hooks_completed"
                ],
                "properties": {
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "thread_id": {"type": "string"},
                    "agent_id": {"type": "string"},
                    "task_id": {"type": "string"},
                    "goal": {"type": "string"},
                    "goal_status": {"type": "string"},
                    "goal_accomplished": {"type": "boolean"},
                    "summary": {"type": "string"},
                    "hooks_completed": {"type": "integer"}
                },
                "additionalProperties": false
            }),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: None,
            created_at: now.to_string(),
            ..ToolDescriptor::default()
        },
        ToolDescriptor {
            tool_id: "tool_update_checklist".to_string(),
            name: "update_checklist".to_string(),
            version: "0.2.0".to_string(),
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
            ..ToolDescriptor::default()
        },
        ToolDescriptor {
            tool_id: "tool_record_evidence".to_string(),
            name: "record_evidence".to_string(),
            version: "0.2.0".to_string(),
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
            ..ToolDescriptor::default()
        },
        ToolDescriptor {
            tool_id: "tool_report_supervisor".to_string(),
            name: "report_supervisor".to_string(),
            version: "0.2.0".to_string(),
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
            ..ToolDescriptor::default()
        },
        ToolDescriptor {
            tool_id: "tool_post_blackboard".to_string(),
            name: "post_blackboard".to_string(),
            version: "0.2.0".to_string(),
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
            ..ToolDescriptor::default()
        },
        ToolDescriptor {
            tool_id: "tool_ask_human".to_string(),
            name: "ask_human".to_string(),
            version: "0.2.0".to_string(),
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
            ..ToolDescriptor::default()
        },
        ToolDescriptor {
            tool_id: "tool_request_permissions".to_string(),
            name: "request_permissions".to_string(),
            version: "0.2.0".to_string(),
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 1,
            input_schema: json!({
                "type": "object",
                "required": ["reason", "permissions", "scope"],
                "properties": {
                    "reason": {"type": "string"},
                    "scope": {"enum": ["turn", "session"]},
                    "permissions": permission_set_schema()
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "required": ["tool", "status", "input", "driver_class", "permission_request_id", "request_status", "scope"],
                "properties": {
                    "tool": {"type": "string"},
                    "status": {"enum": ["pending"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "permission_request_id": {"type": "string"},
                    "request_status": {"enum": ["Pending", "Approved", "Denied", "Cancelled"]},
                    "scope": {"type": "string"},
                    "approver_agent_id": {"type": ["string", "null"]},
                    "approver_thread_id": {"type": ["string", "null"]}
                },
                "additionalProperties": false
            }),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: None,
            created_at: now.to_string(),
            ..ToolDescriptor::default()
        },
        ToolDescriptor {
            tool_id: "tool_load_skill".to_string(),
            name: "load_skill".to_string(),
            description: "Load the full SKILL.md content for one imported skill by name."
                .to_string(),
            version: "0.2.0".to_string(),
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 1,
            input_schema: json!({
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": {"type": "string"}
                },
                "additionalProperties": false
            }),
            model_input_schema: Some(json!({
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": {"type": "string", "description": "Imported skill name."}
                },
                "additionalProperties": false
            })),
            output_schema: json!({
                "type": "object",
                "required": ["tool", "status", "input", "driver_class", "skill_id", "name", "description", "content"],
                "properties": {
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "skill_id": {"type": "string"},
                    "name": {"type": "string"},
                    "description": {"type": "string"},
                    "content": {"type": "string"},
                    "root_path": {"type": "string"},
                    "skill_file_path": {"type": "string"}
                },
                "additionalProperties": false
            }),
            runtime_input_policy: ToolRuntimeInputPolicy {
                required_resource_scopes: vec!["skill:*".to_string()],
                ..ToolRuntimeInputPolicy::default()
            },
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::SourceRef),
            created_at: now.to_string(),
            ..ToolDescriptor::default()
        },
        ToolDescriptor {
            tool_id: "tool_read_skill_resource".to_string(),
            name: "read_skill_resource".to_string(),
            description: "Read a file under one imported skill root without allowing path escape."
                .to_string(),
            version: "0.2.0".to_string(),
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 1,
            input_schema: json!({
                "type": "object",
                "required": ["name", "path"],
                "properties": {
                    "name": {"type": "string"},
                    "path": {"type": "string"}
                },
                "additionalProperties": false
            }),
            model_input_schema: Some(json!({
                "type": "object",
                "required": ["name", "path"],
                "properties": {
                    "name": {"type": "string", "description": "Imported skill name."},
                    "path": {"type": "string", "description": "Skill-root-relative resource path."}
                },
                "additionalProperties": false
            })),
            output_schema: json!({
                "type": "object",
                "required": ["tool", "status", "input", "driver_class", "skill_id", "name", "path", "content", "bytes_read"],
                "properties": {
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "skill_id": {"type": "string"},
                    "name": {"type": "string"},
                    "path": {"type": "string"},
                    "content": {"type": "string"},
                    "bytes_read": {"type": "integer"}
                },
                "additionalProperties": false
            }),
            runtime_input_policy: ToolRuntimeInputPolicy {
                required_resource_scopes: vec!["skill:*".to_string(), "skill_file:*".to_string()],
                ..ToolRuntimeInputPolicy::default()
            },
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::SourceRef),
            created_at: now.to_string(),
            ..ToolDescriptor::default()
        },
        ToolDescriptor {
            tool_id: "tool_submit_final".to_string(),
            name: "submit_final".to_string(),
            version: "0.2.0".to_string(),
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
            ..ToolDescriptor::default()
        },
        ToolDescriptor {
            tool_id: "tool_agent_control".to_string(),
            name: "agent_control".to_string(),
            version: "0.2.0".to_string(),
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
                            "purge_state",
                            "approve_permission",
                            "deny_permission"
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
            ..ToolDescriptor::default()
        },
    ]);
    apply_core_model_metadata(&mut descriptors);
    descriptors
}
