//! Long-running Agent-OS kernel daemon service.

mod app_projection;
mod notifications;
mod runtime_jobs;
mod runtime_model;
mod stdio;
mod types;

use agent_os_app_server::AppKernelService;
pub use agent_os_app_server::AppServer;
use agent_os_kernel::{
    Kernel, RecordApprovalInput, RegisterGoalInput, SpawnAgentInput, SpawnTaskInput,
};
use agent_os_store_sqlite::SqliteStore;
use agent_os_sys::{
    now_rfc3339, AgentOsError, AgentOsResult, AppNotificationEnvelope, AppRequest, AppResponse,
    ApprovalStatus, AutomationRun, AutomationScheduleKind, AutomationScheduleStatus,
    ClientConnection, ClientKind, ClientThread, CreateAutomationScheduleInput,
    OpenResourceSessionInput, ProjectionCursor, ResourceSessionType, SecurityLevel, StatsSnapshot,
    ThreadStatus, TurnInputKind, TurnRecord,
};
use agent_os_thread::{
    expand_command_template, import_workspace_ecosystem, ModelClient, RuntimeConfig, RuntimeJob,
    RuntimeJobRecord, RuntimeRunReport, ThreadRuntime,
};
pub use runtime_model::{
    DaemonRuntimeModelConfig, ExternalRuntimeModelConfig, ProviderRuntimeModelConfig,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
pub use stdio::{run_stdio_daemon, KerneldArgs};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use types::RuntimeWorkerJoinHandle;
pub use types::{DaemonReplaySummary, DaemonShutdownReport};

#[derive(Clone)]
pub struct KernelDaemon {
    kernel: Kernel,
    runtime_jobs: Arc<Mutex<BTreeMap<String, RuntimeJobRecord>>>,
    runtime_workers: Arc<Mutex<BTreeMap<String, RuntimeWorkerJoinHandle>>>,
    runtime_model_config: Option<DaemonRuntimeModelConfig>,
}

impl fmt::Debug for KernelDaemon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let runtime_job_count = self
            .runtime_jobs
            .lock()
            .map(|jobs| jobs.len())
            .unwrap_or_default();
        let runtime_worker_count = self
            .runtime_workers
            .lock()
            .map(|workers| workers.len())
            .unwrap_or_default();
        f.debug_struct("KernelDaemon")
            .field("kernel", &self.kernel)
            .field("runtime_jobs", &runtime_job_count)
            .field("runtime_workers", &runtime_worker_count)
            .finish()
    }
}

impl KernelDaemon {
    pub fn new(kernel: Kernel) -> Self {
        Self::with_runtime_jobs(kernel, BTreeMap::new())
    }

    pub fn try_new(kernel: Kernel) -> AgentOsResult<Self> {
        let runtime_jobs = Self::replay_runtime_jobs(&kernel)?;
        Ok(Self::with_runtime_jobs(kernel, runtime_jobs))
    }

    pub fn in_memory() -> Self {
        Self::new(Kernel::new())
    }

    pub fn open_sqlite(path: impl AsRef<Path>) -> AgentOsResult<Self> {
        Self::try_new(Kernel::with_replayed_store(SqliteStore::open(path)?)?)
    }

    pub fn kernel(&self) -> &Kernel {
        &self.kernel
    }

    pub fn event_count(&self) -> AgentOsResult<usize> {
        Ok(self.kernel.events()?.len())
    }

    pub fn replay_summary(&self) -> AgentOsResult<DaemonReplaySummary> {
        let replayed = Kernel::from_events(&self.kernel.events()?)?;
        let state = replayed.state_snapshot()?;
        Ok(DaemonReplaySummary {
            tasks: state.tasks.len(),
            threads: state.threads.len(),
            artifacts: state.artifacts.len(),
            evidence: state.evidence.len(),
            final_submissions: state.final_submissions.len(),
        })
    }

    pub fn stats_snapshot(&self, query: agent_os_sys::StatsQuery) -> AgentOsResult<StatsSnapshot> {
        self.kernel.store().stats_snapshot(query)
    }

    pub fn run_due_automations_at(&self, now: &str) -> AgentOsResult<Vec<AutomationRun>> {
        let now_instant = OffsetDateTime::parse(now, &Rfc3339).map_err(|error| {
            AgentOsError::Validation(format!("invalid scheduler clock: {error}"))
        })?;
        let schedules = self.kernel.store().automation_schedules()?;
        let mut runs = Vec::new();
        for schedule in schedules {
            if schedule.status != AutomationScheduleStatus::Active {
                continue;
            }
            let Some(next_run_at) = &schedule.next_run_at else {
                continue;
            };
            let due_at = OffsetDateTime::parse(next_run_at, &Rfc3339).map_err(|error| {
                AgentOsError::Validation(format!(
                    "invalid automation next_run_at for {}: {error}",
                    schedule.schedule_id
                ))
            })?;
            if due_at > now_instant {
                continue;
            }
            if schedule.kind == AutomationScheduleKind::ThreadWakeup {
                let Some(thread_id) = &schedule.target_thread_id else {
                    return Err(AgentOsError::Validation(format!(
                        "thread wakeup automation {} has no target_thread_id",
                        schedule.schedule_id
                    )));
                };
                self.thread_by_id(thread_id)?;
            }
            let run = self
                .kernel
                .queue_automation_run(&schedule.schedule_id, next_run_at.clone())?;
            if run.kind == AutomationScheduleKind::ThreadWakeup {
                self.enqueue_thread_wakeup_job(&run)?;
            }
            runs.push(run);
        }
        Ok(runs)
    }

    pub fn register_model_alias(
        &self,
        alias: &str,
        provider_id: &str,
        model: &str,
        capabilities: Value,
        provider_profile_id: &str,
    ) -> AgentOsResult<()> {
        self.kernel.register_model_alias(
            alias,
            provider_id,
            model,
            capabilities,
            provider_profile_id,
        )
    }

    pub fn expand_workspace_command(
        &self,
        workspace: &Path,
        name: &str,
        args: &[String],
        raw_arguments: &str,
    ) -> AgentOsResult<String> {
        import_workspace_ecosystem(&self.kernel, workspace)?;
        let state = self.kernel.state_snapshot()?;
        let command = state
            .command_definitions
            .get(name)
            .ok_or_else(|| AgentOsError::NotFound(format!("command /{name}")))?;
        Ok(expand_command_template(
            &command.template,
            args,
            raw_arguments,
        ))
    }

    pub fn serve_jsonl<R, W>(self, reader: R, writer: W) -> AgentOsResult<()>
    where
        R: BufRead,
        W: Write,
    {
        AppServer::new(self).serve_jsonl(reader, writer)
    }

    pub fn run_next_runtime_job<C>(
        &self,
        model_client: C,
    ) -> AgentOsResult<Option<RuntimeRunReport>>
    where
        C: ModelClient,
    {
        let Some(runtime_job_id) = self.next_queued_runtime_job_id()? else {
            return Ok(None);
        };
        self.run_runtime_job(&runtime_job_id, model_client)
            .map(Some)
    }

