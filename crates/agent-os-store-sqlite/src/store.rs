use crate::error::sqlite_error;
use crate::migrations::migrate;
use agent_os_sys::{AgentOsError, AgentOsResult};
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct SqliteStore {
    pub(crate) conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    pub fn open(path: impl AsRef<Path>) -> AgentOsResult<Self> {
        let conn = Connection::open(path).map_err(sqlite_error)?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn in_memory() -> AgentOsResult<Self> {
        let conn = Connection::open_in_memory().map_err(sqlite_error)?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn migration_version(&self) -> AgentOsResult<i64> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT version FROM schema_migrations WHERE name = 'agent_os_store'",
            [],
            |row| row.get(0),
        )
        .map_err(sqlite_error)
    }

    pub(crate) fn lock(&self) -> AgentOsResult<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| AgentOsError::Validation("sqlite connection lock poisoned".to_string()))
    }

    fn migrate(&self) -> AgentOsResult<()> {
        let conn = self.lock()?;
        migrate(&conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::MIGRATION_VERSION;

    #[test]
    fn migration_version_is_recorded() {
        let store = SqliteStore::in_memory().unwrap();
        assert_eq!(store.migration_version().unwrap(), MIGRATION_VERSION);
    }
}
