use crate::AgentOsHost;
use agent_os_app_server::AppKernelService;
use agent_os_kernel::{RecordApprovalInput, RegisterGoalInput, SpawnAgentInput, SpawnTaskInput};
use agent_os_sys::{
    AgentOsError, AgentOsResult, AppNotificationEnvelope, AppRequest, AppResponse, ApprovalStatus,
    ClientConnection, CreateAutomationScheduleInput, OpenResourceSessionInput, ProjectionCursor,
    ResourceSessionType, ThreadStatus, TurnInputKind,
};
use agent_os_thread::{RuntimeJob, RuntimeJobRecord};
use serde::Serialize;
use serde_json::{json, Value};

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
            AppRequest::ThreadList { archived } => self.thread_list(archived),
            AppRequest::ThreadSearch { query } => self.thread_search(&query),
            AppRequest::ThreadArchive { client_thread_id } => {
                self.thread_archive(&client_thread_id)
            }
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

    fn thread_list(&self, archived: Option<bool>) -> AgentOsResult<AppResponse> {
        let threads = self
            .kernel()
            .store()
            .thread_summaries()?
            .into_iter()
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
            .filter(|thread| thread.title.to_ascii_lowercase().contains(&query))
            .collect::<Vec<_>>();
        accepted("threads", threads)
    }

    fn thread_archive(&self, client_thread_id: &str) -> AgentOsResult<AppResponse> {
        let thread = self.kernel().archive_thread(client_thread_id)?;
        accepted("thread", thread)
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
}

fn accepted(key: &str, value: impl Serialize) -> AgentOsResult<AppResponse> {
    let mut body = serde_json::Map::new();
    body.insert(key.to_string(), serde_json::to_value(value)?);
    Ok(AppResponse::Accepted(Value::Object(body)))
}
