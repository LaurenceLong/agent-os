use crate::*;
use agent_os_sys::*;

impl Kernel {
    pub fn load_context(&self, input: LoadContextInput) -> AgentOsResult<ContextSnapshot> {
        self.load_context_with_cause(input, None)
    }

    pub(crate) fn load_context_with_cause(
        &self,
        input: LoadContextInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<ContextSnapshot> {
        self.validate_context_load(&input)?;
        let snapshot = ContextSnapshot {
            context_id: new_id("ctx_"),
            agent_id: input.agent_id,
            task_id: input.task_id,
            loaded_refs: input.loaded_refs,
            summary_artifact_id: input.summary_artifact_id,
            freshness: input.freshness,
            pollution_score: input.pollution_score,
            token_estimate: input.token_estimate,
            created_at: now_rfc3339(),
            invalidated_at: None,
        };
        self.emit(
            "ContextLoaded",
            "context",
            &snapshot.context_id,
            Some(snapshot.agent_id.clone()),
            Some(snapshot.task_id.clone()),
            causation_id,
            None,
            &snapshot,
        )?;
        Ok(snapshot)
    }

    fn validate_context_load(&self, input: &LoadContextInput) -> AgentOsResult<()> {
        if input.loaded_refs.is_empty() && input.summary_artifact_id.is_none() {
            return Err(AgentOsError::Validation(
                "context load requires at least one loaded ref or summary artifact".to_string(),
            ));
        }
        if !(0.0..=1.0).contains(&input.pollution_score) {
            return Err(AgentOsError::Validation(
                "context pollution score must be between 0.0 and 1.0".to_string(),
            ));
        }
        let state = self.read_state()?;
        let acb = state
            .threads
            .values()
            .find(|thread| thread.agent_id == input.agent_id)
            .ok_or_else(|| AgentOsError::NotFound(format!("agent {}", input.agent_id)))?;
        if acb.task.task_id != input.task_id {
            return Err(AgentOsError::PermissionDenied(
                "context load task must match agent thread task".to_string(),
            ));
        }
        if let Some(artifact_id) = &input.summary_artifact_id {
            let artifact = state
                .artifacts
                .get(artifact_id)
                .ok_or_else(|| AgentOsError::NotFound(format!("artifact {artifact_id}")))?;
            if artifact.task_id != input.task_id {
                return Err(AgentOsError::Validation(
                    "context summary artifact must belong to task".to_string(),
                ));
            }
        }
        Ok(())
    }
}
