use crate::artifacts::{diagnostics_array, has_block_diagnostic, required_string, string_array};
use crate::constants::SENTINEL_APPROVAL_PATH;
use crate::error::ValidationEngineError;
use serde_json::{json, Value};
use std::path::Path;

pub(super) struct SentinelApproval {
    pub(super) task_id: String,
    pub(super) decision: String,
    pub(super) reasons: Vec<String>,
    pub(super) diagnostics: Value,
}

impl SentinelApproval {
    pub(super) fn from_value(approval: &Value) -> Result<Self, ValidationEngineError> {
        let approval_path = Path::new(SENTINEL_APPROVAL_PATH);
        Ok(Self {
            task_id: required_string(approval, approval_path, "task_id")?.to_string(),
            decision: required_string(approval, approval_path, "decision")?.to_string(),
            reasons: string_array(approval, approval_path, "reasons")?,
            diagnostics: approval
                .get("diagnostics")
                .cloned()
                .unwrap_or_else(|| json!([])),
        })
    }

    pub(super) fn task_mismatch_diagnostic(&self, expected_task_id: &str) -> Option<Value> {
        if self.task_id == expected_task_id {
            return None;
        }

        Some(json!({
            "rule_id": "star-sentinel.output.task_mismatch",
            "severity": "block",
            "message": format!(
                "approval task_id {} did not match expected {}",
                self.task_id,
                expected_task_id
            )
        }))
    }

    pub(super) fn inconsistent_auto_pass_diagnostics(&self) -> Option<Vec<Value>> {
        if self.decision != "AUTO_PASS" || !has_block_diagnostic(&self.diagnostics) {
            return None;
        }

        let mut failed_diagnostics = diagnostics_array(&self.diagnostics);
        failed_diagnostics.push(json!({
            "rule_id": "star-sentinel.output.inconsistent",
            "severity": "block",
            "message": "approval decision AUTO_PASS included a block diagnostic"
        }));
        Some(failed_diagnostics)
    }
}
