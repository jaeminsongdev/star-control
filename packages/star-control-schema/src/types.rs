use crate::SchemaLoadError;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Schema {
    value: Value,
    source_path: Option<PathBuf>,
}

impl Schema {
    pub fn from_value(value: Value) -> Result<Self, SchemaLoadError> {
        Self::from_checked(value, None)
    }

    pub(crate) fn from_loaded(value: Value, source_path: PathBuf) -> Result<Self, SchemaLoadError> {
        Self::from_checked(value, Some(source_path))
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }

    fn from_checked(value: Value, source_path: Option<PathBuf>) -> Result<Self, SchemaLoadError> {
        if !value.is_object() {
            return Err(SchemaLoadError::RootNotObject {
                path: source_path,
                actual: type_name(&value).to_string(),
            });
        }

        Ok(Self { value, source_path })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub location: String,
    pub message: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub schema_path: Option<String>,
    pub document_path: Option<PathBuf>,
}

impl ValidationError {
    pub(crate) fn new(location: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            location: location.into(),
            message: message.into(),
            expected: None,
            actual: None,
            schema_path: None,
            document_path: None,
        }
    }

    pub(crate) fn expected(mut self, value: impl Into<String>) -> Self {
        self.expected = Some(value.into());
        self
    }

    pub(crate) fn actual(mut self, value: impl Into<String>) -> Self {
        self.actual = Some(value.into());
        self
    }

    pub(crate) fn schema_path(mut self, value: impl Into<String>) -> Self {
        self.schema_path = Some(value.into());
        self
    }

    fn with_document_path(mut self, path: &Path) -> Self {
        self.document_path = Some(path.to_path_buf());
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
}

impl ValidationResult {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn ok(&self) -> bool {
        self.is_ok()
    }

    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    pub(crate) fn push(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    pub(crate) fn with_document_path(mut self, path: &Path) -> Self {
        self.errors = self
            .errors
            .into_iter()
            .map(|error| error.with_document_path(path))
            .collect();
        self
    }
}

pub(crate) fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

pub(crate) fn type_matches(value: &Value, expected: &str) -> bool {
    match expected {
        "null" => value.is_null(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value
            .as_number()
            .map(|number| number.is_i64() || number.is_u64())
            .unwrap_or(false),
        _ => false,
    }
}
