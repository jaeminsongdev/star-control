use crate::cloud_io::validate_contract;
use crate::{ExecutionRequest, ProviderAdapterError};
use serde_json::{json, Value};
use std::path::Path;

pub(crate) const COST_METRIC_FILE: &str = "cost-metric.json";
const COST_METRIC_SCHEMA: &str = "cost-metric.schema.json";

pub(crate) fn zero_cost_metric_value(request: &ExecutionRequest, wall_time_ms: u64) -> Value {
    json!({
        "schema_version": "1.0.0",
        "job_id": request.job_id(),
        "stage": request.stage(),
        "provider_instance_id": request.provider_instance_id(),
        "input_tokens": 0,
        "output_tokens": 0,
        "estimated_cost": 0,
        "currency": "USD",
        "wall_time_ms": wall_time_ms,
        "quota_remaining": null
    })
}

pub(crate) fn validate_cost_metric(
    value: &Value,
    schema_root: &Path,
) -> Result<(), ProviderAdapterError> {
    validate_contract(
        value,
        Path::new(COST_METRIC_FILE),
        schema_root,
        COST_METRIC_SCHEMA,
    )
}
