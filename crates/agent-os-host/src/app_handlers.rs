use crate::AgentOsHost;
use agent_os_app_server::AppKernelService;
use agent_os_config::{ModelEntry, ProviderCatalog, ProviderEntry, ResolvedAgentOsConfig};
use agent_os_kernel::{
    CompactContextInput, ForkThreadInput, RecordApprovalInput, RegisterGoalInput,
    RollbackThreadInput, SpawnAgentInput, SpawnTaskInput,
};
use agent_os_sys::{
    AgentOsError, AgentOsResult, AppConfigProjection, AppConfigRecoveryProjection,
    AppCredentialProjection, AppEcosystemProjection, AppEcosystemSourceProjection,
    AppModelProjection, AppNotificationEnvelope, AppProjectProjection,
    AppProviderCapabilitiesProjection, AppProviderProjection, AppProviderUsageProjection,
    AppRequest, AppResponse, ApprovalStatus, ClientConnection, CreateAutomationScheduleInput,
    CredentialSource, EcosystemSource, OpenResourceSessionInput, PermissionProfile,
    ProjectionCursor, ResourceSessionType, StatsQuery, ThreadStatus, TurnInputKind,
};
use agent_os_thread::{RuntimeJob, RuntimeJobRecord};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;

impl AppKernelService for AgentOsHost {
    fn handle_app_request(
        &self,
        client: &ClientConnection,
        request: AppRequest,
    ) -> AgentOsResult<AppResponse> {
        match request {
            AppRequest::ThreadStart { goal, workspace } => {
                self.thread_start(client, goal, workspace)
            }
            AppRequest::ThreadResume { client_thread_id } => self.thread_resume(&client_thread_id),
            AppRequest::ThreadRead { client_thread_id } => self.thread_read(&client_thread_id),
            AppRequest::ThreadTurnsRead {
                client_thread_id,
                offset,
                limit,
            } => self.thread_turns_read(&client_thread_id, offset, limit),
            AppRequest::ThreadItemsRead {
                client_thread_id,
                offset,
                limit,
            } => self.thread_items_read(&client_thread_id, offset, limit),
            AppRequest::ThreadFork {
                client_thread_id,
                from_turn_id,
                title,
                goal,
            } => self.thread_fork(client, &client_thread_id, from_turn_id, title, goal),
            AppRequest::ThreadRollback {
                client_thread_id,
                target_turn_id,
                target_item_id,
                target_event_id,
                reason,
            } => self.thread_rollback(
                client,
                &client_thread_id,
                target_turn_id,
                target_item_id,
                target_event_id,
                reason,
            ),
            AppRequest::ThreadCompact {
                client_thread_id,
                summary_artifact_id,
                superseded_refs,
                token_estimate,
            } => self.thread_compact(
                &client_thread_id,
                summary_artifact_id,
                superseded_refs,
                token_estimate,
            ),
            AppRequest::ThreadList { archived } => self.thread_list(archived),
            AppRequest::ThreadSearch { query } => self.thread_search(&query),
            AppRequest::ThreadArchive { client_thread_id } => {
                self.thread_archive(&client_thread_id)
            }
            AppRequest::ThreadUnarchive { client_thread_id } => {
                self.thread_unarchive(&client_thread_id)
            }
            AppRequest::ThreadDelete { client_thread_id } => self.thread_delete(&client_thread_id),
            AppRequest::ThreadNameSet {
                client_thread_id,
                title,
            } => self.thread_name_set(&client_thread_id, title),
            AppRequest::TaskBundleExport { client_thread_id } => {
                self.task_bundle_export(&client_thread_id)
            }
            AppRequest::TurnStart {
                client_thread_id,
                input,
            } => self.turn_start(client, &client_thread_id, input),
            AppRequest::TurnSteer { turn_id, input } => self.turn_steer(client, &turn_id, input),
            AppRequest::TurnInterrupt { turn_id } => self.turn_interrupt(&turn_id),
            AppRequest::ApprovalRespond {
                approval_id,
                approved,
            } => self.approval_respond(client, approval_id, approved),
            AppRequest::ResourceSessionOpen {
                resource_type,
                client_thread_id,
                lease_expires_at,
                payload,
            } => self.resource_session_open(
                resource_type,
                client_thread_id,
                lease_expires_at,
                payload,
            ),
            AppRequest::ResourceSessionClose { session_id } => {
                self.resource_session_close(&session_id)
            }
            AppRequest::ProcessStop { process_id, reason } => {
                self.process_stop(client, process_id, reason)
            }
            AppRequest::ProcessKill { process_id, reason } => {
                self.process_kill(client, process_id, reason)
            }
            AppRequest::AutomationScheduleCreate {
                name,
                kind,
                target_thread_id,
                workspace,
                prompt,
                next_run_at,
                interval_seconds,
                payload,
            } => self.automation_schedule_create(CreateAutomationScheduleInput {
                name,
                kind,
                target_thread_id,
                workspace,
                prompt,
                next_run_at,
                interval_seconds,
                created_by_client_id: client.client_id.clone(),
                payload,
            }),
            AppRequest::AutomationScheduleList => self.automation_schedule_list(),
            AppRequest::AutomationRunList { schedule_id } => self.automation_run_list(schedule_id),
            AppRequest::StatsRead { query } => {
                accepted("snapshot", self.kernel().store().stats_snapshot(query)?)
            }
            AppRequest::ConfigRead { workspace } => self.config_read(workspace),
            AppRequest::ModelList { workspace } => self.model_list(workspace),
            AppRequest::ProviderCapabilitiesRead {
                workspace,
                provider_id,
            } => self.provider_capabilities_read(workspace, provider_id),
            AppRequest::ProviderUsageRead { query } => self.provider_usage_read(query),
            AppRequest::PermissionProfileList => self.permission_profile_list(),
            AppRequest::Initialize
            | AppRequest::Subscribe { .. }
            | AppRequest::Unsubscribe { .. } => Err(AgentOsError::Validation(
                "protocol-level request reached hostd service".to_string(),
            )),
        }
    }

