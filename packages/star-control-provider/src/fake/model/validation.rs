use super::error::ProviderAdapterError;
use serde_json::Value;
use star_control_schema::{load_schema, validate_json};
use std::path::Path;

pub(super) const EXECUTION_REQUEST_SCHEMA: &str = "execution-request.schema.json";
pub(super) const PROVIDER_RUN_RESULT_SCHEMA: &str = "provider-run-result.schema.json";

pub(super) fn validate_contract(
    value: &Value,
    path: &Path,
    schema_root: &Path,
    schema_file: &str,
) -> Result<(), ProviderAdapterError> {
    let schema_path = schema_root.join(schema_file);
    let schema =
        load_schema(&schema_path).map_err(|source| ProviderAdapterError::SchemaLoadFailed {
            path: schema_path.clone(),
            message: source.to_string(),
        })?;
    let result = validate_json(value, &schema);
    if result.is_ok() {
        Ok(())
    } else {
        Err(ProviderAdapterError::SchemaValidationFailed {
            path: path.to_path_buf(),
            schema_path,
            errors: result.errors,
        })
    }
}

pub(super) fn required_string(
    value: &Value,
    path: &Path,
    field: &str,
) -> Result<String, ProviderAdapterError> {
    value
        .get(field)
        .ok_or_else(|| ProviderAdapterError::MissingField {
            path: path.to_path_buf(),
            field: field.to_string(),
        })?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| ProviderAdapterError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field.to_string(),
            expected: "string".to_string(),
        })
}
