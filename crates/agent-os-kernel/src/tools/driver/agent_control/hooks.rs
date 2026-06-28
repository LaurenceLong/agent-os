use crate::util::required_string;
use crate::*;
use agent_os_sys::*;
use serde_json::Value;

/// Configure any hooks declared inline on an `agent_control start` payload.
pub(super) fn configure_start_hooks(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    target: &AgentControlBlock,
    payload: &Value,
) -> AgentOsResult<Vec<AgentHook>> {
    let Some(hooks) = payload.get("hooks").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    hooks
        .iter()
        .map(|hook_payload| configure_agent_hook(kernel, syscall, target, hook_payload))
        .collect()
}

/// Configure a single agent hook from a tool payload and persist it through
/// an `AgentHookConfigured` event.
pub(super) fn configure_agent_hook(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    target: &AgentControlBlock,
    payload: &Value,
) -> AgentOsResult<AgentHook> {
    let now = now_rfc3339();
    let hook = AgentHook {
        hook_id: new_id("hook_"),
        agent_id: target.agent_id.clone(),
        thread_id: target.thread_id.clone(),
        hook_type: payload
            .get("hook_type")
            .and_then(Value::as_str)
            .unwrap_or("progress_report")
            .to_string(),
        interval_seconds: payload
            .get("interval_seconds")
            .and_then(Value::as_u64)
            .unwrap_or(120),
        prompt: required_string(payload, "prompt")?,
        response_route: MessageRoute::Supervisor,
        max_response_chars: payload
            .get("max_response_chars")
            .and_then(Value::as_u64)
            .unwrap_or(200),
        stop_when: payload
            .get("stop_when")
            .and_then(Value::as_str)
            .unwrap_or("terminal")
            .to_string(),
        on_missed_reports: payload
            .get("on_missed_reports")
            .and_then(Value::as_str)
            .unwrap_or("report")
            .to_string(),
        status: AgentHookStatus::Active,
        created_at: now.clone(),
        updated_at: now,
    };
    kernel.emit(
        "AgentHookConfigured",
        "agent_hook",
        &hook.hook_id,
        Some(syscall.agent_id.clone()),
        Some(syscall.task_id.clone()),
        Some(syscall.syscall_id.clone()),
        Some(target.task.goal_id.clone()),
        &hook,
    )?;
    Ok(hook)
}

/// Snapshot the hooks currently configured for an agent (used by `status`).
pub(super) fn agent_hooks_for(kernel: &Kernel, agent_id: &str) -> AgentOsResult<Vec<AgentHook>> {
    Ok(kernel
        .read_state()?
        .agent_hooks
        .values()
        .filter(|hook| hook.agent_id == agent_id)
        .cloned()
        .collect())
}
