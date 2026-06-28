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

const MAX_PROJECTED_TOOL_RESULTS: usize = 8;
const MAX_PROJECTED_ARTIFACTS: usize = 8;

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
            vec!["tool:*".to_string()],
            config.tool_risk_ceiling,
            overrides.tool_approval_id.clone(),
        )?;

        let mut provider_stream_session_ids = Vec::new();
        let mut tool_results = self.hydrated_tool_results(&acb.task.task_id)?;
        let mut artifacts = self.hydrated_artifacts(&acb.task.task_id)?;
        let mut final_submitted = false;

        for step_index in 0..config.max_steps {
            // Yield boundary: before model call.
            self.kernel.record_checkpoint(
                &acb.thread_id,
                format!("ckpt_before_model_{}", new_id("y_")),
            )?;
            let stream = self.open_stream_session(&acb, step_index, &config)?;
            provider_stream_session_ids.push(stream.session_id.clone());
            let state = self.kernel.state_snapshot()?;
            let tool_result_recent_start = tool_results
                .len()
                .saturating_sub(MAX_PROJECTED_TOOL_RESULTS);
            let projected_tool_results = tool_results
                .iter()
                .enumerate()
                .filter(|(index, result)| {
                    *index >= tool_result_recent_start || !result.evidence_ids.is_empty()
                })
                .map(|(_, result)| result.clone())
                .collect();
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
            for action in response.actions {
                match action {
                    ModelAction::OutputText { text } => {
                        self.kernel.record_provider_stream_event(
                            &stream.session_id,
                            ProviderStreamEventType::OutputTextDelta,
                            json!({ "text": text }),
                        )?;
                    }
                    ModelAction::ToolCall(action) => {
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
                        enforce_tool_policy(&record, &config)?;
                        tool_results.push(record);
                        // Yield boundary: after tool result.
                        self.kernel.record_checkpoint(
                            &acb.thread_id,
                            format!("ckpt_after_tool_{}", new_id("y_")),
                        )?;
                    }
                    ModelAction::Final { submission } => {
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
            self.kernel
                .record_provider_usage(&stream.session_id, response.usage)?;
            self.kernel.complete_stream_session(&stream.session_id)?;
            if final_submitted {
                break;
            }
            acb = self.acb()?;
        }
        if !final_submitted {
            return Err(AgentOsError::Validation(
                "runtime reached max_steps without final submission".to_string(),
            ));
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
                invocation.task_id == task_id && invocation.status == ToolCallStatus::Completed
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

fn enforce_tool_policy(record: &ToolExecutionRecord, config: &RuntimeConfig) -> AgentOsResult<()> {
    if config.fail_on_process_nonzero && record.tool_name == "run_command" {
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
    }
    Ok(())
}

#[cfg(test)]
#[path = "runtime/tests.rs"]
mod tests;
