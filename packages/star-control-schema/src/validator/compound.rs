use super::path::{child_location, child_schema_path};
use super::validate_value;
use crate::{ValidationError, ValidationResult};
use serde_json::{Map, Value};

pub(super) fn validate_object(
    object: &Map<String, Value>,
    schema_object: &Map<String, Value>,
    location: &str,
    schema_path: &str,
    result: &mut ValidationResult,
) {
    if let Some(required) = schema_object.get("required").and_then(Value::as_array) {
        for key in required.iter().filter_map(Value::as_str) {
            if !object.contains_key(key) {
                result.push(
                    ValidationError::new(location, format!("missing required property {:?}", key))
                        .expected(key)
                        .schema_path(child_schema_path(schema_path, "required")),
                );
            }
        }
    }

    let properties = schema_object.get("properties").and_then(Value::as_object);
    if let Some(properties) = properties {
        for (key, child_schema) in properties {
            if let Some(child_value) = object.get(key) {
                let child_location = child_location(location, key);
                let child_schema_path =
                    format!("{}.{}", child_schema_path(schema_path, "properties"), key);
                validate_value(
                    child_value,
                    child_schema,
                    &child_location,
                    &child_schema_path,
                    result,
                );
            }
        }
    }

    match schema_object.get("additionalProperties") {
        Some(Value::Object(_)) => {
            let additional_schema = &schema_object["additionalProperties"];
            for (key, child_value) in object {
                if properties
                    .map(|properties| properties.contains_key(key))
                    .unwrap_or(false)
                {
                    continue;
                }
                let child_location = child_location(location, key);
                let child_schema_path = child_schema_path(schema_path, "additionalProperties");
                validate_value(
                    child_value,
                    additional_schema,
                    &child_location,
                    &child_schema_path,
                    result,
                );
            }
        }
        Some(Value::Bool(false)) => {
            for key in object.keys() {
                if properties
                    .map(|properties| properties.contains_key(key))
                    .unwrap_or(false)
                {
                    continue;
                }
                result.push(
                    ValidationError::new(
                        child_location(location, key),
                        "additional property is not allowed",
                    )
                    .expected("known property")
                    .actual(key)
                    .schema_path(child_schema_path(schema_path, "additionalProperties")),
                );
            }
        }
        _ => {}
    }
}

pub(super) fn validate_array(
    array: &[Value],
    schema_object: &Map<String, Value>,
    location: &str,
    schema_path: &str,
    result: &mut ValidationResult,
) {
    if let Some(item_schema) = schema_object.get("items").filter(|value| value.is_object()) {
        for (index, item) in array.iter().enumerate() {
            validate_value(
                item,
                item_schema,
                &format!("{}[{}]", location, index),
                &child_schema_path(schema_path, "items"),
                result,
            );
        }
    }
}
