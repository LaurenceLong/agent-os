use crate::util::{parse_payload, required_string};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(in crate::tools) fn run_request_permissions(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let reason = required_string(input, "reason")?;
    let permissions: PermissionSet = parse_payload(input.get("permissions").ok_or_else(|| {
        AgentOsError::Validation("missing required field permissions".to_string())
    })?)?;
    let scope: PermissionGrantScope =
        parse_payload(input.get("scope").ok_or_else(|| {
            AgentOsError::Validation("missing required field scope".to_string())
        })?)?;
    let request = kernel.request_permissions_with_cause(
        &syscall.agent_id,
        reason,
        permissions,
        scope,
        Some(syscall.syscall_id.clone()),
    )?;
    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "pending",
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "permission_request_id": request.permission_request_id,
        "request_status": request.status,
        "scope": request.scope,
        "approver_agent_id": request.approver_agent_id,
        "approver_thread_id": request.approver_thread_id,
    }))
}
