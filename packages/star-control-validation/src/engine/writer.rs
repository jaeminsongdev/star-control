mod artifacts;
mod events;
mod run_state;

use super::ValidationEngine;
use crate::constants::{
    APPROVAL_REQUEST_FILE, APPROVAL_REQUEST_SCHEMA, REVIEW_PACK_HANDOFF_FILE,
    REVIEW_PACK_HANDOFF_SCHEMA, SENTINEL_TOOL_OUTPUT_DIR, VALIDATION_DECISION_FILE,
    VALIDATION_DECISION_SCHEMA, VALIDATION_RUNS_FILE, VALIDATION_RUN_SCHEMA,
};
use crate::error::ValidationEngineError;
use crate::types::{ValidationContext, ValidationOutcome, WrittenValidationArtifacts};

impl<'a> ValidationEngine<'a> {
    pub fn write_outcome(
        &self,
        context: &ValidationContext,
        outcome: &ValidationOutcome,
    ) -> Result<WrittenValidationArtifacts, ValidationEngineError> {
        self.validate_core_schema(
            outcome.validation_run(),
            VALIDATION_RUN_SCHEMA,
            &format!(
                "tool-output/{}/{}",
                SENTINEL_TOOL_OUTPUT_DIR, VALIDATION_RUNS_FILE
            ),
        )?;
        self.validate_core_schema(
            outcome.decision(),
            VALIDATION_DECISION_SCHEMA,
            &format!("validation/{}", VALIDATION_DECISION_FILE),
        )?;

        let validation_run_ref = self.write_or_reference_validation_run(context, outcome)?;
        let decision_ref = self.state_store.write_validation_json(
            context.job_id(),
            VALIDATION_DECISION_FILE,
            outcome.decision(),
        )?;

        let handoff_ref = if let Some(handoff) = outcome.handoff() {
            self.validate_core_schema(
                handoff,
                REVIEW_PACK_HANDOFF_SCHEMA,
                &format!("review-packs/{}", REVIEW_PACK_HANDOFF_FILE),
            )?;
            Some(self.state_store.write_review_pack_json(
                context.job_id(),
                REVIEW_PACK_HANDOFF_FILE,
                handoff,
            )?)
        } else {
            None
        };

        let approval_request_ref = if let Some(approval_request) = outcome.approval_request() {
            self.validate_core_schema(
                approval_request,
                APPROVAL_REQUEST_SCHEMA,
                &format!("approvals/{}", APPROVAL_REQUEST_FILE),
            )?;
            Some(self.state_store.write_approval_json(
                context.job_id(),
                APPROVAL_REQUEST_FILE,
                approval_request,
            )?)
        } else {
            None
        };

        let state = self.update_run_state(
            context,
            outcome,
            &validation_run_ref,
            &decision_ref,
            approval_request_ref.as_ref(),
            handoff_ref.as_ref(),
        )?;
        self.append_gate_events(context, outcome)?;

        Ok(WrittenValidationArtifacts {
            validation_run_ref,
            decision_ref,
            approval_request_ref,
            handoff_ref,
            state,
        })
    }
}
