use crate::fake::provider_output_path;
use crate::{ExecutionRequest, ProviderAdapterError, ProviderRunContext};
use serde_json::{json, Map, Value};
use star_control_security::{
    redact_value_with_report, redaction_report, RedactionFinding, REDACTION_PLACEHOLDER,
};
use star_control_state::StateStoreError;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub(crate) struct RedactedJsonArtifact {
    value: Value,
    report_path: Option<String>,
}

impl RedactedJsonArtifact {
    pub(crate) fn value(&self) -> &Value {
        &self.value
    }

    pub(crate) fn report_path(&self) -> Option<&str> {
        self.report_path.as_deref()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RedactedTextArtifact {
    content: String,
    report_path: Option<String>,
}

impl RedactedTextArtifact {
    pub(crate) fn content(&self) -> &str {
        &self.content
    }

    pub(crate) fn report_path(&self) -> Option<&str> {
        self.report_path.as_deref()
    }
}

pub(crate) fn redact_provider_json_artifact(
    context: &ProviderRunContext<'_>,
    request: &ExecutionRequest,
    file_name: &str,
    value: &Value,
) -> Result<RedactedJsonArtifact, ProviderAdapterError> {
    let mut findings = Vec::new();
    let redacted = redact_json_value(value.clone(), "$", &mut findings);
    let artifact_path = provider_output_path(request.provider_instance_id(), file_name);
    let report_path = write_report(context, request, &artifact_path, file_name, &findings)?;
    Ok(RedactedJsonArtifact {
        value: redacted,
        report_path,
    })
}

pub(crate) fn redact_provider_text_artifact(
    context: &ProviderRunContext<'_>,
    request: &ExecutionRequest,
    file_name: &str,
    content: &str,
) -> Result<RedactedTextArtifact, ProviderAdapterError> {
    let artifact_path = provider_output_path(request.provider_instance_id(), file_name);
    let mut findings = Vec::new();
    let redacted = redact_text(content, "$", &mut findings);
    let report_path = write_report(context, request, &artifact_path, file_name, &findings)?;
    Ok(RedactedTextArtifact {
        content: redacted,
        report_path,
    })
}

pub(crate) fn redact_provider_text_file_artifact(
    context: &ProviderRunContext<'_>,
    request: &ExecutionRequest,
    file_name: &str,
    path: &Path,
) -> Result<Option<String>, ProviderAdapterError> {
    let content = fs::read_to_string(path).map_err(|source| ProviderAdapterError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let redacted = redact_provider_text_artifact(context, request, file_name, &content)?;
    if redacted.report_path().is_some() {
        fs::write(path, redacted.content()).map_err(|source| ProviderAdapterError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }
    Ok(redacted.report_path().map(ToString::to_string))
}

fn redact_json_value(value: Value, path: &str, findings: &mut Vec<RedactionFinding>) -> Value {
    match value {
        Value::Object(object) => Value::Object(redact_json_object(object, path, findings)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .enumerate()
                .map(|(index, value)| {
                    redact_json_value(value, &format!("{}[{}]", path, index), findings)
                })
                .collect(),
        ),
        Value::String(text) => json!(redact_text(&text, path, findings)),
        other => other,
    }
}

fn redact_json_object(
    object: Map<String, Value>,
    path: &str,
    findings: &mut Vec<RedactionFinding>,
) -> Map<String, Value> {
    object
        .into_iter()
        .map(|(key, value)| {
            let child_path = json_child_path(path, &key);
            if provider_sensitive_key(&key) {
                match value {
                    Value::String(_) => {
                        findings.push(RedactionFinding::new(
                            "sensitive_key",
                            child_path,
                            "redacted",
                        ));
                        (key, json!(REDACTION_PLACEHOLDER))
                    }
                    other => (key, redact_json_value(other, &child_path, findings)),
                }
            } else {
                (key, redact_json_value(value, &child_path, findings))
            }
        })
        .collect()
}

fn redact_text(text: &str, path: &str, findings: &mut Vec<RedactionFinding>) -> String {
    let outcome = redact_value_with_report(json!(text));
    if outcome.redacted() {
        findings.push(RedactionFinding::new(
            "credential_candidate",
            path,
            "redacted",
        ));
        REDACTION_PLACEHOLDER.to_string()
    } else {
        text.to_string()
    }
}

fn write_report(
    context: &ProviderRunContext<'_>,
    request: &ExecutionRequest,
    artifact_path: &str,
    file_name: &str,
    findings: &[RedactionFinding],
) -> Result<Option<String>, ProviderAdapterError> {
    if findings.is_empty() {
        return Ok(None);
    }
    let report_file_name = format!(
        "provider-redaction-{}-{}.json",
        safe_segment(request.provider_instance_id()),
        safe_segment(file_name)
    );
    let report = redaction_report(request.job_id(), artifact_path, findings);
    match context.state_store().write_redaction_report_json(
        request.job_id(),
        &report_file_name,
        &report,
    ) {
        Ok(_) | Err(StateStoreError::ArtifactAlreadyExists { .. }) => {
            Ok(Some(format!("audit/{}", report_file_name)))
        }
        Err(source) => Err(ProviderAdapterError::State(source)),
    }
}

fn provider_sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower.contains("secret")
        || lower.contains("password")
        || lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("authorization")
        || lower == "token"
        || lower.ends_with("_token")
        || lower == "credential_raw"
        || lower == "credential_value"
}

fn json_child_path(parent: &str, key: &str) -> String {
    if key
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        format!("{}.{}", parent, key)
    } else {
        format!("{}[{}]", parent, json!(key))
    }
}

fn safe_segment(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect()
}
