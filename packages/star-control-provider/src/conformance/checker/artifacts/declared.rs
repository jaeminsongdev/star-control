use super::super::super::error::ProviderConformanceError;
use super::super::super::helpers::{check_provider_relative_path, required_artifact_paths};
use serde_json::Value;

pub(super) fn collect_declared_artifacts(
    value: &Value,
    provider_instance_id: &str,
    checked_artifacts: &mut Vec<String>,
) -> Result<(), ProviderConformanceError> {
    for path in required_artifact_paths(value)? {
        check_provider_relative_path("artifacts[]", &path, provider_instance_id)?;
        if !checked_artifacts.contains(&path) {
            checked_artifacts.push(path);
        }
    }
    Ok(())
}
