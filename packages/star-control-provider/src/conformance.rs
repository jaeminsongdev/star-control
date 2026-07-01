use crate::{ProviderExecution, ProviderRunContext};
use serde_json::Value;
use star_control_state::StateStoreError;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

const REQUEST_FILE: &str = "request.json";
const RESPONSE_FILE: &str = "response.json";
const STDOUT_FILE: &str = "stdout.txt";
const STDERR_FILE: &str = "stderr.txt";
const PRIVACY_HANDOFF_FILE: &str = "privacy-handoff.json";
const COST_METRIC_FILE: &str = "cost-metric.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderConformanceProfile {
    Basic,
    Cloud,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderConformanceChecker;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConformanceReport {
    provider_instance_id: String,
    job_id: String,
    status: String,
    checked_artifacts: Vec<String>,
}

impl ProviderConformanceReport {
    pub fn provider_instance_id(&self) -> &str {
        &self.provider_instance_id
    }

    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn checked_artifacts(&self) -> &[String] {
        &self.checked_artifacts
    }
}

#[derive(Debug)]
pub enum ProviderConformanceError {
    MissingField {
        field: String,
    },
    InvalidFieldType {
        field: String,
        expected: &'static str,
    },
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    InvalidArtifactPath {
        field: String,
        path: String,
        reason: String,
    },
    ArtifactMissing {
        path: PathBuf,
    },
    State(StateStoreError),
}

impl fmt::Display for ProviderConformanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField { field } => {
                write!(formatter, "provider conformance missing field {}", field)
            }
            Self::InvalidFieldType { field, expected } => write!(
                formatter,
                "provider conformance invalid field type for {}, expected {}",
                field, expected
            ),
            Self::FieldMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "provider conformance field mismatch for {}: expected {}, got {}",
                field, expected, actual
            ),
            Self::InvalidArtifactPath {
                field,
                path,
                reason,
            } => write!(
                formatter,
                "provider conformance invalid artifact path in {}: {} ({})",
                field, path, reason
            ),
            Self::ArtifactMissing { path } => {
                write!(
                    formatter,
                    "provider conformance artifact missing: {}",
                    path.display()
                )
            }
            Self::State(source) => {
                write!(formatter, "provider conformance state error: {}", source)
            }
        }
    }
}

impl Error for ProviderConformanceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::State(source) => Some(source),
            _ => None,
        }
    }
}

impl From<StateStoreError> for ProviderConformanceError {
    fn from(source: StateStoreError) -> Self {
        Self::State(source)
    }
}

