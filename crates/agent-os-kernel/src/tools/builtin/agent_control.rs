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
            description: "Supervisor control for child agent lifecycle, status, bounded output, hooks, permission decisions, and privileged state actions. Documented action families are start: start; read-only: status, output, export_trace; mutation: set_hook, send, resume, set_timeout; terminal: stop, kill; cleanup: delete_session, purge_state; permission: approve_permission, deny_permission. Use either agent_id or thread_id when targeting an existing agent. Do not invent agent_id or thread_id values. For action=output, omit payload.tool_call_id unless an exact non-empty background tool call id is provided.",
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 6,
            input_schema: input_schema(),
            model_input_schema: input_schema(),
            examples: vec![
                schema::example(
                    "Start a producer child with a concrete local goal.",
                    json!({
                        "action": "start",
                        "payload": {
                            "goal": "Inspect the failing test and report the smallest fix.",
                            "role_profile_id": "role_producer",
                            "success_criteria": ["Root cause identified with file references"]
                        }
                    }),
                    "Creates a supervised child agent and returns agent_id, thread_id, session_id, and output_handle.",
                ),
                schema::example(
                    "Read-only family: check child status by exact thread_id.",
                    json!({
                        "action": "status",
                        "thread_id": "thread_example",
                        "payload": {}
                    }),
                    "Returns lifecycle status, session id, security level, and active hooks for the child thread.",
                ),
                schema::example(
                    "Read-only family: read bounded child thread output when no exact tool call id is known.",
                    json!({
                        "action": "output",
                        "thread_id": "thread_example",
                        "payload": {"cursor": 0, "limit": 20}
                    }),
                    "Returns recent child-thread output items and cursor metadata.",
                ),
                schema::example(
                    "Read-only family: read new stdout/stderr for a known background tool call.",
                    json!({
                        "action": "output",
                        "thread_id": "thread_example",
                        "payload": {"tool_call_id": "call_example", "new": 200}
                    }),
                    "Returns a bounded output window and cursor metadata for the target tool call.",
                ),
                schema::example(
                    "Read-only family: export a bounded trace preview for a child thread.",
                    json!({
                        "action": "export_trace",
                        "thread_id": "thread_example",
                        "payload": {"limit": 10}
                    }),
                    "Returns trace metadata plus a bounded event preview for the target child.",
                ),
                schema::example(
                    "Mutation family: install a progress hook on a supervised child.",
                    json!({
                        "action": "set_hook",
                        "thread_id": "thread_example",
                        "payload": {
                            "prompt": "Report current progress and blockers in two bullets.",
                            "interval_seconds": 120,
                            "max_response_chars": 300
                        }
                    }),
                    "Records an active progress_report hook for the target child.",
                ),
                schema::example(
                    "Mutation family: send a follow-up instruction to a child session.",
                    json!({
                        "action": "send",
                        "thread_id": "thread_example",
                        "payload": {"message": "Focus on the parser failure before editing."}
                    }),
                    "Records a supervised follow-up command for the target child session.",
                ),
                schema::example(
                    "Mutation family: resume a persisted child session.",
                    json!({
                        "action": "resume",
                        "thread_id": "thread_example",
                        "payload": {}
                    }),
                    "Marks the target child ready to continue against its existing session.",
                ),
                schema::example(
                    "Mutation family: update a child wall-clock timeout.",
                    json!({
                        "action": "set_timeout",
                        "thread_id": "thread_example",
                        "payload": {"timeout_seconds": 600}
                    }),
                    "Persists the updated timeout budget for the target child.",
                ),
                schema::example(
                    "Terminal family: gracefully stop a child while preserving audit state.",
                    json!({
                        "action": "stop",
                        "thread_id": "thread_example",
                        "payload": {"reason": "Supervisor has enough evidence to continue directly."}
                    }),
                    "Transitions the target child to a terminal state through the control plane.",
                ),
                schema::example(
                    "Terminal family: kill a child that cannot be stopped gracefully.",
                    json!({
                        "action": "kill",
                        "thread_id": "thread_example",
                        "payload": {"reason": "The child session is unresponsive."}
                    }),
                    "Terminates the target child through a privileged control-plane action.",
                ),
                schema::example(
                    "Cleanup family: delete a stale provider session from a child.",
                    json!({
                        "action": "delete_session",
                        "thread_id": "thread_example",
                        "payload": {"reason": "The provider session is invalid and must be recreated."}
                    }),
                    "Clears the target child session id while preserving durable audit records.",
                ),
                schema::example(
                    "Cleanup family: purge unusable child runtime state.",
                    json!({
                        "action": "purge_state",
                        "thread_id": "thread_example",
                        "payload": {"reason": "Local state is corrupt and cannot be resumed."}
                    }),
                    "Records a privileged purge tombstone for the target child state.",
                ),
                schema::example(
                    "Permission family: approve a child permission request within supervisor authority.",
                    json!({
                        "action": "approve_permission",
                        "payload": {
                            "permission_request_id": "permreq_example",
                            "decision_reason": "Required to run the requested verification command.",
                            "permissions": {
                                "max_risk_level": 4,
                                "allowed_syscalls": ["tool.invoke"],
                                "resource_scopes": ["workspace:*"],
                                "allowed_tool_names": ["run_command"],
                                "allowed_tool_driver_classes": ["shell"],
                                "approval_required_above": 4,
                                "requires_evidence_for": ["run_command"]
                            }
                        }
                    }),
                    "Records a permission grant bounded by the request and supervisor authority.",
                ),
                schema::example(
                    "Permission family: deny a child permission request with a concise reason.",
                    json!({
                        "action": "deny_permission",
                        "payload": {
                            "permission_request_id": "permreq_example",
                            "decision_reason": "The requested operation exceeds the child task scope."
                        }
                    }),
                    "Records a durable denial for the child permission request.",
                ),
            ],
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
                "description": "Action-specific payload. start requires payload.goal. set_hook requires payload.prompt. send uses payload.message. set_timeout uses payload.timeout_seconds or payload.timeout_ms. output accepts payload.cursor and payload.limit for child-thread output; omit tool_call_id unless you know the exact non-empty background tool call id. approve_permission requires payload.permission_request_id and payload.permissions; deny_permission requires payload.permission_request_id."
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
    use std::collections::BTreeSet;

    #[test]
    fn schema_keeps_existing_actions_and_documents_action_families() {
        let descriptor = descriptor("now");
        let actions = descriptor
            .input_schema
            .pointer("/properties/action/enum")
            .and_then(Value::as_array)
            .unwrap();
        assert!(actions.iter().any(|value| value == "output"));
        assert!(actions.iter().any(|value| value == "deny_permission"));
        assert!(descriptor
            .description
            .contains("read-only: status, output, export_trace"));
        assert!(descriptor
            .description
            .contains("mutation: set_hook, send, resume, set_timeout"));
        assert!(descriptor.description.contains("terminal: stop, kill"));
        assert!(descriptor
            .description
            .contains("cleanup: delete_session, purge_state"));
        assert!(descriptor
            .description
            .contains("permission: approve_permission, deny_permission"));
        let payload_description = descriptor
            .input_schema
            .pointer("/properties/payload/description")
            .and_then(Value::as_str)
            .unwrap();
        assert!(payload_description.contains("tool_call_id"));
        assert!(payload_description.contains("omit tool_call_id"));
        assert!(payload_description.contains("payload.limit"));
        assert!(payload_description.contains("payload.goal"));
        assert!(descriptor.examples.iter().any(|example| {
            example.parameters["action"] == "output"
                && example.parameters["thread_id"] == "thread_example"
                && example.parameters["payload"]["limit"] == 20
        }));
    }

    #[test]
    fn required_model_input_fields_are_represented_by_examples() {
        let descriptor = descriptor("now");
        let required = descriptor
            .model_input_schema
            .as_ref()
            .unwrap()
            .pointer("/required")
            .and_then(Value::as_array)
            .unwrap();
        for field in required.iter().filter_map(Value::as_str) {
            assert!(
                descriptor
                    .examples
                    .iter()
                    .any(|example| example.parameters.get(field).is_some()),
                "required model_input_schema field `{field}` must appear in at least one agent_control example"
            );
        }
    }

    #[test]
    fn action_enum_is_covered_by_examples_or_documented_families() {
        let descriptor = descriptor("now");
        let enum_actions = descriptor
            .input_schema
            .pointer("/properties/action/enum")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().to_string())
            .collect::<BTreeSet<_>>();
        let directly_exampled = descriptor
            .examples
            .iter()
            .filter_map(|example| example.parameters["action"].as_str())
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        let documented_families: &[(&str, &[&str])] = &[
            ("start", &["start"]),
            ("read-only", &["status", "output", "export_trace"]),
            ("mutation", &["set_hook", "send", "resume", "set_timeout"]),
            ("terminal", &["stop", "kill"]),
            ("cleanup", &["delete_session", "purge_state"]),
            ("permission", &["approve_permission", "deny_permission"]),
        ];
        let grouped_actions = documented_families
            .iter()
            .flat_map(|(_, actions)| actions.iter().copied())
            .collect::<BTreeSet<_>>();
        for (family, actions) in documented_families {
            assert!(
                descriptor.description.contains(family),
                "agent_control description must document the `{family}` action family"
            );
            assert!(
                actions
                    .iter()
                    .any(|action| directly_exampled.contains(*action)),
                "agent_control action family `{family}` must have at least one direct example"
            );
        }
        for action in &enum_actions {
            assert!(
                directly_exampled.contains(action) || grouped_actions.contains(action.as_str()),
                "agent_control action `{action}` must be directly example-covered or grouped by a documented family"
            );
        }
        for action in grouped_actions {
            assert!(
                enum_actions.contains(action),
                "documented action family references non-enum action `{action}`"
            );
        }
    }

    #[test]
    fn descriptor_emits_runtime_trace_evidence() {
        assert_eq!(
            descriptor("now").evidence_type,
            Some(EvidenceType::RuntimeTrace)
        );
    }
}
