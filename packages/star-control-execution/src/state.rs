use crate::constants::SCHEMA_VERSION;
use crate::contract::object_type_error_path;
use crate::error::ExecutionError;
use serde_json::{json, Value};

pub(crate) fn state_for_provider_status(stage: &str, status: &str) -> &'static str {
    match status {
        "success" => completed_state_for_stage(stage),
        "blocked" => "BLOCKED",
        "cancelled" => "CANCELLED",
        "failed" | "timeout" | "error" => "FAILED",
        _ => "FAILED",
    }
}

fn completed_state_for_stage(stage: &str) -> &'static str {
    match stage {
        "route" => "ROUTED",
        "plan" => "PLANNED",
        "design" => "PLANNED",
        "implement" => "IMPLEMENTED",
        "validate" => "VALIDATED",
        "review" => "REVIEWED",
        "polish" => "POLISHED",
        "report" => "DONE",
        _ => "DONE",
    }
}

pub(crate) fn initial_state(job_id: &str, stage: &str, created_at: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id,
        "state": "REQUESTED",
        "current_stage": stage,
        "updated_at": created_at,
        "threads": {},
        "workers": {},
        "artifacts": {},
        "latest_event_id": "",
        "active_provider": null,
        "next_action": "continue",
        "budget": {},
        "history": []
    })
}

pub(crate) fn set_object_field(
    value: &mut Value,
    key: &str,
    field_value: Value,
) -> Result<(), ExecutionError> {
    let Some(object) = value.as_object_mut() else {
        return Err(ExecutionError::InvalidFieldType {
            path: object_type_error_path(),
            field: "$".to_string(),
            expected: "object".to_string(),
        });
    };
    object.insert(key.to_string(), field_value);
    Ok(())
}

pub(crate) fn push_history(value: &mut Value, entry: Value) -> Result<(), ExecutionError> {
    let Some(object) = value.as_object_mut() else {
        return Err(ExecutionError::InvalidFieldType {
            path: object_type_error_path(),
            field: "$".to_string(),
            expected: "object".to_string(),
        });
    };
    let history = object
        .entry("history")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(history) = history.as_array_mut() else {
        return Err(ExecutionError::InvalidFieldType {
            path: object_type_error_path(),
            field: "history".to_string(),
            expected: "array".to_string(),
        });
    };
    history.push(entry);
    Ok(())
}
