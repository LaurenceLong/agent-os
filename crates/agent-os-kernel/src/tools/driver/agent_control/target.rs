use crate::*;
use agent_os_sys::*;
use serde_json::Value;

/// Resolve the target `AgentControlBlock` for a non-`start` action.
///
/// The target may be specified by `agent_id` (top-level input or payload) or
/// by `thread_id`. At least one identifier is required.
pub(super) fn resolve_agent_control_target(
    kernel: &Kernel,
    input: &Value,
    payload: &Value,
) -> AgentOsResult<AgentControlBlock> {
    let agent_id = input
        .get("agent_id")
        .or_else(|| payload.get("agent_id"))
        .and_then(Value::as_str);
    let thread_id = input
        .get("thread_id")
        .or_else(|| payload.get("thread_id"))
        .and_then(Value::as_str);
    let state = kernel.read_state()?;
    if let Some(agent_id) = agent_id {
        return state
            .threads
            .values()
            .find(|thread| thread.agent_id == agent_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("agent {agent_id}")));
    }
    if let Some(thread_id) = thread_id {
        return state
            .threads
            .get(thread_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {thread_id}")));
    }
    Err(AgentOsError::Validation(
        "agent_control action requires agent_id or thread_id".to_string(),
    ))
}

/// Resolve workspace roots for a spawned child agent.
///
/// Falls back to the requester's configured roots when the caller does not
/// provide explicit roots or a workdir.
pub(super) fn agent_control_workspace_roots(
    payload: &Value,
    default_roots: &[String],
) -> AgentOsResult<Vec<String>> {
    if let Some(roots) = payload.get("workspace_roots") {
        return roots
            .as_array()
            .ok_or_else(|| {
                AgentOsError::Validation("workspace_roots must be an array".to_string())
            })?
            .iter()
            .map(|root| {
                root.as_str().map(str::to_string).ok_or_else(|| {
                    AgentOsError::Validation("workspace_roots entries must be strings".to_string())
                })
            })
            .collect();
    }
    if let Some(workdir) = payload.get("workdir").and_then(Value::as_str) {
        return Ok(vec![workdir.to_string()]);
    }
    Ok(default_roots.to_vec())
}
