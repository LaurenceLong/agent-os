use super::{schema, BuiltinTool, FOREGROUND_TIMEOUT};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "record_evidence",
        descriptor,
        execute,
        foreground_timeout: FOREGROUND_TIMEOUT,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_record_evidence",
            name: "record_evidence",
            description: "Record evidence for a task, claim, artifact, or runtime event. Inline content is bounded by the tool output budget; prefer blob_ref or content_hash for large evidence.",
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 2,
            input_schema: input_schema(),
            model_input_schema: input_schema(),
            examples: vec![schema::example(
                "Record a bounded verification evidence reference.",
                json!({
                    "evidence_type": "command_log",
                    "claim": "agent-os-kernel tests passed",
                    "content_hash": "sha256:example"
                }),
                "Creates an evidence record that can be cited by submit_final.",
            )],
            output_schema: schema::object(
            &["tool", "status", "input", "driver_class", "evidence_id", "evidence_type"],
            json!({
                "tool": {"type": "string"},
                "status": {"enum": ["ok"]},
                "input": {"type": "object"},
                "driver_class": {"type": "string"},
                "evidence_id": {"type": "string"},
                "evidence_type": {"type": "string"},
                "claim": {"type": "string"}
            }),
        ),
            runtime_input_policy: ToolRuntimeInputPolicy::default(),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: None,
        },
    )
}

fn input_schema() -> Value {
    schema::object(
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
            "inline_content": {"type": "string", "maxLength": 8000},
            "metadata": {"type": "object"}
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
    super::super::driver::work_state::run_record_evidence(kernel, syscall, descriptor, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_requires_evidence_type_and_claim() {
        let required = descriptor("now").input_schema["required"]
            .as_array()
            .unwrap()
            .clone();
        assert!(required.iter().any(|value| value == "evidence_type"));
        assert!(required.iter().any(|value| value == "claim"));
    }
}
