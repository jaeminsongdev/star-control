use super::helpers::validate;
use serde_json::json;

#[test]
fn detects_missing_required_properties() {
    let result = validate(
        json!({ "schema_version": "1.0.0" }),
        json!({
            "type": "object",
            "required": ["schema_version", "job_id"]
        }),
    );

    assert_eq!(result.error_count(), 1);
    assert!(result.errors[0]
        .message
        .contains("missing required property"));
}

#[test]
fn validates_nested_properties() {
    let result = validate(
        json!({ "outer": { "inner": 10 } }),
        json!({
            "type": "object",
            "properties": {
                "outer": {
                    "type": "object",
                    "properties": {
                        "inner": { "type": "string" }
                    }
                }
            }
        }),
    );

    assert_eq!(result.errors[0].location, "$.outer.inner");
}

#[test]
fn validates_array_items() {
    let result = validate(
        json!(["ok", 3]),
        json!({
            "type": "array",
            "items": { "type": "string" }
        }),
    );

    assert_eq!(result.error_count(), 1);
    assert_eq!(result.errors[0].location, "$[1]");
}

#[test]
fn validates_additional_properties_schema_and_false() {
    assert!(validate(
        json!({ "implement": "workspecs/implement.json" }),
        json!({
            "type": "object",
            "additionalProperties": { "type": "string" }
        }),
    )
    .is_ok());

    let typed_result = validate(
        json!({ "implement": 3 }),
        json!({
            "type": "object",
            "additionalProperties": { "type": "string" }
        }),
    );
    assert_eq!(typed_result.errors[0].location, "$.implement");

    let closed_result = validate(
        json!({ "known": "ok", "extra": "no" }),
        json!({
            "type": "object",
            "properties": {
                "known": { "type": "string" }
            },
            "additionalProperties": false
        }),
    );
    assert_eq!(closed_result.errors[0].location, "$.extra");
}
