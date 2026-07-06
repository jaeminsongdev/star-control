mod budget;
mod io;
mod paths;
mod validation;

pub use budget::CostBudgetThresholds;

use crate::constants::SCHEMA_VERSION;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CostMetricWriter {
    schema_root: PathBuf,
}

impl CostMetricWriter {
    pub fn new(schema_root: impl Into<PathBuf>) -> Self {
        Self {
            schema_root: schema_root.into(),
        }
    }

    pub fn metric(
        &self,
        job_id: &str,
        stage: impl Into<String>,
        provider_instance_id: impl Into<String>,
        estimated_cost: f64,
        currency: impl Into<String>,
        wall_time_ms: u64,
    ) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": job_id,
            "stage": stage.into(),
            "provider_instance_id": provider_instance_id.into(),
            "input_tokens": 0,
            "output_tokens": 0,
            "estimated_cost": estimated_cost,
            "currency": currency.into(),
            "wall_time_ms": wall_time_ms,
            "quota_remaining": Value::Null
        })
    }
}
