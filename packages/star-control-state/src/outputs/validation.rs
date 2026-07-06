use crate::paths::validate_safe_name;
use crate::{ArtifactKind, StateStore, StateStoreError};
use serde_json::Value;

impl StateStore {
    pub fn write_validation_json(
        &self,
        job_id: &str,
        file_name: &str,
        value: &Value,
    ) -> Result<Value, StateStoreError> {
        validate_safe_name(file_name)?;
        let relative_path = format!("validation/{}", file_name);
        self.write_new_json_artifact(job_id, &relative_path, value)?;
        self.artifact_ref(
            job_id,
            &relative_path,
            ArtifactKind::Other,
            "validation-engine",
            None,
            Some("validation JSON artifact"),
        )
    }
}
