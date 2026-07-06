use serde_json::{json, Value};
use star_control_validation::ValidationContext;

pub(crate) fn context(task_id: &str) -> ValidationContext {
    ValidationContext::new("J-0001", "validate", task_id, "2026-07-01T00:00:00Z")
}

pub(crate) fn changed_lines_for(task_id: &str, path: &str, change_type: &str) -> Value {
    json!({
        "schema_version": "1.0.0",
        "task_id": task_id,
        "files": [
            {
                "path": path,
                "change_type": change_type,
                "hunks": [
                    {
                        "old_start": 1,
                        "old_lines": 1,
                        "new_start": 1,
                        "new_lines": 1,
                        "lines": [
                            {
                                "kind": "added",
                                "new_line": 1,
                                "content": "smoke fixture line"
                            }
                        ]
                    }
                ]
            }
        ]
    })
}

pub(super) fn sentinel_task<const N: usize>(task_id: &str, allowed_paths: [&str; N]) -> Value {
    let allowed_paths = allowed_paths.into_iter().collect::<Vec<_>>();
    json!({
        "schema_version": "1.0.0",
        "task_id": task_id,
        "goal": "v0 fake integration smoke",
        "allowed_paths": allowed_paths,
        "forbidden_paths": [],
        "forbidden_change_types": [],
        "required_validation": ["policy:p0"],
        "approval_required_changes": ["dependency"]
    })
}
