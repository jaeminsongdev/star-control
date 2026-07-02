use serde_json::{json, Map, Value};

pub const SCHEMA_VERSION: &str = "1.0.0";
pub const REDACTION_PLACEHOLDER: &str = "[REDACTED]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactionFinding {
    kind: String,
    path: String,
    action: String,
}

impl RedactionFinding {
    pub fn new(
        kind: impl Into<String>,
        path: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        Self {
            kind: kind.into(),
            path: path.into(),
            action: action.into(),
        }
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn action(&self) -> &str {
        &self.action
    }

    pub fn to_json(&self) -> Value {
        json!({
            "kind": self.kind,
            "path": self.path,
            "action": self.action
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RedactionOutcome {
    value: Value,
    findings: Vec<RedactionFinding>,
}

impl RedactionOutcome {
    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn into_value(self) -> Value {
        self.value
    }

    pub fn findings(&self) -> &[RedactionFinding] {
        &self.findings
    }

    pub fn redacted(&self) -> bool {
        !self.findings.is_empty()
    }

    pub fn report(&self, job_id: &str, artifact_path: &str) -> Value {
        redaction_report(job_id, artifact_path, self.findings())
    }
}

pub fn redact_value(value: Value) -> Value {
    redact_value_with_report(value).into_value()
}

pub fn redact_value_with_report(value: Value) -> RedactionOutcome {
    let mut findings = Vec::new();
    let value = redact_at_path(value, "$", &mut findings);
    RedactionOutcome { value, findings }
}

pub fn redaction_report(job_id: &str, artifact_path: &str, findings: &[RedactionFinding]) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id,
        "artifact_path": artifact_path,
        "redacted": !findings.is_empty(),
        "placeholder": REDACTION_PLACEHOLDER,
        "findings": findings
            .iter()
            .map(RedactionFinding::to_json)
            .collect::<Vec<_>>()
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use star_control_schema::{load_schema, validate_json};
    use std::path::PathBuf;

    fn schema_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../specs/schemas")
    }

    #[test]
    fn redacts_sensitive_keys_and_secret_like_strings() {
        let api_key = format!("{}{}", "sk-test", "-secret");
        let token_value = format!("{}{}", "raw", "-secret-value");
        let outcome = redact_value_with_report(json!({
            "authorization": format!("Bearer {}", api_key),
            "nested": {
                "message": format!("token={}", token_value),
                "safe": "keep me"
            },
            "items": [
                "-----BEGIN PRIVATE KEY-----test"
            ]
        }));

        assert_eq!(outcome.value()["authorization"], REDACTION_PLACEHOLDER);
        assert_eq!(outcome.value()["nested"]["message"], REDACTION_PLACEHOLDER);
        assert_eq!(outcome.value()["nested"]["safe"], "keep me");
        assert_eq!(outcome.value()["items"][0], REDACTION_PLACEHOLDER);
        assert_eq!(outcome.findings().len(), 3);
        assert!(outcome.redacted());
    }

    #[test]
    fn report_is_schema_valid_and_never_contains_raw_secret() {
        let api_key = format!("{}{}", "sk-test", "-secret");
        let outcome = redact_value_with_report(json!({
            "stdout": format!("Authorization: Bearer {}", api_key)
        }));
        let report = outcome.report("J-0001", "provider-output/fake-default/stdout.txt");

        let schema =
            load_schema(schema_root().join("redaction-report.schema.json")).expect("load schema");
        let validation = validate_json(&report, &schema);
        assert!(validation.is_ok(), "{:?}", validation.errors);

        let report_text = serde_json::to_string(&report).expect("serialize report");
        assert!(!report_text.contains(&api_key));
        assert!(!report_text.contains("Bearer"));
        assert!(report_text.contains("credential_candidate"));
    }

    #[test]
    fn clean_value_has_empty_report() {
        let outcome = redact_value_with_report(json!({
            "message": "safe text",
            "count": 1
        }));
        let report = outcome.report("J-0001", "reports/report.json");

        assert_eq!(outcome.value()["message"], "safe text");
        assert!(!outcome.redacted());
        assert_eq!(report["redacted"], false);
        assert_eq!(report["findings"].as_array().expect("findings").len(), 0);
    }
}
