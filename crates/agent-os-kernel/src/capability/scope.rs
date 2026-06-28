use agent_os_sys::{AgentOsError, AgentOsResult};
use serde_json::Value;

pub(super) fn requested_resource_scopes(value: &Value) -> AgentOsResult<Vec<String>> {
    requested_scope_key(value)
}

pub(super) fn requested_scope_key(value: &Value) -> AgentOsResult<Vec<String>> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Object(map) if map.is_empty() => Ok(Vec::new()),
        Value::String(scope) => Ok(vec![scope.clone()]),
        Value::Array(values) => values
            .iter()
            .map(requested_scope_key)
            .collect::<AgentOsResult<Vec<_>>>()
            .map(|nested| nested.into_iter().flatten().collect()),
        Value::Object(map) => {
            if let Some(scope) = map.get("scope").and_then(Value::as_str) {
                return Ok(vec![scope.to_string()]);
            }
            let resource_type = map
                .get("resource_type")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    AgentOsError::Validation(
                        "resource_scope object requires scope or resource_type".to_string(),
                    )
                })?;
            let resource_id = map
                .get("resource_id")
                .and_then(Value::as_str)
                .unwrap_or("*");
            Ok(vec![format!("{resource_type}:{resource_id}")])
        }
        _ => Err(AgentOsError::Validation(
            "resource_scope must be an object, string, or array".to_string(),
        )),
    }
}

pub(super) fn scope_list_allows(allowed_scopes: &[String], requested_scope: &str) -> bool {
    allowed_scopes
        .iter()
        .any(|allowed| scope_pattern_allows(allowed, requested_scope))
}

fn scope_pattern_allows(allowed: &str, requested: &str) -> bool {
    if allowed == "*" || allowed == requested {
        return true;
    }
    allowed.strip_suffix(":*").is_some_and(|prefix| {
        requested == prefix
            || requested
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with(':'))
    })
}
