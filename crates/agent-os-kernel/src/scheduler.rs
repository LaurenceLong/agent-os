//! Scheduler admission control.
//!
//! The kernel's cooperative scheduler decides whether an Agent Thread may
//! start a new turn. Budget ledgers are an admission-control mechanism (see
//! `docs/10-kernel-design/scheduler-and-resource-arbitration.md:143-153`), not
//! just reporting counters. This module evaluates the admission signals that
//! are durable today: budget exhaustion across goal/task/agent scopes and task
//! dependency readiness. Provider-slot and environment-capacity admission are
//! layered on top via the provider/resource modules.

use crate::*;
use agent_os_sys::*;

/// Reason a turn was denied admission. Mirrors the machine-readable rejection
/// reasons documented in
/// `docs/10-kernel-design/agent-thread-core-module.md:488-501`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionRejection {
    /// A budget ledger bound to the thread's goal, task, or agent scope is
    /// exhausted. Corresponds to the documented `OutOfBudget` reason.
    OutOfBudget {
        scope_id: String,
        budget_ledger_id: String,
    },
    /// The task is blocked on an unfinished dependency. Corresponds to the
    /// documented `DependencyBlocked` reason.
    DependencyBlocked {
        task_id: String,
        blocked_on: Vec<String>,
    },
    ProviderSlotUnavailable {
        provider_id: String,
    },
}

/// Outcome of evaluating turn admission.
#[derive(Debug, Clone)]
pub enum AdmissionDecision {
    Allowed,
    Rejected(AdmissionRejection),
}

impl AdmissionDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, AdmissionDecision::Allowed)
    }
}

impl Kernel {
    /// Evaluate whether `thread_id` may start a new turn under the current
    /// budget ledgers and task dependency state.
    ///
    /// This is purely advisory against existing durable state: it never
    /// mutates state and never emits events. `start_turn` calls it and
    /// translates a rejection into the appropriate `AgentOsError`. Callers
    /// that want to preview admission (for example, a scheduler ordering the
    /// ready queue) may call it directly.
    pub fn evaluate_turn_admission(&self, thread_id: &str) -> AgentOsResult<AdmissionDecision> {
        let state = self.read_state()?;
        let Some(acb) = state.threads.get(thread_id) else {
            return Err(AgentOsError::NotFound(format!("thread {thread_id}")));
        };
        Ok(evaluate_admission(&state, acb))
    }

    /// Cooperative ready-queue management.
    ///
    /// Records that a thread has reached the `Ready` status and is eligible
    /// for scheduling. The queue is ordered by descending task priority with a
    /// deterministic thread-id tie break. Duplicates are collapsed so a thread
    /// that cycles Ready -> Running -> Ready does not occupy two slots.
    pub(crate) fn enqueue_ready(&self, thread_id: &str) -> AgentOsResult<()> {
        let mut state = self.write_state()?;
        if !state.ready_queue.iter().any(|id| id == thread_id) {
            state.ready_queue.push_back(thread_id.to_string());
        }
        let mut queued: Vec<String> = state.ready_queue.drain(..).collect();
        queued.sort_by(|left, right| {
            let left_priority = state
                .threads
                .get(left)
                .and_then(|thread| state.tasks.get(&thread.task.task_id))
                .map(|task| task.priority)
                .unwrap_or_default();
            let right_priority = state
                .threads
                .get(right)
                .and_then(|thread| state.tasks.get(&thread.task.task_id))
                .map(|task| task.priority)
                .unwrap_or_default();
            right_priority
                .cmp(&left_priority)
                .then_with(|| left.cmp(right))
        });
        state.ready_queue.extend(queued);
        Ok(())
    }

    /// Drain and return the next ready thread id, if any. Used by a
    /// cooperative scheduler driver; the synchronous runtime does not call
    /// this today.
    pub fn drain_next_ready(&self) -> AgentOsResult<Option<String>> {
        let mut state = self.write_state()?;
        Ok(state.ready_queue.pop_front())
    }

    /// Snapshot the current ready queue without draining.
    pub fn ready_queue_snapshot(&self) -> AgentOsResult<Vec<String>> {
        Ok(self.read_state()?.ready_queue.iter().cloned().collect())
    }
}

fn evaluate_admission(state: &KernelState, acb: &AgentControlBlock) -> AdmissionDecision {
    if let Some(rejection) = budget_rejection(state, acb) {
        return AdmissionDecision::Rejected(rejection);
    }
    if let Some(rejection) = dependency_rejection(state, acb) {
        return AdmissionDecision::Rejected(rejection);
    }
    if let Some(rejection) = provider_slot_rejection(state, acb) {
        return AdmissionDecision::Rejected(rejection);
    }
    AdmissionDecision::Allowed
}

fn budget_rejection(state: &KernelState, acb: &AgentControlBlock) -> Option<AdmissionRejection> {
    // A thread is bound to one goal and one task; honor ledgers scoped to
    // either, plus any agent-scoped ledger for this thread's agent.
    for ledger in state.budget_ledgers.values() {
        let matches_scope = match ledger.scope_type {
            BudgetScope::Goal => ledger.scope_id == acb.task.goal_id,
            BudgetScope::Task => ledger.scope_id == acb.task.task_id,
            BudgetScope::Agent => ledger.scope_id == acb.agent_id,
            BudgetScope::ProviderProfile => {
                ledger.scope_id == acb.config_snapshot.provider_profile_id
            }
            BudgetScope::HumanAttention => {
                ledger.scope_id == acb.task.goal_id
                    || ledger.scope_id == acb.task.task_id
                    || ledger.scope_id == acb.agent_id
            }
        };
        if matches_scope && ledger.status == BudgetStatus::Exhausted {
            return Some(AdmissionRejection::OutOfBudget {
                scope_id: ledger.scope_id.clone(),
                budget_ledger_id: ledger.budget_ledger_id.clone(),
            });
        }
    }
    None
}

fn provider_slot_rejection(
    state: &KernelState,
    acb: &AgentControlBlock,
) -> Option<AdmissionRejection> {
    let profile = state
        .provider_profiles
        .get(&acb.config_snapshot.provider_profile_id)?;
    let provider_id = profile.default_provider_id.as_ref()?;
    let slot_busy = state.resource_leases.values().any(|lease| {
        lease.resource_type == ResourceType::ProviderSlot
            && lease.resource_id == *provider_id
            && lease.status == ResourceLeaseStatus::Granted
            && lease.thread_id != acb.thread_id
    });
    slot_busy.then(|| AdmissionRejection::ProviderSlotUnavailable {
        provider_id: provider_id.clone(),
    })
}

fn dependency_rejection(
    state: &KernelState,
    acb: &AgentControlBlock,
) -> Option<AdmissionRejection> {
    let task = state.tasks.get(&acb.task.task_id)?;
    let unfinished: Vec<String> = task
        .depends_on
        .iter()
        .filter(|dep| {
            state
                .tasks
                .get(*dep)
                .is_some_and(|dep_task| dep_task.status != TaskStatus::Completed)
        })
        .cloned()
        .collect();
    if unfinished.is_empty() {
        None
    } else {
        Some(AdmissionRejection::DependencyBlocked {
            task_id: task.task_id.clone(),
            blocked_on: unfinished,
        })
    }
}
