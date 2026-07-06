use crate::error::ValidationEngineError;
use serde_json::Value;
use std::path::PathBuf;

pub(crate) fn next_action_for_state(state: &str) -> &'static str {
    match state {
        "VALIDATED" => "continue",
        "WAITING_APPROVAL" => "await_approval",
        "BLOCKED" => "manual_intervention",
        _ => "inspect_validation_failure",
    }
}

pub(crate) fn set_object_field(
    value: &mut Value,
    field: &str,
    field_value: Value,
) -> Result<(), ValidationEngineError> {
    let Some(object) = value.as_object_mut() else {
        return Err(ValidationEngineError::InvalidFieldType {
            path: PathBuf::from("run-state.json"),
            field: "$".to_string(),
            expected: "object".to_string(),
        });
    };
    object.insert(field.to_string(), field_value);
    Ok(())
}

pub(crate) fn push_history(value: &mut Value, entry: Value) -> Result<(), ValidationEngineError> {
    let Some(object) = value.as_object_mut() else {
        return Err(ValidationEngineError::InvalidFieldType {
            path: PathBuf::from("run-state.json"),
            field: "$".to_string(),
            expected: "object".to_string(),
        });
    };
    let history = object
        .entry("history")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(history_items) = history.as_array_mut() else {
        return Err(ValidationEngineError::InvalidFieldType {
            path: PathBuf::from("run-state.json"),
            field: "history".to_string(),
            expected: "array".to_string(),
        });
    };
    history_items.push(entry);
    Ok(())
}
