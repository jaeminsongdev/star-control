use crate::RouterError;
use serde_json::Value;
use star_control_schema::{load_schema, validate_json};
use std::path::Path;

pub(crate) fn validate_contract(
    value: &Value,
    path: &Path,
    schema_root: &Path,
    schema_file: &str,
) -> Result<(), RouterError> {
    let schema_path = schema_root.join(schema_file);
    let schema = load_schema(&schema_path).map_err(|source| RouterError::SchemaLoadFailed {
        path: schema_path.clone(),
        message: source.to_string(),
    })?;
    let result = validate_json(value, &schema);
    if result.is_ok() {
        Ok(())
    } else {
        Err(RouterError::SchemaValidationFailed {
            path: path.to_path_buf(),
            schema_path,
            errors: result.errors,
        })
    }
}

pub(crate) fn required_string(
    value: &Value,
    path: &Path,
    field: &str,
) -> Result<String, RouterError> {
    value
        .get(field)
        .ok_or_else(|| RouterError::MissingField {
            path: path.to_path_buf(),
            field: field.to_string(),
        })?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| RouterError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field.to_string(),
            expected: "string".to_string(),
        })
}

pub(crate) fn optional_string_array(
    value: &Value,
    path: &Path,
    field: &str,
) -> Result<Vec<String>, RouterError> {
    let Some(values) = value.get(field) else {
        return Ok(Vec::new());
    };
    let values = values
        .as_array()
        .ok_or_else(|| RouterError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field.to_string(),
            expected: "array of string".to_string(),
        })?;
    values
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| RouterError::InvalidFieldType {
                    path: path.to_path_buf(),
                    field: field.to_string(),
                    expected: "array of string".to_string(),
                })
        })
        .collect()
}
