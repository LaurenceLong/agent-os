use super::{schema, BuiltinTool, FOREGROUND_TIMEOUT};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "update_checklist",
        descriptor,
        execute,
        foreground_timeout: FOREGROUND_TIMEOUT,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_update_checklist",
            name: "update_checklist",
            description: "Replace the current task checklist with structured item states.",
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 2,
            input_schema: input_schema(),
            model_input_schema: input_schema(),
            examples: vec![schema::example(
                "Replace the task checklist with current statuses.",
                json!({
                    "items": [
                        {"text": "Inspect current tool schema", "status": "completed"},
                        {"text": "Add examples to descriptors", "status": "in_progress"}
                    ]
                }),
                "Persists the checklist items for the current task.",
            )],
            output_schema: schema::object(
                &[
                    "tool",
                    "status",
                    "input",
                    "driver_class",
                    "task_id",
                    "items",
                ],
                json!({
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "task_id": {"type": "string"},
                    "items": {"type": "array"}
                }),
            ),
            runtime_input_policy: ToolRuntimeInputPolicy::default(),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::RuntimeTrace),
        },
    )
}

fn input_schema() -> Value {
    schema::object(
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
    )
}

fn execute(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    _tool_call_id: &str,
    input: &Value,
) -> AgentOsResult<Value> {
    super::super::driver::work_state::run_update_checklist(kernel, syscall, descriptor, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_requires_items_and_item_text() {
        let descriptor = descriptor("now");
        assert_eq!(
            descriptor
                .input_schema
                .pointer("/required/0")
                .and_then(Value::as_str),
            Some("items")
        );
        assert_eq!(
            descriptor
                .input_schema
                .pointer("/properties/items/items/required/0")
                .and_then(Value::as_str),
            Some("text")
        );
    }

    #[test]
    fn descriptor_emits_runtime_trace_evidence() {
        assert_eq!(
            descriptor("now").evidence_type,
            Some(EvidenceType::RuntimeTrace)
        );
    }
}
