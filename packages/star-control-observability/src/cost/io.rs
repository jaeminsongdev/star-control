use super::validation::required_string;
use super::CostMetricWriter;
use crate::constants::COST_METRIC_FILE;
use crate::error::ObservabilityError;
use serde_json::Value;
use star_control_security::redact_value;
use star_control_state::StateStore;
use std::fs;

impl CostMetricWriter {
    pub fn write_provider_metric(
        &self,
        store: &StateStore,
        metric: &Value,
    ) -> Result<Value, ObservabilityError> {
        let metric = redact_value(metric.clone());
        self.validate_metric(&metric)?;
        let job_id = required_string(&metric, "job_id", "cost metric")?;
        let provider_instance_id = required_string(&metric, "provider_instance_id", "cost metric")?;
        store
            .write_provider_json(&job_id, &provider_instance_id, COST_METRIC_FILE, &metric)
            .map_err(ObservabilityError::from)
    }

    pub fn read_provider_metric(
        &self,
        store: &StateStore,
        job_id: &str,
        provider_instance_id: &str,
    ) -> Result<Option<Value>, ObservabilityError> {
        let provider_dir = store.resolve_provider_output_dir(job_id, provider_instance_id)?;
        let path = provider_dir.join(COST_METRIC_FILE);
        if !path.is_file() {
            return Ok(None);
        }
        let content =
            fs::read_to_string(&path).map_err(|source| ObservabilityError::ReadFailed {
                path: path.clone(),
                source,
            })?;
        let metric: Value =
            serde_json::from_str(&content).map_err(|source| ObservabilityError::InvalidJson {
                path: path.clone(),
                source,
            })?;
        self.validate_metric(&metric)?;
        Ok(Some(metric))
    }
}
