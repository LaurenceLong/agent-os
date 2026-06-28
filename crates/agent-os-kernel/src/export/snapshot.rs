use super::selection::selected_object_ids;
use super::types::{
    BundleSelection, FinalSubmissionRecord, ProviderRouteDecisionRecord, TaskBundleProfiles,
    TaskBundleProjection,
};
use crate::KernelState;
use agent_os_sys::*;

pub(super) fn profile_snapshot(
    state: &KernelState,
    selection: &BundleSelection,
) -> TaskBundleProfiles {
    let mut profiles = TaskBundleProfiles {
        role_profiles: state
            .role_profiles
            .values()
            .filter(|profile| {
                selection
                    .role_profile_ids
                    .contains(&profile.role_profile_id)
            })
            .cloned()
            .collect(),
        permission_profiles: state
            .permission_profiles
            .values()
            .filter(|profile| {
                selection
                    .permission_profile_ids
                    .contains(&profile.permission_profile_id)
            })
            .cloned()
            .collect(),
        sandbox_profiles: state
            .sandbox_profiles
            .values()
            .filter(|profile| {
                selection
                    .sandbox_profile_ids
                    .contains(&profile.sandbox_profile_id)
            })
            .cloned()
            .collect(),
        scheduler_policies: state
            .scheduler_policies
            .values()
            .filter(|policy| {
                selection
                    .scheduler_policy_ids
                    .contains(&policy.scheduler_policy_id)
            })
            .cloned()
            .collect(),
        provider_profiles: state
            .provider_profiles
            .values()
            .filter(|profile| {
                selection
                    .provider_profile_ids
                    .contains(&profile.provider_profile_id)
            })
            .cloned()
            .collect(),
        routing_policies: state
            .routing_policies
            .values()
            .filter(|policy| {
                selection
                    .routing_policy_ids
                    .contains(&policy.routing_policy_id)
            })
            .cloned()
            .collect(),
        model_aliases: state
            .model_aliases
            .values()
            .filter(|alias| selection.model_aliases.contains(&alias.alias))
            .cloned()
            .collect(),
        communication_profiles: state
            .communication_profiles
            .values()
            .filter(|profile| {
                selection
                    .communication_profile_ids
                    .contains(&profile.communication_profile_id)
            })
            .cloned()
            .collect(),
        tool_descriptors: state
            .tool_descriptors
            .values()
            .filter(|descriptor| selection.tool_names.contains(&descriptor.name))
            .cloned()
            .collect(),
    };
    sort_profiles(&mut profiles);
    profiles
}

