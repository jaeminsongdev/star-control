use super::super::ValidationEngine;
use crate::builders::{
    build_approval_request, build_review_pack_handoff, build_validation_decision,
    build_validation_run,
};
use crate::constants::{
    APPROVAL_REQUEST_FILE, APPROVAL_REQUEST_PATH, APPROVAL_REQUEST_SCHEMA,
    REVIEW_PACK_HANDOFF_FILE, REVIEW_PACK_HANDOFF_SCHEMA, REVIEW_PACK_MARKDOWN_PATH,
    SENTINEL_TOOL_OUTPUT_DIR, VALIDATION_DECISION_FILE, VALIDATION_DECISION_SCHEMA,
    VALIDATION_RUNS_FILE, VALIDATION_RUN_SCHEMA,
};
use crate::error::ValidationEngineError;
use crate::types::{ValidationContext, ValidationOutcome};
use serde_json::Value;

impl<'a> ValidationEngine<'a> {
    pub(super) fn normal_outcome(
        &self,
        context: &ValidationContext,
        decision: &str,
        next_state: &str,
        reasons: Vec<String>,
        diagnostics: Value,
        review_pack: Option<&Value>,
    ) -> Result<ValidationOutcome, ValidationEngineError> {
        let needs_approval = decision == "HUMAN_REVIEW" || decision == "BLOCK";
        let review_pack_path = needs_approval.then_some(REVIEW_PACK_MARKDOWN_PATH);
        let approval_request_path = needs_approval.then_some(APPROVAL_REQUEST_PATH);
        let decision_artifact = build_validation_decision(
            context,
            decision,
            reasons.clone(),
            diagnostics.clone(),
            next_state,
            review_pack_path,
            approval_request_path,
        );
        self.validate_core_schema(
            &decision_artifact,
            VALIDATION_DECISION_SCHEMA,
            &format!("validation/{}", VALIDATION_DECISION_FILE),
        )?;

        let approval_request = if needs_approval {
            Some(build_approval_request(
                context,
                decision,
                reasons,
                diagnostics.clone(),
                review_pack,
            ))
        } else {
            None
        };
        if let Some(approval_request) = approval_request.as_ref() {
            self.validate_core_schema(
                approval_request,
                APPROVAL_REQUEST_SCHEMA,
                &format!("approvals/{}", APPROVAL_REQUEST_FILE),
            )?;
        }

        let handoff = if needs_approval {
            Some(build_review_pack_handoff(context, decision, review_pack))
        } else {
            None
        };
        if let Some(handoff) = handoff.as_ref() {
            self.validate_core_schema(
                handoff,
                REVIEW_PACK_HANDOFF_SCHEMA,
                &format!("review-packs/{}", REVIEW_PACK_HANDOFF_FILE),
            )?;
        }

        let validation_run = build_validation_run(context, next_state);
        self.validate_core_schema(
            &validation_run,
            VALIDATION_RUN_SCHEMA,
            &format!(
                "tool-output/{}/{}",
                SENTINEL_TOOL_OUTPUT_DIR, VALIDATION_RUNS_FILE
            ),
        )?;

        Ok(ValidationOutcome {
            validation_run,
            decision: decision_artifact,
            approval_request,
            handoff,
        })
    }

    pub(super) fn failed_outcome(
        &self,
        context: &ValidationContext,
        reason: &str,
        diagnostics: Vec<Value>,
    ) -> Result<ValidationOutcome, ValidationEngineError> {
        let decision = build_validation_decision(
            context,
            "BLOCK",
            vec![reason.to_string()],
            Value::Array(diagnostics),
            "FAILED",
            None,
            None,
        );
        self.validate_core_schema(
            &decision,
            VALIDATION_DECISION_SCHEMA,
            &format!("validation/{}", VALIDATION_DECISION_FILE),
        )?;
        let validation_run = build_validation_run(context, "FAILED");
        self.validate_core_schema(
            &validation_run,
            VALIDATION_RUN_SCHEMA,
            &format!(
                "tool-output/{}/{}",
                SENTINEL_TOOL_OUTPUT_DIR, VALIDATION_RUNS_FILE
            ),
        )?;
        Ok(ValidationOutcome {
            validation_run,
            decision,
            approval_request: None,
            handoff: None,
        })
    }
}
