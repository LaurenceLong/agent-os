use crate::{EventStore, IdempotencyStore};
use agent_os_sys::{AgentOsError, AgentOsResult, EventEnvelope, SyscallResult};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Default)]
struct InMemoryStoreInner {
    events: Vec<EventEnvelope>,
    idempotency: HashMap<String, SyscallResult>,
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
