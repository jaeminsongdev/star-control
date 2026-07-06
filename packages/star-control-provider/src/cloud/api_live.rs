mod artifacts;
mod sidecars;

use artifacts::{prepare_live_approval_artifacts, write_live_approval_plan_artifacts};
use sidecars::write_live_approval_sidecars;

use crate::cloud_api_artifacts::api_live_approval_response_value;
use crate::cloud_constants::*;
use crate::cloud_sidecars::{assert_provider_sidecar_refs, planned_output_files};
use crate::fake::{ensure_output_files_absent, provider_output_path};
use crate::{
    ExecutionRequest, ProviderAdapterError, ProviderExecution, ProviderInstance, ProviderManifest,
    ProviderRunContext, ProviderRunResult,
};

pub(super) fn execute_cloud_api_live_approval_required(
    request: &ExecutionRequest,
    context: &ProviderRunContext<'_>,
    manifest: &ProviderManifest,
    instance: &ProviderInstance,
) -> Result<ProviderExecution, ProviderAdapterError> {
    ensure_output_files_absent(
        context.state_store(),
        request.job_id(),
        &planned_output_files(request.provider_instance_id()),
    )?;

    let live_artifacts = prepare_live_approval_artifacts(request, manifest, instance)?;

    let request_ref = context.state_store().write_provider_json(
        request.job_id(),
        request.provider_instance_id(),
        REQUEST_FILE,
        request.value(),
    )?;
    write_live_approval_plan_artifacts(request, context, &live_artifacts)?;
    let sidecars =
        write_live_approval_sidecars(request, context, manifest, instance, &live_artifacts)?;

    let response_value = api_live_approval_response_value(
        request,
        manifest,
        instance,
        &live_artifacts.prepared_request,
        live_artifacts.wall_time_ms,
    );
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
        sidecars.stdout_ref,
        Some(sidecars.stderr_ref),
    );
    assert_provider_sidecar_refs(&execution, &sidecars.privacy_ref, &sidecars.cost_ref);
    Ok(execution)
}
