use serde_json::Value;
use std::fs;
use std::path::Path;

pub(super) fn read_file_snapshot(path: &Path, context: &str) -> String {
    fs::read_to_string(path).expect(context)
}

pub(super) fn assert_success(response: &Value) {
    assert_eq!(response["status"], "success");
}

pub(super) fn assert_failed_code(response: &Value, code: &str) {
    assert_eq!(response["status"], "failed");
    assert_eq!(response["error"]["code"], code);
}

pub(super) fn assert_state_unchanged(path: &Path, before: &str, context: &str) {
    let after = read_file_snapshot(path, context);
    assert_eq!(after, before);
}

pub(super) fn assert_api_response_not_written(project: &Path) {
    assert!(!project.join(".ai-runs/J-0001/api-response.json").exists());
}

pub(super) fn assert_redacted_text(text: &str, secret: &str) {
    assert!(!text.contains(secret));
    assert!(text.contains("[REDACTED]"));
}
