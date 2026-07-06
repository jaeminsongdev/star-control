use super::super::error::ProviderConformanceError;
use super::super::helpers::{check_result_field, provider_path, read_and_validate_json_artifact};
use super::super::{
    COST_METRIC_FILE, COST_METRIC_SCHEMA, PRIVACY_HANDOFF_FILE, PRIVACY_HANDOFF_SCHEMA,
    PROVIDER_RUN_RESULT_SCHEMA, RESPONSE_FILE,
};
use crate::ProviderRunContext;
use serde_json::Value;

pub(super) fn validate_stored_response_artifact(
    context: &ProviderRunContext<'_>,
    job_id: &str,
    provider_instance_id: &str,
    value: &Value,
) -> Result<(), ProviderConformanceError> {
    let stored_response = read_and_validate_json_artifact(
        context,
        job_id,
        &provider_path(provider_instance_id, RESPONSE_FILE),
        PROVIDER_RUN_RESULT_SCHEMA,
    )?;
    if stored_response != *value {
        return Err(ProviderConformanceError::FieldMismatch {
            field: "provider-output response.json".to_string(),
            expected: "execution result value".to_string(),
            actual: "stored response artifact differs".to_string(),
        });
    }
    Ok(())
}

pub(super) fn validate_cloud_sidecars(
    context: &ProviderRunContext<'_>,
    job_id: &str,
    provider_instance_id: &str,
    stage: &str,
) -> Result<(), ProviderConformanceError> {
    let privacy_handoff = read_and_validate_json_artifact(
        context,
        job_id,
        &provider_path(provider_instance_id, PRIVACY_HANDOFF_FILE),
        PRIVACY_HANDOFF_SCHEMA,
    )?;
    check_result_field(&privacy_handoff, "job_id", job_id)?;

    let cost_metric = read_and_validate_json_artifact(
        context,
        job_id,
        &provider_path(provider_instance_id, COST_METRIC_FILE),
        COST_METRIC_SCHEMA,
    )?;
    check_result_field(&cost_metric, "job_id", job_id)?;
    check_result_field(&cost_metric, "provider_instance_id", provider_instance_id)?;
    check_result_field(&cost_metric, "stage", stage)?;
    Ok(())
}
