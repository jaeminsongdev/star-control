use super::helpers::unique_temp_path;
use crate::{load_document, validate_file, DocumentLoadError, Schema, SchemaLoadError};
use serde_json::json;
use std::fs;

#[test]
fn detects_invalid_schema_root() {
    let error = Schema::from_value(json!(["not", "object"])).unwrap_err();
    assert!(matches!(error, SchemaLoadError::RootNotObject { .. }));
}

#[test]
fn detects_invalid_json_document() {
    let path = unique_temp_path("invalid-document.json");
    fs::write(&path, "{ invalid json").expect("write invalid json fixture");

    let error = load_document(&path).unwrap_err();
    fs::remove_file(&path).ok();

    assert!(matches!(error, DocumentLoadError::InvalidJson { .. }));
}

#[test]
fn validate_file_attaches_document_path_to_errors() {
    let schema_path = unique_temp_path("schema.json");
    let document_path = unique_temp_path("document.json");
    fs::write(
        &schema_path,
        r#"{ "type": "object", "required": ["job_id"] }"#,
    )
    .expect("write schema fixture");
    fs::write(&document_path, r#"{ "schema_version": "1.0.0" }"#).expect("write document fixture");

    let result = validate_file(&document_path, &schema_path).expect("validate file");
    fs::remove_file(&schema_path).ok();
    fs::remove_file(&document_path).ok();

    assert_eq!(result.error_count(), 1);
    assert_eq!(
        result.errors[0].document_path.as_deref(),
        Some(document_path.as_path())
    );
}