    fn app_notifications_since(
        &self,
        cursor: &ProjectionCursor,
    ) -> AgentOsResult<Vec<AppNotificationEnvelope>> {
        self.notifications_since(cursor)
    }
}

impl AgentOsHost {
    fn thread_start(
        &self,
        client: &ClientConnection,
        goal: String,
        workspace: Option<String>,
    ) -> AgentOsResult<AppResponse> {
        let goal_record = self.kernel().register_goal(RegisterGoalInput {
            namespace: "app".to_string(),
            created_by: client.client_id.clone(),
            title: goal.clone(),
            description: goal.clone(),
            acceptance_criteria: vec!["agent thread reaches a final response".to_string()],
            constraints: Vec::new(),
            risk_level: 0,
            deadline: None,
        })?;
        let task = self.kernel().spawn_task(SpawnTaskInput {
            goal_id: goal_record.goal_id.clone(),
            parent_task_id: None,
            title: goal.clone(),
            description: goal.clone(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: Vec::new(),
            priority: 10,
            risk_level: 0,
        })?;
        let acb = self.kernel().spawn_agent(SpawnAgentInput {
            task_id: task.task_id,
            role_profile_id: "role_producer".to_string(),
            owner: client.client_id.clone(),
            goal,
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: workspace.into_iter().collect(),
        })?;
        accepted("thread", self.thread_by_id(&acb.thread_id)?)
    }

    fn thread_read(&self, client_thread_id: &str) -> AgentOsResult<AppResponse> {
        self.spawn_configured_runtime_job_for_ready_thread(client_thread_id)?;
        self.thread_read_projection(client_thread_id)
    }

    fn thread_turns_read(
        &self,
        client_thread_id: &str,
        offset: usize,
        limit: usize,
    ) -> AgentOsResult<AppResponse> {
        self.thread_by_id(client_thread_id)?;
        let turns = self
            .kernel()
            .store()
            .turn_summaries()?
            .into_iter()
            .filter(|turn| turn.client_thread_id.as_deref() == Some(client_thread_id))
            .collect::<Vec<_>>();
        accepted("turns", page_projection(turns, offset, limit)?)
    }

    fn thread_items_read(
        &self,
        client_thread_id: &str,
        offset: usize,
        limit: usize,
    ) -> AgentOsResult<AppResponse> {
        self.thread_by_id(client_thread_id)?;
        let items = self
            .kernel()
            .store()
            .timeline_items(Some(client_thread_id))?;
        accepted("items", page_projection(items, offset, limit)?)
    }

    fn thread_fork(
        &self,
        client: &ClientConnection,
        client_thread_id: &str,
        from_turn_id: Option<String>,
        title: Option<String>,
        goal: Option<String>,
    ) -> AgentOsResult<AppResponse> {
        let (fork, forked) = self.kernel().fork_thread(ForkThreadInput {
            source_thread_id: client_thread_id.to_string(),
            from_turn_id,
            created_by_client_id: client.client_id.clone(),
            title,
            goal,
        })?;
        Ok(AppResponse::Accepted(json!({
            "fork": fork,
            "thread": self.thread_by_id(&forked.thread_id)?,
        })))
    }

    fn thread_rollback(
        &self,
        client: &ClientConnection,
        client_thread_id: &str,
        target_turn_id: Option<String>,
        target_item_id: Option<String>,
        target_event_id: Option<String>,
        reason: String,
    ) -> AgentOsResult<AppResponse> {
        let (rollback, _) = self.kernel().rollback_thread(RollbackThreadInput {
            thread_id: client_thread_id.to_string(),
            target_turn_id,
            target_item_id,
            target_event_id,
            reason,
            created_by_client_id: client.client_id.clone(),
        })?;
        Ok(AppResponse::Accepted(json!({
            "rollback": rollback,
            "thread": self.thread_by_id(client_thread_id)?,
        })))
    }

    fn thread_compact(
        &self,
        client_thread_id: &str,
        summary_artifact_id: Option<String>,
        superseded_refs: Vec<String>,
        token_estimate: u64,
    ) -> AgentOsResult<AppResponse> {
        let thread = self
            .kernel()
            .state_snapshot()?
            .threads
            .get(client_thread_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {client_thread_id}")))?;
        let compaction = self.kernel().compact_context(CompactContextInput {
            thread_id: client_thread_id.to_string(),
            agent_id: thread.agent_id,
            task_id: thread.task.task_id,
            summary_artifact_id,
            superseded_refs,
            token_estimate,
        })?;
        accepted("compaction", compaction)
    }

    fn thread_list(&self, archived: Option<bool>) -> AgentOsResult<AppResponse> {
        let threads = self
            .kernel()
            .store()
            .thread_summaries()?
            .into_iter()
            .filter(|thread| !thread.deleted)
            .filter(|thread| archived.is_none_or(|flag| thread.archived == flag))
            .collect::<Vec<_>>();
        accepted("threads", threads)
    }

    fn thread_search(&self, query: &str) -> AgentOsResult<AppResponse> {
        let query = query.to_ascii_lowercase();
        let threads = self
            .kernel()
            .store()
            .thread_summaries()?
            .into_iter()
            .filter(|thread| !thread.deleted)
            .filter(|thread| thread.title.to_ascii_lowercase().contains(&query))
            .collect::<Vec<_>>();
        accepted("threads", threads)
    }

    fn thread_archive(&self, client_thread_id: &str) -> AgentOsResult<AppResponse> {
        let thread = self.kernel().archive_thread(client_thread_id)?;
        accepted("thread", thread)
    }

    fn thread_unarchive(&self, client_thread_id: &str) -> AgentOsResult<AppResponse> {
        let thread = self.kernel().unarchive_thread(client_thread_id)?;
        accepted("thread", thread)
    }

    fn thread_delete(&self, client_thread_id: &str) -> AgentOsResult<AppResponse> {
        let thread = self.kernel().delete_thread(client_thread_id)?;
        accepted("thread", thread)
    }

    fn thread_name_set(&self, client_thread_id: &str, title: String) -> AgentOsResult<AppResponse> {
        let thread = self.kernel().rename_thread(client_thread_id, title)?;
        accepted("thread", thread)
    }

    fn config_read(&self, workspace: Option<String>) -> AgentOsResult<AppResponse> {
        let config = config_projection(load_config(workspace)?, self.ecosystem_projection()?)?;
        accepted("config", config)
    }

    fn model_list(&self, workspace: Option<String>) -> AgentOsResult<AppResponse> {
        let config = load_config(workspace)?;
        accepted("models", model_projections(&config.providers, None)?)
    }

    fn provider_capabilities_read(
        &self,
        workspace: Option<String>,
        provider_id: Option<String>,
    ) -> AgentOsResult<AppResponse> {
        let config = load_config(workspace)?;
        let providers = provider_ids(&config.providers, provider_id.as_deref())?;
        let capabilities = providers
            .into_iter()
            .map(|provider_id| {
                Ok(AppProviderCapabilitiesProjection {
                    models: model_projections(&config.providers, Some(&provider_id))?,
                    provider_id,
                })
            })
            .collect::<AgentOsResult<Vec<_>>>()?;
        accepted("providers", capabilities)
    }

    fn provider_usage_read(&self, query: StatsQuery) -> AgentOsResult<AppResponse> {
        let snapshot = self.kernel().store().stats_snapshot(query.clone())?;
        accepted("usage", AppProviderUsageProjection { query, snapshot })
    }

    fn permission_profile_list(&self) -> AgentOsResult<AppResponse> {
        let mut permission_profiles = self
            .kernel()
            .state_snapshot()?
            .permission_profiles
            .values()
            .cloned()
            .collect::<Vec<PermissionProfile>>();
        permission_profiles
            .sort_by(|left, right| left.permission_profile_id.cmp(&right.permission_profile_id));
        accepted("permission_profiles", permission_profiles)
    }

    fn task_bundle_export(&self, client_thread_id: &str) -> AgentOsResult<AppResponse> {
        let thread = self.thread_by_id(client_thread_id)?;
        let task_id = thread.task_id.ok_or_else(|| {
            AgentOsError::Validation(format!(
                "thread {client_thread_id} has no task for task/bundle/export"
            ))
        })?;
        accepted("bundle", self.kernel().export_task_bundle(&task_id)?)
    }

    fn thread_resume(&self, client_thread_id: &str) -> AgentOsResult<AppResponse> {
        let before = self.thread_by_id(client_thread_id)?;
        let reconciliation = self.kernel().reconcile_thread_recovery(client_thread_id)?;
        self.prepare_thread_for_resume(client_thread_id)?;
        Ok(AppResponse::Accepted(json!({
            "thread": self.thread_by_id(client_thread_id)?,
            "previous_thread_status": before.status,
            "reconciliation": {
                "reconciliation_id": reconciliation.reconciliation_id,
                "orphan_tool_call_ids": reconciliation.orphan_tool_call_ids,
                "orphan_process_ids": reconciliation.orphan_process_ids,
                "workspace_diff_refs": reconciliation.workspace_diff_refs,
                "reclaimed_resource_lease_ids": reconciliation.reclaimed_resource_lease_ids,
                "reclaimed_environment_lease_ids": reconciliation.reclaimed_environment_lease_ids,
            },
        })))
    }

    fn turn_start(
        &self,
        client: &ClientConnection,
        client_thread_id: &str,
        input: String,
    ) -> AgentOsResult<AppResponse> {
        let acb = self.kernel().start_turn(client_thread_id)?;
        let turn_id = acb.active_turn.turn_id.clone().ok_or_else(|| {
            AgentOsError::InvalidTransition("turn/start did not produce a turn id".to_string())
        })?;
        let input_record = self.kernel().record_turn_input(
            client,
            client_thread_id,
            &turn_id,
            TurnInputKind::Start,
            input,
        )?;
        let runtime_job = RuntimeJobRecord::queued(RuntimeJob::from_active_turn(&acb)?);
        self.enqueue_runtime_job(runtime_job.clone())?;
        if self.has_runtime_model_config() {
            self.spawn_next_configured_runtime_job_worker()?;
        }
        let thread = self.thread_by_id(client_thread_id)?;
        let turn = self.turn_by_id(&turn_id)?;
        Ok(AppResponse::Accepted(json!({
            "thread": thread,
            "turn": turn,
            "input": input_record,
            "runtime_job": runtime_job,
        })))
    }

    fn turn_steer(
        &self,
        client: &ClientConnection,
        turn_id: &str,
        input: String,
    ) -> AgentOsResult<AppResponse> {
        let turn = self.turn_by_id(turn_id)?;
        let client_thread_id = turn.client_thread_id.clone().ok_or_else(|| {
            AgentOsError::Validation(format!("turn {turn_id} has no client thread"))
        })?;
        let input_record = self.kernel().record_turn_input(
            client,
            &client_thread_id,
            turn_id,
            TurnInputKind::Steer,
            input,
        )?;
        Ok(AppResponse::Accepted(json!({
            "turn": self.turn_by_id(turn_id)?,
            "input": input_record,
        })))
    }

    fn turn_interrupt(&self, turn_id: &str) -> AgentOsResult<AppResponse> {
        let turn = self.turn_by_id(turn_id)?;
        let client_thread_id = turn.client_thread_id.clone().ok_or_else(|| {
            AgentOsError::Validation(format!("turn {turn_id} has no client thread"))
        })?;
        self.kernel().transition_thread(
            &client_thread_id,
            ThreadStatus::Interrupted,
            Some("interrupted by app client".to_string()),
        )?;
        let interrupted_jobs = self.interrupt_runtime_jobs_for_turn(turn_id)?;
        Ok(AppResponse::Accepted(json!({
            "thread": self.thread_by_id(&client_thread_id)?,
            "turn": self.turn_by_id(turn_id)?,
            "runtime_jobs": interrupted_jobs,
        })))
    }

    fn approval_respond(
        &self,
        client: &ClientConnection,
        approval_id: String,
        approved: bool,
    ) -> AgentOsResult<AppResponse> {
        let approval = self.kernel().record_approval(RecordApprovalInput {
            approval_id: approval_id.clone(),
            status: if approved {
                ApprovalStatus::Approved
            } else {
                ApprovalStatus::Denied
            },
            decision_by: client.client_id.clone(),
            decision_reason: Some("resolved through app-server approval/respond".to_string()),
        })?;
        accepted("approval", approval)
    }

    fn resource_session_open(
        &self,
        resource_type: ResourceSessionType,
        client_thread_id: Option<String>,
        lease_expires_at: Option<String>,
        payload: Value,
    ) -> AgentOsResult<AppResponse> {
        let owner_agent_id = match &client_thread_id {
            Some(thread_id) => {
                let state = self.kernel().state_snapshot()?;
                let thread = state
                    .threads
                    .get(thread_id)
                    .ok_or_else(|| AgentOsError::NotFound(format!("thread {thread_id}")))?;
                Some(thread.agent_id.clone())
            }
            None => None,
        };
        let session = self
            .kernel()
            .open_resource_session(OpenResourceSessionInput {
                resource_type,
                client_thread_id,
                owner_agent_id,
                lease_expires_at,
                payload,
            })?;
        accepted("resource_session", session)
    }

    fn resource_session_close(&self, session_id: &str) -> AgentOsResult<AppResponse> {
        let session = self.kernel().close_resource_session(session_id)?;
        accepted("resource_session", session)
    }

    fn process_stop(
        &self,
        client: &ClientConnection,
        process_id: String,
        reason: Option<String>,
    ) -> AgentOsResult<AppResponse> {
        let reason =
            reason.unwrap_or_else(|| format!("process stopped by app client {}", client.client_id));
        let session = self
            .kernel()
            .interrupt_process_session(&process_id, &reason)?;
        accepted("process_session", session)
    }

    fn process_kill(
        &self,
        client: &ClientConnection,
        process_id: String,
        reason: Option<String>,
    ) -> AgentOsResult<AppResponse> {
        let reason =
            reason.unwrap_or_else(|| format!("process killed by app client {}", client.client_id));
        let session = self
            .kernel()
            .terminate_process_session(&process_id, &reason)?;
        accepted("process_session", session)
    }

    fn automation_schedule_create(
        &self,
        input: CreateAutomationScheduleInput,
    ) -> AgentOsResult<AppResponse> {
        let schedule = self.kernel().create_automation_schedule(input)?;
        accepted("automation_schedule", schedule)
    }

    fn automation_schedule_list(&self) -> AgentOsResult<AppResponse> {
        accepted(
            "automation_schedules",
            self.kernel().store().automation_schedules()?,
        )
    }

    fn automation_run_list(&self, schedule_id: Option<String>) -> AgentOsResult<AppResponse> {
        let runs = self
            .kernel()
            .store()
            .automation_runs()?
            .into_iter()
            .filter(|run| {
                schedule_id
                    .as_deref()
                    .map(|id| run.schedule_id == id)
                    .unwrap_or(true)
            })
            .collect::<Vec<_>>();
        accepted("automation_runs", runs)
    }

    fn ecosystem_projection(&self) -> AgentOsResult<AppEcosystemProjection> {
        let state = self.kernel().state_snapshot()?;
        let mut projection = AppEcosystemProjection::default();
        for document in state.instruction_documents.values() {
            projection.instructions += 1;
            source_projection_mut(
                &mut projection.sources,
                &document.source,
                Some(document.precedence_rank),
            )
            .instructions += 1;
        }
        for skill in state.skill_definitions.values() {
            projection.skills += 1;
            source_projection_mut(&mut projection.sources, &skill.source, None).skills += 1;
        }
        for command in state.command_definitions.values() {
            projection.commands += 1;
            source_projection_mut(&mut projection.sources, &command.source, None).commands += 1;
        }
        for server in state.mcp_servers.values() {
            projection.mcp_servers += 1;
            source_projection_mut(&mut projection.sources, &server.source, None).mcp_servers += 1;
        }
        for tool in state.mcp_tools.values() {
            projection.mcp_tools += 1;
            source_projection_mut(&mut projection.sources, &tool.source, None).mcp_tools += 1;
        }
        for profile in state.imported_agent_profiles.values() {
            projection.agents += 1;
            source_projection_mut(&mut projection.sources, &profile.source, None).agents += 1;
        }
        projection.sources.sort_by(|left, right| {
            left.precedence_rank
                .unwrap_or(u32::MAX)
                .cmp(&right.precedence_rank.unwrap_or(u32::MAX))
                .then_with(|| left.source_path.cmp(&right.source_path))
        });
        Ok(projection)
    }
}

fn accepted(key: &str, value: impl Serialize) -> AgentOsResult<AppResponse> {
    let mut body = serde_json::Map::new();
    body.insert(key.to_string(), serde_json::to_value(value)?);
    Ok(AppResponse::Accepted(Value::Object(body)))
}

#[derive(Debug, Serialize)]
struct PageProjection<T> {
    offset: usize,
    limit: usize,
    total: usize,
    items: Vec<T>,
}

fn page_projection<T>(
    items: Vec<T>,
    offset: usize,
    limit: usize,
) -> AgentOsResult<PageProjection<T>> {
    if limit == 0 || limit > 500 {
        return Err(AgentOsError::Validation(
            "page limit must be between 1 and 500".to_string(),
        ));
    }
    let total = items.len();
    Ok(PageProjection {
        offset,
        limit,
        total,
        items: items.into_iter().skip(offset).take(limit).collect(),
    })
}

fn load_config(workspace: Option<String>) -> AgentOsResult<ResolvedAgentOsConfig> {
    let workspace = workspace.map(PathBuf::from);
    ResolvedAgentOsConfig::load(workspace.as_deref())
}

fn config_projection(
    config: ResolvedAgentOsConfig,
    ecosystem: AppEcosystemProjection,
) -> AgentOsResult<AppConfigProjection> {
    Ok(AppConfigProjection {
        config_path: path_string(config.paths.config_file()),
        data_dir: path_string(config.paths.data_dir),
        state_dir: path_string(config.paths.state_dir),
        cache_dir: path_string(config.paths.cache_dir),
        log_dir: path_string(config.paths.log_dir),
        project: config.project.map(|project| AppProjectProjection {
            canonical_root: path_string(project.canonical_root),
            slug: project.slug,
            hash: project.hash,
        }),
        ecosystem,
        model: config.providers.model.clone(),
        small_model: config.providers.small_model.clone(),
        providers: provider_projections(&config.providers)?,
        global_config_recovery: config.global_config_recovery.map(|recovery| {
            AppConfigRecoveryProjection {
                primary_path: path_string(recovery.primary_path),
                backup_path: path_string(recovery.backup_path),
                primary_error: recovery.primary_error,
            }
        }),
    })
}

fn source_projection_mut<'a>(
    sources: &'a mut Vec<AppEcosystemSourceProjection>,
    source: &EcosystemSource,
    precedence_rank: Option<u32>,
) -> &'a mut AppEcosystemSourceProjection {
    if let Some(index) = sources.iter().position(|candidate| {
        candidate.source_kind == source.source_kind
            && candidate.source_scope == source.source_scope
            && candidate.source_path == source.source_path
    }) {
        let projection = &mut sources[index];
        match (projection.precedence_rank, precedence_rank) {
            (Some(current), Some(next)) if next < current => {
                projection.precedence_rank = Some(next);
            }
            (None, Some(next)) => {
                projection.precedence_rank = Some(next);
            }
            _ => {}
        }
        return projection;
    }
    sources.push(AppEcosystemSourceProjection {
        source_kind: source.source_kind,
        source_scope: source.source_scope,
        source_path: source.source_path.clone(),
        precedence_rank,
        instructions: 0,
        skills: 0,
        commands: 0,
        mcp_servers: 0,
        mcp_tools: 0,
        agents: 0,
    });
    let index = sources.len() - 1;
    &mut sources[index]
}

