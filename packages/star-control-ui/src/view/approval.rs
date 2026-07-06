use super::artifacts::paths_for_section;
use serde_json::{json, Value};

pub(crate) fn approval_summary(job_view: &Value, sections: &[Value]) -> Value {
    let required = job_view
        .get("approval_required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let request_paths = paths_for_section(sections, "approval_request");
    json!({
        "required": required,
        "paths": request_paths,
        "response_contract": "approval-response.schema.json",
        "mutation_surface": "api_or_cli",
        "mutations_enabled": false,
        "actions": ["approved", "rejected", "needs_changes", "cancelled"]
    })
}
