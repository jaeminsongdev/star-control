use star_control_schema::ValidationError;
use star_control_state::StateStoreError;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ReleaseReadinessError {
    State {
        source: StateStoreError,
    },
    SchemaLoadFailed {
        path: PathBuf,
        message: String,
    },
    SchemaValidationFailed {
        path: PathBuf,
        errors: Vec<ValidationError>,
    },
    InvalidReleaseReadiness {
        message: String,
    },
    InvalidReleaseEvidence {
        message: String,
    },
    WriteFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    ReadFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidJson {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl fmt::Display for ReleaseReadinessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::State { source } => write!(formatter, "state store error: {}", source),
            Self::SchemaLoadFailed { path, message } => write!(
                formatter,
                "release readiness schema load failed at {}: {}",
                path.display(),
                message
            ),
            Self::SchemaValidationFailed { path, errors } => write!(
                formatter,
                "release readiness schema validation failed for {} with {} error(s)",
                path.display(),
                errors.len()
            ),
            Self::InvalidReleaseReadiness { message } => {
                write!(formatter, "invalid release readiness: {}", message)
            }
            Self::InvalidReleaseEvidence { message } => {
                write!(formatter, "invalid release evidence: {}", message)
            }
            Self::WriteFailed { path, source } => write!(
                formatter,
                "failed to write release artifact {}: {}",
                path.display(),
                source
            ),
            Self::ReadFailed { path, source } => write!(
                formatter,
                "failed to read release readiness artifact {}: {}",
                path.display(),
                source
            ),
            Self::InvalidJson { path, source } => write!(
                formatter,
                "invalid release readiness JSON at {}: {}",
                path.display(),
                source
            ),
        }
    }
}

impl Error for ReleaseReadinessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::State { source } => Some(source),
            Self::WriteFailed { source, .. } => Some(source),
            Self::ReadFailed { source, .. } => Some(source),
            Self::InvalidJson { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<StateStoreError> for ReleaseReadinessError {
    fn from(source: StateStoreError) -> Self {
        Self::State { source }
    }
}
