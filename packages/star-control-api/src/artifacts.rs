use serde_json::Value;
use star_control_schema::{load_schema, validate_json};
use star_control_state::{StateStore, StateStoreError};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(crate) enum ControlArtifactError {
    Missing { path: String },
    ReadFailed { path: PathBuf, message: String },
    InvalidJson { path: PathBuf, message: String },
    SchemaInvalid { schema: String, errors: usize },
    State { source: StateStoreError },
}

impl fmt::Display for ControlArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { path } => write!(formatter, "required artifact not found: {}", path),
            Self::ReadFailed { path, message } => {
                write!(formatter, "failed to read {}: {}", path.display(), message)
            }
            Self::InvalidJson { path, message } => {
                write!(formatter, "invalid JSON at {}: {}", path.display(), message)
            }
            Self::SchemaInvalid { schema, errors } => {
                write!(
                    formatter,
                    "artifact failed schema validation against {} with {} error(s)",
                    schema, errors
                )
            }
            Self::State { source } => write!(formatter, "state store error: {}", source),
        }
    }
}

pub(crate) fn load_job_json(
    store: &StateStore,
    job_id: &str,
    relative_path: &str,
    schema_file: &str,
    schema_root: &Path,
) -> Result<Value, ControlArtifactError> {
    let path = store
        .resolve_job_path(job_id, relative_path)
        .map_err(|source| ControlArtifactError::State { source })?;
    if !path.is_file() {
        return Err(ControlArtifactError::Missing {
            path: format!(".ai-runs/{}/{}", job_id, relative_path),
        });
    }
    let text = fs::read_to_string(&path).map_err(|source| ControlArtifactError::ReadFailed {
        path: path.clone(),
        message: source.to_string(),
    })?;
    let value: Value =
        serde_json::from_str(&text).map_err(|source| ControlArtifactError::InvalidJson {
            path,
            message: source.to_string(),
        })?;
    validate_schema_value(&value, schema_root, schema_file).map_err(|errors| {
        ControlArtifactError::SchemaInvalid {
            schema: schema_file.to_string(),
            errors,
        }
    })?;
    Ok(value)
}

pub(crate) fn validate_schema_value(
    value: &Value,
    schema_root: &Path,
    schema_file: &str,
) -> Result<(), usize> {
    let schema_path = schema_root.join(schema_file);
    let schema = match load_schema(&schema_path) {
        Ok(value) => value,
        Err(_) => return Err(1),
    };
    let result = validate_json(value, &schema);
    if result.is_ok() {
        Ok(())
    } else {
        Err(result.errors.len())
    }
}
