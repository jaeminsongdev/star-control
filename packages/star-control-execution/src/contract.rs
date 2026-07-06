use crate::constants::SCHEMA_VERSION;
use crate::error::ExecutionError;
use serde_json::{json, Value};
use star_control_provider::{ExecutionRequest, ProviderExecution};
use star_control_schema::{load_schema, validate_json};
use std::path::{Path, PathBuf};

pub(crate) fn execution_attempt(request: &ExecutionRequest, status: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "attempt_id": "attempt-0001",
        "job_id": request.job_id(),
        "stage": request.stage(),
        "status": status
    })
}

pub(crate) fn verify_provider_result(
    request: &ExecutionRequest,
    provider_execution: &ProviderExecution,
) -> Result<(), ExecutionError> {
    let result = provider_execution.result();
    compare_output("job_id", request.job_id(), result.job_id())?;
    compare_output("stage", request.stage(), result.stage())?;
    compare_output(
        "provider_instance_id",
        request.provider_instance_id(),
        result.provider_instance_id(),
    )
}

fn compare_output(field: &str, expected: &str, actual: &str) -> Result<(), ExecutionError> {
    if expected == actual {
        Ok(())
    } else {
        Err(ExecutionError::ProviderOutputMismatch {
            field: field.to_string(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    }
}

pub(crate) fn validate_contract(
    value: &Value,
    path: &Path,
    schema_root: &Path,
    schema_file: &str,
) -> Result<(), ExecutionError> {
    let schema_path = schema_root.join(schema_file);
    let schema = load_schema(&schema_path).map_err(|source| ExecutionError::SchemaLoadFailed {
        path: schema_path.clone(),
        message: source.to_string(),
    })?;
    let result = validate_json(value, &schema);
    if result.is_ok() {
        Ok(())
    } else {
        Err(ExecutionError::SchemaValidationFailed {
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
) -> Result<String, ExecutionError> {
    value
        .get(field)
        .ok_or_else(|| ExecutionError::MissingField {
            path: path.to_path_buf(),
            field: field.to_string(),
        })?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| ExecutionError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field.to_string(),
            expected: "string".to_string(),
        })
}

pub(crate) fn object_type_error_path() -> PathBuf {
    PathBuf::from("run-state.json")
}
