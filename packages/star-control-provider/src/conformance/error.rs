use star_control_schema::ValidationError;
use star_control_state::StateStoreError;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ProviderConformanceError {
    MissingField {
        field: String,
    },
    InvalidFieldType {
        field: String,
        expected: &'static str,
    },
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    InvalidArtifactPath {
        field: String,
        path: String,
        reason: String,
    },
    ArtifactMissing {
        path: PathBuf,
    },
    ArtifactReadFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidJson {
        path: PathBuf,
        source: serde_json::Error,
    },
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
}

impl fmt::Display for ProviderConformanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField { field } => {
                write!(formatter, "provider conformance missing field {}", field)
            }
            Self::InvalidFieldType { field, expected } => write!(
                formatter,
                "provider conformance invalid field type for {}, expected {}",
                field, expected
            ),
            Self::FieldMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "provider conformance field mismatch for {}: expected {}, got {}",
                field, expected, actual
            ),
            Self::InvalidArtifactPath {
                field,
                path,
                reason,
            } => write!(
                formatter,
                "provider conformance invalid artifact path in {}: {} ({})",
                field, path, reason
            ),
            Self::ArtifactMissing { path } => {
                write!(
                    formatter,
                    "provider conformance artifact missing: {}",
                    path.display()
                )
            }
            Self::ArtifactReadFailed { path, source } => write!(
                formatter,
                "provider conformance failed to read artifact {}: {}",
                path.display(),
                source
            ),
            Self::InvalidJson { path, source } => write!(
                formatter,
                "provider conformance invalid JSON in {}: {}",
                path.display(),
                source
            ),
            Self::SchemaLoadFailed { path, message } => write!(
                formatter,
                "provider conformance failed to load schema {}: {}",
                path.display(),
                message
            ),
            Self::SchemaValidationFailed {
                path,
                schema_path,
                errors,
            } => write!(
                formatter,
                "provider conformance schema validation failed for {} against {} with {} error(s)",
                path.display(),
                schema_path.display(),
                errors.len()
            ),
            Self::State(source) => {
                write!(formatter, "provider conformance state error: {}", source)
            }
        }
    }
}

impl Error for ProviderConformanceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ArtifactReadFailed { source, .. } => Some(source),
            Self::InvalidJson { source, .. } => Some(source),
            Self::State(source) => Some(source),
            _ => None,
        }
    }
}

impl From<StateStoreError> for ProviderConformanceError {
    fn from(source: StateStoreError) -> Self {
        Self::State(source)
    }
}
