use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum LockStatus {
    Active,
    Released,
    Expired,
    ForceReleased,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueClass {
    Foreground,
    Background,
    Review,
    Verify,
    HumanWait,
    Batch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    File,
    Workspace,
    Environment,
    ProviderSlot,
    BlackboardChannel,
    Artifact,
    DeploymentTarget,
    MemoryNamespace,
    HumanAttention,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseMode {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ResourceLeaseStatus {
    Requested,
    Granted,
    Released,
    Expired,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum BudgetStatus {
    Active,
    Exhausted,
    Suspended,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetScope {
    Goal,
    Task,
    Agent,
    ProviderProfile,
    HumanAttention,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLease {
    pub resource_lease_id: String,
    pub resource_type: ResourceType,
    pub resource_id: String,
    pub owner_agent_id: String,
    pub thread_id: String,
    pub goal_id: String,
    pub task_id: String,
    pub mode: LeaseMode,
    pub status: ResourceLeaseStatus,
    pub reason: Option<String>,
    pub lease_expires_at: Option<String>,
    pub created_at: String,
    pub released_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetLedger {
    pub budget_ledger_id: String,
    pub scope_type: BudgetScope,
    pub scope_id: String,
    pub status: BudgetStatus,
    pub token_limit: Option<u64>,
    pub tool_call_limit: Option<u64>,
    pub wall_time_limit_ms: Option<u64>,
    pub cost_limit: Option<f64>,
    pub human_interrupt_limit: Option<u64>,
    pub model_request_limit: Option<u64>,
    pub tokens_used: u64,
    pub tool_calls_used: u64,
    pub wall_time_used_ms: u64,
    pub cost_used: f64,
    pub human_interrupts_used: u64,
    pub model_requests_used: u64,
    pub reserved: Option<Value>,
    pub reset_policy: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lock {
    pub lock_id: String,
    pub resource_type: ResourceType,
    pub resource_id: String,
    pub owner_agent_id: String,
    pub task_id: String,
    pub lease_expires_at: String,
    pub reason: String,
    pub risk_level: u8,
    pub status: LockStatus,
    pub created_at: String,
    pub released_at: Option<String>,
}
