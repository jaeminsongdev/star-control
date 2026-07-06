use super::ExecutionEngine;
use crate::constants::SCHEMA_VERSION;
use crate::contract::required_string;
use crate::error::ExecutionError;
use crate::types::ProviderAssignment;
use serde_json::{json, Value};
use star_control_provider::ExecutionRequest;
use std::path::Path;

impl<'a> ExecutionEngine<'a> {
    pub(super) fn execution_request(
        &self,
        job: &Value,
        workspec: &Value,
        assignment: &ProviderAssignment,
    ) -> Result<ExecutionRequest, ExecutionError> {
        let job_path = Path::new("job.json");
        let workspec_path = Path::new("workspec.json");
        let job_id = required_string(job, job_path, "job_id")?;
        let stage = required_string(workspec, workspec_path, "stage")?;
        let created_at = required_string(job, job_path, "created_at")?;
        let goal = required_string(workspec, workspec_path, "goal")?;

        let request_value = json!({
            "schema_version": SCHEMA_VERSION,
            "request_id": format!("{}-{}-request-0001", job_id.to_lowercase(), stage),
            "job_id": job_id,
            "stage": stage,
            "provider_instance_id": assignment.provider_instance,
            "attempt_id": "attempt-0001",
            "workspec_path": format!("workspecs/{}.json", stage),
            "created_at": created_at,
            "goal": goal,
            "allowed_scope": workspec.get("allowed_scope").cloned().unwrap_or_else(|| json!([])),
            "forbidden_actions": workspec
                .get("forbidden_actions")
                .cloned()
                .unwrap_or_else(|| json!([])),
            "required_outputs": workspec
                .get("required_outputs")
                .cloned()
                .unwrap_or_else(|| json!([])),
            "validation_requirements": workspec
                .get("validation_requirements")
                .cloned()
                .unwrap_or_else(|| json!([])),
            "context_pack": workspec
                .get("context_pack")
                .cloned()
                .unwrap_or_else(|| json!({}))
        });

        ExecutionRequest::from_value(request_value, "execution-request.json", &self.schema_root)
            .map_err(ExecutionError::from)
    }

    pub(super) fn ensure_stage_not_executed(
        &self,
        job_id: &str,
        stage: &str,
        provider_instance_id: &str,
    ) -> Result<(), ExecutionError> {
        let response_path = self.state_store.resolve_job_path(
            job_id,
            &format!("provider-output/{}/response.json", provider_instance_id),
        )?;
        if response_path.exists() {
            return Err(ExecutionError::StageAlreadyExecuted {
                job_id: job_id.to_string(),
                stage: stage.to_string(),
                provider_instance_id: provider_instance_id.to_string(),
            });
        }
        Ok(())
    }
}
