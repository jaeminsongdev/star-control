use star_control_schema::ValidationError;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ApiError {
    DuplicateProject {
        project_id: String,
    },
    InvalidProjectId {
        project_id: String,
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

impl fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateProject { project_id } => {
                write!(formatter, "duplicate API project id: {}", project_id)
            }
            Self::InvalidProjectId { project_id } => {
                write!(formatter, "invalid API project id: {}", project_id)
            }
            Self::SchemaLoadFailed { path, message } => {
                write!(
                    formatter,
                    "schema load failed at {}: {}",
                    path.display(),
                    message
                )
            }
            Self::SchemaValidationFailed { path, errors } => {
                write!(
                    formatter,
                    "schema validation failed for {} with {} error(s)",
                    path.display(),
                    errors.len()
                )
            }
        }
    }
}

impl Error for ApiError {}
