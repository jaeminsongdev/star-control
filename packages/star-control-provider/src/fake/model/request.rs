use super::error::ProviderAdapterError;
use super::validation::{required_string, validate_contract, EXECUTION_REQUEST_SCHEMA};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionRequest {
    request_id: String,
    job_id: String,
    stage: String,
    provider_instance_id: String,
    workspec_path: String,
    created_at: String,
    goal: String,
    value: Value,
}

impl ExecutionRequest {
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
            EXECUTION_REQUEST_SCHEMA,
        )?;

        Ok(Self {
            request_id: required_string(&value, &source_path, "request_id")?,
            job_id: required_string(&value, &source_path, "job_id")?,
            stage: required_string(&value, &source_path, "stage")?,
            provider_instance_id: required_string(&value, &source_path, "provider_instance_id")?,
            workspec_path: required_string(&value, &source_path, "workspec_path")?,
            created_at: required_string(&value, &source_path, "created_at")?,
            goal: required_string(&value, &source_path, "goal")?,
            value,
        })
    }

    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    pub fn stage(&self) -> &str {
        &self.stage
    }

    pub fn provider_instance_id(&self) -> &str {
        &self.provider_instance_id
    }

    pub fn workspec_path(&self) -> &str {
        &self.workspec_path
    }

    pub fn created_at(&self) -> &str {
        &self.created_at
    }

    pub fn goal(&self) -> &str {
        &self.goal
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}

pub fn load_execution_request(
    path: impl AsRef<Path>,
    schema_root: impl AsRef<Path>,
) -> Result<ExecutionRequest, ProviderAdapterError> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|source| ProviderAdapterError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let value: Value =
        serde_json::from_str(&content).map_err(|source| ProviderAdapterError::InvalidJson {
            path: path.to_path_buf(),
            source,
        })?;
    ExecutionRequest::from_value(value, path.to_path_buf(), schema_root)
}
