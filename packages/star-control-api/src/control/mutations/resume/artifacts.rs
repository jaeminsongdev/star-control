use crate::artifacts::{load_job_json, ControlArtifactError};
use crate::constants::{APPROVAL_REQUEST_SCHEMA, APPROVAL_RESPONSE_SCHEMA};
use serde_json::Value;
use star_control_state::StateStore;
use std::path::Path;

const APPROVAL_REQUEST_RELATIVE_PATH: &str = "approvals/approval-request.json";
const APPROVAL_RESPONSE_RELATIVE_PATH: &str = "approvals/approval-response.json";
const DEFAULT_RESUME_NEXT_ACTION: &str = "report";

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

pub(super) fn load_approval_response(
    store: &StateStore,
    job_id: &str,
    schema_root: &Path,
) -> Result<Value, ControlArtifactError> {
    load_job_json(
        store,
        job_id,
        APPROVAL_RESPONSE_RELATIVE_PATH,
        APPROVAL_RESPONSE_SCHEMA,
        schema_root,
    )
}

pub(super) fn next_action_from_approval_response(approval_response: &Value) -> &str {
    approval_response
        .get("allowed_next_stage")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_RESUME_NEXT_ACTION)
}
