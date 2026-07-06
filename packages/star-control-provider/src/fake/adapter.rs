use super::model::{
    ExecutionRequest, ProviderAdapter, ProviderAdapterError, ProviderExecution, ProviderRunContext,
    ProviderRunResult,
};
use super::output::{ensure_output_files_absent, planned_output_files, provider_output_path};
use super::simulation::FakeProviderSimulation;
use serde_json::{json, Value};

const FAKE_PROVIDER_ID: &str = "provider.fake";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeProviderAdapter {
    simulation: FakeProviderSimulation,
}

impl FakeProviderAdapter {
    pub fn success() -> Self {
        Self {
            simulation: FakeProviderSimulation::Success,
        }
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            simulation: FakeProviderSimulation::Failed(message.into()),
        }
    }

    pub fn blocked(reason: impl Into<String>) -> Self {
        Self {
            simulation: FakeProviderSimulation::Blocked(reason.into()),
        }
    }

    pub fn simulation(&self) -> &FakeProviderSimulation {
        &self.simulation
    }
}

impl Default for FakeProviderAdapter {
    fn default() -> Self {
        Self::success()
    }
}

impl ProviderAdapter for FakeProviderAdapter {
    fn execute(
        &self,
        request: &ExecutionRequest,
        context: &ProviderRunContext<'_>,
    ) -> Result<ProviderExecution, ProviderAdapterError> {
        let manifest = context
            .registry()
            .manifest_for_instance(request.provider_instance_id())?;
        if manifest.id() != FAKE_PROVIDER_ID {
            return Err(ProviderAdapterError::UnsupportedProvider {
                provider_instance_id: request.provider_instance_id().to_string(),
                provider_id: manifest.id().to_string(),
            });
        }

        let output_files = planned_output_files(
            request.provider_instance_id(),
            self.simulation.stderr().is_some(),
        );
        ensure_output_files_absent(context.state_store(), request.job_id(), &output_files)?;

        let response_value = self.response_value(request);
        let result = ProviderRunResult::from_value(
            response_value.clone(),
            format!(
                "provider-output/{}/response.json",
                request.provider_instance_id()
            ),
            context.schema_root(),
        )?;

        let request_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            "request.json",
            request.value(),
        )?;
        let stdout_ref = context.state_store().write_provider_text(
            request.job_id(),
            request.provider_instance_id(),
            "stdout.txt",
            &self.simulation.stdout(),
        )?;
        let stderr_content = self.simulation.stderr();
        let stderr_ref = if let Some(stderr) = stderr_content {
            Some(context.state_store().write_provider_text(
                request.job_id(),
                request.provider_instance_id(),
                "stderr.txt",
                &stderr,
            )?)
        } else {
            None
        };
        let response_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            "response.json",
            &response_value,
        )?;

        Ok(ProviderExecution::new(
            result,
            request_ref,
            response_ref,
            stdout_ref,
            stderr_ref,
        ))
    }
}

impl FakeProviderAdapter {
    fn response_value(&self, request: &ExecutionRequest) -> Value {
        let stderr_path = if self.simulation.stderr().is_some() {
            Value::String(provider_output_path(
                request.provider_instance_id(),
                "stderr.txt",
            ))
        } else {
            Value::Null
        };

        json!({
            "schema_version": "1.0.0",
            "provider_instance_id": request.provider_instance_id(),
            "job_id": request.job_id(),
            "stage": request.stage(),
            "status": self.simulation.status(),
            "started_at": request.created_at(),
            "finished_at": request.created_at(),
            "stdout_path": provider_output_path(request.provider_instance_id(), "stdout.txt"),
            "stderr_path": stderr_path,
            "summary": self.simulation.summary(),
            "changed_files": [],
            "artifacts": [
                provider_output_path(request.provider_instance_id(), "response.json")
            ],
            "metrics": {
                "estimated_cost": 0,
                "input_tokens": 0,
                "output_tokens": 0
            },
            "error": self.simulation.error()
        })
    }
}
