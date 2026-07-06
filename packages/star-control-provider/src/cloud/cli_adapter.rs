use super::manifest::is_cloud_cli_manifest;
use super::preflight_adapter::CloudProviderPreflightAdapter;
use crate::cloud_cli::{run_cloud_cli_process, CloudCliCommandPolicy};
use crate::cloud_constants::*;
use crate::cloud_io::{create_new_output_file, validate_contract};
use crate::cloud_policy::CloudProviderPolicyDecision;
use crate::cloud_sidecars::{
    artifact_ref, assert_provider_sidecar_refs, cli_response_value,
    cost_metric_value_with_wall_time, planned_output_files, privacy_handoff_value,
};
use crate::fake::{ensure_output_files_absent, provider_output_path};
use crate::{
    ExecutionRequest, ProviderAdapter, ProviderAdapterError, ProviderExecution, ProviderRunContext,
    ProviderRunResult,
};
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CloudCliProviderAdapter;

impl ProviderAdapter for CloudCliProviderAdapter {
    fn execute(
        &self,
        request: &ExecutionRequest,
        context: &ProviderRunContext<'_>,
    ) -> Result<ProviderExecution, ProviderAdapterError> {
        let manifest = context
            .registry()
            .manifest_for_instance(request.provider_instance_id())?;
        if !is_cloud_cli_manifest(manifest) {
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
        let decision = CloudProviderPolicyDecision::evaluate(manifest, instance);
        if !decision.allows_transport_execution() {
            return CloudProviderPreflightAdapter.execute(request, context);
        }

        let policy = CloudCliCommandPolicy::from_instance(instance)?;

        ensure_output_files_absent(
            context.state_store(),
            request.job_id(),
            &planned_output_files(request.provider_instance_id()),
        )?;

        let request_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            REQUEST_FILE,
            request.value(),
        )?;
        let privacy_handoff = privacy_handoff_value(request, manifest, true);
        validate_contract(
            &privacy_handoff,
            Path::new(PRIVACY_HANDOFF_FILE),
            context.schema_root(),
            PRIVACY_HANDOFF_SCHEMA,
        )?;
        let privacy_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            PRIVACY_HANDOFF_FILE,
            &privacy_handoff,
        )?;

        let output_dir = context
            .state_store()
            .resolve_provider_output_dir(request.job_id(), request.provider_instance_id())?;
        std::fs::create_dir_all(&output_dir).map_err(|source| ProviderAdapterError::Io {
            path: output_dir.clone(),
            source,
        })?;
        let stdout_path = output_dir.join(STDOUT_FILE);
        let stderr_path = output_dir.join(STDERR_FILE);
        let stdout_file = create_new_output_file(&stdout_path)?;
        let stderr_file = create_new_output_file(&stderr_path)?;

        let started_at = Instant::now();
        let process_result = run_cloud_cli_process(
            &policy,
            request,
            context,
            &request_ref,
            stdout_file,
            stderr_file,
        )?;
        let wall_time_ms = started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;

        let stdout_ref = artifact_ref(context, request, STDOUT_FILE)?;
        let stderr_ref = artifact_ref(context, request, STDERR_FILE)?;
        let cost_metric = cost_metric_value_with_wall_time(request, instance, wall_time_ms);
        validate_contract(
            &cost_metric,
            Path::new(COST_METRIC_FILE),
            context.schema_root(),
            COST_METRIC_SCHEMA,
        )?;
        let cost_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            COST_METRIC_FILE,
            &cost_metric,
        )?;

        let response_value =
            cli_response_value(request, manifest, instance, &process_result, wall_time_ms);
        let result = ProviderRunResult::from_value(
            response_value.clone(),
            provider_output_path(request.provider_instance_id(), RESPONSE_FILE),
            context.schema_root(),
        )?;
        let response_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            RESPONSE_FILE,
            &response_value,
        )?;

        let execution = ProviderExecution::new(
            result,
            request_ref,
            response_ref,
            stdout_ref,
            Some(stderr_ref),
        );
        assert_provider_sidecar_refs(&execution, &privacy_ref, &cost_ref);
        Ok(execution)
    }
}
