use crate::StateStoreError;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryIssue {
    pub artifact_path: String,
    pub kind: String,
    pub severity: String,
    pub message: String,
    pub recommended_action: String,
}

impl RecoveryIssue {
    pub fn new(
        artifact_path: impl Into<String>,
        kind: impl Into<String>,
        severity: impl Into<String>,
        message: impl Into<String>,
        recommended_action: impl Into<String>,
    ) -> Self {
        Self {
            artifact_path: artifact_path.into(),
            kind: kind.into(),
            severity: severity.into(),
            message: message.into(),
            recommended_action: recommended_action.into(),
        }
    }

    pub fn to_value(&self) -> Value {
        json!({
            "artifact_path": self.artifact_path,
            "kind": self.kind,
            "severity": self.severity,
            "message": self.message,
            "recommended_action": self.recommended_action
        })
    }
}

pub(super) fn recovery_issue_from_error(
    relative_path: &str,
    error: &StateStoreError,
) -> RecoveryIssue {
    match error {
        StateStoreError::ArtifactNotFound { .. } => RecoveryIssue::new(
            relative_path,
            "missing_required_file",
            "block",
            "required artifact is missing",
            "inspect the job and recreate only through an explicit recovery command",
        ),
        StateStoreError::InvalidJson { .. } => RecoveryIssue::new(
            relative_path,
            "invalid_json",
            "block",
            "artifact is not valid JSON",
            "preserve the original artifact and prepare a replacement through an explicit recovery command",
        ),
        StateStoreError::SchemaValidationFailed { errors, .. } => RecoveryIssue::new(
            relative_path,
            "schema_mismatch",
            "block",
            format!("artifact failed schema validation with {} error(s)", errors.len()),
            "inspect schema errors and write a corrected artifact only through an explicit recovery command",
        ),
        StateStoreError::CorruptEventLog { line, .. } => RecoveryIssue::new(
            relative_path,
            "corrupt_event_log",
            "block",
            format!("event log contains an invalid line at {}", line),
            "preserve the original log and create a recovered copy before replacing anything",
        ),
        StateStoreError::PathTraversalBlocked { .. }
        | StateStoreError::PathOutsideJobDirectory { .. } => RecoveryIssue::new(
            relative_path,
            "path_violation",
            "block",
            "artifact path violates job directory containment",
            "reject the recovery input and inspect the caller-provided path",
        ),
        _ => RecoveryIssue::new(
            relative_path,
            "inspection_failed",
            "block",
            "artifact inspection failed",
            "inspect the job manually before attempting recovery",
        ),
    }
}
