use crate::{
    ArtifactRecord, ModelAction, ModelClient, ModelContextProjection, ModelTurnRequest, ToolAction,
    ToolExecutionRecord,
};
use agent_os_kernel::{
    CommitArtifactInput, CompactContextInput, CompleteTaskInput, Kernel, ToolInvokeInput,
    UpdateTaskInput,
};
use agent_os_sys::*;
use serde_json::{json, Value};

#[path = "runtime/context_projection.rs"]
mod context_projection;
#[path = "runtime/ecosystem_projection.rs"]
mod ecosystem_projection;
#[path = "runtime/feedback.rs"]
mod feedback;
#[path = "runtime/job.rs"]
mod job;
#[path = "runtime/report.rs"]
mod report;
#[path = "runtime/tool_policy.rs"]
mod tool_policy;

use context_projection::{project_tool_results, prune_context_for_model_limit};
use feedback::*;
pub use job::{RuntimeJob, RuntimeJobRecord, RuntimeJobStatus};
pub use report::{RuntimeConfig, RuntimeRunOverrides, RuntimeRunReport};

const MAX_PROJECTED_ARTIFACTS: usize = 8;
const RUNTIME_GRANT_RESOURCE_SCOPE_CANDIDATES: &[&str] = &[
    "tool:*",
    "process:*",
    "instruction:*",
    "skill:*",
    "skill_file:*",
    "mcp:*",
];

pub struct ThreadRuntime<C> {
    kernel: Kernel,
    thread_id: String,
    model_client: C,
    job: Option<RuntimeJob>,
}

impl<C: ModelClient> ThreadRuntime<C> {
    pub fn new(kernel: Kernel, thread_id: impl Into<String>, model_client: C) -> Self {
        Self {
            kernel,
            thread_id: thread_id.into(),
            model_client,
            job: None,
        }
    }

