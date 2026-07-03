mod builtin;
mod driver;
mod output;
mod validation;

use crate::schema::validate_json_schema;
use crate::state::{ToolStreamOutput, ToolWorkerOutput, ToolWorkerRecord};
use crate::util::{hash_json, to_value};
use crate::*;
use agent_os_sys::*;
use serde_json::json;
use std::sync::mpsc;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub(in crate::tools) enum ToolOutputStream {
    Stdout,
    Stderr,
}

pub(in crate::tools) struct StreamWindow {
    pub text: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub truncated: bool,
}

pub(crate) fn core_tool_descriptors(now: &str) -> Vec<ToolDescriptor> {
    builtin::core_tool_descriptors(now)
}

impl Kernel {
    pub fn plan_tools_for_turn(
        &self,
        thread: &AgentControlBlock,
        model_capabilities: ModelCapabilities,
        mode: ToolPlanningMode,
    ) -> AgentOsResult<ToolPlan> {
        let state = self.read_state()?;
        let mut descriptors = state.tool_descriptors.values().cloned().collect::<Vec<_>>();
        descriptors.sort_by(|left, right| left.name.cmp(&right.name));
        drop(state);

        let mut entries: Vec<ToolPlanEntry> = descriptors
            .into_iter()
            .map(|descriptor| {
                let (exposure, reason) =
                    self.plan_tool_exposure(thread, &model_capabilities, mode, &descriptor);
                ToolPlanEntry {
                    descriptor,
                    exposure,
                    reason,
                }
            })
            .collect();
        let has_deferred_tools = entries
            .iter()
            .any(|entry| entry.exposure == ToolExposure::Deferred);
        for entry in &mut entries {
            if entry.descriptor.name != "tool_search" {
                continue;
            }
            if has_deferred_tools
                && entry.exposure == ToolExposure::Hidden
                && planning_mode_allows_tool(mode, &entry.descriptor.name)
            {
                entry.exposure = ToolExposure::Direct;
                entry.reason = Some("deferred tools are available for discovery".to_string());
            } else {
                entry.exposure = ToolExposure::Hidden;
                entry.reason = Some("no deferred tools are available for discovery".to_string());
            }
        }

        Ok(ToolPlan {
            plan_id: new_id("tool_plan_"),
            thread_id: thread.thread_id.clone(),
            agent_id: thread.agent_id.clone(),
            task_id: thread.task.task_id.clone(),
            mode,
            model_capabilities,
            entries,
            created_at: now_rfc3339(),
        })
    }

    fn plan_tool_exposure(
        &self,
        thread: &AgentControlBlock,
        model_capabilities: &ModelCapabilities,
        mode: ToolPlanningMode,
        descriptor: &ToolDescriptor,
    ) -> (ToolExposure, Option<String>) {
        if !planning_mode_allows_tool(mode, &descriptor.name) {
            return (
                ToolExposure::Hidden,
                Some(format!("tool hidden by {mode:?} planning mode")),
            );
        }
        if descriptor.name == "read_image" && !model_capabilities.image_input {
            return (
                ToolExposure::Disabled,
                Some("read_image requires a model with image_input capability".to_string()),
            );
        }
        if let Err(error) = self.require_tool_authority(thread, descriptor, descriptor.risk_level) {
            return (ToolExposure::Disabled, Some(error.to_string()));
        }
        if descriptor.name == "tool_search" {
            return (
                ToolExposure::Hidden,
                Some("no deferred tools are available for discovery".to_string()),
            );
        }
        if descriptor.driver_class == ToolDriverClass::Mcp {
            return (
                ToolExposure::Deferred,
                Some("MCP tool is discoverable through tool_search".to_string()),
            );
        }
        (ToolExposure::Direct, None)
    }

    pub fn register_tool_descriptor(
        &self,
        descriptor: ToolDescriptor,
    ) -> AgentOsResult<ToolDescriptor> {
        validation::validate_tool_descriptor(&descriptor)?;
        self.emit(
            "ToolRegistered",
            "tool_descriptor",
            &descriptor.tool_id,
            None,
            None,
            None,
            None,
            &descriptor,
        )?;
        Ok(descriptor)
    }

