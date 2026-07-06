use crate::constants::APPROVAL_RESPONSE_SCHEMA;
use crate::error::DaemonError;
use crate::queue::DaemonQueue;
use serde_json::Value;
use star_control_state::StateStore;
use std::fs;

impl DaemonQueue {
    pub(crate) fn ensure_approved_response(
        &self,
        project_store: &StateStore,
        job_id: &str,
    ) -> Result<(), DaemonError> {
        let response_path =
            project_store.resolve_job_path(job_id, "approvals/approval-response.json")?;
        if !response_path.is_file() {
            return Err(DaemonError::ApprovalRequired {
                job_id: job_id.to_string(),
                path: response_path,
            });
        }
        let content =
            fs::read_to_string(&response_path).map_err(|source| DaemonError::StateReadFailed {
                path: response_path.clone(),
                source,
            })?;
        let response: Value =
            serde_json::from_str(&content).map_err(|source| DaemonError::InvalidJson {
                path: response_path.clone(),
                source,
            })?;
        self.validate_schema(APPROVAL_RESPONSE_SCHEMA, &response_path, &response)?;
        let actual_job_id = response
            .get("job_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if actual_job_id != job_id {
            return Err(DaemonError::ApprovalJobMismatch {
                expected: job_id.to_string(),
                actual: actual_job_id.to_string(),
            });
        }
        let response_value = response
            .get("response")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if response_value != "approved" {
            return Err(DaemonError::ApprovalResponseNotApproved {
                job_id: job_id.to_string(),
                response: response_value.to_string(),
            });
        }
        Ok(())
    }
}
