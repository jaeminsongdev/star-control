use super::ExecutionEngine;
use crate::constants::SCHEMA_VERSION;
use crate::contract::required_string;
use crate::error::ExecutionError;
use crate::state::{initial_state, push_history, set_object_field, state_for_provider_status};
use serde_json::{json, Value};
use star_control_provider::ProviderExecution;
use star_control_state::StateStoreError;
use std::path::Path;

impl<'a> ExecutionEngine<'a> {
    pub(super) fn update_run_state(
        &self,
        job: &Value,
        stage: &str,
        provider_execution: &ProviderExecution,
        attempt: &Value,
    ) -> Result<Value, ExecutionError> {
        let job_path = Path::new("job.json");
        let job_id = required_string(job, job_path, "job_id")?;
        let created_at = required_string(job, job_path, "created_at")?;
        let mut state = match self.state_store.load_state(&job_id) {
            Ok(state) => state,
            Err(StateStoreError::ArtifactNotFound { .. }) => {
                initial_state(&job_id, stage, &created_at)
            }
            Err(source) => return Err(ExecutionError::State(source)),
        };

        let result = provider_execution.result();
        let next_state = state_for_provider_status(stage, result.status());
        set_object_field(&mut state, "state", Value::String(next_state.to_string()))?;
        set_object_field(
            &mut state,
            "current_stage",
            Value::String(stage.to_string()),
        )?;
        set_object_field(&mut state, "updated_at", Value::String(created_at))?;
        set_object_field(&mut state, "active_provider", Value::Null)?;
        set_object_field(
            &mut state,
            "latest_event_id",
            Value::String(format!(
                "{}-{}-provider-finished",
                job_id.to_lowercase(),
                stage
            )),
        )?;

        self.state_store.register_artifact_ref(
            &mut state,
            &format!("{}_provider_request", stage),
            provider_execution.request_ref(),
        )?;
        self.state_store.register_artifact_ref(
            &mut state,
            &format!("{}_provider_response", stage),
            provider_execution.response_ref(),
        )?;
        self.state_store.register_artifact_ref(
            &mut state,
            &format!("{}_provider_stdout", stage),
            provider_execution.stdout_ref(),
        )?;
        if let Some(stderr_ref) = provider_execution.stderr_ref() {
            self.state_store.register_artifact_ref(
                &mut state,
                &format!("{}_provider_stderr", stage),
                stderr_ref,
            )?;
        }

        push_history(
            &mut state,
            json!({
                "stage": stage,
                "provider_instance_id": result.provider_instance_id(),
                "status": result.status(),
                "attempt": attempt
            }),
        )?;
        self.state_store.save_state(&job_id, &state)?;
        Ok(state)
    }

    pub(super) fn append_event(
        &self,
        job_id: &str,
        stage: &str,
        event_type: &str,
        message: &str,
        artifact_paths: &[String],
        details: Value,
    ) -> Result<(), ExecutionError> {
        let event = json!({
            "schema_version": SCHEMA_VERSION,
            "event_id": format!("{}-{}-{}", job_id.to_lowercase(), stage, event_type.to_lowercase().replace('_', "-")),
            "job_id": job_id,
            "type": event_type,
            "created_at": "execution:deterministic",
            "stage": stage,
            "state": "",
            "message": message,
            "artifact_paths": artifact_paths,
            "details": details
        });
        self.state_store.append_event(job_id, &event)?;
        Ok(())
    }
}