    pub fn invoke_tool(
        &self,
        agent_id: &str,
        task_id: &str,
        session_id: &str,
        capability_id: String,
        risk_level: u8,
        input: ToolInvokeInput,
    ) -> AgentOsResult<ToolInvocation> {
        let descriptor = self.tool_descriptor_for_invocation(&input)?;
        let mut syscall = SyscallEnvelope::new(
            "tool.invoke",
            agent_id,
            task_id,
            session_id,
            Some(capability_id),
            risk_level,
            to_value(input.clone())?,
        );
        syscall.resource_scope = tool_resource_scope(&descriptor, &input.input);
        if let Err(error) = self.authorize(&syscall) {
            self.record_denied_tool_call(&syscall, &input, &error, None)?;
            return Err(error);
        }
        self.invoke_tool_with_cause(&syscall, input, None)
    }

    pub fn record_tool_proposal(
        &self,
        agent_id: &str,
        task_id: &str,
        input: ToolInvokeInput,
        risk_level: u8,
    ) -> AgentOsResult<ToolInvocation> {
        let descriptor = self.tool_descriptor_for_invocation(&input)?;
        let invocation = ToolInvocation {
            call_id: new_id("call_"),
            tool_id: descriptor.tool_id,
            tool_name: input.tool_name,
            agent_id: agent_id.to_string(),
            task_id: task_id.to_string(),
            status: ToolCallStatus::Proposed,
            risk_level,
            input: input.input,
            output: None,
            evidence_ids: Vec::new(),
            audit_refs: Vec::new(),
            created_at: now_rfc3339(),
            completed_at: None,
        };
        self.emit(
            "ToolCallProposed",
            "tool_invocation",
            &invocation.call_id,
            Some(invocation.agent_id.clone()),
            Some(invocation.task_id.clone()),
            None,
            None,
            &invocation,
        )?;
        Ok(invocation)
    }

