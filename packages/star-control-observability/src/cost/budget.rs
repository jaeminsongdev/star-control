use super::paths::provider_cost_metric_path;
use super::validation::{optional_u64, required_f64, required_string, required_u64};
use super::CostMetricWriter;
use crate::constants::SCHEMA_VERSION;
use crate::error::ObservabilityError;
use serde_json::{json, Value};
use star_control_security::redact_value;

#[derive(Debug, Clone, Default)]
pub struct CostBudgetThresholds {
    pub(super) max_estimated_cost: Option<f64>,
    pub(super) max_wall_time_ms: Option<u64>,
    pub(super) max_total_tokens: Option<u64>,
}

impl CostBudgetThresholds {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_estimated_cost(mut self, value: f64) -> Self {
        self.max_estimated_cost = Some(value);
        self
    }

    pub fn with_max_wall_time_ms(mut self, value: u64) -> Self {
        self.max_wall_time_ms = Some(value);
        self
    }

    pub fn with_max_total_tokens(mut self, value: u64) -> Self {
        self.max_total_tokens = Some(value);
        self
    }
}

impl CostMetricWriter {
    pub fn evaluate_budget(
        &self,
        metric: &Value,
        thresholds: &CostBudgetThresholds,
    ) -> Result<Value, ObservabilityError> {
        let metric = redact_value(metric.clone());
        self.validate_metric(&metric)?;
        let job_id = required_string(&metric, "job_id", "cost metric")?;
        let stage = required_string(&metric, "stage", "cost metric")?;
        let provider_instance_id = required_string(&metric, "provider_instance_id", "cost metric")?;
        let metric_path = provider_cost_metric_path(&provider_instance_id)?;
        let estimated_cost = required_f64(&metric, "estimated_cost")?;
        let wall_time_ms = required_u64(&metric, "wall_time_ms")?;
        let total_tokens =
            optional_u64(&metric, "input_tokens")? + optional_u64(&metric, "output_tokens")?;

        let mut reasons = Vec::new();
        if let Some(limit) = thresholds.max_estimated_cost {
            if estimated_cost > limit {
                reasons.push(json!({
                    "kind": "estimated_cost_exceeded",
                    "actual": estimated_cost,
                    "limit": limit
                }));
            }
        }
        if let Some(limit) = thresholds.max_wall_time_ms {
            if wall_time_ms > limit {
                reasons.push(json!({
                    "kind": "wall_time_exceeded",
                    "actual": wall_time_ms,
                    "limit": limit
                }));
            }
        }
        if let Some(limit) = thresholds.max_total_tokens {
            if total_tokens > limit {
                reasons.push(json!({
                    "kind": "total_tokens_exceeded",
                    "actual": total_tokens,
                    "limit": limit
                }));
            }
        }

        let status = if reasons.is_empty() { "ok" } else { "warning" };
        Ok(json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": job_id,
            "stage": stage,
            "provider_instance_id": provider_instance_id,
            "status": status,
            "enforcement": "warn_only",
            "metric_path": metric_path,
            "reasons": reasons,
            "thresholds": thresholds_value(thresholds)
        }))
    }
}

fn thresholds_value(thresholds: &CostBudgetThresholds) -> Value {
    json!({
        "max_estimated_cost": thresholds.max_estimated_cost,
        "max_wall_time_ms": thresholds.max_wall_time_ms,
        "max_total_tokens": thresholds.max_total_tokens
    })
}
