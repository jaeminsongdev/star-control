use super::ValidationEngine;
use crate::artifacts::{ensure_response_field_matches, read_json_file, required_string};
use crate::constants::{APPROVAL_RESPONSE_PATH, APPROVAL_RESPONSE_SCHEMA};
use crate::error::ValidationEngineError;
use crate::types::ValidationContext;
use serde_json::Value;
use std::path::Path;

impl<'a> ValidationEngine<'a> {
    pub fn ensure_approval_response_allows_next_stage(
        &self,
        context: &ValidationContext,
    ) -> Result<Value, ValidationEngineError> {
        let path = self
            .state_store
            .resolve_job_path(context.job_id(), APPROVAL_RESPONSE_PATH)?;
        if !path.is_file() {
            return Err(ValidationEngineError::ApprovalResponseMissing { path });
        }
        let response = read_json_file(&path)?;
        self.validate_core_schema(&response, APPROVAL_RESPONSE_SCHEMA, APPROVAL_RESPONSE_PATH)?;
        ensure_response_field_matches(&response, "job_id", context.job_id())?;
        ensure_response_field_matches(&response, "stage", context.stage())?;
        ensure_response_field_matches(&response, "task_id", context.task_id())?;
        let response_value =
            required_string(&response, Path::new(APPROVAL_RESPONSE_PATH), "response")?;
        if response_value == "approved" {
            Ok(response)
        } else {
            Err(ValidationEngineError::ApprovalResponseNotApproved {
                response: response_value.to_string(),
            })
        }
    }
}
