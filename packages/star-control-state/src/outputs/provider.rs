use crate::paths::validate_safe_name;
use crate::{ArtifactKind, StateStore, StateStoreError};
use serde_json::Value;

impl StateStore {
    pub fn write_provider_json(
        &self,
        job_id: &str,
        provider_instance_id: &str,
        file_name: &str,
        value: &Value,
    ) -> Result<Value, StateStoreError> {
        validate_safe_name(provider_instance_id)?;
        validate_safe_name(file_name)?;
        let relative_path = format!("provider-output/{}/{}", provider_instance_id, file_name);
        self.write_new_json_artifact(job_id, &relative_path, value)?;
        self.artifact_ref(
            job_id,
            &relative_path,
            ArtifactKind::ProviderOutput,
            provider_instance_id,
            None,
            Some("provider JSON output"),
        )
    }

    pub fn write_provider_text(
        &self,
        job_id: &str,
        provider_instance_id: &str,
        file_name: &str,
        content: &str,
    ) -> Result<Value, StateStoreError> {
        validate_safe_name(provider_instance_id)?;
        validate_safe_name(file_name)?;
        let relative_path = format!("provider-output/{}/{}", provider_instance_id, file_name);
        self.write_new_text_artifact(job_id, &relative_path, content)?;
        self.artifact_ref(
            job_id,
            &relative_path,
            ArtifactKind::Log,
            provider_instance_id,
            None,
            Some("provider text output"),
        )
    }
}
