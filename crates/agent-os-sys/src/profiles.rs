use crate::{QueueClass, ToolDriverClass};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ProfileStatus {
    Active,
    Superseded,
    Revoked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleFamily {
    Producer,
    Reviewer,
    Operator,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewMode {
    None,
    Independent,
    Dual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistroScope {
    Core,
    Distribution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilesystemMode {
    ReadOnly,
    WorkspaceWrite,
    IsolatedWorktree,
    TempOnly,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkMode {
    Off,
    Allowlist,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessBackend {
    Native,
    JobObject,
    Container,
    Vm,
    RemoteWorker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretPolicy {
    None,
    ScopedHandles,
    InjectedEphemeral,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleProfile {
    pub role_profile_id: String,
    pub status: ProfileStatus,
    pub name: String,
    pub role_family: RoleFamily,
    pub purpose: String,
    pub default_permission_profile_id: String,
    pub default_sandbox_profile_id: String,
    pub default_provider_profile_id: Option<String>,
    pub default_scheduler_policy_id: Option<String>,
    pub allowed_child_role_profile_ids: Vec<String>,
    pub required_review_mode: ReviewMode,
    pub escalation_policy: Option<Value>,
    pub distro_scope: DistroScope,
    pub created_at: String,
    pub updated_at: String,
    pub superseded_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionProfile {
    pub permission_profile_id: String,
    pub status: ProfileStatus,
    pub name: String,
    pub permission_set: PermissionSet,
    pub created_at: String,
    pub updated_at: String,
    pub superseded_by: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecurityLevel(pub u32);

impl SecurityLevel {
    pub const HUMAN_ROOT: Self = Self(0);
    pub const ROOT_AGENT: Self = Self(1);

    pub fn child(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    pub fn allows_control_plane(self) -> bool {
        self.0 <= 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionSet {
    pub max_risk_level: u8,
    pub allowed_syscalls: Vec<String>,
    pub resource_scopes: Vec<String>,
    pub allowed_tool_names: Vec<String>,
    pub allowed_tool_driver_classes: Vec<ToolDriverClass>,
    pub approval_required_above: u8,
    pub requires_evidence_for: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxProfile {
    pub sandbox_profile_id: String,
    pub status: ProfileStatus,
    pub name: String,
    pub filesystem_mode: FilesystemMode,
    pub network_mode: NetworkMode,
    pub process_backend: ProcessBackend,
    pub secret_policy: SecretPolicy,
    pub toolchain_profile_id: Option<String>,
    pub mount_policy: Option<Value>,
    pub created_at: String,
    pub updated_at: String,
    pub superseded_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerPolicy {
    pub scheduler_policy_id: String,
    pub status: ProfileStatus,
    pub name: String,
    pub queue_class: QueueClass,
    pub priority: i32,
    pub max_concurrent_children: u32,
    pub max_inflight_model_calls: Option<u32>,
    pub yield_policy: Option<Value>,
    pub retry_policy: Option<Value>,
    pub backoff_policy: Option<Value>,
    pub starvation_window_ms: Option<u64>,
    pub budget_reservation_policy: Option<Value>,
    pub created_at: String,
    pub updated_at: String,
    pub superseded_by: Option<String>,
}
