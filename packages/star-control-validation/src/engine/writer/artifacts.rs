use super::ValidationEngine;
use crate::artifacts::read_json_file;
use crate::constants::{SENTINEL_TOOL_OUTPUT_DIR, VALIDATION_RUNS_FILE, VALIDATION_RUN_SCHEMA};
use crate::error::ValidationEngineError;
use crate::types::{ValidationContext, ValidationOutcome};
use serde_json::Value;
use star_control_state::ArtifactKind;

impl<'a> ValidationEngine<'a> {
    pub(super) fn write_or_reference_validation_run(
        &self,
        context: &ValidationContext,
        outcome: &ValidationOutcome,
    ) -> Result<Value, ValidationEngineError> {
        let relative_path = format!(
            "tool-output/{}/{}",
            SENTINEL_TOOL_OUTPUT_DIR, VALIDATION_RUNS_FILE
        );
        let resolved = self
            .state_store
            .resolve_job_path(context.job_id(), &relative_path)?;
        if resolved.exists() {
            let existing = read_json_file(&resolved)?;
            self.validate_core_schema(&existing, VALIDATION_RUN_SCHEMA, &relative_path)?;
            self.state_store
                .artifact_ref(
                    context.job_id(),
                    &relative_path,
                    ArtifactKind::ToolOutput,
                    SENTINEL_TOOL_OUTPUT_DIR,
                    Some("specs/schemas/validation-run.schema.json"),
                    Some("validation run output"),
                )
                .map_err(ValidationEngineError::from)
        } else {
            self.state_store
                .write_tool_json(
                    context.job_id(),
                    SENTINEL_TOOL_OUTPUT_DIR,
                    VALIDATION_RUNS_FILE,
                    outcome.validation_run(),
                )
                .map_err(ValidationEngineError::from)
        }
    }
}
