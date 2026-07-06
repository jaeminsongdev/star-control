mod display;
mod source;

use star_control_provider::{ProviderAdapterError, ProviderRegistryError};
use star_control_schema::ValidationError;
use star_control_state::StateStoreError;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ExecutionError {
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
    ProviderAdapter(ProviderAdapterError),
    State(StateStoreError),
    ProviderAssignmentMissing {
        stage: String,
    },
    ProviderAssignmentMismatch {
        provider: String,
        provider_instance: String,
    },
    ProviderOutputMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    StageAlreadyExecuted {
        job_id: String,
        stage: String,
        provider_instance_id: String,
    },
}

impl From<ProviderRegistryError> for ExecutionError {
    fn from(source: ProviderRegistryError) -> Self {
        Self::ProviderRegistry(source)
    }
}

impl From<ProviderAdapterError> for ExecutionError {
    fn from(source: ProviderAdapterError) -> Self {
        Self::ProviderAdapter(source)
    }
}

impl From<StateStoreError> for ExecutionError {
    fn from(source: StateStoreError) -> Self {
        Self::State(source)
    }
}
