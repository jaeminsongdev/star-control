use super::ProviderRegistryError;
use std::fmt;

impl fmt::Display for ProviderRegistryError {
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
            Self::UnsupportedFormat { path } => {
                write!(
                    formatter,
                    "unsupported provider contract format: {}",
                    path.display()
                )
            }
            Self::InvalidYamlSubset {
                path,
                line,
                message,
            } => write!(
                formatter,
                "failed to parse Star-Control YAML subset {} at line {}: {}",
                path.display(),
                line,
                message
            ),
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
            Self::PathTraversalBlocked { path } => {
                write!(formatter, "registry path traversal blocked: {}", path)
            }
            Self::AbsoluteRegistryPathBlocked { path } => {
                write!(formatter, "absolute registry path blocked: {}", path)
            }
            Self::DuplicateProvider { provider_id } => {
                write!(formatter, "duplicate provider manifest: {}", provider_id)
            }
            Self::DuplicateCapabilityProfile { provider_id } => {
                write!(formatter, "duplicate capability profile: {}", provider_id)
            }
            Self::DuplicateInstance { instance_id } => {
                write!(formatter, "duplicate provider instance: {}", instance_id)
            }
            Self::ProviderNotFound { provider_id } => {
                write!(formatter, "provider not found: {}", provider_id)
            }
            Self::InstanceNotFound { instance_id } => {
                write!(formatter, "provider instance not found: {}", instance_id)
            }
            Self::CapabilityProfileNotFound { provider_id } => {
                write!(formatter, "capability profile not found: {}", provider_id)
            }
            Self::RegistryManifestIdMismatch {
                registry_id,
                manifest_id,
                manifest_path,
            } => write!(
                formatter,
                "registry provider id {} does not match manifest id {} at {}",
                registry_id,
                manifest_id,
                manifest_path.display()
            ),
            Self::RegistryCapabilityProviderMismatch {
                registry_id,
                capability_provider,
                capability_path,
            } => write!(
                formatter,
                "registry provider id {} does not match capability provider {} at {}",
                registry_id,
                capability_provider,
                capability_path.display()
            ),
        }
    }
}