    pub(crate) fn invoke_tool_with_cause(
        &self,
        syscall: &SyscallEnvelope,
        input: ToolInvokeInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<ToolInvocation> {
        let descriptor = self
            .read_state()?
            .tool_descriptors
            .get(&input.tool_name)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("tool {}", input.tool_name)))?;
        if syscall.risk_level > descriptor.risk_level {
            return Err(AgentOsError::PermissionDenied(
                "syscall risk exceeds tool descriptor risk declaration".to_string(),
            ));
        }
        let acb = self
            .thread_by_agent(&syscall.agent_id)?
            .ok_or_else(|| AgentOsError::NotFound(format!("agent {}", syscall.agent_id)))?;
        self.require_tool_authority(&acb, &descriptor, syscall.risk_level)?;
        let now = now_rfc3339();
        let mut invocation = ToolInvocation {
            call_id: new_id("call_"),
            tool_id: descriptor.tool_id.clone(),
            tool_name: descriptor.name.clone(),
            agent_id: syscall.agent_id.clone(),
            task_id: syscall.task_id.clone(),
            status: ToolCallStatus::Running,
            risk_level: syscall.risk_level,
            input: input.input.clone(),
            output: None,
            evidence_ids: Vec::new(),
            audit_refs: Vec::new(),
            created_at: now,
            completed_at: None,
        };
        if let Err(error) =
            validate_json_schema(&descriptor.input_schema, &input.input, "tool.input")
        {
            invocation.status = ToolCallStatus::Failed;
            invocation.output = Some(tool_failure_output("input_schema", &error));
            invocation.completed_at = Some(now_rfc3339());
            self.emit(
                "ToolCallFailed",
                "tool_invocation",
                &invocation.call_id,
                Some(invocation.agent_id.clone()),
                Some(invocation.task_id.clone()),
                causation_id,
                None,
                &invocation,
            )?;
            self.audit(
                AuditActorType::Agent,
                &syscall.agent_id,
                "tool.invoke",
                "tool_invocation",
                &invocation.call_id,
                Some("tool input schema validation failed".to_string()),
                AuditResult::Error,
            )?;
            return Ok(invocation);
        }
        self.emit(
            "ToolCallStarted",
            "tool_invocation",
            &invocation.call_id,
            Some(invocation.agent_id.clone()),
            Some(invocation.task_id.clone()),
            causation_id.clone(),
            None,
            &invocation,
        )?;
        self.run_started_tool_with_foreground_timeout(
            syscall.clone(),
            input,
            descriptor,
            invocation,
            causation_id,
        )
    }

    fn run_started_tool_with_foreground_timeout(
        &self,
        syscall: SyscallEnvelope,
        input: ToolInvokeInput,
        descriptor: ToolDescriptor,
        invocation: ToolInvocation,
        causation_id: Option<String>,
    ) -> AgentOsResult<ToolInvocation> {
        let foreground_timeout = Duration::from_millis(descriptor.lifecycle.foreground_timeout_ms);
        let call_id = invocation.call_id.clone();
        let running_snapshot = invocation.clone();
        self.register_tool_worker(&invocation)?;
        let (sender, receiver) = mpsc::channel();
        let worker_kernel = self.clone();
        let worker_call_id = call_id.clone();
        std::thread::spawn(move || {
            let result = worker_kernel.complete_started_tool_invocation(
                syscall,
                input,
                descriptor,
                invocation,
                causation_id,
            );
            worker_kernel.unregister_tool_worker(&worker_call_id);
            let _ = sender.send(result);
        });
        match receiver.recv_timeout(foreground_timeout) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let mut progressed = running_snapshot;
                let process_id = self
                    .process_session_by_tool_call_id(&progressed.call_id)?
                    .map(|session| session.process_id);
                progressed.output = Some(json!({
                    "status": "running",
                    "tool_call_id": progressed.call_id,
                    "process_id": process_id,
                    "tool_name": progressed.tool_name,
                    "foreground_timeout_ms": foreground_timeout.as_millis() as u64,
                    "message": "tool exceeded the foreground wait cap and is still running in the background"
                }));
                self.emit(
                    "ToolCallProgressed",
                    "tool_invocation",
                    &progressed.call_id,
                    Some(progressed.agent_id.clone()),
                    Some(progressed.task_id.clone()),
                    None,
                    None,
                    &progressed,
                )?;
                Ok(progressed)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let mut failed = running_snapshot;
                failed.status = ToolCallStatus::Failed;
                failed.output = Some(json!({
                    "status": "failed",
                    "stage": "worker",
                    "error": "tool worker disconnected before reporting a result"
                }));
                failed.completed_at = Some(now_rfc3339());
                self.emit(
                    "ToolCallFailed",
                    "tool_invocation",
                    &failed.call_id,
                    Some(failed.agent_id.clone()),
                    Some(failed.task_id.clone()),
                    None,
                    None,
                    &failed,
                )?;
                Ok(failed)
            }
        }
    }

    fn complete_started_tool_invocation(
        &self,
        syscall: SyscallEnvelope,
        input: ToolInvokeInput,
        descriptor: ToolDescriptor,
        mut invocation: ToolInvocation,
        causation_id: Option<String>,
    ) -> AgentOsResult<ToolInvocation> {
        let output = match driver::run_tool_driver(
            self,
            &syscall,
            &descriptor,
            &invocation.call_id,
            &invocation.input,
        ) {
            Ok(output) => output,
            Err(error) => {
                invocation.status = ToolCallStatus::Failed;
                invocation.output = Some(tool_failure_output("driver", &error));
                invocation.completed_at = Some(now_rfc3339());
                self.emit(
                    "ToolCallFailed",
                    "tool_invocation",
                    &invocation.call_id,
                    Some(invocation.agent_id.clone()),
                    Some(invocation.task_id.clone()),
                    causation_id,
                    None,
                    &invocation,
                )?;
                self.audit(
                    AuditActorType::Agent,
                    &syscall.agent_id,
                    "tool.invoke",
                    "tool_invocation",
                    &invocation.call_id,
                    Some("tool driver failed".to_string()),
                    AuditResult::Error,
                )?;
                self.ready_thread_after_background_tool(&syscall)?;
                return Ok(invocation);
            }
        };
        if let Err(error) = validate_json_schema(&descriptor.output_schema, &output, "tool.output")
        {
            invocation.status = ToolCallStatus::Failed;
            invocation.output = Some(json!({
                "status": "failed",
                "stage": "output_schema",
                "error": error.to_string(),
                "invalid_output": output,
            }));
            invocation.completed_at = Some(now_rfc3339());
            self.emit(
                "ToolCallFailed",
                "tool_invocation",
                &invocation.call_id,
                Some(invocation.agent_id.clone()),
                Some(invocation.task_id.clone()),
                causation_id,
                None,
                &invocation,
            )?;
            self.audit(
                AuditActorType::Agent,
                &syscall.agent_id,
                "tool.invoke",
                "tool_invocation",
                &invocation.call_id,
                Some("tool output schema validation failed".to_string()),
                AuditResult::Error,
            )?;
            self.ready_thread_after_background_tool(&syscall)?;
            return Ok(invocation);
        }
        let output = output::attach_output_management(self, &invocation.call_id, output)?;
        invocation.output = Some(output.clone());
        invocation.status = ToolCallStatus::Completed;
        invocation.completed_at = Some(now_rfc3339());
        if let Some(evidence_type) = descriptor.evidence_type {
            let acb = self
                .thread_by_agent(&syscall.agent_id)?
                .ok_or_else(|| AgentOsError::NotFound(format!("agent {}", syscall.agent_id)))?;
            let evidence = self.attach_evidence_with_cause(
                AttachEvidenceInput {
                    goal_id: acb.task.goal_id,
                    task_id: Some(syscall.task_id.clone()),
                    artifact_id: None,
                    evidence_type,
                    producer_agent_id: Some(syscall.agent_id.clone()),
                    claim: input.evidence_claim,
                    blob_ref: None,
                    content_hash: Some(hash_json(&output)?),
                    inline_bytes: None,
                    metadata: json!({
                        "tool_call_id": invocation.call_id,
                        "tool_name": invocation.tool_name,
                        "output": output,
                    }),
                },
                causation_id.clone(),
            )?;
            invocation.evidence_ids.push(evidence.evidence_id);
        }
        self.emit(
            "ToolCallCompleted",
            "tool_invocation",
            &invocation.call_id,
            Some(invocation.agent_id.clone()),
            Some(invocation.task_id.clone()),
            causation_id,
            None,
            &invocation,
        )?;
        self.audit(
            AuditActorType::Agent,
            &syscall.agent_id,
            "tool.invoke",
            "tool_invocation",
            &invocation.call_id,
            None,
            AuditResult::Success,
        )?;
        self.ready_thread_after_background_tool(&syscall)?;
        Ok(invocation)
    }

    fn register_tool_worker(&self, invocation: &ToolInvocation) -> AgentOsResult<()> {
        let mut workers = self.tool_workers.lock().map_err(|_| {
            AgentOsError::Validation("tool worker registry lock poisoned".to_string())
        })?;
        workers.insert(
            invocation.call_id.clone(),
            ToolWorkerRecord {
                call_id: invocation.call_id.clone(),
                tool_name: invocation.tool_name.clone(),
                started_at: now_rfc3339(),
                child: None,
                stdin: None,
                output: ToolWorkerOutput::default(),
            },
        );
        Ok(())
    }

    pub(in crate::tools) fn append_tool_worker_output(
        &self,
        call_id: &str,
        stream: ToolOutputStream,
        chunk: &[u8],
    ) {
        if chunk.is_empty() {
            return;
        }
        let Ok(mut workers) = self.tool_workers.lock() else {
            return;
        };
        let Some(worker) = workers.get_mut(call_id) else {
            return;
        };
        match stream {
            ToolOutputStream::Stdout => append_stream_output(&mut worker.output.stdout, chunk),
            ToolOutputStream::Stderr => append_stream_output(&mut worker.output.stderr, chunk),
        }
        worker.output.updated_at = Some(now_rfc3339());
    }

    pub(in crate::tools) fn set_tool_worker_output_spool(
        &self,
        call_id: &str,
        stdout_path: String,
        stderr_path: String,
    ) {
        let Ok(mut workers) = self.tool_workers.lock() else {
            return;
        };
        let Some(worker) = workers.get_mut(call_id) else {
            return;
        };
        worker.output.stdout.spool_path = Some(stdout_path);
        worker.output.stderr.spool_path = Some(stderr_path);
        worker.output.updated_at = Some(now_rfc3339());
    }

    pub(in crate::tools) fn set_tool_worker_stdin(
        &self,
        call_id: &str,
        stdin: std::process::ChildStdin,
    ) -> AgentOsResult<()> {
        let mut workers = self.tool_workers.lock().map_err(|_| {
            AgentOsError::Validation("tool worker registry lock poisoned".to_string())
        })?;
        let Some(worker) = workers.get_mut(call_id) else {
            return Err(AgentOsError::NotFound(format!("tool worker {call_id}")));
        };
        worker.stdin = Some(std::sync::Arc::new(std::sync::Mutex::new(stdin)));
        Ok(())
    }

    pub(in crate::tools) fn set_tool_worker_child(
        &self,
        call_id: &str,
        child: std::sync::Arc<std::sync::Mutex<std::process::Child>>,
    ) -> AgentOsResult<()> {
        let mut workers = self.tool_workers.lock().map_err(|_| {
            AgentOsError::Validation("tool worker registry lock poisoned".to_string())
        })?;
        let Some(worker) = workers.get_mut(call_id) else {
            return Err(AgentOsError::NotFound(format!("tool worker {call_id}")));
        };
        worker.child = Some(child);
        Ok(())
    }

    fn unregister_tool_worker(&self, call_id: &str) {
        if let Ok(mut workers) = self.tool_workers.lock() {
            workers.remove(call_id);
        }
    }

    fn ready_thread_after_background_tool(&self, syscall: &SyscallEnvelope) -> AgentOsResult<()> {
        let acb = self
            .thread_by_agent(&syscall.agent_id)?
            .ok_or_else(|| AgentOsError::NotFound(format!("agent {}", syscall.agent_id)))?;
        if acb.status != ThreadStatus::WaitingTool {
            return Ok(());
        }
        let has_running_tool = self
            .read_state()?
            .tool_invocations
            .values()
            .any(|invocation| {
                invocation.task_id == syscall.task_id
                    && invocation.status == ToolCallStatus::Running
            });
        if has_running_tool {
            return Ok(());
        }
        self.transition_thread(
            &acb.thread_id,
            ThreadStatus::Ready,
            Some("background tool completed".to_string()),
        )?;
        Ok(())
    }

    fn record_denied_tool_call(
        &self,
        syscall: &SyscallEnvelope,
        input: &ToolInvokeInput,
        error: &AgentOsError,
        causation_id: Option<String>,
    ) -> AgentOsResult<ToolInvocation> {
        let descriptor = self.tool_descriptor_for_invocation(input)?;
        let now = now_rfc3339();
        let invocation = ToolInvocation {
            call_id: new_id("call_"),
            tool_id: descriptor.tool_id,
            tool_name: input.tool_name.clone(),
            agent_id: syscall.agent_id.clone(),
            task_id: syscall.task_id.clone(),
            status: ToolCallStatus::Denied,
            risk_level: syscall.risk_level,
            input: input.input.clone(),
            output: Some(json!({
                "status": "denied",
                "reason": error.to_string()
            })),
            evidence_ids: Vec::new(),
            audit_refs: Vec::new(),
            created_at: now.clone(),
            completed_at: Some(now),
        };
        self.emit(
            "ToolCallDenied",
            "tool_invocation",
            &invocation.call_id,
            Some(invocation.agent_id.clone()),
            Some(invocation.task_id.clone()),
            causation_id,
            None,
            &invocation,
        )?;
        self.audit(
            AuditActorType::Agent,
            &syscall.agent_id,
            "tool.invoke",
            "tool_invocation",
            &invocation.call_id,
            Some(error.to_string()),
            AuditResult::Deny,
        )?;
        Ok(invocation)
    }

    fn tool_descriptor_for_invocation(
        &self,
        input: &ToolInvokeInput,
    ) -> AgentOsResult<ToolDescriptor> {
        self.read_state()?
            .tool_descriptors
            .get(&input.tool_name)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("tool {}", input.tool_name)))
    }
}

