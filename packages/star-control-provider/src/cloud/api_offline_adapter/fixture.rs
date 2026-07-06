use crate::cloud_api_artifacts::{http_transport_plan_value, prepared_request_value};
use crate::cloud_io::{read_json_file, resolve_project_relative_path};
use crate::cloud_policy::cloud_policy_denied;
use crate::{
    ExecutionRequest, OpenAiCompatibleParsedResponse, OpenAiCompatiblePreparedRequest,
    OpenAiCompatibleRequestBuilder, OpenAiCompatibleResponseParser, ProviderAdapterError,
    ProviderInstance, ProviderManifest, ProviderRunContext,
};
use serde_json::Value;
use std::time::Instant;

pub(super) struct OfflineFixture {
    pub(super) fixture_relative_path: String,
    pub(super) prepared_request: OpenAiCompatiblePreparedRequest,
    pub(super) raw_response: Value,
    pub(super) parsed_response: OpenAiCompatibleParsedResponse,
    pub(super) http_request_value: Value,
    pub(super) http_transport_plan: Value,
    pub(super) wall_time_ms: u64,
}

pub(super) fn prepare_offline_fixture(
    request: &ExecutionRequest,
    context: &ProviderRunContext<'_>,
    manifest: &ProviderManifest,
    instance: &ProviderInstance,
    fixture_relative_path: String,
) -> Result<OfflineFixture, ProviderAdapterError> {
    let started_at = Instant::now();
    let prepared_request = OpenAiCompatibleRequestBuilder
        .build(request, instance)
        .map_err(|source| {
            cloud_policy_denied(
                instance.id(),
                &format!("OpenAI-compatible request build failed: {}", source),
            )
        })?;
    let fixture_path = resolve_project_relative_path(
        context.state_store().project_root(),
        &fixture_relative_path,
        instance.id(),
    )?;
    let raw_response = read_json_file(&fixture_path)?;
    let parsed_response = OpenAiCompatibleResponseParser
        .parse(&raw_response)
        .map_err(|source| {
            cloud_policy_denied(
                instance.id(),
                &format!("OpenAI-compatible response parse failed: {}", source),
            )
        })?;
    let http_request_value = prepared_request_value(&prepared_request);
    let http_transport_plan = http_transport_plan_value(
        request,
        manifest,
        instance,
        &prepared_request,
        "offline_fixture",
        true,
    )?;
    let wall_time_ms = started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;

    Ok(OfflineFixture {
        fixture_relative_path,
        prepared_request,
        raw_response,
        parsed_response,
        http_request_value,
        http_transport_plan,
        wall_time_ms,
    })
}
