use super::*;
use star_control_state::StateStore;

#[test]
fn builds_schema_valid_gate_artifacts_for_block() {
    let result = scope_block_result();
    let task = task_with_allowed_paths(["src/allowed/**"]);
    let diagnostics = build_diagnostics_artifact(&result);
    let approval = build_approval_artifact(&task, &result);

    validate_diagnostics_artifact(&diagnostics, schema_root()).expect("diagnostics schema");
    validate_approval_artifact(&approval, schema_root()).expect("approval schema");
    assert_eq!(approval["decision"], "BLOCK");
    assert_eq!(
        approval["diagnostics"][0]["rule_id"],
        RULE_SCOPE_ALLOWED_PATHS
    );
}

#[test]
fn builds_human_review_gate_for_dependency_change() {
    let evaluator = P0Evaluator::new(builtin_registry());
    let task = task_with_allowed_paths(["**"]);
    let changed_lines = changed_lines(json!([file("Cargo.toml", "modified", json!([]))]));
    let result = evaluator.evaluate(&task, &changed_lines).expect("evaluate");
    let approval = build_approval_artifact(&task, &result);

    validate_approval_artifact(&approval, schema_root()).expect("approval schema");
    assert_eq!(approval["decision"], "HUMAN_REVIEW");
    assert_eq!(
        approval["required_human_actions"][0],
        "Review HUMAN_REVIEW diagnostics and record approval before continuing."
    );
}

#[test]
fn writes_gate_artifacts_to_state_store_tool_output() {
    let temp_project = temp_dir();
    let store = StateStore::open(&temp_project, repo_root().join("specs/schemas")).expect("store");
    let job = store
        .create_job("validate p0 output", "star-sentinel", Vec::new())
        .expect("job");
    let job_id = job["job_id"].as_str().expect("job_id");
    let task = task_with_allowed_paths(["src/allowed/**"]);
    let result = scope_block_result();

    let refs = write_gate_artifacts(&store, job_id, &task, &result, schema_root()).expect("write");

    assert_eq!(refs.diagnostics_ref["kind"], "tool_output");
    assert_eq!(
        refs.diagnostics_ref["path"],
        "tool-output/star-sentinel/diagnostics.json"
    );
    assert_eq!(
        refs.approval_ref["path"],
        "tool-output/star-sentinel/approval.json"
    );
    assert!(temp_project
        .join(".ai-runs/J-0001/tool-output/star-sentinel/diagnostics.json")
        .is_file());
    assert!(temp_project
        .join(".ai-runs/J-0001/tool-output/star-sentinel/approval.json")
        .is_file());
    fs::remove_dir_all(temp_project).ok();
}

#[test]
fn gate_writer_refuses_to_overwrite_existing_artifacts() {
    let temp_project = temp_dir();
    let store = StateStore::open(&temp_project, repo_root().join("specs/schemas")).expect("store");
    let job = store
        .create_job("validate p0 output", "star-sentinel", Vec::new())
        .expect("job");
    let job_id = job["job_id"].as_str().expect("job_id");
    let task = task_with_allowed_paths(["src/allowed/**"]);
    let result = scope_block_result();

    write_gate_artifacts(&store, job_id, &task, &result, schema_root()).expect("first write");
    let overwrite = write_gate_artifacts(&store, job_id, &task, &result, schema_root());

    assert!(matches!(overwrite, Err(SentinelError::State { .. })));
    fs::remove_dir_all(temp_project).ok();
}
