use crate::*;
use agent_os_sys::*;
use serde_json::Value;

/// Record an `AgentControlCommand` for audit and replay.
///
/// Commands are recorded for every action, including rejected actions, so the
/// append-only store preserves the full control history.
pub(super) fn record_agent_control_command(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    requester: &AgentControlBlock,
    target: Option<&AgentControlBlock>,
    action: AgentControlAction,
    payload: Value,
    status: AgentControlCommandStatus,
) -> AgentOsResult<AgentControlCommand> {
    let command = AgentControlCommand {
        command_id: new_id("actl_"),
        action,
        requested_by_agent_id: requester.agent_id.clone(),
        requested_by_thread_id: requester.thread_id.clone(),
        target_agent_id: target.map(|thread| thread.agent_id.clone()),
        target_thread_id: target.map(|thread| thread.thread_id.clone()),
        task_id: syscall.task_id.clone(),
        goal_id: requester.task.goal_id.clone(),
        payload,
        status,
        created_at: now_rfc3339(),
    };
    kernel.emit(
        "AgentControlCommandRecorded",
        "agent_control_command",
        &command.command_id,
        Some(syscall.agent_id.clone()),
        Some(syscall.task_id.clone()),
        Some(syscall.syscall_id.clone()),
        Some(requester.task.goal_id.clone()),
        &command,
    )?;
    Ok(command)
}
