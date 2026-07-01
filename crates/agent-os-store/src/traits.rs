use agent_os_sys::{
    AgentOsResult, ApprovalQueueProjection, ArtifactIndexProjection, AutomationRunProjection,
    AutomationScheduleProjection, ClientThread, EventEnvelope, EvidenceIndexProjection,
    ProjectionCheckpoint, ResourceSessionProjection, StatsQuery, StatsSnapshot, SyscallResult,
    TimelineItem, TurnRecord,
};

pub trait EventStore: Send + Sync {
    fn append(&self, event: EventEnvelope) -> AgentOsResult<()>;
    fn all_events(&self) -> AgentOsResult<Vec<EventEnvelope>>;
    fn events_by_aggregate(&self, aggregate_id: &str) -> AgentOsResult<Vec<EventEnvelope>>;
    fn event_ordinal(&self, event_id: &str) -> AgentOsResult<u64>;
}

pub trait IdempotencyStore: Send + Sync {
    fn get_syscall_result(&self, idempotency_key: &str) -> AgentOsResult<Option<SyscallResult>>;
    fn put_syscall_result(
        &self,
        idempotency_key: String,
        result: SyscallResult,
    ) -> AgentOsResult<()>;
}

pub trait KernelStore: ProjectionStore + IdempotencyStore {}

impl<T> KernelStore for T where T: ProjectionStore + IdempotencyStore {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobRecord {
    pub blob_ref: String,
    pub content_hash: String,
    pub byte_len: usize,
}

pub trait BlobStore: Send + Sync {
    fn put_blob(&self, bytes: &[u8]) -> AgentOsResult<BlobRecord>;
    fn get_blob(&self, blob_ref: &str) -> AgentOsResult<Vec<u8>>;
    fn has_blob(&self, blob_ref: &str) -> AgentOsResult<bool>;
}

pub trait ArtifactBlobStore: BlobStore {}
pub trait EvidenceBlobStore: BlobStore {}

impl<T> ArtifactBlobStore for T where T: BlobStore {}
impl<T> EvidenceBlobStore for T where T: BlobStore {}

/// Query the event log for every event whose `aggregate_type` matches.
///
/// This is the foundation read surface for the projection-style store
/// families: each family below is a typed view over a slice of the event log,
/// rebuilt into a projection by the kernel.
pub trait ProjectionStore: EventStore {
    /// All events that project into a given aggregate family (for example
    /// `"lock"`, `"resource_lease"`, `"memory"`, `"context"`).
    fn events_by_aggregate_type(&self, aggregate_type: &str) -> AgentOsResult<Vec<EventEnvelope>> {
        Ok(self
            .all_events()?
            .into_iter()
            .filter(|event| event.aggregate_type == aggregate_type)
            .collect())
    }

    fn clear_projections(&self) -> AgentOsResult<()>;
    fn append_projected(&self, event: EventEnvelope) -> AgentOsResult<u64>;
    fn project_event(&self, ordinal: u64, event: &EventEnvelope) -> AgentOsResult<()>;
    fn rebuild_projections(&self) -> AgentOsResult<()>;
    fn thread_summaries(&self) -> AgentOsResult<Vec<ClientThread>>;
    fn turn_summaries(&self) -> AgentOsResult<Vec<TurnRecord>>;
    fn timeline_items(&self, client_thread_id: Option<&str>) -> AgentOsResult<Vec<TimelineItem>>;
    fn stats_snapshot(&self, query: StatsQuery) -> AgentOsResult<StatsSnapshot>;
    fn approval_queue(&self) -> AgentOsResult<Vec<ApprovalQueueProjection>>;
    fn resource_sessions(&self) -> AgentOsResult<Vec<ResourceSessionProjection>>;
    fn automation_schedules(&self) -> AgentOsResult<Vec<AutomationScheduleProjection>>;
    fn automation_runs(&self) -> AgentOsResult<Vec<AutomationRunProjection>>;
    fn artifact_index(&self) -> AgentOsResult<Vec<ArtifactIndexProjection>>;
    fn evidence_index(&self) -> AgentOsResult<Vec<EvidenceIndexProjection>>;
    fn projection_checkpoint(
        &self,
        projection_name: &str,
    ) -> AgentOsResult<Option<ProjectionCheckpoint>>;
}

/// Durable record of resource locks (`docs/10-kernel-design/state-storage-and-replay.md:30-47`).
pub trait LockStore: ProjectionStore {}

/// Durable record of resource and environment leases.
pub trait LeaseStore: ProjectionStore {}

/// Durable record of provisioned execution environments and their leases.
pub trait EnvironmentStore: ProjectionStore {}

/// Durable record of role / permission / sandbox / scheduler / provider /
/// routing / communication profiles.
pub trait ProfileStore: ProjectionStore {}

/// Durable record of scheduler policies and ready-queue state.
pub trait SchedulerStore: ProjectionStore {}

/// Durable record of agent-to-agent and human communication messages.
pub trait MessageStore: ProjectionStore {}

/// Durable record of memento fragments.
pub trait MementoStore: ProjectionStore {}

/// Durable record of long-term memory records and their write policy state.
pub trait MemoryStore: ProjectionStore {}

/// Durable record of provider profiles, route decisions, and stream sessions.
pub trait ProviderStore: ProjectionStore {}

/// Durable record of audit events.
pub trait AuditStore: ProjectionStore {}

// Blanket implementations: any projection store satisfies every
// projection-family trait because the families are event-derived read views.
impl<T: ProjectionStore> LockStore for T {}
impl<T: ProjectionStore> LeaseStore for T {}
impl<T: ProjectionStore> EnvironmentStore for T {}
impl<T: ProjectionStore> ProfileStore for T {}
impl<T: ProjectionStore> SchedulerStore for T {}
impl<T: ProjectionStore> MessageStore for T {}
impl<T: ProjectionStore> MementoStore for T {}
impl<T: ProjectionStore> MemoryStore for T {}
impl<T: ProjectionStore> ProviderStore for T {}
impl<T: ProjectionStore> AuditStore for T {}
