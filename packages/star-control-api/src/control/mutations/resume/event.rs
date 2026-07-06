use super::super::super::helpers::ApiControlEvent;
use serde_json::{json, Value};

const RESUME_EVENT_TYPE: &str = "STATE_CHANGED";
const RESUME_NEXT_STATE: &str = "VALIDATED";
const RESUME_MESSAGE: &str = "Approval accepted; job is ready to continue";
const RUN_STATE_RELATIVE_PATH: &str = "run-state.json";
const APPROVAL_RESPONSE_RELATIVE_PATH: &str = "approvals/approval-response.json";

pub(super) fn resume_event_id(job_id: &str) -> String {
    format!("{}-api-resumed", job_id.to_ascii_lowercase())
}

pub(super) fn resume_skipped_payload(
    job_id: &str,
    current_state: &str,
    current_stage: &str,
    state: &Value,
) -> Value {
    json!({
        "command": "resume",
        "job_id": job_id,
        "state": current_state,
        "current_stage": current_stage,
        "next_action": state.get("next_action").cloned().unwrap_or_else(|| json!("")),
        "resumed": false,
        "artifacts": [format!(".ai-runs/{}/{}", job_id, RUN_STATE_RELATIVE_PATH)]
    })
}

pub(super) fn resume_state_changed_event<'a>(
    event_id: String,
    previous_state: &'a str,
    current_stage: &'a str,
    next_action: &'a str,
) -> ApiControlEvent<'a> {
    ApiControlEvent {
        event_id,
        event_type: RESUME_EVENT_TYPE,
        state: RESUME_NEXT_STATE,
        stage: current_stage,
        message: RESUME_MESSAGE,
        artifact_paths: vec![
            RUN_STATE_RELATIVE_PATH.to_string(),
            APPROVAL_RESPONSE_RELATIVE_PATH.to_string(),
        ],
        details: json!({ "previous_state": previous_state, "next_action": next_action }),
    }
}

pub(super) fn resume_success_payload(
    job_id: &str,
    previous_state: &str,
    next_action: &str,
) -> Value {
    json!({
        "command": "resume",
        "job_id": job_id,
        "state": RESUME_NEXT_STATE,
        "previous_state": previous_state,
        "next_action": next_action,
        "resumed": true,
        "artifacts": [
            format!(".ai-runs/{}/{}", job_id, RUN_STATE_RELATIVE_PATH),
            format!(".ai-runs/{}/{}", job_id, APPROVAL_RESPONSE_RELATIVE_PATH)
        ]
    })
}
