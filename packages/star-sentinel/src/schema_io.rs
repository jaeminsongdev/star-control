use crate::SentinelError;
use serde_json::Value;
use star_control_schema::{load_document, load_schema, validate_json};
use std::path::Path;

pub(crate) fn read_validated_json(
    path: &Path,
    schema_root: &Path,
    schema_name: &str,
) -> Result<Value, SentinelError> {
    let value = load_document(path).map_err(|source| SentinelError::DocumentLoad { source })?;
    validate_against_schema(
        &value,
        schema_root,
        schema_name,
        &path.display().to_string(),
    )?;
    Ok(value)
}

pub(crate) fn validate_against_schema(
    value: &Value,
    schema_root: &Path,
    schema_name: &str,
    artifact: &str,
) -> Result<(), SentinelError> {
    let schema_path = schema_root.join(schema_name);
    let schema =
        load_schema(&schema_path).map_err(|source| SentinelError::SchemaLoad { source })?;
    let validation = validate_json(value, &schema);
    if validation.is_ok() {
        Ok(())
    } else {
        Err(SentinelError::SchemaValidation {
            artifact: artifact.to_string(),
            schema: schema_name.to_string(),
            errors: schema_errors(&validation),
        })
    }
}

fn schema_errors(validation: &star_control_schema::ValidationResult) -> Vec<String> {
    validation
        .errors
        .iter()
        .map(|error| format!("{}: {}", error.location, error.message))
        .collect()
}
