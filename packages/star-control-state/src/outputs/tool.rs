use crate::paths::validate_safe_name;
use crate::{ArtifactKind, StateStore, StateStoreError};
use serde_json::Value;

impl StateStore {
    pub fn write_tool_json(
        &self,
        job_id: &str,
        tool_output_dir: &str,
        file_name: &str,
        value: &Value,
    ) -> Result<Value, StateStoreError> {
        validate_safe_name(tool_output_dir)?;
        validate_safe_name(file_name)?;
        let relative_path = format!("tool-output/{}/{}", tool_output_dir, file_name);
        self.write_new_json_artifact(job_id, &relative_path, value)?;
        self.artifact_ref(
            job_id,
            &relative_path,
            ArtifactKind::ToolOutput,
            tool_output_dir,
            None,
            Some("tool JSON output"),
        )
    }

    pub fn write_tool_text(
        &self,
        job_id: &str,
        tool_output_dir: &str,
        file_name: &str,
        content: &str,
    ) -> Result<Value, StateStoreError> {
        validate_safe_name(tool_output_dir)?;
        validate_safe_name(file_name)?;
        let relative_path = format!("tool-output/{}/{}", tool_output_dir, file_name);
        self.write_new_text_artifact(job_id, &relative_path, content)?;
        self.artifact_ref(
            job_id,
            &relative_path,
            ArtifactKind::ToolOutput,
            tool_output_dir,
            None,
            Some("tool text output"),
        )
    }
}
