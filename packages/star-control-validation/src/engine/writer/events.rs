use super::ValidationEngine;
use crate::constants::{
    APPROVAL_REQUEST_FILE, REVIEW_PACK_HANDOFF_FILE, SCHEMA_VERSION, SENTINEL_TOOL_OUTPUT_DIR,
    VALIDATION_DECISION_FILE, VALIDATION_RUNS_FILE,
};
use crate::error::ValidationEngineError;
use crate::types::{ValidationContext, ValidationOutcome};
use serde_json::{json, Value};

impl<'a> ValidationEngine<'a> {
    pub(super) fn append_gate_events(
        &self,
        context: &ValidationContext,
        outcome: &ValidationOutcome,
    ) -> Result<(), ValidationEngineError> {
        self.append_event(
            context,
            "VALIDATION_RECORDED",
            "Validation run recorded",
            vec![format!(
                "tool-output/{}/{}",
                SENTINEL_TOOL_OUTPUT_DIR, VALIDATION_RUNS_FILE
            )],
            json!({ "status": outcome.validation_run()["status"] }),
        )?;
        self.append_event(
            context,
            "GATE_DECIDED",
            "Validation gate decision recorded",
            vec![format!("validation/{}", VALIDATION_DECISION_FILE)],
            json!({
                "decision": outcome.decision()["decision"],
                "next_state": outcome.decision()["next_state"]
            }),
        )?;
        if outcome.handoff().is_some() {
            self.append_event(
                context,
                "REVIEW_PACK_CREATED",
                "Review pack handoff recorded",
                vec![format!("review-packs/{}", REVIEW_PACK_HANDOFF_FILE)],
                json!({ "decision": outcome.decision()["decision"] }),
            )?;
        }
        if outcome.approval_request().is_some() {
            self.append_event(
                context,
                "APPROVAL_REQUESTED",
                "Human approval requested",
                vec![format!("approvals/{}", APPROVAL_REQUEST_FILE)],
                json!({ "decision": outcome.decision()["decision"] }),
            )?;
        }
        Ok(())
    }

    fn append_event(
        &self,
        context: &ValidationContext,
        event_type: &str,
        message: &str,
        artifact_paths: Vec<String>,
        details: Value,
    ) -> Result<(), ValidationEngineError> {
        let event = json!({
            "schema_version": SCHEMA_VERSION,
            "event_id": format!(
                "{}-{}-{}",
                context.job_id().to_lowercase(),
                context.stage(),
                event_type.to_lowercase().replace('_', "-")
            ),
            "job_id": context.job_id(),
            "type": event_type,
            "created_at": context.requested_at(),
            "stage": context.stage(),
            "state": "",
            "message": message,
            "artifact_paths": artifact_paths,
            "details": details
        });
        self.state_store.append_event(context.job_id(), &event)?;
        Ok(())
    }
}
