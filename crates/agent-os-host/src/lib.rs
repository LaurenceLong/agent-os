//! Long-running Agent-OS kernel host service.

mod app_handlers;
mod app_projection;
mod notifications;
mod runtime_jobs;
mod runtime_model;
mod stdio;
mod types;

pub use agent_os_app_server::AppServer;
use agent_os_config::{AgentOsPaths, ProjectRuntimePaths};
use agent_os_ecosystem::{
    discover_ecosystem, expand_command_template, EcosystemCatalog, EcosystemDiscoverOptions,
    EcosystemImportReport,
};
use agent_os_kernel::Kernel;
use agent_os_store::LocalBlobStore;
use agent_os_store_sqlite::SqliteStore;
use agent_os_sys::{
    now_rfc3339, AgentOsError, AgentOsResult, AutomationRun, AutomationScheduleStatus,
    ClientConnection, ClientKind, ClientThread, ModelCapabilities, ModelLimit, SecurityLevel,
    StatsSnapshot, ThreadStatus, TurnInputKind, TurnRecord,
};
use agent_os_thread::{
    ModelClient, RuntimeConfig, RuntimeJob, RuntimeJobRecord, RuntimeRunReport, ThreadRuntime,
};
pub use runtime_model::{
    ExternalRuntimeModelConfig, HostRuntimeModelConfig, ProviderRuntimeModelConfig,
};
use std::collections::BTreeMap;
use std::fmt;
use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
pub use stdio::{run_stdio_host, HostArgs};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use types::RuntimeWorkerJoinHandle;
pub use types::{HostReplaySummary, HostShutdownReport};

#[derive(Clone)]
pub struct AgentOsHost {
    kernel: Kernel,
    runtime_jobs: Arc<Mutex<BTreeMap<String, RuntimeJobRecord>>>,
    runtime_workers: Arc<Mutex<BTreeMap<String, RuntimeWorkerJoinHandle>>>,
    runtime_model_config: Option<HostRuntimeModelConfig>,
}

impl fmt::Debug for AgentOsHost {
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
        f.debug_struct("AgentOsHost")
            .field("kernel", &self.kernel)
            .field("runtime_jobs", &runtime_job_count)
            .field("runtime_workers", &runtime_worker_count)
            .finish()
    }
}

impl AgentOsHost {
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

    pub fn open_global(workspace: impl AsRef<Path>) -> AgentOsResult<Self> {
        let paths = AgentOsPaths::resolve()?;
        let runtime = paths.project_runtime_paths(workspace)?;
        paths.create_runtime_dirs(&runtime)?;
        Self::open_sqlite_with_runtime_paths(&runtime)
    }

    pub fn open_sqlite_with_runtime_paths(runtime: &ProjectRuntimePaths) -> AgentOsResult<Self> {
        let kernel = Kernel::with_replayed_store(SqliteStore::open(&runtime.state_db)?)?
            .with_blob_stores(
                LocalBlobStore::new(&runtime.artifact_blobs)?,
                LocalBlobStore::new(&runtime.evidence_blobs)?,
            );
        Self::try_new(kernel)
    }

    pub fn kernel(&self) -> &Kernel {
        &self.kernel
    }

    pub fn event_count(&self) -> AgentOsResult<usize> {
        Ok(self.kernel.events()?.len())
    }

