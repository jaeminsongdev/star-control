use super::path::child_schema_path;
use super::pattern::known_pattern_matches;
use crate::types::{type_matches, type_name};
use crate::{ValidationError, ValidationResult};
use serde_json::{Map, Value};

pub(super) fn validate_type(
    value: &Value,
    expected_type: &Value,
    location: &str,
    schema_path: &str,
    result: &mut ValidationResult,
) {
    let expected_types = if let Some(expected) = expected_type.as_str() {
        vec![expected]
    } else if let Some(items) = expected_type.as_array() {
        let mut expected = Vec::new();
        for item in items {
            if let Some(item) = item.as_str() {
                expected.push(item);
            } else {
                result.push(
                    ValidationError::new(
                        location,
                        "schema type must be a string or list of strings",
                    )
                    .actual(expected_type.to_string())
                    .schema_path(child_schema_path(schema_path, "type")),
                );
                return;
            }
        }
        expected
    } else {
        result.push(
            ValidationError::new(location, "schema type must be a string or list of strings")
                .actual(expected_type.to_string())
                .schema_path(child_schema_path(schema_path, "type")),
        );
        return;
    };

    if !expected_types
        .iter()
        .any(|expected| type_matches(value, expected))
    {
        result.push(
            ValidationError::new(location, "value has wrong JSON type")
                .expected(format!("{:?}", expected_types))
                .actual(type_name(value))
                .schema_path(child_schema_path(schema_path, "type")),
        );
    }
}

pub(super) fn validate_string(
    text: &str,
    schema_object: &Map<String, Value>,
    location: &str,
    schema_path: &str,
    result: &mut ValidationResult,
) {
    if let Some(min_length) = schema_object.get("minLength").and_then(Value::as_u64) {
        if (text.chars().count() as u64) < min_length {
            result.push(
                ValidationError::new(location, "string is shorter than minLength")
                    .expected(format!("minLength {}", min_length))
                    .actual(format!("length {}", text.chars().count()))
                    .schema_path(child_schema_path(schema_path, "minLength")),
            );
        }
    }

    if let Some(pattern) = schema_object.get("pattern").and_then(Value::as_str) {
        match known_pattern_matches(text, pattern) {
            Some(true) => {}
            Some(false) => result.push(
                ValidationError::new(location, "string does not match pattern")
                    .expected(pattern)
                    .actual(text)
                    .schema_path(child_schema_path(schema_path, "pattern")),
            ),
            None => result.push(
                ValidationError::new(
                    location,
                    "schema pattern is not supported by runtime validator",
                )
                .expected("supported repository pattern")
                .actual(pattern)
                .schema_path(child_schema_path(schema_path, "pattern")),
            ),
        }
    }
}
