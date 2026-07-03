use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessLifecycleState {
    Starting,
    Running,
    Exited,
    Failed,
    Interrupted,
    Terminated,
    TimedOut,
    Orphaned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessCommandMode {
    Shell,
    Exec,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessTtyMode {
    #[default]
    None,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessStdinMode {
    #[default]
    Closed,
    Piped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessOutputStreamName {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessOutputStream {
    pub name: ProcessOutputStreamName,
    pub sequence: u64,
    pub bytes: u64,
    pub cursor: u64,
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spool_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessOutputChunk {
    pub chunk_id: String,
    pub process_id: String,
    pub tool_call_id: String,
    pub stream: ProcessOutputStreamName,
    pub sequence: u64,
    pub start_byte: u64,
    pub end_byte: u64,
    pub bytes: u64,
    pub text: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStdinWrite {
    pub write_id: String,
    pub process_id: String,
    pub tool_call_id: String,
    pub sequence: u64,
    pub bytes: u64,
    pub text: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSession {
    pub process_id: String,
    pub tool_call_id: String,
    pub agent_id: String,
    pub thread_id: String,
    pub task_id: String,
    pub session_id: String,
    pub syscall_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_id: Option<String>,
    pub workspace_root: String,
    pub cwd: String,
    pub command_mode: ProcessCommandMode,
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    pub executed_program: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub executed_args: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_keys: Vec<String>,
    pub tty_mode: ProcessTtyMode,
    pub stdin_mode: ProcessStdinMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os_pid: Option<u32>,
    pub state: ProcessLifecycleState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub stdout: ProcessOutputStream,
    pub stderr: ProcessOutputStream,
    pub started_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}
