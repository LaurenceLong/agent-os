use crate::util::{parse_payload, required_string, to_value};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

impl Kernel {
    pub fn handle_syscall(&self, syscall: SyscallEnvelope) -> AgentOsResult<SyscallResult> {
        if syscall.abi_version != ABI_VERSION {
            return Ok(SyscallResult::rejected(
                syscall.syscall_id,
                "unsupported ABI version",
            ));
        }
        if syscall.agent_id.is_empty()
            || syscall.task_id.is_empty()
            || syscall.syscall_type.is_empty()
        {
            return Ok(SyscallResult::rejected(
                syscall.syscall_id,
                "syscall lacks identity, type, or task binding",
            ));
        }
        if let Some(existing) = self.store.get_syscall_result(&syscall.idempotency_key)? {
            return Ok(existing);
        }

        let mut event_ids = Vec::new();
        let syscall_id = syscall.syscall_id.clone();
        let output = match syscall.syscall_type.as_str() {
            "goal.register" => {
                let input: RegisterGoalInput = parse_payload(&syscall.payload)?;
                let goal = self.register_goal_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&goal.goal_id)?);
                to_value(goal)?
            }
            "task.spawn" => {
                self.authorize(&syscall)?;
                let input: SpawnTaskInput = parse_payload(&syscall.payload)?;
                let task = self.spawn_task_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&task.task_id)?);
                to_value(task)?
            }
            "task.update" => {
                self.authorize(&syscall)?;
                let input: UpdateTaskInput = parse_payload(&syscall.payload)?;
                let task = self.update_task_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&task.task_id)?);
                to_value(task)?
            }
            "task.block" => {
                self.authorize(&syscall)?;
                let mut input: UpdateTaskInput = parse_payload(&syscall.payload)?;
                input.status = Some(TaskStatus::Blocked);
                let task = self.update_task_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&task.task_id)?);
                to_value(task)?
            }
            "task.complete" => {
                self.authorize(&syscall)?;
                let input: CompleteTaskInput = parse_payload(&syscall.payload)?;
                let task = self.complete_task_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&task.task_id)?);
                to_value(task)?
            }
            "agent.spawn" => {
                self.authorize(&syscall)?;
                let input: SpawnAgentInput = parse_payload(&syscall.payload)?;
                let acb = self.spawn_agent_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&acb.thread_id)?);
                to_value(acb)?
            }
            "agent.yield" => {
                self.authorize(&syscall)?;
                let acb = self.transition_thread_by_agent(
                    &syscall.agent_id,
                    ThreadStatus::Ready,
                    Some("cooperative yield".to_string()),
                    Some(syscall_id.clone()),
                )?;
                event_ids.extend(self.latest_events_for(&acb.thread_id)?);
                to_value(acb)?
            }
            "agent.suspend" => {
                self.authorize(&syscall)?;
                let acb = self.transition_thread_by_agent(
                    &syscall.agent_id,
                    ThreadStatus::Suspended,
                    Some("suspend requested".to_string()),
                    Some(syscall_id.clone()),
                )?;
                event_ids.extend(self.latest_events_for(&acb.thread_id)?);
                to_value(acb)?
            }
            "agent.resume" => {
                self.authorize(&syscall)?;
                let acb = self.transition_thread_by_agent(
                    &syscall.agent_id,
                    ThreadStatus::Ready,
                    Some("resume requested".to_string()),
                    Some(syscall_id.clone()),
                )?;
                event_ids.extend(self.latest_events_for(&acb.thread_id)?);
                to_value(acb)?
            }
            "agent.fail" => {
                self.authorize(&syscall)?;
                let acb = self.transition_thread_by_agent(
                    &syscall.agent_id,
                    ThreadStatus::Failed,
                    syscall
                        .payload
                        .get("reason")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    Some(syscall_id.clone()),
                )?;
                event_ids.extend(self.latest_events_for(&acb.thread_id)?);
                to_value(acb)?
            }
            "agent.complete" => {
                self.authorize(&syscall)?;
                let acb = self.transition_thread_by_agent(
                    &syscall.agent_id,
                    ThreadStatus::Completed,
                    Some("assignment completed".to_string()),
                    Some(syscall_id.clone()),
                )?;
                self.trigger_child_completion_mementos(&acb.thread_id, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&acb.thread_id)?);
                to_value(acb)?
            }
            "comm.send_supervisor" | "human.message" => {
                self.authorize(&syscall)?;
                let input: SendMessageInput = parse_payload(&syscall.payload)?;
                let message = self.send_message_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&message.message_id)?);
                to_value(message)?
            }
            "blackboard.post" => {
                self.authorize(&syscall)?;
                let input: PostBlackboardInput = parse_payload(&syscall.payload)?;
                let entry = self.post_blackboard_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&entry.entry_id)?);
                to_value(entry)?
            }
            "context.load" => {
                self.authorize(&syscall)?;
                let input: LoadContextInput = parse_payload(&syscall.payload)?;
                let snapshot = self.load_context_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&snapshot.context_id)?);
                to_value(snapshot)?
            }
            "context.invalidate" => {
                self.authorize(&syscall)?;
                let context_id = required_string(&syscall.payload, "context_id")?;
                let snapshot =
                    self.invalidate_context_with_cause(&context_id, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&snapshot.context_id)?);
                to_value(snapshot)?
            }
            "context.commit_summary" => {
                self.authorize(&syscall)?;
                let input: CompactContextInput = parse_payload(&syscall.payload)?;
                let compaction =
                    self.compact_context_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&compaction.compaction_id)?);
                to_value(compaction)?
            }
            "memory.propose_write" => {
                self.authorize(&syscall)?;
                let input: ProposeMemoryWriteInput = parse_payload(&syscall.payload)?;
                let memory =
                    self.propose_memory_write_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&memory.memory_id)?);
                to_value(memory)?
            }
            "memory.commit_write" => {
                self.authorize(&syscall)?;
                let input: CommitMemoryWriteInput = parse_payload(&syscall.payload)?;
                let memory =
                    self.commit_memory_write_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&memory.memory_id)?);
                to_value(memory)?
            }
            "memory.invalidate" => {
                self.authorize(&syscall)?;
                let memory_id = required_string(&syscall.payload, "memory_id")?;
                let memory =
                    self.invalidate_memory_with_cause(&memory_id, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&memory.memory_id)?);
                to_value(memory)?
            }
            "tool.discover" => {
                self.authorize(&syscall)?;
                let tools: Vec<_> = self
                    .read_state()?
                    .tool_descriptors
                    .values()
                    .cloned()
                    .collect();
                to_value(tools)?
            }
            "tool.invoke" => {
                self.authorize(&syscall)?;
                let input: ToolInvokeInput = parse_payload(&syscall.payload)?;
                let invocation =
                    self.invoke_tool_with_cause(&syscall, input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&invocation.call_id)?);
                to_value(invocation)?
            }
            "memento.create" => {
                self.authorize(&syscall)?;
                let input: CreateMementoInput = parse_payload(&syscall.payload)?;
                let memento = self.create_memento_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&memento.memento_id)?);
                to_value(memento)?
            }
            "memento.arm" => {
                self.authorize(&syscall)?;
                let memento_id = required_string(&syscall.payload, "memento_id")?;
                let memento = self.arm_memento_with_cause(
                    &syscall.agent_id,
                    &memento_id,
                    Some(syscall_id.clone()),
                )?;
                event_ids.extend(self.latest_events_for(&memento.memento_id)?);
                to_value(memento)?
            }
            "memento.consume" => {
                self.authorize(&syscall)?;
                let memento_id = required_string(&syscall.payload, "memento_id")?;
                let memento = self.consume_memento_with_cause(
                    &syscall.agent_id,
                    &memento_id,
                    Some(syscall_id.clone()),
                )?;
                event_ids.extend(self.latest_events_for(&memento.memento_id)?);
                to_value(memento)?
            }
            "artifact.commit" => {
                self.authorize(&syscall)?;
                let input: CommitArtifactInput = parse_payload(&syscall.payload)?;
                let artifact = self.commit_artifact_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&artifact.artifact_id)?);
                to_value(artifact)?
            }
            "evidence.attach" => {
                self.authorize(&syscall)?;
                let input: AttachEvidenceInput = parse_payload(&syscall.payload)?;
                let evidence = self.attach_evidence_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&evidence.evidence_id)?);
                to_value(evidence)?
            }
            "review.request" => {
                self.authorize(&syscall)?;
                let input: RequestReviewInput = parse_payload(&syscall.payload)?;
                let review = self.request_review_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&review.review_id)?);
                to_value(review)?
            }
            "review.submit" => {
                self.authorize(&syscall)?;
                let input: SubmitReviewInput = parse_payload(&syscall.payload)?;
                let review = self.submit_review_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&review.review_id)?);
                to_value(review)?
            }
            "verify.submit" => {
                self.authorize(&syscall)?;
                let input: SubmitVerificationInput = parse_payload(&syscall.payload)?;
                let verification =
                    self.submit_verification_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&verification.verification_id)?);
                to_value(verification)?
            }
            "approval.request" => {
                self.authorize(&syscall)?;
                let input: RequestApprovalInput = parse_payload(&syscall.payload)?;
                let approval = self.request_approval_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&approval.approval_id)?);
                to_value(approval)?
            }
            "approval.record" => {
                let input: RecordApprovalInput = parse_payload(&syscall.payload)?;
                let approval = self.record_approval_with_cause(input, Some(syscall_id.clone()))?;
                event_ids.extend(self.latest_events_for(&approval.approval_id)?);
                to_value(approval)?
            }
            "final.submit" => {
                self.authorize(&syscall)?;
                let final_submission: FinalSubmission = parse_payload(&syscall.payload)?;
                self.submit_final_with_cause(
                    &syscall.agent_id,
                    &syscall.task_id,
                    final_submission.clone(),
                    Some(syscall_id.clone()),
                )?;
                event_ids.extend(self.latest_events_for(&syscall.task_id)?);
                to_value(final_submission)?
            }
            "policy.check" => {
                self.authorize(&syscall)?;
                json!({"decision": "allow"})
            }
            other => {
                return Err(AgentOsError::Validation(format!(
                    "unknown syscall op {other}"
                )));
            }
        };

        let result = SyscallResult::accepted(syscall.syscall_id, event_ids, output);
        self.store
            .put_syscall_result(syscall.idempotency_key, result.clone())?;
        Ok(result)
    }
}