    pub fn run_next_runtime_job_with_factory<F, C>(
        &self,
        factory: F,
    ) -> AgentOsResult<Option<RuntimeRunReport>>
    where
        F: FnOnce(&RuntimeJob) -> AgentOsResult<C>,
        C: ModelClient,
    {
        let Some(runtime_job_id) = self.next_queued_runtime_job_id()? else {
            return Ok(None);
        };
        let job = self.runtime_job(&runtime_job_id)?;
        let model_client = factory(&job)?;
        self.run_runtime_job(&runtime_job_id, model_client)
            .map(Some)
    }

    pub fn run_runtime_job<C>(
        &self,
        runtime_job_id: &str,
        model_client: C,
    ) -> AgentOsResult<RuntimeRunReport>
    where
        C: ModelClient,
    {
        let job = self.runtime_job(runtime_job_id)?;
        self.run_runtime_job_with_config(
            runtime_job_id,
            model_client,
            RuntimeConfig::workspace_write(&job.workspace),
        )
    }

    pub fn run_runtime_job_with_config<C>(
        &self,
        runtime_job_id: &str,
        model_client: C,
        config: RuntimeConfig,
    ) -> AgentOsResult<RuntimeRunReport>
    where
        C: ModelClient,
    {
        let job = self.mark_runtime_job_running(runtime_job_id)?;
        let mut runtime =
            ThreadRuntime::new_for_job(self.kernel.clone(), job.clone(), model_client);
        let report = runtime.run_job_to_completion(config);
        match report {
            Ok(report) => {
                self.finish_runtime_job(runtime_job_id, &report)?;
                Ok(report)
            }
            Err(error) => {
                self.fail_runtime_job(runtime_job_id, error.to_string())?;
                Err(error)
            }
        }
    }

    pub fn spawn_runtime_job_worker<C>(
        &self,
        runtime_job_id: &str,
        model_client: C,
        config: RuntimeConfig,
    ) -> AgentOsResult<()>
    where
        C: ModelClient + Send + 'static,
    {
        self.ensure_runtime_job_can_spawn(runtime_job_id)?;
        let runtime_job_id = runtime_job_id.to_string();
        let worker_daemon = self.clone();
        let worker_job_id = runtime_job_id.clone();
        let handle = std::thread::spawn(move || {
            worker_daemon.run_runtime_job_with_config(&worker_job_id, model_client, config)
        });
        let mut workers = self.runtime_workers.lock().map_err(|_| {
            AgentOsError::Validation("runtime worker registry lock poisoned".to_string())
        })?;
        workers.insert(runtime_job_id, handle);
        Ok(())
    }

    pub fn spawn_next_runtime_job_worker_with_factory<F, C>(
        &self,
        factory: F,
    ) -> AgentOsResult<Option<String>>
    where
        F: FnOnce(&RuntimeJob) -> AgentOsResult<C>,
        C: ModelClient + Send + 'static,
    {
        let Some(runtime_job_id) = self.next_queued_runtime_job_id()? else {
            return Ok(None);
        };
        let job = self.runtime_job(&runtime_job_id)?;
        let config = RuntimeConfig::workspace_write(&job.workspace);
        let model_client = factory(&job)?;
        self.spawn_runtime_job_worker(&runtime_job_id, model_client, config)?;
        Ok(Some(runtime_job_id))
    }

    pub fn shutdown(&self) -> AgentOsResult<DaemonShutdownReport> {
        let workers = {
            let mut workers = self.runtime_workers.lock().map_err(|_| {
                AgentOsError::Validation("runtime worker registry lock poisoned".to_string())
            })?;
            std::mem::take(&mut *workers)
        };
        let mut report = DaemonShutdownReport {
            joined_runtime_workers: 0,
            failed_runtime_workers: Vec::new(),
            runtime_reports: Vec::new(),
        };
        for (runtime_job_id, handle) in workers {
            match handle.join() {
                Ok(Ok(runtime_report)) => {
                    report.joined_runtime_workers += 1;
                    report.runtime_reports.push(runtime_report);
                }
                Ok(Err(error)) => {
                    report.joined_runtime_workers += 1;
                    report
                        .failed_runtime_workers
                        .push(format!("{runtime_job_id}: {error}"));
                }
                Err(_) => {
                    report
                        .failed_runtime_workers
                        .push(format!("{runtime_job_id}: runtime worker panicked"));
                }
            }
        }
        Ok(report)
    }
}

