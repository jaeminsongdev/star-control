use crate::{Schema, ValidationError, ValidationResult};
use serde_json::Value;

mod compound;
mod path;
mod pattern;
mod scalar;

pub fn validate_json(document: &Value, schema: &Schema) -> ValidationResult {
    let mut result = ValidationResult::default();
    validate_value(document, schema.value(), "$", "$", &mut result);
    result
}

pub fn assert_valid(document: &Value, schema: &Schema) -> Result<(), ValidationResult> {
    let result = validate_json(document, schema);
    if result.is_ok() {
        Ok(())
    } else {
        Err(result)
    }
}

fn validate_value(
    value: &Value,
    schema: &Value,
    location: &str,
    schema_path: &str,
    result: &mut ValidationResult,
) {
    let Some(schema_object) = schema.as_object() else {
        return;
    };

    if let Some(expected_const) = schema_object.get("const") {
        if value != expected_const {
            result.push(
                ValidationError::new(location, "value does not match const")
                    .expected(expected_const.to_string())
                    .actual(value.to_string())
                    .schema_path(path::child_schema_path(schema_path, "const")),
            );
        }
    }

    if let Some(expected_enum) = schema_object.get("enum").and_then(Value::as_array) {
        if !expected_enum.iter().any(|item| item == value) {
            result.push(
                ValidationError::new(location, "value is not in enum")
                    .expected(Value::Array(expected_enum.clone()).to_string())
                    .actual(value.to_string())
                    .schema_path(path::child_schema_path(schema_path, "enum")),
            );
        }
    }

    if let Some(expected_type) = schema_object.get("type") {
        scalar::validate_type(value, expected_type, location, schema_path, result);
    }

    if let Some(text) = value.as_str() {
        scalar::validate_string(text, schema_object, location, schema_path, result);
    }

    if let Some(object) = value.as_object() {
        compound::validate_object(object, schema_object, location, schema_path, result);
    }

    if let Some(array) = value.as_array() {
        compound::validate_array(array, schema_object, location, schema_path, result);
    }
}
