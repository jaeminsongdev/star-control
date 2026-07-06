mod path;
mod secret;

use crate::model::Diagnostic;
pub(crate) use path::normalize_path;
pub(super) use path::{is_dependency_path, is_test_path, is_validator_path, path_is_allowed};
pub(super) use secret::is_plaintext_secret_candidate;
use serde_json::Value;

pub(super) fn is_self_bypass_line(content: &str) -> bool {
    let lower = content.trim().to_ascii_lowercase();
    lower.contains("bypass")
        || lower.contains("skip validation")
        || lower.contains("disable validation")
        || lower.contains("ignore validation")
        || lower.contains("continue-on-error: true")
        || lower.contains("allow_failure: true")
        || lower.contains("exit 0")
        || lower.contains("|| true")
        || lower.contains("set +e")
}

pub(super) fn diagnostic_matches_expected(diagnostic: &Diagnostic, expected: &Value) -> bool {
    if let Some(rule_id) = expected.get("rule_id").and_then(Value::as_str) {
        if diagnostic.rule_id != rule_id {
            return false;
        }
    }
    if let Some(severity) = expected.get("severity").and_then(Value::as_str) {
        if diagnostic.severity.as_str() != severity {
            return false;
        }
    }
    if let Some(path) = expected.get("path").and_then(Value::as_str) {
        if !diagnostic
            .locations
            .iter()
            .any(|location| location.path == normalize_path(path))
        {
            return false;
        }
    }
    true
}