fn planning_mode_allows_tool(mode: ToolPlanningMode, tool_name: &str) -> bool {
    match mode {
        ToolPlanningMode::Normal => true,
        ToolPlanningMode::FinalizationOnly => {
            matches!(tool_name, "submit_final" | "accomplish_goal")
        }
        ToolPlanningMode::PrePatchResolution => {
            matches!(
                tool_name,
                "apply_patch" | "submit_final" | "accomplish_goal"
            )
        }
    }
}

fn append_stream_output(output: &mut ToolStreamOutput, chunk: &[u8]) {
    let limit = builtin::run_command::OUTPUT_PREVIEW_CHARS;
    let head_remaining = limit.saturating_sub(output.head.len());
    if head_remaining > 0 {
        output
            .head
            .extend_from_slice(&chunk[..chunk.len().min(head_remaining)]);
    }
    output.tail.extend_from_slice(chunk);
    if output.tail.len() > limit {
        let excess = output.tail.len() - limit;
        output.tail.drain(0..excess);
        output.truncated = true;
    }
    output.bytes = output.bytes.saturating_add(chunk.len());
}

impl ToolStreamOutput {
    pub(in crate::tools) fn append_bounded(&mut self, chunk: &[u8]) {
        append_stream_output(self, chunk);
    }

