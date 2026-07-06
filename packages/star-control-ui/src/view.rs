use crate::constants::SCHEMA_VERSION;
use crate::error::UiError;
use crate::helpers::{invalid_data, string_field};
use serde_json::{json, Value};

mod approval;
mod artifacts;
mod state;

pub(crate) use approval::approval_summary;
pub(crate) use artifacts::{artifact_sections, paths_for_section};
pub(crate) use state::{latest_event_id, next_action_for_state, state_is_waiting_approval};

use artifacts::artifact_paths;

pub(crate) fn job_summary_view(summary: &Value) -> Result<Value, UiError> {
    let endpoint = "job summary";
    let job_id = string_field(summary, "job_id")
        .ok_or_else(|| invalid_data(endpoint, "job_id is missing"))?;
    let state = string_field(summary, "state").unwrap_or("UNKNOWN");
    let current_stage = string_field(summary, "current_stage").unwrap_or("unknown");
    let title = string_field(summary, "summary")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(job_id);
    Ok(json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id,
        "title": title,
        "state": state,
        "current_stage": current_stage,
        "approval_required": state == "WAITING_APPROVAL",
        "next_action": next_action_for_state(state),
        "latest_event": Value::Null,
        "artifacts": []
    }))
}

pub(crate) fn job_detail_view(
    job: &Value,
    state: &Value,
    latest_event: &Value,
) -> Result<Value, UiError> {
    let endpoint = "job detail";
    let job_id = string_field(state, "job_id")
        .or_else(|| string_field(job, "job_id"))
        .ok_or_else(|| invalid_data(endpoint, "job_id is missing"))?;
    let state_value = string_field(state, "state").unwrap_or("UNKNOWN");
    let current_stage = string_field(state, "current_stage").unwrap_or("unknown");
    let next_action = string_field(state, "next_action").unwrap_or_else(|| {
        if state_value == "WAITING_APPROVAL" {
            "approve"
        } else {
            next_action_for_state(state_value)
        }
    });
    let title = string_field(job, "request_text")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(job_id);
    let paths = artifact_paths(state.get("artifacts").unwrap_or(&Value::Null));
    Ok(json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id,
        "title": title,
        "state": state_value,
        "current_stage": current_stage,
        "approval_required": state_is_waiting_approval(state) || next_action == "approve",
        "next_action": next_action,
        "latest_event": latest_event_id(latest_event, state).map(Value::String).unwrap_or(Value::Null),
        "artifacts": paths
    }))
}
