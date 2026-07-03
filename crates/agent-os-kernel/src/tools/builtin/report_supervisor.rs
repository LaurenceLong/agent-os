use super::{schema, BuiltinTool};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "report_supervisor",
        descriptor,
        execute,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_report_supervisor",
            name: "report_supervisor",
            description:
                "Send a concise status, blocker, risk, or completion report to the direct supervisor.",
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 2,
            input_schema: input_schema(),
            model_input_schema: input_schema(),
            examples: vec![schema::example(
                "Send a concise progress update to the supervisor.",
                json!({
                    "message": "Parser fix is implemented; running conformance tests now.",
                    "message_type": "StatusUpdate"
                }),
                "Routes the report and returns message delivery metadata.",
            )],
            output_schema: message_output_schema(),
            runtime_input_policy: ToolRuntimeInputPolicy::default(),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::RuntimeTrace),
        },
    )
}

fn input_schema() -> Value {
    schema::object(
        &["message"],
        json!({
            "message": {"type": "string", "maxLength": 8000},
            "message_type": {"enum": ["StatusUpdate", "BlockerReport", "RiskReport", "CompletionReport"]},
            "artifact_refs": {"type": "array", "items": {"type": "string"}},
            "evidence_refs": {"type": "array", "items": {"type": "string"}}
        }),
    )
}

fn message_output_schema() -> Value {
    schema::object(
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
    )
}

fn execute(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    _tool_call_id: &str,
    input: &Value,
) -> AgentOsResult<Value> {
    super::super::driver::communication::run_report_supervisor(kernel, syscall, descriptor, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_requires_message() {
        assert_eq!(
            descriptor("now")
                .input_schema
                .pointer("/required/0")
                .and_then(Value::as_str),
            Some("message")
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
