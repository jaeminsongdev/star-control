use crate::constants::REDACTION_PLACEHOLDER;
use crate::model::{RedactionFinding, RedactionOutcome};
use serde_json::{json, Map, Value};

pub fn redact_value(value: Value) -> Value {
    redact_value_with_report(value).into_value()
}

pub fn redact_value_with_report(value: Value) -> RedactionOutcome {
    let mut findings = Vec::new();
    let value = redact_at_path(value, "$", &mut findings);
    RedactionOutcome { value, findings }
}

fn redact_at_path(value: Value, path: &str, findings: &mut Vec<RedactionFinding>) -> Value {
    match value {
        Value::Object(object) => Value::Object(redact_object(object, path, findings)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .enumerate()
                .map(|(index, value)| {
                    redact_at_path(value, &format!("{}[{}]", path, index), findings)
                })
                .collect(),
        ),
        Value::String(text) if looks_sensitive_string(&text) => {
            findings.push(RedactionFinding::new(
                "credential_candidate",
                path,
                "redacted",
            ));
            json!(REDACTION_PLACEHOLDER)
        }
        other => other,
    }
}

fn redact_object(
    object: Map<String, Value>,
    path: &str,
    findings: &mut Vec<RedactionFinding>,
) -> Map<String, Value> {
    object
        .into_iter()
        .map(|(key, value)| {
            let child_path = json_child_path(path, &key);
            if is_sensitive_key(&key) {
                findings.push(RedactionFinding::new(
                    "sensitive_key",
                    child_path,
                    "redacted",
                ));
                (key, json!(REDACTION_PLACEHOLDER))
            } else {
                let redacted = redact_at_path(value, &child_path, findings);
                (key, redacted)
            }
        })
        .collect()
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

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("credential")
        || key.contains("secret")
        || key.contains("password")
        || key.contains("api_key")
        || key.contains("apikey")
        || key.contains("authorization")
        || key == "token"
        || key.ends_with("_token")
}

fn looks_sensitive_string(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("bearer ")
        || lower.contains("api_key=")
        || lower.contains("apikey=")
        || lower.contains("password=")
        || lower.contains("token=")
        || lower.contains("x-api-key:")
        || value.contains("sk-")
        || value.contains("-----BEGIN PRIVATE KEY-----")
}
