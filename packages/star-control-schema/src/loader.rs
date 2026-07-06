use crate::{
    validate_json, DocumentLoadError, FileValidationError, Schema, SchemaLoadError,
    ValidationResult,
};
use serde_json::Value;
use std::fs;
use std::path::Path;

pub fn load_schema(schema_path: impl AsRef<Path>) -> Result<Schema, SchemaLoadError> {
    let path = schema_path.as_ref();
    let content = fs::read_to_string(path).map_err(|source| SchemaLoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let value: Value =
        serde_json::from_str(&content).map_err(|source| SchemaLoadError::InvalidJson {
            path: path.to_path_buf(),
            source,
        })?;

    Schema::from_loaded(value, path.to_path_buf())
}

pub fn load_document(document_path: impl AsRef<Path>) -> Result<Value, DocumentLoadError> {
    let path = document_path.as_ref();
    let content = fs::read_to_string(path).map_err(|source| DocumentLoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&content).map_err(|source| DocumentLoadError::InvalidJson {
        path: path.to_path_buf(),
        source,
    })
}

pub fn validate_file(
    document_path: impl AsRef<Path>,
    schema_path: impl AsRef<Path>,
) -> Result<ValidationResult, FileValidationError> {
    let document_path = document_path.as_ref();
    let schema = load_schema(schema_path)?;
    let document = load_document(document_path)?;
    Ok(validate_json(&document, &schema).with_document_path(document_path))
}
