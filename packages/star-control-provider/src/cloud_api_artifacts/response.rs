use super::names::{request_api_name, response_kind_name};
use crate::cloud_constants::*;
use crate::cloud_policy::{currency, estimated_cost};
use crate::fake::provider_output_path;
use crate::{
    ExecutionRequest, OpenAiCompatibleParsedResponse, OpenAiCompatiblePreparedRequest,
    ProviderInstance, ProviderManifest,
};
use serde_json::{json, Value};

pub(crate) fn api_offline_response_value(
    request: &ExecutionRequest,
    manifest: &ProviderManifest,
    instance: &ProviderInstance,
    prepared_request: &OpenAiCompatiblePreparedRequest,
    parsed_response: &OpenAiCompatibleParsedResponse,
    wall_time_ms: u64,
) -> Value {
    let request_path = provider_output_path(request.provider_instance_id(), REQUEST_FILE);
    let http_request_path = provider_output_path(request.provider_instance_id(), HTTP_REQUEST_FILE);
    let http_transport_plan_path =
        provider_output_path(request.provider_instance_id(), HTTP_TRANSPORT_PLAN_FILE);
    let raw_response_path = provider_output_path(request.provider_instance_id(), RAW_RESPONSE_FILE);
    let response_path = provider_output_path(request.provider_instance_id(), RESPONSE_FILE);
    let stdout_path = provider_output_path(request.provider_instance_id(), STDOUT_FILE);
    let stderr_path = provider_output_path(request.provider_instance_id(), STDERR_FILE);
    let privacy_path = provider_output_path(request.provider_instance_id(), PRIVACY_HANDOFF_FILE);
    let cost_path = provider_output_path(request.provider_instance_id(), COST_METRIC_FILE);

    json!({
        "schema_version": "1.0.0",
        "provider_instance_id": request.provider_instance_id(),
        "job_id": request.job_id(),
        "stage": request.stage(),
        "status": "success",
        "started_at": request.created_at(),
        "finished_at": request.created_at(),
        "stdout_path": stdout_path,
        "stderr_path": stderr_path,
        "summary": parsed_response.text(),
        "changed_files": [],
        "artifacts": [
            response_path,
            request_path,
            http_request_path,
            http_transport_plan_path,
            raw_response_path,
            stdout_path,
            stderr_path,
            privacy_path,
            cost_path
        ],
        "metrics": {
            "estimated_cost": estimated_cost(instance),
            "currency": currency(instance),
            "input_tokens": parsed_response.input_tokens(),
            "output_tokens": parsed_response.output_tokens(),
            "total_tokens": parsed_response.total_tokens(),
            "wall_time_ms": wall_time_ms,
            "transport": HTTP_TRANSPORT,
            "transport_execution": "offline_fixture",
            "request_api": request_api_name(prepared_request.api()),
            "response_kind": response_kind_name(parsed_response.kind()),
            "response_id": parsed_response.response_id(),
            "model": parsed_response.model(),
            "provider_id": manifest.id()
        },
        "error": Value::Null
    })
}

pub(crate) fn api_live_approval_response_value(
    request: &ExecutionRequest,
    manifest: &ProviderManifest,
    instance: &ProviderInstance,
    prepared_request: &OpenAiCompatiblePreparedRequest,
    wall_time_ms: u64,
) -> Value {
    let request_path = provider_output_path(request.provider_instance_id(), REQUEST_FILE);
    let http_request_path = provider_output_path(request.provider_instance_id(), HTTP_REQUEST_FILE);
    let http_transport_plan_path =
        provider_output_path(request.provider_instance_id(), HTTP_TRANSPORT_PLAN_FILE);
    let live_approval_path =
        provider_output_path(request.provider_instance_id(), LIVE_TRANSPORT_APPROVAL_FILE);
    let response_path = provider_output_path(request.provider_instance_id(), RESPONSE_FILE);
    let stdout_path = provider_output_path(request.provider_instance_id(), STDOUT_FILE);
    let stderr_path = provider_output_path(request.provider_instance_id(), STDERR_FILE);
    let privacy_path = provider_output_path(request.provider_instance_id(), PRIVACY_HANDOFF_FILE);
    let cost_path = provider_output_path(request.provider_instance_id(), COST_METRIC_FILE);

    json!({
        "schema_version": "1.0.0",
        "provider_instance_id": request.provider_instance_id(),
        "job_id": request.job_id(),
        "stage": request.stage(),
        "status": "blocked",
        "started_at": request.created_at(),
        "finished_at": request.created_at(),
        "stdout_path": stdout_path,
        "stderr_path": stderr_path,
        "summary": "cloud API live HTTP transport requires explicit approval before credential lookup or external API call",
        "changed_files": [],
        "artifacts": [
            response_path,
            request_path,
            http_request_path,
            http_transport_plan_path,
            live_approval_path,
            stdout_path,
            stderr_path,
            privacy_path,
            cost_path
        ],
        "metrics": {
            "estimated_cost": estimated_cost(instance),
            "currency": currency(instance),
            "input_tokens": 0,
            "output_tokens": 0,
            "total_tokens": 0,
            "wall_time_ms": wall_time_ms,
            "transport": HTTP_TRANSPORT,
            "transport_execution": "approval_required",
            "request_api": request_api_name(prepared_request.api()),
            "provider_id": manifest.id(),
            "live_api_call": false,
            "approval_required_for_live_call": true
        },
        "error": {
            "kind": "cloud_api_live_transport_approval_required",
            "message": "cloud API live HTTP transport requires explicit approval before credential lookup or external API call",
            "action": "approve_live_cloud_api_transport",
            "field": "transport_config.live_api_call_requested"
        }
    })
}
