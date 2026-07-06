use crate::constants::APPROVAL_RESPONSE_PATH;
use crate::error::ValidationEngineError;
use serde_json::{json, Value};
use star_control_schema::{load_schema, validate_json};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn validate_schema_value(
    value: &Value,
    schema_root: &Path,
    schema_file: &str,
    relative_path: &str,
) -> Result<(), ValidationEngineError> {
    let schema_path = schema_root.join(schema_file);
    let schema =
        load_schema(&schema_path).map_err(|source| ValidationEngineError::SchemaLoadFailed {
            path: schema_path.clone(),
            message: source.to_string(),
        })?;
    let result = validate_json(value, &schema);
    if result.is_ok() {
        Ok(())
    } else {
        Err(ValidationEngineError::SchemaValidationFailed {
            path: PathBuf::from(relative_path),
            schema_path,
            errors: result.errors,
        })
    }
}

pub(crate) fn read_json_file(path: &Path) -> Result<Value, ValidationEngineError> {
    let content = fs::read_to_string(path).map_err(|source| ValidationEngineError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&content).map_err(|source| ValidationEngineError::InvalidJson {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn required_string<'a>(
    value: &'a Value,
    path: &Path,
    field: &str,
) -> Result<&'a str, ValidationEngineError> {
    let Some(field_value) = value.get(field) else {
        return Err(ValidationEngineError::MissingField {
            path: path.to_path_buf(),
            field: field.to_string(),
        });
    };
    field_value
        .as_str()
        .ok_or_else(|| ValidationEngineError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field.to_string(),
            expected: "string".to_string(),
        })
}

pub(crate) fn ensure_response_field_matches(
    response: &Value,
    field: &str,
    expected: &str,
) -> Result<(), ValidationEngineError> {
    let actual = required_string(response, Path::new(APPROVAL_RESPONSE_PATH), field)?;
    if actual == expected {
        Ok(())
    } else {
        Err(ValidationEngineError::ApprovalResponseMismatch {
            field: field.to_string(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    }
}

pub(crate) fn string_array(
    value: &Value,
    path: &Path,
    field: &str,
) -> Result<Vec<String>, ValidationEngineError> {
    let Some(field_value) = value.get(field) else {
        return Err(ValidationEngineError::MissingField {
            path: path.to_path_buf(),
            field: field.to_string(),
        });
    };
    let Some(items) = field_value.as_array() else {
        return Err(ValidationEngineError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field.to_string(),
            expected: "array".to_string(),
        });
    };
    Ok(items
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect())
}

pub(crate) fn diagnostics_array(value: &Value) -> Vec<Value> {
    value.as_array().cloned().unwrap_or_default()
}

pub(crate) fn has_block_diagnostic(diagnostics: &Value) -> bool {
    diagnostics
        .as_array()
        .map(|items| {
            items.iter().any(|item| {
                item.get("severity")
                    .and_then(Value::as_str)
                    .is_some_and(|severity| severity == "block")
            })
        })
        .unwrap_or(false)
}

pub(crate) fn diagnostic_for_error(rule_id: &str, error: &ValidationEngineError) -> Value {
    json!({
        "rule_id": rule_id,
        "severity": "block",
        "message": error.to_string()
    })
}
