use agent_os_sys::*;
use serde_json::json;

pub(super) fn descriptors(now: &str) -> Vec<ToolDescriptor> {
    vec![
        ToolDescriptor {
            tool_id: "tool_write_file".to_string(),
            name: "write_file".to_string(),
            version: "0.2.0".to_string(),
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
            ..ToolDescriptor::default()
        },
        ToolDescriptor {
            tool_id: "tool_read_file".to_string(),
            name: "read_file".to_string(),
            version: "0.2.0".to_string(),
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
            ..ToolDescriptor::default()
        },
        ToolDescriptor {
            tool_id: "tool_delete_file".to_string(),
            name: "delete_file".to_string(),
            version: "0.2.0".to_string(),
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
            ..ToolDescriptor::default()
        },
        ToolDescriptor {
            tool_id: "tool_replace_text".to_string(),
            name: "replace_text".to_string(),
            version: "0.2.0".to_string(),
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
            ..ToolDescriptor::default()
        },
        ToolDescriptor {
            tool_id: "tool_run_command".to_string(),
            name: "run_command".to_string(),
            version: "0.2.0".to_string(),
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
            ..ToolDescriptor::default()
        },
    ]
}
