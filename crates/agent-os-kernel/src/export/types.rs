use agent_os_sys::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleKind {
    Task,
    Replay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskBundle {
    pub abi_version: String,
    pub bundle_kind: BundleKind,
    pub exported_at: String,
    pub root_task_id: String,
    pub goal_id: String,
    pub task_ids: Vec<String>,
    pub profile_snapshot: TaskBundleProfiles,
    pub projection_snapshot: TaskBundleProjection,
    pub events: Vec<EventEnvelope>,
    pub replay_summary: TaskBundleReplaySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskBundleProfiles {
    pub role_profiles: Vec<RoleProfile>,
    pub permission_profiles: Vec<PermissionProfile>,
    pub sandbox_profiles: Vec<SandboxProfile>,
    pub scheduler_policies: Vec<SchedulerPolicy>,
    pub provider_profiles: Vec<ProviderProfile>,
    pub routing_policies: Vec<RoutingPolicy>,
    pub model_aliases: Vec<ModelAlias>,
    pub communication_profiles: Vec<CommunicationProfile>,
    pub tool_descriptors: Vec<ToolDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskBundleProjection {
    pub goal: Goal,
    pub tasks: Vec<Task>,
    pub threads: Vec<AgentControlBlock>,
    pub agent_invocations: Vec<AgentInvocation>,
    pub agent_hooks: Vec<AgentHook>,
    pub agent_control_commands: Vec<AgentControlCommand>,
    pub blackboard_entries: Vec<BlackboardEntry>,
    pub blackboard_channels: Vec<BlackboardChannel>,
    pub context_snapshots: Vec<ContextSnapshot>,
    pub capabilities: Vec<CapabilityToken>,
    pub tool_invocations: Vec<ToolInvocation>,
    pub environments: Vec<ExecutionEnvironment>,
    pub environment_leases: Vec<EnvironmentLease>,
    pub resource_leases: Vec<ResourceLease>,
    pub budget_ledgers: Vec<BudgetLedger>,
    pub messages: Vec<AgentMessage>,
    pub mementos: Vec<MementoFragment>,
    pub artifacts: Vec<Artifact>,
    pub evidence: Vec<Evidence>,
    pub reviews: Vec<Review>,
    pub review_findings: Vec<ReviewFinding>,
    pub verifications: Vec<Verification>,
    pub approvals: Vec<Approval>,
    pub audit_events: Vec<AuditEvent>,
    pub locks: Vec<Lock>,
    pub memory_records: Vec<MemoryRecord>,
    pub provider_route_decisions: Vec<ProviderRouteDecisionRecord>,
    pub provider_stream_sessions: Vec<ProviderStreamSession>,
    pub final_submissions: Vec<FinalSubmissionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRouteDecisionRecord {
    pub decision_id: String,
    pub decision: ProviderRouteDecision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalSubmissionRecord {
    pub task_id: String,
    pub submission: FinalSubmission,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskBundleReplaySummary {
    pub event_count: usize,
    pub task_count: usize,
    pub thread_count: usize,
    pub artifact_count: usize,
    pub evidence_count: usize,
    pub final_submission_count: usize,
}

#[derive(Debug, Default)]
pub(super) struct BundleSelection {
    pub(super) goal_id: String,
    pub(super) task_ids: BTreeSet<String>,
    pub(super) thread_ids: BTreeSet<String>,
    pub(super) invocation_ids: BTreeSet<String>,
    pub(super) hook_ids: BTreeSet<String>,
    pub(super) agent_control_command_ids: BTreeSet<String>,
    pub(super) agent_ids: BTreeSet<String>,
    pub(super) artifact_ids: BTreeSet<String>,
    pub(super) evidence_ids: BTreeSet<String>,
    pub(super) review_ids: BTreeSet<String>,
    pub(super) review_finding_ids: BTreeSet<String>,
    pub(super) verification_ids: BTreeSet<String>,
    pub(super) approval_ids: BTreeSet<String>,
    pub(super) capability_ids: BTreeSet<String>,
    pub(super) tool_call_ids: BTreeSet<String>,
    pub(super) environment_ids: BTreeSet<String>,
    pub(super) environment_lease_ids: BTreeSet<String>,
    pub(super) resource_lease_ids: BTreeSet<String>,
    pub(super) budget_ledger_ids: BTreeSet<String>,
    pub(super) message_ids: BTreeSet<String>,
    pub(super) memento_ids: BTreeSet<String>,
    pub(super) blackboard_entry_ids: BTreeSet<String>,
    pub(super) blackboard_channel_ids: BTreeSet<String>,
    pub(super) context_snapshot_ids: BTreeSet<String>,
    pub(super) audit_ids: BTreeSet<String>,
    pub(super) lock_ids: BTreeSet<String>,
    pub(super) memory_ids: BTreeSet<String>,
    pub(super) provider_route_decision_ids: BTreeSet<String>,
    pub(super) provider_stream_session_ids: BTreeSet<String>,
    pub(super) tool_names: BTreeSet<String>,
    pub(super) role_profile_ids: BTreeSet<String>,
    pub(super) permission_profile_ids: BTreeSet<String>,
    pub(super) sandbox_profile_ids: BTreeSet<String>,
    pub(super) scheduler_policy_ids: BTreeSet<String>,
    pub(super) provider_profile_ids: BTreeSet<String>,
    pub(super) routing_policy_ids: BTreeSet<String>,
    pub(super) model_aliases: BTreeSet<String>,
    pub(super) communication_profile_ids: BTreeSet<String>,
}