    pub(in crate::tools) fn head_window(&self, limit: usize) -> StreamWindow {
        let byte_limit = limit.min(builtin::run_command::OUTPUT_PREVIEW_CHARS);
        let end_byte = self.head.len().min(byte_limit);
        StreamWindow {
            text: String::from_utf8_lossy(&self.head[..end_byte]).to_string(),
            start_byte: 0,
            end_byte,
            truncated: self.bytes > end_byte,
        }
    }

    pub(in crate::tools) fn tail_window(&self, limit: usize) -> StreamWindow {
        let byte_limit = limit.min(builtin::run_command::OUTPUT_PREVIEW_CHARS);
        let selected_len = self.tail.len().min(byte_limit);
        let start = self.tail.len().saturating_sub(selected_len);
        let start_byte = self.bytes.saturating_sub(self.tail.len()) + start;
        StreamWindow {
            text: String::from_utf8_lossy(&self.tail[start..]).to_string(),
            start_byte,
            end_byte: self.bytes,
            truncated: self.bytes > selected_len,
        }
    }

    pub(in crate::tools) fn new_window(&self, cursor: usize, limit: usize) -> StreamWindow {
        let byte_limit = limit.min(builtin::run_command::OUTPUT_PREVIEW_CHARS);
        let tail_start_byte = self.bytes.saturating_sub(self.tail.len());
        let effective_cursor = cursor.max(tail_start_byte).min(self.bytes);
        let tail_offset = effective_cursor.saturating_sub(tail_start_byte);
        let available = &self.tail[tail_offset..];
        let selected_len = available.len().min(byte_limit);
        StreamWindow {
            text: String::from_utf8_lossy(&available[..selected_len]).to_string(),
            start_byte: effective_cursor,
            end_byte: effective_cursor + selected_len,
            truncated: cursor < tail_start_byte || available.len() > selected_len,
        }
    }
}