fn provider_projections(catalog: &ProviderCatalog) -> AgentOsResult<Vec<AppProviderProjection>> {
    catalog
        .provider
        .iter()
        .map(|(provider_id, provider)| {
            Ok(AppProviderProjection {
                provider_id: provider_id.clone(),
                endpoint: provider.endpoint,
                base_url: provider.options.base_url.clone(),
                timeout_ms: provider.options.timeout_ms,
                credential: AppCredentialProjection {
                    source: CredentialSource::LocalConfig,
                    name: format!("provider/{provider_id}/api_key"),
                    redacted: true,
                },
                models: provider
                    .models
                    .iter()
                    .map(|(model_id, model)| {
                        model_projection(provider_id, model_id, provider, model)
                    })
                    .collect::<AgentOsResult<Vec<_>>>()?,
            })
        })
        .collect()
}

fn model_projections(
    catalog: &ProviderCatalog,
    provider_id: Option<&str>,
) -> AgentOsResult<Vec<AppModelProjection>> {
    let provider_ids = provider_ids(catalog, provider_id)?;
    let mut models = Vec::new();
    for provider_id in provider_ids {
        let provider = catalog
            .provider
            .get(&provider_id)
            .ok_or_else(|| AgentOsError::NotFound(format!("provider {provider_id}")))?;
        for (model_id, model) in &provider.models {
            models.push(model_projection(&provider_id, model_id, provider, model)?);
        }
    }
    Ok(models)
}

