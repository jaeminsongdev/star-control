use super::model::{
    ExecutionRequest, ProviderAdapter, ProviderAdapterError, ProviderExecution, ProviderRunContext,
    ProviderRunResult,
};
use super::output::{ensure_output_files_absent, planned_output_files, provider_output_path};
use super::simulation::FakeProviderSimulation;
use crate::provider_cost::{validate_cost_metric, zero_cost_metric_value, COST_METRIC_FILE};
use crate::provider_redaction::{redact_provider_json_artifact, redact_provider_text_artifact};
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

        let request_redaction =
            redact_provider_json_artifact(context, request, "request.json", request.value())?;
        let stdout_content = self.simulation.stdout();
        let stdout_redaction =
            redact_provider_text_artifact(context, request, "stdout.txt", &stdout_content)?;
        let stderr_content = self.simulation.stderr();
        let stderr_redaction = stderr_content
            .as_ref()
            .map(|stderr| redact_provider_text_artifact(context, request, "stderr.txt", stderr))
            .transpose()?;
        let redaction_artifacts = redaction_artifacts([
            request_redaction.report_path(),
            stdout_redaction.report_path(),
            stderr_redaction
                .as_ref()
                .and_then(|redaction| redaction.report_path()),
        ]);
        let response_value = self.response_value(request, &redaction_artifacts);
        let response_redaction =
            redact_provider_json_artifact(context, request, "response.json", &response_value)?;
        let result = ProviderRunResult::from_value(
            response_redaction.value().clone(),
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
            request_redaction.value(),
        )?;
        let stdout_ref = context.state_store().write_provider_text(
            request.job_id(),
            request.provider_instance_id(),
            "stdout.txt",
            stdout_redaction.content(),
        )?;
        let stderr_ref = if let Some(stderr) = stderr_redaction {
            Some(context.state_store().write_provider_text(
                request.job_id(),
                request.provider_instance_id(),
                "stderr.txt",
                stderr.content(),
            )?)
        } else {
            None
        };
        let cost_metric = zero_cost_metric_value(request, 0);
        validate_cost_metric(&cost_metric, context.schema_root())?;
        let cost_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            COST_METRIC_FILE,
            &cost_metric,
        )?;
        let response_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            "response.json",
            response_redaction.value(),
        )?;
        debug_assert_eq!(cost_ref["kind"], "provider_output");

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
    fn response_value(&self, request: &ExecutionRequest, redaction_artifacts: &[String]) -> Value {
        let stderr_path = if self.simulation.stderr().is_some() {
            Value::String(provider_output_path(
                request.provider_instance_id(),
                "stderr.txt",
            ))
        } else {
            Value::Null
        };

        let mut artifacts = vec![
            provider_output_path(request.provider_instance_id(), "response.json"),
            provider_output_path(request.provider_instance_id(), COST_METRIC_FILE),
        ];
        artifacts.extend(redaction_artifacts.iter().cloned());

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
            "artifacts": artifacts,
            "metrics": {
                "estimated_cost": 0,
                "currency": "USD",
                "input_tokens": 0,
                "output_tokens": 0,
                "wall_time_ms": 0
            },
            "error": self.simulation.error()
        })
    }
}

fn redaction_artifacts<'a>(paths: impl IntoIterator<Item = Option<&'a str>>) -> Vec<String> {
    paths
        .into_iter()
        .flatten()
        .map(ToString::to_string)
        .collect()
}