fn tool_failure_output(stage: &str, error: &AgentOsError) -> serde_json::Value {
    json!({
        "status": "failed",
        "stage": stage,
        "error": error.to_string(),
    })
}

fn tool_resource_scope(
    descriptor: &ToolDescriptor,
    input: &serde_json::Value,
) -> serde_json::Value {
    let mut scopes = vec![format!("tool:{}", descriptor.name)];
    scopes.extend(
        descriptor
            .runtime_input_policy
            .required_resource_scopes
            .iter()
            .filter(|scope| !scope.contains('*'))
            .cloned(),
    );
    match descriptor.name.as_str() {
        "load_skill" => {
            if let Some(name) = input.get("name").and_then(serde_json::Value::as_str) {
                scopes.push(format!("skill:{name}"));
            }
        }
        "read_skill_resource" => {
            if let Some(name) = input.get("name").and_then(serde_json::Value::as_str) {
                scopes.push(format!("skill:{name}"));
                if let Some(path) = input.get("path").and_then(serde_json::Value::as_str) {
                    scopes.push(format!("skill_file:{name}:{path}"));
                }
            }
        }
        _ if descriptor.driver_class == ToolDriverClass::Mcp => {
            let server = descriptor
                .driver_config
                .get("server_name")
                .and_then(serde_json::Value::as_str);
            let tool = descriptor
                .driver_config
                .get("tool_name")
                .and_then(serde_json::Value::as_str);
            if let (Some(server), Some(tool)) = (server, tool) {
                scopes.push(format!("mcp:{server}:{tool}"));
            }
        }
        _ => {}
    }
    json!(scopes)
}
