use super::super::{ALLOWED_NEXT_STAGES, STATE_FAILED};
use super::time::timestamp_string;
use crate::error::CliError;
use serde_json::{json, Value};
use star_control_state::StateStore;

pub(in crate::control) fn state_string(state: &Value) -> String {
    state
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or(STATE_FAILED)
        .to_string()
}

pub(in crate::control) fn allowed_next_stage_for(stage: &str) -> Option<&'static str> {
    ALLOWED_NEXT_STAGES
        .iter()
        .find_map(|(current, next)| (*current == stage).then_some(*next))
}

pub(in crate::control) fn update_state_for_control_command(
    state: &mut Value,
    store: &StateStore,
    next_state: &str,
    current_stage: &str,
    next_action: &str,
    latest_event_id: &str,
    artifact_ref: Option<(&str, &Value)>,
) -> Result<(), CliError> {
    {
        let Some(state_object) = state.as_object_mut() else {
            return Err(CliError::Internal {
                command: "control".to_string(),
                message: "RunState must be a JSON object".to_string(),
            });
        };
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
        let Some(history) = history.as_array_mut() else {
            return Err(CliError::Internal {
                command: "control".to_string(),
                message: "RunState history must be an array".to_string(),
            });
        };
        history.push(json!({
            "stage": current_stage,
            "state": next_state,
            "next_action": next_action,
            "event_id": latest_event_id
        }));
    }
    if let Some((key, artifact_ref)) = artifact_ref {
        store
            .register_artifact_ref(state, key, artifact_ref)
            .map_err(|source| CliError::State {
                command: "control".to_string(),
                source,
            })?;
    }
    Ok(())
}
