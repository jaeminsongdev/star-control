use super::ExecutionEngine;
use crate::constants::{FAKE_PROVIDER_ID, LOCAL_PROCESS_KIND, PROCESS_TRANSPORT};
use crate::error::ExecutionError;
use star_control_provider::{
    is_cloud_api_manifest, is_cloud_cli_manifest, is_cloud_provider_manifest,
    is_local_openai_compatible_manifest, ExecutionRequest, ProviderAdapter, ProviderAdapterError,
    ProviderExecution, ProviderRunContext,
};

impl<'a> ExecutionEngine<'a> {
    pub(super) fn execute_provider(
        &self,
        request: &ExecutionRequest,
        context: &ProviderRunContext<'_>,
    ) -> Result<ProviderExecution, ExecutionError> {
        let manifest = self
            .registry
            .manifest_for_instance(request.provider_instance_id())?;
        if manifest.id() == FAKE_PROVIDER_ID {
            return Ok(self.fake_adapter.execute(request, context)?);
        }
        if manifest.kind() == LOCAL_PROCESS_KIND && manifest.transport() == PROCESS_TRANSPORT {
            return Ok(self.local_process_adapter.execute(request, context)?);
        }
        if is_local_openai_compatible_manifest(manifest) {
            return Ok(self
                .local_openai_compatible_adapter
                .execute(request, context)?);
        }
        if is_cloud_cli_manifest(manifest) {
            return Ok(self.cloud_cli_adapter.execute(request, context)?);
        }
        if is_cloud_api_manifest(manifest) {
            return Ok(self.cloud_api_adapter.execute(request, context)?);
        }
        if is_cloud_provider_manifest(manifest) {
            return Ok(self.cloud_provider_adapter.execute(request, context)?);
        }

        Err(ProviderAdapterError::UnsupportedProvider {
            provider_instance_id: request.provider_instance_id().to_string(),
            provider_id: manifest.id().to_string(),
        }
        .into())
    }
}
