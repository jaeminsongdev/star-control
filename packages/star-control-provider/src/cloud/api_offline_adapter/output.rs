use super::fixture::OfflineFixture;
use crate::cloud_api_artifacts::{api_offline_response_value, api_offline_stdout_value};
use crate::cloud_constants::*;
use crate::cloud_io::validate_contract;
use crate::cloud_sidecars::{
    assert_provider_sidecar_refs, cost_metric_value_with_response_usage, privacy_handoff_value,
};
use crate::fake::provider_output_path;
use crate::provider_redaction::{redact_provider_json_artifact, redact_provider_text_artifact};
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
    let request_redaction =
        redact_provider_json_artifact(context, request, REQUEST_FILE, request.value())?;
    let request_ref = context.state_store().write_provider_json(
        request.job_id(),
        request.provider_instance_id(),
        REQUEST_FILE,
        request_redaction.value(),
    )?;
    let http_request_redaction = redact_provider_json_artifact(
        context,
        request,
        HTTP_REQUEST_FILE,
        &fixture.http_request_value,
    )?;
    let http_request_ref = context.state_store().write_provider_json(
        request.job_id(),
        request.provider_instance_id(),
        HTTP_REQUEST_FILE,
        http_request_redaction.value(),
    )?;
    let http_transport_plan_redaction = redact_provider_json_artifact(
        context,
        request,
        HTTP_TRANSPORT_PLAN_FILE,
        &fixture.http_transport_plan,
    )?;
    let http_transport_plan_ref = context.state_store().write_provider_json(
        request.job_id(),
        request.provider_instance_id(),
        HTTP_TRANSPORT_PLAN_FILE,
        http_transport_plan_redaction.value(),
    )?;
    let raw_response_redaction =
        redact_provider_json_artifact(context, request, RAW_RESPONSE_FILE, &fixture.raw_response)?;
    let raw_response_ref = context.state_store().write_provider_json(
        request.job_id(),
        request.provider_instance_id(),
        RAW_RESPONSE_FILE,
        raw_response_redaction.value(),
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

    let stdout = api_offline_stdout_value(
        manifest,
        &fixture.prepared_request,
        &fixture.fixture_relative_path,
    );
    let stdout_redaction = redact_provider_text_artifact(context, request, STDOUT_FILE, &stdout)?;
    let stdout_ref = context.state_store().write_provider_text(
        request.job_id(),
        request.provider_instance_id(),
        STDOUT_FILE,
        stdout_redaction.content(),
    )?;
    let stderr = "cloud API offline fixture completed without live API call\n";
    let stderr_redaction = redact_provider_text_artifact(context, request, STDERR_FILE, stderr)?;
    let stderr_ref = context.state_store().write_provider_text(
        request.job_id(),
        request.provider_instance_id(),
        STDERR_FILE,
        stderr_redaction.content(),
    )?;

    let redaction_artifacts = [
        request_redaction.report_path().map(ToString::to_string),
        http_request_redaction
            .report_path()
            .map(ToString::to_string),
        http_transport_plan_redaction
            .report_path()
            .map(ToString::to_string),
        raw_response_redaction
            .report_path()
            .map(ToString::to_string),
        stdout_redaction.report_path().map(ToString::to_string),
        stderr_redaction.report_path().map(ToString::to_string),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let response_value = api_offline_response_value(
        request,
        manifest,
        instance,
        &fixture.prepared_request,
        &fixture.parsed_response,
        fixture.wall_time_ms,
        &redaction_artifacts,
    );
    let response_redaction =
        redact_provider_json_artifact(context, request, RESPONSE_FILE, &response_value)?;
    let result = ProviderRunResult::from_value(
        response_redaction.value().clone(),
        provider_output_path(request.provider_instance_id(), RESPONSE_FILE),
        context.schema_root(),
    )?;
    let response_ref = context.state_store().write_provider_json(
        request.job_id(),
        request.provider_instance_id(),
        RESPONSE_FILE,
        response_redaction.value(),
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
