use super::schema::CoreSchema;
use crate::{StateStore, StateStoreError};
use serde_json::Value;
use std::fs;

impl StateStore {
    pub(crate) fn read_json_artifact(
        &self,
        job_id: &str,
        relative_path: &str,
        schema: CoreSchema,
    ) -> Result<Value, StateStoreError> {
        let path = self.resolve_job_path(job_id, relative_path)?;
        if !path.is_file() {
            return Err(StateStoreError::ArtifactNotFound { path });
        }
        let content =
            fs::read_to_string(&path).map_err(|source| StateStoreError::AtomicWriteFailed {
                path: path.clone(),
                source,
            })?;
        let value: Value =
            serde_json::from_str(&content).map_err(|source| StateStoreError::InvalidJson {
                path: path.clone(),
                source,
            })?;
        self.validate_artifact(schema, path, &value)?;
        Ok(value)
    }

    pub(crate) fn write_json_artifact(
        &self,
        job_id: &str,
        relative_path: &str,
        schema: CoreSchema,
        value: &Value,
    ) -> Result<(), StateStoreError> {
        let target_path = self.resolve_job_path(job_id, relative_path)?;
        self.validate_artifact(schema, target_path.clone(), value)?;
        self.write_json_value_atomic(job_id, relative_path, value)
    }

    pub(crate) fn write_new_json_artifact(
        &self,
        job_id: &str,
        relative_path: &str,
        value: &Value,
    ) -> Result<(), StateStoreError> {
        let target_path = self.resolve_job_path(job_id, relative_path)?;
        if target_path.exists() {
            return Err(StateStoreError::ArtifactAlreadyExists { path: target_path });
        }
        self.write_json_value_atomic(job_id, relative_path, value)
    }

    pub(crate) fn write_json_value_atomic(
        &self,
        job_id: &str,
        relative_path: &str,
        value: &Value,
    ) -> Result<(), StateStoreError> {
        let target_path = self.resolve_job_path(job_id, relative_path)?;
        let mut bytes =
            serde_json::to_vec_pretty(value).map_err(|source| StateStoreError::InvalidJson {
                path: target_path.clone(),
                source,
            })?;
        bytes.push(b'\n');
        self.write_bytes_atomic(job_id, &target_path, &bytes)
    }

    pub(crate) fn write_new_text_artifact(
        &self,
        job_id: &str,
        relative_path: &str,
        content: &str,
    ) -> Result<(), StateStoreError> {
        let target_path = self.resolve_job_path(job_id, relative_path)?;
        if target_path.exists() {
            return Err(StateStoreError::ArtifactAlreadyExists { path: target_path });
        }
        self.write_bytes_atomic(job_id, &target_path, content.as_bytes())
    }
}
