use crate::artifacts::timestamp_nanos;
use crate::paths::validate_safe_name;
use crate::{StateStore, StateStoreError};
use serde_json::Value;

impl StateStore {
    pub fn write_tmp_json(
        &self,
        job_id: &str,
        target_name: &str,
        value: &Value,
    ) -> Result<String, StateStoreError> {
        validate_safe_name(target_name)?;
        let relative_path = format!(
            "tmp/{}.tmp-{}-{}",
            target_name,
            std::process::id(),
            timestamp_nanos()
        );
        self.write_new_json_artifact(job_id, &relative_path, value)?;
        Ok(relative_path)
    }
}
