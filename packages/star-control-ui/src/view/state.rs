use crate::helpers::string_field;
use serde_json::Value;

pub(crate) fn state_is_waiting_approval(state: &Value) -> bool {
    string_field(state, "state") == Some("WAITING_APPROVAL")
}

pub(crate) fn latest_event_id(latest_event: &Value, state: &Value) -> Option<String> {
    latest_event
        .get("event_id")
        .and_then(Value::as_str)
        .or_else(|| state.get("latest_event_id").and_then(Value::as_str))
        .map(str::to_string)
}

pub(crate) fn next_action_for_state(state: &str) -> &'static str {
    match state {
        "WAITING_APPROVAL" => "approve",
        "DONE" | "FAILED" | "BLOCKED" | "CANCELLED" => "none",
        "UNKNOWN" => "inspect",
        _ => "inspect",
    }
}
