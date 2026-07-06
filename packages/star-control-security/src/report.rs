use crate::constants::{REDACTION_PLACEHOLDER, SCHEMA_VERSION};
use crate::model::RedactionFinding;
use serde_json::{json, Value};

pub fn redaction_report(job_id: &str, artifact_path: &str, findings: &[RedactionFinding]) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id,
        "artifact_path": artifact_path,
        "redacted": !findings.is_empty(),
        "placeholder": REDACTION_PLACEHOLDER,
        "findings": findings
            .iter()
            .map(RedactionFinding::to_json)
            .collect::<Vec<_>>()
    })
}
