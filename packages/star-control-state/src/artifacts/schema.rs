use crate::{StateStore, StateStoreError};
use serde_json::Value;
use star_control_schema::{load_schema, validate_json};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub(crate) enum CoreSchema {
    Job,
    RunState,
    Route,
    WorkSpec,
    Report,
    Event,
    ArtifactRef,
    RedactionReport,
}

impl CoreSchema {
    fn file_name(self) -> &'static str {
        match self {
            Self::Job => "job.schema.json",
            Self::RunState => "run-state.schema.json",
            Self::Route => "route.schema.json",
            Self::WorkSpec => "workspec.schema.json",
            Self::Report => "report.schema.json",
            Self::Event => "event.schema.json",
            Self::ArtifactRef => "artifact-ref.schema.json",
            Self::RedactionReport => "redaction-report.schema.json",
        }
    }
}

impl StateStore {
    pub(crate) fn validate_artifact(
        &self,
        schema: CoreSchema,
        artifact_path: PathBuf,
        value: &Value,
    ) -> Result<(), StateStoreError> {
        let schema_path = self.schema_root.join(schema.file_name());
        let schema =
            load_schema(&schema_path).map_err(|source| StateStoreError::SchemaLoadFailed {
                path: schema_path,
                message: source.to_string(),
            })?;
        let result = validate_json(value, &schema);
        if result.is_ok() {
            Ok(())
        } else {
            Err(StateStoreError::SchemaValidationFailed {
                path: artifact_path,
                errors: result.errors,
            })
        }
    }
}
