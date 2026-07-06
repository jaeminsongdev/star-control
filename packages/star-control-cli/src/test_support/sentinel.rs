use super::repo_root;
use serde_json::{json, Value};
use star_control_state::StateStore;
use std::path::Path;

pub(crate) fn write_sentinel_input_job(
    project: &Path,
    task_id: &str,
    allowed_paths: Vec<&str>,
    changed_path: &str,
) {
    let store = StateStore::open(project, repo_root().join("specs/schemas")).expect("open store");
    store
        .create_job("sentinel input", "codex", vec![])
        .expect("create job");
    store
        .write_tool_json(
            "J-0001",
            "star-sentinel",
            "task.json",
            &sentinel_task_value(task_id, allowed_paths),
        )
        .expect("write sentinel task");
    store
        .write_tool_json(
            "J-0001",
            "star-sentinel",
            "changed_lines.json",
            &changed_lines_value(task_id, changed_path),
        )
        .expect("write changed lines");
}

fn sentinel_task_value(task_id: &str, allowed_paths: Vec<&str>) -> Value {
    json!({
        "schema_version": "1.0.0",
        "task_id": task_id,
        "goal": "Validate a scoped CLI sentinel fixture.",
        "allowed_paths": allowed_paths,
        "forbidden_paths": [
            ".github/workflows/**",
            "package.json",
            "package-lock.json"
        ],
        "forbidden_change_types": [
            "test_deletion",
            "assertion_weakening",
            "validator_bypass",
            "secret_exposure"
        ],
        "required_validation": [
            "policy:p0"
        ],
        "approval_required_changes": [
            "public_api_change",
            "schema_change",
            "dependency_addition"
        ],
        "notes": "CLI sentinel command fixture."
    })
}

fn changed_lines_value(task_id: &str, path: &str) -> Value {
    json!({
        "schema_version": "1.0.0",
        "task_id": task_id,
        "files": [
            {
                "path": path,
                "change_type": "modified",
                "old_path": null,
                "hunks": [
                    {
                        "old_start": 1,
                        "old_lines": 2,
                        "new_start": 1,
                        "new_lines": 3,
                        "lines": [
                            {
                                "kind": "context",
                                "old_line": 1,
                                "new_line": 1,
                                "content": "fn existing() {}"
                            },
                            {
                                "kind": "added",
                                "old_line": null,
                                "new_line": 2,
                                "content": "fn added() {}"
                            }
                        ]
                    }
                ]
            }
        ]
    })
}
