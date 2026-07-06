use crate::constants::{CONTROL_TRANSPORT, TERMINAL_STATES};
use crate::view::next_action_for_state;
use serde_json::{json, Value};

pub(crate) fn control_actions(project_id: &str, job_id: &str, state: &Value) -> Vec<Value> {
    let state_value = string_field(state, "state").unwrap_or("UNKNOWN");
    let next_action = string_field(state, "next_action").unwrap_or_else(|| {
        if state_value == "WAITING_APPROVAL" {
            "approve"
        } else {
            next_action_for_state(state_value)
        }
    });
    let terminal = TERMINAL_STATES.contains(&state_value);
    let waiting_approval = state_value == "WAITING_APPROVAL";

    vec![
        json!({
            "id": "approve",
            "label": "Approve",
            "method": "POST",
            "endpoint": format!("/projects/{}/jobs/{}/approve", project_id, job_id),
            "transport": CONTROL_TRANSPORT,
            "enabled": waiting_approval && next_action == "approve",
            "disabled_reason": disabled_reason(waiting_approval && next_action == "approve", "approval response already recorded or job is not waiting for approval"),
            "body_contract": "approval-response.schema.json",
            "response_options": ["approved", "rejected", "needs_changes", "cancelled"],
            "required_fields": ["response", "reason"]
        }),
        json!({
            "id": "cancel",
            "label": "Cancel",
            "method": "POST",
            "endpoint": format!("/projects/{}/jobs/{}/cancel", project_id, job_id),
            "transport": CONTROL_TRANSPORT,
            "enabled": !terminal,
            "disabled_reason": disabled_reason(!terminal, "terminal job cannot be cancelled"),
            "body_contract": Value::Null,
            "response_options": [],
            "required_fields": []
        }),
        json!({
            "id": "resume",
            "label": "Resume",
            "method": "POST",
            "endpoint": format!("/projects/{}/jobs/{}/resume", project_id, job_id),
            "transport": CONTROL_TRANSPORT,
            "enabled": waiting_approval && next_action == "resume",
            "disabled_reason": disabled_reason(waiting_approval && next_action == "resume", "resume requires an approved approval response"),
            "body_contract": Value::Null,
            "response_options": [],
            "required_fields": []
        }),
    ]
}

fn disabled_reason(enabled: bool, reason: &str) -> Value {
    if enabled {
        Value::Null
    } else {
        Value::String(reason.to_string())
    }
}

fn string_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}
