use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub const ABI_VERSION: &str = "0.1";

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub type AgentOsResult<T> = Result<T, AgentOsError>;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AgentOsError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid state transition: {0}")]
    InvalidTransition(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("approval required: {0}")]
    ApprovalRequired(String),
    #[error("resource conflict: {0}")]
    ResourceConflict(String),
    #[error("budget exhausted: {0}")]
    BudgetExhausted(String),
    #[error("idempotency conflict: {0}")]
    IdempotencyConflict(String),
    #[error("unsupported operation: {0}")]
    Unsupported(String),
    #[error("serialization failed: {0}")]
    Serialization(String),
}

impl From<serde_json::Error> for AgentOsError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialization(value.to_string())
    }
}

pub fn new_id(prefix: &str) -> String {
    format!("{prefix}{:016x}", NEXT_ID.fetch_add(1, Ordering::SeqCst))
}

pub fn seed_id_allocator_from_events(events: &[crate::EventEnvelope]) {
    for event in events {
        observe_existing_id(&event.event_id);
        observe_existing_id(&event.aggregate_id);
        if let Some(agent_id) = &event.agent_id {
            observe_existing_id(agent_id);
        }
        if let Some(task_id) = &event.task_id {
            observe_existing_id(task_id);
        }
        if let Some(causation_id) = &event.causation_id {
            observe_existing_id(causation_id);
        }
        if let Some(correlation_id) = &event.correlation_id {
            observe_existing_id(correlation_id);
        }
        observe_json_ids(&event.payload);
    }
}

fn observe_json_ids(value: &Value) {
    match value {
        Value::String(text) => observe_existing_id(text),
        Value::Array(values) => {
            for value in values {
                observe_json_ids(value);
            }
        }
        Value::Object(entries) => {
            for value in entries.values() {
                observe_json_ids(value);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn observe_existing_id(value: &str) {
    let Some(number) = parse_hex_id_suffix(value) else {
        return;
    };
    let target = number.saturating_add(1);
    loop {
        let current = NEXT_ID.load(Ordering::SeqCst);
        if current >= target {
            break;
        }
        if NEXT_ID
            .compare_exchange(current, target, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            break;
        }
    }
}

fn parse_hex_id_suffix(value: &str) -> Option<u64> {
    let (prefix, suffix) = value.rsplit_once('_')?;
    if prefix.is_empty() || suffix.len() != 16 || !suffix.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    u64::from_str_radix(suffix, 16).ok()
}

pub fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

pub fn empty_object() -> Value {
    json!({})
}

pub fn wildcard_allows(list: &[String], value: &str) -> bool {
    list.iter().any(|item| item == "*" || item == value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn seeding_id_allocator_from_events_avoids_reused_ids() {
        let event = crate::EventEnvelope {
            event_id: "evt_00000000000000f0".to_string(),
            event_type: "TaskSpawned".to_string(),
            abi_version: ABI_VERSION.to_string(),
            aggregate_type: "task".to_string(),
            aggregate_id: "task_00000000000000f1".to_string(),
            agent_id: Some("agt_00000000000000f2".to_string()),
            task_id: Some("task_00000000000000f1".to_string()),
            causation_id: None,
            correlation_id: Some("goal_00000000000000f3".to_string()),
            payload: json!({
                "artifact_id": "art_00000000000000f4",
                "nested": ["evd_00000000000000f5"]
            }),
            created_at: now_rfc3339(),
        };
        seed_id_allocator_from_events(&[event]);
        let next = new_id("evt_");
        assert!(next.as_str() > "evt_00000000000000f5");
    }
}
