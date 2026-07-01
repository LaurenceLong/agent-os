use crate::*;
use agent_os_sys::*;
use std::path::Path;

impl Kernel {
    pub fn create_environment(
        &self,
        backend_type: BackendType,
        template_name: impl Into<String>,
        sandbox_profile_id: impl Into<String>,
        reuse_policy: ReusePolicy,
    ) -> AgentOsResult<ExecutionEnvironment> {
        let template_name = template_name.into();
        if matches!(backend_type, BackendType::IsolatedWorktree) {
            std::fs::create_dir_all(Path::new(&template_name)).map_err(|error| {
                AgentOsError::Validation(format!("create isolated workspace root: {error}"))
            })?;
        }
        let now = now_rfc3339();
        let env = ExecutionEnvironment {
            environment_id: new_id("env_"),
            status: EnvironmentStatus::Ready,
            backend_type,
            template_name,
            sandbox_profile_id: sandbox_profile_id.into(),
            host_id: None,
            workspace_mounts: Vec::new(),
            artifact_mounts: Vec::new(),
            toolchain_profile_id: None,
            network_policy_id: None,
            secret_projection_id: None,
            reuse_policy,
            created_at: now.clone(),
            updated_at: now,
            terminated_at: None,
        };
        self.emit(
            "EnvironmentProvisioned",
            "environment",
            &env.environment_id,
            None,
            None,
            None,
            None,
            &env,
        )?;
        Ok(env)
    }

    pub fn attach_environment(
        &self,
        environment_id: &str,
        agent_id: &str,
        thread_id: &str,
        task_id: &str,
        attach_mode: AttachMode,
    ) -> AgentOsResult<EnvironmentLease> {
        let state = self.read_state()?;
        let env = state
            .environments
            .get(environment_id)
            .ok_or_else(|| AgentOsError::NotFound(format!("environment {environment_id}")))?;
        if env.status != EnvironmentStatus::Ready && env.status != EnvironmentStatus::Attached {
            return Err(AgentOsError::Validation(
                "environment is not attachable".to_string(),
            ));
        }
        let conflicts = state.environment_leases.values().any(|lease| {
            lease.environment_id == environment_id
                && lease.status == EnvironmentLeaseStatus::Active
                && (lease.attach_mode == AttachMode::Exclusive
                    || attach_mode == AttachMode::Exclusive)
        });
        drop(state);
        if conflicts {
            return Err(AgentOsError::ResourceConflict(
                "exclusive environment lease conflict".to_string(),
            ));
        }

        let lease = EnvironmentLease {
            environment_lease_id: new_id("envl_"),
            environment_id: environment_id.to_string(),
            agent_id: agent_id.to_string(),
            thread_id: thread_id.to_string(),
            task_id: task_id.to_string(),
            attach_mode,
            status: EnvironmentLeaseStatus::Active,
            started_at: now_rfc3339(),
            expires_at: None,
            released_at: None,
        };
        self.emit(
            "EnvironmentLeaseGranted",
            "environment_lease",
            &lease.environment_lease_id,
            Some(agent_id.to_string()),
            Some(task_id.to_string()),
            None,
            None,
            &lease,
        )?;
        Ok(lease)
    }