    pub fn new_for_job(kernel: Kernel, job: RuntimeJob, model_client: C) -> Self {
        Self {
            kernel,
            thread_id: job.agent_thread_id.clone(),
            model_client,
            job: Some(job),
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
        let acb = self.acb()?;
        self.kernel.update_task(UpdateTaskInput {
            task_id: acb.task.task_id.clone(),
            status: Some(TaskStatus::Running),
            blocked_reason: None,
            owner_agent_id: Some(acb.agent_id.clone()),
            title: None,
            description: None,
            checklist: None,
        })?;
        let acb = self.kernel.start_turn(&self.thread_id)?;
        let job = RuntimeJob::from_active_turn(&acb)?;
        self.run_active_job_to_completion(job, acb, config, overrides)
    }

    pub fn run_job_to_completion(
        &mut self,
        config: RuntimeConfig,
    ) -> AgentOsResult<RuntimeRunReport> {
        self.run_job_to_completion_with_overrides(config, RuntimeRunOverrides::default())
    }

    pub fn run_job_to_completion_with_overrides(
        &mut self,
        config: RuntimeConfig,
        overrides: RuntimeRunOverrides,
    ) -> AgentOsResult<RuntimeRunReport> {
        let job = self.job.clone().ok_or_else(|| {
            AgentOsError::Validation("run_job_to_completion requires RuntimeJob".to_string())
        })?;
        let acb = self.acb()?;
        if let Some(interrupted) = self.runtime_job_interrupted(&job)? {
            return self.interrupted_report(&interrupted, Vec::new(), Vec::new(), Vec::new());
        }
        self.validate_runtime_job(&job, &acb, &config)?;
        self.kernel.update_task(UpdateTaskInput {
            task_id: acb.task.task_id.clone(),
            status: Some(TaskStatus::Running),
            blocked_reason: None,
            owner_agent_id: Some(acb.agent_id.clone()),
            title: None,
            description: None,
            checklist: None,
        })?;
        self.run_active_job_to_completion(job, acb, config, overrides)
    }

    fn run_active_job_to_completion(
        &mut self,
        job: RuntimeJob,
        mut acb: AgentControlBlock,
        config: RuntimeConfig,
        overrides: RuntimeRunOverrides,
    ) -> AgentOsResult<RuntimeRunReport> {
        self.validate_runtime_job(&job, &acb, &config)?;
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
            runtime_grant_resource_scopes(&acb.effective_permissions_snapshot),
            config.tool_risk_ceiling,
            overrides.tool_approval_id.clone(),
        )?;

        let mut provider_stream_session_ids = Vec::new();
        let mut tool_results = self.hydrated_tool_results(&acb.task.task_id)?;
        let mut artifacts = self.hydrated_artifacts(&acb.task.task_id)?;
        let mut final_submitted = false;
        let mut consecutive_no_action_turns = 0;
        let mut repeated_tool_call = RepeatedToolCallTracker::default();
        let mut finalization_feedback_sent = false;
        let mut pre_patch_resolution_feedback_sent = false;

        for step_index in 0..config.max_steps {
            if let Some(interrupted) = self.runtime_job_interrupted(&job)? {
                return self.interrupted_report(
                    &interrupted,
                    provider_stream_session_ids,
                    tool_results,
                    artifacts,
                );
            }
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
            let mementos = self
                .kernel
                .visible_mementos_for_thread(&acb.thread_id, &acb.thread_id)?;
            let mut thread_forks: Vec<_> = state
                .thread_forks
                .values()
                .filter(|fork| {
                    fork.source_thread_id == acb.thread_id || fork.forked_thread_id == acb.thread_id
                })
                .cloned()
                .collect();
            thread_forks.sort_by(|left, right| {
                left.created_at
                    .cmp(&right.created_at)
                    .then_with(|| left.fork_id.cmp(&right.fork_id))
            });
            let mut thread_rollbacks: Vec<_> = state
                .thread_rollbacks
                .values()
                .filter(|rollback| rollback.thread_id == acb.thread_id)
                .cloned()
                .collect();
            thread_rollbacks.sort_by(|left, right| {
                left.created_at
                    .cmp(&right.created_at)
                    .then_with(|| left.rollback_id.cmp(&right.rollback_id))
            });
            let mut ecosystem_projection = ecosystem_projection::from_state(&state);
            let tool_planning_mode = if finalization_feedback_sent {
                ToolPlanningMode::FinalizationOnly
            } else if pre_patch_resolution_feedback_sent
                && should_enforce_pre_patch_resolution_gate(&tool_results, &artifacts)
            {
                ToolPlanningMode::PrePatchResolution
            } else {
                ToolPlanningMode::Normal
            };
            let mut tool_plan = self.kernel.plan_tools_for_turn(
                &acb,
                stream.route_decision.model_capabilities.clone(),
                tool_planning_mode,
            )?;
            expose_tool_search_matches(&mut tool_plan, &projected_tool_results);
            ecosystem_projection.tool_descriptors = tool_plan.direct_descriptors();
            let provider_profile = state
                .provider_profiles
                .get(&acb.config_snapshot.provider_profile_id)
                .ok_or_else(|| {
                    AgentOsError::NotFound(format!(
                        "provider profile {}",
                        acb.config_snapshot.provider_profile_id
                    ))
                })?;
            let mut context = ModelContextProjection {
                tool_results: projected_tool_results,
                artifacts: projected_artifacts,
                context_snapshots,
                memory_records,
                context_compactions,
                mementos,
                thread_forks,
                thread_rollbacks,
                tool_plan,
                tool_descriptors: ecosystem_projection.tool_descriptors,
                instruction_documents: ecosystem_projection.instruction_documents,
                skill_definitions: ecosystem_projection.skill_definitions,
                command_definitions: ecosystem_projection.command_definitions,
                mcp_tools: ecosystem_projection.mcp_tools,
                mcp_resources: ecosystem_projection.mcp_resources,
                mcp_resource_templates: ecosystem_projection.mcp_resource_templates,
                imported_agent_profiles: ecosystem_projection.imported_agent_profiles,
            };
            let context_budget =
                prune_context_for_model_limit(&mut context, &stream.route_decision.model_limit);
            if context_budget.pruned() {
                let compaction = self.kernel.compact_context(CompactContextInput {
                    thread_id: acb.thread_id.clone(),
                    agent_id: acb.agent_id.clone(),
                    task_id: acb.task.task_id.clone(),
                    summary_artifact_id: None,
                    superseded_refs: context_budget.pruned_refs.clone(),
                    token_estimate: context_budget.before_tokens,
                })?;
                self.kernel.record_provider_stream_event(
                    &stream.session_id,
                    ProviderStreamEventType::ProviderWarning,
                    json!({
                        "type": "context_pruned",
                        "compaction_id": compaction.compaction_id,
                        "before_tokens": context_budget.before_tokens,
                        "after_tokens": context_budget.after_tokens,
                        "usable_input_tokens": context_budget.usable_input_tokens,
                        "pruned_refs": context_budget.pruned_refs
                    }),
                )?;
            }
            if context_budget.over_budget_after_prune {
                let message = format!(
                    "model context projection remains over budget after prune: estimated={} usable={}",
                    context_budget.after_tokens, context_budget.usable_input_tokens
                );
                self.kernel
                    .fail_stream_session(&stream.session_id, message.clone())?;
                return Err(AgentOsError::BudgetExhausted(message));
            }
            let request = ModelTurnRequest {
                thread: acb.clone(),
                workspace_root: config.workspace_root.clone(),
                step_index,
                model_capabilities: stream.route_decision.model_capabilities.clone(),
                context,
            };
            let retry_policy =
                RuntimeRetryPolicy::from_json(provider_profile.retry_policy.as_ref());
            let mut attempt = 1;
            let response = loop {
                match self.model_client.next(&request) {
                    Ok(response) => break response,
                    Err(error) => {
                        let retry_after_ms = retry_after_ms_from_error(&error);
                        if !retry_policy.should_retry(&error)
                            || attempt >= retry_policy.max_attempts
                        {
                            self.kernel
                                .fail_stream_session(&stream.session_id, error.to_string())?;
                            return Err(error);
                        }
                        self.kernel.record_provider_stream_event(
                            &stream.session_id,
                            ProviderStreamEventType::ProviderRetry,
                            json!({
                                "attempt": attempt,
                                "max_attempts": retry_policy.max_attempts,
                                "retry_after_ms": retry_after_ms,
                                "error": error.to_string()
                            }),
                        )?;
                        let sleep_ms = retry_after_ms
                            .unwrap_or_else(|| retry_policy.backoff_ms_for_attempt(attempt));
                        if sleep_ms > 0 {
                            std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
                        }
                        attempt += 1;
                    }
                }
            };
            let mut turn_had_completion_action = false;
            let mut duplicate_block_reason = None;
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
                        if finalization_feedback_sent && !is_finalization_allowed_tool_call(&action)
                        {
                            tool_results
                                .push(finalization_gate_feedback_record(step_index, &action));
                            self.kernel.record_checkpoint(
                                &acb.thread_id,
                                format!("ckpt_after_finalization_gate_{}", new_id("y_")),
                            )?;
                            continue;
                        }
                        if pre_patch_resolution_feedback_sent
                            && should_enforce_pre_patch_resolution_gate(&tool_results, &artifacts)
                            && !is_pre_patch_resolution_allowed_tool_call(&action)
                        {
                            tool_results.push(pre_patch_resolution_gate_feedback_record(
                                step_index, &action,
                            ));
                            self.kernel.record_checkpoint(
                                &acb.thread_id,
                                format!("ckpt_after_pre_patch_resolution_gate_{}", new_id("y_")),
                            )?;
                            continue;
                        }
                        if action.tool_name == "read_image"
                            && !request.model_capabilities.image_input
                        {
                            tool_results
                                .push(unsupported_image_input_tool_record(step_index, &action));
                            self.kernel.record_checkpoint(
                                &acb.thread_id,
                                format!("ckpt_after_model_capability_feedback_{}", new_id("y_")),
                            )?;
                            continue;
                        }
                        if !request
                            .context
                            .tool_descriptors
                            .iter()
                            .any(|descriptor| descriptor.name == action.tool_name)
                        {
                            let visible_tool_names = request
                                .context
                                .tool_descriptors
                                .iter()
                                .map(|descriptor| descriptor.name.clone())
                                .collect();
                            tool_results.push(non_visible_tool_feedback_record(
                                step_index,
                                &action,
                                visible_tool_names,
                            ));
                            self.kernel.record_checkpoint(
                                &acb.thread_id,
                                format!("ckpt_after_tool_visibility_feedback_{}", new_id("y_")),
                            )?;
                            continue;
                        }
                        if should_guard_duplicate_tool_call(&action) {
                            let duplicate_count = repeated_tool_call.observe(&action);
                            if duplicate_count >= DUPLICATE_TOOL_WARNING_COUNT {
                                tool_results.push(duplicate_tool_feedback_record(
                                    step_index,
                                    duplicate_count,
                                    &action,
                                ));
                                self.kernel.record_checkpoint(
                                    &acb.thread_id,
                                    format!("ckpt_after_duplicate_tool_feedback_{}", new_id("y_")),
                                )?;
                                if duplicate_count >= MAX_CONSECUTIVE_IDENTICAL_TOOL_CALLS {
                                    duplicate_block_reason = Some(format!(
                                        "runtime received {duplicate_count} consecutive identical tool calls for `{}`",
                                        action.tool_name
                                    ));
                                }
                                continue;
                            }
                        } else {
                            repeated_tool_call.reset();
                        }
                        let record = self.execute_tool_action(
                            &acb,
                            &stream.session_id,
                            &capability.capability_id,
                            action,
                        )?;
                        if record.status == ToolCallStatus::Running {
                            let continue_with_model =
                                should_continue_with_running_tool_result(&record);
                            tool_results.push(record);
                            self.kernel.record_checkpoint(
                                &acb.thread_id,
                                format!("ckpt_after_tool_background_{}", new_id("y_")),
                            )?;
                            if continue_with_model {
                                break;
                            }
                            self.kernel.transition_thread(
                                &acb.thread_id,
                                ThreadStatus::WaitingTool,
                                Some("background tool is still running".to_string()),
                            )?;
                            let waiting = self.acb()?;
                            return Ok(RuntimeRunReport {
                                thread_id: self.thread_id.clone(),
                                task_id: waiting.task.task_id,
                                status: waiting.status,
                                provider_stream_session_ids,
                                tool_results,
                                artifacts,
                                final_submitted: false,
                                events: self.kernel.events()?.len(),
                            });
                        }
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
                        let submit_final_completed = record.tool_name == "submit_final"
                            && record.status == ToolCallStatus::Completed;
                        tool_results.push(record);
                        // Yield boundary: after tool result.
                        self.kernel.record_checkpoint(
                            &acb.thread_id,
                            format!("ckpt_after_tool_{}", new_id("y_")),
                        )?;
                        if submit_final_completed {
                            self.complete_after_submit_final_tool(&acb, &artifacts, &tool_results)?;
                            final_submitted = true;
                            break;
                        }
                    }
                    ModelAction::Final { submission } => {
                        turn_had_completion_action = true;
                        repeated_tool_call.reset();
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
            if !final_submitted
                && !finalization_feedback_sent
                && should_project_finalization_feedback(&tool_results, &artifacts)
            {
                finalization_feedback_sent = true;
                let remaining_steps = config.max_steps.saturating_sub(step_index + 1);
                tool_results.push(finalization_feedback_record(
                    step_index,
                    remaining_steps,
                    artifacts.len(),
                ));
                self.kernel.record_checkpoint(
                    &acb.thread_id,
                    format!("ckpt_after_finalization_feedback_{}", new_id("y_")),
                )?;
            }
            if !final_submitted
                && !finalization_feedback_sent
                && !pre_patch_resolution_feedback_sent
                && should_project_pre_patch_resolution_feedback(&tool_results, &artifacts)
            {
                pre_patch_resolution_feedback_sent = true;
                let investigation_tool_results =
                    count_pre_patch_investigation_tool_results(&tool_results);
                tool_results.push(pre_patch_resolution_feedback_record(
                    step_index,
                    investigation_tool_results,
                ));
                self.kernel.record_checkpoint(
                    &acb.thread_id,
                    format!("ckpt_after_pre_patch_resolution_feedback_{}", new_id("y_")),
                )?;
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
            if !final_submitted {
                if let Some(reason) = duplicate_block_reason {
                    return self.block_without_final(
                        &acb,
                        reason,
                        provider_stream_session_ids,
                        tool_results,
                        artifacts,
                    );
                }
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

    fn validate_runtime_job(
        &self,
        job: &RuntimeJob,
        acb: &AgentControlBlock,
        config: &RuntimeConfig,
    ) -> AgentOsResult<()> {
        if job.agent_thread_id != acb.thread_id || job.client_thread_id != acb.thread_id {
            return Err(AgentOsError::Validation(
                "RuntimeJob thread ids do not match active thread".to_string(),
            ));
        }
        if acb.active_turn.turn_id.as_deref() != Some(&job.turn_id) {
            return Err(AgentOsError::Validation(format!(
                "RuntimeJob turn {} is not active on thread {}",
                job.turn_id, acb.thread_id
            )));
        }
        if acb.active_turn.status != Some(TurnStatus::InProgress) {
            return Err(AgentOsError::InvalidTransition(format!(
                "RuntimeJob turn {} is not InProgress",
                job.turn_id
            )));
        }
        if config.workspace_root.as_os_str() != std::ffi::OsStr::new(&job.workspace) {
            return Err(AgentOsError::Validation(
                "RuntimeJob workspace does not match RuntimeConfig workspace".to_string(),
            ));
        }
        if job.provider_profile != acb.config_snapshot.provider_profile_id {
            return Err(AgentOsError::Validation(
                "RuntimeJob provider profile does not match thread binding".to_string(),
            ));
        }
        if job.model != acb.config_snapshot.model_id {
            return Err(AgentOsError::Validation(
                "RuntimeJob model does not match thread binding".to_string(),
            ));
        }
        Ok(())
    }

    fn runtime_job_interrupted(
        &self,
        job: &RuntimeJob,
    ) -> AgentOsResult<Option<AgentControlBlock>> {
        let acb = self.acb()?;
        if acb.active_turn.turn_id.as_deref() != Some(&job.turn_id) {
            return Ok(None);
        }
        if acb.status == ThreadStatus::Interrupted
            || acb.active_turn.status == Some(TurnStatus::Interrupted)
        {
            return Ok(Some(acb));
        }
        Ok(None)
    }

    fn interrupted_report(
        &self,
        acb: &AgentControlBlock,
        provider_stream_session_ids: Vec<String>,
        tool_results: Vec<ToolExecutionRecord>,
        artifacts: Vec<ArtifactRecord>,
    ) -> AgentOsResult<RuntimeRunReport> {
        Ok(RuntimeRunReport {
            thread_id: self.thread_id.clone(),
            task_id: acb.task.task_id.clone(),
            status: acb.status,
            provider_stream_session_ids,
            tool_results,
            artifacts,
            final_submitted: false,
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
        let next_status = if acb.status == ThreadStatus::Completing {
            ThreadStatus::Failed
        } else {
            ThreadStatus::Blocked
        };
        self.kernel
            .transition_thread(&acb.thread_id, next_status, Some(reason))?;
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
            "apply_patch" => output.get("path").and_then(Value::as_str),
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

    fn complete_after_submit_final_tool(
        &self,
        acb: &AgentControlBlock,
        artifacts: &[ArtifactRecord],
        tool_results: &[ToolExecutionRecord],
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

fn expose_tool_search_matches(tool_plan: &mut ToolPlan, tool_results: &[ToolExecutionRecord]) {
    let activated = tool_results
        .iter()
        .filter(|record| {
            record.tool_name == "tool_search" && record.status == ToolCallStatus::Completed
        })
        .filter_map(|record| record.output.as_ref())
        .filter_map(|output| output.get("matches"))
        .filter_map(Value::as_array)
        .flat_map(|matches| matches.iter())
        .filter_map(|item| item.get("name"))
        .filter_map(Value::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    if activated.is_empty() {
        return;
    }
    for entry in &mut tool_plan.entries {
        if entry.exposure == ToolExposure::Deferred
            && activated.contains(entry.descriptor.name.as_str())
        {
            entry.exposure = ToolExposure::Direct;
            entry.reason = Some("exposed after tool_search match".to_string());
        }
    }
}

fn should_continue_with_running_tool_result(record: &ToolExecutionRecord) -> bool {
    record.tool_name == "run_command"
        && record
            .output
            .as_ref()
            .and_then(|output| output.get("stdin_mode"))
            .and_then(Value::as_str)
            == Some("piped")
        && record
            .output
            .as_ref()
            .and_then(|output| output.get("process_id"))
            .and_then(Value::as_str)
            .is_some_and(|process_id| !process_id.is_empty())
}

fn runtime_grant_resource_scopes(permissions: &PermissionSet) -> Vec<String> {
    RUNTIME_GRANT_RESOURCE_SCOPE_CANDIDATES
        .iter()
        .filter(|scope| {
            permissions
                .resource_scopes
                .iter()
                .any(|allowed| scope_pattern_allows(allowed, scope))
        })
        .map(|scope| (*scope).to_string())
        .collect()
}

fn scope_pattern_allows(allowed: &str, requested: &str) -> bool {
    if allowed == "*" || allowed == requested {
        return true;
    }
    allowed.strip_suffix(":*").is_some_and(|prefix| {
        requested == prefix
            || requested
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with(':'))
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeRetryPolicy {
    max_attempts: u64,
    initial_backoff_ms: u64,
    max_backoff_ms: u64,
}

impl RuntimeRetryPolicy {
    fn from_json(value: Option<&Value>) -> Self {
        let max_attempts = value
            .and_then(|policy| policy.get("max_attempts"))
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .max(1);
        let initial_backoff_ms = value
            .and_then(|policy| policy.get("initial_backoff_ms"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let max_backoff_ms = value
            .and_then(|policy| policy.get("max_backoff_ms"))
            .and_then(Value::as_u64)
            .unwrap_or(initial_backoff_ms)
            .max(initial_backoff_ms);
        Self {
            max_attempts,
            initial_backoff_ms,
            max_backoff_ms,
        }
    }

    fn should_retry(&self, error: &AgentOsError) -> bool {
        match error {
            AgentOsError::ResourceConflict(message)
            | AgentOsError::Validation(message)
            | AgentOsError::Serialization(message) => message.contains("retryable=true"),
            _ => false,
        }
    }

    fn backoff_ms_for_attempt(&self, attempt: u64) -> u64 {
        if self.initial_backoff_ms == 0 {
            return 0;
        }
        let multiplier = 1_u64
            .checked_shl((attempt.saturating_sub(1)) as u32)
            .unwrap_or(u64::MAX);
        self.initial_backoff_ms
            .saturating_mul(multiplier)
            .min(self.max_backoff_ms)
    }
}

fn retry_after_ms_from_error(error: &AgentOsError) -> Option<u64> {
    let message = error.to_string();
    let marker = "retry_after_ms=";
    let start = message.find(marker)? + marker.len();
    let digits = message[start..]
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    digits.parse::<u64>().ok().filter(|value| *value > 0)
}

#[cfg(test)]
#[path = "runtime/tests.rs"]
mod tests;
