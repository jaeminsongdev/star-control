use crate::SentinelError;
use serde_json::Value;

pub(crate) fn required_string(
    value: &Value,
    field: &str,
    artifact: &str,
) -> Result<String, SentinelError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| missing_field(artifact, field))
}

pub(crate) fn optional_string(
    value: &Value,
    field: &str,
    artifact: &str,
) -> Result<Option<String>, SentinelError> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(invalid_field(artifact, field, "expected string or null")),
    }
}

pub(crate) fn required_integer(
    value: &Value,
    field: &str,
    artifact: &str,
) -> Result<i64, SentinelError> {
    value
        .get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| missing_field(artifact, field))
}

pub(crate) fn optional_integer(
    value: &Value,
    field: &str,
    artifact: &str,
) -> Result<Option<i64>, SentinelError> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_i64()
            .map(Some)
            .ok_or_else(|| invalid_field(artifact, field, "expected integer")),
        Some(_) => Err(invalid_field(artifact, field, "expected integer or null")),
    }
}

pub(crate) fn required_array<'a>(
    value: &'a Value,
    field: &str,
    artifact: &str,
) -> Result<&'a Vec<Value>, SentinelError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| missing_field(artifact, field))
}

pub(crate) fn required_string_array(
    value: &Value,
    field: &str,
    artifact: &str,
) -> Result<Vec<String>, SentinelError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| missing_field(artifact, field))?
        .iter()
        .enumerate()
        .map(|(index, item)| {
            item.as_str().map(str::to_string).ok_or_else(|| {
                invalid_field(
                    artifact,
                    &format!("{}[{}]", field, index),
                    "expected string",
                )
            })
        })
        .collect()
}

pub(crate) fn optional_string_array(
    value: &Value,
    field: &str,
    artifact: &str,
) -> Result<Vec<String>, SentinelError> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                item.as_str().map(str::to_string).ok_or_else(|| {
                    invalid_field(
                        artifact,
                        &format!("{}[{}]", field, index),
                        "expected string",
                    )
                })
            })
            .collect(),
        Some(_) => Err(invalid_field(artifact, field, "expected array")),
    }
}

pub(crate) fn missing_field(artifact: &str, field: &str) -> SentinelError {
    SentinelError::MissingField {
        artifact: artifact.to_string(),
        field: field.to_string(),
    }
}

pub(crate) fn invalid_field(artifact: &str, field: &str, message: &str) -> SentinelError {
    SentinelError::InvalidField {
        artifact: artifact.to_string(),
        field: field.to_string(),
        message: message.to_string(),
    }
}