    pub fn replay_summary(&self) -> AgentOsResult<HostReplaySummary> {
        let replayed = Kernel::from_events(&self.kernel.events()?)?;
        let state = replayed.state_snapshot()?;
        Ok(HostReplaySummary {
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
            let Some(thread_id) = &schedule.target_thread_id else {
                return Err(AgentOsError::Validation(format!(
                    "thread wakeup automation {} has no target_thread_id",
                    schedule.schedule_id
                )));
            };
            self.thread_by_id(thread_id)?;
            let run = self
                .kernel
                .queue_automation_run(&schedule.schedule_id, next_run_at.clone())?;
            self.enqueue_thread_wakeup_job(&run)?;
            runs.push(run);
        }
        Ok(runs)
    }

    pub fn register_model_alias(
        &self,
        alias: &str,
        provider_id: &str,
        model: &str,
        capabilities: ModelCapabilities,
        limit: ModelLimit,
        provider_profile_id: &str,
    ) -> AgentOsResult<()> {
        self.kernel.register_model_alias(
            alias,
            provider_id,
            model,
            capabilities,
            limit,
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
        self.import_workspace_ecosystem(workspace)?;
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
        self.import_workspace_ecosystem(&config.workspace_root)?;
        self.prepare_runtime_job_thread_for_running(runtime_job_id)?;
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

    pub fn import_workspace_ecosystem(
        &self,
        workspace: &Path,
    ) -> AgentOsResult<EcosystemImportReport> {
        let options = EcosystemDiscoverOptions::for_workspace(workspace)?;
        let catalog = discover_ecosystem(&options)?;
        self.import_ecosystem_catalog(&catalog)
    }

    pub fn import_ecosystem_catalog(
        &self,
        catalog: &EcosystemCatalog,
    ) -> AgentOsResult<EcosystemImportReport> {
        for document in &catalog.instruction_documents {
            self.kernel.import_instruction_document(document.clone())?;
        }
        for skill in &catalog.skill_definitions {
            self.kernel.import_skill_definition(skill.clone())?;
        }
        for command in &catalog.command_definitions {
            self.kernel.import_command_definition(command.clone())?;
        }
        for profile in &catalog.imported_agent_profiles {
            self.kernel
                .register_imported_agent_profile(profile.clone())?;
        }
        for server in &catalog.mcp_servers {
            self.kernel.register_mcp_server_spec(server.clone())?;
        }
        for tool in &catalog.mcp_tools {
            self.kernel.register_mcp_tool_definition(tool.clone())?;
        }
        Ok(catalog.import_report())
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
        self.reap_finished_runtime_worker(runtime_job_id)?;
        self.ensure_runtime_job_can_spawn(runtime_job_id)?;
        let runtime_job_id = runtime_job_id.to_string();
        let worker_host = self.clone();
        let worker_job_id = runtime_job_id.clone();
        let handle = std::thread::spawn(move || {
            worker_host.run_runtime_job_with_config(&worker_job_id, model_client, config)
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

    pub fn shutdown(&self) -> AgentOsResult<HostShutdownReport> {
        let workers = {
            let mut workers = self.runtime_workers.lock().map_err(|_| {
                AgentOsError::Validation("runtime worker registry lock poisoned".to_string())
            })?;
            std::mem::take(&mut *workers)
        };
        let mut report = HostShutdownReport {
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

impl AgentOsHost {
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
            .find(|thread| thread.client_thread_id == client_thread_id && !thread.deleted)
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
        self.reap_finished_runtime_worker(&runtime_job_id)?;
        if self.runtime_worker_exists(&runtime_job_id)? {
            return Ok(Some(runtime_job_id));
        }
        self.spawn_configured_runtime_job_worker(&runtime_job_id)?;
        Ok(Some(runtime_job_id))
    }

    fn prepare_runtime_job_thread_for_running(&self, runtime_job_id: &str) -> AgentOsResult<()> {
        let job = self.runtime_job(runtime_job_id)?;
        let acb = self
            .kernel
            .state_snapshot()?
            .threads
            .get(&job.agent_thread_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {}", job.agent_thread_id)))?;
        if acb.active_turn.turn_id.as_deref() != Some(&job.turn_id) {
            return Ok(());
        }
        if acb.status == ThreadStatus::Ready
            && acb.active_turn.status == Some(agent_os_sys::TurnStatus::Completed)
        {
            self.kernel.transition_thread(
                &job.agent_thread_id,
                ThreadStatus::Running,
                Some("runtime job resumed after background tool completion".to_string()),
            )?;
        }
        Ok(())
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

    fn runtime_worker_exists(&self, runtime_job_id: &str) -> AgentOsResult<bool> {
        let workers = self.runtime_workers.lock().map_err(|_| {
            AgentOsError::Validation("runtime worker registry lock poisoned".to_string())
        })?;
        Ok(workers.contains_key(runtime_job_id))
    }

    fn reap_finished_runtime_worker(&self, runtime_job_id: &str) -> AgentOsResult<()> {
        let finished_worker = {
            let mut workers = self.runtime_workers.lock().map_err(|_| {
                AgentOsError::Validation("runtime worker registry lock poisoned".to_string())
            })?;
            if workers
                .get(runtime_job_id)
                .is_some_and(|handle| handle.is_finished())
            {
                workers.remove(runtime_job_id)
            } else {
                None
            }
        };
        if let Some(handle) = finished_worker {
            if handle.join().is_err() {
                self.fail_runtime_job(runtime_job_id, "runtime worker panicked".to_string())?;
            }
        }
        Ok(())
    }

    fn finish_runtime_job(
        &self,
        runtime_job_id: &str,
        report: &RuntimeRunReport,
    ) -> AgentOsResult<()> {
        let blocked_reason = if report.status == ThreadStatus::Blocked {
            let state = self.kernel.state_snapshot()?;
            let reason = state
                .tasks
                .get(&report.task_id)
                .and_then(|task| task.blocked_reason.clone())
                .unwrap_or_else(|| "runtime blocked without final submission".to_string());
            Some(reason)
        } else {
            None
        };
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
            } else if report.status == ThreadStatus::Blocked {
                record.block(blocked_reason.expect("blocked report has blocked reason"));
            } else {
                record.fail(format!("runtime finished with status {:?}", report.status));
            }
            record.clone()
        };
        let event_type = match record.status {
            agent_os_thread::RuntimeJobStatus::Completed => "RuntimeJobCompleted",
            agent_os_thread::RuntimeJobStatus::Blocked => "RuntimeJobBlocked",
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

#[cfg(test)]
mod tests {
    use super::*;
    use agent_os_app_server::{AppKernelService, AppServer};
    use agent_os_sys::{
        AppRequest, AppRequestEnvelope, AppResponse, AutomationScheduleKind, ClientConnection,
        ClientKind, EvidenceMapEntry, FinalSubmission, ProjectionCursor, ProviderUsage,
        ResourceSessionType, SecurityLevel, StatsQuery, StatsSnapshot, TurnStatus,
    };
    use agent_os_thread::{
        ModelAction, ModelClient, ModelTurnRequest, ModelTurnResponse, ToolAction,
    };
    use serde_json::Value;
    use std::collections::VecDeque;
    use std::env;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn app_server_runs_thread_lifecycle_updates_through_host() {
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
                client_thread_id: thread_id.clone(),
            },
        );
        assert_eq!(accepted_body(archived)["thread"]["archived"], true);

        let unarchived = request(
            &mut server,
            "req_thread_unarchive",
            AppRequest::ThreadUnarchive {
                client_thread_id: thread_id.clone(),
            },
        );
        assert_eq!(accepted_body(unarchived)["thread"]["archived"], false);

        let renamed = request(
            &mut server,
            "req_thread_name_set",
            AppRequest::ThreadNameSet {
                client_thread_id: thread_id.clone(),
                title: "GOAT protocol thread".to_string(),
            },
        );
        assert_eq!(
            accepted_body(renamed)["thread"]["title"],
            "GOAT protocol thread"
        );

        let deleted = request(
            &mut server,
            "req_thread_delete",
            AppRequest::ThreadDelete {
                client_thread_id: thread_id.clone(),
            },
        );
        let deleted_body = accepted_body(deleted);
        assert_eq!(deleted_body["thread"]["deleted"], true);
        assert_eq!(deleted_body["thread"]["archived"], true);

        let listed = request(
            &mut server,
            "req_thread_list_after_delete",
            AppRequest::ThreadList { archived: None },
        );
        assert!(accepted_body(listed)["threads"]
            .as_array()
            .unwrap()
            .is_empty());

        let read_deleted = request(
            &mut server,
            "req_thread_read_deleted",
            AppRequest::ThreadRead {
                client_thread_id: thread_id,
            },
        );
        assert!(matches!(
            read_deleted.response,
            AppResponse::Rejected { code, .. } if code == "not_found"
        ));
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
    fn host_resource_session_lifecycle_flows_through_app_server() {
        let host = AgentOsHost::in_memory();
        let mut server = initialized_server_with_host(host.clone());
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

        let notifications = host
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
    fn host_queues_due_thread_wakeup_automation_with_injected_clock() {
        let host = AgentOsHost::in_memory();
        let mut server = initialized_server_with_host(host.clone());
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

        let runs = host.run_due_automations_at("2026-06-30T00:00:01Z").unwrap();
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
        assert!(host
            .kernel()
            .store()
            .automation_schedules()
            .unwrap()
            .iter()
            .any(|schedule| schedule.schedule_id == schedule_id && schedule.next_run_at.is_none()));
    }

    #[test]
    fn host_turn_start_steer_and_interrupt_update_projection_records() {
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
    fn host_runs_next_queued_runtime_job_and_updates_job_state() {
        let workspace = temp_workspace("runtime-worker");
        let host = AgentOsHost::in_memory();
        let mut server = initialized_server_with_host(host.clone());
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

        let report = host
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
        let host = AgentOsHost::in_memory();
        let mut server = initialized_server_with_host(host.clone());
        let thread_id = start_thread_with_workspace(&mut server, workspace.to_string_lossy());
        request(
            &mut server,
            "req_turn_start",
            AppRequest::TurnStart {
                client_thread_id: thread_id.clone(),
                input: "start runtime worker".to_string(),
            },
        );

        host.run_next_runtime_job(ScriptedModelClient::patch_then_final(&workspace))
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
    fn host_runtime_job_factory_receives_queued_job_before_running() {
        let workspace = temp_workspace("runtime-worker-factory");
        let host = AgentOsHost::in_memory();
        let mut server = initialized_server_with_host(host.clone());
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

        let report = host
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
    fn host_shutdown_waits_for_background_runtime_worker() {
        let workspace = temp_workspace("background-runtime-worker");
        let host = AgentOsHost::in_memory();
        let mut server = initialized_server_with_host(host.clone());
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

        host.spawn_runtime_job_worker(
            &runtime_job_id,
            ScriptedModelClient::command_then_final(&workspace),
            RuntimeConfig::workspace_write(&workspace),
        )
        .unwrap();
        let shutdown = host.shutdown().unwrap();

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
    fn host_spawns_next_background_runtime_job_with_factory() {
        let workspace = temp_workspace("background-runtime-worker-factory");
        let host = AgentOsHost::in_memory();
        let mut server = initialized_server_with_host(host.clone());
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

        let runtime_job_id = host
            .spawn_next_runtime_job_worker_with_factory(|job| {
                seen_job = Some(job.clone());
                Ok(ScriptedModelClient::command_then_final(&workspace))
            })
            .unwrap()
            .expect("queued runtime job");
        let shutdown = host.shutdown().unwrap();

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
    fn host_requeues_runtime_job_when_runtime_waits_for_background_tool() {
        let workspace = temp_workspace("background-tool-requeue");
        let host = AgentOsHost::in_memory();
        let mut server = initialized_server_with_host(host.clone());
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

        let job = host.mark_runtime_job_running(&runtime_job_id).unwrap();
        host.finish_runtime_job(
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
    fn host_resumes_requeued_runtime_job_after_background_tool_readies_thread() {
        let workspace = temp_workspace("background-tool-requeue-resume");
        let host = AgentOsHost::in_memory();
        let mut server = initialized_server_with_host(host.clone());
        let thread_id = start_thread_with_workspace(&mut server, workspace.to_string_lossy());
        let started = request(
            &mut server,
            "req_turn_start",
            AppRequest::TurnStart {
                client_thread_id: thread_id.clone(),
                input: "resume runtime job after background tool completion".to_string(),
            },
        );
        let runtime_job_id = accepted_body(started)["runtime_job"]["runtime_job_id"]
            .as_str()
            .unwrap()
            .to_string();

        let job = host.mark_runtime_job_running(&runtime_job_id).unwrap();
        host.kernel()
            .transition_thread(
                &thread_id,
                ThreadStatus::WaitingTool,
                Some("test background tool wait".to_string()),
            )
            .unwrap();
        host.finish_runtime_job(
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
        let ready = host
            .kernel()
            .transition_thread(
                &thread_id,
                ThreadStatus::Ready,
                Some("test background tool completed".to_string()),
            )
            .unwrap();
        assert_eq!(ready.active_turn.status, Some(TurnStatus::Completed));

        host.spawn_runtime_job_worker(
            &runtime_job_id,
            ScriptedModelClient::command_then_final(&workspace),
            RuntimeConfig::workspace_write(&workspace),
        )
        .unwrap();
        let shutdown = host.shutdown().unwrap();

        assert_eq!(shutdown.joined_runtime_workers, 1);
        assert!(shutdown.failed_runtime_workers.is_empty());
        let read = request(
            &mut server,
            "req_thread_read_after_requeued_resume",
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
    fn host_reaps_finished_background_worker_before_requeued_spawn() {
        let workspace = temp_workspace("background-worker-requeue-respawn");
        let host = AgentOsHost::in_memory();
        let mut server = initialized_server_with_host(host.clone());
        let thread_id = start_thread_with_workspace(&mut server, workspace.to_string_lossy());
        let started = request(
            &mut server,
            "req_turn_start",
            AppRequest::TurnStart {
                client_thread_id: thread_id.clone(),
                input: "start runtime worker with stale finished handle".to_string(),
            },
        );
        let runtime_job_id = accepted_body(started)["runtime_job"]["runtime_job_id"]
            .as_str()
            .unwrap()
            .to_string();
        let finished_thread_id = thread_id.clone();
        let finished_handle = std::thread::spawn(move || {
            Ok(RuntimeRunReport {
                thread_id: finished_thread_id,
                task_id: "task_finished_background_worker".to_string(),
                status: ThreadStatus::WaitingTool,
                provider_stream_session_ids: Vec::new(),
                tool_results: Vec::new(),
                artifacts: Vec::new(),
                final_submitted: false,
                events: 1,
            })
        });
        while !finished_handle.is_finished() {
            std::thread::yield_now();
        }
        host.runtime_workers
            .lock()
            .unwrap()
            .insert(runtime_job_id.clone(), finished_handle);

        host.spawn_runtime_job_worker(
            &runtime_job_id,
            ScriptedModelClient::command_then_final(&workspace),
            RuntimeConfig::workspace_write(&workspace),
        )
        .unwrap();
        let shutdown = host.shutdown().unwrap();

        assert_eq!(shutdown.joined_runtime_workers, 1);
        assert!(shutdown.failed_runtime_workers.is_empty());
        let read = request(
            &mut server,
            "req_thread_read_after_respawn",
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
    fn configured_autostart_does_not_respawn_queued_job_with_live_worker() {
        let workspace = temp_workspace("background-worker-read-idempotent");
        let host = AgentOsHost::in_memory();
        let mut server = initialized_server_with_host(host.clone());
        let thread_id = start_thread_with_workspace(&mut server, workspace.to_string_lossy());
        let started = request(
            &mut server,
            "req_turn_start",
            AppRequest::TurnStart {
                client_thread_id: thread_id.clone(),
                input: "start runtime worker with live handle".to_string(),
            },
        );
        let runtime_job_id = accepted_body(started)["runtime_job"]["runtime_job_id"]
            .as_str()
            .unwrap()
            .to_string();
        host.kernel()
            .transition_thread(
                &thread_id,
                ThreadStatus::WaitingTool,
                Some("test background tool wait".to_string()),
            )
            .unwrap();
        host.kernel()
            .transition_thread(
                &thread_id,
                ThreadStatus::Ready,
                Some("test background tool ready".to_string()),
            )
            .unwrap();
        let (release_worker, wait_for_release) = std::sync::mpsc::channel();
        let live_thread_id = thread_id.clone();
        let live_handle = std::thread::spawn(move || {
            wait_for_release.recv().unwrap();
            Ok(RuntimeRunReport {
                thread_id: live_thread_id,
                task_id: "task_live_background_worker".to_string(),
                status: ThreadStatus::WaitingTool,
                provider_stream_session_ids: Vec::new(),
                tool_results: Vec::new(),
                artifacts: Vec::new(),
                final_submitted: false,
                events: 1,
            })
        });
        host.runtime_workers
            .lock()
            .unwrap()
            .insert(runtime_job_id.clone(), live_handle);
        let configured_host =
            host.clone()
                .with_runtime_model_config(HostRuntimeModelConfig::External(
                    ExternalRuntimeModelConfig {
                        program: env::current_exe().unwrap(),
                        args: vec!["--help".to_string()],
                        max_steps: 1,
                    },
                ));

        let spawned = configured_host
            .spawn_configured_runtime_job_for_ready_thread(&thread_id)
            .unwrap();

        assert_eq!(spawned.as_deref(), Some(runtime_job_id.as_str()));
        release_worker.send(()).unwrap();
        let shutdown = host.shutdown().unwrap();
        assert_eq!(shutdown.joined_runtime_workers, 1);
        assert!(shutdown.failed_runtime_workers.is_empty());
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn host_marks_runtime_job_failed_when_worker_returns_error() {
        struct FailingModelClient;

        impl ModelClient for FailingModelClient {
            fn next(&mut self, _request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
                Err(AgentOsError::Validation("model exploded".to_string()))
            }
        }

        let workspace = temp_workspace("runtime-worker-failure");
        let host = AgentOsHost::in_memory();
        let mut server = initialized_server_with_host(host.clone());
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

        let error = host.run_next_runtime_job(FailingModelClient).unwrap_err();

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
    fn host_marks_runtime_job_blocked_when_runtime_blocks_without_final() {
        let workspace = temp_workspace("runtime-worker-blocked");
        let host = AgentOsHost::in_memory();
        let mut server = initialized_server_with_host(host.clone());
        let thread_id = start_thread_with_workspace(&mut server, workspace.to_string_lossy());
        let started = request(
            &mut server,
            "req_turn_start_blocked",
            AppRequest::TurnStart {
                client_thread_id: thread_id.clone(),
                input: "start runtime worker that blocks".to_string(),
            },
        );
        let body = accepted_body(started);
        let runtime_job_id = body["runtime_job"]["runtime_job_id"]
            .as_str()
            .unwrap()
            .to_string();

        let job = host.mark_runtime_job_running(&runtime_job_id).unwrap();
        host.kernel()
            .transition_thread(
                &thread_id,
                ThreadStatus::Blocked,
                Some("runtime reached max_steps without final submission".to_string()),
            )
            .unwrap();
        host.finish_runtime_job(
            &runtime_job_id,
            &RuntimeRunReport {
                thread_id: job.agent_thread_id,
                task_id: job.turn_id,
                status: ThreadStatus::Blocked,
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
            "req_thread_read_after_blocked",
            AppRequest::ThreadRead {
                client_thread_id: thread_id,
            },
        );
        let body = accepted_body(read);
        assert_eq!(body["runtime_jobs"][0]["runtime_job_id"], runtime_job_id);
        assert_eq!(body["runtime_jobs"][0]["status"], "blocked");
        assert!(body["runtime_jobs"][0]["last_error"]
            .as_str()
            .unwrap()
            .contains("runtime blocked without final submission"));
        assert!(host.kernel().events().unwrap().iter().any(|event| {
            event.event_type == "RuntimeJobBlocked" && event.aggregate_id == runtime_job_id
        }));
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
    fn app_server_reads_provider_usage_and_permission_profiles() {
        let mut server = initialized_server();

        let usage = request(
            &mut server,
            "req_provider_usage",
            AppRequest::ProviderUsageRead {
                query: StatsQuery {
                    provider_id: Some("provider_default".to_string()),
                    ..StatsQuery::default()
                },
            },
        );
        let usage = accepted_body(usage);
        assert_eq!(usage["usage"]["query"]["provider_id"], "provider_default");
        assert_eq!(usage["usage"]["snapshot"]["provider_calls"], 0);

        let permissions = request(
            &mut server,
            "req_permission_profiles",
            AppRequest::PermissionProfileList,
        );
        let profiles = accepted_body(permissions)["permission_profiles"]
            .as_array()
            .unwrap()
            .clone();
        assert!(profiles.iter().any(|profile| {
            profile["permission_profile_id"]
                .as_str()
                .is_some_and(|id| id == "perm_producer")
        }));
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
    fn host_notifications_replay_projection_changes_after_cursor() {
        let host = AgentOsHost::in_memory();
        let mut server = initialized_server_with_host(host.clone());
        let thread_id = start_thread(&mut server);

        let notifications = host
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
        assert!(host.app_notifications_since(&cursor).unwrap().is_empty());

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

        let notifications = host.app_notifications_since(&cursor).unwrap();

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
    fn sqlite_host_replays_projection_after_restart() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "agent-os-host-{}-{unique}.sqlite",
            std::process::id()
        ));
        {
            let host = AgentOsHost::open_sqlite(&path).unwrap();
            let mut server = AppServer::new(host);
            let response = request(&mut server, "req_init", AppRequest::Initialize);
            assert!(matches!(response.response, AppResponse::Accepted(_)));
            start_thread(&mut server);
        }

        {
            let host = AgentOsHost::open_sqlite(&path).unwrap();
            let mut server = AppServer::new(host);
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
    fn sqlite_host_replays_resource_sessions_after_restart() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "agent-os-host-resource-sessions-{}-{unique}.sqlite",
            std::process::id()
        ));
        let thread_id;
        let session_id;
        {
            let host = AgentOsHost::open_sqlite(&path).unwrap();
            let mut server = initialized_server_with_host(host);
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
            let host = AgentOsHost::open_sqlite(&path).unwrap();
            let mut server = initialized_server_with_host(host);
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
    fn sqlite_host_replays_automation_schedules_and_runs_after_restart() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "agent-os-host-automation-{}-{unique}.sqlite",
            std::process::id()
        ));
        let thread_id;
        let schedule_id;
        let run_id;
        {
            let host = AgentOsHost::open_sqlite(&path).unwrap();
            let mut server = initialized_server_with_host(host.clone());
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
            let runs = host.run_due_automations_at("2026-06-30T00:00:01Z").unwrap();
            run_id = runs[0].run_id.clone();
        }

        {
            let host = AgentOsHost::open_sqlite(&path).unwrap();
            assert!(host
                .kernel()
                .store()
                .automation_schedules()
                .unwrap()
                .iter()
                .any(|schedule| schedule.schedule_id == schedule_id
                    && schedule.next_run_at.is_none()));
            assert!(host
                .kernel()
                .store()
                .automation_runs()
                .unwrap()
                .iter()
                .any(|run| run.run_id == run_id && run.schedule_id == schedule_id));
            let mut server = initialized_server_with_host(host);
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
    fn sqlite_host_replays_runtime_jobs_after_restart() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "agent-os-host-runtime-jobs-{}-{unique}.sqlite",
            std::process::id()
        ));
        let workspace = temp_workspace("runtime-job-restart");
        let thread_id;
        {
            let host = AgentOsHost::open_sqlite(&path).unwrap();
            let mut server = initialized_server_with_host(host.clone());
            thread_id = start_thread_with_workspace(&mut server, workspace.to_string_lossy());
            request(
                &mut server,
                "req_turn_start",
                AppRequest::TurnStart {
                    client_thread_id: thread_id.clone(),
                    input: "run durable runtime job".to_string(),
                },
            );
            host.run_next_runtime_job(ScriptedModelClient::command_then_final(&workspace))
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
            let host = AgentOsHost::open_sqlite(&path).unwrap();
            let mut server = initialized_server_with_host(host);
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

    fn initialized_server() -> AppServer<AgentOsHost> {
        initialized_server_with_host(AgentOsHost::in_memory())
    }

    fn initialized_server_with_host(host: AgentOsHost) -> AppServer<AgentOsHost> {
        let mut server = AppServer::new(host);
        let response = request(&mut server, "req_init", AppRequest::Initialize);
        assert!(matches!(response.response, AppResponse::Accepted(_)));
        server
    }

    fn start_thread(server: &mut AppServer<AgentOsHost>) -> String {
        start_thread_with_workspace(server, "D:/work/example")
    }

    fn start_thread_with_workspace(
        server: &mut AppServer<AgentOsHost>,
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
        server: &mut AppServer<AgentOsHost>,
        request_id: &str,
        request: AppRequest,
    ) -> agent_os_sys::AppResponseEnvelope {
        server.handle_envelope(AppRequestEnvelope {
            protocol: agent_os_sys::app_protocol_version(),
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
            client_name: "Agent-OS Desktop".to_string(),
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
            "agent-os-host-{label}-{}-{unique}",
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
                        "command": env::current_exe().unwrap().to_string_lossy(),
                        "mode": "exec",
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
                        .filter(|result| !result.evidence_ids.is_empty())
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
