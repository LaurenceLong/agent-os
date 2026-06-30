use crate::{
    ArtifactRecord, ModelAction, ModelClient, ModelContextProjection, ModelTurnRequest, ToolAction,
    ToolExecutionRecord,
};
use agent_os_kernel::{
    CommitArtifactInput, CompleteTaskInput, Kernel, ToolInvokeInput, UpdateTaskInput,
};
use agent_os_sys::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

#[path = "runtime/ecosystem_projection.rs"]
mod ecosystem_projection;
#[path = "runtime/tool_policy.rs"]
mod tool_policy;

const MAX_PROJECTED_TOOL_RESULTS: usize = 8;
const MAX_PROJECTED_ARTIFACTS: usize = 8;
const MAX_PROJECTED_OLDER_TOOL_STRING_CHARS: usize = 2000;
const RUNTIME_FEEDBACK_TOOL: &str = "runtime_feedback";
const MAX_CONSECUTIVE_NO_ACTION_TURNS: u32 = 2;

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub workspace_root: PathBuf,
    pub attach_mode: AttachMode,
    pub max_steps: u32,
    pub requested_model_alias: Option<String>,
    pub tool_risk_ceiling: u8,
    pub auto_commit_patch_artifacts: bool,
    pub fail_on_process_nonzero: bool,
}

