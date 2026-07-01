use agent_os_sys::AgentOsResult;
use agent_os_thread::RuntimeRunReport;
use serde::Serialize;
use std::thread::JoinHandle;

#[derive(Debug, Clone, Serialize)]
pub struct DaemonReplaySummary {
    pub tasks: usize,
    pub threads: usize,
    pub artifacts: usize,
    pub evidence: usize,
    pub final_submissions: usize,
}

pub(crate) type RuntimeWorkerJoinHandle = JoinHandle<AgentOsResult<RuntimeRunReport>>;

#[derive(Debug, Clone, Serialize)]
pub struct DaemonShutdownReport {
    pub joined_runtime_workers: usize,
    pub failed_runtime_workers: Vec<String>,
    pub runtime_reports: Vec<RuntimeRunReport>,
}
