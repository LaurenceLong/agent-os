use super::{schema, BuiltinTool, FOREGROUND_TIMEOUT};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "post_blackboard",
        descriptor,
        execute,
        foreground_timeout: FOREGROUND_TIMEOUT,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_post_blackboard",
            name: "post_blackboard",
            description: "Post a bounded structured entry to the task, goal, or global blackboard.",
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 2,
            input_schema: input_schema(),
            model_input_schema: input_schema(),
            examples: vec![schema::example(
                "Publish a scoped task decision.",
                json!({
                    "channel_id": "task",
                    "section": "decision",
                    "content": {"decision": "Treat plain apply_patch hunk lines as context."},
                    "confidence": 0.9
                }),
                "Creates a blackboard entry with bounded structured content.",
            )],
            output_schema: schema::object(
                &[
                    "tool",
                    "status",
                    "input",
                    "driver_class",
                    "entry_id",
                    "section",
                    "message_id",
                ],
                json!({
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "entry_id": {"type": "string"},
                    "section": {"type": "string"},
                    "message_id": {"type": "string"},
                    "delivery_status": {"type": "string"}
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
        &["channel_id", "section", "content"],
        json!({
            "channel_id": {"type": "string"},
            "scope": {"enum": ["task", "goal", "global"]},
            "section": {
                "enum": [
                    "known_fact_candidate",
                    "risk",
                    "decision",
                    "test_result",
                    "handoff_note"
                ]
            },
            "content": {"type": "object"},
            "confidence": {"type": "number", "minimum": 0, "maximum": 1},
            "source_evidence_ids": {"type": "array", "items": {"type": "string"}}
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
    super::super::driver::communication::run_post_blackboard(kernel, syscall, descriptor, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_requires_channel_section_content() {
        let required = descriptor("now").input_schema["required"]
            .as_array()
            .unwrap()
            .clone();
        assert!(required.iter().any(|value| value == "channel_id"));
        assert!(required.iter().any(|value| value == "section"));
        assert!(required.iter().any(|value| value == "content"));
    }

    #[test]
    fn descriptor_emits_runtime_trace_evidence() {
        assert_eq!(
            descriptor("now").evidence_type,
            Some(EvidenceType::RuntimeTrace)
        );
    }
}
