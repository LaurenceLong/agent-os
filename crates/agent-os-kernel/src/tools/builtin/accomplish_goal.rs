use super::{schema, BuiltinTool, FOREGROUND_TIMEOUT};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "accomplish_goal",
        descriptor,
        execute,
        foreground_timeout: FOREGROUND_TIMEOUT,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_accomplish_goal",
            name: "accomplish_goal",
            description: "Mark this agent's local goal accomplished before final submission.",
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 2,
            input_schema: schema::object(
                &["summary"],
                json!({
                    "summary": {"type": "string"},
                    "evidence_refs": {"type": "array", "items": {"type": "string"}},
                    "artifact_refs": {"type": "array", "items": {"type": "string"}},
                    "known_risks": {"type": "array", "items": {"type": "string"}}
                }),
            ),
            model_input_schema: schema::object(
                &["summary"],
                json!({
                    "summary": {"type": "string"},
                    "evidence_refs": {"type": "array", "items": {"type": "string"}},
                    "artifact_refs": {"type": "array", "items": {"type": "string"}},
                    "known_risks": {"type": "array", "items": {"type": "string"}}
                }),
            ),
            examples: vec![schema::example(
                "Mark the local goal complete with supporting evidence.",
                json!({
                    "summary": "Implemented pagination and verified focused tests.",
                    "evidence_refs": ["evi_read_file_test"]
                }),
                "Marks the agent goal accomplished and completes active hooks.",
            )],
            output_schema: schema::object(
                &[
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
                    "hooks_completed",
                ],
                json!({
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
                }),
            ),
            runtime_input_policy: ToolRuntimeInputPolicy::default(),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::RuntimeTrace),
        },
    )
}

fn execute(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    _tool_call_id: &str,
    input: &Value,
) -> AgentOsResult<Value> {
    super::super::driver::work_state::run_accomplish_goal(kernel, syscall, descriptor, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_requires_summary() {
        assert_eq!(
            descriptor("now")
                .model_input_schema
                .unwrap()
                .pointer("/required/0")
                .and_then(Value::as_str),
            Some("summary")
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
