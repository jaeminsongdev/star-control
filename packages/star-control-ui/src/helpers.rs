use crate::constants::UI_JOB_VIEW_SCHEMA;
use crate::error::UiError;
use serde_json::Value;
use star_control_schema::{load_schema, validate_json};
use std::path::{Path, PathBuf};

pub(crate) fn invalid_data(endpoint: &str, message: &str) -> UiError {
    UiError::InvalidApiData {
        endpoint: endpoint.to_string(),
        message: message.to_string(),
    }
}

pub(crate) fn data_or_error(response: Value, endpoint: &str) -> Result<Value, UiError> {
    data_or_error_with_message(response, endpoint, "API request failed")
}

pub(crate) fn data_or_error_with_message(
    response: Value,
    endpoint: &str,
    default_message: &str,
) -> Result<Value, UiError> {
    if response.get("status").and_then(Value::as_str) == Some("failed") {
        return Err(UiError::ApiEnvelopeFailed {
            endpoint: endpoint.to_string(),
            code: response
                .get("error")
                .and_then(|value| value.get("code"))
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
            message: response
                .get("error")
                .and_then(|value| value.get("message"))
                .and_then(Value::as_str)
                .unwrap_or(default_message)
                .to_string(),
        });
    }
    let data = response
        .get("data")
        .cloned()
        .ok_or_else(|| invalid_data(endpoint, "data object is missing"))?;
    if data.is_object() {
        Ok(data)
    } else {
        Err(invalid_data(endpoint, "data is not an object"))
    }
}

pub(crate) fn validate_job_view_at(schema_root: &Path, view: &Value) -> Result<(), UiError> {
    let schema_path = schema_root.join(UI_JOB_VIEW_SCHEMA);
    let schema = load_schema(&schema_path).map_err(|source| UiError::SchemaLoadFailed {
        path: schema_path.clone(),
        message: source.to_string(),
    })?;
    let result = validate_json(view, &schema);
    if result.is_ok() {
        Ok(())
    } else {
        Err(UiError::SchemaValidationFailed {
            path: PathBuf::from(UI_JOB_VIEW_SCHEMA),
            errors: result.errors,
        })
    }
}

pub(crate) fn string_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}
