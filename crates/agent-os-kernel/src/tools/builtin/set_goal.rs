use super::{schema, BuiltinTool};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "set_goal",
        descriptor,
        execute,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_set_goal",
            name: "set_goal",
            description: "Supervisor-only goal setting and direct-child retargeting.",
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 2,
            input_schema: schema::object(
                &["goal"],
                json!({
                    "goal": {"type": "string"},
                    "target_thread_id": {"type": "string"},
                    "target_agent_id": {"type": "string"},
                    "title": {"type": "string"},
                    "success_criteria": {"type": "array", "items": {"type": "string"}},
                    "failure_criteria": {"type": "array", "items": {"type": "string"}}
                }),
            ),
            model_input_schema: schema::object(
                &["goal"],
                json!({
                    "goal": {"type": "string"},
                    "target_thread_id": {"type": "string"},
                    "target_agent_id": {"type": "string"},
                    "title": {"type": "string"},
                    "success_criteria": {"type": "array", "items": {"type": "string"}},
                    "failure_criteria": {"type": "array", "items": {"type": "string"}}
                }),
            ),
            examples: vec![schema::example(
                "Set or retarget a clear supervisor goal.",
                json!({
                    "goal": "Implement read_file pagination",
                    "success_criteria": ["read_file supports offset and limit", "focused tests pass"]
                }),
                "Records the goal update and returns the active goal revision.",
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
                    "goal_revision",
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
                    "goal_revision": {"type": "integer"}
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
    super::super::driver::work_state::run_set_goal(kernel, syscall, descriptor, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_requires_goal() {
        let descriptor = descriptor("now");
        assert_eq!(
            descriptor
                .input_schema
                .pointer("/required/0")
                .and_then(Value::as_str),
            Some("goal")
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