    pub fn release_environment_lease(
        &self,
        environment_lease_id: &str,
    ) -> AgentOsResult<EnvironmentLease> {
        let current = self
            .read_state()?
            .environment_leases
            .get(environment_lease_id)
            .cloned()
            .ok_or_else(|| {
                AgentOsError::NotFound(format!("environment lease {environment_lease_id}"))
            })?;
        if current.status != EnvironmentLeaseStatus::Active {
            return Err(AgentOsError::InvalidTransition(
                "only active environment leases can be released".to_string(),
            ));
        }
        let mut lease = current;
        lease.status = EnvironmentLeaseStatus::Released;
        lease.released_at = Some(now_rfc3339());
        self.emit(
            "EnvironmentLeaseReleased",
            "environment_lease",
            &lease.environment_lease_id,
            Some(lease.agent_id.clone()),
            Some(lease.task_id.clone()),
            None,
            None,
            &lease,
        )?;
        Ok(lease)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn request_resource_lease(
        &self,
        resource_type: ResourceType,
        resource_id: impl Into<String>,
        owner_agent_id: impl Into<String>,
        thread_id: impl Into<String>,
        goal_id: impl Into<String>,
        task_id: impl Into<String>,
        mode: LeaseMode,
        reason: Option<String>,
    ) -> AgentOsResult<ResourceLease> {
        let resource_id = resource_id.into();
        let owner_agent_id = owner_agent_id.into();
        let thread_id = thread_id.into();
        let goal_id = goal_id.into();
        let task_id = task_id.into();
        let conflicting = self.read_state()?.resource_leases.values().any(|lease| {
            lease.resource_type == resource_type
                && lease.resource_id == resource_id
                && lease.status == ResourceLeaseStatus::Granted
                && (lease.mode == LeaseMode::Exclusive || mode == LeaseMode::Exclusive)
        });
        let status = if conflicting {
            ResourceLeaseStatus::Denied
        } else {
            ResourceLeaseStatus::Granted
        };
        let lease = ResourceLease {
            resource_lease_id: new_id("rlease_"),
            resource_type,
            resource_id,
            owner_agent_id,
            thread_id,
            goal_id,
            task_id,
            mode,
            status,
            reason,
            lease_expires_at: None,
            created_at: now_rfc3339(),
            released_at: None,
        };
        let event_type = if conflicting {
            "ResourceLeaseDenied"
        } else {
            "ResourceLeaseGranted"
        };
        self.emit(
            event_type,
            "resource_lease",
            &lease.resource_lease_id,
            Some(lease.owner_agent_id.clone()),
            Some(lease.task_id.clone()),
            None,
            Some(lease.goal_id.clone()),
            &lease,
        )?;
        if conflicting {
            return Err(AgentOsError::ResourceConflict(
                "resource lease conflict resolved as denial".to_string(),
            ));
        }
        Ok(lease)
    }

    pub fn release_resource_lease(&self, resource_lease_id: &str) -> AgentOsResult<ResourceLease> {
        let current = self
            .read_state()?
            .resource_leases
            .get(resource_lease_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("resource lease {resource_lease_id}")))?;
        if current.status != ResourceLeaseStatus::Granted {
            return Err(AgentOsError::InvalidTransition(
                "only granted resource leases can be released".to_string(),
            ));
        }
        let mut lease = current;
        lease.status = ResourceLeaseStatus::Released;
        lease.released_at = Some(now_rfc3339());
        self.emit(
            "ResourceLeaseReleased",
            "resource_lease",
            &lease.resource_lease_id,
            Some(lease.owner_agent_id.clone()),
            Some(lease.task_id.clone()),
            None,
            Some(lease.goal_id.clone()),
            &lease,
        )?;
        Ok(lease)
    }

    pub fn open_resource_session(
        &self,
        input: OpenResourceSessionInput,
    ) -> AgentOsResult<ResourceSession> {
        if let Some(thread_id) = &input.client_thread_id {
            let state = self.read_state()?;
            if !state.threads.contains_key(thread_id) {
                return Err(AgentOsError::NotFound(format!("thread {thread_id}")));
            }
        }
        if let Some(agent_id) = &input.owner_agent_id {
            let state = self.read_state()?;
            if !state
                .threads
                .values()
                .any(|thread| thread.agent_id == *agent_id)
            {
                return Err(AgentOsError::NotFound(format!("agent {agent_id}")));
            }
        }
        let now = now_rfc3339();
        let session = ResourceSession {
            session_id: new_id("rsess_"),
            resource_type: input.resource_type,
            client_thread_id: input.client_thread_id,
            owner_agent_id: input.owner_agent_id,
            status: ResourceSessionStatus::Active,
            lease_expires_at: input.lease_expires_at,
            created_at: now.clone(),
            updated_at: now,
            closed_at: None,
            payload: input.payload,
        };
        self.emit(
            "ResourceSessionOpened",
            "resource_session",
            &session.session_id,
            session.owner_agent_id.clone(),
            None,
            None,
            session.client_thread_id.clone(),
            &session,
        )?;
        Ok(session)
    }

