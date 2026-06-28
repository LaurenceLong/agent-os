use agent_os_sys::*;
use serde_json::Value;

pub fn turn_start_op(thread_id: impl Into<String>) -> AgentOp {
    AgentOp {
        abi_version: ABI_VERSION.to_string(),
        op_id: new_id("op_"),
        thread_id: thread_id.into(),
        op_type: "turn.start".to_string(),
        expected_turn_id: None,
        idempotency_key: new_id("idem_"),
        causation_id: None,
        submitted_by: "kernel".to_string(),
        created_at: now_rfc3339(),
        payload: Value::Object(Default::default()),
    }
}
