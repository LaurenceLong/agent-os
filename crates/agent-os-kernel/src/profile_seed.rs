mod permissions;
mod provider;
mod roles;
mod sandboxes;
mod scheduler;
mod tool_schemas;
mod tools;

use crate::*;
use agent_os_sys::*;

impl Kernel {
    pub(crate) fn install_core_profiles(&self) {
        let mut state = self.state.write().expect("kernel state poisoned at init");
        let now = now_rfc3339();

        for profile in permissions::core_permissions(&now) {
            state
                .permission_profiles
                .insert(profile.permission_profile_id.clone(), profile);
        }
        for profile in sandboxes::core_sandboxes(&now) {
            state
                .sandbox_profiles
                .insert(profile.sandbox_profile_id.clone(), profile);
        }
        for policy in scheduler::core_scheduler_policies(&now) {
            state
                .scheduler_policies
                .insert(policy.scheduler_policy_id.clone(), policy);
        }

        let routing = provider::default_routing_policy(&now);
        state
            .routing_policies
            .insert(routing.routing_policy_id.clone(), routing);

        let provider = provider::default_provider_profile(&now);
        state
            .provider_profiles
            .insert(provider.provider_profile_id.clone(), provider);
        let provider = provider::strict_text_provider_profile(&now);
        state
            .provider_profiles
            .insert(provider.provider_profile_id.clone(), provider);

        for alias in provider::core_model_aliases(&now) {
            state.model_aliases.insert(alias.alias.clone(), alias);
        }
        for tool in tools::core_tool_descriptors(&now) {
            state.tool_descriptors.insert(tool.name.clone(), tool);
        }
        for role in roles::core_roles(&now) {
            state
                .role_profiles
                .insert(role.role_profile_id.clone(), role);
        }
    }
}

pub(super) fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}
