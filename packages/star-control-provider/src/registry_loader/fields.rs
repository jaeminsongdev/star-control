use crate::registry_error::ProviderRegistryError;
use serde_json::Value;
use std::path::Path;

pub(super) fn required_string(
    value: &Value,
    path: &Path,
    field: &str,
) -> Result<String, ProviderRegistryError> {
    value
        .get(field)
        .ok_or_else(|| ProviderRegistryError::MissingField {
            path: path.to_path_buf(),
            field: field.to_string(),
        })?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| ProviderRegistryError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field.to_string(),
            expected: "string".to_string(),
        })
}

pub(super) fn required_bool(
    value: &Value,
    path: &Path,
    field: &str,
) -> Result<bool, ProviderRegistryError> {
    value
        .get(field)
        .ok_or_else(|| ProviderRegistryError::MissingField {
            path: path.to_path_buf(),
            field: field.to_string(),
        })?
        .as_bool()
        .ok_or_else(|| ProviderRegistryError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field.to_string(),
            expected: "boolean".to_string(),
        })
}

pub(super) fn required_string_array(
    value: &Value,
    path: &Path,
    field: &str,
) -> Result<Vec<String>, ProviderRegistryError> {
    let values = value.get(field).and_then(Value::as_array).ok_or_else(|| {
        ProviderRegistryError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field.to_string(),
            expected: "array of string".to_string(),
        }
    })?;
    string_array_from_values(values, path, field)
}

pub(super) fn pointer_string_array(
    value: &Value,
    path: &Path,
    pointer: &str,
) -> Result<Vec<String>, ProviderRegistryError> {
    let values = value
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderRegistryError::InvalidFieldType {
            path: path.to_path_buf(),
            field: pointer.to_string(),
            expected: "array of string".to_string(),
        })?;
    string_array_from_values(values, path, pointer)
}

pub(super) fn nested_required_string(
    value: &Value,
    path: &Path,
    parent: &str,
    field: &str,
) -> Result<String, ProviderRegistryError> {
    let full_field = format!("{}.{}", parent, field);
    value
        .get(field)
        .ok_or_else(|| ProviderRegistryError::MissingField {
            path: path.to_path_buf(),
            field: full_field.clone(),
        })?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| ProviderRegistryError::InvalidFieldType {
            path: path.to_path_buf(),
            field: full_field,
            expected: "string".to_string(),
        })
}

fn string_array_from_values(
    values: &[Value],
    path: &Path,
    field: &str,
) -> Result<Vec<String>, ProviderRegistryError> {
    values
        .iter()
        .map(|value| {
            value.as_str().map(str::to_string).ok_or_else(|| {
                ProviderRegistryError::InvalidFieldType {
                    path: path.to_path_buf(),
                    field: field.to_string(),
                    expected: "array of string".to_string(),
                }
            })
        })
        .collect()
}
