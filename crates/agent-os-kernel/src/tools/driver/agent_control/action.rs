use agent_os_sys::*;

/// Enforce the minimum risk level for an `agent_control` action.
///
/// This is the kernel-side guard. It rejects privileged or higher-impact
/// actions when a caller understates risk level, even when a hand-written
/// tool invocation bypasses the model adapter's risk mapping.
pub(super) fn require_agent_control_action_risk(
    action: AgentControlAction,
    risk_level: u8,
) -> AgentOsResult<()> {
    let required = match action {
        AgentControlAction::Kill
        | AgentControlAction::DeleteSession
        | AgentControlAction::PurgeState => 6,
        AgentControlAction::Start
        | AgentControlAction::SetHook
        | AgentControlAction::Send
        | AgentControlAction::Resume
        | AgentControlAction::Stop
        | AgentControlAction::SetTimeout
        | AgentControlAction::ApprovePermission
        | AgentControlAction::DenyPermission => 4,
        AgentControlAction::Status
        | AgentControlAction::Output
        | AgentControlAction::ExportTrace => 1,
    };
    if risk_level < required {
        return Err(AgentOsError::PermissionDenied(format!(
            "agent_control action requires risk level {required}"
        )));
    }
    Ok(())
}

pub(super) fn parse_agent_control_action(value: &str) -> AgentOsResult<AgentControlAction> {
    match value {
        "start" => Ok(AgentControlAction::Start),
        "status" => Ok(AgentControlAction::Status),
        "output" => Ok(AgentControlAction::Output),
        "set_hook" => Ok(AgentControlAction::SetHook),
        "send" => Ok(AgentControlAction::Send),
        "resume" => Ok(AgentControlAction::Resume),
        "stop" => Ok(AgentControlAction::Stop),
        "set_timeout" => Ok(AgentControlAction::SetTimeout),
        "export_trace" => Ok(AgentControlAction::ExportTrace),
        "kill" => Ok(AgentControlAction::Kill),
        "delete_session" => Ok(AgentControlAction::DeleteSession),
        "purge_state" => Ok(AgentControlAction::PurgeState),
        "approve_permission" => Ok(AgentControlAction::ApprovePermission),
        "deny_permission" => Ok(AgentControlAction::DenyPermission),
        _ => Err(AgentOsError::Validation(format!(
            "unknown agent_control action {value}"
        ))),
    }
}
