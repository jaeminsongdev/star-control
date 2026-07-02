use serde_json::Value;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Schema {
    value: Value,
    source_path: Option<PathBuf>,
}

impl Schema {
    pub fn from_value(value: Value) -> Result<Self, SchemaLoadError> {
        if !value.is_object() {
            return Err(SchemaLoadError::RootNotObject {
                path: None,
                actual: type_name(&value).to_string(),
            });
        }

        Ok(Self {
            value,
            source_path: None,
        })
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }
}

#[derive(Debug)]
pub enum SchemaLoadError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidJson {
        path: PathBuf,
        source: serde_json::Error,
    },
    RootNotObject {
        path: Option<PathBuf>,
        actual: String,
    },
}

impl fmt::Display for SchemaLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    formatter,
                    "failed to read schema {}: {}",
                    path.display(),
                    source
                )
            }
            Self::InvalidJson { path, source } => {
                write!(
                    formatter,
                    "failed to parse schema JSON {}: {}",
                    path.display(),
                    source
                )
            }
            Self::RootNotObject { path, actual } => {
                if let Some(path) = path {
                    write!(
                        formatter,
                        "schema root must be object in {}, got {}",
                        path.display(),
                        actual
                    )
                } else {
                    write!(formatter, "schema root must be object, got {}", actual)
                }
            }
        }
    }
}

impl Error for SchemaLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::InvalidJson { source, .. } => Some(source),
            Self::RootNotObject { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum DocumentLoadError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidJson {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl fmt::Display for DocumentLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    formatter,
                    "failed to read document {}: {}",
                    path.display(),
                    source
                )
            }
            Self::InvalidJson { path, source } => {
                write!(
                    formatter,
                    "failed to parse document JSON {}: {}",
                    path.display(),
                    source
                )
            }
        }
    }
}

impl Error for DocumentLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::InvalidJson { source, .. } => Some(source),
        }
    }
}

#[derive(Debug)]
pub enum FileValidationError {
    Schema(SchemaLoadError),
    Document(DocumentLoadError),
}

impl fmt::Display for FileValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Schema(source) => write!(formatter, "schema load failed: {}", source),
            Self::Document(source) => write!(formatter, "document load failed: {}", source),
        }
    }
}

impl Error for FileValidationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Schema(source) => Some(source),
            Self::Document(source) => Some(source),
        }
    }
}

impl From<SchemaLoadError> for FileValidationError {
    fn from(source: SchemaLoadError) -> Self {
        Self::Schema(source)
    }
}

impl From<DocumentLoadError> for FileValidationError {
    fn from(source: DocumentLoadError) -> Self {
        Self::Document(source)
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
    fn new(location: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            location: location.into(),
            message: message.into(),
            expected: None,
            actual: None,
            schema_path: None,
            document_path: None,
        }
    }

    fn expected(mut self, value: impl Into<String>) -> Self {
        self.expected = Some(value.into());
        self
    }

    fn actual(mut self, value: impl Into<String>) -> Self {
        self.actual = Some(value.into());
        self
    }

    fn schema_path(mut self, value: impl Into<String>) -> Self {
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

    fn push(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    fn with_document_path(mut self, path: &Path) -> Self {
        self.errors = self
            .errors
            .into_iter()
            .map(|error| error.with_document_path(path))
            .collect();
        self
    }
}

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

    if !value.is_object() {
        return Err(SchemaLoadError::RootNotObject {
            path: Some(path.to_path_buf()),
            actual: type_name(&value).to_string(),
        });
    }

    Ok(Schema {
        value,
        source_path: Some(path.to_path_buf()),
    })
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

pub fn validate_json(document: &Value, schema: &Schema) -> ValidationResult {
    let mut result = ValidationResult::default();
    validate_value(document, schema.value(), "$", "$", &mut result);
    result
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
                    .schema_path(child_schema_path(schema_path, "const")),
            );
        }
    }

    if let Some(expected_enum) = schema_object.get("enum").and_then(Value::as_array) {
        if !expected_enum.iter().any(|item| item == value) {
            result.push(
                ValidationError::new(location, "value is not in enum")
                    .expected(Value::Array(expected_enum.clone()).to_string())
                    .actual(value.to_string())
                    .schema_path(child_schema_path(schema_path, "enum")),
            );
        }
    }

    if let Some(expected_type) = schema_object.get("type") {
        validate_type(value, expected_type, location, schema_path, result);
    }

    if let Some(text) = value.as_str() {
        validate_string(text, schema_object, location, schema_path, result);
    }

    if let Some(object) = value.as_object() {
        validate_object(object, schema_object, location, schema_path, result);
    }

    if let Some(array) = value.as_array() {
        validate_array(array, schema_object, location, schema_path, result);
    }
}

fn validate_type(
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

fn validate_string(
    text: &str,
    schema_object: &serde_json::Map<String, Value>,
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

fn validate_object(
    object: &serde_json::Map<String, Value>,
    schema_object: &serde_json::Map<String, Value>,
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

fn validate_array(
    array: &[Value],
    schema_object: &serde_json::Map<String, Value>,
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

fn type_name(value: &Value) -> &'static str {
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

fn type_matches(value: &Value, expected: &str) -> bool {
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

fn child_location(location: &str, key: &str) -> String {
    if key
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-')
    {
        format!("{}.{}", location, key)
    } else {
        format!("{}[{}]", location, serde_json::to_string(key).unwrap())
    }
}

fn child_schema_path(schema_path: &str, key: &str) -> String {
    format!("{}.{}", schema_path, key)
}

fn known_pattern_matches(text: &str, pattern: &str) -> Option<bool> {
    match pattern {
        "^J-[0-9]{4,}$" => Some(matches_job_id(text)),
        "^[a-z0-9][a-z0-9.-]*$" => Some(matches_slug(text, ".-")),
        "^[a-z0-9][a-z0-9_.-]*$" => Some(matches_slug(text, "_.-")),
        "^provider\\.[a-z0-9][a-z0-9.-]*$" => Some(
            text.strip_prefix("provider.")
                .is_some_and(|provider_id| matches_slug(provider_id, ".-")),
        ),
        _ => None,
    }
}

fn matches_job_id(text: &str) -> bool {
    let Some(suffix) = text.strip_prefix("J-") else {
        return false;
    };
    suffix.len() >= 4 && suffix.chars().all(|character| character.is_ascii_digit())
}

fn matches_slug(text: &str, extra_allowed: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    is_lower_ascii_alnum(first)
        && chars
            .all(|character| is_lower_ascii_alnum(character) || extra_allowed.contains(character))
}

fn is_lower_ascii_alnum(character: char) -> bool {
    character.is_ascii_lowercase() || character.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn schema(value: Value) -> Schema {
        Schema::from_value(value).expect("schema root should be object")
    }

    fn validate(document: Value, schema_value: Value) -> ValidationResult {
        validate_json(&document, &schema(schema_value))
    }

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
        fs::write(&document_path, r#"{ "schema_version": "1.0.0" }"#)
            .expect("write document fixture");

        let result = validate_file(&document_path, &schema_path).expect("validate file");
        fs::remove_file(&schema_path).ok();
        fs::remove_file(&document_path).ok();

        assert_eq!(result.error_count(), 1);
        assert_eq!(
            result.errors[0].document_path.as_deref(),
            Some(document_path.as_path())
        );
    }

    fn unique_temp_path(name: &str) -> PathBuf {
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
}
