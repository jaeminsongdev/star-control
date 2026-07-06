use super::super::constants::{DEFAULT_TIMEOUT_SECONDS, MAX_TIMEOUT_SECONDS};
use super::policy_denied;
use crate::ProviderAdapterError;
use serde_json::Value;
use std::path::Path;

pub(super) fn required_string(
    value: &Value,
    path: &Path,
    display_field: &str,
    field: &str,
) -> Result<String, ProviderAdapterError> {
    value
        .get(field)
        .ok_or_else(|| ProviderAdapterError::MissingField {
            path: path.to_path_buf(),
            field: display_field.to_string(),
        })?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| ProviderAdapterError::InvalidFieldType {
            path: path.to_path_buf(),
            field: display_field.to_string(),
            expected: "string".to_string(),
        })
}

pub(super) fn required_string_array(
    value: &Value,
    provider_instance_id: &str,
    display_field: &str,
    field: &str,
) -> Result<Vec<String>, ProviderAdapterError> {
    let array = value.get(field).and_then(Value::as_array).ok_or_else(|| {
        policy_denied(
            provider_instance_id,
            &format!("{} must be an array of strings", display_field),
        )
    })?;
    if array.is_empty() {
        return Err(policy_denied(
            provider_instance_id,
            &format!("{} must not be empty", display_field),
        ));
    }
    strings_from_array(provider_instance_id, display_field, array)
}

pub(super) fn optional_string_array(
    value: &Value,
    provider_instance_id: &str,
    display_field: &str,
    field: &str,
) -> Result<Vec<String>, ProviderAdapterError> {
    let Some(array) = value.get(field) else {
        return Ok(Vec::new());
    };
    let Some(array) = array.as_array() else {
        return Err(policy_denied(
            provider_instance_id,
            &format!("{} must be an array of strings", display_field),
        ));
    };
    strings_from_array(provider_instance_id, display_field, array)
}

pub(super) fn timeout_seconds(
    value: &Value,
    provider_instance_id: &str,
) -> Result<u64, ProviderAdapterError> {
    let timeout_seconds = value
        .pointer("/limits/timeout_seconds")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_TIMEOUT_SECONDS);
    if timeout_seconds > MAX_TIMEOUT_SECONDS {
        return Err(policy_denied(
            provider_instance_id,
            &format!("limits.timeout_seconds must be <= {}", MAX_TIMEOUT_SECONDS),
        ));
    }
    Ok(timeout_seconds)
}

fn strings_from_array(
    provider_instance_id: &str,
    display_field: &str,
    array: &[Value],
) -> Result<Vec<String>, ProviderAdapterError> {
    array
        .iter()
        .map(|value| {
            value.as_str().map(str::to_string).ok_or_else(|| {
                policy_denied(
                    provider_instance_id,
                    &format!("{} must be an array of strings", display_field),
                )
            })
        })
        .collect()
}
