use super::super::super::helpers::{allowed_next_stage_for, string_field, timestamp_string};
use super::request::ApprovalDecision;
use crate::artifacts::{load_job_json, validate_schema_value, ControlArtifactError};
use crate::constants::{APPROVAL_REQUEST_SCHEMA, APPROVAL_RESPONSE_SCHEMA, SCHEMA_VERSION};
use serde_json::{json, Value};
use star_control_state::{StateStore, StateStoreError};
use std::path::Path;

pub(super) const APPROVAL_RESPONSE_RELATIVE_PATH: &str = "approvals/approval-response.json";

const APPROVAL_REQUEST_RELATIVE_PATH: &str = "approvals/approval-request.json";
const APPROVAL_RESPONSE_FILE_NAME: &str = "approval-response.json";
const DEFAULT_APPROVAL_STAGE: &str = "validate";
const DEFAULT_APPROVAL_TASK_ID: &str = "approval";

#[derive(Debug, Clone)]
pub(super) struct ApprovalMetadata {
    stage: String,
    task_id: String,
}

impl ApprovalMetadata {
    pub(super) fn stage(&self) -> &str {
        &self.stage
    }

    fn task_id(&self) -> &str {
        &self.task_id
    }
}

pub(super) fn load_approval_request(
    store: &StateStore,
    job_id: &str,
    schema_root: &Path,
) -> Result<Value, ControlArtifactError> {
    load_job_json(
        store,
        job_id,
        APPROVAL_REQUEST_RELATIVE_PATH,
        APPROVAL_REQUEST_SCHEMA,
        schema_root,
    )
}

pub(super) fn approval_metadata(approval_request: &Value) -> ApprovalMetadata {
    ApprovalMetadata {
        stage: string_field(approval_request, "stage")
            .unwrap_or(DEFAULT_APPROVAL_STAGE)
            .to_string(),
        task_id: string_field(approval_request, "task_id")
            .unwrap_or(DEFAULT_APPROVAL_TASK_ID)
            .to_string(),
    }
}

pub(super) fn build_approval_response(
    job_id: &str,
    metadata: &ApprovalMetadata,
    decision: &ApprovalDecision,
) -> Value {
    let allowed_next_stage = (decision.response() == "approved")
        .then(|| allowed_next_stage_for(metadata.stage()))
        .flatten();
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id,
        "stage": metadata.stage(),
        "task_id": metadata.task_id(),
        "response": decision.response(),
        "reviewer": decision.reviewer(),
        "responded_at": timestamp_string(),
        "reason": decision.reason(),
        "allowed_next_stage": allowed_next_stage,
        "constraints": decision.constraints()
    })
}

pub(super) fn validate_approval_response(value: &Value, schema_root: &Path) -> Result<(), usize> {
    validate_schema_value(value, schema_root, APPROVAL_RESPONSE_SCHEMA)
}

pub(super) fn write_approval_response(
    store: &StateStore,
    job_id: &str,
    value: &Value,
) -> Result<Value, StateStoreError> {
    store.write_approval_json(job_id, APPROVAL_RESPONSE_FILE_NAME, value)
}
