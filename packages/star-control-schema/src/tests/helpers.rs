use crate::{validate_json, Schema, ValidationResult};
use serde_json::Value;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn validate(document: Value, schema_value: Value) -> ValidationResult {
    validate_json(&document, &schema(schema_value))
}

pub(super) fn unique_temp_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "star-control-schema-{}-{}-{}",
        std::process::id(),
        nanos,
        name
    ))
}

fn schema(value: Value) -> Schema {
    Schema::from_value(value).expect("schema root should be object")
}
