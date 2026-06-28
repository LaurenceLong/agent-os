use crate::error::sqlite_error;
use crate::store::SqliteStore;
use agent_os_store::IdempotencyStore;
use agent_os_sys::{AgentOsError, AgentOsResult, SyscallResult};
use rusqlite::{params, OptionalExtension};

impl IdempotencyStore for SqliteStore {
    fn get_syscall_result(&self, idempotency_key: &str) -> AgentOsResult<Option<SyscallResult>> {
        let conn = self.lock()?;
        let result_json: Option<String> = conn
            .query_row(
                "SELECT result_json FROM idempotency_results WHERE idempotency_key = ?1",
                params![idempotency_key],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)?;
        result_json
            .map(|json| serde_json::from_str(&json).map_err(AgentOsError::from))
            .transpose()
    }

    fn put_syscall_result(
        &self,
        idempotency_key: String,
        result: SyscallResult,
    ) -> AgentOsResult<()> {
        let result_json = serde_json::to_string(&result)?;
        let conn = self.lock()?;
        let inserted = conn
            .execute(
                "
                INSERT OR IGNORE INTO idempotency_results(idempotency_key, result_json)
                VALUES(?1, ?2)
                ",
                params![idempotency_key, result_json],
            )
            .map_err(sqlite_error)?;
        if inserted == 0 {
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
    fn idempotency_results_round_trip() {
        let store = SqliteStore::in_memory().unwrap();
        let result =
            SyscallResult::accepted("sys_1", vec!["evt_1".to_string()], json!({"ok": true}));
        store
            .put_syscall_result("idem_1".to_string(), result.clone())
            .unwrap();
        assert_eq!(
            store
                .get_syscall_result("idem_1")
                .unwrap()
                .unwrap()
                .syscall_id,
            result.syscall_id
        );
        assert!(matches!(
            store.put_syscall_result("idem_1".to_string(), result),
            Err(AgentOsError::IdempotencyConflict(_))
        ));
    }
}