impl RuntimeConfig {
    pub fn workspace_write(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            attach_mode: AttachMode::WorkspaceWrite,
            max_steps: 16,
            requested_model_alias: None,
            tool_risk_ceiling: 4,
            auto_commit_patch_artifacts: true,
            fail_on_process_nonzero: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeRunOverrides {
    pub sandbox_profile_id: Option<String>,
    pub tool_approval_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeRunReport {
    pub thread_id: String,
    pub task_id: String,
    pub status: ThreadStatus,
    pub provider_stream_session_ids: Vec<String>,
    pub tool_results: Vec<ToolExecutionRecord>,
    pub artifacts: Vec<ArtifactRecord>,
    pub final_submitted: bool,
    pub events: usize,
}

pub struct ThreadRuntime<C> {
    kernel: Kernel,
    thread_id: String,
    model_client: C,
}

impl<C: ModelClient> ThreadRuntime<C> {
    pub fn new(kernel: Kernel, thread_id: impl Into<String>, model_client: C) -> Self {
        Self {
            kernel,
            thread_id: thread_id.into(),
            model_client,
        }
    }

    pub fn run_to_completion(&mut self, config: RuntimeConfig) -> AgentOsResult<RuntimeRunReport> {
        self.run_to_completion_with_overrides(config, RuntimeRunOverrides::default())
    }

    pub fn run_to_completion_with_overrides(
        &mut self,
        config: RuntimeConfig,
        overrides: RuntimeRunOverrides,
    ) -> AgentOsResult<RuntimeRunReport> {
        crate::ecosystem::import_workspace_ecosystem(&self.kernel, &config.workspace_root)?;
        let mut acb = self.acb()?;
        self.kernel.update_task(UpdateTaskInput {
            task_id: acb.task.task_id.clone(),
            status: Some(TaskStatus::Running),
            blocked_reason: None,
            owner_agent_id: Some(acb.agent_id.clone()),
            title: None,
            description: None,
            checklist: None,
        })?;
        acb = self.kernel.start_turn(&self.thread_id)?;
        let env = self.kernel.create_environment(
            BackendType::IsolatedWorktree,
            config.workspace_root.to_string_lossy(),
            overrides
                .sandbox_profile_id
                .clone()
                .unwrap_or_else(|| acb.config_snapshot.sandbox_profile_id.clone()),
            ReusePolicy::TaskScoped,
        )?;
        let environment_lease = self.kernel.attach_environment(
            &env.environment_id,
            &acb.agent_id,
            &acb.thread_id,
            &acb.task.task_id,
            config.attach_mode,
        )?;
        let capability = self.kernel.grant_capability(
            &acb.agent_id,
            &acb.task.task_id,
            vec!["tool.invoke".to_string()],
            vec![
                "tool:*".to_string(),
                "instruction:*".to_string(),
                "skill:*".to_string(),
                "skill_file:*".to_string(),
                "mcp:*".to_string(),
            ],
            config.tool_risk_ceiling,
            overrides.tool_approval_id.clone(),
        )?;

        let mut provider_stream_session_ids = Vec::new();
        let mut tool_results = self.hydrated_tool_results(&acb.task.task_id)?;
        let mut artifacts = self.hydrated_artifacts(&acb.task.task_id)?;
        let mut final_submitted = false;
        let mut consecutive_no_action_turns = 0;

        for step_index in 0..config.max_steps {
            // Yield boundary: before model call.
            self.kernel.record_checkpoint(
                &acb.thread_id,
                format!("ckpt_before_model_{}", new_id("y_")),
            )?;
            let stream = self.open_stream_session(&acb, step_index, &config)?;
            provider_stream_session_ids.push(stream.session_id.clone());
            let state = self.kernel.state_snapshot()?;
            let projected_tool_results = project_tool_results(&tool_results);
            let artifact_recent_start = artifacts.len().saturating_sub(MAX_PROJECTED_ARTIFACTS);
            let projected_artifacts = artifacts
                .iter()
                .enumerate()
                .filter(|(index, artifact)| {
                    *index >= artifact_recent_start || !artifact.evidence_ids.is_empty()
                })
                .map(|(_, artifact)| artifact.clone())
                .collect();
            let mut context_snapshots: Vec<_> = state
                .context_snapshots
                .values()
                .filter(|snapshot| {
                    snapshot.task_id == acb.task.task_id
                        && snapshot.freshness == ContextFreshness::Fresh
                })
                .cloned()
                .collect();
            context_snapshots.sort_by(|left, right| {
                left.created_at
                    .cmp(&right.created_at)
                    .then_with(|| left.context_id.cmp(&right.context_id))
            });
            let mut memory_records: Vec<_> = state
                .memory_records
                .values()
                .filter(|record| record.status == MemoryStatus::Active)
                .cloned()
                .collect();
            memory_records.sort_by(|left, right| {
                left.created_at
                    .cmp(&right.created_at)
                    .then_with(|| left.memory_id.cmp(&right.memory_id))
            });
            let mut context_compactions: Vec<_> = state
                .context_compactions
                .values()
                .filter(|compaction| compaction.task_id == acb.task.task_id)
                .cloned()
                .collect();
            context_compactions.sort_by(|left, right| {
                left.created_at
                    .cmp(&right.created_at)
                    .then_with(|| left.compaction_id.cmp(&right.compaction_id))
            });
            let ecosystem_projection = ecosystem_projection::from_state(&state);
            let provider_profile = state
                .provider_profiles
                .get(&acb.config_snapshot.provider_profile_id)
                .ok_or_else(|| {
                    AgentOsError::NotFound(format!(
                        "provider profile {}",
                        acb.config_snapshot.provider_profile_id
                    ))
                })?;
            let max_attempts = provider_profile
                .retry_policy
                .as_ref()
                .and_then(|policy| policy.get("max_attempts"))
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .max(1);
            let backoff_ms = provider_profile
                .retry_policy
                .as_ref()
                .and_then(|policy| policy.get("backoff_ms"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let request = ModelTurnRequest {
                thread: acb.clone(),
                workspace_root: config.workspace_root.clone(),
                step_index,
                context: ModelContextProjection {
                    tool_results: projected_tool_results,
                    artifacts: projected_artifacts,
                    context_snapshots,
                    memory_records,
                    context_compactions,
                    tool_descriptors: ecosystem_projection.tool_descriptors,
                    instruction_documents: ecosystem_projection.instruction_documents,
                    skill_definitions: ecosystem_projection.skill_definitions,
                    command_definitions: ecosystem_projection.command_definitions,
                    mcp_tools: ecosystem_projection.mcp_tools,
                    imported_agent_profiles: ecosystem_projection.imported_agent_profiles,
                },
            };
            let mut attempt = 1;
            let response = loop {
                match self.model_client.next(&request) {
                    Ok(response) => break response,
                    Err(error) => {
                        if attempt >= max_attempts {
                            self.kernel
                                .fail_stream_session(&stream.session_id, error.to_string())?;
                            return Err(error);
                        }
                        self.kernel.record_provider_stream_event(
                            &stream.session_id,
                            ProviderStreamEventType::ProviderRetry,
                            json!({
                                "attempt": attempt,
                                "max_attempts": max_attempts,
                                "error": error.to_string()
                            }),
                        )?;
                        if backoff_ms > 0 {
                            std::thread::sleep(std::time::Duration::from_millis(backoff_ms));
                        }
                        attempt += 1;
                    }
                }
            };
            let mut turn_had_completion_action = false;
            let mut output_texts = Vec::new();
            for action in response.actions {
                match action {
                    ModelAction::OutputText { text } => {
                        output_texts.push(text.clone());
                        self.kernel.record_provider_stream_event(
                            &stream.session_id,
                            ProviderStreamEventType::OutputTextDelta,
                            json!({ "text": text }),
                        )?;
                    }
                    ModelAction::ToolCall(action) => {
                        turn_had_completion_action = true;
                        let record = self.execute_tool_action(
                            &acb,
                            &stream.session_id,
                            &capability.capability_id,
                            action,
                        )?;
                        if config.auto_commit_patch_artifacts {
                            if let Some(artifact) = self.commit_patch_artifact_for_tool(
                                &acb,
                                &env.environment_id,
                                &environment_lease.environment_lease_id,
                                &record,
                            )? {
                                artifacts.push(artifact);
                                // Yield boundary: after artifact commit.
                                self.kernel.record_checkpoint(
                                    &acb.thread_id,
                                    format!("ckpt_after_artifact_commit_{}", new_id("y_")),
                                )?;
                            }
                        }
                        tool_policy::enforce(&record, &config)?;
                        tool_results.push(record);
                        // Yield boundary: after tool result.
                        self.kernel.record_checkpoint(
                            &acb.thread_id,
                            format!("ckpt_after_tool_{}", new_id("y_")),
                        )?;
                    }
                    ModelAction::Final { submission } => {
                        turn_had_completion_action = true;
                        // Yield boundary: before final submission.
                        self.kernel.record_checkpoint(
                            &acb.thread_id,
                            format!("ckpt_before_final_{}", new_id("y_")),
                        )?;
                        self.complete_with_final(&acb, &artifacts, &tool_results, submission)?;
                        final_submitted = true;
                    }
                }
            }
            if !turn_had_completion_action {
                consecutive_no_action_turns += 1;
                tool_results.push(runtime_feedback_record(
                    step_index,
                    consecutive_no_action_turns,
                    &output_texts,
                ));
                self.kernel.record_checkpoint(
                    &acb.thread_id,
                    format!("ckpt_after_runtime_feedback_{}", new_id("y_")),
                )?;
            } else {
                consecutive_no_action_turns = 0;
            }
            self.kernel
                .record_provider_usage(&stream.session_id, response.usage)?;
            self.kernel.complete_stream_session(&stream.session_id)?;
            if consecutive_no_action_turns >= MAX_CONSECUTIVE_NO_ACTION_TURNS {
                return self.block_without_final(
                    &acb,
                    format!("runtime received {MAX_CONSECUTIVE_NO_ACTION_TURNS} consecutive model turns with no tool call or final submission"),
                    provider_stream_session_ids,
                    tool_results,
                    artifacts,
                );
            }
            if final_submitted {
                break;
            }
            acb = self.acb()?;
        }
        if !final_submitted {
            return self.block_without_final(
                &acb,
                "runtime reached max_steps without final submission".to_string(),
                provider_stream_session_ids,
                tool_results,
                artifacts,
            );
        }
        let state = self.kernel.state_snapshot()?;
        let acb = state
            .threads
            .get(&self.thread_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {}", self.thread_id)))?;
        Ok(RuntimeRunReport {
            thread_id: self.thread_id.clone(),
            task_id: acb.task.task_id,
            status: acb.status,
            provider_stream_session_ids,
            tool_results,
            artifacts,
            final_submitted,
            events: self.kernel.events()?.len(),
        })
    }

    fn block_without_final(
        &self,
        acb: &AgentControlBlock,
        reason: String,
        provider_stream_session_ids: Vec<String>,
        tool_results: Vec<ToolExecutionRecord>,
        artifacts: Vec<ArtifactRecord>,
    ) -> AgentOsResult<RuntimeRunReport> {
        self.kernel.update_task(UpdateTaskInput {
            task_id: acb.task.task_id.clone(),
            status: Some(TaskStatus::Blocked),
            blocked_reason: Some(reason.clone()),
            owner_agent_id: Some(acb.agent_id.clone()),
            title: None,
            description: None,
            checklist: None,
        })?;
        self.kernel
            .transition_thread(&acb.thread_id, ThreadStatus::Blocked, Some(reason))?;
        self.kernel.record_checkpoint(
            &acb.thread_id,
            format!("ckpt_after_runtime_blocked_{}", new_id("y_")),
        )?;
        let state = self.kernel.state_snapshot()?;
        let acb = state
            .threads
            .get(&self.thread_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {}", self.thread_id)))?;
        Ok(RuntimeRunReport {
            thread_id: self.thread_id.clone(),
            task_id: acb.task.task_id,
            status: acb.status,
            provider_stream_session_ids,
            tool_results,
            artifacts,
            final_submitted: false,
            events: self.kernel.events()?.len(),
        })
    }

    fn execute_tool_action(
        &self,
        acb: &AgentControlBlock,
        stream_session_id: &str,
        capability_id: &str,
        action: ToolAction,
    ) -> AgentOsResult<ToolExecutionRecord> {
        self.kernel.record_provider_stream_event(
            stream_session_id,
            ProviderStreamEventType::ToolCallProposed,
            json!({
                "tool_name": action.tool_name,
                "input": action.input,
                "risk_level": action.risk_level,
                "evidence_claim": action.evidence_claim,
            }),
        )?;
        self.kernel.record_tool_proposal(
            &acb.agent_id,
            &acb.task.task_id,
            ToolInvokeInput {
                tool_name: action.tool_name.clone(),
                input: action.input.clone(),
                evidence_claim: action.evidence_claim.clone(),
            },
            action.risk_level,
        )?;
        let invocation = self.kernel.invoke_tool(
            &acb.agent_id,
            &acb.task.task_id,
            &acb.session_id,
            capability_id.to_string(),
            action.risk_level,
            ToolInvokeInput {
                tool_name: action.tool_name,
                input: action.input,
                evidence_claim: action.evidence_claim.clone(),
            },
        )?;
        self.kernel.record_provider_stream_event(
            stream_session_id,
            ProviderStreamEventType::ToolCallCompleted,
            json!({
                "tool_call_id": invocation.call_id,
                "tool_name": invocation.tool_name,
                "status": invocation.status,
                "evidence_ids": invocation.evidence_ids,
            }),
        )?;
        Ok(ToolExecutionRecord {
            call_id: invocation.call_id,
            tool_name: invocation.tool_name,
            status: invocation.status,
            input: Some(invocation.input),
            output: invocation.output,
            evidence_ids: invocation.evidence_ids,
            evidence_claim: action.evidence_claim,
        })
    }

    fn commit_patch_artifact_for_tool(
        &self,
        acb: &AgentControlBlock,
        environment_id: &str,
        environment_lease_id: &str,
        record: &ToolExecutionRecord,
    ) -> AgentOsResult<Option<ArtifactRecord>> {
        if record.status != ToolCallStatus::Completed || record.evidence_ids.is_empty() {
            return Ok(None);
        }
        let Some(output) = &record.output else {
            return Ok(None);
        };
        let path = match record.tool_name.as_str() {
            "write_file" => output.get("written_path").and_then(Value::as_str),
            "replace_text" => output.get("changed_path").and_then(Value::as_str),
            "delete_file" => output.get("deleted_path").and_then(Value::as_str),
            _ => None,
        };
        let Some(path) = path else {
            return Ok(None);
        };
        let artifact = self.kernel.commit_artifact(CommitArtifactInput {
            goal_id: acb.task.goal_id.clone(),
            task_id: acb.task.task_id.clone(),
            owner_agent_id: acb.agent_id.clone(),
            artifact_type: ArtifactType::Patch,
            blob_ref: Some(path.to_string()),
            content_hash: None,
            inline_bytes: None,
            metadata: json!({
                "tool_call_id": record.call_id,
                "tool_name": record.tool_name,
                "environment_id": environment_id,
                "environment_lease_id": environment_lease_id,
                "output": output,
            }),
            evidence_ids: record.evidence_ids.clone(),
            supersedes: None,
        })?;
        Ok(Some(ArtifactRecord {
            artifact_id: artifact.artifact_id,
            artifact_type: artifact.artifact_type,
            blob_ref: artifact.blob_ref,
            evidence_ids: record.evidence_ids.clone(),
        }))
    }

    fn complete_with_final(
        &self,
        acb: &AgentControlBlock,
        artifacts: &[ArtifactRecord],
        tool_results: &[ToolExecutionRecord],
        submission: FinalSubmission,
    ) -> AgentOsResult<()> {
        self.kernel.complete_task(CompleteTaskInput {
            task_id: acb.task.task_id.clone(),
            artifact_ids: artifacts
                .iter()
                .map(|artifact| artifact.artifact_id.clone())
                .collect(),
            evidence_ids: tool_results
                .iter()
                .flat_map(|result| result.evidence_ids.clone())
                .collect(),
        })?;
        self.kernel
            .submit_final(&acb.agent_id, &acb.task.task_id, submission)?;
        self.kernel.transition_thread(
            &acb.thread_id,
            ThreadStatus::Completed,
            Some("runtime final submitted".to_string()),
        )?;
        self.kernel
            .record_checkpoint(&acb.thread_id, format!("ckpt_after_task_{}", new_id("y_")))?;
        Ok(())
    }

    fn open_stream_session(
        &self,
        acb: &AgentControlBlock,
        step_index: u32,
        config: &RuntimeConfig,
    ) -> AgentOsResult<ProviderStreamSession> {
        let stream = self.kernel.open_stream_session(StreamRequest {
            thread_id: acb.thread_id.clone(),
            turn_id: acb.active_turn.turn_id.clone(),
            provider_profile_id: acb.config_snapshot.provider_profile_id.clone(),
            model_routing_policy_id: acb.config_snapshot.model_routing_policy_id.clone(),
            requested_model_alias: config.requested_model_alias.clone(),
            role: acb.role.clone(),
            task_id: acb.task.task_id.clone(),
            reasoning_profile: acb.config_snapshot.reasoning_profile.clone(),
            tool_visibility_profile: None,
            output_schema: None,
        })?;
        self.kernel.record_provider_stream_event(
            &stream.session_id,
            ProviderStreamEventType::ReasoningStarted,
            json!({ "step_index": step_index }),
        )?;
        Ok(stream)
    }

    fn acb(&self) -> AgentOsResult<AgentControlBlock> {
        self.kernel
            .state_snapshot()?
            .threads
            .get(&self.thread_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {}", self.thread_id)))
    }

    fn hydrated_tool_results(&self, task_id: &str) -> AgentOsResult<Vec<ToolExecutionRecord>> {
        let state = self.kernel.state_snapshot()?;
        let mut invocations: Vec<_> = state
            .tool_invocations
            .values()
            .filter(|invocation| {
                invocation.task_id == task_id
                    && matches!(
                        invocation.status,
                        ToolCallStatus::Completed
                            | ToolCallStatus::Failed
                            | ToolCallStatus::Denied
                            | ToolCallStatus::Cancelled
                            | ToolCallStatus::TimedOut
                    )
            })
            .cloned()
            .collect();
        invocations.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.call_id.cmp(&right.call_id))
        });
        Ok(invocations
            .into_iter()
            .map(|invocation| {
                let evidence_claim = invocation
                    .evidence_ids
                    .iter()
                    .find_map(|evidence_id| state.evidence.get(evidence_id))
                    .and_then(|evidence| evidence.claim.clone());
                ToolExecutionRecord {
                    call_id: invocation.call_id,
                    tool_name: invocation.tool_name,
                    status: invocation.status,
                    input: Some(invocation.input),
                    output: invocation.output,
                    evidence_ids: invocation.evidence_ids,
                    evidence_claim,
                }
            })
            .collect())
    }

    fn hydrated_artifacts(&self, task_id: &str) -> AgentOsResult<Vec<ArtifactRecord>> {
        let state = self.kernel.state_snapshot()?;
        let mut artifacts: Vec<_> = state
            .artifacts
            .values()
            .filter(|artifact| artifact.task_id == task_id)
            .cloned()
            .collect();
        artifacts.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.artifact_id.cmp(&right.artifact_id))
        });
        Ok(artifacts
            .into_iter()
            .map(|artifact| {
                let mut evidence_ids: Vec<String> = artifact
                    .metadata
                    .get("evidence_ids")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect();
                evidence_ids.extend(
                    state
                        .evidence
                        .values()
                        .filter(|evidence| {
                            evidence.artifact_id.as_deref() == Some(&artifact.artifact_id)
                        })
                        .map(|evidence| evidence.evidence_id.clone()),
                );
                evidence_ids.sort();
                evidence_ids.dedup();
                ArtifactRecord {
                    artifact_id: artifact.artifact_id,
                    artifact_type: artifact.artifact_type,
                    blob_ref: artifact.blob_ref,
                    evidence_ids,
                }
            })
            .collect())
    }
}

fn project_tool_results(tool_results: &[ToolExecutionRecord]) -> Vec<ToolExecutionRecord> {
    let recent_start = tool_results
        .len()
        .saturating_sub(MAX_PROJECTED_TOOL_RESULTS);
    tool_results
        .iter()
        .enumerate()
        .filter(|(index, result)| *index >= recent_start || !result.evidence_ids.is_empty())
        .map(|(index, result)| {
            if index >= recent_start {
                result.clone()
            } else {
                compact_tool_result(result)
            }
        })
        .collect()
}

fn compact_tool_result(result: &ToolExecutionRecord) -> ToolExecutionRecord {
    let mut compacted = result.clone();
    if let Some(output) = &result.output {
        let (mut value, truncated) =
            compact_json_value(output, MAX_PROJECTED_OLDER_TOOL_STRING_CHARS);
        if truncated {
            if let Value::Object(map) = &mut value {
                map.insert("projection_truncated".to_string(), Value::Bool(true));
                map.insert(
                    "projection_note".to_string(),
                    Value::String(
                        "Older evidence output was truncated for projection; rerun a narrower command if exact omitted content is needed."
                            .to_string(),
                    ),
                );
            }
        }
        compacted.output = Some(value);
    }
    compacted
}

fn compact_json_value(value: &Value, max_string_chars: usize) -> (Value, bool) {
    match value {
        Value::String(text) if text.chars().count() > max_string_chars => {
            let prefix = text.chars().take(max_string_chars).collect::<String>();
            let omitted = text.chars().count().saturating_sub(max_string_chars);
            (
                Value::String(format!(
                    "{prefix}\n...[truncated for projection: {omitted} chars omitted]"
                )),
                true,
            )
        }
        Value::String(_) | Value::Null | Value::Bool(_) | Value::Number(_) => {
            (value.clone(), false)
        }
        Value::Array(items) => {
            let mut truncated = false;
            let values = items
                .iter()
                .map(|item| {
                    let (value, item_truncated) = compact_json_value(item, max_string_chars);
                    truncated |= item_truncated;
                    value
                })
                .collect();
            (Value::Array(values), truncated)
        }
        Value::Object(map) => {
            let mut truncated = false;
            let values = map
                .iter()
                .map(|(key, item)| {
                    let (value, item_truncated) = compact_json_value(item, max_string_chars);
                    truncated |= item_truncated;
                    (key.clone(), value)
                })
                .collect();
            (Value::Object(values), truncated)
        }
    }
}

fn runtime_feedback_record(
    step_index: u32,
    consecutive_no_action_turns: u32,
    output_texts: &[String],
) -> ToolExecutionRecord {
    let text = output_texts.join("\n\n");
    let text_excerpt = text.chars().take(1200).collect::<String>();
    ToolExecutionRecord {
        call_id: new_id("feedback_"),
        tool_name: RUNTIME_FEEDBACK_TOOL.to_string(),
        status: ToolCallStatus::Failed,
        input: Some(json!({
            "step_index": step_index,
            "consecutive_no_action_turns": consecutive_no_action_turns
        })),
        output: Some(json!({
            "message": "The previous model response had no tool call or final submission. On the next turn, call exactly one available tool or call submit_final if the task is complete or blocked with evidence.",
            "max_consecutive_no_action_turns": MAX_CONSECUTIVE_NO_ACTION_TURNS,
            "text_excerpt": text_excerpt,
        })),
        evidence_ids: Vec::new(),
        evidence_claim: None,
    }
}

#[cfg(test)]
#[path = "runtime/tests.rs"]
mod tests;
