use crate::ProviderRegistryError;
use star_control_schema::ValidationError;
use star_control_state::StateStoreError;
use std::path::PathBuf;

mod display;
mod source;

#[derive(Debug)]
pub enum ProviderAdapterError {
    Io {
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
    MissingField {
        path: PathBuf,
        field: String,
    },
    InvalidFieldType {
        path: PathBuf,
        field: String,
        expected: String,
    },
    Registry(ProviderRegistryError),
    State(StateStoreError),
    UnsupportedProvider {
        provider_instance_id: String,
        provider_id: String,
    },
    ProviderOutputAlreadyExists {
        path: PathBuf,
    },
    CommandPolicyDenied {
        provider_instance_id: String,
        reason: String,
    },
    TransportFailed {
        provider_instance_id: String,
        message: String,
    },
}

impl From<ProviderRegistryError> for ProviderAdapterError {
    fn from(source: ProviderRegistryError) -> Self {
        Self::Registry(source)
    }
}

impl From<StateStoreError> for ProviderAdapterError {
    fn from(source: StateStoreError) -> Self {
        Self::State(source)
    }
}
