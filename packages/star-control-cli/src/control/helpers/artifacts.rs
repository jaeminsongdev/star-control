use crate::error::CliError;
use serde_json::Value;
use star_control_schema::{load_schema, validate_json};
use star_control_state::StateStore;
use std::fs;
use std::path::Path;

pub(in crate::control) fn load_job_json(
    store: &StateStore,
    job_id: &str,
    relative_path: &str,
    schema_file: &str,
    command: &str,
    schema_root: &Path,
) -> Result<Value, CliError> {
    let path = store
        .resolve_job_path(job_id, relative_path)
        .map_err(|source| CliError::State {
            command: command.to_string(),
            source,
        })?;
    if !path.is_file() {
        return Err(CliError::MissingArtifact {
            command: command.to_string(),
            message: format!("required artifact not found: {}", relative_path),
            artifact_paths: vec![format!(".ai-runs/{}/{}", job_id, relative_path)],
        });
    }
    let value: Value =
        serde_json::from_str(
            &fs::read_to_string(&path).map_err(|source| CliError::Internal {
                command: command.to_string(),
                message: format!("failed to read {}: {}", path.display(), source),
            })?,
        )
        .map_err(|source| CliError::Internal {
            command: command.to_string(),
            message: format!("invalid JSON at {}: {}", path.display(), source),
        })?;
    validate_schema_value(&value, schema_root, schema_file, relative_path).map_err(|message| {
        CliError::Internal {
            command: command.to_string(),
            message,
        }
    })?;
    Ok(value)
}

pub(in crate::control) fn validate_schema_value(
    value: &Value,
    schema_root: &Path,
    schema_file: &str,
    logical_path: &str,
) -> Result<(), String> {
    let schema_path = schema_root.join(schema_file);
    let schema = load_schema(&schema_path).map_err(|source| source.to_string())?;
    let result = validate_json(value, &schema);
    if result.is_ok() {
        Ok(())
    } else {
        Err(format!(
            "{} failed schema validation against {} with {} error(s)",
            logical_path,
            schema_file,
            result.errors.len()
        ))
    }
}