fn model_projection(
    provider_id: &str,
    model_id: &str,
    provider: &ProviderEntry,
    model: &ModelEntry,
) -> AgentOsResult<AppModelProjection> {
    Ok(AppModelProjection {
        id: format!("{provider_id}/{model_id}"),
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        provider_model_name: model.name.clone(),
        endpoint: provider.endpoint,
        base_url: provider.options.base_url.clone(),
        timeout_ms: provider.options.timeout_ms,
        capabilities: model.capabilities.clone(),
        limit: model.limit.clone(),
        options: serde_json::to_value(&model.options)?,
    })
}

fn provider_ids(
    catalog: &ProviderCatalog,
    provider_id: Option<&str>,
) -> AgentOsResult<Vec<String>> {
    match provider_id {
        Some(provider_id) => {
            if catalog.provider.contains_key(provider_id) {
                Ok(vec![provider_id.to_string()])
            } else {
                Err(AgentOsError::NotFound(format!("provider {provider_id}")))
            }
        }
        None => Ok(catalog.provider.keys().cloned().collect()),
    }
}

fn path_string(path: PathBuf) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_os_config::{
        AgentOsConfigFile, AgentOsPaths, ModelConfigEntry, ProviderConfigEntry, ProviderOptions,
    };
    use agent_os_sys::{
        EcosystemSourceKind, EcosystemSourceScope, InstructionDocument, LlmApiStyle,
        ModelCapabilities, ModelLimit, SkillDefinition,
    };
    use std::collections::BTreeMap;

    #[test]
    fn config_projection_redacts_provider_api_keys() {
        let mut models = BTreeMap::new();
        models.insert(
            "gpt-4o".to_string(),
            ModelConfigEntry {
                name: Some("gpt-4o".to_string()),
                limit: Some(ModelLimit {
                    context: 128_000,
                    input: Some(120_000),
                    output: 4096,
                }),
                capabilities: Some(agent_os_config::ModelCapabilitiesConfig::from_capabilities(
                    ModelCapabilities {
                        streaming: true,
                        tool_calling: true,
                        reasoning: true,
                        temperature: true,
                        image_input: true,
                        structured_output: true,
                    },
                )),
                ..ModelConfigEntry::default()
            },
        );
        let mut provider = BTreeMap::new();
        provider.insert(
            "openai".to_string(),
            ProviderConfigEntry {
                api_key: Some("secret-key".to_string()),
                endpoint: Some("openai_responses".to_string()),
                options: ProviderOptions {
                    base_url: Some("https://api.openai.com/v1".to_string()),
                    timeout_ms: Some(30_000),
                },
                models,
            },
        );
        let providers = ProviderCatalog::from_config(AgentOsConfigFile {
            model: Some("openai/gpt-4o".to_string()),
            provider,
            ..AgentOsConfigFile::default()
        })
        .unwrap();
        let root = PathBuf::from("D:/agent-os-test");
        let projection = config_projection(
            ResolvedAgentOsConfig {
                paths: AgentOsPaths {
                    home: root.clone(),
                    config_dir: root.join("config"),
                    data_dir: root.join("data"),
                    state_dir: root.join("state"),
                    cache_dir: root.join("cache"),
                    log_dir: root.join("log"),
                    bin_dir: root.join("cache").join("bin"),
                },
                project: None,
                providers,
                global_config_recovery: None,
            },
            AppEcosystemProjection::default(),
        )
        .unwrap();

        let encoded = serde_json::to_string(&projection).unwrap();
        assert!(!encoded.contains("secret-key"));
        assert_eq!(
            projection.providers[0].credential.source,
            CredentialSource::LocalConfig
        );
        assert_eq!(
            projection.providers[0].credential.name,
            "provider/openai/api_key"
        );
        assert!(projection.providers[0].credential.redacted);
        assert_eq!(
            projection.providers[0].endpoint,
            LlmApiStyle::OpenAiResponses
        );
        assert_eq!(projection.providers[0].models[0].id, "openai/gpt-4o");
    }

    #[test]
    fn ecosystem_projection_groups_imported_kernel_sources() {
        let host = AgentOsHost::in_memory();
        let source = EcosystemSource {
            source_kind: EcosystemSourceKind::Agents,
            source_scope: EcosystemSourceScope::Project,
            source_path: "D:/repo/AGENTS.md".to_string(),
        };
        host.kernel()
            .import_instruction_document(InstructionDocument {
                instruction_id: "inst_agents".to_string(),
                source: source.clone(),
                precedence_rank: 7,
                content: "project rule".to_string(),
                content_hash: "hash_inst".to_string(),
                created_at: "2026-07-03T00:00:00Z".to_string(),
            })
            .unwrap();
        host.kernel()
            .import_skill_definition(SkillDefinition {
                skill_id: "skill_agents".to_string(),
                name: "agents-source".to_string(),
                description: "Agents source skill".to_string(),
                root_path: "D:/repo/.agents/skills/agents-source".to_string(),
                skill_file_path: "D:/repo/.agents/skills/agents-source/SKILL.md".to_string(),
                source: source.clone(),
                content: "Use the project rule.".to_string(),
                metadata: BTreeMap::new(),
                content_hash: "hash_skill".to_string(),
                created_at: "2026-07-03T00:00:00Z".to_string(),
            })
            .unwrap();

        let projection = host.ecosystem_projection().unwrap();
        let source_projection = projection
            .sources
            .iter()
            .find(|candidate| candidate.source_path == source.source_path)
            .unwrap();

        assert_eq!(projection.instructions, 1);
        assert_eq!(projection.skills, 1);
        assert_eq!(source_projection.source_kind, EcosystemSourceKind::Agents);
        assert_eq!(
            source_projection.source_scope,
            EcosystemSourceScope::Project
        );
        assert_eq!(source_projection.precedence_rank, Some(7));
        assert_eq!(source_projection.instructions, 1);
        assert_eq!(source_projection.skills, 1);
    }
}
