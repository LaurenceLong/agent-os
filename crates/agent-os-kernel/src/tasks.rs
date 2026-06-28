use crate::*;
use agent_os_sys::*;
use std::collections::HashSet;

impl Kernel {
    pub fn spawn_task(&self, input: SpawnTaskInput) -> AgentOsResult<Task> {
        self.spawn_task_with_cause(input, None)
    }

    pub fn update_task(&self, input: UpdateTaskInput) -> AgentOsResult<Task> {
        self.update_task_with_cause(input, None)
    }

    pub fn complete_task(&self, input: CompleteTaskInput) -> AgentOsResult<Task> {
        self.complete_task_with_cause(input, None)
    }

    pub(crate) fn spawn_task_with_cause(
        &self,
        input: SpawnTaskInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<Task> {
        {
            let state = self.read_state()?;
            if !state.goals.contains_key(&input.goal_id) {
                return Err(AgentOsError::NotFound(format!("goal {}", input.goal_id)));
            }
            if let Some(parent) = &input.parent_task_id {
                if !state.tasks.contains_key(parent) {
                    return Err(AgentOsError::NotFound(format!("parent task {parent}")));
                }
            }
            for dep in &input.depends_on {
                if !state.tasks.contains_key(dep) {
                    return Err(AgentOsError::NotFound(format!("dependency task {dep}")));
                }
            }
        }
        let now = now_rfc3339();
        let task = Task {
            task_id: new_id("task_"),
            goal_id: input.goal_id,
            parent_task_id: input.parent_task_id,
            status: TaskStatus::Created,
            title: input.title,
            description: input.description,
            checklist: Vec::new(),
            owner_agent_id: None,
            depends_on: input.depends_on,
            blocks: Vec::new(),
            required_artifact_types: input.required_artifact_types,
            required_evidence_types: input.required_evidence_types,
            blocked_reason: None,
            priority: input.priority,
            risk_level: input.risk_level,
            created_at: now.clone(),
            updated_at: now,
        };
        self.emit(
            "TaskSpawned",
            "task",
            &task.task_id,
            None,
            Some(task.task_id.clone()),
            causation_id,
            Some(task.goal_id.clone()),
            &task,
        )?;
        Ok(task)
    }

    pub(crate) fn update_task_with_cause(
        &self,
        input: UpdateTaskInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<Task> {
        let current = self
            .read_state()?
            .tasks
            .get(&input.task_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("task {}", input.task_id)))?;
        let mut task = current;
        if let Some(next_status) = input.status {
            self.validate_task_transition(&task, next_status)?;
            task.status = next_status;
        }
        if input.blocked_reason.is_some() {
            task.blocked_reason = input.blocked_reason;
        }
        if input.owner_agent_id.is_some() {
            task.owner_agent_id = input.owner_agent_id;
        }
        if let Some(title) = input.title {
            task.title = title;
        }
        if let Some(description) = input.description {
            task.description = description;
        }
        if let Some(checklist) = input.checklist {
            task.checklist = checklist;
        }
        task.updated_at = now_rfc3339();
        self.emit(
            "TaskUpdated",
            "task",
            &task.task_id,
            task.owner_agent_id.clone(),
            Some(task.task_id.clone()),
            causation_id,
            Some(task.goal_id.clone()),
            &task,
        )?;
        Ok(task)
    }

    pub(crate) fn complete_task_with_cause(
        &self,
        input: CompleteTaskInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<Task> {
        let state = self.read_state()?;
        let current = state
            .tasks
            .get(&input.task_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("task {}", input.task_id)))?;
        let provided_artifact_types: HashSet<_> = input
            .artifact_ids
            .iter()
            .filter_map(|id| state.artifacts.get(id))
            .map(|artifact| artifact.artifact_type)
            .collect();
        let provided_evidence_types: HashSet<_> = input
            .evidence_ids
            .iter()
            .filter_map(|id| state.evidence.get(id))
            .filter(|evidence| evidence.status == EvidenceStatus::Active)
            .map(|evidence| evidence.evidence_type)
            .collect();
        for required in &current.required_artifact_types {
            if !provided_artifact_types.contains(required) {
                return Err(AgentOsError::Validation(format!(
                    "task completion missing required artifact type {:?}",
                    required
                )));
            }
        }
        for required in &current.required_evidence_types {
            if !provided_evidence_types.contains(required) {
                return Err(AgentOsError::Validation(format!(
                    "task completion missing required evidence type {:?}",
                    required
                )));
            }
        }
        drop(state);
        let mut task = current;
        task.status = TaskStatus::Completed;
        task.updated_at = now_rfc3339();
        self.emit(
            "TaskCompleted",
            "task",
            &task.task_id,
            task.owner_agent_id.clone(),
            Some(task.task_id.clone()),
            causation_id,
            Some(task.goal_id.clone()),
            &task,
        )?;
        Ok(task)
    }

    fn validate_task_transition(&self, task: &Task, next: TaskStatus) -> AgentOsResult<()> {
        if task.status == next {
            return Ok(());
        }
        if matches!(
            task.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        ) {
            return Err(AgentOsError::InvalidTransition(format!(
                "task {:?} -> {:?}",
                task.status, next
            )));
        }
        if next == TaskStatus::Ready {
            let state = self.read_state()?;
            for dependency_id in &task.depends_on {
                let dependency = state.tasks.get(dependency_id).ok_or_else(|| {
                    AgentOsError::NotFound(format!("dependency task {dependency_id}"))
                })?;
                if dependency.status != TaskStatus::Completed {
                    return Err(AgentOsError::InvalidTransition(format!(
                        "task {} cannot become Ready until dependency {} is Completed",
                        task.task_id, dependency_id
                    )));
                }
            }
        }
        Ok(())
    }
}
