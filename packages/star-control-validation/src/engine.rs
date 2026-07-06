mod approval;
mod gate;
mod provider;
mod writer;

use crate::artifacts::validate_schema_value;
use crate::error::ValidationEngineError;
use serde_json::Value;
use star_control_state::StateStore;
use std::path::{Path, PathBuf};

pub struct ValidationEngine<'a> {
    state_store: &'a StateStore,
    core_schema_root: PathBuf,
    sentinel_schema_root: PathBuf,
}

impl<'a> ValidationEngine<'a> {
    pub fn new(
        state_store: &'a StateStore,
        core_schema_root: impl AsRef<Path>,
        sentinel_schema_root: impl AsRef<Path>,
    ) -> Self {
        Self {
            state_store,
            core_schema_root: core_schema_root.as_ref().to_path_buf(),
            sentinel_schema_root: sentinel_schema_root.as_ref().to_path_buf(),
        }
    }

    fn validate_core_schema(
        &self,
        value: &Value,
        schema_file: &str,
        relative_path: &str,
    ) -> Result<(), ValidationEngineError> {
        validate_schema_value(value, &self.core_schema_root, schema_file, relative_path)
    }

    fn validate_sentinel_schema(
        &self,
        value: &Value,
        schema_file: &str,
        relative_path: &str,
    ) -> Result<(), ValidationEngineError> {
        validate_schema_value(
            value,
            &self.sentinel_schema_root,
            schema_file,
            relative_path,
        )
    }
}
