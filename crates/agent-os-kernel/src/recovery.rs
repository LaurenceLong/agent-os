//! Recovery reconciliation.
//!
//! On restart the kernel must reconcile durable state that an Agent Thread
//! may have left mid-flight: orphan tool calls still marked `Running`, and
//! resource/environment leases whose expiry passed while the process was
//! down. It also records patch artifacts that represent workspace diffs for
//! the resumed task. The contract is documented in
//! `docs/10-kernel-design/agent-thread-core-module.md:747-770` and
//! `docs/10-kernel-design/state-storage-and-replay.md:224`.
//!
//! Reconciliation is deterministic: it records tool, lease, and thread
//! reconciliation events instead of rewriting prior event payloads.

use crate::util::rfc3339_is_past;
use crate::*;
use agent_os_sys::*;
use serde::{Deserialize, Serialize};

/// A durable record of one reconciliation pass for a thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationReport {
    pub reconciliation_id: String,
    pub thread_id: String,
    pub agent_id: String,
    pub task_id: String,
    pub orphan_tool_call_ids: Vec<String>,
    pub workspace_diff_refs: Vec<String>,
    pub reclaimed_resource_lease_ids: Vec<String>,
    pub reclaimed_environment_lease_ids: Vec<String>,
    pub created_at: String,
}

impl Kernel {
    /// Reconcile durable state for a thread after a restart.
    ///
    /// Marks orphan `Running` tool invocations for the thread's task as
    /// cancelled, reclaims expired resource and environment leases owned by
    /// the thread, and records a durable `ReconciliationReport`. The thread
    /// itself is not transitioned here; callers (the CLI resume path) own
    /// the status transition. Returns the report so callers can surface it.
    pub fn reconcile_thread_recovery(
        &self,
        thread_id: &str,
    ) -> AgentOsResult<ReconciliationReport> {
        let state = self.read_state()?;
        let acb = state
            .threads
            .get(thread_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {thread_id}")))?;
        let orphan_tool_call_ids: Vec<String> = state
            .tool_invocations
            .values()
            .filter(|invocation| {
                invocation.task_id == acb.task.task_id
                    && invocation.status == ToolCallStatus::Running
            })
            .map(|invocation| invocation.call_id.clone())
            .collect();
        let reclaimed_resource_lease_ids: Vec<String> = state
            .resource_leases
            .values()
            .filter(|lease| {
                lease.thread_id == thread_id
                    && lease.status == ResourceLeaseStatus::Granted
                    && lease_expiration_is_past(lease).unwrap_or(false)
            })
            .map(|lease| lease.resource_lease_id.clone())
            .collect();
        let reclaimed_environment_lease_ids: Vec<String> = state
            .environment_leases
            .values()
            .filter(|lease| {
                lease.thread_id == thread_id
                    && lease.status == EnvironmentLeaseStatus::Active
                    && env_lease_expiration_is_past(lease).unwrap_or(false)
            })
            .map(|lease| lease.environment_lease_id.clone())
            .collect();
        let workspace_diff_refs: Vec<String> = state
            .artifacts
            .values()
            .filter(|artifact| {
                artifact.task_id == acb.task.task_id
                    && artifact.artifact_type == ArtifactType::Patch
            })
            .map(|artifact| artifact.artifact_id.clone())
            .collect();
        drop(state);

        for call_id in &orphan_tool_call_ids {
            self.reconcile_orphan_tool_call(call_id)?;
        }
        for lease_id in &reclaimed_resource_lease_ids {
            self.reclaim_expired_resource_lease(lease_id)?;
        }
        for lease_id in &reclaimed_environment_lease_ids {
            self.reclaim_expired_environment_lease(lease_id)?;
        }

        let report = ReconciliationReport {
            reconciliation_id: new_id("rec_"),
            thread_id: thread_id.to_string(),
            agent_id: acb.agent_id.clone(),
            task_id: acb.task.task_id.clone(),
            orphan_tool_call_ids,
            workspace_diff_refs,
            reclaimed_resource_lease_ids,
            reclaimed_environment_lease_ids,
            created_at: now_rfc3339(),
        };
        self.emit(
            "ThreadReconciled",
            "reconciliation",
            &report.reconciliation_id,
            Some(acb.agent_id),
            Some(acb.task.task_id),
            None,
            None,
            &report,
        )?;
        Ok(report)
    }

    fn reconcile_orphan_tool_call(&self, call_id: &str) -> AgentOsResult<()> {
        let invocation = self
            .read_state()?
            .tool_invocations
            .get(call_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("tool call {call_id}")))?;
        let mut reconciled = invocation.clone();
        reconciled.status = ToolCallStatus::Cancelled;
        reconciled.completed_at = Some(now_rfc3339());
        self.emit(
            "ToolCallReconciled",
            "tool_call",
            &reconciled.call_id,
            Some(reconciled.agent_id.clone()),
            Some(reconciled.task_id.clone()),
            None,
            None,
            &reconciled,
        )?;
        Ok(())
    }

    fn reclaim_expired_resource_lease(&self, resource_lease_id: &str) -> AgentOsResult<()> {
        let lease = self
            .read_state()?
            .resource_leases
            .get(resource_lease_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("resource lease {resource_lease_id}")))?;
        let mut reclaimed = lease.clone();
        reclaimed.status = ResourceLeaseStatus::Expired;
        reclaimed.released_at = Some(now_rfc3339());
        self.emit(
            "ResourceLeaseReclaimed",
            "resource_lease",
            &reclaimed.resource_lease_id,
            Some(reclaimed.owner_agent_id.clone()),
            Some(reclaimed.task_id.clone()),
            None,
            Some(reclaimed.goal_id.clone()),
            &reclaimed,
        )?;
        Ok(())
    }

    fn reclaim_expired_environment_lease(&self, environment_lease_id: &str) -> AgentOsResult<()> {
        let lease = self
            .read_state()?
            .environment_leases
            .get(environment_lease_id)
            .cloned()
            .ok_or_else(|| {
                AgentOsError::NotFound(format!("environment lease {environment_lease_id}"))
            })?;
        let mut reclaimed = lease.clone();
        reclaimed.status = EnvironmentLeaseStatus::Expired;
        reclaimed.released_at = Some(now_rfc3339());
        self.emit(
            "EnvironmentLeaseReclaimed",
            "environment_lease",
            &reclaimed.environment_lease_id,
            Some(reclaimed.agent_id.clone()),
            Some(reclaimed.task_id.clone()),
            None,
            None,
            &reclaimed,
        )?;
        Ok(())
    }
}

fn lease_expiration_is_past(lease: &ResourceLease) -> AgentOsResult<bool> {
    lease
        .lease_expires_at
        .as_deref()
        .map(rfc3339_is_past)
        .transpose()
        .map(|expired| expired.unwrap_or(false))
}

fn env_lease_expiration_is_past(lease: &EnvironmentLease) -> AgentOsResult<bool> {
    lease
        .expires_at
        .as_deref()
        .map(rfc3339_is_past)
        .transpose()
        .map(|expired| expired.unwrap_or(false))
}
