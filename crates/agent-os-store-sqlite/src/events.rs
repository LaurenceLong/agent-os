use crate::error::sqlite_error;
use crate::store::SqliteStore;
use agent_os_store::EventStore;
use agent_os_sys::{AgentOsResult, EventEnvelope};
use rusqlite::params;

impl EventStore for SqliteStore {
    fn append(&self, event: EventEnvelope) -> AgentOsResult<()> {
        let event_json = serde_json::to_string(&event)?;
        let conn = self.lock()?;
        conn.execute(
            "
            INSERT INTO events(
                event_id,
                event_type,
                abi_version,
                aggregate_type,
                aggregate_id,
                agent_id,
                task_id,
                causation_id,
                correlation_id,
                event_json,
                created_at
            )
            VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ",
            params![
                event.event_id,
                event.event_type,
                event.abi_version,
                event.aggregate_type,
                event.aggregate_id,
                event.agent_id,
                event.task_id,
                event.causation_id,
                event.correlation_id,
                event_json,
                event.created_at
            ],
        )
        .map_err(sqlite_error)?;
        Ok(())
    }

    fn all_events(&self) -> AgentOsResult<Vec<EventEnvelope>> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare("SELECT event_json FROM events ORDER BY ordinal ASC")
            .map_err(sqlite_error)?;
        read_event_rows(&mut stmt, [])
    }

    fn events_by_aggregate(&self, aggregate_id: &str) -> AgentOsResult<Vec<EventEnvelope>> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare("SELECT event_json FROM events WHERE aggregate_id = ?1 ORDER BY ordinal ASC")
            .map_err(sqlite_error)?;
        read_event_rows(&mut stmt, params![aggregate_id])
    }
}

fn read_event_rows<P>(
    stmt: &mut rusqlite::Statement<'_>,
    params: P,
) -> AgentOsResult<Vec<EventEnvelope>>
where
    P: rusqlite::Params,
{
    let rows = stmt
        .query_map(params, |row| row.get::<_, String>(0))
        .map_err(sqlite_error)?;
    let mut events = Vec::new();
    for row in rows {
        let json = row.map_err(sqlite_error)?;
        events.push(serde_json::from_str(&json)?);
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn appends_and_reads_events_in_order() {
        let store = SqliteStore::in_memory().unwrap();
        let event_a = EventEnvelope::new(
            "GoalRegistered",
            "goal",
            "goal_1",
            None,
            None,
            None,
            Some("goal_1".to_string()),
            json!({"n": 1}),
        );
        let event_b = EventEnvelope::new(
            "TaskSpawned",
            "task",
            "task_1",
            None,
            Some("task_1".to_string()),
            Some(event_a.event_id.clone()),
            Some("goal_1".to_string()),
            json!({"n": 2}),
        );
        store.append(event_a.clone()).unwrap();
        store.append(event_b.clone()).unwrap();

        let all = store.all_events().unwrap();
        assert_eq!(all[0].event_id, event_a.event_id);
        assert_eq!(all[1].event_id, event_b.event_id);
        let task_events = store.events_by_aggregate("task_1").unwrap();
        assert_eq!(task_events.len(), 1);
        assert_eq!(task_events[0].event_id, event_b.event_id);
    }
}
