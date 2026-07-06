mod display;
mod source;

use star_control_schema::ValidationError;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ProviderRegistryError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidJson {
        path: PathBuf,
        source: serde_json::Error,
    },
    UnsupportedFormat {
        path: PathBuf,
    },
    InvalidYamlSubset {
        path: PathBuf,
        line: usize,
        message: String,
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
    MissingField {
        path: PathBuf,
        field: String,
    },
    InvalidFieldType {
        path: PathBuf,
        field: String,
        expected: String,
    },
    PathTraversalBlocked {
        path: String,
    },
    AbsoluteRegistryPathBlocked {
        path: String,
    },
    DuplicateProvider {
        provider_id: String,
    },
    DuplicateCapabilityProfile {
        provider_id: String,
    },
    DuplicateInstance {
        instance_id: String,
    },
    ProviderNotFound {
        provider_id: String,
    },
    InstanceNotFound {
        instance_id: String,
    },
    CapabilityProfileNotFound {
        provider_id: String,
    },
    RegistryManifestIdMismatch {
        registry_id: String,
        manifest_id: String,
        manifest_path: PathBuf,
    },
    RegistryCapabilityProviderMismatch {
        registry_id: String,
        capability_provider: String,
        capability_path: PathBuf,
    },
}
