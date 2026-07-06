use star_control_schema::{DocumentLoadError, SchemaLoadError};
use star_control_state::StateStoreError;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum SentinelError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    SchemaLoad {
        source: SchemaLoadError,
    },
    DocumentLoad {
        source: DocumentLoadError,
    },
    State {
        source: StateStoreError,
    },
    SchemaValidation {
        artifact: String,
        schema: String,
        errors: Vec<String>,
    },
    MissingField {
        artifact: String,
        field: String,
    },
    InvalidField {
        artifact: String,
        field: String,
        message: String,
    },
    Registry {
        message: String,
    },
}

impl fmt::Display for SentinelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "failed to read {}: {}", path.display(), source)
            }
            Self::SchemaLoad { source } => write!(formatter, "schema load failed: {}", source),
            Self::DocumentLoad { source } => write!(formatter, "document load failed: {}", source),
            Self::State { source } => write!(formatter, "state store operation failed: {}", source),
            Self::SchemaValidation {
                artifact,
                schema,
                errors,
            } => write!(
                formatter,
                "{} failed {} validation with {} error(s)",
                artifact,
                schema,
                errors.len()
            ),
            Self::MissingField { artifact, field } => {
                write!(formatter, "{} missing required field {}", artifact, field)
            }
            Self::InvalidField {
                artifact,
                field,
                message,
            } => write!(
                formatter,
                "{} field {} is invalid: {}",
                artifact, field, message
            ),
            Self::Registry { message } => write!(formatter, "rule registry invalid: {}", message),
        }
    }
}

impl Error for SentinelError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::SchemaLoad { source } => Some(source),
            Self::DocumentLoad { source } => Some(source),
            Self::State { source } => Some(source),
            _ => None,
        }
    }
}
