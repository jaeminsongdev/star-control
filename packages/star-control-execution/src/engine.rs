use crate::constants::EXECUTION_ATTEMPT_SCHEMA;
use crate::contract::{execution_attempt, validate_contract, verify_provider_result};
use crate::error::ExecutionError;
use crate::types::{ExecutionOutcome, ProviderAssignment};
use serde_json::json;
use star_control_provider::{
    CloudApiOfflineProviderAdapter, CloudCliProviderAdapter, CloudProviderPreflightAdapter,
    FakeProviderAdapter, LocalOpenAiCompatibleServerAdapter, LocalProcessProviderAdapter,
    ProviderRegistry, ProviderRegistryError, ProviderRunContext,
};
use star_control_state::StateStore;
use std::path::{Path, PathBuf};

mod provider;
mod request;
mod state;

#[derive(Debug, Clone)]
pub struct ExecutionEngine<'a> {
    state_store: &'a StateStore,
    registry: &'a ProviderRegistry,
    schema_root: PathBuf,
    fake_adapter: FakeProviderAdapter,
    local_process_adapter: LocalProcessProviderAdapter,
    local_openai_compatible_adapter: LocalOpenAiCompatibleServerAdapter,
    cloud_cli_adapter: CloudCliProviderAdapter,
    cloud_api_adapter: CloudApiOfflineProviderAdapter,
    cloud_provider_adapter: CloudProviderPreflightAdapter,
}

impl<'a> ExecutionEngine<'a> {
    pub fn new(
        state_store: &'a StateStore,
        registry: &'a ProviderRegistry,
        schema_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            state_store,
            registry,
            schema_root: schema_root.into(),
            fake_adapter: FakeProviderAdapter::success(),
            local_process_adapter: LocalProcessProviderAdapter,
            local_openai_compatible_adapter: LocalOpenAiCompatibleServerAdapter,
            cloud_cli_adapter: CloudCliProviderAdapter,
            cloud_api_adapter: CloudApiOfflineProviderAdapter,
            cloud_provider_adapter: CloudProviderPreflightAdapter,
        }
    }

    pub fn with_fake_adapter(mut self, adapter: FakeProviderAdapter) -> Self {
        self.fake_adapter = adapter;
        self
    }

    pub fn execute_stage(
        &self,
        job_id: &str,
        stage: &str,
    ) -> Result<ExecutionOutcome, ExecutionError> {
        let job = self.state_store.load_job(job_id)?;
        let workspec = self.state_store.load_workspec(job_id, stage)?;
        let assignment = ProviderAssignment::from_workspec(&workspec, stage)?;
        self.registry
            .instance(&assignment.provider_instance)
            .ok_or_else(|| ProviderRegistryError::InstanceNotFound {
                instance_id: assignment.provider_instance.clone(),
            })?;
        self.ensure_stage_not_executed(job_id, stage, &assignment.provider_instance)?;

        let request = self.execution_request(&job, &workspec, &assignment)?;
        let attempt = execution_attempt(&request, "running");
        validate_contract(
            &attempt,
            Path::new("execution-attempt.json"),
            &self.schema_root,
            EXECUTION_ATTEMPT_SCHEMA,
        )?;

        self.append_event(
            job_id,
            stage,
            "PROVIDER_STARTED",
            "Provider execution started",
            &[format!(
                "provider-output/{}/request.json",
                request.provider_instance_id()
            )],
            json!({
                "provider_instance_id": request.provider_instance_id(),
                "attempt_id": attempt["attempt_id"]
            }),
        )?;

        let context = ProviderRunContext::new(self.registry, self.state_store, &self.schema_root);
        let provider_execution = self.execute_provider(&request, &context)?;
        verify_provider_result(&request, &provider_execution)?;

        let completed_attempt = execution_attempt(&request, provider_execution.result().status());
        validate_contract(
            &completed_attempt,
            Path::new("execution-attempt.json"),
            &self.schema_root,
            EXECUTION_ATTEMPT_SCHEMA,
        )?;
        let state = self.update_run_state(&job, stage, &provider_execution, &completed_attempt)?;

        self.append_event(
            job_id,
            stage,
            "PROVIDER_FINISHED",
            "Provider execution finished",
            &[
                format!(
                    "provider-output/{}/request.json",
                    request.provider_instance_id()
                ),
                format!(
                    "provider-output/{}/response.json",
                    request.provider_instance_id()
                ),
            ],
            json!({
                "provider_instance_id": request.provider_instance_id(),
                "attempt_id": completed_attempt["attempt_id"],
                "status": provider_execution.result().status()
            }),
        )?;

        Ok(ExecutionOutcome {
            request,
            provider_execution,
            attempt: completed_attempt,
            state,
        })
    }
}
