use super::{schema, BuiltinTool, FOREGROUND_TIMEOUT};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "ask_human",
        descriptor,
        execute,
        foreground_timeout: FOREGROUND_TIMEOUT,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_ask_human",
            name: "ask_human",
            description: "Ask a bounded human-facing question or escalation through the communication system.",
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 2,
            input_schema: input_schema(),
            model_input_schema: input_schema(),
            examples: vec![schema::example(
                "Ask the human for a blocking product decision.",
                json!({
                    "question": "Should this benchmark gate run against the OpenAI-compatible provider or Anthropic-compatible provider?",
                    "message_type": "HumanQuestion"
                }),
                "Routes the question through the human communication channel.",
            )],
            output_schema: schema::object(
            &[
                "tool",
                "status",
                "input",
                "driver_class",
                "message_id",
                "delivery_status",
            ],
            json!({
                "tool": {"type": "string"},
                "status": {"enum": ["ok"]},
                "input": {"type": "object"},
                "driver_class": {"type": "string"},
                "message_id": {"type": "string"},
                "delivery_status": {"type": "string"},
                "requires_review": {"type": "boolean"},
                "trigger_turn": {"type": ["boolean", "null"]}
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
        &["question"],
        json!({
            "question": {"type": "string", "maxLength": 8000},
            "message_type": {"enum": ["HumanQuestion", "HumanEscalation", "ApprovalRequest"]},
            "context": {"type": "object"},
            "artifact_refs": {"type": "array", "items": {"type": "string"}},
            "evidence_refs": {"type": "array", "items": {"type": "string"}}
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
    super::super::driver::communication::run_ask_human(kernel, syscall, descriptor, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_requires_question() {
        assert_eq!(
            descriptor("now")
                .input_schema
                .pointer("/required/0")
                .and_then(Value::as_str),
            Some("question")
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
