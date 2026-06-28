use crate::*;
use agent_os_sys::*;
use serde_json::Value;

impl Kernel {
    pub fn send_message(&self, input: SendMessageInput) -> AgentOsResult<AgentMessage> {
        self.send_message_with_cause(input, None)
    }

    pub(crate) fn send_message_with_cause(
        &self,
        input: SendMessageInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<AgentMessage> {
        let (profile, allowed, reason) = {
            let state = self.read_state()?;
            let acb = state.threads.get(&input.source_thread_id).ok_or_else(|| {
                AgentOsError::NotFound(format!("thread {}", input.source_thread_id))
            })?;
            if acb.agent_id != input.source_agent_id {
                return Err(AgentOsError::Validation(
                    "message source agent does not match thread".to_string(),
                ));
            }
            let profile = state
                .communication_profiles
                .get(&acb.config_snapshot.communication_profile_id)
                .cloned()
                .ok_or_else(|| {
                    AgentOsError::NotFound(format!(
                        "communication profile {}",
                        acb.config_snapshot.communication_profile_id
                    ))
                })?;
            let (allowed, reason) = communication_allowed(&profile, &input);
            (profile, allowed, reason)
        };
        let now = now_rfc3339();
        let message = AgentMessage {
            message_id: new_id("msg_"),
            message_type: input.message_type,
            route: input.route,
            source_agent_id: input.source_agent_id,
            source_thread_id: input.source_thread_id,
            target_agent_id: input.target_agent_id,
            target_thread_id: input.target_thread_id,
            channel_id: input.channel_id,
            goal_id: input.goal_id,
            task_id: input.task_id,
            risk_level: input.risk_level,
            trigger_turn: route_trigger_turn(&profile, input.route),
            requires_review: route_requires_review(&profile, input.route),
            payload: input.payload,
            artifact_refs: input.artifact_refs,
            evidence_refs: input.evidence_refs,
            delivery_status: if allowed {
                MessageDeliveryStatus::Delivered
            } else {
                MessageDeliveryStatus::Rejected
            },
            rejected_reason: reason.clone(),
            created_at: now.clone(),
            delivered_at: allowed.then_some(now),
        };
        self.emit(
            if allowed {
                "CommunicationMessageDelivered"
            } else {
                "CommunicationMessageRejected"
            },
            "message",
            &message.message_id,
            Some(message.source_agent_id.clone()),
            Some(message.task_id.clone()),
            causation_id,
            Some(message.goal_id.clone()),
            &message,
        )?;
        if !allowed {
            self.audit(
                AuditActorType::Agent,
                &message.source_agent_id,
                "communication.denied",
                "message",
                &message.message_id,
                reason,
                AuditResult::Deny,
            )?;
        }
        Ok(message)
    }
}

fn communication_allowed(
    profile: &CommunicationProfile,
    input: &SendMessageInput,
) -> (bool, Option<String>) {
    match input.route {
        MessageRoute::Supervisor => route_allowed(
            profile.supervisor.enabled,
            &profile.supervisor.allowed_message_types,
            &input.message_type,
            "supervisor route is not allowed",
        ),
        MessageRoute::Blackboard => {
            if !profile.blackboard.enabled {
                return (false, Some("blackboard route is not allowed".to_string()));
            }
            if input.message_type != "BlackboardPost" {
                return (
                    false,
                    Some("blackboard route requires BlackboardPost message type".to_string()),
                );
            }
            let scope = input
                .payload
                .get("scope")
                .and_then(Value::as_str)
                .map(parse_comm_scope)
                .unwrap_or(CommunicationScope::Task);
            if scope > profile.blackboard.allowed_scopes {
                return (false, Some("blackboard scope exceeds profile".to_string()));
            }
            if let Some(channel) = &input.channel_id {
                if !profile.blackboard.allowed_channels.contains(channel) {
                    return (false, Some("blackboard channel is not allowed".to_string()));
                }
            }
            let Some(entry_type) = input.payload.get("entry_type").and_then(Value::as_str) else {
                return (
                    false,
                    Some("blackboard post requires entry_type".to_string()),
                );
            };
            if !profile
                .blackboard
                .allowed_entry_types
                .iter()
                .any(|allowed| allowed == entry_type)
            {
                return (
                    false,
                    Some("blackboard entry type is not allowed".to_string()),
                );
            }
            (true, None)
        }
        MessageRoute::Human => route_allowed(
            profile.human.enabled,
            &profile.human.allowed_message_types,
            &input.message_type,
            "human route is not allowed",
        ),
    }
}

fn route_allowed(
    enabled: bool,
    allowed_types: &[String],
    message_type: &str,
    disabled_reason: &str,
) -> (bool, Option<String>) {
    if !enabled {
        return (false, Some(disabled_reason.to_string()));
    }
    if !wildcard_allows(allowed_types, message_type) {
        return (false, Some("message type is not allowed".to_string()));
    }
    (true, None)
}

fn route_trigger_turn(profile: &CommunicationProfile, route: MessageRoute) -> bool {
    match route {
        MessageRoute::Supervisor => profile.supervisor.trigger_turn,
        MessageRoute::Blackboard => false,
        MessageRoute::Human => false,
    }
}

fn route_requires_review(profile: &CommunicationProfile, route: MessageRoute) -> bool {
    match route {
        MessageRoute::Blackboard => profile.blackboard.requires_review,
        MessageRoute::Human => profile.human.requires_supervisor_approval,
        _ => false,
    }
}

fn parse_comm_scope(value: &str) -> CommunicationScope {
    match value {
        "task" => CommunicationScope::Task,
        "goal" => CommunicationScope::Goal,
        "global" => CommunicationScope::Global,
        _ => CommunicationScope::None,
    }
}
