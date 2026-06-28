use crate::*;
use agent_os_sys::*;

impl Kernel {
    pub fn spawn_agent(&self, input: SpawnAgentInput) -> AgentOsResult<AgentControlBlock> {
        self.spawn_agent_with_cause(input, None)
    }

    pub fn transition_thread(
        &self,
        thread_id: &str,
        next: ThreadStatus,
        reason: Option<String>,
    ) -> AgentOsResult<AgentControlBlock> {
        self.transition_thread_with_cause(thread_id, next, reason, None)
    }

    pub fn start_turn(&self, thread_id: &str) -> AgentOsResult<AgentControlBlock> {
        let acb = self
            .read_state()?
            .threads
            .get(thread_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {thread_id}")))?;
        if matches!(
            acb.status,
            ThreadStatus::Running
                | ThreadStatus::WaitingTool
                | ThreadStatus::WaitingPermission
                | ThreadStatus::WaitingUser
        ) {
            return Err(AgentOsError::InvalidTransition(
                "turn.start rejected because thread is busy".to_string(),
            ));
        }
        if !valid_thread_transition(acb.status, ThreadStatus::Running) {
            return Err(AgentOsError::InvalidTransition(format!(
                "thread {:?} -> {:?}",
                acb.status,
                ThreadStatus::Running
            )));
        }
        let mut next = acb.clone();
        next.status = ThreadStatus::Running;
        next.status_reason = None;
        next.active_turn.turn_id = Some(new_id("turn_"));
        next.active_turn.status = Some(TurnStatus::InProgress);
        next.active_turn.started_at = Some(now_rfc3339());
        next.audit.updated_at = now_rfc3339();
        self.emit(
            "TurnStarted",
            "thread",
            &next.thread_id,
            Some(next.agent_id.clone()),
            Some(next.task.task_id.clone()),
            None,
            Some(next.task.goal_id.clone()),
            &next,
        )?;
        Ok(next)
    }

    pub fn record_checkpoint(
        &self,
        thread_id: &str,
        checkpoint_id: impl Into<String>,
    ) -> AgentOsResult<AgentControlBlock> {
        let mut acb = self
            .read_state()?
            .threads
            .get(thread_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {thread_id}")))?;
        acb.recovery.last_checkpoint_id = Some(checkpoint_id.into());
        acb.recovery.dirty = false;
        acb.audit.updated_at = now_rfc3339();
        self.emit(
            "CheckpointCommitted",
            "thread",
            &acb.thread_id,
            Some(acb.agent_id.clone()),
            Some(acb.task.task_id.clone()),
            None,
            Some(acb.task.goal_id.clone()),
            &acb,
        )?;
        Ok(acb)
    }

    pub(crate) fn spawn_agent_with_cause(
        &self,
        input: SpawnAgentInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<AgentControlBlock> {
        let task = self
            .read_state()?
            .tasks
            .get(&input.task_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("task {}", input.task_id)))?;
        let role = self.active_role(&input.role_profile_id)?;
        let permission = self.active_permission(&role.default_permission_profile_id)?;
        let sandbox = self.active_sandbox(&role.default_sandbox_profile_id)?;
        let parent_thread_id = input.parent_thread_id.clone();
        let parent = if let Some(parent_thread_id) = &parent_thread_id {
            let parent = self
                .read_state()?
                .threads
                .get(parent_thread_id)
                .cloned()
                .ok_or_else(|| {
                    AgentOsError::NotFound(format!("parent thread {parent_thread_id}"))
                })?;
            if !parent
                .config_snapshot
                .effective_binding
                .role_profile_id
                .is_empty()
            {
                let parent_role =
                    self.active_role(&parent.config_snapshot.effective_binding.role_profile_id)?;
                if !parent_role
                    .allowed_child_role_profile_ids
                    .contains(&input.role_profile_id)
                    && !parent_role
                        .allowed_child_role_profile_ids
                        .contains(&"*".to_string())
                {
                    return Err(AgentOsError::PermissionDenied(
                        "parent role cannot spawn requested child role".to_string(),
                    ));
                }
            }
            Some(parent)
        } else {
            None
        };

        let now = now_rfc3339();
        let thread_id = new_id("thread_");
        let agent_id = new_id("agt_");
        let invocation_id = new_id("inv_");
        let communication_profile_id = new_id("comm_");
        let root_thread_id = parent
            .as_ref()
            .map(|parent| parent.root_thread_id.clone())
            .unwrap_or_else(|| thread_id.clone());
        let agent_path = match &parent_thread_id {
            Some(parent) => format!("/{}/{}", parent.trim_start_matches("thread_"), role.name),
            None => "/".to_string(),
        };
        let supervisor_level = supervisor_level_for_spawn(parent.as_ref(), &role)?;
        let relationship = invocation_relationship(parent.as_ref(), &role);
        let assignment = input.local_goal.clone();
        let caller_supervisor_level = parent.as_ref().and_then(|parent| parent.supervisor_level);
        let provider_profile_id = role
            .default_provider_profile_id
            .clone()
            .unwrap_or_else(|| "prov_default".to_string());
        let routing_policy_id = "route_default".to_string();
        let binding = EffectiveBindingSnapshot {
            role_profile_id: role.role_profile_id.clone(),
            permission_profile_id: permission.permission_profile_id.clone(),
            sandbox_profile_id: sandbox.sandbox_profile_id.clone(),
            provider_profile_id: role.default_provider_profile_id.clone(),
            scheduler_policy_id: role.default_scheduler_policy_id.clone(),
            communication_profile_id: communication_profile_id.clone(),
            reasoning_profile: None,
            revision: 1,
            resolved_at: now.clone(),
        };
        let acb = AgentControlBlock {
            thread_id: thread_id.clone(),
            agent_id: agent_id.clone(),
            invocation_id: invocation_id.clone(),
            session_id: new_id("sess_"),
            root_thread_id,
            parent_thread_id,
            supervisor_level,
            agent_path,
            role: role.name.clone(),
            owner: input.owner.clone(),
            status: ThreadStatus::Created,
            status_reason: None,
            task: ThreadTaskBinding {
                task_id: task.task_id.clone(),
                goal_id: task.goal_id.clone(),
                local_goal: input.local_goal,
                success_criteria: input.success_criteria,
                failure_criteria: input.failure_criteria,
            },
            config_snapshot: ThreadConfigSnapshot {
                model_provider_id: "mock-provider".to_string(),
                model_id: "mock-model".to_string(),
                provider_profile_id,
                model_routing_policy_id: routing_policy_id,
                provider_adapter_version: "mock-0.1".to_string(),
                role_profile_id: role.role_profile_id.clone(),
                communication_profile_id: communication_profile_id.clone(),
                permission_profile_id: permission.permission_profile_id.clone(),
                sandbox_profile_id: sandbox.sandbox_profile_id.clone(),
                context_policy_id: "ctx_default".to_string(),
                memory_policy_id: "mem_default".to_string(),
                tool_registry_snapshot_id: "tools_default".to_string(),
                workspace_roots: input.workspace_roots,
                environment_ids: Vec::new(),
                reasoning_profile: None,
                effective_binding: binding,
            },
            queues: ThreadQueues::default(),
            active_turn: ActiveTurn::default(),
            resources: ThreadResources::default(),
            budgets: ThreadBudgets {
                token_budget: None,
                tool_call_budget: None,
                wall_time_budget_ms: None,
                cost_budget: None,
                max_steps_per_turn: 16,
                max_child_threads: 4,
            },
            recovery: ThreadRecovery {
                last_checkpoint_id: None,
                replay_cursor: None,
                last_materialized_event_sequence: 0,
                dirty: false,
            },
            audit: ThreadAudit {
                created_at: now.clone(),
                updated_at: now.clone(),
                created_by: input.owner,
                termination_reason: None,
            },
        };
        let communication_profile = self.default_communication_profile(
            &communication_profile_id,
            &agent_id,
            &thread_id,
            &role,
        );
        let invocation = AgentInvocation {
            invocation_id,
            goal_id: task.goal_id.clone(),
            task_id: task.task_id.clone(),
            caller_thread_id: parent.as_ref().map(|parent| parent.thread_id.clone()),
            caller_agent_id: parent.as_ref().map(|parent| parent.agent_id.clone()),
            caller_supervisor_level,
            callee_thread_id: thread_id.clone(),
            callee_agent_id: agent_id.clone(),
            callee_supervisor_level: acb.supervisor_level,
            relationship,
            assignment,
            status: AgentInvocationStatus::Active,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        self.emit(
            "AgentInvocationRecorded",
            "agent_invocation",
            &invocation.invocation_id,
            Some(agent_id.clone()),
            Some(task.task_id.clone()),
            causation_id.clone(),
            Some(task.goal_id.clone()),
            &invocation,
        )?;
        self.emit(
            "CommunicationProfileAssigned",
            "communication_profile",
            &communication_profile.communication_profile_id,
            Some(agent_id.clone()),
            Some(task.task_id.clone()),
            causation_id.clone(),
            Some(task.goal_id.clone()),
            &communication_profile,
        )?;
        self.emit(
            "ThreadConfigured",
            "thread",
            &thread_id,
            Some(agent_id),
            Some(task.task_id),
            causation_id,
            Some(task.goal_id),
            &acb,
        )?;
        Ok(acb)
    }

    pub(crate) fn transition_thread_by_agent(
        &self,
        agent_id: &str,
        next: ThreadStatus,
        reason: Option<String>,
        causation_id: Option<String>,
    ) -> AgentOsResult<AgentControlBlock> {
        let acb = self
            .thread_by_agent(agent_id)?
            .ok_or_else(|| AgentOsError::NotFound(format!("agent {agent_id}")))?;
        self.transition_thread_with_cause(&acb.thread_id, next, reason, causation_id)
    }

    pub(crate) fn transition_thread_with_cause(
        &self,
        thread_id: &str,
        next: ThreadStatus,
        reason: Option<String>,
        causation_id: Option<String>,
    ) -> AgentOsResult<AgentControlBlock> {
        let current = self
            .read_state()?
            .threads
            .get(thread_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {thread_id}")))?;
        if !valid_thread_transition(current.status, next) {
            return Err(AgentOsError::InvalidTransition(format!(
                "thread {:?} -> {:?}",
                current.status, next
            )));
        }
        let mut acb = current;
        acb.status = next;
        acb.status_reason = reason.clone();
        acb.audit.updated_at = now_rfc3339();
        if matches!(
            next,
            ThreadStatus::Completed | ThreadStatus::Failed | ThreadStatus::Terminated
        ) {
            acb.audit.termination_reason = reason;
            acb.active_turn.status = Some(match next {
                ThreadStatus::Completed => TurnStatus::Completed,
                ThreadStatus::Failed => TurnStatus::Failed,
                _ => TurnStatus::Interrupted,
            });
        }
        self.emit(
            "ThreadStatusChanged",
            "thread",
            &acb.thread_id,
            Some(acb.agent_id.clone()),
            Some(acb.task.task_id.clone()),
            causation_id,
            Some(acb.task.goal_id.clone()),
            &acb,
        )?;
        Ok(acb)
    }
}

fn valid_thread_transition(current: ThreadStatus, next: ThreadStatus) -> bool {
    if current == next {
        return true;
    }
    match current {
        ThreadStatus::Created => matches!(
            next,
            ThreadStatus::Ready
                | ThreadStatus::Running
                | ThreadStatus::Failed
                | ThreadStatus::Terminated
        ),
        ThreadStatus::Ready => matches!(
            next,
            ThreadStatus::Running
                | ThreadStatus::Blocked
                | ThreadStatus::Suspended
                | ThreadStatus::Unloaded
                | ThreadStatus::Failed
                | ThreadStatus::Terminated
        ),
        ThreadStatus::Running => matches!(
            next,
            ThreadStatus::Ready
                | ThreadStatus::WaitingTool
                | ThreadStatus::WaitingPermission
                | ThreadStatus::WaitingUser
                | ThreadStatus::Blocked
                | ThreadStatus::Completing
                | ThreadStatus::Completed
                | ThreadStatus::Failed
                | ThreadStatus::Interrupted
                | ThreadStatus::Suspended
        ),
        ThreadStatus::WaitingTool | ThreadStatus::WaitingPermission | ThreadStatus::WaitingUser => {
            matches!(
                next,
                ThreadStatus::Ready
                    | ThreadStatus::Running
                    | ThreadStatus::Blocked
                    | ThreadStatus::Failed
                    | ThreadStatus::Interrupted
                    | ThreadStatus::Suspended
            )
        }
        ThreadStatus::Blocked
        | ThreadStatus::Suspended
        | ThreadStatus::ResidentIdle
        | ThreadStatus::Unloaded => {
            matches!(
                next,
                ThreadStatus::Ready
                    | ThreadStatus::Running
                    | ThreadStatus::Failed
                    | ThreadStatus::Terminated
                    | ThreadStatus::Interrupted
            )
        }
        ThreadStatus::Completing => matches!(next, ThreadStatus::Completed | ThreadStatus::Failed),
        ThreadStatus::Interrupted => matches!(
            next,
            ThreadStatus::Ready
                | ThreadStatus::Running
                | ThreadStatus::Failed
                | ThreadStatus::Terminated
        ),
        ThreadStatus::Quarantined => {
            matches!(next, ThreadStatus::Terminated | ThreadStatus::Failed)
        }
        ThreadStatus::Completed | ThreadStatus::Failed | ThreadStatus::Terminated => false,
    }
}

fn supervisor_level_for_spawn(
    parent: Option<&AgentControlBlock>,
    role: &RoleProfile,
) -> AgentOsResult<Option<u32>> {
    if role.name != "SupervisorAgent" {
        return Ok(None);
    }
    match parent {
        None => Ok(Some(0)),
        Some(parent) => parent
            .supervisor_level
            .map(|level| Some(level + 1))
            .ok_or_else(|| {
                AgentOsError::PermissionDenied(
                    "only a SupervisorAgent can delegate another SupervisorAgent".to_string(),
                )
            }),
    }
}

fn invocation_relationship(
    parent: Option<&AgentControlBlock>,
    role: &RoleProfile,
) -> AgentInvocationRelationship {
    if role.name == "SupervisorAgent" {
        if parent.is_some() {
            AgentInvocationRelationship::SupervisorDelegation
        } else {
            AgentInvocationRelationship::RootSupervisor
        }
    } else if role.name == "ReviewerAgent" {
        AgentInvocationRelationship::ReviewRequest
    } else {
        AgentInvocationRelationship::WorkerAssignment
    }
}
