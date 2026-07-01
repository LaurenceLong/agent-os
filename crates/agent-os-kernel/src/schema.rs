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
    if let Some(expected) = schema.get("type") {
        validate_type_schema(expected, value, path)?;
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
        Some("array") => {
            validate_array_schema(schema, value, path)?;
            validate_length_bounds(schema, value, path)
        }
        Some("string") => validate_length_bounds(schema, value, path),
        Some("number") | Some("integer") => validate_numeric_bounds(schema, value, path),
        _ => {
            if schema.get("type").and_then(Value::as_array).is_some() {
                validate_union_constraints(schema, value, path)
            } else {
                Ok(())
            }
        }
    }
}

fn validate_type_schema(expected: &Value, value: &Value, path: &str) -> AgentOsResult<()> {
    if let Some(expected) = expected.as_str() {
        return validate_type(expected, value, path);
    }
    let Some(types) = expected.as_array() else {
        return Err(AgentOsError::Validation(format!(
            "{path} uses unsupported schema type declaration"
        )));
    };
    let mut errors = Vec::new();
    for expected in types {
        let expected = expected.as_str().ok_or_else(|| {
            AgentOsError::Validation(format!("{path} type array contains non-string entry"))
        })?;
        match validate_type(expected, value, path) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error.to_string()),
        }
    }
    Err(AgentOsError::Validation(format!(
        "{path} did not match any allowed type: {}",
        errors.join("; ")
    )))
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

fn validate_union_constraints(schema: &Value, value: &Value, path: &str) -> AgentOsResult<()> {
    if value.is_number() {
        validate_numeric_bounds(schema, value, path)?;
    }
    if value.is_string() || value.is_array() {
        validate_length_bounds(schema, value, path)?;
    }
    Ok(())
}

fn validate_numeric_bounds(schema: &Value, value: &Value, path: &str) -> AgentOsResult<()> {
    let Some(number) = value.as_f64() else {
        return Ok(());
    };
    if let Some(minimum) = schema.get("minimum").and_then(Value::as_f64) {
        if number < minimum {
            return Err(AgentOsError::Validation(format!(
                "{path} must be >= {minimum}"
            )));
        }
    }
    if let Some(maximum) = schema.get("maximum").and_then(Value::as_f64) {
        if number > maximum {
            return Err(AgentOsError::Validation(format!(
                "{path} must be <= {maximum}"
            )));
        }
    }
    Ok(())
}

fn validate_length_bounds(schema: &Value, value: &Value, path: &str) -> AgentOsResult<()> {
    let length = if let Some(text) = value.as_str() {
        Some(text.chars().count())
    } else {
        value.as_array().map(Vec::len)
    };
    let Some(length) = length else {
        return Ok(());
    };
    if let Some(min_length) = schema
        .get("minLength")
        .or_else(|| schema.get("minItems"))
        .and_then(Value::as_u64)
    {
        if length < min_length as usize {
            return Err(AgentOsError::Validation(format!(
                "{path} length must be >= {min_length}"
            )));
        }
    }
    if let Some(max_length) = schema
        .get("maxLength")
        .or_else(|| schema.get("maxItems"))
        .and_then(Value::as_u64)
    {
        if length > max_length as usize {
            return Err(AgentOsError::Validation(format!(
                "{path} length must be <= {max_length}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validates_numeric_bounds() {
        let schema = json!({"type": "integer", "minimum": 1, "maximum": 3});
        assert!(validate_json_schema(&schema, &json!(1), "value").is_ok());
        assert!(validate_json_schema(&schema, &json!(0), "value").is_err());
        assert!(validate_json_schema(&schema, &json!(4), "value").is_err());
    }

    #[test]
    fn validates_string_and_array_lengths() {
        let string_schema = json!({"type": "string", "minLength": 2, "maxLength": 3});
        assert!(validate_json_schema(&string_schema, &json!("ab"), "value").is_ok());
        assert!(validate_json_schema(&string_schema, &json!("a"), "value").is_err());
        assert!(validate_json_schema(&string_schema, &json!("abcd"), "value").is_err());

        let array_schema = json!({"type": "array", "minItems": 1, "maxItems": 2});
        assert!(validate_json_schema(&array_schema, &json!([1]), "value").is_ok());
        assert!(validate_json_schema(&array_schema, &json!([]), "value").is_err());
        assert!(validate_json_schema(&array_schema, &json!([1, 2, 3]), "value").is_err());
    }

    #[test]
    fn validates_union_type() {
        let schema = json!({"type": ["string", "null"], "maxLength": 3});
        assert!(validate_json_schema(&schema, &json!("abc"), "value").is_ok());
        assert!(validate_json_schema(&schema, &Value::Null, "value").is_ok());
        assert!(validate_json_schema(&schema, &json!(1), "value").is_err());
        assert!(validate_json_schema(&schema, &json!("abcd"), "value").is_err());
    }
}
