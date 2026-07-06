use super::super::super::error::ProviderConformanceError;
use super::super::super::helpers::{
    check_path_equals, check_ref_contract, nullable_string, provider_path,
};
use super::super::super::{LOG_KIND, STDERR_FILE};
use crate::ProviderExecution;
use serde_json::Value;

pub(super) fn collect_stderr_artifact(
    execution: &ProviderExecution,
    value: &Value,
    provider_instance_id: &str,
    checked_artifacts: &mut Vec<String>,
) -> Result<(), ProviderConformanceError> {
    match nullable_string(value, "stderr_path")? {
        Some(stderr_path) => {
            check_path_equals(
                "stderr_path",
                &stderr_path,
                &provider_path(provider_instance_id, STDERR_FILE),
            )?;
            let stderr_ref =
                execution
                    .stderr_ref()
                    .ok_or_else(|| ProviderConformanceError::MissingField {
                        field: "stderr_ref".to_string(),
                    })?;
            check_ref_contract(
                stderr_ref,
                "stderr_ref",
                &provider_path(provider_instance_id, STDERR_FILE),
                LOG_KIND,
                provider_instance_id,
            )?;
            checked_artifacts.push(provider_path(provider_instance_id, STDERR_FILE));
        }
        None => {
            if execution.stderr_ref().is_some() {
                return Err(ProviderConformanceError::FieldMismatch {
                    field: "stderr_ref".to_string(),
                    expected: "None".to_string(),
                    actual: "Some".to_string(),
                });
            }
        }
    }
    Ok(())
}
