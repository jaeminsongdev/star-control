use super::super::error::ProviderConformanceError;
use serde_json::Value;

pub(crate) fn required_artifact_paths(
    value: &Value,
) -> Result<Vec<String>, ProviderConformanceError> {
    let artifacts =
        value
            .get("artifacts")
            .ok_or_else(|| ProviderConformanceError::MissingField {
                field: "artifacts".to_string(),
            })?;
    let Some(items) = artifacts.as_array() else {
        return Err(ProviderConformanceError::InvalidFieldType {
            field: "artifacts".to_string(),
            expected: "array",
        });
    };
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            item.as_str().map(ToString::to_string).ok_or_else(|| {
                ProviderConformanceError::InvalidFieldType {
                    field: format!("artifacts[{}]", index),
                    expected: "string",
                }
            })
        })
        .collect()
}

pub(crate) fn require_artifact(
    value: &Value,
    provider_instance_id: &str,
    expected_path: &str,
) -> Result<(), ProviderConformanceError> {
    let artifacts = required_artifact_paths(value)?;
    if artifacts.iter().any(|path| path == expected_path) {
        return Ok(());
    }
    Err(ProviderConformanceError::FieldMismatch {
        field: format!("artifacts for {}", provider_instance_id),
        expected: expected_path.to_string(),
        actual: artifacts.join(","),
    })
}
