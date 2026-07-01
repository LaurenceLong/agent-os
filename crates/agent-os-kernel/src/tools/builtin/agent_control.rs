use super::{schema, BuiltinTool, FOREGROUND_TIMEOUT};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "agent_control",
        descriptor,
        execute,
        foreground_timeout: FOREGROUND_TIMEOUT,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_agent_control",
            name: "agent_control",
            description: "Supervisor control for child agent lifecycle, status, bounded output, hooks, permission decisions, and privileged state actions. Use either agent_id or thread_id when targeting an existing agent. Do not invent agent_id or thread_id values. For background tool progress, use action=output with payload.tool_call_id.",
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 6,
            input_schema: input_schema(),
            model_input_schema: input_schema(),
            examples: vec![schema::example(
                "Read new stdout/stderr for a background tool call.",
                json!({
                    "action": "output",
                    "thread_id": "thread_example",
                    "payload": {"tool_call_id": "call_example", "new": 200}
                }),
                "Returns a bounded output window and cursor metadata for the target tool call.",
            )],
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
            runtime_input_policy: ToolRuntimeInputPolicy::default(),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::RuntimeTrace),
        },
    )
}

fn input_schema() -> Value {
    schema::object(
        &["action"],
        json!({
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
            "agent_id": {"type": "string", "description": "Existing target agent_id. Do not invent this from a thread_id; omit agent_id when only thread_id is known."},
            "thread_id": {"type": "string", "description": "Existing target thread_id. Use this by itself when the task provides a thread_id and no exact agent_id. Do not invent an agent_id from this thread_id."},
            "idempotency_key": {"type": "string"},
            "payload": {
                "type": "object",
                "description": "Action-specific payload. For action=output, payload may include cursor, limit, and tool_call_id."
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
    super::super::driver::agent_control::run_agent_control(kernel, syscall, descriptor, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_keeps_existing_actions_and_documents_tool_call_id_payload() {
        let descriptor = descriptor("now");
        let actions = descriptor
            .input_schema
            .pointer("/properties/action/enum")
            .and_then(Value::as_array)
            .unwrap();
        assert!(actions.iter().any(|value| value == "output"));
        assert!(actions.iter().any(|value| value == "deny_permission"));
        let payload_description = descriptor
            .input_schema
            .pointer("/properties/payload/description")
            .and_then(Value::as_str)
            .unwrap();
        assert!(payload_description.contains("tool_call_id"));
    }

    #[test]
    fn descriptor_emits_runtime_trace_evidence() {
        assert_eq!(
            descriptor("now").evidence_type,
            Some(EvidenceType::RuntimeTrace)
        );
    }
}
