use super::constants::{
    BLOCKED_STATE, BLOCKED_STATUS, DONE_STATE, ERROR_STATUS, FAILED_STATE, IMPLEMENT_STAGE,
    NEXT_ACTION_RUN, NEXT_ACTION_STATUS, ROUTED_STATE, SUCCESS_STATUS,
};
use crate::constants::SCHEMA_VERSION;
use serde_json::{json, Value};

pub(super) fn routed_state(job_id: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id,
        "state": ROUTED_STATE,
        "current_stage": IMPLEMENT_STAGE,
        "updated_at": "cli:dry-run",
        "threads": {},
        "workers": {},
        "artifacts": {},
        "latest_event_id": "",
        "active_provider": null,
        "next_action": NEXT_ACTION_RUN,
        "budget": {},
        "history": []
    })
}

pub(super) fn report_from_provider_result(result: &Value) -> Value {
    let provider_status = result
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or(ERROR_STATUS);
    let report_status = match provider_status {
        SUCCESS_STATUS => DONE_STATE,
        BLOCKED_STATUS => BLOCKED_STATE,
        _ => FAILED_STATE,
    };
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": result.get("job_id").cloned().unwrap_or_else(|| json!("J-0000")),
        "stage": result.get("stage").cloned().unwrap_or_else(|| json!(IMPLEMENT_STAGE)),
        "status": report_status,
        "changed_files": result.get("changed_files").cloned().unwrap_or_else(|| json!([])),
        "commands_run": [],
        "validation": [],
        "risks": [],
        "blocked_reason": if provider_status == BLOCKED_STATUS {
            result.pointer("/error/message").cloned().unwrap_or_else(|| json!(BLOCKED_STATUS))
        } else {
            Value::Null
        },
        "next_step": NEXT_ACTION_STATUS,
        "artifacts": result.get("artifacts").cloned().unwrap_or_else(|| json!([]))
    })
}
