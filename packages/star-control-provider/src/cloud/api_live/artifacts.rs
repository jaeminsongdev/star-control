use crate::cloud_api_artifacts::{
    http_transport_plan_value, live_transport_approval_value, prepared_request_value,
};
use crate::cloud_constants::{
    HTTP_REQUEST_FILE, HTTP_TRANSPORT_PLAN_FILE, LIVE_TRANSPORT_APPROVAL_FILE,
};
use crate::cloud_policy::cloud_policy_denied;
use crate::{
    ExecutionRequest, OpenAiCompatiblePreparedRequest, OpenAiCompatibleRequestBuilder,
    ProviderAdapterError, ProviderInstance, ProviderManifest, ProviderRunContext,
};
use serde_json::Value;
use std::time::Instant;

pub(super) struct LiveApprovalArtifacts {
    pub(super) prepared_request: OpenAiCompatiblePreparedRequest,
    pub(super) http_request_value: Value,
    pub(super) http_transport_plan: Value,
    pub(super) live_approval: Value,
    pub(super) wall_time_ms: u64,
}

pub(super) fn prepare_live_approval_artifacts(
    request: &ExecutionRequest,
    manifest: &ProviderManifest,
    instance: &ProviderInstance,
) -> Result<LiveApprovalArtifacts, ProviderAdapterError> {
    let started_at = Instant::now();
    let prepared_request = OpenAiCompatibleRequestBuilder
        .build(request, instance)
        .map_err(|source| {
            cloud_policy_denied(
                instance.id(),
                &format!("OpenAI-compatible request build failed: {}", source),
            )
        })?;
    let http_request_value = prepared_request_value(&prepared_request);
    let http_transport_plan = http_transport_plan_value(
        request,
        manifest,
        instance,
        &prepared_request,
        "live_approval_required",
        false,
    )?;
    let live_approval =
        live_transport_approval_value(request, manifest, instance, &prepared_request)?;
    let wall_time_ms = started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;

    Ok(LiveApprovalArtifacts {
        prepared_request,
        http_request_value,
        http_transport_plan,
        live_approval,
        wall_time_ms,
    })
}

pub(super) fn write_live_approval_plan_artifacts(
    request: &ExecutionRequest,
    context: &ProviderRunContext<'_>,
    artifacts: &LiveApprovalArtifacts,
) -> Result<(), ProviderAdapterError> {
    let http_request_ref = context.state_store().write_provider_json(
        request.job_id(),
        request.provider_instance_id(),
        HTTP_REQUEST_FILE,
        &artifacts.http_request_value,
    )?;
    let http_transport_plan_ref = context.state_store().write_provider_json(
        request.job_id(),
        request.provider_instance_id(),
        HTTP_TRANSPORT_PLAN_FILE,
        &artifacts.http_transport_plan,
    )?;
    let live_approval_ref = context.state_store().write_provider_json(
        request.job_id(),
        request.provider_instance_id(),
        LIVE_TRANSPORT_APPROVAL_FILE,
        &artifacts.live_approval,
    )?;

    debug_assert_eq!(http_request_ref["kind"], "provider_output");
    debug_assert_eq!(http_transport_plan_ref["kind"], "provider_output");
    debug_assert_eq!(live_approval_ref["kind"], "provider_output");
    Ok(())
}
