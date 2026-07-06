use super::CostMetricWriter;
use crate::constants::COST_METRIC_SCHEMA;
use crate::error::ObservabilityError;
use serde_json::Value;
use star_control_schema::{load_schema, validate_json};
use std::path::PathBuf;

impl CostMetricWriter {
    pub fn validate_metric(&self, metric: &Value) -> Result<(), ObservabilityError> {
        let schema_path = self.schema_root.join(COST_METRIC_SCHEMA);
        let schema =
            load_schema(&schema_path).map_err(|source| ObservabilityError::SchemaLoadFailed {
                path: schema_path.clone(),
                message: source.to_string(),
            })?;
        let result = validate_json(metric, &schema);
        if result.is_ok() {
            validate_cost_metric_semantics(metric)
        } else {
            Err(ObservabilityError::SchemaValidationFailed {
                path: PathBuf::from(COST_METRIC_SCHEMA),
                errors: result.errors,
            })
        }
    }
}

fn validate_cost_metric_semantics(metric: &Value) -> Result<(), ObservabilityError> {
    let estimated_cost = required_f64(metric, "estimated_cost")?;
    if estimated_cost < 0.0 {
        return Err(ObservabilityError::InvalidCostMetric {
            message: "estimated_cost must be non-negative".to_string(),
        });
    }
    required_u64(metric, "wall_time_ms")?;
    optional_u64(metric, "input_tokens")?;
    optional_u64(metric, "output_tokens")?;
    Ok(())
}

pub(super) fn required_string(
    value: &Value,
    field: &str,
    label: &str,
) -> Result<String, ObservabilityError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ObservabilityError::InvalidCostMetric {
            message: format!("{} requires string field {}", label, field),
        })
}

pub(super) fn required_f64(value: &Value, field: &str) -> Result<f64, ObservabilityError> {
    value
        .get(field)
        .and_then(Value::as_f64)
        .ok_or_else(|| ObservabilityError::InvalidCostMetric {
            message: format!("cost metric requires numeric field {}", field),
        })
}

pub(super) fn required_u64(value: &Value, field: &str) -> Result<u64, ObservabilityError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| ObservabilityError::InvalidCostMetric {
            message: format!("cost metric requires non-negative integer field {}", field),
        })
}

pub(super) fn optional_u64(value: &Value, field: &str) -> Result<u64, ObservabilityError> {
    match value.get(field) {
        Some(Value::Null) | None => Ok(0),
        Some(item) => item
            .as_u64()
            .ok_or_else(|| ObservabilityError::InvalidCostMetric {
                message: format!("cost metric field {} must be a non-negative integer", field),
            }),
    }
}
