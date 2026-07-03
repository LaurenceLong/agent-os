use agent_os_store::{ArtifactBlobStore, EvidenceBlobStore, InMemoryStore, KernelStore};
use agent_os_sys::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::process::{Child, ChildStdin};
use std::sync::{Arc, Mutex, RwLock};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KernelState {
    pub goals: HashMap<String, Goal>,
    pub tasks: HashMap<String, Task>,
    pub threads: HashMap<String, AgentControlBlock>,
    pub agent_invocations: HashMap<String, AgentInvocation>,
    pub thread_forks: HashMap<String, ThreadForkRecord>,
    pub thread_rollbacks: HashMap<String, ThreadRollbackRecord>,
    pub agent_hooks: HashMap<String, AgentHook>,
    pub agent_control_commands: HashMap<String, AgentControlCommand>,
    pub permission_requests: HashMap<String, PermissionRequest>,
    pub permission_grants: HashMap<String, PermissionGrant>,
    pub blackboard_entries: HashMap<String, BlackboardEntry>,
    pub blackboard_channels: HashMap<String, BlackboardChannel>,
    pub context_snapshots: HashMap<String, ContextSnapshot>,
    pub role_profiles: HashMap<String, RoleProfile>,
    pub permission_profiles: HashMap<String, PermissionProfile>,
    pub sandbox_profiles: HashMap<String, SandboxProfile>,
    pub scheduler_policies: HashMap<String, SchedulerPolicy>,
    pub routing_policies: HashMap<String, RoutingPolicy>,
    pub provider_profiles: HashMap<String, ProviderProfile>,
    pub model_aliases: HashMap<String, ModelAlias>,
    pub provider_route_decisions: HashMap<String, ProviderRouteDecision>,
    pub provider_stream_sessions: HashMap<String, ProviderStreamSession>,
    pub process_sessions: HashMap<String, ProcessSession>,
    pub process_output_chunks: Vec<ProcessOutputChunk>,
    pub process_stdin_writes: Vec<ProcessStdinWrite>,
    pub communication_profiles: HashMap<String, CommunicationProfile>,
    pub capabilities: HashMap<String, CapabilityToken>,
    pub tool_descriptors: HashMap<String, ToolDescriptor>,
    pub tool_plans: HashMap<String, ToolPlan>,
    pub tool_invocations: HashMap<String, ToolInvocation>,
    pub environments: HashMap<String, ExecutionEnvironment>,
    pub environment_leases: HashMap<String, EnvironmentLease>,
    pub resource_leases: HashMap<String, ResourceLease>,
    pub resource_sessions: HashMap<String, ResourceSession>,
    pub automation_schedules: HashMap<String, AutomationSchedule>,
    pub automation_runs: HashMap<String, AutomationRun>,
    pub budget_ledgers: HashMap<String, BudgetLedger>,
    pub messages: HashMap<String, AgentMessage>,
    pub mementos: HashMap<String, MementoFragment>,
    pub artifacts: HashMap<String, Artifact>,
    pub evidence: HashMap<String, Evidence>,
    pub reviews: HashMap<String, Review>,
    pub review_findings: HashMap<String, ReviewFinding>,
    pub verifications: HashMap<String, Verification>,
    pub approvals: HashMap<String, Approval>,
    pub audit_events: HashMap<String, AuditEvent>,
    pub locks: HashMap<String, Lock>,
    pub memory_records: HashMap<String, MemoryRecord>,
    pub package_installs: HashMap<String, PackageInstallRecord>,
    pub package_contributions: HashMap<String, PackageContributionRecord>,
    pub instruction_documents: HashMap<String, InstructionDocument>,
    pub skill_definitions: HashMap<String, SkillDefinition>,
    pub command_definitions: HashMap<String, CommandDefinition>,
    pub mcp_servers: HashMap<String, McpServerSpec>,
    pub mcp_tools: HashMap<String, McpToolDefinition>,
    pub imported_agent_profiles: HashMap<String, ImportedAgentProfile>,
    pub final_submissions: HashMap<String, FinalSubmission>,
    pub reconciliation_reports: HashMap<String, crate::recovery::ReconciliationReport>,
    pub context_compactions: HashMap<String, ContextCompaction>,
    pub ready_queue: VecDeque<String>,
}

