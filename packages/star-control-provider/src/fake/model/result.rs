use super::error::ProviderAdapterError;
use super::validation::{required_string, validate_contract, PROVIDER_RUN_RESULT_SCHEMA};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderRunResult {
    provider_instance_id: String,
    job_id: String,
    stage: String,
    status: String,
    value: Value,
}

impl ProviderRunResult {
    pub fn from_value(
        value: Value,
        source_path: impl Into<PathBuf>,
        schema_root: impl AsRef<Path>,
    ) -> Result<Self, ProviderAdapterError> {
        let source_path = source_path.into();
        validate_contract(
            &value,
            &source_path,
            schema_root.as_ref(),
            PROVIDER_RUN_RESULT_SCHEMA,
        )?;

        Ok(Self {
            provider_instance_id: required_string(&value, &source_path, "provider_instance_id")?,
            job_id: required_string(&value, &source_path, "job_id")?,
            stage: required_string(&value, &source_path, "stage")?,
            status: required_string(&value, &source_path, "status")?,
            value,
        })
    }

    pub fn provider_instance_id(&self) -> &str {
        &self.provider_instance_id
    }

    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    pub fn stage(&self) -> &str {
        &self.stage
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}
