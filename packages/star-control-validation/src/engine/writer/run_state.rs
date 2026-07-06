use super::ValidationEngine;
use crate::error::ValidationEngineError;
use crate::state::{next_action_for_state, push_history, set_object_field};
use crate::types::{ValidationContext, ValidationOutcome};
use serde_json::{json, Value};

impl<'a> ValidationEngine<'a> {
    pub(super) fn update_run_state(
        &self,
        context: &ValidationContext,
        outcome: &ValidationOutcome,
        validation_run_ref: &Value,
        decision_ref: &Value,
        approval_request_ref: Option<&Value>,
        handoff_ref: Option<&Value>,
    ) -> Result<Value, ValidationEngineError> {
        let mut state = self.state_store.load_state(context.job_id())?;
        set_object_field(
            &mut state,
            "state",
            Value::String(outcome.next_state().unwrap_or("FAILED").to_string()),
        )?;
        set_object_field(
            &mut state,
            "current_stage",
            Value::String(context.stage().to_string()),
        )?;
        set_object_field(
            &mut state,
            "updated_at",
            Value::String(context.requested_at().to_string()),
        )?;
        set_object_field(
            &mut state,
            "latest_event_id",
            Value::String(format!(
                "{}-{}-gate-decided",
                context.job_id().to_lowercase(),
                context.stage()
            )),
        )?;
        set_object_field(
            &mut state,
            "next_action",
            Value::String(
                next_action_for_state(outcome.next_state().unwrap_or("FAILED")).to_string(),
            ),
        )?;
        self.state_store.register_artifact_ref(
            &mut state,
            &format!("{}_validation_run", context.stage()),
            validation_run_ref,
        )?;
        self.state_store.register_artifact_ref(
            &mut state,
            &format!("{}_validation_decision", context.stage()),
            decision_ref,
        )?;
        if let Some(approval_request_ref) = approval_request_ref {
            self.state_store.register_artifact_ref(
                &mut state,
                &format!("{}_approval_request", context.stage()),
                approval_request_ref,
            )?;
        }
        if let Some(handoff_ref) = handoff_ref {
            self.state_store.register_artifact_ref(
                &mut state,
                &format!("{}_review_pack_handoff", context.stage()),
                handoff_ref,
            )?;
        }
        push_history(
            &mut state,
            json!({
                "stage": context.stage(),
                "task_id": context.task_id(),
                "decision": outcome.decision()["decision"],
                "next_state": outcome.decision()["next_state"]
            }),
        )?;
        self.state_store.save_state(context.job_id(), &state)?;
        Ok(state)
    }
}
