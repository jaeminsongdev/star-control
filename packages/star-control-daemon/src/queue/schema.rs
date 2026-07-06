use crate::error::DaemonError;
use crate::queue::DaemonQueue;
use serde_json::Value;
use star_control_schema::{load_schema, validate_json};
use std::path::Path;

impl DaemonQueue {
    pub(crate) fn validate_schema(
        &self,
        schema_file: &str,
        document_path: &Path,
        value: &Value,
    ) -> Result<(), DaemonError> {
        let schema_path = self.config.schema_root().join(schema_file);
        let schema = load_schema(&schema_path).map_err(|source| DaemonError::SchemaLoadFailed {
            path: schema_path,
            message: source.to_string(),
        })?;
        let result = validate_json(value, &schema);
        if result.is_ok() {
            Ok(())
        } else {
            Err(DaemonError::SchemaValidationFailed {
                path: document_path.to_path_buf(),
                errors: result.errors,
            })
        }
    }
}
