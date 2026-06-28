mod communication;
mod runtime_state;
mod topology;
mod work_products;

use super::types::BundleSelection;
use crate::KernelState;
use std::collections::BTreeSet;

pub(super) fn task_subtree_ids(state: &KernelState, root_task_id: &str) -> BTreeSet<String> {
    let mut task_ids = BTreeSet::from([root_task_id.to_string()]);
    loop {
        let before = task_ids.len();
        for task in state.tasks.values() {
            if task
                .parent_task_id
                .as_ref()
                .is_some_and(|parent| task_ids.contains(parent))
            {
                task_ids.insert(task.task_id.clone());
            }
        }
        if task_ids.len() == before {
            break;
        }
    }
    task_ids
}

pub(super) fn collect_bundle_selection(state: &KernelState, selection: &mut BundleSelection) {
    for _ in 0..4 {
        topology::collect_threads(state, selection);
        topology::collect_capabilities(state, selection);
        topology::collect_agent_control(state, selection);
        topology::collect_tool_invocations(state, selection);
        work_products::collect_artifacts(state, selection);
        work_products::collect_evidence(state, selection);
        work_products::collect_reviews(state, selection);
        work_products::collect_verifications(state, selection);
        work_products::collect_finals(state, selection);
        work_products::collect_approvals(state, selection);
        runtime_state::collect_leases_and_budgets(state, selection);
        communication::collect_messages(state, selection);
        communication::collect_mementos(state, selection);
        communication::collect_blackboard(state, selection);
        runtime_state::collect_context(state, selection);
        runtime_state::collect_locks_and_memory(state, selection);
        runtime_state::collect_provider_state(state, selection);
        runtime_state::collect_audit_events(state, selection);
    }
    runtime_state::collect_profile_dependencies(state, selection);
}

pub(super) fn selected_object_ids(selection: &BundleSelection) -> BTreeSet<String> {
    let mut ids = BTreeSet::from([selection.goal_id.clone()]);
    ids.extend(selection.task_ids.iter().cloned());
    ids.extend(selection.thread_ids.iter().cloned());
    ids.extend(selection.invocation_ids.iter().cloned());
    ids.extend(selection.hook_ids.iter().cloned());
    ids.extend(selection.agent_control_command_ids.iter().cloned());
    ids.extend(selection.agent_ids.iter().cloned());
    ids.extend(selection.artifact_ids.iter().cloned());
    ids.extend(selection.evidence_ids.iter().cloned());
    ids.extend(selection.review_ids.iter().cloned());
    ids.extend(selection.review_finding_ids.iter().cloned());
    ids.extend(selection.verification_ids.iter().cloned());
    ids.extend(selection.approval_ids.iter().cloned());
    ids.extend(selection.capability_ids.iter().cloned());
    ids.extend(selection.tool_call_ids.iter().cloned());
    ids.extend(selection.environment_ids.iter().cloned());
    ids.extend(selection.environment_lease_ids.iter().cloned());
    ids.extend(selection.resource_lease_ids.iter().cloned());
    ids.extend(selection.budget_ledger_ids.iter().cloned());
    ids.extend(selection.message_ids.iter().cloned());
    ids.extend(selection.memento_ids.iter().cloned());
    ids.extend(selection.blackboard_entry_ids.iter().cloned());
    ids.extend(selection.blackboard_channel_ids.iter().cloned());
    ids.extend(selection.context_snapshot_ids.iter().cloned());
    ids.extend(selection.audit_ids.iter().cloned());
    ids.extend(selection.lock_ids.iter().cloned());
    ids.extend(selection.memory_ids.iter().cloned());
    ids.extend(selection.provider_route_decision_ids.iter().cloned());
    ids.extend(selection.provider_stream_session_ids.iter().cloned());
    ids
}
