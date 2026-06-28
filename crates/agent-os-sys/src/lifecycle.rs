use crate::{ArtifactType, EvidenceType, MessageRoute};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum GoalStatus {
    Registered,
    Active,
    Suspended,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TaskStatus {
    Created,
    Ready,
    Running,
    Blocked,
    Reviewing,
    Verifying,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ThreadStatus {
    Created,
    Ready,
    Running,
    WaitingTool,
    WaitingPermission,
    WaitingUser,
    Blocked,
    Suspended,
    ResidentIdle,
    Unloaded,
    Completing,
    Completed,
    Failed,
    Interrupted,
    Quarantined,
    Terminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TurnStatus {
    Pending,
    InProgress,
    AwaitingTool,
    AwaitingPermission,
    AwaitingUser,
    Compacting,
    Completed,
    Failed,
    Interrupted,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum StepStatus {
    Created,
    CallingModel,
    StreamingModel,
    DispatchingTools,
    WaitingTools,
    RecordingResults,
    CheckingCompaction,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub goal_id: String,
    pub namespace: String,
    pub created_by: String,
    pub status: GoalStatus,
    pub title: String,
    pub description: String,
    pub acceptance_criteria: Vec<String>,
    pub constraints: Vec<String>,
    pub risk_level: u8,
    pub deadline: Option<String>,
    pub root_task_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub task_id: String,
    pub goal_id: String,
    pub parent_task_id: Option<String>,
    pub status: TaskStatus,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub checklist: Vec<ChecklistItem>,
    pub owner_agent_id: Option<String>,
    pub depends_on: Vec<String>,
    pub blocks: Vec<String>,
    pub required_artifact_types: Vec<ArtifactType>,
    pub required_evidence_types: Vec<EvidenceType>,
    pub blocked_reason: Option<String>,
    pub priority: i32,
    pub risk_level: u8,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectiveBindingSnapshot {
    pub role_profile_id: String,
    pub permission_profile_id: String,
    pub sandbox_profile_id: String,
    pub provider_profile_id: Option<String>,
    pub scheduler_policy_id: Option<String>,
    pub communication_profile_id: String,
    pub reasoning_profile: Option<String>,
    pub revision: u64,
    pub resolved_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadTaskBinding {
    pub task_id: String,
    pub goal_id: String,
    pub local_goal: String,
    pub success_criteria: Vec<String>,
    pub failure_criteria: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChecklistItemStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub text: String,
    #[serde(default)]
    pub status: ChecklistItemStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentInvocationRelationship {
    RootSupervisor,
    SupervisorDelegation,
    WorkerAssignment,
    ReviewRequest,
    HumanEscalation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AgentInvocationStatus {
    Active,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInvocation {
    pub invocation_id: String,
    pub goal_id: String,
    pub task_id: String,
    pub caller_thread_id: Option<String>,
    pub caller_agent_id: Option<String>,
    pub caller_supervisor_level: Option<u32>,
    pub callee_thread_id: String,
    pub callee_agent_id: String,
    pub callee_supervisor_level: Option<u32>,
    pub relationship: AgentInvocationRelationship,
    pub assignment: String,
    pub status: AgentInvocationStatus,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentControlAction {
    Start,
    Status,
    Output,
    SetHook,
    Send,
    Resume,
    Stop,
    SetTimeout,
    ExportTrace,
    Kill,
    DeleteSession,
    PurgeState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AgentControlCommandStatus {
    Applied,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AgentHookStatus {
    Active,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHook {
    pub hook_id: String,
    pub agent_id: String,
    pub thread_id: String,
    pub hook_type: String,
    pub interval_seconds: u64,
    pub prompt: String,
    pub response_route: MessageRoute,
    pub max_response_chars: u64,
    pub stop_when: String,
    pub on_missed_reports: String,
    pub status: AgentHookStatus,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentControlCommand {
    pub command_id: String,
    pub action: AgentControlAction,
    pub requested_by_agent_id: String,
    pub requested_by_thread_id: String,
    pub target_agent_id: Option<String>,
    pub target_thread_id: Option<String>,
    pub task_id: String,
    pub goal_id: String,
    pub payload: Value,
    pub status: AgentControlCommandStatus,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadConfigSnapshot {
    pub model_provider_id: String,
    pub model_id: String,
    pub provider_profile_id: String,
    pub model_routing_policy_id: String,
    pub provider_adapter_version: String,
    pub role_profile_id: String,
    pub communication_profile_id: String,
    pub permission_profile_id: String,
    pub sandbox_profile_id: String,
    pub context_policy_id: String,
    pub memory_policy_id: String,
    pub tool_registry_snapshot_id: String,
    pub workspace_roots: Vec<String>,
    pub environment_ids: Vec<String>,
    pub reasoning_profile: Option<String>,
    pub effective_binding: EffectiveBindingSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThreadQueues {
    pub submission_cursor: Option<String>,
    pub event_sequence: u64,
    pub mailbox_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActiveTurn {
    pub turn_id: Option<String>,
    pub status: Option<TurnStatus>,
    pub active_step_id: Option<String>,
    pub expected_turn_id: Option<String>,
    pub model_turn_state_ref: Option<String>,
    pub started_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThreadResources {
    pub held_locks: Vec<String>,
    pub workspace_isolation_ref: Option<String>,
    pub sandbox_ref: Option<String>,
    pub background_process_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadBudgets {
    pub token_budget: Option<u64>,
    pub tool_call_budget: Option<u64>,
    pub wall_time_budget_ms: Option<u64>,
    pub cost_budget: Option<f64>,
    pub max_steps_per_turn: u32,
    pub max_child_threads: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadRecovery {
    pub last_checkpoint_id: Option<String>,
    pub replay_cursor: Option<String>,
    pub last_materialized_event_sequence: u64,
    pub dirty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadAudit {
    pub created_at: String,
    pub updated_at: String,
    pub created_by: String,
    pub termination_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentControlBlock {
    pub thread_id: String,
    pub agent_id: String,
    pub invocation_id: String,
    pub session_id: String,
    pub root_thread_id: String,
    pub parent_thread_id: Option<String>,
    pub supervisor_level: Option<u32>,
    pub agent_path: String,
    pub role: String,
    pub owner: String,
    pub status: ThreadStatus,
    pub status_reason: Option<String>,
    pub task: ThreadTaskBinding,
    pub config_snapshot: ThreadConfigSnapshot,
    pub queues: ThreadQueues,
    pub active_turn: ActiveTurn,
    pub resources: ThreadResources,
    pub budgets: ThreadBudgets,
    pub recovery: ThreadRecovery,
    pub audit: ThreadAudit,
}
