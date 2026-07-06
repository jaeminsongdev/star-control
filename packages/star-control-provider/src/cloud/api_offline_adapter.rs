mod fixture;
mod output;

use super::api_live::execute_cloud_api_live_approval_required;
use super::manifest::is_cloud_api_manifest;
use super::preflight_adapter::CloudProviderPreflightAdapter;
use crate::cloud_io::{live_api_call_requested, offline_response_fixture_path};
use crate::cloud_sidecars::planned_output_files;
use crate::fake::ensure_output_files_absent;
use crate::{
    ExecutionRequest, ProviderAdapter, ProviderAdapterError, ProviderExecution, ProviderRunContext,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CloudApiOfflineProviderAdapter;

impl ProviderAdapter for CloudApiOfflineProviderAdapter {
    fn execute(
        &self,
        request: &ExecutionRequest,
        context: &ProviderRunContext<'_>,
    ) -> Result<ProviderExecution, ProviderAdapterError> {
        let manifest = context
            .registry()
            .manifest_for_instance(request.provider_instance_id())?;
        if !is_cloud_api_manifest(manifest) {
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
        let decision =
            crate::cloud_policy::CloudProviderPolicyDecision::evaluate(manifest, instance);
        if !decision.allows_transport_execution() {
            return CloudProviderPreflightAdapter.execute(request, context);
        }
        let Some(fixture_relative_path) = offline_response_fixture_path(instance)? else {
            if live_api_call_requested(instance)? {
                return execute_cloud_api_live_approval_required(
                    request, context, manifest, instance,
                );
            }
            return CloudProviderPreflightAdapter.execute(request, context);
        };

        ensure_output_files_absent(
            context.state_store(),
            request.job_id(),
            &planned_output_files(request.provider_instance_id()),
        )?;

        let fixture = fixture::prepare_offline_fixture(
            request,
            context,
            manifest,
            instance,
            fixture_relative_path,
        )?;
        output::write_offline_execution(request, context, manifest, instance, &fixture)
    }
}
