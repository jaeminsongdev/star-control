use super::helpers::validate;
use serde_json::json;

#[test]
fn validates_const_success_and_failure() {
    assert!(validate(json!("1.0.0"), json!({ "const": "1.0.0" })).is_ok());

    let result = validate(json!("2.0.0"), json!({ "const": "1.0.0" }));
    assert_eq!(result.error_count(), 1);
    assert_eq!(result.errors[0].location, "$");
}

#[test]
fn validates_enum_success_and_failure() {
    assert!(validate(json!("LOW"), json!({ "enum": ["LOW", "HIGH"] })).is_ok());
    assert!(!validate(json!("MEDIUM"), json!({ "enum": ["LOW", "HIGH"] })).is_ok());
}

#[test]
fn validates_primitive_types() {
    assert!(validate(json!("text"), json!({ "type": "string" })).is_ok());
    assert!(validate(json!(true), json!({ "type": "boolean" })).is_ok());
    assert!(validate(json!({}), json!({ "type": "object" })).is_ok());
    assert!(validate(json!([]), json!({ "type": "array" })).is_ok());
    assert!(validate(json!(null), json!({ "type": "null" })).is_ok());
    assert!(validate(json!(1), json!({ "type": "integer" })).is_ok());
    assert!(validate(json!(1.5), json!({ "type": "number" })).is_ok());
    assert!(!validate(json!(true), json!({ "type": "integer" })).is_ok());
    assert!(!validate(json!(true), json!({ "type": "number" })).is_ok());
}

#[test]
fn validates_union_types() {
    let schema_value = json!({ "type": ["string", "null"] });
    assert!(validate(json!("text"), schema_value.clone()).is_ok());
    assert!(validate(json!(null), schema_value.clone()).is_ok());
    assert!(!validate(json!(false), schema_value).is_ok());
}

#[test]
fn validates_min_length() {
    let result = validate(json!(""), json!({ "type": "string", "minLength": 1 }));
    assert_eq!(result.error_count(), 1);
}

#[test]
fn validates_known_patterns() {
    assert!(validate(json!("J-0001"), json!({ "pattern": "^J-[0-9]{4,}$" })).is_ok());
    assert!(validate(
        json!("abc-1.2"),
        json!({ "pattern": "^[a-z0-9][a-z0-9.-]*$" })
    )
    .is_ok());
    assert!(validate(
        json!("abc_1.2"),
        json!({ "pattern": "^[a-z0-9][a-z0-9_.-]*$" })
    )
    .is_ok());
    assert!(validate(
        json!("provider.fake-local"),
        json!({ "pattern": "^provider\\.[a-z0-9][a-z0-9.-]*$" })
    )
    .is_ok());

    assert!(!validate(json!("J-01"), json!({ "pattern": "^J-[0-9]{4,}$" })).is_ok());
    assert!(!validate(json!("ABC"), json!({ "pattern": "^[a-z0-9][a-z0-9.-]*$" })).is_ok());
    assert!(!validate(
        json!("fake-local"),
        json!({ "pattern": "^provider\\.[a-z0-9][a-z0-9.-]*$" })
    )
    .is_ok());
}

#[test]
fn reports_unknown_patterns() {
    let result = validate(json!("anything"), json!({ "pattern": "^[A-Z]+$" }));
    assert_eq!(result.error_count(), 1);
    assert!(result.errors[0].message.contains("not supported"));
}
