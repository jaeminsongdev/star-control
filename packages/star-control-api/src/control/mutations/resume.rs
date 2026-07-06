mod artifacts;
mod event;

use self::artifacts::{
    load_approval_request, load_approval_response, next_action_from_approval_response,
};
use self::event::{
    resume_event_id, resume_skipped_payload, resume_state_changed_event, resume_success_payload,
};
use super::super::helpers::{
    append_api_event, ensure_approval_response_matches_request, state_string, string_field,
    update_state_for_control_command,
};
use super::super::ApiControlService;
use crate::artifacts::ControlArtifactError;
use crate::error::ApiError;
use serde_json::{json, Value};

impl ApiControlService {
    pub(in crate::control) fn resume_response(
        &self,
        project_id: &str,
        job_id: &str,
    ) -> Result<Value, ApiError> {
        let Some(store) = self.read_only.projects.get(project_id) else {
            return self.read_only.project_not_found(project_id);
        };
        if let Err(source) = store.ensure_resume_allowed(job_id) {
            return self
                .read_only
                .state_error_envelope("resume_precondition_failed", source);
        }
        let mut state = match store.load_state(job_id) {
            Ok(value) => value,
            Err(source) => {
                return self
                    .read_only
                    .state_error_envelope("state_read_failed", source)
            }
        };
        let current_state = state_string(&state);
        let current_stage = string_field(&state, "current_stage")
            .unwrap_or("implement")
            .to_string();

        if current_state != "WAITING_APPROVAL" {
            return self.read_only.success_envelope(resume_skipped_payload(
                job_id,
                &current_state,
                &current_stage,
                &state,
            ));
        }

        let approval_request =
            match load_approval_request(store, job_id, &self.read_only.schema_root) {
                Ok(value) => value,
                Err(ControlArtifactError::Missing { path }) => {
                    return self.read_only.error_envelope(
                        "approval_request_missing",
                        "approval request artifact is required before resume",
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
        let approval_response =
            match load_approval_response(store, job_id, &self.read_only.schema_root) {
                Ok(value) => value,
                Err(ControlArtifactError::Missing { path }) => {
                    return self.read_only.error_envelope(
                        "approval_response_missing",
                        "approval response artifact is required before resume",
                        json!({ "path": path }),
                    )
                }
                Err(error) => {
                    return self.read_only.error_envelope(
                        "approval_response_invalid",
                        &error.to_string(),
                        json!({ "job_id": job_id }),
                    )
                }
            };
        if let Err(message) =
            ensure_approval_response_matches_request(&approval_request, &approval_response)
        {
            return self.read_only.error_envelope(
                "invalid_control_state",
                &message,
                json!({ "job_id": job_id }),
            );
        }

        let event_id = resume_event_id(job_id);
        let next_action = next_action_from_approval_response(&approval_response);
        if let Err(source) = update_state_for_control_command(
            &mut state,
            store,
            "VALIDATED",
            &current_stage,
            next_action,
            &event_id,
            None,
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
            resume_state_changed_event(event_id, &current_state, &current_stage, next_action),
        ) {
            return self
                .read_only
                .state_error_envelope("event_write_failed", source);
        }

        self.read_only
            .success_envelope(resume_success_payload(job_id, &current_state, next_action))
    }
}
