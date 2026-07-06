use super::super::super::helpers::ApiControlEvent;
use super::response::APPROVAL_RESPONSE_RELATIVE_PATH;
use serde_json::{json, Value};

const APPROVAL_RECORDED_EVENT_TYPE: &str = "APPROVAL_RECORDED";
const APPROVAL_RECORDED_MESSAGE: &str = "Approval response recorded by API";

pub(super) fn approval_recorded_event_id(job_id: &str) -> String {
    format!("{}-api-approval-recorded", job_id.to_ascii_lowercase())
}

pub(super) fn approval_recorded_event<'a>(
    event_id: String,
    next_state: &'a str,
    stage: &'a str,
    approval_response: &Value,
) -> ApiControlEvent<'a> {
    ApiControlEvent {
        event_id,
        event_type: APPROVAL_RECORDED_EVENT_TYPE,
        state: next_state,
        stage,
        message: APPROVAL_RECORDED_MESSAGE,
        artifact_paths: vec![APPROVAL_RESPONSE_RELATIVE_PATH.to_string()],
        details: json!({
            "response": approval_response["response"],
            "allowed_next_stage": approval_response["allowed_next_stage"]
        }),
    }
}

pub(super) fn approval_success_payload(
    job_id: &str,
    state: &Value,
    approval_response: &Value,
) -> Value {
    json!({
        "command": "approve",
        "job_id": job_id,
        "state": state["state"],
        "approval_response": approval_response["response"],
        "allowed_next_stage": approval_response["allowed_next_stage"],
        "artifacts": [format!(".ai-runs/{}/{}", job_id, APPROVAL_RESPONSE_RELATIVE_PATH)]
    })
}