    pub fn close_resource_session(&self, session_id: &str) -> AgentOsResult<ResourceSession> {
        let current = self
            .read_state()?
            .resource_sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("resource session {session_id}")))?;
        if current.status != ResourceSessionStatus::Active {
            return Err(AgentOsError::InvalidTransition(
                "only active resource sessions can be closed".to_string(),
            ));
        }
        let now = now_rfc3339();
        let mut session = current;
        session.status = ResourceSessionStatus::Closed;
        session.updated_at = now.clone();
        session.closed_at = Some(now);
        self.emit(
            "ResourceSessionClosed",
            "resource_session",
            &session.session_id,
            session.owner_agent_id.clone(),
            None,
            None,
            session.client_thread_id.clone(),
            &session,
        )?;
        Ok(session)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_budget_ledger(
        &self,
        scope_type: BudgetScope,
        scope_id: impl Into<String>,
        token_limit: Option<u64>,
        tool_call_limit: Option<u64>,
        wall_time_limit_ms: Option<u64>,
        cost_limit: Option<f64>,
        human_interrupt_limit: Option<u64>,
        model_request_limit: Option<u64>,
    ) -> AgentOsResult<BudgetLedger> {
        let now = now_rfc3339();
        let ledger = BudgetLedger {
            budget_ledger_id: new_id("bgt_"),
            scope_type,
            scope_id: scope_id.into(),
            status: BudgetStatus::Active,
            token_limit,
            tool_call_limit,
            wall_time_limit_ms,
            cost_limit,
            human_interrupt_limit,
            model_request_limit,
            tokens_used: 0,
            tool_calls_used: 0,
            wall_time_used_ms: 0,
            cost_used: 0.0,
            human_interrupts_used: 0,
            model_requests_used: 0,
            reserved: None,
            reset_policy: None,
            created_at: now.clone(),
            updated_at: now,
        };
        self.emit(
            "BudgetLedgerCreated",
            "budget_ledger",
            &ledger.budget_ledger_id,
            None,
            None,
            None,
            Some(ledger.scope_id.clone()),
            &ledger,
        )?;
        Ok(ledger)
    }

    pub fn debit_budget(
        &self,
        budget_ledger_id: &str,
        debit: BudgetDebit,
    ) -> AgentOsResult<BudgetLedger> {
        let current = self
            .read_state()?
            .budget_ledgers
            .get(budget_ledger_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("budget ledger {budget_ledger_id}")))?;
        if current.status != BudgetStatus::Active {
            return Err(AgentOsError::BudgetExhausted(
                "budget ledger is not active".to_string(),
            ));
        }
        let mut next = current;
        next.tokens_used += debit.tokens;
        next.tool_calls_used += debit.tool_calls;
        next.wall_time_used_ms += debit.wall_time_ms;
        next.cost_used += debit.cost;
        next.human_interrupts_used += debit.human_interrupts;
        next.model_requests_used += debit.model_requests;
        next.updated_at = now_rfc3339();

        let exhausted = next
            .token_limit
            .is_some_and(|limit| next.tokens_used > limit)
            || next
                .tool_call_limit
                .is_some_and(|limit| next.tool_calls_used > limit)
            || next
                .wall_time_limit_ms
                .is_some_and(|limit| next.wall_time_used_ms > limit)
            || next.cost_limit.is_some_and(|limit| next.cost_used > limit)
            || next
                .human_interrupt_limit
                .is_some_and(|limit| next.human_interrupts_used > limit)
            || next
                .model_request_limit
                .is_some_and(|limit| next.model_requests_used > limit);
        if exhausted {
            next.status = BudgetStatus::Exhausted;
        }
        self.emit(
            if exhausted {
                "BudgetExhausted"
            } else {
                "BudgetDebited"
            },
            "budget_ledger",
            &next.budget_ledger_id,
            None,
            None,
            None,
            Some(next.scope_id.clone()),
            &next,
        )?;
        if exhausted {
            return Err(AgentOsError::BudgetExhausted(
                "budget debit exhausted ledger".to_string(),
            ));
        }
        Ok(next)
    }
}