pub(super) fn projection_snapshot(
    state: &KernelState,
    selection: &BundleSelection,
    goal: Goal,
) -> AgentOsResult<TaskBundleProjection> {
    let mut projection = TaskBundleProjection {
        goal,
        tasks: state
            .tasks
            .values()
            .filter(|task| selection.task_ids.contains(&task.task_id))
            .cloned()
            .collect(),
        threads: state
            .threads
            .values()
            .filter(|thread| selection.thread_ids.contains(&thread.thread_id))
            .cloned()
            .collect(),
        agent_invocations: state
            .agent_invocations
            .values()
            .filter(|invocation| selection.invocation_ids.contains(&invocation.invocation_id))
            .cloned()
            .collect(),
        agent_hooks: state
            .agent_hooks
            .values()
            .filter(|hook| selection.hook_ids.contains(&hook.hook_id))
            .cloned()
            .collect(),
        agent_control_commands: state
            .agent_control_commands
            .values()
            .filter(|command| {
                selection
                    .agent_control_command_ids
                    .contains(&command.command_id)
            })
            .cloned()
            .collect(),
        blackboard_entries: state
            .blackboard_entries
            .values()
            .filter(|entry| selection.blackboard_entry_ids.contains(&entry.entry_id))
            .cloned()
            .collect(),
        blackboard_channels: state
            .blackboard_channels
            .values()
            .filter(|channel| {
                selection
                    .blackboard_channel_ids
                    .contains(&channel.channel_id)
            })
            .cloned()
            .collect(),
        context_snapshots: state
            .context_snapshots
            .values()
            .filter(|snapshot| {
                selection
                    .context_snapshot_ids
                    .contains(&snapshot.context_id)
            })
            .cloned()
            .collect(),
        capabilities: state
            .capabilities
            .values()
            .filter(|capability| selection.capability_ids.contains(&capability.capability_id))
            .cloned()
            .collect(),
        tool_invocations: state
            .tool_invocations
            .values()
            .filter(|invocation| selection.tool_call_ids.contains(&invocation.call_id))
            .cloned()
            .collect(),
        environments: state
            .environments
            .values()
            .filter(|environment| {
                selection
                    .environment_ids
                    .contains(&environment.environment_id)
            })
            .cloned()
            .collect(),
        environment_leases: state
            .environment_leases
            .values()
            .filter(|lease| {
                selection
                    .environment_lease_ids
                    .contains(&lease.environment_lease_id)
            })
            .cloned()
            .collect(),
        resource_leases: state
            .resource_leases
            .values()
            .filter(|lease| {
                selection
                    .resource_lease_ids
                    .contains(&lease.resource_lease_id)
            })
            .cloned()
            .collect(),
        budget_ledgers: state
            .budget_ledgers
            .values()
            .filter(|ledger| {
                selection
                    .budget_ledger_ids
                    .contains(&ledger.budget_ledger_id)
            })
            .cloned()
            .collect(),
        messages: state
            .messages
            .values()
            .filter(|message| selection.message_ids.contains(&message.message_id))
            .cloned()
            .collect(),
        mementos: state
            .mementos
            .values()
            .filter(|memento| selection.memento_ids.contains(&memento.memento_id))
            .cloned()
            .collect(),
        artifacts: state
            .artifacts
            .values()
            .filter(|artifact| selection.artifact_ids.contains(&artifact.artifact_id))
            .cloned()
            .collect(),
        evidence: state
            .evidence
            .values()
            .filter(|evidence| selection.evidence_ids.contains(&evidence.evidence_id))
            .cloned()
            .collect(),
        reviews: state
            .reviews
            .values()
            .filter(|review| selection.review_ids.contains(&review.review_id))
            .cloned()
            .collect(),
        review_findings: state
            .review_findings
            .values()
            .filter(|finding| selection.review_finding_ids.contains(&finding.finding_id))
            .cloned()
            .collect(),
        verifications: state
            .verifications
            .values()
            .filter(|verification| {
                selection
                    .verification_ids
                    .contains(&verification.verification_id)
            })
            .cloned()
            .collect(),
        approvals: state
            .approvals
            .values()
            .filter(|approval| selection.approval_ids.contains(&approval.approval_id))
            .cloned()
            .collect(),
        audit_events: state
            .audit_events
            .values()
            .filter(|audit| selection.audit_ids.contains(&audit.audit_id))
            .cloned()
            .collect(),
        locks: state
            .locks
            .values()
            .filter(|lock| selection.lock_ids.contains(&lock.lock_id))
            .cloned()
            .collect(),
        memory_records: state
            .memory_records
            .values()
            .filter(|memory| selection.memory_ids.contains(&memory.memory_id))
            .cloned()
            .collect(),
        provider_route_decisions: state
            .provider_route_decisions
            .iter()
            .filter(|(decision_id, _)| selection.provider_route_decision_ids.contains(*decision_id))
            .map(|(decision_id, decision)| ProviderRouteDecisionRecord {
                decision_id: decision_id.clone(),
                decision: decision.clone(),
            })
            .collect(),
        provider_stream_sessions: state
            .provider_stream_sessions
            .values()
            .filter(|session| {
                selection
                    .provider_stream_session_ids
                    .contains(&session.session_id)
            })
            .cloned()
            .collect(),
        final_submissions: state
            .final_submissions
            .iter()
            .filter(|(task_id, _)| selection.task_ids.contains(*task_id))
            .map(|(task_id, submission)| FinalSubmissionRecord {
                task_id: task_id.clone(),
                submission: submission.clone(),
            })
            .collect(),
    };
    sort_projection(&mut projection);
    if projection.tasks.len() != selection.task_ids.len() {
        return Err(AgentOsError::Validation(
            "task bundle projection lost selected tasks".to_string(),
        ));
    }
    Ok(projection)
}

pub(super) fn filter_events(
    events: Vec<EventEnvelope>,
    selection: &BundleSelection,
) -> Vec<EventEnvelope> {
    let object_ids = selected_object_ids(selection);
    events
        .into_iter()
        .filter(|event| {
            object_ids.contains(&event.aggregate_id)
                || event
                    .task_id
                    .as_ref()
                    .is_some_and(|task_id| selection.task_ids.contains(task_id))
                || event
                    .agent_id
                    .as_ref()
                    .is_some_and(|agent_id| selection.agent_ids.contains(agent_id))
                || event
                    .correlation_id
                    .as_ref()
                    .is_some_and(|correlation_id| object_ids.contains(correlation_id))
        })
        .collect()
}

