use agent_os_sys::*;
use serde_json::Value;

pub(crate) fn validate_json_schema(
    schema: &Value,
    value: &Value,
    label: &str,
) -> AgentOsResult<()> {
    validate_schema_at(schema, value, label)
}

fn validate_schema_at(schema: &Value, value: &Value, path: &str) -> AgentOsResult<()> {
    if let Some(expected) = schema.get("type").and_then(Value::as_str) {
        validate_type(expected, value, path)?;
    }
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        if !values.iter().any(|allowed| allowed == value) {
            return Err(AgentOsError::Validation(format!(
                "{path} does not match any enum value"
            )));
        }
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("object") => validate_object_schema(schema, value, path),
        Some("array") => validate_array_schema(schema, value, path),
        _ => Ok(()),
    }
}

fn validate_type(expected: &str, value: &Value, path: &str) -> AgentOsResult<()> {
    let ok = match expected {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        other => {
            return Err(AgentOsError::Validation(format!(
                "{path} uses unsupported schema type {other}"
            )));
        }
    };
    if ok {
        Ok(())
    } else {
        Err(AgentOsError::Validation(format!(
            "{path} expected {expected}"
        )))
    }
}

fn validate_object_schema(schema: &Value, value: &Value, path: &str) -> AgentOsResult<()> {
    let object = value
        .as_object()
        .ok_or_else(|| AgentOsError::Validation(format!("{path} expected object")))?;
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for field in required {
            let field = field.as_str().ok_or_else(|| {
                AgentOsError::Validation(format!("{path}.required contains non-string field"))
            })?;
            if !object.contains_key(field) {
                return Err(AgentOsError::Validation(format!(
                    "{path} missing required field {field}"
                )));
            }
        }
    }
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        for (field, field_schema) in properties {
            if let Some(field_value) = object.get(field) {
                validate_schema_at(field_schema, field_value, &format!("{path}.{field}"))?;
            }
        }
        if schema.get("additionalProperties").and_then(Value::as_bool) == Some(false) {
            for field in object.keys() {
                if !properties.contains_key(field) {
                    return Err(AgentOsError::Validation(format!(
                        "{path} contains unsupported field {field}"
                    )));
                }
            }
        }
    }
    Ok(())
}

fn validate_array_schema(schema: &Value, value: &Value, path: &str) -> AgentOsResult<()> {
    let items = schema.get("items");
    if let Some(item_schema) = items {
        for (index, item) in value
            .as_array()
            .ok_or_else(|| AgentOsError::Validation(format!("{path} expected array")))?
            .iter()
            .enumerate()
        {
            validate_schema_at(item_schema, item, &format!("{path}[{index}]"))?;
        }
    }
    Ok(())
}
