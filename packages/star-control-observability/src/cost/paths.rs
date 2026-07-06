use crate::constants::COST_METRIC_FILE;
use crate::error::ObservabilityError;

pub(super) fn provider_cost_metric_path(
    provider_instance_id: &str,
) -> Result<String, ObservabilityError> {
    validate_safe_segment(provider_instance_id, "provider_instance_id")?;
    Ok(format!(
        "provider-output/{}/{}",
        provider_instance_id, COST_METRIC_FILE
    ))
}

fn validate_safe_segment(value: &str, field: &str) -> Result<(), ObservabilityError> {
    if value.is_empty()
        || value.contains('\0')
        || value.contains(':')
        || value.contains('/')
        || value.contains('\\')
        || value == "."
        || value == ".."
        || value == ".git"
    {
        return Err(ObservabilityError::InvalidCostMetric {
            message: format!("{} must be a safe path segment", field),
        });
    }
    Ok(())
}