impl ProviderConformanceChecker {
    pub fn check_execution(
        &self,
        execution: &ProviderExecution,
        context: &ProviderRunContext<'_>,
        profile: ProviderConformanceProfile,
    ) -> Result<ProviderConformanceReport, ProviderConformanceError> {
        let result = execution.result();
        let value = result.value();
        let provider_instance_id = result.provider_instance_id();
        let job_id = result.job_id();
        let mut checked_artifacts = Vec::new();

        check_ref_path(
            execution.request_ref(),
            "request_ref.path",
            &provider_path(provider_instance_id, REQUEST_FILE),
        )?;
        check_ref_path(
            execution.response_ref(),
            "response_ref.path",
            &provider_path(provider_instance_id, RESPONSE_FILE),
        )?;
        check_ref_path(
            execution.stdout_ref(),
            "stdout_ref.path",
            &provider_path(provider_instance_id, STDOUT_FILE),
        )?;

        let stdout_path = required_string(value, "stdout_path")?;
        check_path_equals(
            "stdout_path",
            &stdout_path,
            &provider_path(provider_instance_id, STDOUT_FILE),
        )?;

        checked_artifacts.push(provider_path(provider_instance_id, REQUEST_FILE));
        checked_artifacts.push(provider_path(provider_instance_id, RESPONSE_FILE));
        checked_artifacts.push(provider_path(provider_instance_id, STDOUT_FILE));

        match nullable_string(value, "stderr_path")? {
            Some(stderr_path) => {
                check_path_equals(
                    "stderr_path",
                    &stderr_path,
                    &provider_path(provider_instance_id, STDERR_FILE),
                )?;
                let stderr_ref = execution.stderr_ref().ok_or_else(|| {
                    ProviderConformanceError::MissingField {
                        field: "stderr_ref".to_string(),
                    }
                })?;
                check_ref_path(
                    stderr_ref,
                    "stderr_ref.path",
                    &provider_path(provider_instance_id, STDERR_FILE),
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

        for path in required_artifact_paths(value)? {
            check_provider_relative_path("artifacts[]", &path, provider_instance_id)?;
            if !checked_artifacts.contains(&path) {
                checked_artifacts.push(path);
            }
        }

        if profile == ProviderConformanceProfile::Cloud {
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
        }

        checked_artifacts.sort();
        checked_artifacts.dedup();
        for path in &checked_artifacts {
            check_provider_relative_path("checked_artifacts[]", path, provider_instance_id)?;
            let absolute = context.state_store().resolve_job_path(job_id, path)?;
            if !absolute.is_file() {
                return Err(ProviderConformanceError::ArtifactMissing { path: absolute });
            }
        }

        Ok(ProviderConformanceReport {
            provider_instance_id: provider_instance_id.to_string(),
            job_id: job_id.to_string(),
            status: result.status().to_string(),
            checked_artifacts,
        })
    }
}

fn required_artifact_paths(value: &Value) -> Result<Vec<String>, ProviderConformanceError> {
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

fn require_artifact(
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

fn required_string(value: &Value, field: &str) -> Result<String, ProviderConformanceError> {
    let item = value
        .get(field)
        .ok_or_else(|| ProviderConformanceError::MissingField {
            field: field.to_string(),
        })?;
    item.as_str().map(ToString::to_string).ok_or_else(|| {
        ProviderConformanceError::InvalidFieldType {
            field: field.to_string(),
            expected: "string",
        }
    })
}

fn nullable_string(value: &Value, field: &str) -> Result<Option<String>, ProviderConformanceError> {
    let item = value
        .get(field)
        .ok_or_else(|| ProviderConformanceError::MissingField {
            field: field.to_string(),
        })?;
    if item.is_null() {
        return Ok(None);
    }
    item.as_str()
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| ProviderConformanceError::InvalidFieldType {
            field: field.to_string(),
            expected: "string or null",
        })
}

fn check_ref_path(
    value: &Value,
    field: &str,
    expected: &str,
) -> Result<(), ProviderConformanceError> {
    let actual = value.get("path").and_then(Value::as_str).ok_or_else(|| {
        ProviderConformanceError::MissingField {
            field: field.to_string(),
        }
    })?;
    check_path_equals(field, actual, expected)
}

fn check_path_equals(
    field: &str,
    actual: &str,
    expected: &str,
) -> Result<(), ProviderConformanceError> {
    if actual == expected {
        Ok(())
    } else {
        Err(ProviderConformanceError::FieldMismatch {
            field: field.to_string(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    }
}

fn check_provider_relative_path(
    field: &str,
    path: &str,
    provider_instance_id: &str,
) -> Result<(), ProviderConformanceError> {
    if path.is_empty() {
        return invalid_path(field, path, "path is empty");
    }
    if path.contains('\\') {
        return invalid_path(
            field,
            path,
            "backslash is not a canonical artifact separator",
        );
    }
    if path.starts_with('/') {
        return invalid_path(field, path, "absolute paths are not allowed");
    }
    if path.split('/').any(|segment| {
        segment.is_empty() || segment == "." || segment == ".." || segment.contains(':')
    }) {
        return invalid_path(field, path, "path must use normalized relative segments");
    }
    let expected_prefix = format!("provider-output/{}/", provider_instance_id);
    if !path.starts_with(&expected_prefix) {
        return invalid_path(
            field,
            path,
            "path must stay inside provider output directory",
        );
    }
    Ok(())
}

fn invalid_path<T>(field: &str, path: &str, reason: &str) -> Result<T, ProviderConformanceError> {
    Err(ProviderConformanceError::InvalidArtifactPath {
        field: field.to_string(),
        path: path.to_string(),
        reason: reason.to_string(),
    })
}

fn provider_path(provider_instance_id: &str, file_name: &str) -> String {
    format!("provider-output/{}/{}", provider_instance_id, file_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_relative_path_accepts_canonical_provider_output() {
        check_provider_relative_path(
            "artifacts[]",
            "provider-output/cloud-default/response.json",
            "cloud-default",
        )
        .expect("canonical provider output path");
    }

    #[test]
    fn provider_relative_path_rejects_unsafe_or_wrong_scope_paths() {
        for path in [
            "../response.json",
            "provider-output/cloud-default/../response.json",
            "provider-output/cloud-default\\response.json",
            "tool-output/cloud-default/response.json",
            "provider-output/other/response.json",
        ] {
            let error = check_provider_relative_path("artifacts[]", path, "cloud-default")
                .expect_err("unsafe provider artifact path should fail");
            assert!(matches!(
                error,
                ProviderConformanceError::InvalidArtifactPath { .. }
            ));
        }
    }
}
