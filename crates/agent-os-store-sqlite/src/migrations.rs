use crate::error::sqlite_error;
use agent_os_sys::AgentOsResult;
use rusqlite::{params, Connection};

pub(crate) const MIGRATION_VERSION: i64 = 1;

pub(crate) fn migrate(conn: &Connection) -> AgentOsResult<()> {
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS schema_migrations (
            name TEXT PRIMARY KEY NOT NULL,
            version INTEGER NOT NULL,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS events (
            ordinal INTEGER PRIMARY KEY AUTOINCREMENT,
            event_id TEXT NOT NULL UNIQUE,
            event_type TEXT NOT NULL,
            abi_version TEXT NOT NULL,
            aggregate_type TEXT NOT NULL,
            aggregate_id TEXT NOT NULL,
            agent_id TEXT,
            task_id TEXT,
            causation_id TEXT,
            correlation_id TEXT,
            event_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_events_aggregate
            ON events(aggregate_id, ordinal);
        CREATE INDEX IF NOT EXISTS idx_events_causation
            ON events(causation_id);

        CREATE TABLE IF NOT EXISTS idempotency_results (
            idempotency_key TEXT PRIMARY KEY NOT NULL,
            result_json TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        ",
    )
    .map_err(sqlite_error)?;
    conn.execute(
        "
        INSERT INTO schema_migrations(name, version)
        VALUES('agent_os_store', ?1)
        ON CONFLICT(name) DO UPDATE SET version = excluded.version
        ",
        params![MIGRATION_VERSION],
    )
    .map_err(sqlite_error)?;
    Ok(())
}
