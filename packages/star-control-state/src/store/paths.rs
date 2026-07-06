use crate::paths::{resolve_inside_job, validate_safe_name};
use crate::{StateStore, StateStoreError};
use std::path::PathBuf;

impl StateStore {
    pub fn resolve_job_path(
        &self,
        job_id: &str,
        relative_path: &str,
    ) -> Result<PathBuf, StateStoreError> {
        let job_dir = self.job_dir(job_id)?;
        resolve_inside_job(&job_dir, relative_path)
    }

    pub fn resolve_provider_output_dir(
        &self,
        job_id: &str,
        provider_instance_id: &str,
    ) -> Result<PathBuf, StateStoreError> {
        validate_safe_name(provider_instance_id)?;
        self.resolve_job_path(job_id, &format!("provider-output/{}", provider_instance_id))
    }

    pub fn resolve_tool_output_dir(
        &self,
        job_id: &str,
        tool_output_dir: &str,
    ) -> Result<PathBuf, StateStoreError> {
        validate_safe_name(tool_output_dir)?;
        self.resolve_job_path(job_id, &format!("tool-output/{}", tool_output_dir))
    }
}
