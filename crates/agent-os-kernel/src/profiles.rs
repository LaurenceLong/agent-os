use crate::*;
use agent_os_sys::*;

impl Kernel {
    pub(crate) fn default_communication_profile(
        &self,
        id: &str,
        agent_id: &str,
        thread_id: &str,
        role: &RoleProfile,
    ) -> CommunicationProfile {
        let now = now_rfc3339();
        CommunicationProfile {
            communication_profile_id: id.to_string(),
            agent_id: agent_id.to_string(),
            thread_id: thread_id.to_string(),
            status: ProfileStatus::Active,
            supervisor: DirectRoutePolicy {
                enabled: true,
                allowed_message_types: vec![
                    "StatusUpdate".to_string(),
                    "BlockerReport".to_string(),
                    "RiskReport".to_string(),
                    "CompletionReport".to_string(),
                ],
                trigger_turn: false,
                rate_limit: Some("10/hour".to_string()),
            },
            blackboard: BlackboardRoutePolicy {
                enabled: true,
                allowed_scopes: CommunicationScope::Goal,
                allowed_channels: vec![
                    "facts".to_string(),
                    "risks".to_string(),
                    "blockers".to_string(),
                    "artifacts".to_string(),
                    "evidence".to_string(),
                    "test-results".to_string(),
                    "review-results".to_string(),
                ],
                allowed_entry_types: vec![
                    "known_fact".to_string(),
                    "hypothesis".to_string(),
                    "risk".to_string(),
                    "open_question".to_string(),
                    "test_result".to_string(),
                    "review_result".to_string(),
                ],
                broadcast: role.name == "SupervisorAgent",
                requires_review: false,
            },
            human: HumanRoutePolicy {
                enabled: role.name == "SupervisorAgent",
                allowed_message_types: vec![
                    "HumanQuestion".to_string(),
                    "HumanEscalation".to_string(),
                    "ApprovalRequest".to_string(),
                ],
                requires_supervisor_approval: role.name != "SupervisorAgent",
                attention_budget: Some(3),
            },
            completion: CompletionRoutePolicy {
                required_report: true,
                allowed_artifact_refs: true,
                allowed_evidence_refs: true,
            },
            created_at: now.clone(),
            updated_at: now,
            superseded_by: None,
        }
    }
}
