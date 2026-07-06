use super::super::error::ProviderConformanceError;
use crate::ProviderRunContext;
use serde_json::Value;
use star_control_schema::{load_schema, validate_json};
use std::fs;

pub(crate) fn read_and_validate_json_artifact(
    context: &ProviderRunContext<'_>,
    job_id: &str,
    relative_path: &str,
    schema_file: &str,
) -> Result<Value, ProviderConformanceError> {
    let artifact_path = context
        .state_store()
        .resolve_job_path(job_id, relative_path)?;
    if !artifact_path.is_file() {
        return Err(ProviderConformanceError::ArtifactMissing {
            path: artifact_path,
        });
    }
    let content = fs::read_to_string(&artifact_path).map_err(|source| {
        ProviderConformanceError::ArtifactReadFailed {
            path: artifact_path.clone(),
            source,
        }
    })?;
    let value: Value =
        serde_json::from_str(&content).map_err(|source| ProviderConformanceError::InvalidJson {
            path: artifact_path.clone(),
            source,
        })?;
    let schema_path = context.schema_root().join(schema_file);
    let schema =
        load_schema(&schema_path).map_err(|source| ProviderConformanceError::SchemaLoadFailed {
            path: schema_path.clone(),
            message: source.to_string(),
        })?;
    let result = validate_json(&value, &schema);
    if result.is_ok() {
        Ok(value)
    } else {
        Err(ProviderConformanceError::SchemaValidationFailed {
            path: artifact_path,
            schema_path,
            errors: result.errors,
        })
    }
}
