use crate::cloud_policy::{currency, estimated_cost};
use crate::{ExecutionRequest, OpenAiCompatibleParsedResponse, ProviderInstance};
use serde_json::{json, Value};

pub(crate) fn cost_metric_value(request: &ExecutionRequest, instance: &ProviderInstance) -> Value {
    cost_metric_value_with_wall_time(request, instance, 0)
}

pub(crate) fn cost_metric_value_with_wall_time(
    request: &ExecutionRequest,
    instance: &ProviderInstance,
    wall_time_ms: u64,
) -> Value {
    cost_metric_value_with_usage(request, instance, 0, 0, wall_time_ms)
}

pub(crate) fn cost_metric_value_with_response_usage(
    request: &ExecutionRequest,
    instance: &ProviderInstance,
    parsed_response: &OpenAiCompatibleParsedResponse,
    wall_time_ms: u64,
) -> Value {
    cost_metric_value_with_usage(
        request,
        instance,
        parsed_response.input_tokens(),
        parsed_response.output_tokens(),
        wall_time_ms,
    )
}

fn cost_metric_value_with_usage(
    request: &ExecutionRequest,
    instance: &ProviderInstance,
    input_tokens: u64,
    output_tokens: u64,
    wall_time_ms: u64,
) -> Value {
    json!({
        "schema_version": "1.0.0",
        "job_id": request.job_id(),
        "stage": request.stage(),
        "provider_instance_id": request.provider_instance_id(),
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "estimated_cost": estimated_cost(instance),
        "currency": currency(instance),
        "wall_time_ms": wall_time_ms,
        "quota_remaining": null
    })
}
