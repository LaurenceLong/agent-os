mod approval;
mod scope;

use approval::{validate_capability_approval, validate_syscall_approval};
use scope::{requested_resource_scopes, scope_list_allows};

use crate::*;
use agent_os_sys::*;

impl Kernel {
    pub fn grant_capability(
        &self,
        agent_id: &str,
        task_id: &str,
        syscalls: Vec<String>,
        resource_scopes: Vec<String>,
        risk_ceiling: u8,
        approval_id: Option<String>,
    ) -> AgentOsResult<CapabilityToken> {
        let acb = self
            .thread_by_agent(agent_id)?
            .ok_or_else(|| AgentOsError::NotFound(format!("agent {agent_id}")))?;
        if acb.task.task_id != task_id {
            return Err(AgentOsError::Validation(
                "capability task does not match agent task".to_string(),
            ));
        }
        let permission = self.effective_permission_set(&acb)?;
        if risk_ceiling > permission.max_risk_level {
            return Err(AgentOsError::PermissionDenied(
                "capability exceeds effective permission risk ceiling".to_string(),
            ));
        }
        for syscall in &syscalls {
            if !wildcard_allows(&permission.allowed_syscalls, syscall) {
                return Err(AgentOsError::PermissionDenied(format!(
                    "effective permissions do not allow syscall {syscall}"
                )));
            }
        }
        for scope in &resource_scopes {
            if !scope_list_allows(&permission.resource_scopes, scope) {
                return Err(AgentOsError::PermissionDenied(format!(
                    "effective permissions do not allow resource scope {scope}"
                )));
            }
        }
        if risk_ceiling > permission.approval_required_above {
            let approval_id = approval_id.as_ref().ok_or_else(|| {
                AgentOsError::ApprovalRequired(
                    "high-risk capability requires an approved bounded approval scope".to_string(),
                )
            })?;
            let state = self.read_state()?;
            let approval = state
                .approvals
                .get(approval_id)
                .ok_or_else(|| AgentOsError::NotFound(format!("approval {approval_id}")))?;
            validate_capability_approval(
                approval,
                &syscalls,
                risk_ceiling,
                &acb.task.goal_id,
                task_id,
            )?;
        }

        let now = now_rfc3339();
        let token = CapabilityToken {
            capability_id: new_id("cap_"),
            agent_id: agent_id.to_string(),
            task_id: task_id.to_string(),
            role: acb.role.clone(),
            syscalls,
            resource_scopes,
            risk_ceiling,
            expires_at: None,
            approval_id,
            created_at: now,
            revoked_at: None,
        };
        self.emit(
            "CapabilityGranted",
            "capability",
            &token.capability_id,
            Some(agent_id.to_string()),
            Some(task_id.to_string()),
            None,
            Some(acb.task.goal_id.clone()),
            &token,
        )?;
        Ok(token)
    }

    pub(crate) fn authorize(&self, syscall: &SyscallEnvelope) -> AgentOsResult<()> {
        let capability_id = syscall.capability_token.as_ref().ok_or_else(|| {
            AgentOsError::PermissionDenied(
                "syscall lacks capability context and is rejected by ABI".to_string(),
            )
        })?;
        let state = self.read_state()?;
        let cap = state.capabilities.get(capability_id).ok_or_else(|| {
            AgentOsError::PermissionDenied("unknown capability token".to_string())
        })?;
        if cap.revoked_at.is_some() {
            return Err(AgentOsError::PermissionDenied(
                "capability token is revoked".to_string(),
            ));
        }
        if cap.agent_id != syscall.agent_id || cap.task_id != syscall.task_id {
            return Err(AgentOsError::PermissionDenied(
                "capability token identity or task scope mismatch".to_string(),
            ));
        }
        if !wildcard_allows(&cap.syscalls, &syscall.syscall_type) {
            return Err(AgentOsError::PermissionDenied(format!(
                "capability does not allow syscall {}",
                syscall.syscall_type
            )));
        }
        if syscall.risk_level > cap.risk_ceiling {
            return Err(AgentOsError::PermissionDenied(
                "syscall risk exceeds capability ceiling".to_string(),
            ));
        }
        let acb = state
            .threads
            .values()
            .find(|thread| thread.agent_id == syscall.agent_id)
            .ok_or_else(|| AgentOsError::NotFound(format!("agent {}", syscall.agent_id)))?;
        let permission = crate::permissions::effective_permission_set_for_thread(&state, acb);
        if !wildcard_allows(&permission.allowed_syscalls, &syscall.syscall_type) {
            return Err(AgentOsError::PermissionDenied(format!(
                "effective permissions do not allow syscall {}",
                syscall.syscall_type
            )));
        }
        if syscall.risk_level > permission.max_risk_level {
            return Err(AgentOsError::PermissionDenied(
                "syscall risk exceeds effective permission ceiling".to_string(),
            ));
        }
        for scope in requested_resource_scopes(&syscall.resource_scope)? {
            if !scope_list_allows(&cap.resource_scopes, &scope) {
                return Err(AgentOsError::PermissionDenied(format!(
                    "capability does not allow resource scope {scope}"
                )));
            }
            if !scope_list_allows(&permission.resource_scopes, &scope) {
                return Err(AgentOsError::PermissionDenied(format!(
                    "effective permissions do not allow resource scope {scope}"
                )));
            }
        }
        if syscall.risk_level > permission.approval_required_above {
            let approval_id = cap.approval_id.as_ref().ok_or_else(|| {
                AgentOsError::ApprovalRequired(
                    "syscall risk requires approved bounded approval scope".to_string(),
                )
            })?;
            let approval = state
                .approvals
                .get(approval_id)
                .ok_or_else(|| AgentOsError::NotFound(format!("approval {approval_id}")))?;
            validate_syscall_approval(approval, syscall, &acb.task.goal_id)?;
        }
        drop(state);
        self.audit(
            AuditActorType::Agent,
            &syscall.agent_id,
            &syscall.syscall_type,
            "syscall",
            &syscall.syscall_id,
            None,
            AuditResult::Allow,
        )?;
        Ok(())
    }

    pub(crate) fn active_role(&self, id: &str) -> AgentOsResult<RoleProfile> {
        self.read_state()?
            .role_profiles
            .get(id)
            .filter(|profile| profile.status == ProfileStatus::Active)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("active role profile {id}")))
    }

    pub(crate) fn active_permission(&self, id: &str) -> AgentOsResult<PermissionProfile> {
        self.read_state()?
            .permission_profiles
            .get(id)
            .filter(|profile| profile.status == ProfileStatus::Active)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("active permission profile {id}")))
    }

    pub(crate) fn active_sandbox(&self, id: &str) -> AgentOsResult<SandboxProfile> {
        self.read_state()?
            .sandbox_profiles
            .get(id)
            .filter(|profile| profile.status == ProfileStatus::Active)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("active sandbox profile {id}")))
    }

    pub(crate) fn thread_by_agent(
        &self,
        agent_id: &str,
    ) -> AgentOsResult<Option<AgentControlBlock>> {
        Ok(self
            .read_state()?
            .threads
            .values()
            .find(|thread| thread.agent_id == agent_id)
            .cloned())
    }
}
