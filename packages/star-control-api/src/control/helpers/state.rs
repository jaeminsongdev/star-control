use super::body::string_field;
use super::time::timestamp_string;
use serde_json::{json, Value};
use star_control_state::{StateStore, StateStoreError};

pub(in crate::control) fn state_string(state: &Value) -> String {
    string_field(state, "state").unwrap_or("FAILED").to_string()
}

pub(in crate::control) fn state_after_approval_response(response: &str) -> &'static str {
    match response {
        "approved" => "WAITING_APPROVAL",
        "cancelled" => "CANCELLED",
        _ => "BLOCKED",
    }
}

pub(in crate::control) fn next_action_after_approval_response(response: &str) -> &'static str {
    match response {
        "approved" => "resume",
        "cancelled" => "stop",
        "needs_changes" => "revise",
        _ => "stop",
    }
}

pub(in crate::control) fn allowed_next_stage_for(stage: &str) -> Option<&'static str> {
    match stage {
        "route" => Some("plan"),
        "plan" => Some("design"),
        "design" => Some("implement"),
        "implement" => Some("validate"),
        "validate" => Some("report"),
        "review" => Some("polish"),
        "polish" => Some("report"),
        _ => None,
    }
}

pub(in crate::control) fn ensure_approval_response_matches_request(
    approval_request: &Value,
    approval_response: &Value,
) -> Result<(), String> {
    for field in ["job_id", "stage", "task_id"] {
        let expected = string_field(approval_request, field)
            .ok_or_else(|| format!("approval request missing {}", field))?;
        let actual = string_field(approval_response, field)
            .ok_or_else(|| format!("approval response missing {}", field))?;
        if expected != actual {
            return Err(format!(
                "approval response {} mismatch: expected {}, got {}",
                field, expected, actual
            ));
        }
    }
    let response = string_field(approval_response, "response")
        .ok_or_else(|| "approval response missing response".to_string())?;
    if response != "approved" {
        return Err(format!(
            "resume requires approved response, got {}",
            response
        ));
    }
    Ok(())
}

pub(in crate::control) fn update_state_for_control_command(
    state: &mut Value,
    store: &StateStore,
    next_state: &str,
    current_stage: &str,
    next_action: &str,
    latest_event_id: &str,
    artifact_ref: Option<(&str, &Value)>,
) -> Result<(), StateStoreError> {
    if let Some(state_object) = state.as_object_mut() {
        state_object.insert("state".to_string(), Value::String(next_state.to_string()));
        state_object.insert(
            "current_stage".to_string(),
            Value::String(current_stage.to_string()),
        );
        state_object.insert("updated_at".to_string(), Value::String(timestamp_string()));
        state_object.insert(
            "latest_event_id".to_string(),
            Value::String(latest_event_id.to_string()),
        );
        state_object.insert(
            "next_action".to_string(),
            Value::String(next_action.to_string()),
        );
        let history = state_object
            .entry("history")
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(history) = history.as_array_mut() {
            history.push(json!({
                "stage": current_stage,
                "state": next_state,
                "next_action": next_action,
                "event_id": latest_event_id
            }));
        } else {
            state_object.insert(
                "history".to_string(),
                json!([{
                    "stage": current_stage,
                    "state": next_state,
                    "next_action": next_action,
                    "event_id": latest_event_id
                }]),
            );
        }
    } else {
        return Err(StateStoreError::InvalidArtifactShape {
            message: "RunState must be a JSON object".to_string(),
        });
    }
    if let Some((key, artifact_ref)) = artifact_ref {
        store.register_artifact_ref(state, key, artifact_ref)?;
    }
    Ok(())
}
