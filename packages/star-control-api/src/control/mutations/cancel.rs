use super::super::helpers::{
    append_api_event, state_string, string_field, update_state_for_control_command, ApiControlEvent,
};
use super::super::ApiControlService;
use crate::constants::TERMINAL_STATES;
use crate::error::ApiError;
use serde_json::{json, Value};

impl ApiControlService {
    pub(in crate::control) fn cancel_response(
        &self,
        project_id: &str,
        job_id: &str,
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
        if TERMINAL_STATES.contains(&current_state.as_str()) {
            return self.read_only.error_envelope(
                "invalid_control_state",
                "cannot cancel terminal job state",
                json!({ "job_id": job_id, "state": current_state }),
            );
        }
        let current_stage = string_field(&state, "current_stage")
            .unwrap_or("implement")
            .to_string();
        let event_id = format!("{}-api-cancelled", job_id.to_ascii_lowercase());
        if let Err(source) = update_state_for_control_command(
            &mut state,
            store,
            "CANCELLED",
            &current_stage,
            "stop",
            &event_id,
            None,
        ) {
            return self
                .read_only
                .state_error_envelope("state_update_failed", source);
        }
        if let Some(state_object) = state.as_object_mut() {
            state_object.insert("active_provider".to_string(), Value::Null);
        }
        if let Err(source) = store.save_state(job_id, &state) {
            return self
                .read_only
                .state_error_envelope("state_write_failed", source);
        }
        if let Err(source) = append_api_event(
            store,
            job_id,
            ApiControlEvent {
                event_id,
                event_type: "STATE_CHANGED",
                state: "CANCELLED",
                stage: &current_stage,
                message: "Job cancelled by API",
                artifact_paths: vec!["run-state.json".to_string()],
                details: json!({ "previous_state": current_state }),
            },
        ) {
            return self
                .read_only
                .state_error_envelope("event_write_failed", source);
        }

        self.read_only.success_envelope(json!({
            "command": "cancel",
            "job_id": job_id,
            "state": "CANCELLED",
            "previous_state": current_state,
            "next_action": "stop",
            "artifacts": [format!(".ai-runs/{}/run-state.json", job_id)]
        }))
    }
}