fn sort_profiles(profiles: &mut TaskBundleProfiles) {
    profiles
        .role_profiles
        .sort_by(|left, right| left.role_profile_id.cmp(&right.role_profile_id));
    profiles
        .permission_profiles
        .sort_by(|left, right| left.permission_profile_id.cmp(&right.permission_profile_id));
    profiles
        .sandbox_profiles
        .sort_by(|left, right| left.sandbox_profile_id.cmp(&right.sandbox_profile_id));
    profiles
        .scheduler_policies
        .sort_by(|left, right| left.scheduler_policy_id.cmp(&right.scheduler_policy_id));
    profiles
        .provider_profiles
        .sort_by(|left, right| left.provider_profile_id.cmp(&right.provider_profile_id));
    profiles
        .routing_policies
        .sort_by(|left, right| left.routing_policy_id.cmp(&right.routing_policy_id));
    profiles
        .model_aliases
        .sort_by(|left, right| left.alias.cmp(&right.alias));
    profiles.communication_profiles.sort_by(|left, right| {
        left.communication_profile_id
            .cmp(&right.communication_profile_id)
    });
    profiles
        .tool_descriptors
        .sort_by(|left, right| left.name.cmp(&right.name));
}

fn sort_projection(projection: &mut TaskBundleProjection) {
    projection
        .tasks
        .sort_by(|left, right| left.task_id.cmp(&right.task_id));
    projection
        .threads
        .sort_by(|left, right| left.thread_id.cmp(&right.thread_id));
    projection
        .agent_invocations
        .sort_by(|left, right| left.invocation_id.cmp(&right.invocation_id));
    projection
        .agent_hooks
        .sort_by(|left, right| left.hook_id.cmp(&right.hook_id));
    projection
        .agent_control_commands
        .sort_by(|left, right| left.command_id.cmp(&right.command_id));
    projection
        .blackboard_entries
        .sort_by(|left, right| left.entry_id.cmp(&right.entry_id));
    projection
        .blackboard_channels
        .sort_by(|left, right| left.channel_id.cmp(&right.channel_id));
    projection
        .context_snapshots
        .sort_by(|left, right| left.context_id.cmp(&right.context_id));
    projection
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    projection
        .tool_invocations
        .sort_by(|left, right| left.call_id.cmp(&right.call_id));
    projection
        .environments
        .sort_by(|left, right| left.environment_id.cmp(&right.environment_id));
    projection
        .environment_leases
        .sort_by(|left, right| left.environment_lease_id.cmp(&right.environment_lease_id));
    projection
        .resource_leases
        .sort_by(|left, right| left.resource_lease_id.cmp(&right.resource_lease_id));
    projection
        .budget_ledgers
        .sort_by(|left, right| left.budget_ledger_id.cmp(&right.budget_ledger_id));
    projection
        .messages
        .sort_by(|left, right| left.message_id.cmp(&right.message_id));
    projection
        .mementos
        .sort_by(|left, right| left.memento_id.cmp(&right.memento_id));
    projection
        .artifacts
        .sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
    projection
        .evidence
        .sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
    projection
        .reviews
        .sort_by(|left, right| left.review_id.cmp(&right.review_id));
    projection
        .review_findings
        .sort_by(|left, right| left.finding_id.cmp(&right.finding_id));
    projection
        .verifications
        .sort_by(|left, right| left.verification_id.cmp(&right.verification_id));
    projection
        .approvals
        .sort_by(|left, right| left.approval_id.cmp(&right.approval_id));
    projection
        .audit_events
        .sort_by(|left, right| left.audit_id.cmp(&right.audit_id));
    projection
        .locks
        .sort_by(|left, right| left.lock_id.cmp(&right.lock_id));
    projection
        .memory_records
        .sort_by(|left, right| left.memory_id.cmp(&right.memory_id));
    projection
        .provider_route_decisions
        .sort_by(|left, right| left.decision_id.cmp(&right.decision_id));
    projection
        .provider_stream_sessions
        .sort_by(|left, right| left.session_id.cmp(&right.session_id));
    projection
        .final_submissions
        .sort_by(|left, right| left.task_id.cmp(&right.task_id));
}
