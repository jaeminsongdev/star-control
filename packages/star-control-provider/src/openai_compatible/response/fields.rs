use super::OpenAiCompatibleParseError;
use serde_json::Value;

pub(super) fn optional_string(
    value: &Value,
    field: &str,
) -> Result<Option<String>, OpenAiCompatibleParseError> {
    let Some(item) = value.get(field) else {
        return Ok(None);
    };
    if item.is_null() {
        return Ok(None);
    }
    item.as_str()
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| invalid_type(field, "string or null"))
}

pub(super) fn optional_u64(
    value: &Value,
    field: &str,
) -> Result<Option<u64>, OpenAiCompatibleParseError> {
    let Some(item) = value.get(field) else {
        return Ok(None);
    };
    if item.is_null() {
        return Ok(None);
    }
    item.as_u64()
        .map(Some)
        .ok_or_else(|| invalid_type(field, "unsigned integer or null"))
}

pub(super) fn invalid_type(field: &str, expected: &'static str) -> OpenAiCompatibleParseError {
    OpenAiCompatibleParseError::InvalidFieldType {
        field: field.to_string(),
        expected,
    }
}
