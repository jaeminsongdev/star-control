mod event;
mod request;
mod response;

use self::event::{approval_recorded_event, approval_recorded_event_id, approval_success_payload};
use self::request::ApprovalDecision;
use self::response::{
    approval_metadata, build_approval_response, load_approval_request, validate_approval_response,
    write_approval_response,
};
use super::super::helpers::{
    append_api_event, next_action_after_approval_response, state_after_approval_response,
    state_string, update_state_for_control_command,
};
use super::super::ApiControlService;
use crate::artifacts::ControlArtifactError;
use crate::error::ApiError;
use serde_json::{json, Value};

impl ApiControlService {
    pub(in crate::control) fn approve_response(
        &self,
        project_id: &str,
        job_id: &str,
        body: &Value,
    ) -> Result<Value, ApiError> {
        let Some(store) = self.read_only.projects.get(project_id) else {
            return self.read_only.project_not_found(project_id);
        };
        let mut state = match store.load_state(job_id) {
            Ok(value) => value,
            Err(source) => {
                return self
                    .read_only
                    .state_error_envelope("state_read_failed", source)
            }
        };
        let current_state = state_string(&state);
        if current_state != "WAITING_APPROVAL" {
            return self.read_only.error_envelope(
                "invalid_control_state",
                "approve requires WAITING_APPROVAL state",
                json!({ "job_id": job_id, "state": current_state }),
            );
        }

        let decision = match ApprovalDecision::from_body(body) {
            Ok(value) => value,
            Err(message) => return self.invalid_control_request(&message),
        };

        let approval_request =
            match load_approval_request(store, job_id, &self.read_only.schema_root) {
                Ok(value) => value,
                Err(ControlArtifactError::Missing { path }) => {
                    return self.read_only.error_envelope(
                        "approval_request_missing",
                        "approval request artifact is required before approve",
                        json!({ "path": path }),
                    )
                }
                Err(error) => {
                    return self.read_only.error_envelope(
                        "approval_request_invalid",
                        &error.to_string(),
                        json!({ "job_id": job_id }),
                    )
                }
            };
        let metadata = approval_metadata(&approval_request);
        let approval_response = build_approval_response(job_id, &metadata, &decision);
        if let Err(errors) =
            validate_approval_response(&approval_response, &self.read_only.schema_root)
        {
            let message = format!(
                "approval response failed schema validation with {} error(s)",
                errors
            );
            return self.read_only.error_envelope(
                "approval_response_invalid",
                &message,
                json!({ "job_id": job_id }),
            );
        }

        let approval_ref = match write_approval_response(store, job_id, &approval_response) {
            Ok(value) => value,
            Err(source) => {
                return self
                    .read_only
                    .state_error_envelope("approval_response_write_failed", source)
            }
        };
        let next_state = state_after_approval_response(decision.response());
        let next_action = next_action_after_approval_response(decision.response());
        let event_id = approval_recorded_event_id(job_id);
        if let Err(source) = update_state_for_control_command(
            &mut state,
            store,
            next_state,
            metadata.stage(),
            next_action,
            &event_id,
            Some(("approval_response", &approval_ref)),
        ) {
            return self
                .read_only
                .state_error_envelope("state_update_failed", source);
        }
        if let Err(source) = store.save_state(job_id, &state) {
            return self
                .read_only
                .state_error_envelope("state_write_failed", source);
        }
        if let Err(source) = append_api_event(
            store,
            job_id,
            approval_recorded_event(event_id, next_state, metadata.stage(), &approval_response),
        ) {
            return self
                .read_only
                .state_error_envelope("event_write_failed", source);
        }

        self.read_only.success_envelope(approval_success_payload(
            job_id,
            &state,
            &approval_response,
        ))
    }
}
