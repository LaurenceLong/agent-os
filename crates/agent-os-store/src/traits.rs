use agent_os_sys::{AgentOsResult, EventEnvelope, SyscallResult};

pub trait EventStore: Send + Sync {
    fn append(&self, event: EventEnvelope) -> AgentOsResult<()>;
    fn all_events(&self) -> AgentOsResult<Vec<EventEnvelope>>;
    fn events_by_aggregate(&self, aggregate_id: &str) -> AgentOsResult<Vec<EventEnvelope>>;
}

pub trait IdempotencyStore: Send + Sync {
    fn get_syscall_result(&self, idempotency_key: &str) -> AgentOsResult<Option<SyscallResult>>;
    fn put_syscall_result(
        &self,
        idempotency_key: String,
        result: SyscallResult,
    ) -> AgentOsResult<()>;
}

pub trait KernelStore: EventStore + IdempotencyStore {}

impl<T> KernelStore for T where T: EventStore + IdempotencyStore {}

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
