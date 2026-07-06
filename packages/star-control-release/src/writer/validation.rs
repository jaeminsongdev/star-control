use crate::constants::{RELEASE_READINESS_PATH, RELEASE_READINESS_SCHEMA};
use crate::error::ReleaseReadinessError;
use serde_json::Value;
use star_control_schema::{load_schema, validate_json};
use std::path::{Path, PathBuf};

pub(super) fn validate_readiness(
    schema_root: &Path,
    readiness: &Value,
) -> Result<(), ReleaseReadinessError> {
    validate_schema(schema_root, readiness)?;
    let status = readiness
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| ReleaseReadinessError::InvalidReleaseReadiness {
            message: "status is required".to_string(),
        })?;
    if status == "ready" {
        return Err(ReleaseReadinessError::InvalidReleaseReadiness {
            message: "ready status is reserved until release process approval is implemented"
                .to_string(),
        });
    }
    if status == "reserved" {
        let blockers = readiness
            .get("blockers")
            .and_then(Value::as_array)
            .ok_or_else(|| ReleaseReadinessError::InvalidReleaseReadiness {
                message: "blockers array is required".to_string(),
            })?;
        if blockers.is_empty() {
            return Err(ReleaseReadinessError::InvalidReleaseReadiness {
                message: "reserved readiness must explain why release automation is reserved"
                    .to_string(),
            });
        }
    }
    Ok(())
}

fn validate_schema(schema_root: &Path, readiness: &Value) -> Result<(), ReleaseReadinessError> {
    let schema_path = schema_root.join(RELEASE_READINESS_SCHEMA);
    let schema =
        load_schema(&schema_path).map_err(|source| ReleaseReadinessError::SchemaLoadFailed {
            path: schema_path.clone(),
            message: source.to_string(),
        })?;
    let result = validate_json(readiness, &schema);
    if result.is_ok() {
        Ok(())
    } else {
        Err(ReleaseReadinessError::SchemaValidationFailed {
            path: PathBuf::from(RELEASE_READINESS_PATH),
            errors: result.errors,
        })
    }
}
