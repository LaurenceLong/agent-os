use agent_os_sys::AgentOsError;

pub(crate) fn sqlite_error(error: rusqlite::Error) -> AgentOsError {
    AgentOsError::Validation(format!("sqlite store error: {error}"))
}
