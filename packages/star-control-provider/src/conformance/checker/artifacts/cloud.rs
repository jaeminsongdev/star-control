use super::super::super::error::ProviderConformanceError;
use super::super::super::helpers::{provider_path, require_artifact};
use super::super::super::{COST_METRIC_FILE, PRIVACY_HANDOFF_FILE};
use serde_json::Value;

pub(super) fn collect_required_cloud_artifacts(
    value: &Value,
    provider_instance_id: &str,
    checked_artifacts: &mut Vec<String>,
) -> Result<(), ProviderConformanceError> {
    require_artifact(
        value,
        provider_instance_id,
        &provider_path(provider_instance_id, PRIVACY_HANDOFF_FILE),
    )?;
    require_artifact(
        value,
        provider_instance_id,
        &provider_path(provider_instance_id, COST_METRIC_FILE),
    )?;
    for file_name in [PRIVACY_HANDOFF_FILE, COST_METRIC_FILE] {
        let path = provider_path(provider_instance_id, file_name);
        if !checked_artifacts.contains(&path) {
            checked_artifacts.push(path);
        }
    }
    Ok(())
}
