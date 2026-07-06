use star_control_provider::ProviderRegistryError;
use star_control_schema::ValidationError;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum RouterError {
    SchemaLoadFailed {
        path: PathBuf,
        message: String,
    },
    SchemaValidationFailed {
        path: PathBuf,
        schema_path: PathBuf,
        errors: Vec<ValidationError>,
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
    ProviderRegistry(ProviderRegistryError),
    NoProviderAvailable {
        role: String,
        capability: String,
    },
}

impl fmt::Display for RouterError {
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
            Self::ProviderRegistry(source) => {
                write!(formatter, "provider registry error: {}", source)
            }
            Self::NoProviderAvailable { role, capability } => write!(
                formatter,
                "no provider available for role {} requiring {}",
                role, capability
            ),
        }
    }
}

impl Error for RouterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ProviderRegistry(source) => Some(source),
            _ => None,
        }
    }
}

impl From<ProviderRegistryError> for RouterError {
    fn from(source: ProviderRegistryError) -> Self {
        Self::ProviderRegistry(source)
    }
}
