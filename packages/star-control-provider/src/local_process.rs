mod constants;
mod evidence;
mod policy;
mod runner;
mod sidecars;

use crate::fake::{ensure_output_files_absent, provider_output_path};
use crate::provider_cost::{validate_cost_metric, zero_cost_metric_value, COST_METRIC_FILE};
use crate::provider_redaction::{
    redact_provider_json_artifact, redact_provider_text_file_artifact,
};
use crate::{
    ExecutionRequest, ProviderAdapter, ProviderAdapterError, ProviderExecution, ProviderRunContext,
    ProviderRunResult,
};
use constants::{LOCAL_PROCESS_KIND, PROCESS_TRANSPORT, STDERR_FILE, STDOUT_FILE};
use evidence::forbidden_action_evidence;
pub use policy::LocalProcessCommandPolicy;
use runner::{run_process, LocalProcessRunResult};
use sidecars::{artifact_ref, create_new_output_file, planned_output_files, response_value};
use std::fs;
use std::time::Instant;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalProcessProviderAdapter;

impl ProviderAdapter for LocalProcessProviderAdapter {
    fn execute(
        &self,
        request: &ExecutionRequest,
        context: &ProviderRunContext<'_>,
    ) -> Result<ProviderExecution, ProviderAdapterError> {
        let manifest = context
            .registry()
            .manifest_for_instance(request.provider_instance_id())?;
        if manifest.kind() != LOCAL_PROCESS_KIND || manifest.transport() != PROCESS_TRANSPORT {
            return Err(ProviderAdapterError::UnsupportedProvider {
                provider_instance_id: request.provider_instance_id().to_string(),
                provider_id: manifest.id().to_string(),
            });
        }

        let instance = context
            .registry()
            .instance(request.provider_instance_id())
            .ok_or_else(|| crate::ProviderRegistryError::InstanceNotFound {
                instance_id: request.provider_instance_id().to_string(),
            })?;
        let policy = LocalProcessCommandPolicy::from_instance(instance)?;

        let output_files = planned_output_files(request.provider_instance_id());
        ensure_output_files_absent(context.state_store(), request.job_id(), &output_files)?;

        let request_redaction =
            redact_provider_json_artifact(context, request, "request.json", request.value())?;
        let request_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            "request.json",
            request_redaction.value(),
        )?;
        let output_dir = context
            .state_store()
            .resolve_provider_output_dir(request.job_id(), request.provider_instance_id())?;
        fs::create_dir_all(&output_dir).map_err(|source| ProviderAdapterError::Io {
            path: output_dir.clone(),
            source,
        })?;

        let stdout_path = output_dir.join(STDOUT_FILE);
        let stderr_path = output_dir.join(STDERR_FILE);
        let stdout_file = create_new_output_file(&stdout_path)?;
        let stderr_file = create_new_output_file(&stderr_path)?;

        let started_at = Instant::now();
        let mut process_result = run_process(&policy, request, context, stdout_file, stderr_file)?;
        let wall_time_ms = started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        if matches!(process_result, LocalProcessRunResult::Exited { .. }) {
            if let Some(evidence) = forbidden_action_evidence(request, &stdout_path, &stderr_path)?
            {
                process_result = LocalProcessRunResult::BlockedForbiddenAction { evidence };
            }
        }
        let stdout_redaction =
            redact_provider_text_file_artifact(context, request, STDOUT_FILE, &stdout_path)?;
        let stderr_redaction =
            redact_provider_text_file_artifact(context, request, STDERR_FILE, &stderr_path)?;
        let redaction_artifacts = [
            request_redaction.report_path().map(ToString::to_string),
            stdout_redaction,
            stderr_redaction,
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        let response_value = response_value(
            request,
            &policy,
            &process_result,
            wall_time_ms,
            &redaction_artifacts,
        );
        let response_redaction =
            redact_provider_json_artifact(context, request, "response.json", &response_value)?;
        let result = ProviderRunResult::from_value(
            response_redaction.value().clone(),
            provider_output_path(request.provider_instance_id(), "response.json"),
            context.schema_root(),
        )?;

        let stdout_ref = artifact_ref(
            context,
            request,
            &provider_output_path(request.provider_instance_id(), STDOUT_FILE),
        )?;
        let stderr_ref = artifact_ref(
            context,
            request,
            &provider_output_path(request.provider_instance_id(), STDERR_FILE),
        )?;
        let cost_metric = zero_cost_metric_value(request, wall_time_ms);
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
            Some(stderr_ref),
        ))
    }
}

#[cfg(test)]
mod tests;
