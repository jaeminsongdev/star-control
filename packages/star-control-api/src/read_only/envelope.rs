use super::ApiReadOnlyService;
use crate::constants::{API_RESPONSE_SCHEMA, SCHEMA_VERSION};
use crate::error::ApiError;
use serde_json::{json, Value};
use star_control_schema::{load_schema, validate_json};
use star_control_security::redact_value;
use star_control_state::StateStoreError;
use std::path::PathBuf;

impl ApiReadOnlyService {
    pub(crate) fn state_error_envelope(
        &self,
        code: &str,
        source: StateStoreError,
    ) -> Result<Value, ApiError> {
        let status = match source {
            StateStoreError::ArtifactNotFound { .. } | StateStoreError::JobNotFound { .. } => {
                "failed"
            }
            StateStoreError::TerminalStateBlocked { .. } => "blocked",
            _ => "failed",
        };
        self.envelope(
            status,
            json!({}),
            json!({
                "code": code,
                "message": source.to_string()
            }),
            Vec::new(),
        )
    }

    pub(crate) fn success_envelope(&self, data: Value) -> Result<Value, ApiError> {
        self.envelope("success", data, Value::Null, Vec::new())
    }

    pub(crate) fn error_envelope(
        &self,
        code: &str,
        message: &str,
        details: Value,
    ) -> Result<Value, ApiError> {
        self.envelope(
            "failed",
            json!({}),
            json!({
                "code": code,
                "message": message,
                "details": details
            }),
            Vec::new(),
        )
    }

    pub(crate) fn envelope(
        &self,
        status: &str,
        data: Value,
        error: Value,
        warnings: Vec<String>,
    ) -> Result<Value, ApiError> {
        let envelope = json!({
            "schema_version": SCHEMA_VERSION,
            "status": status,
            "data": redact_value(data),
            "error": redact_value(error),
            "warnings": warnings
        });
        self.validate_response(&envelope)?;
        Ok(envelope)
    }

    fn validate_response(&self, envelope: &Value) -> Result<(), ApiError> {
        let schema_path = self.schema_root.join(API_RESPONSE_SCHEMA);
        let schema = load_schema(&schema_path).map_err(|source| ApiError::SchemaLoadFailed {
            path: schema_path,
            message: source.to_string(),
        })?;
        let result = validate_json(envelope, &schema);
        if result.is_ok() {
            Ok(())
        } else {
            Err(ApiError::SchemaValidationFailed {
                path: PathBuf::from(API_RESPONSE_SCHEMA),
                errors: result.errors,
            })
        }
    }
}
