use super::{selected_object_ids, BundleSelection};
use crate::KernelState;
use agent_os_sys::BudgetScope;

pub(super) fn collect_leases_and_budgets(state: &KernelState, selection: &mut BundleSelection) {
    for lease in state.environment_leases.values() {
        if selection.task_ids.contains(&lease.task_id)
            || selection.agent_ids.contains(&lease.agent_id)
            || selection.thread_ids.contains(&lease.thread_id)
        {
            selection
                .environment_lease_ids
                .insert(lease.environment_lease_id.clone());
            selection
                .environment_ids
                .insert(lease.environment_id.clone());
        }
    }
    for lease in state.resource_leases.values() {
        if selection.task_ids.contains(&lease.task_id)
            || selection.agent_ids.contains(&lease.owner_agent_id)
            || selection.thread_ids.contains(&lease.thread_id)
        {
            selection
                .resource_lease_ids
                .insert(lease.resource_lease_id.clone());
        }
    }
    for ledger in state.budget_ledgers.values() {
        if (ledger.scope_type == BudgetScope::Task && selection.task_ids.contains(&ledger.scope_id))
            || (ledger.scope_type == BudgetScope::Goal && ledger.scope_id == selection.goal_id)
            || (ledger.scope_type == BudgetScope::Agent
                && selection.agent_ids.contains(&ledger.scope_id))
            || selection
                .budget_ledger_ids
                .contains(&ledger.budget_ledger_id)
        {
            selection
                .budget_ledger_ids
                .insert(ledger.budget_ledger_id.clone());
        }
    }
}

pub(super) fn collect_context(state: &KernelState, selection: &mut BundleSelection) {
    for snapshot in state.context_snapshots.values() {
        if selection.task_ids.contains(&snapshot.task_id)
            || selection.agent_ids.contains(&snapshot.agent_id)
        {
            selection
                .context_snapshot_ids
                .insert(snapshot.context_id.clone());
            for loaded_ref in &snapshot.loaded_refs {
                if state.artifacts.contains_key(loaded_ref) {
                    selection.artifact_ids.insert(loaded_ref.clone());
                }
                if state.evidence.contains_key(loaded_ref) {
                    selection.evidence_ids.insert(loaded_ref.clone());
                }
            }
            if let Some(artifact_id) = &snapshot.summary_artifact_id {
                selection.artifact_ids.insert(artifact_id.clone());
            }
        }
    }
}

pub(super) fn collect_locks_and_memory(state: &KernelState, selection: &mut BundleSelection) {
    for lock in state.locks.values() {
        if selection.task_ids.contains(&lock.task_id)
            || selection.agent_ids.contains(&lock.owner_agent_id)
        {
            selection.lock_ids.insert(lock.lock_id.clone());
        }
    }
    for memory in state.memory_records.values() {
        if memory
            .created_by_agent_id
            .as_ref()
            .is_some_and(|agent_id| selection.agent_ids.contains(agent_id))
            || memory
                .source_evidence_ids
                .iter()
                .any(|evidence_id| selection.evidence_ids.contains(evidence_id))
        {
            selection.memory_ids.insert(memory.memory_id.clone());
            selection
                .evidence_ids
                .extend(memory.source_evidence_ids.iter().cloned());
        }
    }
}

pub(super) fn collect_provider_state(state: &KernelState, selection: &mut BundleSelection) {
    for (decision_id, decision) in &state.provider_route_decisions {
        if selection
            .provider_profile_ids
            .contains(&decision.provider_profile_id)
            || selection
                .routing_policy_ids
                .contains(&decision.routing_policy_id)
            || selection
                .model_aliases
                .contains(&decision.selected_model_alias)
        {
            selection
                .provider_route_decision_ids
                .insert(decision_id.clone());
            selection
                .provider_profile_ids
                .insert(decision.provider_profile_id.clone());
            selection
                .routing_policy_ids
                .insert(decision.routing_policy_id.clone());
            selection
                .model_aliases
                .insert(decision.selected_model_alias.clone());
        }
    }
    for session in state.provider_stream_sessions.values() {
        if selection.task_ids.contains(&session.request.task_id)
            || selection.thread_ids.contains(&session.request.thread_id)
        {
            selection
                .provider_stream_session_ids
                .insert(session.session_id.clone());
            selection
                .provider_profile_ids
                .insert(session.route_decision.provider_profile_id.clone());
            selection
                .routing_policy_ids
                .insert(session.route_decision.routing_policy_id.clone());
            selection
                .model_aliases
                .insert(session.route_decision.selected_model_alias.clone());
        }
    }
}

pub(super) fn collect_audit_events(state: &KernelState, selection: &mut BundleSelection) {
    let object_ids = selected_object_ids(selection);
    for audit in state.audit_events.values() {
        if selection.agent_ids.contains(&audit.actor_id) || object_ids.contains(&audit.resource_id)
        {
            selection.audit_ids.insert(audit.audit_id.clone());
        }
    }
}

pub(super) fn collect_profile_dependencies(state: &KernelState, selection: &mut BundleSelection) {
    for role_id in selection.role_profile_ids.clone() {
        if let Some(role) = state.role_profiles.get(&role_id) {
            selection
                .permission_profile_ids
                .insert(role.default_permission_profile_id.clone());
            selection
                .sandbox_profile_ids
                .insert(role.default_sandbox_profile_id.clone());
            if let Some(provider_profile_id) = &role.default_provider_profile_id {
                selection
                    .provider_profile_ids
                    .insert(provider_profile_id.clone());
            }
            if let Some(scheduler_policy_id) = &role.default_scheduler_policy_id {
                selection
                    .scheduler_policy_ids
                    .insert(scheduler_policy_id.clone());
            }
        }
    }
}
