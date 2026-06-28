use agent_os_sys::*;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub(crate) fn parse_payload<T: DeserializeOwned>(value: &Value) -> AgentOsResult<T> {
    Ok(serde_json::from_value(value.clone())?)
}

pub(crate) fn to_value<T: Serialize>(value: T) -> AgentOsResult<Value> {
    Ok(serde_json::to_value(value)?)
}

pub(crate) fn required_string(value: &Value, field: &str) -> AgentOsResult<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| AgentOsError::Validation(format!("missing required field {field}")))
}

pub(crate) fn hash_json<T: Serialize>(value: &T) -> AgentOsResult<String> {
    let encoded = serde_json::to_vec(value)?;
    let digest = Sha256::digest(encoded);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub(crate) fn rfc3339_is_past(value: &str) -> AgentOsResult<bool> {
    let expires_at = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|error| AgentOsError::Validation(format!("invalid RFC3339 timestamp: {error}")))?;
    Ok(OffsetDateTime::now_utc() >= expires_at)
}
