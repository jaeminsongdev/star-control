use super::names::{credential_reference_kind, request_api_name};
use crate::cloud_cli::timeout_seconds;
use crate::cloud_constants::*;
use crate::cloud_policy::{cloud_policy_denied, string_field};
use crate::fake::provider_output_path;
use crate::{
    ExecutionRequest, OpenAiCompatiblePreparedRequest, ProviderAdapterError, ProviderInstance,
    ProviderManifest,
};
use serde_json::{json, Value};

pub(crate) fn prepared_request_value(prepared_request: &OpenAiCompatiblePreparedRequest) -> Value {
    json!({
        "schema_version": "1.0.0",
        "api": request_api_name(prepared_request.api()),
        "method": prepared_request.method(),
        "url": prepared_request.url(),
        "body": prepared_request.body()
    })
}

pub(crate) fn http_transport_plan_value(
    request: &ExecutionRequest,
    manifest: &ProviderManifest,
    instance: &ProviderInstance,
    prepared_request: &OpenAiCompatiblePreparedRequest,
    execution_mode: &str,
    raw_response_expected: bool,
) -> Result<Value, ProviderAdapterError> {
    let credential_ref = string_field(instance.value(), "credential_ref").ok_or_else(|| {
        cloud_policy_denied(
            instance.id(),
            "cloud API transport plan requires a credential_ref declaration",
        )
    })?;
    let raw_response_path = if raw_response_expected {
        json!(provider_output_path(
            request.provider_instance_id(),
            RAW_RESPONSE_FILE
        ))
    } else {
        Value::Null
    };
    Ok(json!({
        "schema_version": "1.0.0",
        "provider_instance_id": request.provider_instance_id(),
        "job_id": request.job_id(),
        "stage": request.stage(),
        "provider_id": manifest.id(),
        "provider_kind": manifest.kind(),
        "adapter": manifest.adapter(),
        "transport": HTTP_TRANSPORT,
        "execution_mode": execution_mode,
        "method": prepared_request.method(),
        "url": prepared_request.url(),
        "request_api": request_api_name(prepared_request.api()),
        "request_body_path": provider_output_path(request.provider_instance_id(), HTTP_REQUEST_FILE),
        "raw_response_path": raw_response_path,
        "raw_response_expected": raw_response_expected,
        "credential": {
            "required": true,
            "reference_present": true,
            "reference_kind": credential_reference_kind(credential_ref),
            "materialized": false,
            "value_present": false
        },
        "header_policy": [
            {
                "name": "Content-Type",
                "value_policy": "literal",
                "value": "application/json"
            },
            {
                "name": "Authorization",
                "value_policy": "deferred_credential_reference",
                "scheme": "Bearer",
                "materialized": false
            }
        ],
        "timeout_seconds": timeout_seconds(instance.value(), instance.id())?,
        "live_api_call": false,
        "approval_required_for_live_call": true
    }))
}

pub(crate) fn live_transport_approval_value(
    request: &ExecutionRequest,
    manifest: &ProviderManifest,
    instance: &ProviderInstance,
    prepared_request: &OpenAiCompatiblePreparedRequest,
) -> Result<Value, ProviderAdapterError> {
    let credential_ref = string_field(instance.value(), "credential_ref").ok_or_else(|| {
        cloud_policy_denied(
            instance.id(),
            "cloud API live transport approval requires a credential_ref declaration",
        )
    })?;
    Ok(json!({
        "schema_version": "1.0.0",
        "provider_instance_id": request.provider_instance_id(),
        "job_id": request.job_id(),
        "stage": request.stage(),
        "provider_id": manifest.id(),
        "provider_kind": manifest.kind(),
        "adapter": manifest.adapter(),
        "transport": HTTP_TRANSPORT,
        "kind": "cloud_api_live_transport_approval_required",
        "status": "blocked",
        "approval_required": true,
        "approval_required_actions": [
            "credential_lookup",
            "authorization_header_value_construction",
            "live_http_request",
            "paid_api_call"
        ],
        "request": {
            "method": prepared_request.method(),
            "url": prepared_request.url(),
            "api": request_api_name(prepared_request.api()),
            "body_path": provider_output_path(request.provider_instance_id(), HTTP_REQUEST_FILE)
        },
        "credential": {
            "required": true,
            "reference_present": true,
            "reference_kind": credential_reference_kind(credential_ref),
            "materialized": false,
            "value_present": false
        },
        "live_api_call": false,
        "approval_required_for_live_call": true,
        "notes": "No credential value is read and no external HTTP request is sent until a separate approval step is implemented."
    }))
}
