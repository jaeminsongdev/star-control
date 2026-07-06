use super::super::error::ProviderConformanceError;
use super::paths::check_path_equals;
use serde_json::Value;

pub(crate) fn required_string(
    value: &Value,
    field: &str,
) -> Result<String, ProviderConformanceError> {
    let item = value
        .get(field)
        .ok_or_else(|| ProviderConformanceError::MissingField {
            field: field.to_string(),
        })?;
    item.as_str().map(ToString::to_string).ok_or_else(|| {
        ProviderConformanceError::InvalidFieldType {
            field: field.to_string(),
            expected: "string",
        }
    })
}

pub(crate) fn nullable_string(
    value: &Value,
    field: &str,
) -> Result<Option<String>, ProviderConformanceError> {
    let item = value
        .get(field)
        .ok_or_else(|| ProviderConformanceError::MissingField {
            field: field.to_string(),
        })?;
    if item.is_null() {
        return Ok(None);
    }
    item.as_str()
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| ProviderConformanceError::InvalidFieldType {
            field: field.to_string(),
            expected: "string or null",
        })
}

pub(crate) fn check_result_field(
    value: &Value,
    field: &str,
    expected: &str,
) -> Result<(), ProviderConformanceError> {
    let actual = required_string(value, field)?;
    check_path_equals(field, &actual, expected)
}

pub(crate) fn check_ref_contract(
    value: &Value,
    field: &str,
    expected: &str,
    expected_kind: &str,
    expected_producer: &str,
) -> Result<(), ProviderConformanceError> {
    let path_field = format!("{}.path", field);
    let actual = value.get("path").and_then(Value::as_str).ok_or_else(|| {
        ProviderConformanceError::MissingField {
            field: path_field.clone(),
        }
    })?;
    check_path_equals(&path_field, actual, expected)?;

    let kind_field = format!("{}.kind", field);
    let kind = value.get("kind").and_then(Value::as_str).ok_or_else(|| {
        ProviderConformanceError::MissingField {
            field: kind_field.clone(),
        }
    })?;
    check_path_equals(&kind_field, kind, expected_kind)?;

    let producer_field = format!("{}.producer", field);
    let producer = value
        .get("producer")
        .and_then(Value::as_str)
        .ok_or_else(|| ProviderConformanceError::MissingField {
            field: producer_field.clone(),
        })?;
    check_path_equals(&producer_field, producer, expected_producer)
}
