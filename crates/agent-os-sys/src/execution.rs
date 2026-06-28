use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum EnvironmentStatus {
    Requested,
    Provisioning,
    Ready,
    Attached,
    Draining,
    Terminated,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendType {
    LocalProcess,
    IsolatedWorktree,
    Container,
    Vm,
    RemoteWorker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReusePolicy {
    Exclusive,
    TaskScoped,
    Pooled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachMode {
    ReadOnly,
    WorkspaceWrite,
    Exclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum EnvironmentLeaseStatus {
    Active,
    Released,
    Expired,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEnvironment {
    pub environment_id: String,
    pub status: EnvironmentStatus,
    pub backend_type: BackendType,
    pub template_name: String,
    pub sandbox_profile_id: String,
    pub host_id: Option<String>,
    pub workspace_mounts: Vec<Value>,
    pub artifact_mounts: Vec<Value>,
    pub toolchain_profile_id: Option<String>,
    pub network_policy_id: Option<String>,
    pub secret_projection_id: Option<String>,
    pub reuse_policy: ReusePolicy,
    pub created_at: String,
    pub updated_at: String,
    pub terminated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentLease {
    pub environment_lease_id: String,
    pub environment_id: String,
    pub agent_id: String,
    pub thread_id: String,
    pub task_id: String,
    pub attach_mode: AttachMode,
    pub status: EnvironmentLeaseStatus,
    pub started_at: String,
    pub expires_at: Option<String>,
    pub released_at: Option<String>,
}
