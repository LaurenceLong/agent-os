use crate::*;
use agent_os_sys::*;

impl Kernel {
    pub fn post_blackboard(&self, input: PostBlackboardInput) -> AgentOsResult<BlackboardEntry> {
        self.post_blackboard_with_cause(input, None)
    }

    pub(crate) fn post_blackboard_with_cause(
        &self,
        input: PostBlackboardInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<BlackboardEntry> {
        let (agent_id, task_id_for_event) = self.validate_blackboard_post(&input)?;
        let entry = BlackboardEntry {
            entry_id: new_id("bb_"),
            goal_id: input.goal_id,
            task_id: input.task_id,
            section: input.section,
            status: BlackboardStatus::Active,
            content: input.content,
            confidence: input.confidence,
            source_evidence_ids: input.source_evidence_ids,
            created_by_agent_id: Some(input.source_agent_id),
            created_at: now_rfc3339(),
            superseded_by: None,
        };
        self.emit(
            "BlackboardPostSubmitted",
            "blackboard_entry",
            &entry.entry_id,
            Some(agent_id.clone()),
            task_id_for_event.clone(),
            causation_id.clone(),
            Some(entry.goal_id.clone()),
            &entry,
        )?;
        self.emit(
            "BlackboardPostPublished",
            "blackboard_entry",
            &entry.entry_id,
            Some(agent_id),
            task_id_for_event,
            causation_id,
            Some(entry.goal_id.clone()),
            &entry,
        )?;
        Ok(entry)
    }

    fn validate_blackboard_post(
        &self,
        input: &PostBlackboardInput,
    ) -> AgentOsResult<(String, Option<String>)> {
        let state = self.read_state()?;
        let acb = state
            .threads
            .get(&input.source_thread_id)
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {}", input.source_thread_id)))?;
        if acb.agent_id != input.source_agent_id {
            return Err(AgentOsError::Validation(
                "blackboard source agent does not match thread".to_string(),
            ));
        }
        if !state.goals.contains_key(&input.goal_id) {
            return Err(AgentOsError::NotFound(format!("goal {}", input.goal_id)));
        }
        if input.goal_id != acb.task.goal_id {
            return Err(AgentOsError::PermissionDenied(
                "blackboard post goal must match source thread goal".to_string(),
            ));
        }
        if input.scope == CommunicationScope::Task && input.task_id.is_none() {
            return Err(AgentOsError::Validation(
                "task-scoped blackboard post requires task_id".to_string(),
            ));
        }
        if let Some(task_id) = &input.task_id {
            let task = state
                .tasks
                .get(task_id)
                .ok_or_else(|| AgentOsError::NotFound(format!("task {task_id}")))?;
            if task.goal_id != input.goal_id {
                return Err(AgentOsError::Validation(
                    "blackboard task does not belong to goal".to_string(),
                ));
            }
        }
        let profile = state
            .communication_profiles
            .get(&acb.config_snapshot.communication_profile_id)
            .ok_or_else(|| {
                AgentOsError::NotFound(format!(
                    "communication profile {}",
                    acb.config_snapshot.communication_profile_id
                ))
            })?;
        if !profile.blackboard.enabled {
            return Err(AgentOsError::PermissionDenied(
                "blackboard route is not allowed".to_string(),
            ));
        }
        if input.scope > profile.blackboard.allowed_scopes {
            return Err(AgentOsError::PermissionDenied(
                "blackboard scope exceeds profile".to_string(),
            ));
        }
        let channel_id = input.channel_id.as_deref().ok_or_else(|| {
            AgentOsError::Validation("blackboard post requires a channel".to_string())
        })?;
        if !profile
            .blackboard
            .allowed_channels
            .iter()
            .any(|allowed| allowed == channel_id)
        {
            return Err(AgentOsError::PermissionDenied(
                "blackboard channel is not allowed".to_string(),
            ));
        }
        let section = blackboard_section_key(input.section);
        if !profile
            .blackboard
            .allowed_entry_types
            .iter()
            .any(|allowed| allowed == section)
        {
            return Err(AgentOsError::PermissionDenied(
                "blackboard entry type is not allowed".to_string(),
            ));
        }
        if matches!(
            input.section,
            BlackboardSection::KnownFact | BlackboardSection::Decision
        ) && input.source_evidence_ids.is_empty()
        {
            return Err(AgentOsError::Validation(
                "facts and decisions require evidence provenance".to_string(),
            ));
        }
        for evidence_id in &input.source_evidence_ids {
            let evidence = state
                .evidence
                .get(evidence_id)
                .ok_or_else(|| AgentOsError::NotFound(format!("evidence {evidence_id}")))?;
            if evidence.status != EvidenceStatus::Active {
                return Err(AgentOsError::Validation(
                    "blackboard provenance evidence must be active".to_string(),
                ));
            }
            if evidence.goal_id != input.goal_id {
                return Err(AgentOsError::Validation(
                    "blackboard provenance evidence must belong to goal".to_string(),
                ));
            }
        }
        Ok((acb.agent_id.clone(), input.task_id.clone()))
    }
}

pub(crate) fn blackboard_section_key(section: BlackboardSection) -> &'static str {
    match section {
        BlackboardSection::Goal => "goal",
        BlackboardSection::Constraint => "constraint",
        BlackboardSection::KnownFact => "known_fact",
        BlackboardSection::Hypothesis => "hypothesis",
        BlackboardSection::Decision => "decision",
        BlackboardSection::OpenQuestion => "open_question",
        BlackboardSection::Risk => "risk",
        BlackboardSection::TestResult => "test_result",
        BlackboardSection::ReviewResult => "review_result",
        BlackboardSection::AcceptanceCriterion => "acceptance_criterion",
    }
}
