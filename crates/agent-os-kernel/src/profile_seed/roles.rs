use super::strings;
use agent_os_sys::*;

pub(super) fn core_roles(now: &str) -> Vec<RoleProfile> {
    [
        (
            "role_supervisor",
            "SupervisorAgent",
            RoleFamily::Operator,
            "Owns orchestration, final contract, and escalation.",
            "perm_supervisor",
            "sbox_readonly",
            "sched_foreground",
            strings(&["role_supervisor", "role_producer", "role_reviewer"]),
            ReviewMode::Independent,
        ),
        (
            "role_producer",
            "ProducerAgent",
            RoleFamily::Producer,
            "Produces assigned artifacts with evidence, including plans, patches, commands, tests, and reports.",
            "perm_producer",
            "sbox_workspace_write",
            "sched_background",
            Vec::new(),
            ReviewMode::Independent,
        ),
        (
            "role_reviewer",
            "ReviewerAgent",
            RoleFamily::Reviewer,
            "Independently reviews artifacts and verifies supporting evidence with tools.",
            "perm_reviewer",
            "sbox_workspace_write",
            "sched_review",
            Vec::new(),
            ReviewMode::None,
        ),
    ]
    .into_iter()
    .map(|spec| RoleProfile {
        role_profile_id: spec.0.to_string(),
        status: ProfileStatus::Active,
        name: spec.1.to_string(),
        role_family: spec.2,
        purpose: spec.3.to_string(),
        default_permission_profile_id: spec.4.to_string(),
        default_sandbox_profile_id: spec.5.to_string(),
        default_provider_profile_id: Some("prov_default".to_string()),
        default_scheduler_policy_id: Some(spec.6.to_string()),
        allowed_child_role_profile_ids: spec.7,
        required_review_mode: spec.8,
        escalation_policy: None,
        distro_scope: DistroScope::Core,
        created_at: now.to_string(),
        updated_at: now.to_string(),
        superseded_by: None,
    })
    .collect()
}
