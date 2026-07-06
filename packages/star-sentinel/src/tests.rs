mod evaluation;
mod gate_artifacts;
mod ledger;
mod review_pack_artifacts;
mod selfcheck;

use super::*;
use serde_json::{json, Value};
use star_control_schema::{load_schema, validate_json};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn builtin_registry() -> P0RuleRegistry {
    read_p0_rule_registry(
        repo_root().join("builtin-tools/star-sentinel/policies/p0-rule-registry.json"),
        schema_root(),
    )
    .expect("builtin registry")
}

pub(super) fn scope_block_result() -> EvaluationResult {
    let evaluator = P0Evaluator::new(builtin_registry());
    let task = task_with_allowed_paths(["src/allowed/**"]);
    let changed_lines = changed_lines(json!([
        file("src/allowed/index.ts", "modified", json!([])),
        file("src/other/hidden.ts", "modified", json!([]))
    ]));
    evaluator.evaluate(&task, &changed_lines).expect("evaluate")
}

pub(super) fn assert_diagnostics_schema_valid(diagnostics: &[Diagnostic]) {
    let schema =
        load_schema(schema_root().join("diagnostic.schema.json")).expect("diagnostic schema");
    for diagnostic in diagnostics {
        let result = validate_json(&diagnostic.to_value(), &schema);
        assert!(result.is_ok(), "{:?}", result.errors);
    }
}

pub(super) fn task_with_allowed_paths<const N: usize>(allowed_paths: [&str; N]) -> SentinelTask {
    SentinelTask::from_value(&task_value(allowed_paths)).expect("task")
}

pub(super) fn task_value<const N: usize>(allowed_paths: [&str; N]) -> Value {
    let allowed_paths: Vec<&str> = allowed_paths.into_iter().collect();
    json!({
        "schema_version": "1.0.0",
        "task_id": "p0-task-demo",
        "goal": "Validate P0 evaluator behavior.",
        "allowed_paths": allowed_paths,
        "forbidden_paths": [],
        "forbidden_change_types": [],
        "required_validation": [],
        "approval_required_changes": []
    })
}

pub(super) fn changed_lines(files: Value) -> ChangedLines {
    ChangedLines::from_value(&changed_lines_value(files)).expect("changed lines")
}

pub(super) fn changed_lines_value(files: Value) -> Value {
    json!({
        "schema_version": "1.0.0",
        "task_id": "p0-task-demo",
        "files": files
    })
}

pub(super) fn file(path: &str, change_type: &str, lines: Value) -> Value {
    json!({
        "path": path,
        "change_type": change_type,
        "old_path": null,
        "hunks": [
            {
                "old_start": 1,
                "old_lines": 1,
                "new_start": 1,
                "new_lines": 1,
                "lines": lines
            }
        ]
    })
}

pub(super) fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("packages dir")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

pub(super) fn schema_root() -> PathBuf {
    repo_root().join("builtin-tools/star-sentinel/schemas")
}

pub(super) fn temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("star-sentinel-{}-{}", std::process::id(), nanos));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

pub(super) fn copy_dir(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create destination");
    for entry in fs::read_dir(source).expect("read source") {
        let entry = entry.expect("entry");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir(&source_path, &destination_path);
        } else {
            fs::copy(&source_path, &destination_path).expect("copy file");
        }
    }
}
