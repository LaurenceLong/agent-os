use super::BundleSelection;
use crate::KernelState;

pub(super) fn collect_threads(state: &KernelState, selection: &mut BundleSelection) {
    for thread in state.threads.values() {
        if selection.task_ids.contains(&thread.task.task_id) {
            selection.thread_ids.insert(thread.thread_id.clone());
            selection
                .invocation_ids
                .insert(thread.invocation_id.clone());
            selection.agent_ids.insert(thread.agent_id.clone());
            selection
                .role_profile_ids
                .insert(thread.config_snapshot.role_profile_id.clone());
            selection
                .permission_profile_ids
                .insert(thread.config_snapshot.permission_profile_id.clone());
            selection
                .sandbox_profile_ids
                .insert(thread.config_snapshot.sandbox_profile_id.clone());
            selection
                .provider_profile_ids
                .insert(thread.config_snapshot.provider_profile_id.clone());
            selection
                .routing_policy_ids
                .insert(thread.config_snapshot.model_routing_policy_id.clone());
            selection
                .model_aliases
                .insert(thread.config_snapshot.model_id.clone());
            selection
                .communication_profile_ids
                .insert(thread.config_snapshot.communication_profile_id.clone());
            selection
                .environment_ids
                .extend(thread.config_snapshot.environment_ids.iter().cloned());
        }
    }
    for invocation in state.agent_invocations.values() {
        if selection.thread_ids.contains(&invocation.callee_thread_id)
            || invocation
                .caller_thread_id
                .as_ref()
                .is_some_and(|thread_id| selection.thread_ids.contains(thread_id))
        {
            selection
                .invocation_ids
                .insert(invocation.invocation_id.clone());
        }
    }
}

pub(super) fn collect_agent_control(state: &KernelState, selection: &mut BundleSelection) {
    for hook in state.agent_hooks.values() {
        if selection.agent_ids.contains(&hook.agent_id)
            || selection.thread_ids.contains(&hook.thread_id)
            || selection.hook_ids.contains(&hook.hook_id)
        {
            selection.hook_ids.insert(hook.hook_id.clone());
            selection.agent_ids.insert(hook.agent_id.clone());
            selection.thread_ids.insert(hook.thread_id.clone());
        }
    }
    for command in state.agent_control_commands.values() {
        if selection.task_ids.contains(&command.task_id)
            || selection.agent_ids.contains(&command.requested_by_agent_id)
            || selection
                .thread_ids
                .contains(&command.requested_by_thread_id)
            || command
                .target_agent_id
                .as_ref()
                .is_some_and(|agent_id| selection.agent_ids.contains(agent_id))
            || command
                .target_thread_id
                .as_ref()
                .is_some_and(|thread_id| selection.thread_ids.contains(thread_id))
            || selection
                .agent_control_command_ids
                .contains(&command.command_id)
        {
            selection
                .agent_control_command_ids
                .insert(command.command_id.clone());
            selection
                .agent_ids
                .insert(command.requested_by_agent_id.clone());
            selection
                .thread_ids
                .insert(command.requested_by_thread_id.clone());
            if let Some(agent_id) = &command.target_agent_id {
                selection.agent_ids.insert(agent_id.clone());
            }
            if let Some(thread_id) = &command.target_thread_id {
                selection.thread_ids.insert(thread_id.clone());
            }
        }
    }
}

pub(super) fn collect_capabilities(state: &KernelState, selection: &mut BundleSelection) {
    for capability in state.capabilities.values() {
        if selection.task_ids.contains(&capability.task_id)
            || selection.agent_ids.contains(&capability.agent_id)
        {
            selection
                .capability_ids
                .insert(capability.capability_id.clone());
        }
    }
}

pub(super) fn collect_tool_invocations(state: &KernelState, selection: &mut BundleSelection) {
    for invocation in state.tool_invocations.values() {
        if selection.task_ids.contains(&invocation.task_id)
            || selection.agent_ids.contains(&invocation.agent_id)
        {
            selection.tool_call_ids.insert(invocation.call_id.clone());
            selection.tool_names.insert(invocation.tool_name.clone());
            selection
                .evidence_ids
                .extend(invocation.evidence_ids.iter().cloned());
        }
    }
}
