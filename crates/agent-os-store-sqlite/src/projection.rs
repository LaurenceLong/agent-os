use crate::error::sqlite_error;
use crate::store::SqliteStore;
use agent_os_store::ProjectionStore;
use agent_os_sys::{AgentOsResult, EventEnvelope};
use rusqlite::params;

impl ProjectionStore for SqliteStore {
    fn events_by_aggregate_type(&self, aggregate_type: &str) -> AgentOsResult<Vec<EventEnvelope>> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare("SELECT event_json FROM events WHERE aggregate_type = ?1 ORDER BY ordinal ASC")
            .map_err(sqlite_error)?;
        let rows = stmt
            .query_map(params![aggregate_type], |row| row.get::<_, String>(0))
            .map_err(sqlite_error)?;
        let mut events = Vec::new();
        for row in rows {
            let json = row.map_err(sqlite_error)?;
            events.push(serde_json::from_str(&json)?);
        }
        Ok(events)
    }
}
