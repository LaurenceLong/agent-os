use super::{schema, BuiltinTool, FOREGROUND_TIMEOUT};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "submit_final",
        descriptor,
        execute,
        foreground_timeout: FOREGROUND_TIMEOUT,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_submit_final",
            name: "submit_final",
            description:
                "Submit the structured final answer. This must be the last tool call in a session.",
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 2,
            input_schema: input_schema(),
            model_input_schema: input_schema(),
            examples: vec![schema::example(
                "Submit the final answer with cited evidence.",
                json!({
                    "summary": "Implemented the parser fix and verified relevant tests.",
                    "evidence_map": [
                        {"claim": "Focused tests passed", "evidence_refs": ["evi_tests"]}
                    ],
                    "tests_run": ["cargo test -p agent-os-kernel"]
                }),
                "Records the final submission; no further tool calls are allowed afterward.",
            )],
            output_schema: schema::object(
                &[
                    "tool",
                    "status",
                    "input",
                    "driver_class",
                    "task_id",
                    "final_submitted",
                    "summary",
                    "evidence_map_entries",
                ],
                json!({
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "task_id": {"type": "string"},
                    "final_submitted": {"type": "boolean"},
                    "summary": {"type": "string"},
                    "evidence_map_entries": {"type": "integer"}
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
        &["summary", "evidence_map"],
        json!({
            "summary": {"type": "string"},
            "changed_artifacts": {"type": "array", "items": {"type": "string"}},
            "evidence_map": {
                "type": "array",
                "minItems": 1,
                "description": "Required. Map each important final claim to one or more evidence_ids from completed tool results.",
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
    )
}

fn execute(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    _tool_call_id: &str,
    input: &Value,
) -> AgentOsResult<Value> {
    super::super::driver::session::run_submit_final(kernel, syscall, descriptor, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_requires_summary_and_evidence_map() {
        let required = descriptor("now").input_schema["required"]
            .as_array()
            .unwrap()
            .clone();
        assert!(required.iter().any(|value| value == "summary"));
        assert!(required.iter().any(|value| value == "evidence_map"));
    }
}
