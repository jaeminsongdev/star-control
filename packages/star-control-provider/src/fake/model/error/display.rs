use super::ProviderAdapterError;
use std::fmt;

impl fmt::Display for ProviderAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "failed to read {}: {}", path.display(), source)
            }
            Self::InvalidJson { path, source } => {
                write!(
                    formatter,
                    "failed to parse JSON {}: {}",
                    path.display(),
                    source
                )
            }
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
            Self::Registry(source) => write!(formatter, "provider registry error: {}", source),
            Self::State(source) => write!(formatter, "state store error: {}", source),
            Self::UnsupportedProvider {
                provider_instance_id,
                provider_id,
            } => write!(
                formatter,
                "provider instance {} resolves to unsupported provider adapter provider {}",
                provider_instance_id, provider_id
            ),
            Self::ProviderOutputAlreadyExists { path } => {
                write!(
                    formatter,
                    "provider output already exists: {}",
                    path.display()
                )
            }
            Self::CommandPolicyDenied {
                provider_instance_id,
                reason,
            } => write!(
                formatter,
                "provider instance {} command policy denied: {}",
                provider_instance_id, reason
            ),
            Self::TransportFailed {
                provider_instance_id,
                message,
            } => write!(
                formatter,
                "provider instance {} transport failed: {}",
                provider_instance_id, message
            ),
        }
    }
}
