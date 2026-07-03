use crate::state::Kernel;
use agent_os_sys::*;

pub(crate) struct StartProcessSessionInput {
    pub tool_call_id: String,
    pub agent_id: String,
    pub thread_id: String,
    pub task_id: String,
    pub session_id: String,
    pub syscall_id: String,
    pub capability_id: Option<String>,
    pub workspace_root: String,
    pub cwd: String,
    pub command_mode: ProcessCommandMode,
    pub command: String,
    pub args: Vec<String>,
    pub executed_program: String,
    pub executed_args: Vec<String>,
    pub environment_keys: Vec<String>,
}

impl Kernel {
    pub(crate) fn start_process_session(
        &self,
        input: StartProcessSessionInput,
    ) -> AgentOsResult<ProcessSession> {
        let now = now_rfc3339();
        let session = ProcessSession {
            process_id: new_id("proc_"),
            tool_call_id: input.tool_call_id,
            agent_id: input.agent_id,
            thread_id: input.thread_id,
            task_id: input.task_id,
            session_id: input.session_id,
            syscall_id: input.syscall_id,
            capability_id: input.capability_id,
            workspace_root: input.workspace_root,
            cwd: input.cwd,
            command_mode: input.command_mode,
            command: input.command,
            args: input.args,
            executed_program: input.executed_program,
            executed_args: input.executed_args,
            environment_keys: input.environment_keys,
            tty_mode: ProcessTtyMode::None,
            stdin_mode: ProcessStdinMode::Closed,
            os_pid: None,
            state: ProcessLifecycleState::Starting,
            exit_code: None,
            error: None,
            stdout: empty_process_stream(ProcessOutputStreamName::Stdout),
            stderr: empty_process_stream(ProcessOutputStreamName::Stderr),
            started_at: now.clone(),
            updated_at: now,
            completed_at: None,
        };
        self.emit(
            "ProcessSessionStarted",
            "process_session",
            &session.process_id,
            Some(session.agent_id.clone()),
            Some(session.task_id.clone()),
            Some(session.tool_call_id.clone()),
            None,
            &session,
        )?;
        Ok(session)
    }

    pub(crate) fn mark_process_session_running(
        &self,
        process_id: &str,
        os_pid: Option<u32>,
        stdout_spool_path: Option<String>,
        stderr_spool_path: Option<String>,
    ) -> AgentOsResult<ProcessSession> {
        let mut session = self.process_session(process_id)?;
        session.state = ProcessLifecycleState::Running;
        session.os_pid = os_pid;
        session.stdout.spool_path = stdout_spool_path;
        session.stderr.spool_path = stderr_spool_path;
        session.updated_at = now_rfc3339();
        self.emit_process_session("ProcessSessionRunning", &session)?;
        Ok(session)
    }

    pub(crate) fn exit_process_session(
        &self,
        process_id: &str,
        exit_code: Option<i32>,
        mut stdout: ProcessOutputStream,
        mut stderr: ProcessOutputStream,
    ) -> AgentOsResult<ProcessSession> {
        let mut session = self.process_session(process_id)?;
        let now = now_rfc3339();
        stdout.sequence = session.stdout.sequence;
        stdout.cursor = stdout.bytes;
        stderr.sequence = session.stderr.sequence;
        stderr.cursor = stderr.bytes;
        session.state = ProcessLifecycleState::Exited;
        session.exit_code = exit_code;
        session.stdout = stdout;
        session.stderr = stderr;
        session.updated_at = now.clone();
        session.completed_at = Some(now);
        self.emit_process_session("ProcessSessionExited", &session)?;
        Ok(session)
    }

    pub(crate) fn fail_process_session(
        &self,
        process_id: &str,
        error: String,
    ) -> AgentOsResult<ProcessSession> {
        let mut session = self.process_session(process_id)?;
        let now = now_rfc3339();
        session.state = ProcessLifecycleState::Failed;
        session.error = Some(error);
        session.updated_at = now.clone();
        session.completed_at = Some(now);
        self.emit_process_session("ProcessSessionFailed", &session)?;
        Ok(session)
    }

    pub(crate) fn append_process_output_chunk(
        &self,
        process_id: &str,
        stream: ProcessOutputStreamName,
        bytes: &[u8],
    ) -> AgentOsResult<ProcessOutputChunk> {
        if bytes.is_empty() {
            return Err(AgentOsError::Validation(
                "process output chunk must not be empty".to_string(),
            ));
        }
        let session = self.process_session(process_id)?;
        let current = match stream {
            ProcessOutputStreamName::Stdout => &session.stdout,
            ProcessOutputStreamName::Stderr => &session.stderr,
        };
        let start_byte = current.bytes;
        let end_byte = start_byte + bytes.len() as u64;
        let chunk = ProcessOutputChunk {
            chunk_id: new_id("pout_"),
            process_id: session.process_id.clone(),
            tool_call_id: session.tool_call_id.clone(),
            stream,
            sequence: current.sequence + 1,
            start_byte,
            end_byte,
            bytes: bytes.len() as u64,
            text: String::from_utf8_lossy(bytes).to_string(),
            created_at: now_rfc3339(),
        };
        self.emit(
            "ProcessOutputAppended",
            "process_session",
            &chunk.process_id,
            Some(session.agent_id),
            Some(session.task_id),
            Some(chunk.tool_call_id.clone()),
            None,
            &chunk,
        )?;
        Ok(chunk)
    }

    pub(crate) fn process_session_by_tool_call_id(
        &self,
        tool_call_id: &str,
    ) -> AgentOsResult<Option<ProcessSession>> {
        Ok(self
            .read_state()?
            .process_sessions
            .values()
            .find(|session| session.tool_call_id == tool_call_id)
            .cloned())
    }

    fn process_session(&self, process_id: &str) -> AgentOsResult<ProcessSession> {
        self.read_state()?
            .process_sessions
            .get(process_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("process session {process_id}")))
    }

    fn emit_process_session(
        &self,
        event_type: &str,
        session: &ProcessSession,
    ) -> AgentOsResult<()> {
        self.emit(
            event_type,
            "process_session",
            &session.process_id,
            Some(session.agent_id.clone()),
            Some(session.task_id.clone()),
            Some(session.tool_call_id.clone()),
            None,
            session,
        )?;
        Ok(())
    }
}

fn empty_process_stream(name: ProcessOutputStreamName) -> ProcessOutputStream {
    ProcessOutputStream {
        name,
        sequence: 0,
        bytes: 0,
        cursor: 0,
        truncated: false,
        spool_path: None,
    }
}
