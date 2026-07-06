use crate::cloud_policy::cloud_policy_denied;
use crate::ProviderAdapterError;
use serde_json::Value;
use std::path::Path;

pub(super) fn required_string(
    value: &Value,
    path: &Path,
    field_path: &str,
    field: &str,
) -> Result<String, ProviderAdapterError> {
    value
        .get(field)
        .ok_or_else(|| ProviderAdapterError::MissingField {
            path: path.to_path_buf(),
            field: field_path.to_string(),
        })?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| ProviderAdapterError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field_path.to_string(),
            expected: "string".to_string(),
        })
}

pub(super) fn optional_string_array(
    value: &Value,
    provider_instance_id: &str,
    field_path: &str,
    field: &str,
) -> Result<Vec<String>, ProviderAdapterError> {
    let Some(array) = value.get(field) else {
        return Ok(Vec::new());
    };
    let Some(items) = array.as_array() else {
        return Err(cloud_policy_denied(
            provider_instance_id,
            &format!("{} must be an array", field_path),
        ));
    };

    items
        .iter()
        .map(|item| {
            item.as_str().map(str::to_string).ok_or_else(|| {
                cloud_policy_denied(
                    provider_instance_id,
                    &format!("{} must contain strings", field_path),
                )
            })
        })
        .collect()
}
