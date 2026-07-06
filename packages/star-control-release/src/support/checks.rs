use serde_json::{json, Value};

pub(crate) fn release_check(name: &str, status: &str, evidence_paths: Vec<String>) -> Value {
    json!({
        "name": name,
        "status": status,
        "evidence_paths": evidence_paths
    })
}

pub(crate) fn check_status(passed: bool) -> &'static str {
    if passed {
        "pass"
    } else {
        "fail"
    }
}
