use super::manifest::is_cloud_provider_manifest;
use crate::cloud_constants::*;
use crate::cloud_io::validate_contract;
use crate::cloud_policy::CloudProviderPolicyDecision;
use crate::cloud_sidecars::{
    assert_provider_sidecar_refs, cost_metric_value, planned_output_files, privacy_handoff_value,
    response_value, stderr_value, stdout_value,
};
use crate::fake::{ensure_output_files_absent, provider_output_path};
use crate::{
    ExecutionRequest, ProviderAdapter, ProviderAdapterError, ProviderExecution, ProviderRunContext,
    ProviderRunResult,
};
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CloudProviderPreflightAdapter;

impl ProviderAdapter for CloudProviderPreflightAdapter {
    fn execute(
        &self,
        request: &ExecutionRequest,
        context: &ProviderRunContext<'_>,
    ) -> Result<ProviderExecution, ProviderAdapterError> {
        let manifest = context
            .registry()
            .manifest_for_instance(request.provider_instance_id())?;
        if !is_cloud_provider_manifest(manifest) {
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
        let privacy_handoff = privacy_handoff_value(request, manifest, decision.privacy_approved);
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

        let cost_metric = cost_metric_value(request, instance);
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

        let stdout_ref = context.state_store().write_provider_text(
            request.job_id(),
            request.provider_instance_id(),
            STDOUT_FILE,
            &stdout_value(manifest, &decision),
        )?;
        let stderr_ref = context.state_store().write_provider_text(
            request.job_id(),
            request.provider_instance_id(),
            STDERR_FILE,
            &stderr_value(&decision),
        )?;

        let response_value = response_value(request, manifest, instance, &decision);
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
