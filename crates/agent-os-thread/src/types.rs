use agent_os_sys::{ActiveTurn, ThreadStatus};

#[derive(Debug, Clone)]
pub struct AgentOpAck {
    pub op_id: String,
    pub accepted: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TurnStartAck {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone)]
pub struct ThreadStatusSnapshot {
    pub thread_id: String,
    pub status: ThreadStatus,
    pub active_turn: ActiveTurn,
}