#[derive(Clone)]
pub struct Kernel {
    pub(crate) store: Arc<dyn KernelStore>,
    pub(crate) artifact_blobs: Option<Arc<dyn ArtifactBlobStore>>,
    pub(crate) evidence_blobs: Option<Arc<dyn EvidenceBlobStore>>,
    pub(crate) state: Arc<RwLock<KernelState>>,
    pub(crate) tool_workers: Arc<Mutex<HashMap<String, ToolWorkerRecord>>>,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolWorkerRecord {
    pub call_id: String,
    pub tool_name: String,
    pub started_at: String,
    pub child: Option<Arc<Mutex<Child>>>,
    pub stdin: Option<Arc<Mutex<ChildStdin>>>,
    pub output: ToolWorkerOutput,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ToolWorkerOutput {
    pub stdout: ToolStreamOutput,
    pub stderr: ToolStreamOutput,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ToolStreamOutput {
    pub head: Vec<u8>,
    pub tail: Vec<u8>,
    pub bytes: usize,
    pub truncated: bool,
    pub spool_path: Option<String>,
}

impl fmt::Debug for Kernel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.state.read().map_err(|_| fmt::Error)?;
        f.debug_struct("Kernel")
            .field("goals", &state.goals.len())
            .field("tasks", &state.tasks.len())
            .field("threads", &state.threads.len())
            .field("artifact_blobs", &self.artifact_blobs.is_some())
            .field("evidence_blobs", &self.evidence_blobs.is_some())
            .field(
                "tool_workers",
                &self
                    .tool_workers
                    .lock()
                    .map(|workers| workers.len())
                    .unwrap_or(0),
            )
            .finish()
    }
}

impl Kernel {
    pub fn new() -> Self {
        Self::with_store(InMemoryStore::new())
    }

    pub fn with_store<S>(store: S) -> Self
    where
        S: KernelStore + 'static,
    {
        let kernel = Self {
            store: Arc::new(store),
            artifact_blobs: None,
            evidence_blobs: None,
            state: Arc::new(RwLock::new(KernelState::default())),
            tool_workers: Arc::new(Mutex::new(HashMap::new())),
        };
        kernel.install_core_profiles();
        kernel
    }

    pub fn with_replayed_store<S>(store: S) -> AgentOsResult<Self>
    where
        S: KernelStore + 'static,
    {
        let events = store.all_events()?;
        seed_id_allocator_from_events(&events);
        let kernel = Self::with_store(store);
        {
            let mut state = kernel.write_state()?;
            clear_event_projection(&mut state);
        }
        for event in &events {
            kernel.apply_event(event)?;
        }
        kernel.store.rebuild_projections()?;
        Ok(kernel)
    }

    pub fn with_blob_stores<A, E>(mut self, artifact_blobs: A, evidence_blobs: E) -> Self
    where
        A: ArtifactBlobStore + 'static,
        E: EvidenceBlobStore + 'static,
    {
        self.artifact_blobs = Some(Arc::new(artifact_blobs));
        self.evidence_blobs = Some(Arc::new(evidence_blobs));
        self
    }

    pub fn from_events(events: &[EventEnvelope]) -> AgentOsResult<Self> {
        seed_id_allocator_from_events(events);
        let kernel = Self::new();
        {
            let mut state = kernel.write_state()?;
            clear_event_projection(&mut state);
        }
        for event in events {
            kernel.store.append_projected(event.clone())?;
            kernel.apply_event(event)?;
        }
        Ok(kernel)
    }

    pub fn store(&self) -> Arc<dyn KernelStore> {
        self.store.clone()
    }

    pub fn state_snapshot(&self) -> AgentOsResult<KernelState> {
        Ok(self.read_state()?.clone())
    }

    pub fn events(&self) -> AgentOsResult<Vec<EventEnvelope>> {
        self.store.all_events()
    }
}

fn clear_event_projection(state: &mut KernelState) {
    state.goals.clear();
    state.tasks.clear();
    state.threads.clear();
    state.agent_invocations.clear();
    state.thread_forks.clear();
    state.thread_rollbacks.clear();
    state.agent_hooks.clear();
    state.agent_control_commands.clear();
    state.permission_requests.clear();
    state.permission_grants.clear();
    state.blackboard_entries.clear();
    state.blackboard_channels.clear();
    state.context_snapshots.clear();
    state.communication_profiles.clear();
    state.provider_route_decisions.clear();
    state.provider_stream_sessions.clear();
    state.process_sessions.clear();
    state.process_output_chunks.clear();
    state.process_stdin_writes.clear();
    state.tool_invocations.clear();
    state.capabilities.clear();
    state.environments.clear();
    state.environment_leases.clear();
    state.resource_leases.clear();
    state.resource_sessions.clear();
    state.budget_ledgers.clear();
    state.messages.clear();
    state.mementos.clear();
    state.artifacts.clear();
    state.evidence.clear();
    state.reviews.clear();
    state.review_findings.clear();
    state.verifications.clear();
    state.approvals.clear();
    state.audit_events.clear();
    state.locks.clear();
    state.memory_records.clear();
    state.package_installs.clear();
    state.package_contributions.clear();
    state.instruction_documents.clear();
    state.skill_definitions.clear();
    state.command_definitions.clear();
    state.mcp_servers.clear();
    state.mcp_tools.clear();
    state.imported_agent_profiles.clear();
    state.final_submissions.clear();
    state.reconciliation_reports.clear();
    state.context_compactions.clear();
    state.ready_queue.clear();
}

impl Default for Kernel {
    fn default() -> Self {
        Self::new()
    }
}
