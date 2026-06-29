mod driver;
mod validation;

use crate::schema::validate_json_schema;
use crate::util::{hash_json, to_value};
use crate::*;
use agent_os_sys::*;
use serde_json::json;

impl Kernel {
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
        validate_json_schema(&descriptor.input_schema, &input.input, "tool.input")?;
        let now = now_rfc3339();
        let mut invocation = ToolInvocation {
            call_id: new_id("call_"),
            tool_id: descriptor.tool_id.clone(),
            tool_name: descriptor.name.clone(),
            agent_id: syscall.agent_id.clone(),
            task_id: syscall.task_id.clone(),
            status: ToolCallStatus::Running,
            risk_level: syscall.risk_level,
            input: input.input,
            output: None,
            evidence_ids: Vec::new(),
            audit_refs: Vec::new(),
            created_at: now,
            completed_at: None,
        };
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
        let output = match driver::run_tool_driver(self, syscall, &descriptor, &invocation.input) {
            Ok(output) => output,
            Err(error) => {
                invocation.status = ToolCallStatus::Failed;
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
                return Err(error);
            }
        };
        if let Err(error) = validate_json_schema(&descriptor.output_schema, &output, "tool.output")
        {
            invocation.output = Some(output);
            invocation.status = ToolCallStatus::Failed;
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
            return Err(error);
        }
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
        Ok(invocation)
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
