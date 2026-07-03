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
        // Scheduler admission: budget ledgers and task dependencies are
        // durable admission-control signals.
        match self.evaluate_turn_admission(thread_id)? {
            AdmissionDecision::Allowed => {}
            AdmissionDecision::Rejected(rejection) => match rejection {
                AdmissionRejection::OutOfBudget {
                    scope_id,
                    budget_ledger_id,
                } => {
                    return Err(AgentOsError::BudgetExhausted(format!(
                        "turn.start rejected: budget ledger {budget_ledger_id} for scope {scope_id} is exhausted"
                    )))
                }
                AdmissionRejection::DependencyBlocked { task_id, blocked_on } => {
                    return Err(AgentOsError::InvalidTransition(format!(
                        "turn.start rejected: task {task_id} is blocked on dependencies {blocked_on:?}"
                    )))
                }
                AdmissionRejection::ProviderSlotUnavailable { provider_id } => {
                    return Err(AgentOsError::ResourceConflict(format!(
                        "turn.start rejected: provider slot {provider_id} is unavailable"
                    )))
                }
            },
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

    pub fn archive_thread(&self, thread_id: &str) -> AgentOsResult<ClientThread> {
        let acb = self.thread_for_app_update(thread_id)?;
        let mut thread = self.current_client_thread(&acb)?;
        thread.archived = true;
        thread.updated_at = now_rfc3339();
        self.emit_client_thread_update("ThreadArchived", &acb, &thread)?;
        Ok(thread)
    }

    pub fn unarchive_thread(&self, thread_id: &str) -> AgentOsResult<ClientThread> {
        let acb = self.thread_for_app_update(thread_id)?;
        let mut thread = self.current_client_thread(&acb)?;
        thread.archived = false;
        thread.updated_at = now_rfc3339();
        self.emit_client_thread_update("ThreadUnarchived", &acb, &thread)?;
        Ok(thread)
    }

    pub fn delete_thread(&self, thread_id: &str) -> AgentOsResult<ClientThread> {
        let acb = self.thread_for_app_update(thread_id)?;
        let mut thread = self.current_client_thread(&acb)?;
        thread.archived = true;
        thread.deleted = true;
        thread.updated_at = now_rfc3339();
        self.emit_client_thread_update("ThreadDeleted", &acb, &thread)?;
        Ok(thread)
    }

    pub fn rename_thread(&self, thread_id: &str, title: String) -> AgentOsResult<ClientThread> {
        let title = title.trim().to_string();
        if title.is_empty() {
            return Err(AgentOsError::Validation(
                "thread title must not be empty".to_string(),
            ));
        }
        let acb = self.thread_for_app_update(thread_id)?;
        let mut thread = self.current_client_thread(&acb)?;
        thread.title = title;
        thread.updated_at = now_rfc3339();
        self.emit_client_thread_update("ThreadRenamed", &acb, &thread)?;
        Ok(thread)
    }

    pub fn record_turn_input(
        &self,
        client: &ClientConnection,
        client_thread_id: &str,
        turn_id: &str,
        kind: TurnInputKind,
        input: String,
    ) -> AgentOsResult<TurnInputRecord> {
        let acb = self
            .read_state()?
            .threads
            .get(client_thread_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {client_thread_id}")))?;
        if acb.active_turn.turn_id.as_deref() != Some(turn_id) {
            return Err(AgentOsError::Validation(format!(
                "turn {turn_id} is not active on thread {client_thread_id}"
            )));
        }
        let record = TurnInputRecord {
            input_id: new_id("turn_input_"),
            client_thread_id: client_thread_id.to_string(),
            turn_id: turn_id.to_string(),
            submitted_by_client_id: client.client_id.clone(),
            kind,
            input,
            created_at: now_rfc3339(),
        };
        self.emit(
            "TurnInputRecorded",
            "thread",
            client_thread_id,
            Some(acb.agent_id),
            Some(acb.task.task_id),
            None,
            Some(acb.task.goal_id),
            &record,
        )?;
        Ok(record)
    }

    fn thread_for_app_update(&self, thread_id: &str) -> AgentOsResult<AgentControlBlock> {
        let acb = self
            .read_state()?
            .threads
            .get(thread_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {thread_id}")))?;
        let current = self.current_client_thread(&acb)?;
        if current.deleted {
            return Err(AgentOsError::InvalidTransition(format!(
                "thread {thread_id} is deleted"
            )));
        }
        Ok(acb)
    }

    fn current_client_thread(&self, acb: &AgentControlBlock) -> AgentOsResult<ClientThread> {
        let existing = self
            .store()
            .thread_summaries()?
            .into_iter()
            .find(|thread| thread.client_thread_id == acb.thread_id);
        Ok(ClientThread {
            client_thread_id: acb.thread_id.clone(),
            agent_thread_id: acb.thread_id.clone(),
            task_id: Some(acb.task.task_id.clone()),
            goal_id: Some(acb.task.goal_id.clone()),
            title: existing
                .as_ref()
                .map(|thread| thread.title.clone())
                .unwrap_or_else(|| acb.task.goal.clone()),
            status: acb.status,
            active_turn_id: acb.active_turn.turn_id.clone(),
            archived: existing
                .as_ref()
                .map(|thread| thread.archived)
                .unwrap_or(false),
            deleted: existing
                .as_ref()
                .map(|thread| thread.deleted)
                .unwrap_or(false),
            updated_at: now_rfc3339(),
        })
    }

    fn emit_client_thread_update(
        &self,
        event_type: &str,
        acb: &AgentControlBlock,
        thread: &ClientThread,
    ) -> AgentOsResult<()> {
        self.emit(
            event_type,
            "thread",
            &acb.thread_id,
            Some(acb.agent_id.clone()),
            Some(acb.task.task_id.clone()),
            None,
            Some(acb.task.goal_id.clone()),
            thread,
        )?;
        Ok(())
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
        self.spawn_agent_with_permissions_with_cause(input, None, causation_id)
    }

    pub(crate) fn spawn_agent_with_permissions_with_cause(
        &self,
        input: SpawnAgentInput,
        explicit_permissions: Option<PermissionSet>,
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
        let security_level = parent
            .as_ref()
            .map(|parent| parent.security_level.child())
            .unwrap_or(SecurityLevel::ROOT_AGENT);
        let effective_permissions_snapshot = self.child_permission_snapshot(
            parent.as_ref(),
            &permission.permission_set,
            explicit_permissions,
        )?;
        let relationship = invocation_relationship(parent.as_ref(), &role);
        let goal_text = input.goal.clone();
        let caller_security_level = parent.as_ref().map(|parent| parent.security_level);
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
            security_level,
            agent_path,
            role: role.name.clone(),
            owner: input.owner.clone(),
            status: ThreadStatus::Created,
            status_reason: None,
            task: ThreadTaskBinding {
                task_id: task.task_id.clone(),
                goal_id: task.goal_id.clone(),
                goal: input.goal,
                goal_status: AgentGoalStatus::Active,
                goal_revision: 1,
                accomplished_at: None,
                success_criteria: input.success_criteria,
                failure_criteria: input.failure_criteria,
            },
            config_snapshot: ThreadConfigSnapshot {
                model_provider_id: "primary-provider".to_string(),
                model_id: "general-primary".to_string(),
                provider_profile_id,
                model_routing_policy_id: routing_policy_id,
                provider_adapter_version: "provider-adapter-0.1".to_string(),
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
            effective_permissions_snapshot,
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
            caller_security_level,
            callee_thread_id: thread_id.clone(),
            callee_agent_id: agent_id.clone(),
            callee_security_level: acb.security_level,
            relationship,
            goal: goal_text,
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

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn set_agent_goal_with_cause(
        &self,
        requester_agent_id: &str,
        target_thread_id: Option<String>,
        target_agent_id: Option<String>,
        goal: String,
        title: Option<String>,
        success_criteria: Option<Vec<String>>,
        failure_criteria: Option<Vec<String>>,
        causation_id: Option<String>,
    ) -> AgentOsResult<AgentControlBlock> {
        let state = self.read_state()?;
        let requester = state
            .threads
            .values()
            .find(|thread| thread.agent_id == requester_agent_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("agent {requester_agent_id}")))?;
        self.require_control_plane_security_level(&requester, "set_goal")?;
        if requester.role != "SupervisorAgent" {
            return Err(AgentOsError::PermissionDenied(
                "set_goal requires SupervisorAgent role".to_string(),
            ));
        }
        let target = match (target_thread_id.as_deref(), target_agent_id.as_deref()) {
            (Some(thread_id), Some(agent_id)) => {
                let target = state
                    .threads
                    .get(thread_id)
                    .cloned()
                    .ok_or_else(|| AgentOsError::NotFound(format!("thread {thread_id}")))?;
                if target.agent_id != agent_id {
                    return Err(AgentOsError::Validation(
                        "target_thread_id and target_agent_id identify different agents"
                            .to_string(),
                    ));
                }
                target
            }
            (Some(thread_id), None) => state
                .threads
                .get(thread_id)
                .cloned()
                .ok_or_else(|| AgentOsError::NotFound(format!("thread {thread_id}")))?,
            (None, Some(agent_id)) => state
                .threads
                .values()
                .find(|thread| thread.agent_id == agent_id)
                .cloned()
                .ok_or_else(|| AgentOsError::NotFound(format!("agent {agent_id}")))?,
            (None, None) => requester.clone(),
        };
        if target.thread_id != requester.thread_id
            && target.parent_thread_id.as_deref() != Some(&requester.thread_id)
        {
            return Err(AgentOsError::PermissionDenied(
                "set_goal can only target the Supervisor thread or a direct child".to_string(),
            ));
        }
        drop(state);

        let now = now_rfc3339();
        let mut acb = target;
        acb.task.goal = goal.clone();
        acb.task.goal_status = AgentGoalStatus::Active;
        acb.task.goal_revision = acb.task.goal_revision.saturating_add(1);
        acb.task.accomplished_at = None;
        if let Some(criteria) = success_criteria {
            acb.task.success_criteria = criteria;
        }
        if let Some(criteria) = failure_criteria {
            acb.task.failure_criteria = criteria;
        }
        acb.audit.updated_at = now.clone();
        self.emit(
            "ThreadConfigured",
            "thread",
            &acb.thread_id,
            Some(acb.agent_id.clone()),
            Some(acb.task.task_id.clone()),
            causation_id.clone(),
            Some(acb.task.goal_id.clone()),
            &acb,
        )?;

        if let Some(title) = title {
            self.update_task_with_cause(
                UpdateTaskInput {
                    task_id: acb.task.task_id.clone(),
                    status: None,
                    blocked_reason: None,
                    owner_agent_id: None,
                    title: Some(title),
                    description: None,
                    checklist: None,
                },
                causation_id.clone(),
            )?;
        }

        let invocation = self
            .read_state()?
            .agent_invocations
            .get(&acb.invocation_id)
            .cloned();
        if let Some(mut invocation) = invocation {
            invocation.goal = goal;
            invocation.status = AgentInvocationStatus::Active;
            invocation.updated_at = now;
            self.emit(
                "AgentInvocationRecorded",
                "agent_invocation",
                &invocation.invocation_id,
                Some(invocation.callee_agent_id.clone()),
                Some(invocation.task_id.clone()),
                causation_id,
                Some(invocation.goal_id.clone()),
                &invocation,
            )?;
        }
        Ok(acb)
    }

    pub(crate) fn accomplish_agent_goal_with_cause(
        &self,
        agent_id: &str,
        summary: String,
        evidence_refs: Vec<String>,
        artifact_refs: Vec<String>,
        known_risks: Vec<String>,
        causation_id: Option<String>,
    ) -> AgentOsResult<AgentGoalCompletion> {
        let mut acb = self
            .thread_by_agent(agent_id)?
            .ok_or_else(|| AgentOsError::NotFound(format!("agent {agent_id}")))?;
        let completed_at = now_rfc3339();
        acb.task.goal_status = AgentGoalStatus::Accomplished;
        acb.task.accomplished_at = Some(completed_at.clone());
        acb.status = ThreadStatus::Completing;
        acb.status_reason = Some(summary.clone());
        acb.active_turn.status = Some(TurnStatus::InProgress);
        acb.audit.updated_at = completed_at.clone();
        acb.recovery.dirty = true;

        let hooks_completed =
            self.complete_active_hooks_for_thread_with_cause(&acb.thread_id, causation_id.clone())?;
        self.complete_invocation_for_thread_with_cause(&acb.thread_id, causation_id.clone())?;
        let completion = AgentGoalCompletion {
            thread: acb.clone(),
            summary,
            evidence_refs,
            artifact_refs,
            known_risks,
            hooks_completed,
            completed_at,
        };
        self.emit(
            "AgentGoalAccomplished",
            "thread",
            &acb.thread_id,
            Some(acb.agent_id.clone()),
            Some(acb.task.task_id.clone()),
            causation_id,
            Some(acb.task.goal_id.clone()),
            &completion,
        )?;
        Ok(completion)
    }

    pub(crate) fn complete_active_hooks_for_thread_with_cause(
        &self,
        thread_id: &str,
        causation_id: Option<String>,
    ) -> AgentOsResult<usize> {
        self.close_active_hooks_for_thread_with_cause(
            thread_id,
            AgentHookStatus::Completed,
            causation_id,
        )
    }

    pub(crate) fn close_active_hooks_for_thread_with_cause(
        &self,
        thread_id: &str,
        status: AgentHookStatus,
        causation_id: Option<String>,
    ) -> AgentOsResult<usize> {
        let hooks = self
            .read_state()?
            .agent_hooks
            .values()
            .filter(|hook| hook.thread_id == thread_id && hook.status == AgentHookStatus::Active)
            .cloned()
            .collect::<Vec<_>>();
        let mut completed = 0usize;
        for mut hook in hooks {
            hook.status = status;
            hook.updated_at = now_rfc3339();
            self.emit(
                "AgentHookUpdated",
                "agent_hook",
                &hook.hook_id,
                Some(hook.agent_id.clone()),
                None,
                causation_id.clone(),
                None,
                &hook,
            )?;
            completed += 1;
        }
        Ok(completed)
    }

    pub(crate) fn complete_invocation_for_thread_with_cause(
        &self,
        thread_id: &str,
        causation_id: Option<String>,
    ) -> AgentOsResult<Option<AgentInvocation>> {
        self.close_invocation_for_thread_with_cause(
            thread_id,
            AgentInvocationStatus::Completed,
            causation_id,
        )
    }

    pub(crate) fn close_invocation_for_thread_with_cause(
        &self,
        thread_id: &str,
        status: AgentInvocationStatus,
        causation_id: Option<String>,
    ) -> AgentOsResult<Option<AgentInvocation>> {
        let Some(invocation) = self
            .read_state()?
            .agent_invocations
            .values()
            .find(|invocation| {
                invocation.callee_thread_id == thread_id
                    && invocation.status == AgentInvocationStatus::Active
            })
            .cloned()
        else {
            return Ok(None);
        };
        let mut invocation = invocation;
        invocation.status = status;
        invocation.updated_at = now_rfc3339();
        self.emit(
            "AgentInvocationRecorded",
            "agent_invocation",
            &invocation.invocation_id,
            Some(invocation.callee_agent_id.clone()),
            Some(invocation.task_id.clone()),
            causation_id,
            Some(invocation.goal_id.clone()),
            &invocation,
        )?;
        Ok(Some(invocation))
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
        if next == ThreadStatus::Ready && acb.active_turn.turn_id.is_some() {
            acb.active_turn.status = Some(TurnStatus::Completed);
            acb.active_turn.active_step_id = None;
        }
        if next == ThreadStatus::Running && acb.active_turn.turn_id.is_some() {
            acb.active_turn.status = Some(TurnStatus::InProgress);
        }
        if next == ThreadStatus::Blocked && acb.active_turn.turn_id.is_some() {
            acb.active_turn.status = Some(TurnStatus::Blocked);
            acb.active_turn.active_step_id = None;
        }
        if next == ThreadStatus::Interrupted && acb.active_turn.turn_id.is_some() {
            acb.active_turn.status = Some(TurnStatus::Interrupted);
            acb.active_turn.active_step_id = None;
        }
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
        // Mark the recovery cursor dirty on any side-effecting transition so
        // a checkpoint after this point is meaningful, and record cooperative
        // readiness in the scheduler's ready queue.
        acb.recovery.dirty = true;
        if next == ThreadStatus::Ready {
            // Recorded before the event so the projection reflects the queue
            // state consistent with the emitted ACB. The queue itself is not
            // part of the event payload (it is derived scheduler state), so
            // replay stays deterministic.
            self.enqueue_ready(&acb.thread_id)?;
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
        AgentInvocationRelationship::ProducerAssignment
    }
}
