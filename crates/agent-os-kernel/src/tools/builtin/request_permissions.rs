use super::{schema, BuiltinTool};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "request_permissions",
        descriptor,
        execute,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_request_permissions",
            name: "request_permissions",
            description:
                "Request a direct-parent permission decision for a bounded turn or session scope.",
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 1,
            input_schema: input_schema(),
            model_input_schema: input_schema(),
            examples: vec![schema::example(
                "Request a bounded permission grant from the parent agent.",
                json!({
                    "reason": "Need to run the workspace test suite.",
                    "scope": "turn",
                    "permissions": {
                        "max_risk_level": 4,
                        "allowed_syscalls": ["tool.invoke"],
                        "resource_scopes": ["tool:run_command"],
                        "allowed_tool_names": ["run_command"],
                        "allowed_tool_driver_classes": ["shell"],
                        "approval_required_above": 4,
                        "requires_evidence_for": ["run_command"]
                    }
                }),
                "Creates a pending permission request for parent review.",
            )],
            output_schema: schema::object(
                &[
                    "tool",
                    "status",
                    "input",
                    "driver_class",
                    "permission_request_id",
                    "request_status",
                    "scope",
                ],
                json!({
                    "tool": {"type": "string"},
                    "status": {"enum": ["pending"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "permission_request_id": {"type": "string"},
                    "request_status": {"type": "string"},
                    "scope": {"type": "string"},
                    "approver_agent_id": {"type": ["string", "null"]},
                    "approver_thread_id": {"type": ["string", "null"]}
                }),
            ),
            runtime_input_policy: ToolRuntimeInputPolicy::default(),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::ApprovalRecord),
        },
    )
}

fn input_schema() -> Value {
    schema::object(
        &["permissions", "reason"],
        json!({
            "permissions": schema::permission_set_schema(),
            "reason": {"type": "string", "maxLength": 8000},
            "scope": {"enum": ["turn", "session"]}
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
    super::super::driver::permission::run_request_permissions(kernel, syscall, descriptor, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_requires_permissions_and_reason() {
        let required = descriptor("now").input_schema["required"]
            .as_array()
            .unwrap()
            .clone();
        assert!(required.iter().any(|value| value == "permissions"));
        assert!(required.iter().any(|value| value == "reason"));
    }

    #[test]
    fn descriptor_emits_approval_record_evidence() {
        assert_eq!(
            descriptor("now").evidence_type,
            Some(EvidenceType::ApprovalRecord)
        );
    }
}
