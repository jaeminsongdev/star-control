use star_control_api::ApiError;
use star_control_schema::ValidationError;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum UiError {
    Api {
        source: ApiError,
    },
    ApiEnvelopeFailed {
        endpoint: String,
        code: String,
        message: String,
    },
    InvalidApiData {
        endpoint: String,
        message: String,
    },
    SchemaLoadFailed {
        path: PathBuf,
        message: String,
    },
    SchemaValidationFailed {
        path: PathBuf,
        errors: Vec<ValidationError>,
    },
}

impl fmt::Display for UiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Api { source } => write!(formatter, "UI API error: {}", source),
            Self::ApiEnvelopeFailed {
                endpoint,
                code,
                message,
            } => write!(
                formatter,
                "UI API endpoint {} failed with {}: {}",
                endpoint, code, message
            ),
            Self::InvalidApiData { endpoint, message } => {
                write!(
                    formatter,
                    "invalid UI API data for {}: {}",
                    endpoint, message
                )
            }
            Self::SchemaLoadFailed { path, message } => {
                write!(
                    formatter,
                    "UI schema load failed at {}: {}",
                    path.display(),
                    message
                )
            }
            Self::SchemaValidationFailed { path, errors } => {
                write!(
                    formatter,
                    "UI schema validation failed for {} with {} error(s)",
                    path.display(),
                    errors.len()
                )
            }
        }
    }
}

impl Error for UiError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Api { source } => Some(source),
            _ => None,
        }
    }
}

impl From<ApiError> for UiError {
    fn from(source: ApiError) -> Self {
        Self::Api { source }
    }
}
