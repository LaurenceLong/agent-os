use crate::{EventStore, IdempotencyStore, ProjectionState, ProjectionStore};
use agent_os_sys::{
    AgentOsError, AgentOsResult, ApprovalQueueProjection, ArtifactIndexProjection,
    AutomationRunProjection, AutomationScheduleProjection, ClientThread, EventEnvelope,
    EvidenceIndexProjection, ProjectionCheckpoint, ResourceSessionProjection, StatsQuery,
    StatsSnapshot, SyscallResult, TimelineItem, TurnRecord,
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Default)]
struct InMemoryStoreInner {
    events: Vec<EventEnvelope>,
    idempotency: HashMap<String, SyscallResult>,
    projections: ProjectionState,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryStore {
    inner: Arc<RwLock<InMemoryStoreInner>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> AgentOsResult<usize> {
        Ok(self.read()?.events.len())
    }

    pub fn is_empty(&self) -> AgentOsResult<bool> {
        Ok(self.read()?.events.is_empty())
    }

    fn read(&self) -> AgentOsResult<std::sync::RwLockReadGuard<'_, InMemoryStoreInner>> {
        self.inner
            .read()
            .map_err(|_| AgentOsError::Validation("store read lock poisoned".to_string()))
    }

    fn write(&self) -> AgentOsResult<std::sync::RwLockWriteGuard<'_, InMemoryStoreInner>> {
        self.inner
            .write()
            .map_err(|_| AgentOsError::Validation("store write lock poisoned".to_string()))
    }
}

impl EventStore for InMemoryStore {
    fn append(&self, event: EventEnvelope) -> AgentOsResult<()> {
        self.write()?.events.push(event);
        Ok(())
    }

    fn all_events(&self) -> AgentOsResult<Vec<EventEnvelope>> {
        Ok(self.read()?.events.clone())
    }

    fn events_by_aggregate(&self, aggregate_id: &str) -> AgentOsResult<Vec<EventEnvelope>> {
        Ok(self
            .read()?
            .events
            .iter()
            .filter(|event| event.aggregate_id == aggregate_id)
            .cloned()
            .collect())
    }

    fn event_ordinal(&self, event_id: &str) -> AgentOsResult<u64> {
        self.read()?
            .events
            .iter()
            .position(|event| event.event_id == event_id)
            .map(|index| index as u64 + 1)
            .ok_or_else(|| AgentOsError::NotFound(format!("event {event_id}")))
    }
}

impl IdempotencyStore for InMemoryStore {
    fn get_syscall_result(&self, idempotency_key: &str) -> AgentOsResult<Option<SyscallResult>> {
        Ok(self.read()?.idempotency.get(idempotency_key).cloned())
    }

    fn put_syscall_result(
        &self,
        idempotency_key: String,
        result: SyscallResult,
    ) -> AgentOsResult<()> {
        let previous = self.write()?.idempotency.insert(idempotency_key, result);
        if previous.is_some() {
            return Err(AgentOsError::IdempotencyConflict(
                "idempotency key was already recorded".to_string(),
            ));
        }
        Ok(())
    }
}

// In-memory driver supports the projection-family store traits via the
// event-derived blanket implementations.
impl ProjectionStore for InMemoryStore {
    fn events_by_aggregate_type(&self, aggregate_type: &str) -> AgentOsResult<Vec<EventEnvelope>> {
        Ok(self
            .read()?
            .events
            .iter()
            .filter(|event| event.aggregate_type == aggregate_type)
            .cloned()
            .collect())
    }

    fn clear_projections(&self) -> AgentOsResult<()> {
        self.write()?.projections = ProjectionState::default();
        Ok(())
    }

    fn append_projected(&self, event: EventEnvelope) -> AgentOsResult<u64> {
        let mut inner = self.write()?;
        inner.events.push(event.clone());
        let ordinal = inner.events.len() as u64;
        inner.projections.apply_event(ordinal, &event)?;
        Ok(ordinal)
    }

    fn project_event(&self, ordinal: u64, event: &EventEnvelope) -> AgentOsResult<()> {
        self.write()?.projections.apply_event(ordinal, event)
    }

    fn rebuild_projections(&self) -> AgentOsResult<()> {
        let events = self.read()?.events.clone();
        self.write()?.projections = ProjectionState::rebuild(&events)?;
        Ok(())
    }

    fn thread_summaries(&self) -> AgentOsResult<Vec<ClientThread>> {
        Ok(self.read()?.projections.threads.values().cloned().collect())
    }

    fn turn_summaries(&self) -> AgentOsResult<Vec<TurnRecord>> {
        Ok(self.read()?.projections.turns.values().cloned().collect())
    }

    fn timeline_items(&self, client_thread_id: Option<&str>) -> AgentOsResult<Vec<TimelineItem>> {
        Ok(self
            .read()?
            .projections
            .timeline_items
            .iter()
            .filter(|item| {
                client_thread_id
                    .map(|thread_id| item.client_thread_id.as_deref() == Some(thread_id))
                    .unwrap_or(true)
            })
            .cloned()
            .collect())
    }

    fn stats_snapshot(&self, _query: StatsQuery) -> AgentOsResult<StatsSnapshot> {
        Ok(self.read()?.projections.stats.clone())
    }

    fn approval_queue(&self) -> AgentOsResult<Vec<ApprovalQueueProjection>> {
        Ok(self
            .read()?
            .projections
            .approvals
            .values()
            .cloned()
            .collect())
    }

    fn resource_sessions(&self) -> AgentOsResult<Vec<ResourceSessionProjection>> {
        Ok(self
            .read()?
            .projections
            .resources
            .values()
            .cloned()
            .collect())
    }

    fn automation_schedules(&self) -> AgentOsResult<Vec<AutomationScheduleProjection>> {
        Ok(self
            .read()?
            .projections
            .automation_schedules
            .values()
            .cloned()
            .collect())
    }

    fn automation_runs(&self) -> AgentOsResult<Vec<AutomationRunProjection>> {
        Ok(self
            .read()?
            .projections
            .automation_runs
            .values()
            .cloned()
            .collect())
    }

    fn artifact_index(&self) -> AgentOsResult<Vec<ArtifactIndexProjection>> {
        Ok(self
            .read()?
            .projections
            .artifacts
            .values()
            .cloned()
            .collect())
    }

    fn evidence_index(&self) -> AgentOsResult<Vec<EvidenceIndexProjection>> {
        Ok(self
            .read()?
            .projections
            .evidence
            .values()
            .cloned()
            .collect())
    }

    fn projection_checkpoint(
        &self,
        projection_name: &str,
    ) -> AgentOsResult<Option<ProjectionCheckpoint>> {
        Ok(self
            .read()?
            .projections
            .checkpoints
            .get(projection_name)
            .cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn appends_events_without_mutating_old_entries() {
        let store = InMemoryStore::new();
        let event = EventEnvelope::new(
            "GoalRegistered",
            "goal",
            "goal_1",
            None,
            None,
            None,
            Some("goal_1".to_string()),
            json!({"title": "demo"}),
        );
        store.append(event.clone()).unwrap();

        let events = store.all_events().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, event.event_id);
    }
}
