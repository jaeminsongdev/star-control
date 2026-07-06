use star_control_schema::ValidationError;
use star_control_state::StateStoreError;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ValidationEngineError {
    SchemaLoadFailed {
        path: PathBuf,
        message: String,
    },
    SchemaValidationFailed {
        path: PathBuf,
        schema_path: PathBuf,
        errors: Vec<ValidationError>,
    },
    State(StateStoreError),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidJson {
        path: PathBuf,
        source: serde_json::Error,
    },
    MissingField {
        path: PathBuf,
        field: String,
    },
    InvalidFieldType {
        path: PathBuf,
        field: String,
        expected: String,
    },
    ProviderOutputMissing {
        path: PathBuf,
    },
    ApprovalResponseMissing {
        path: PathBuf,
    },
    ApprovalResponseNotApproved {
        response: String,
    },
    ApprovalResponseMismatch {
        field: String,
        expected: String,
        actual: String,
    },
}

impl fmt::Display for ValidationEngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaLoadFailed { path, message } => {
                write!(
                    formatter,
                    "failed to load schema {}: {}",
                    path.display(),
                    message
                )
            }
            Self::SchemaValidationFailed {
                path,
                schema_path,
                errors,
            } => write!(
                formatter,
                "schema validation failed for {} against {} with {} error(s)",
                path.display(),
                schema_path.display(),
                errors.len()
            ),
            Self::State(source) => write!(formatter, "state store failed: {}", source),
            Self::Io { path, source } => {
                write!(formatter, "failed to read {}: {}", path.display(), source)
            }
            Self::InvalidJson { path, source } => {
                write!(formatter, "invalid JSON at {}: {}", path.display(), source)
            }
            Self::MissingField { path, field } => {
                write!(formatter, "missing field {} in {}", field, path.display())
            }
            Self::InvalidFieldType {
                path,
                field,
                expected,
            } => write!(
                formatter,
                "invalid field type for {} in {}, expected {}",
                field,
                path.display(),
                expected
            ),
            Self::ProviderOutputMissing { path } => {
                write!(formatter, "provider output missing at {}", path.display())
            }
            Self::ApprovalResponseMissing { path } => {
                write!(formatter, "approval response missing at {}", path.display())
            }
            Self::ApprovalResponseNotApproved { response } => {
                write!(formatter, "approval response is not approved: {}", response)
            }
            Self::ApprovalResponseMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "approval response {} mismatch: expected {}, got {}",
                field, expected, actual
            ),
        }
    }
}

impl Error for ValidationEngineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::State(source) => Some(source),
            Self::Io { source, .. } => Some(source),
            Self::InvalidJson { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<StateStoreError> for ValidationEngineError {
    fn from(source: StateStoreError) -> Self {
        Self::State(source)
    }
}
