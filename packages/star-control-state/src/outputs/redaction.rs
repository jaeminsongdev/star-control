use crate::artifacts::CoreSchema;
use crate::paths::validate_safe_name;
use crate::{ArtifactKind, StateStore, StateStoreError};
use serde_json::Value;

const REDACTION_REPORT_SCHEMA: &str = "specs/schemas/redaction-report.schema.json";

impl StateStore {
    pub fn write_redaction_report_json(
        &self,
        job_id: &str,
        file_name: &str,
        value: &Value,
    ) -> Result<Value, StateStoreError> {
        validate_safe_name(file_name)?;
        let relative_path = format!("audit/{}", file_name);
        let target_path = self.resolve_job_path(job_id, &relative_path)?;
        self.validate_artifact(CoreSchema::RedactionReport, target_path, value)?;
        self.write_new_json_artifact(job_id, &relative_path, value)?;
        self.artifact_ref(
            job_id,
            &relative_path,
            ArtifactKind::Other,
            "star-control-security",
            Some(REDACTION_REPORT_SCHEMA),
            Some("RedactionReport artifact"),
        )
    }
}
