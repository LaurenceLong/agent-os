use crate::error::sqlite_error;
use agent_os_sys::AgentOsResult;
use rusqlite::{params, Connection};

pub(crate) const MIGRATION_VERSION: i64 = 3;

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

        CREATE TABLE IF NOT EXISTS thread_summaries (
            client_thread_id TEXT PRIMARY KEY NOT NULL,
            agent_thread_id TEXT NOT NULL,
            task_id TEXT,
            goal_id TEXT,
            title TEXT NOT NULL,
            status TEXT NOT NULL,
            active_turn_id TEXT,
            archived INTEGER NOT NULL,
            updated_at TEXT NOT NULL,
            projection_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS turn_summaries (
            turn_id TEXT PRIMARY KEY NOT NULL,
            client_thread_id TEXT,
            agent_thread_id TEXT NOT NULL,
            task_id TEXT,
            goal_id TEXT,
            status TEXT NOT NULL,
            started_at TEXT NOT NULL,
            completed_at TEXT,
            projection_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS timeline_items (
            item_id TEXT PRIMARY KEY NOT NULL,
            event_id TEXT NOT NULL UNIQUE,
            item_type TEXT NOT NULL,
            client_thread_id TEXT,
            agent_id TEXT,
            task_id TEXT,
            turn_id TEXT,
            summary TEXT NOT NULL,
            created_at TEXT NOT NULL,
            projection_json TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_timeline_thread_created
            ON timeline_items(client_thread_id, created_at);

        CREATE TABLE IF NOT EXISTS stats_rollups (
            scope_key TEXT PRIMARY KEY NOT NULL,
            updated_at TEXT,
            projection_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS approval_queue (
            approval_id TEXT PRIMARY KEY NOT NULL,
            client_thread_id TEXT,
            agent_id TEXT NOT NULL,
            task_id TEXT NOT NULL,
            status TEXT NOT NULL,
            requested_at TEXT NOT NULL,
            resolved_at TEXT,
            projection_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS resource_sessions (
            session_id TEXT PRIMARY KEY NOT NULL,
            resource_type TEXT NOT NULL,
            client_thread_id TEXT,
            owner_agent_id TEXT,
            status TEXT NOT NULL,
            lease_expires_at TEXT,
            updated_at TEXT NOT NULL,
            projection_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS automation_schedules (
            schedule_id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            kind TEXT NOT NULL,
            status TEXT NOT NULL,
            target_thread_id TEXT,
            workspace TEXT,
            next_run_at TEXT,
            interval_seconds INTEGER,
            updated_at TEXT NOT NULL,
            projection_json TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_automation_schedules_due
            ON automation_schedules(status, next_run_at);

        CREATE TABLE IF NOT EXISTS automation_runs (
            run_id TEXT PRIMARY KEY NOT NULL,
            schedule_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            status TEXT NOT NULL,
            target_thread_id TEXT,
            workspace TEXT,
            scheduled_for TEXT NOT NULL,
            queued_at TEXT NOT NULL,
            projection_json TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_automation_runs_schedule
            ON automation_runs(schedule_id, scheduled_for);

        CREATE TABLE IF NOT EXISTS artifact_index (
            artifact_id TEXT PRIMARY KEY NOT NULL,
            client_thread_id TEXT,
            agent_id TEXT NOT NULL,
            task_id TEXT NOT NULL,
            artifact_type TEXT NOT NULL,
            created_at TEXT NOT NULL,
            projection_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS evidence_index (
            evidence_id TEXT PRIMARY KEY NOT NULL,
            client_thread_id TEXT,
            agent_id TEXT NOT NULL,
            task_id TEXT NOT NULL,
            evidence_type TEXT NOT NULL,
            created_at TEXT NOT NULL,
            projection_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS projection_checkpoints (
            projection_name TEXT PRIMARY KEY NOT NULL,
            last_event_ordinal INTEGER NOT NULL,
            updated_at TEXT NOT NULL
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