impl AppKernelService for KernelDaemon {
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
                accepted("snapshot", self.kernel.store().stats_snapshot(query)?)
            }
            AppRequest::Initialize
            | AppRequest::Subscribe { .. }
            | AppRequest::Unsubscribe { .. } => Err(AgentOsError::Validation(
                "protocol-level request reached kerneld service".to_string(),
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

impl KernelDaemon {
    fn thread_start(
        &self,
        client: &ClientConnection,
        goal: String,
        workspace: Option<String>,
    ) -> AgentOsResult<AppResponse> {
        let goal_record = self.kernel.register_goal(RegisterGoalInput {
            namespace: "app".to_string(),
            created_by: client.client_id.clone(),
            title: goal.clone(),
            description: goal.clone(),
            acceptance_criteria: vec!["agent thread reaches a final response".to_string()],
            constraints: Vec::new(),
            risk_level: 0,
            deadline: None,
        })?;
        let task = self.kernel.spawn_task(SpawnTaskInput {
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
        let acb = self.kernel.spawn_agent(SpawnAgentInput {
            task_id: task.task_id,
            role_profile_id: "role_worker".to_string(),
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
            .kernel
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
            .kernel
            .store()
            .thread_summaries()?
            .into_iter()
            .filter(|thread| thread.title.to_ascii_lowercase().contains(&query))
            .collect::<Vec<_>>();
        accepted("threads", threads)
    }

    fn thread_archive(&self, client_thread_id: &str) -> AgentOsResult<AppResponse> {
        let thread = self.kernel.archive_thread(client_thread_id)?;
        accepted("thread", thread)
    }

    fn task_bundle_export(&self, client_thread_id: &str) -> AgentOsResult<AppResponse> {
        let thread = self.thread_by_id(client_thread_id)?;
        let task_id = thread.task_id.ok_or_else(|| {
            AgentOsError::Validation(format!(
                "thread {client_thread_id} has no task for task/bundle/export"
            ))
        })?;
        accepted("bundle", self.kernel.export_task_bundle(&task_id)?)
    }

    fn thread_resume(&self, client_thread_id: &str) -> AgentOsResult<AppResponse> {
        let before = self.thread_by_id(client_thread_id)?;
        let reconciliation = self.kernel.reconcile_thread_recovery(client_thread_id)?;
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
        let acb = self.kernel.start_turn(client_thread_id)?;
        let turn_id = acb.active_turn.turn_id.clone().ok_or_else(|| {
            AgentOsError::InvalidTransition("turn/start did not produce a turn id".to_string())
        })?;
        let input_record = self.kernel.record_turn_input(
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
        let input_record = self.kernel.record_turn_input(
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
        self.kernel.transition_thread(
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
        let approval = self.kernel.record_approval(RecordApprovalInput {
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
                let state = self.kernel.state_snapshot()?;
                let thread = state
                    .threads
                    .get(thread_id)
                    .ok_or_else(|| AgentOsError::NotFound(format!("thread {thread_id}")))?;
                Some(thread.agent_id.clone())
            }
            None => None,
        };
        let session = self
            .kernel
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
        let session = self.kernel.close_resource_session(session_id)?;
        accepted("resource_session", session)
    }

    fn automation_schedule_create(
        &self,
        input: CreateAutomationScheduleInput,
    ) -> AgentOsResult<AppResponse> {
        let schedule = self.kernel.create_automation_schedule(input)?;
        accepted("automation_schedule", schedule)
    }

    fn automation_schedule_list(&self) -> AgentOsResult<AppResponse> {
        accepted(
            "automation_schedules",
            self.kernel.store().automation_schedules()?,
        )
    }

    fn automation_run_list(&self, schedule_id: Option<String>) -> AgentOsResult<AppResponse> {
        let runs = self
            .kernel
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

    fn enqueue_thread_wakeup_job(&self, run: &AutomationRun) -> AgentOsResult<()> {
        let thread_id = run.target_thread_id.as_deref().ok_or_else(|| {
            AgentOsError::Validation(format!(
                "automation run {} has no target_thread_id",
                run.run_id
            ))
        })?;
        let client = ClientConnection {
            client_id: format!("automation_{}", run.schedule_id),
            client_name: "Automation Scheduler".to_string(),
            client_kind: ClientKind::Automation,
            authority: SecurityLevel::HUMAN_ROOT,
            connected_at: now_rfc3339(),
        };
        let acb = self.kernel.start_turn(thread_id)?;
        let turn_id = acb.active_turn.turn_id.clone().ok_or_else(|| {
            AgentOsError::InvalidTransition(
                "automation wakeup did not produce a turn id".to_string(),
            )
        })?;
        self.kernel.record_turn_input(
            &client,
            thread_id,
            &turn_id,
            TurnInputKind::Start,
            run.prompt.clone(),
        )?;
        self.enqueue_runtime_job(RuntimeJobRecord::queued(RuntimeJob::from_active_turn(
            &acb,
        )?))
    }

    fn prepare_thread_for_resume(&self, client_thread_id: &str) -> AgentOsResult<()> {
        let status = self.thread_by_id(client_thread_id)?.status;
        match status {
            ThreadStatus::Created | ThreadStatus::Ready => Ok(()),
            ThreadStatus::Running
            | ThreadStatus::WaitingTool
            | ThreadStatus::WaitingPermission
            | ThreadStatus::WaitingUser => {
                self.kernel.transition_thread(
                    client_thread_id,
                    ThreadStatus::Interrupted,
                    Some("resume recovered incomplete turn".to_string()),
                )?;
                self.kernel.transition_thread(
                    client_thread_id,
                    ThreadStatus::Ready,
                    Some("resume requested after recovery".to_string()),
                )?;
                Ok(())
            }
            ThreadStatus::Interrupted
            | ThreadStatus::Blocked
            | ThreadStatus::Suspended
            | ThreadStatus::ResidentIdle
            | ThreadStatus::Unloaded => {
                self.kernel.transition_thread(
                    client_thread_id,
                    ThreadStatus::Ready,
                    Some("resume requested".to_string()),
                )?;
                Ok(())
            }
            ThreadStatus::Completing
            | ThreadStatus::Completed
            | ThreadStatus::Failed
            | ThreadStatus::Quarantined
            | ThreadStatus::Terminated => Err(AgentOsError::InvalidTransition(format!(
                "thread {:?} cannot be resumed",
                status
            ))),
        }
    }

    pub(crate) fn thread_by_id(&self, client_thread_id: &str) -> AgentOsResult<ClientThread> {
        self.kernel
            .store()
            .thread_summaries()?
            .into_iter()
            .find(|thread| thread.client_thread_id == client_thread_id)
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {client_thread_id}")))
    }

    fn turn_by_id(&self, turn_id: &str) -> AgentOsResult<TurnRecord> {
        self.kernel
            .store()
            .turn_summaries()?
            .into_iter()
            .find(|turn| turn.turn_id == turn_id)
            .ok_or_else(|| AgentOsError::NotFound(format!("turn {turn_id}")))
    }

    fn enqueue_runtime_job(&self, record: RuntimeJobRecord) -> AgentOsResult<()> {
        let runtime_job_id = record.runtime_job_id.clone();
        let mut jobs = self.runtime_jobs.lock().map_err(|_| {
            AgentOsError::Validation("runtime job registry lock poisoned".to_string())
        })?;
        jobs.insert(runtime_job_id.clone(), record);
        let record = jobs
            .get(&runtime_job_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("runtime job {runtime_job_id}")))?;
        drop(jobs);
        self.record_runtime_job_event("RuntimeJobQueued", &record)?;
        Ok(())
    }

    fn next_queued_runtime_job_id(&self) -> AgentOsResult<Option<String>> {
        let jobs = self.runtime_jobs.lock().map_err(|_| {
            AgentOsError::Validation("runtime job registry lock poisoned".to_string())
        })?;
        Ok(jobs
            .values()
            .find(|record| record.status == agent_os_thread::RuntimeJobStatus::Queued)
            .map(|record| record.runtime_job_id.clone()))
    }

    fn next_queued_runtime_job_id_for_thread(
        &self,
        client_thread_id: &str,
    ) -> AgentOsResult<Option<String>> {
        let jobs = self.runtime_jobs.lock().map_err(|_| {
            AgentOsError::Validation("runtime job registry lock poisoned".to_string())
        })?;
        Ok(jobs
            .values()
            .find(|record| {
                record.status == agent_os_thread::RuntimeJobStatus::Queued
                    && record.job.client_thread_id == client_thread_id
            })
            .map(|record| record.runtime_job_id.clone()))
    }

    fn spawn_configured_runtime_job_for_ready_thread(
        &self,
        client_thread_id: &str,
    ) -> AgentOsResult<Option<String>> {
        if !self.has_runtime_model_config() {
            return Ok(None);
        }
        let thread = self.thread_by_id(client_thread_id)?;
        if thread.status != ThreadStatus::Ready {
            return Ok(None);
        }
        let Some(runtime_job_id) = self.next_queued_runtime_job_id_for_thread(client_thread_id)?
        else {
            return Ok(None);
        };
        self.spawn_configured_runtime_job_worker(&runtime_job_id)?;
        Ok(Some(runtime_job_id))
    }

    fn mark_runtime_job_running(&self, runtime_job_id: &str) -> AgentOsResult<RuntimeJob> {
        let (job, record) = {
            let mut jobs = self.runtime_jobs.lock().map_err(|_| {
                AgentOsError::Validation("runtime job registry lock poisoned".to_string())
            })?;
            let record = jobs
                .get_mut(runtime_job_id)
                .ok_or_else(|| AgentOsError::NotFound(format!("runtime job {runtime_job_id}")))?;
            if record.status != agent_os_thread::RuntimeJobStatus::Queued {
                return Err(AgentOsError::InvalidTransition(format!(
                    "runtime job {:?} -> running",
                    record.status
                )));
            }
            record.start();
            (record.job.clone(), record.clone())
        };
        self.record_runtime_job_event("RuntimeJobStarted", &record)?;
        Ok(job)
    }

    fn runtime_job(&self, runtime_job_id: &str) -> AgentOsResult<RuntimeJob> {
        let jobs = self.runtime_jobs.lock().map_err(|_| {
            AgentOsError::Validation("runtime job registry lock poisoned".to_string())
        })?;
        jobs.get(runtime_job_id)
            .map(|record| record.job.clone())
            .ok_or_else(|| AgentOsError::NotFound(format!("runtime job {runtime_job_id}")))
    }

    fn ensure_runtime_job_can_spawn(&self, runtime_job_id: &str) -> AgentOsResult<()> {
        {
            let workers = self.runtime_workers.lock().map_err(|_| {
                AgentOsError::Validation("runtime worker registry lock poisoned".to_string())
            })?;
            if workers.contains_key(runtime_job_id) {
                return Err(AgentOsError::InvalidTransition(format!(
                    "runtime job {runtime_job_id} already has a background worker"
                )));
            }
        }
        let jobs = self.runtime_jobs.lock().map_err(|_| {
            AgentOsError::Validation("runtime job registry lock poisoned".to_string())
        })?;
        let record = jobs
            .get(runtime_job_id)
            .ok_or_else(|| AgentOsError::NotFound(format!("runtime job {runtime_job_id}")))?;
        if record.status != agent_os_thread::RuntimeJobStatus::Queued {
            return Err(AgentOsError::InvalidTransition(format!(
                "runtime job {:?} cannot spawn a background worker",
                record.status
            )));
        }
        Ok(())
    }

    fn finish_runtime_job(
        &self,
        runtime_job_id: &str,
        report: &RuntimeRunReport,
    ) -> AgentOsResult<()> {
        let record = {
            let mut jobs = self.runtime_jobs.lock().map_err(|_| {
                AgentOsError::Validation("runtime job registry lock poisoned".to_string())
            })?;
            let record = jobs
                .get_mut(runtime_job_id)
                .ok_or_else(|| AgentOsError::NotFound(format!("runtime job {runtime_job_id}")))?;
            if report.status == ThreadStatus::Interrupted {
                record.interrupt();
            } else if report.status == ThreadStatus::WaitingTool {
                record.requeue();
            } else if report.final_submitted && report.status == ThreadStatus::Completed {
                record.complete();
            } else {
                record.fail(format!("runtime finished with status {:?}", report.status));
            }
            record.clone()
        };
        let event_type = match record.status {
            agent_os_thread::RuntimeJobStatus::Completed => "RuntimeJobCompleted",
            agent_os_thread::RuntimeJobStatus::Queued => "RuntimeJobQueued",
            agent_os_thread::RuntimeJobStatus::Interrupted => "RuntimeJobInterrupted",
            agent_os_thread::RuntimeJobStatus::Failed => "RuntimeJobFailed",
            _ => "RuntimeJobUpdated",
        };
        self.record_runtime_job_event(event_type, &record)?;
        Ok(())
    }

    fn fail_runtime_job(&self, runtime_job_id: &str, error: String) -> AgentOsResult<()> {
        let record = {
            let mut jobs = self.runtime_jobs.lock().map_err(|_| {
                AgentOsError::Validation("runtime job registry lock poisoned".to_string())
            })?;
            let record = jobs
                .get_mut(runtime_job_id)
                .ok_or_else(|| AgentOsError::NotFound(format!("runtime job {runtime_job_id}")))?;
            record.fail(error);
            record.clone()
        };
        self.record_runtime_job_event("RuntimeJobFailed", &record)?;
        Ok(())
    }

    pub(crate) fn runtime_jobs_for_thread(
        &self,
        client_thread_id: &str,
    ) -> AgentOsResult<Vec<RuntimeJobRecord>> {
        let jobs = self.runtime_jobs.lock().map_err(|_| {
            AgentOsError::Validation("runtime job registry lock poisoned".to_string())
        })?;
        Ok(jobs
            .values()
            .filter(|record| record.job.client_thread_id == client_thread_id)
            .cloned()
            .collect())
    }

    fn interrupt_runtime_jobs_for_turn(
        &self,
        turn_id: &str,
    ) -> AgentOsResult<Vec<RuntimeJobRecord>> {
        let interrupted = {
            let mut jobs = self.runtime_jobs.lock().map_err(|_| {
                AgentOsError::Validation("runtime job registry lock poisoned".to_string())
            })?;
            let mut interrupted = Vec::new();
            for record in jobs.values_mut() {
                if record.job.turn_id == turn_id {
                    record.interrupt();
                    interrupted.push(record.clone());
                }
            }
            interrupted
        };
        for record in &interrupted {
            self.record_runtime_job_event("RuntimeJobInterrupted", record)?;
        }
        Ok(interrupted)
    }
}

fn accepted(key: &str, value: impl Serialize) -> AgentOsResult<AppResponse> {
    let mut body = serde_json::Map::new();
    body.insert(key.to_string(), serde_json::to_value(value)?);
    Ok(AppResponse::Accepted(Value::Object(body)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_os_app_server::AppServer;
    use agent_os_sys::{
        AppRequestEnvelope, AutomationScheduleKind, ClientKind, EvidenceMapEntry, FinalSubmission,
        ProjectionCursor, ProviderUsage, ResourceSessionType, SecurityLevel, StatsQuery,
        StatsSnapshot,
    };
    use agent_os_thread::{
        ModelAction, ModelClient, ModelTurnRequest, ModelTurnResponse, ToolAction,
    };
    use std::collections::VecDeque;
    use std::env;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn app_server_starts_lists_reads_and_archives_threads_through_daemon() {
        let mut server = initialized_server();

        let started = request(
            &mut server,
            "req_thread_start",
            AppRequest::ThreadStart {
                goal: "build projection-backed app thread".to_string(),
                workspace: Some("D:/work/example".to_string()),
            },
        );
        let thread_id = accepted_body(started)["thread"]["client_thread_id"]
            .as_str()
            .unwrap()
            .to_string();

        let listed = request(
            &mut server,
            "req_thread_list",
            AppRequest::ThreadList {
                archived: Some(false),
            },
        );
        assert_eq!(
            accepted_body(listed)["threads"][0]["client_thread_id"],
            thread_id
        );

        let read = request(
            &mut server,
            "req_thread_read",
            AppRequest::ThreadRead {
                client_thread_id: thread_id.clone(),
            },
        );
        assert_eq!(accepted_body(read)["timeline"].as_array().unwrap().len(), 1);

        let archived = request(
            &mut server,
            "req_thread_archive",
            AppRequest::ThreadArchive {
                client_thread_id: thread_id,
            },
        );
        assert_eq!(accepted_body(archived)["thread"]["archived"], true);
    }

    #[test]
    fn app_server_exports_thread_task_bundle_through_protocol() {
        let mut server = initialized_server();
        let thread_id = start_thread(&mut server);
        let read = request(
            &mut server,
            "req_thread_read_for_bundle",
            AppRequest::ThreadRead {
                client_thread_id: thread_id.clone(),
            },
        );
        let task_id = accepted_body(read)["thread"]["task_id"]
            .as_str()
            .unwrap()
            .to_string();

        let exported = request(
            &mut server,
            "req_task_bundle_export",
            AppRequest::TaskBundleExport {
                client_thread_id: thread_id,
            },
        );
        let body = accepted_body(exported);

        assert_eq!(body["bundle"]["root_task_id"], task_id);
        assert_eq!(body["bundle"]["bundle_kind"], "task");
        assert!(
            body["bundle"]["replay_summary"]["task_count"]
                .as_u64()
                .unwrap()
                >= 1
        );
        assert!(
            body["bundle"]["replay_summary"]["thread_count"]
                .as_u64()
                .unwrap()
                >= 1
        );
    }

    #[test]
    fn daemon_resource_session_lifecycle_flows_through_app_server() {
        let daemon = KernelDaemon::in_memory();
        let mut server = initialized_server_with_daemon(daemon.clone());
        let thread_id = start_thread(&mut server);

        let opened = request(
            &mut server,
            "req_resource_open",
            AppRequest::ResourceSessionOpen {
                resource_type: ResourceSessionType::Terminal,
                client_thread_id: Some(thread_id.clone()),
                lease_expires_at: None,
                payload: serde_json::json!({"cwd": "D:/work/example"}),
            },
        );
        let body = accepted_body(opened);
        let session_id = body["resource_session"]["session_id"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(body["resource_session"]["status"], "active");

        let read = request(
            &mut server,
            "req_thread_read_resources",
            AppRequest::ThreadRead {
                client_thread_id: thread_id.clone(),
            },
        );
        assert!(
            accepted_body(read)["resources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|resource| resource["session_id"] == session_id
                    && resource["status"] == "active")
        );

        let notifications = daemon
            .app_notifications_since(&ProjectionCursor {
                last_event_ordinal: 0,
            })
            .unwrap();
        assert!(notifications.iter().any(|envelope| matches!(
            &envelope.notification,
            agent_os_sys::AppNotification::ResourceUpdated(resource)
                if resource.session_id == session_id
        )));

        let closed = request(
            &mut server,
            "req_resource_close",
            AppRequest::ResourceSessionClose {
                session_id: session_id.clone(),
            },
        );
        assert_eq!(
            accepted_body(closed)["resource_session"]["status"],
            "closed"
        );

        let read = request(
            &mut server,
            "req_thread_read_closed_resource",
            AppRequest::ThreadRead {
                client_thread_id: thread_id,
            },
        );
        assert!(
            accepted_body(read)["resources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|resource| resource["session_id"] == session_id
                    && resource["status"] == "closed")
        );
    }

    #[test]
    fn daemon_queues_due_thread_wakeup_automation_with_injected_clock() {
        let daemon = KernelDaemon::in_memory();
        let mut server = initialized_server_with_daemon(daemon.clone());
        let thread_id = start_thread(&mut server);

        let created = request(
            &mut server,
            "req_automation_schedule_create",
            AppRequest::AutomationScheduleCreate {
                name: "wake thread".to_string(),
                kind: AutomationScheduleKind::ThreadWakeup,
                target_thread_id: Some(thread_id.clone()),
                workspace: None,
                prompt: "continue scheduled work".to_string(),
                next_run_at: Some("2026-06-30T00:00:00Z".to_string()),
                interval_seconds: None,
                payload: serde_json::json!({"source": "test"}),
            },
        );
        let schedule_id = accepted_body(created)["automation_schedule"]["schedule_id"]
            .as_str()
            .unwrap()
            .to_string();

        let runs = daemon
            .run_due_automations_at("2026-06-30T00:00:01Z")
            .unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].schedule_id, schedule_id);
        assert_eq!(
            runs[0].target_thread_id.as_deref(),
            Some(thread_id.as_str())
        );

        let read = request(
            &mut server,
            "req_thread_read_after_automation",
            AppRequest::ThreadRead {
                client_thread_id: thread_id,
            },
        );
        let body = accepted_body(read);
        assert_eq!(body["automation_runs"].as_array().unwrap().len(), 1);
        assert_eq!(body["runtime_jobs"].as_array().unwrap().len(), 1);
        assert_eq!(body["runtime_jobs"][0]["status"], "queued");
        assert_eq!(body["turns"].as_array().unwrap().len(), 1);
        assert!(daemon
            .kernel()
            .store()
            .automation_schedules()
            .unwrap()
            .iter()
            .any(|schedule| schedule.schedule_id == schedule_id && schedule.next_run_at.is_none()));
    }

    #[test]
    fn daemon_turn_start_steer_and_interrupt_update_projection_records() {
        let mut server = initialized_server();
        let thread_id = start_thread(&mut server);

        let started = request(
            &mut server,
            "req_turn_start",
            AppRequest::TurnStart {
                client_thread_id: thread_id.clone(),
                input: "start work".to_string(),
            },
        );
        let body = accepted_body(started);
        let turn_id = body["turn"]["turn_id"].as_str().unwrap().to_string();
        assert_eq!(body["turn"]["status"], "InProgress");
        assert_eq!(body["input"]["kind"], "start");
        assert_eq!(body["runtime_job"]["status"], "queued");
        assert_eq!(
            body["runtime_job"]["job"]["client_thread_id"],
            thread_id.clone()
        );
        assert_eq!(body["runtime_job"]["job"]["turn_id"], turn_id.clone());
        assert_eq!(body["runtime_job"]["job"]["workspace"], "D:/work/example");

        let steered = request(
            &mut server,
            "req_turn_steer",
            AppRequest::TurnSteer {
                turn_id: turn_id.clone(),
                input: "adjust direction".to_string(),
            },
        );
        assert_eq!(accepted_body(steered)["input"]["kind"], "steer");

        let interrupted = request(
            &mut server,
            "req_turn_interrupt",
            AppRequest::TurnInterrupt { turn_id },
        );
        let body = accepted_body(interrupted);
        assert_eq!(body["thread"]["status"], "Interrupted");
        assert_eq!(body["turn"]["status"], "Interrupted");
        assert_eq!(body["runtime_jobs"][0]["status"], "interrupted");
    }

    #[test]
    fn daemon_runs_next_queued_runtime_job_and_updates_job_state() {
        let workspace = temp_workspace("runtime-worker");
        let daemon = KernelDaemon::in_memory();
        let mut server = initialized_server_with_daemon(daemon.clone());
        let thread_id = start_thread_with_workspace(&mut server, workspace.to_string_lossy());

        let started = request(
            &mut server,
            "req_turn_start",
            AppRequest::TurnStart {
                client_thread_id: thread_id.clone(),
                input: "start runtime worker".to_string(),
            },
        );
        let body = accepted_body(started);
        assert_eq!(body["runtime_job"]["status"], "queued");

        let report = daemon
            .run_next_runtime_job(ScriptedModelClient::command_then_final(&workspace))
            .unwrap()
            .expect("queued runtime job");

        assert_eq!(report.status, ThreadStatus::Completed);
        assert!(report.final_submitted);
        let read = request(
            &mut server,
            "req_thread_read",
            AppRequest::ThreadRead {
                client_thread_id: thread_id,
            },
        );
        let body = accepted_body(read);
        assert_eq!(body["runtime_jobs"][0]["status"], "completed");
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn thread_read_includes_runtime_artifact_and_evidence_projections() {
        let workspace = temp_workspace("runtime-worker-projections");
        let daemon = KernelDaemon::in_memory();
        let mut server = initialized_server_with_daemon(daemon.clone());
        let thread_id = start_thread_with_workspace(&mut server, workspace.to_string_lossy());
        request(
            &mut server,
            "req_turn_start",
            AppRequest::TurnStart {
                client_thread_id: thread_id.clone(),
                input: "start runtime worker".to_string(),
            },
        );

        daemon
            .run_next_runtime_job(ScriptedModelClient::patch_then_final(&workspace))
            .unwrap()
            .expect("queued runtime job");

        let read = request(
            &mut server,
            "req_thread_read",
            AppRequest::ThreadRead {
                client_thread_id: thread_id,
            },
        );
        let body = accepted_body(read);
        assert_eq!(body["artifacts"].as_array().unwrap().len(), 1);
        assert!(
            !body["evidence"].as_array().unwrap().is_empty(),
            "thread/read should project evidence records for app clients"
        );
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn daemon_runtime_job_factory_receives_queued_job_before_running() {
        let workspace = temp_workspace("runtime-worker-factory");
        let daemon = KernelDaemon::in_memory();
        let mut server = initialized_server_with_daemon(daemon.clone());
        let thread_id = start_thread_with_workspace(&mut server, workspace.to_string_lossy());
        request(
            &mut server,
            "req_turn_start",
            AppRequest::TurnStart {
                client_thread_id: thread_id.clone(),
                input: "start runtime worker through factory".to_string(),
            },
        );
        let mut seen_job = None;

        let report = daemon
            .run_next_runtime_job_with_factory(|job| {
                seen_job = Some(job.clone());
                Ok(ScriptedModelClient::command_then_final(&workspace))
            })
            .unwrap()
            .expect("queued runtime job");

        assert_eq!(report.status, ThreadStatus::Completed);
        let job = seen_job.expect("factory saw runtime job");
        assert_eq!(job.client_thread_id, thread_id);
        assert_eq!(job.workspace, workspace.to_string_lossy());
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn daemon_shutdown_waits_for_background_runtime_worker() {
        let workspace = temp_workspace("background-runtime-worker");
        let daemon = KernelDaemon::in_memory();
        let mut server = initialized_server_with_daemon(daemon.clone());
        let thread_id = start_thread_with_workspace(&mut server, workspace.to_string_lossy());
        let started = request(
            &mut server,
            "req_turn_start",
            AppRequest::TurnStart {
                client_thread_id: thread_id.clone(),
                input: "start background runtime worker".to_string(),
            },
        );
        let runtime_job_id = accepted_body(started)["runtime_job"]["runtime_job_id"]
            .as_str()
            .unwrap()
            .to_string();

        daemon
            .spawn_runtime_job_worker(
                &runtime_job_id,
                ScriptedModelClient::command_then_final(&workspace),
                RuntimeConfig::workspace_write(&workspace),
            )
            .unwrap();
        let shutdown = daemon.shutdown().unwrap();

        assert_eq!(shutdown.joined_runtime_workers, 1);
        assert!(shutdown.failed_runtime_workers.is_empty());
        let read = request(
            &mut server,
            "req_thread_read",
            AppRequest::ThreadRead {
                client_thread_id: thread_id,
            },
        );
        assert_eq!(
            accepted_body(read)["runtime_jobs"][0]["status"],
            "completed"
        );
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn daemon_spawns_next_background_runtime_job_with_factory() {
        let workspace = temp_workspace("background-runtime-worker-factory");
        let daemon = KernelDaemon::in_memory();
        let mut server = initialized_server_with_daemon(daemon.clone());
        let thread_id = start_thread_with_workspace(&mut server, workspace.to_string_lossy());
        request(
            &mut server,
            "req_turn_start",
            AppRequest::TurnStart {
                client_thread_id: thread_id.clone(),
                input: "start background runtime worker through factory".to_string(),
            },
        );
        let mut seen_job = None;

        let runtime_job_id = daemon
            .spawn_next_runtime_job_worker_with_factory(|job| {
                seen_job = Some(job.clone());
                Ok(ScriptedModelClient::command_then_final(&workspace))
            })
            .unwrap()
            .expect("queued runtime job");
        let shutdown = daemon.shutdown().unwrap();

        assert_eq!(shutdown.joined_runtime_workers, 1);
        assert!(shutdown.failed_runtime_workers.is_empty());
        let job = seen_job.expect("factory saw runtime job");
        assert_eq!(job.client_thread_id, thread_id.clone());
        let read = request(
            &mut server,
            "req_thread_read_after_factory_worker",
            AppRequest::ThreadRead {
                client_thread_id: thread_id,
            },
        );
        let body = accepted_body(read);
        assert_eq!(body["runtime_jobs"][0]["runtime_job_id"], runtime_job_id);
        assert_eq!(body["runtime_jobs"][0]["status"], "completed");
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn daemon_requeues_runtime_job_when_runtime_waits_for_background_tool() {
        let workspace = temp_workspace("background-tool-requeue");
        let daemon = KernelDaemon::in_memory();
        let mut server = initialized_server_with_daemon(daemon.clone());
        let thread_id = start_thread_with_workspace(&mut server, workspace.to_string_lossy());
        let started = request(
            &mut server,
            "req_turn_start",
            AppRequest::TurnStart {
                client_thread_id: thread_id.clone(),
                input: "start runtime worker that waits on a background tool".to_string(),
            },
        );
        let body = accepted_body(started);
        let runtime_job_id = body["runtime_job"]["runtime_job_id"]
            .as_str()
            .unwrap()
            .to_string();

        let job = daemon.mark_runtime_job_running(&runtime_job_id).unwrap();
        daemon
            .finish_runtime_job(
                &runtime_job_id,
                &RuntimeRunReport {
                    thread_id: job.agent_thread_id,
                    task_id: "task_background_tool".to_string(),
                    status: ThreadStatus::WaitingTool,
                    provider_stream_session_ids: Vec::new(),
                    tool_results: Vec::new(),
                    artifacts: Vec::new(),
                    final_submitted: false,
                    events: 1,
                },
            )
            .unwrap();

        let read = request(
            &mut server,
            "req_thread_read_after_waiting_tool",
            AppRequest::ThreadRead {
                client_thread_id: thread_id,
            },
        );
        let body = accepted_body(read);
        assert_eq!(body["runtime_jobs"][0]["runtime_job_id"], runtime_job_id);
        assert_eq!(body["runtime_jobs"][0]["status"], "queued");
        assert!(body["runtime_jobs"][0]["last_error"].is_null());
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn daemon_marks_runtime_job_failed_when_worker_returns_error() {
        struct FailingModelClient;

        impl ModelClient for FailingModelClient {
            fn next(&mut self, _request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
                Err(AgentOsError::Validation("model exploded".to_string()))
            }
        }

        let workspace = temp_workspace("runtime-worker-failure");
        let daemon = KernelDaemon::in_memory();
        let mut server = initialized_server_with_daemon(daemon.clone());
        let thread_id = start_thread_with_workspace(&mut server, workspace.to_string_lossy());
        let started = request(
            &mut server,
            "req_turn_start",
            AppRequest::TurnStart {
                client_thread_id: thread_id.clone(),
                input: "start failing runtime worker".to_string(),
            },
        );
        assert_eq!(accepted_body(started)["runtime_job"]["status"], "queued");

        let error = daemon.run_next_runtime_job(FailingModelClient).unwrap_err();

        assert!(error.to_string().contains("model exploded"));
        let read = request(
            &mut server,
            "req_thread_read",
            AppRequest::ThreadRead {
                client_thread_id: thread_id,
            },
        );
        let body = accepted_body(read);
        assert_eq!(body["runtime_jobs"][0]["status"], "failed");
        assert!(body["runtime_jobs"][0]["last_error"]
            .as_str()
            .unwrap()
            .contains("model exploded"));
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn stats_read_uses_projection_snapshot() {
        let mut server = initialized_server();

        let response = request(
            &mut server,
            "req_stats",
            AppRequest::StatsRead {
                query: StatsQuery::default(),
            },
        );

        let snapshot: StatsSnapshot =
            serde_json::from_value(accepted_body(response)["snapshot"].clone()).unwrap();
        assert_eq!(snapshot.provider_calls, 0);
    }

    #[test]
    fn subscription_stays_in_app_server_protocol_layer() {
        let mut server = initialized_server();

        let response = request(
            &mut server,
            "req_subscribe",
            AppRequest::Subscribe {
                cursor: Some(ProjectionCursor {
                    last_event_ordinal: 4,
                }),
            },
        );

        assert_eq!(
            accepted_body(response)["cursor"]["last_event_ordinal"],
            serde_json::json!(4)
        );
    }

    #[test]
    fn daemon_notifications_replay_projection_changes_after_cursor() {
        let daemon = KernelDaemon::in_memory();
        let mut server = initialized_server_with_daemon(daemon.clone());
        let thread_id = start_thread(&mut server);

        let notifications = daemon
            .app_notifications_since(&ProjectionCursor {
                last_event_ordinal: 0,
            })
            .unwrap();

        assert!(notifications.iter().any(|envelope| matches!(
            &envelope.notification,
            agent_os_sys::AppNotification::ThreadChanged(thread)
                if thread.client_thread_id == thread_id
        )));
        let cursor = notifications.last().unwrap().cursor.clone();
        assert!(daemon.app_notifications_since(&cursor).unwrap().is_empty());

        let started = request(
            &mut server,
            "req_turn_start",
            AppRequest::TurnStart {
                client_thread_id: thread_id,
                input: "start work".to_string(),
            },
        );
        let turn_id = accepted_body(started)["turn"]["turn_id"]
            .as_str()
            .unwrap()
            .to_string();

        let notifications = daemon.app_notifications_since(&cursor).unwrap();

        assert!(notifications.iter().any(|envelope| matches!(
            &envelope.notification,
            agent_os_sys::AppNotification::TurnStarted(turn) if turn.turn_id == turn_id
        )));
        assert!(notifications.iter().any(|envelope| matches!(
            &envelope.notification,
            agent_os_sys::AppNotification::AgentMessageDelta(item)
                if item.turn_id.as_deref() == Some(turn_id.as_str())
        )));
    }

    #[test]
    fn sqlite_daemon_replays_projection_after_restart() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "agent-os-kerneld-{}-{unique}.sqlite",
            std::process::id()
        ));
        {
            let daemon = KernelDaemon::open_sqlite(&path).unwrap();
            let mut server = AppServer::new(daemon);
            let response = request(&mut server, "req_init", AppRequest::Initialize);
            assert!(matches!(response.response, AppResponse::Accepted(_)));
            start_thread(&mut server);
        }

        {
            let daemon = KernelDaemon::open_sqlite(&path).unwrap();
            let mut server = AppServer::new(daemon);
            let response = request(&mut server, "req_init", AppRequest::Initialize);
            assert!(matches!(response.response, AppResponse::Accepted(_)));
            let listed = request(
                &mut server,
                "req_thread_list",
                AppRequest::ThreadList {
                    archived: Some(false),
                },
            );

            assert_eq!(
                accepted_body(listed)["threads"].as_array().unwrap().len(),
                1
            );
        }
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_daemon_replays_resource_sessions_after_restart() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "agent-os-kerneld-resource-sessions-{}-{unique}.sqlite",
            std::process::id()
        ));
        let thread_id;
        let session_id;
        {
            let daemon = KernelDaemon::open_sqlite(&path).unwrap();
            let mut server = initialized_server_with_daemon(daemon);
            thread_id = start_thread(&mut server);
            let opened = request(
                &mut server,
                "req_resource_open",
                AppRequest::ResourceSessionOpen {
                    resource_type: ResourceSessionType::Terminal,
                    client_thread_id: Some(thread_id.clone()),
                    lease_expires_at: None,
                    payload: serde_json::json!({"cwd": "D:/work/example"}),
                },
            );
            session_id = accepted_body(opened)["resource_session"]["session_id"]
                .as_str()
                .unwrap()
                .to_string();
            request(
                &mut server,
                "req_resource_close",
                AppRequest::ResourceSessionClose {
                    session_id: session_id.clone(),
                },
            );
        }

        {
            let daemon = KernelDaemon::open_sqlite(&path).unwrap();
            let mut server = initialized_server_with_daemon(daemon);
            let read = request(
                &mut server,
                "req_thread_read_resources_after_restart",
                AppRequest::ThreadRead {
                    client_thread_id: thread_id,
                },
            );
            assert!(accepted_body(read)["resources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|resource| resource["session_id"] == session_id
                    && resource["status"] == "closed"));
        }
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_daemon_replays_automation_schedules_and_runs_after_restart() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "agent-os-kerneld-automation-{}-{unique}.sqlite",
            std::process::id()
        ));
        let thread_id;
        let schedule_id;
        let run_id;
        {
            let daemon = KernelDaemon::open_sqlite(&path).unwrap();
            let mut server = initialized_server_with_daemon(daemon.clone());
            thread_id = start_thread(&mut server);
            let created = request(
                &mut server,
                "req_automation_schedule_create",
                AppRequest::AutomationScheduleCreate {
                    name: "restart wakeup".to_string(),
                    kind: AutomationScheduleKind::ThreadWakeup,
                    target_thread_id: Some(thread_id.clone()),
                    workspace: None,
                    prompt: "resume after restart".to_string(),
                    next_run_at: Some("2026-06-30T00:00:00Z".to_string()),
                    interval_seconds: None,
                    payload: serde_json::json!({"source": "sqlite-test"}),
                },
            );
            schedule_id = accepted_body(created)["automation_schedule"]["schedule_id"]
                .as_str()
                .unwrap()
                .to_string();
            let runs = daemon
                .run_due_automations_at("2026-06-30T00:00:01Z")
                .unwrap();
            run_id = runs[0].run_id.clone();
        }

        {
            let daemon = KernelDaemon::open_sqlite(&path).unwrap();
            assert!(daemon
                .kernel()
                .store()
                .automation_schedules()
                .unwrap()
                .iter()
                .any(|schedule| schedule.schedule_id == schedule_id
                    && schedule.next_run_at.is_none()));
            assert!(daemon
                .kernel()
                .store()
                .automation_runs()
                .unwrap()
                .iter()
                .any(|run| run.run_id == run_id && run.schedule_id == schedule_id));
            let mut server = initialized_server_with_daemon(daemon);
            let read = request(
                &mut server,
                "req_thread_read_automation_after_restart",
                AppRequest::ThreadRead {
                    client_thread_id: thread_id,
                },
            );
            let body = accepted_body(read);
            assert!(body["automation_runs"]
                .as_array()
                .unwrap()
                .iter()
                .any(|run| run["run_id"] == run_id));
            assert_eq!(body["runtime_jobs"][0]["status"], "queued");
        }
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_daemon_replays_runtime_jobs_after_restart() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "agent-os-kerneld-runtime-jobs-{}-{unique}.sqlite",
            std::process::id()
        ));
        let workspace = temp_workspace("runtime-job-restart");
        let thread_id;
        {
            let daemon = KernelDaemon::open_sqlite(&path).unwrap();
            let mut server = initialized_server_with_daemon(daemon.clone());
            thread_id = start_thread_with_workspace(&mut server, workspace.to_string_lossy());
            request(
                &mut server,
                "req_turn_start",
                AppRequest::TurnStart {
                    client_thread_id: thread_id.clone(),
                    input: "run durable runtime job".to_string(),
                },
            );
            daemon
                .run_next_runtime_job(ScriptedModelClient::command_then_final(&workspace))
                .unwrap()
                .expect("queued runtime job");
            let read = request(
                &mut server,
                "req_thread_read",
                AppRequest::ThreadRead {
                    client_thread_id: thread_id.clone(),
                },
            );
            assert_eq!(
                accepted_body(read)["runtime_jobs"][0]["status"],
                "completed"
            );
        }

        {
            let daemon = KernelDaemon::open_sqlite(&path).unwrap();
            let mut server = initialized_server_with_daemon(daemon);
            let read = request(
                &mut server,
                "req_thread_read",
                AppRequest::ThreadRead {
                    client_thread_id: thread_id,
                },
            );

            assert_eq!(
                accepted_body(read)["runtime_jobs"][0]["status"],
                "completed"
            );
        }
        fs::remove_file(path).unwrap();
        fs::remove_dir_all(workspace).unwrap();
    }

    fn initialized_server() -> AppServer<KernelDaemon> {
        initialized_server_with_daemon(KernelDaemon::in_memory())
    }

    fn initialized_server_with_daemon(daemon: KernelDaemon) -> AppServer<KernelDaemon> {
        let mut server = AppServer::new(daemon);
        let response = request(&mut server, "req_init", AppRequest::Initialize);
        assert!(matches!(response.response, AppResponse::Accepted(_)));
        server
    }

    fn start_thread(server: &mut AppServer<KernelDaemon>) -> String {
        start_thread_with_workspace(server, "D:/work/example")
    }

    fn start_thread_with_workspace(
        server: &mut AppServer<KernelDaemon>,
        workspace: impl Into<String>,
    ) -> String {
        let response = request(
            server,
            "req_thread_start",
            AppRequest::ThreadStart {
                goal: "run a turn".to_string(),
                workspace: Some(workspace.into()),
            },
        );
        accepted_body(response)["thread"]["client_thread_id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn request(
        server: &mut AppServer<KernelDaemon>,
        request_id: &str,
        request: AppRequest,
    ) -> agent_os_sys::AppResponseEnvelope {
        server.handle_envelope(AppRequestEnvelope {
            request_id: request_id.to_string(),
            client: human_client(),
            request,
        })
    }

    fn accepted_body(response: agent_os_sys::AppResponseEnvelope) -> Value {
        match response.response {
            AppResponse::Accepted(body) => body,
            AppResponse::Rejected { code, message } => {
                panic!("request rejected: {code}: {message}")
            }
        }
    }

    fn human_client() -> ClientConnection {
        ClientConnection {
            client_id: "human_1".to_string(),
            client_name: "Codex Desktop".to_string(),
            client_kind: ClientKind::DesktopApp,
            authority: SecurityLevel::HUMAN_ROOT,
            connected_at: "2026-06-30T00:00:00Z".to_string(),
        }
    }

    fn temp_workspace(label: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "agent-os-kerneld-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[derive(Debug, Clone)]
    struct ScriptedModelClient {
        steps: VecDeque<ScriptedStep>,
    }

    #[derive(Debug, Clone)]
    enum ScriptedStep {
        Command { workspace: std::path::PathBuf },
        Patch { workspace: std::path::PathBuf },
        Final,
    }

    impl ScriptedModelClient {
        fn command_then_final(workspace: &std::path::Path) -> Self {
            Self {
                steps: VecDeque::from([
                    ScriptedStep::Command {
                        workspace: workspace.to_path_buf(),
                    },
                    ScriptedStep::Final,
                ]),
            }
        }

        fn patch_then_final(workspace: &std::path::Path) -> Self {
            Self {
                steps: VecDeque::from([
                    ScriptedStep::Patch {
                        workspace: workspace.to_path_buf(),
                    },
                    ScriptedStep::Final,
                ]),
            }
        }
    }

    impl ModelClient for ScriptedModelClient {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            let step = self.steps.pop_front().ok_or_else(|| {
                AgentOsError::Validation("scripted runtime worker model exhausted".to_string())
            })?;
            let action = match step {
                ScriptedStep::Command { workspace } => ModelAction::ToolCall(ToolAction::new(
                    "run_command",
                    serde_json::json!({
                        "program": env::current_exe().unwrap().to_string_lossy(),
                        "args": ["--help"],
                        "cwd": workspace.to_string_lossy(),
                    }),
                    4,
                    Some("runtime worker command evidence was captured".to_string()),
                )),
                ScriptedStep::Patch { workspace } => ModelAction::ToolCall(ToolAction::new(
                    "apply_patch",
                    serde_json::json!({
                        "workspace_root": workspace.to_string_lossy(),
                        "patch": "*** Begin Patch\n*** Add File: projection-artifact.md\n+runtime projection artifact\n*** End Patch\n",
                    }),
                    4,
                    Some("runtime worker artifact was captured".to_string()),
                )),
                ScriptedStep::Final => {
                    let evidence_map = request
                        .context
                        .tool_results
                        .iter()
                        .filter_map(|result| {
                            result
                                .evidence_claim
                                .as_ref()
                                .map(|claim| EvidenceMapEntry {
                                    claim: claim.clone(),
                                    evidence_refs: result.evidence_ids.clone(),
                                })
                        })
                        .collect();
                    ModelAction::Final {
                        submission: FinalSubmission {
                            summary: "runtime worker completed".to_string(),
                            changed_artifacts: Vec::new(),
                            evidence_map,
                            unverified_claims: Vec::new(),
                            known_risks: Vec::new(),
                            tests_run: vec!["test binary --help".to_string()],
                            tests_not_run: Vec::new(),
                            approvals: Vec::new(),
                        },
                    }
                }
            };
            Ok(ModelTurnResponse {
                actions: vec![action],
                usage: ProviderUsage::default(),
            })
        }
    }
}
