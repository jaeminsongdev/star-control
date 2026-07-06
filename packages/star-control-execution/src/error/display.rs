use super::ExecutionError;
use std::fmt;

impl fmt::Display for ExecutionError {
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
            Self::ProviderAdapter(source) => {
                write!(formatter, "provider adapter error: {}", source)
            }
            Self::State(source) => write!(formatter, "state store error: {}", source),
            Self::ProviderAssignmentMissing { stage } => {
                write!(formatter, "provider assignment missing for stage {}", stage)
            }
            Self::ProviderAssignmentMismatch {
                provider,
                provider_instance,
            } => write!(
                formatter,
                "workspec provider {} does not match provider_instance {}",
                provider, provider_instance
            ),
            Self::ProviderOutputMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "provider output mismatch for {}: expected {}, got {}",
                field, expected, actual
            ),
            Self::StageAlreadyExecuted {
                job_id,
                stage,
                provider_instance_id,
            } => write!(
                formatter,
                "stage {} for job {} already has provider output for {}",
                stage, job_id, provider_instance_id
            ),
        }
    }
}
