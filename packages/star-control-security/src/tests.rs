use super::*;
use serde_json::json;
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
