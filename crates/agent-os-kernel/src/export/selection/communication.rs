use super::BundleSelection;
use crate::KernelState;

pub(super) fn collect_messages(state: &KernelState, selection: &mut BundleSelection) {
    for message in state.messages.values() {
        if selection.task_ids.contains(&message.task_id)
            || selection.agent_ids.contains(&message.source_agent_id)
            || message
                .target_agent_id
                .as_ref()
                .is_some_and(|agent_id| selection.agent_ids.contains(agent_id))
            || selection.thread_ids.contains(&message.source_thread_id)
            || message
                .target_thread_id
                .as_ref()
                .is_some_and(|thread_id| selection.thread_ids.contains(thread_id))
        {
            selection.message_ids.insert(message.message_id.clone());
            selection
                .artifact_ids
                .extend(message.artifact_refs.iter().cloned());
            selection
                .evidence_ids
                .extend(message.evidence_refs.iter().cloned());
            if let Some(channel_id) = &message.channel_id {
                selection.blackboard_channel_ids.insert(channel_id.clone());
            }
        }
    }
}

pub(super) fn collect_mementos(state: &KernelState, selection: &mut BundleSelection) {
    for memento in state.mementos.values() {
        if selection.task_ids.contains(&memento.task_id)
            || selection.agent_ids.contains(&memento.owner_agent_id)
            || selection.thread_ids.contains(&memento.owner_thread_id)
        {
            selection.memento_ids.insert(memento.memento_id.clone());
            selection
                .artifact_ids
                .extend(memento.links.related_artifact_ids.iter().cloned());
            selection
                .evidence_ids
                .extend(memento.links.related_evidence_ids.iter().cloned());
            selection
                .thread_ids
                .extend(memento.links.related_child_thread_ids.iter().cloned());
            selection
                .tool_call_ids
                .extend(memento.links.related_tool_call_ids.iter().cloned());
        }
    }
}

pub(super) fn collect_blackboard(state: &KernelState, selection: &mut BundleSelection) {
    for entry in state.blackboard_entries.values() {
        if entry.goal_id == selection.goal_id
            && (entry
                .task_id
                .as_ref()
                .is_some_and(|task_id| selection.task_ids.contains(task_id))
                || entry
                    .created_by_agent_id
                    .as_ref()
                    .is_some_and(|agent_id| selection.agent_ids.contains(agent_id))
                || entry
                    .source_evidence_ids
                    .iter()
                    .any(|evidence_id| selection.evidence_ids.contains(evidence_id)))
        {
            selection
                .blackboard_entry_ids
                .insert(entry.entry_id.clone());
            selection
                .evidence_ids
                .extend(entry.source_evidence_ids.iter().cloned());
        }
    }
    for channel in state.blackboard_channels.values() {
        if selection
            .blackboard_channel_ids
            .contains(&channel.channel_id)
            || channel
                .subscriber_agent_ids
                .iter()
                .any(|agent_id| selection.agent_ids.contains(agent_id))
        {
            selection
                .blackboard_channel_ids
                .insert(channel.channel_id.clone());
        }
    }
}
