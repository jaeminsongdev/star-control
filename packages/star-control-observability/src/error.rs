use star_control_schema::ValidationError;
use star_control_state::StateStoreError;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ObservabilityError {
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
    InvalidAuditEvent {
        message: String,
    },
    InvalidCostMetric {
        message: String,
    },
    AppendFailed {
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
    CorruptAuditLog {
        path: PathBuf,
        line: usize,
        message: String,
    },
}

impl fmt::Display for ObservabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::State { source } => write!(formatter, "state store error: {}", source),
            Self::SchemaLoadFailed { path, message } => {
                write!(
                    formatter,
                    "audit schema load failed at {}: {}",
                    path.display(),
                    message
                )
            }
            Self::SchemaValidationFailed { path, errors } => {
                write!(
                    formatter,
                    "audit event schema validation failed for {} with {} error(s)",
                    path.display(),
                    errors.len()
                )
            }
            Self::InvalidAuditEvent { message } => {
                write!(formatter, "invalid audit event: {}", message)
            }
            Self::InvalidCostMetric { message } => {
                write!(formatter, "invalid cost metric: {}", message)
            }
            Self::AppendFailed { path, source } => {
                write!(
                    formatter,
                    "failed to append audit log {}: {}",
                    path.display(),
                    source
                )
            }
            Self::ReadFailed { path, source } => {
                write!(
                    formatter,
                    "failed to read observability artifact {}: {}",
                    path.display(),
                    source
                )
            }
            Self::InvalidJson { path, source } => {
                write!(
                    formatter,
                    "invalid audit JSON at {}: {}",
                    path.display(),
                    source
                )
            }
            Self::CorruptAuditLog {
                path,
                line,
                message,
            } => write!(
                formatter,
                "corrupt audit log {} at line {}: {}",
                path.display(),
                line,
                message
            ),
        }
    }
}

impl Error for ObservabilityError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::State { source } => Some(source),
            Self::AppendFailed { source, .. } => Some(source),
            Self::ReadFailed { source, .. } => Some(source),
            Self::InvalidJson { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<StateStoreError> for ObservabilityError {
    fn from(source: StateStoreError) -> Self {
        Self::State { source }
    }
}
