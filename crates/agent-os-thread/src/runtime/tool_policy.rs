use crate::{RuntimeConfig, ToolExecutionRecord};
use agent_os_sys::{AgentOsError, AgentOsResult, ToolCallStatus};
use serde_json::Value;

pub(super) fn enforce(record: &ToolExecutionRecord, config: &RuntimeConfig) -> AgentOsResult<()> {
    if !config.fail_on_process_nonzero
        || record.tool_name != "run_command"
        || record.status != ToolCallStatus::Completed
    {
        return Ok(());
    }
    let exit_code = record
        .output
        .as_ref()
        .and_then(|output| output.get("exit_code"))
        .and_then(Value::as_i64)
        .ok_or_else(|| {
            AgentOsError::Validation("run_command output omitted exit_code".to_string())
        })?;
    if exit_code != 0 {
        return Err(AgentOsError::Validation(format!(
            "run_command failed with exit code {exit_code}"
        )));
    }
    Ok(())
}
