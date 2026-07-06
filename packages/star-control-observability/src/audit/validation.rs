use super::AuditEventWriter;
use crate::constants::AUDIT_EVENT_SCHEMA;
use crate::error::ObservabilityError;
use serde_json::Value;
use star_control_schema::{load_schema, validate_json};
use std::path::PathBuf;

impl AuditEventWriter {
    pub fn validate_event(&self, event: &Value) -> Result<(), ObservabilityError> {
        let schema_path = self.schema_root.join(AUDIT_EVENT_SCHEMA);
        let schema =
            load_schema(&schema_path).map_err(|source| ObservabilityError::SchemaLoadFailed {
                path: schema_path.clone(),
                message: source.to_string(),
            })?;
        let result = validate_json(event, &schema);
        if result.is_ok() {
            Ok(())
        } else {
            Err(ObservabilityError::SchemaValidationFailed {
                path: PathBuf::from(AUDIT_EVENT_SCHEMA),
                errors: result.errors,
            })
        }
    }
}
