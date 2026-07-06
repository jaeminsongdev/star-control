use crate::paths::validate_safe_name;
use crate::{ArtifactKind, StateStore, StateStoreError};
use serde_json::Value;

impl StateStore {
    pub fn write_review_pack_json(
        &self,
        job_id: &str,
        file_name: &str,
        value: &Value,
    ) -> Result<Value, StateStoreError> {
        validate_safe_name(file_name)?;
        let relative_path = format!("review-packs/{}", file_name);
        self.write_new_json_artifact(job_id, &relative_path, value)?;
        self.artifact_ref(
            job_id,
            &relative_path,
            ArtifactKind::ReviewPack,
            "state-store",
            None,
            Some("review pack JSON artifact"),
        )
    }

    pub fn write_review_pack_markdown(
        &self,
        job_id: &str,
        file_name: &str,
        content: &str,
    ) -> Result<Value, StateStoreError> {
        validate_safe_name(file_name)?;
        let relative_path = format!("review-packs/{}", file_name);
        self.write_new_text_artifact(job_id, &relative_path, content)?;
        self.artifact_ref(
            job_id,
            &relative_path,
            ArtifactKind::ReviewPack,
            "state-store",
            None,
            Some("review pack Markdown artifact"),
        )
    }
}
