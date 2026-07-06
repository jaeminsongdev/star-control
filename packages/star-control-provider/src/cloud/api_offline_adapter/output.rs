use super::fixture::OfflineFixture;
use crate::cloud_api_artifacts::{api_offline_response_value, api_offline_stdout_value};
use crate::cloud_constants::*;
use crate::cloud_io::validate_contract;
use crate::cloud_sidecars::{
    assert_provider_sidecar_refs, cost_metric_value_with_response_usage, privacy_handoff_value,
};
use crate::fake::provider_output_path;
use crate::{
    ExecutionRequest, ProviderAdapterError, ProviderExecution, ProviderInstance, ProviderManifest,
    ProviderRunContext, ProviderRunResult,
};
use std::path::Path;

pub(super) fn write_offline_execution(
    request: &ExecutionRequest,
    context: &ProviderRunContext<'_>,
    manifest: &ProviderManifest,
    instance: &ProviderInstance,
    fixture: &OfflineFixture,
) -> Result<ProviderExecution, ProviderAdapterError> {
    let request_ref = context.state_store().write_provider_json(
        request.job_id(),
        request.provider_instance_id(),
        REQUEST_FILE,
        request.value(),
    )?;
    let http_request_ref = context.state_store().write_provider_json(
        request.job_id(),
        request.provider_instance_id(),
        HTTP_REQUEST_FILE,
        &fixture.http_request_value,
    )?;
    let http_transport_plan_ref = context.state_store().write_provider_json(
        request.job_id(),
        request.provider_instance_id(),
        HTTP_TRANSPORT_PLAN_FILE,
        &fixture.http_transport_plan,
    )?;
    let raw_response_ref = context.state_store().write_provider_json(
        request.job_id(),
        request.provider_instance_id(),
        RAW_RESPONSE_FILE,
        &fixture.raw_response,
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

    let cost_metric = cost_metric_value_with_response_usage(
        request,
        instance,
        &fixture.parsed_response,
        fixture.wall_time_ms,
    );
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
        &api_offline_stdout_value(
            manifest,
            &fixture.prepared_request,
            &fixture.fixture_relative_path,
        ),
    )?;
    let stderr_ref = context.state_store().write_provider_text(
        request.job_id(),
        request.provider_instance_id(),
        STDERR_FILE,
        "cloud API offline fixture completed without live API call\n",
    )?;

    let response_value = api_offline_response_value(
        request,
        manifest,
        instance,
        &fixture.prepared_request,
        &fixture.parsed_response,
        fixture.wall_time_ms,
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
        stdout_ref,
        Some(stderr_ref),
    );
    assert_provider_sidecar_refs(&execution, &privacy_ref, &cost_ref);
    debug_assert_eq!(http_request_ref["kind"], "provider_output");
    debug_assert_eq!(http_transport_plan_ref["kind"], "provider_output");
    debug_assert_eq!(raw_response_ref["kind"], "provider_output");
    Ok(execution)
}
