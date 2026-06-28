use crate::*;
use agent_os_sys::*;

impl Kernel {
    pub fn register_goal(&self, input: RegisterGoalInput) -> AgentOsResult<Goal> {
        self.register_goal_with_cause(input, None)
    }

    pub(crate) fn register_goal_with_cause(
        &self,
        input: RegisterGoalInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<Goal> {
        if input.acceptance_criteria.is_empty() {
            return Err(AgentOsError::Validation(
                "goal must have acceptance criteria before execution begins".to_string(),
            ));
        }
        let now = now_rfc3339();
        let goal = Goal {
            goal_id: new_id("goal_"),
            namespace: input.namespace,
            created_by: input.created_by,
            status: GoalStatus::Registered,
            title: input.title,
            description: input.description,
            acceptance_criteria: input.acceptance_criteria,
            constraints: input.constraints,
            risk_level: input.risk_level,
            deadline: input.deadline,
            root_task_id: None,
            created_at: now.clone(),
            updated_at: now,
        };
        self.emit(
            "GoalRegistered",
            "goal",
            &goal.goal_id,
            None,
            None,
            causation_id,
            Some(goal.goal_id.clone()),
            &goal,
        )?;
        Ok(goal)
    }
}
