use super::*;
use serde_json::json;
use star_control_state::StateStore;

#[test]
fn builds_schema_valid_review_pack_for_block() {
    let task = task_with_allowed_paths(["src/allowed/**"]);
    let changed_lines = changed_lines(json!([
        file("src/allowed/index.ts", "modified", json!([])),
        file("src/other/hidden.ts", "modified", json!([]))
    ]));
    let result = scope_block_result();

    let review_pack = build_review_pack_artifact(&task, &changed_lines, &result, &[]);

    validate_review_pack_artifact(&review_pack, schema_root()).expect("review pack schema");
    assert_eq!(review_pack["decision"], "BLOCK");
    assert_eq!(review_pack["risks"][0], "scope_violation");
    assert!(review_pack["review_pack_markdown"]
        .as_str()
        .expect("markdown")
        .contains("## Questions For Human"));
}

#[test]
fn review_pack_for_dependency_contains_human_question() {
    let evaluator = P0Evaluator::new(builtin_registry());
    let task = task_with_allowed_paths(["**"]);
    let changed_lines = changed_lines(json!([file("Cargo.toml", "modified", json!([]))]));
    let result = evaluator.evaluate(&task, &changed_lines).expect("evaluate");

    let review_pack = build_review_pack_artifact(&task, &changed_lines, &result, &[]);

    assert_eq!(review_pack["decision"], "HUMAN_REVIEW");
    assert_eq!(review_pack["risks"][0], "dependency_addition");
    assert_eq!(
        review_pack["questions_for_human"][0],
        "Was this dependency change explicitly approved?"
    );
}

#[test]
fn writes_review_pack_artifacts_to_tool_output_and_review_packs() {
    let temp_project = temp_dir();
    let store = StateStore::open(&temp_project, repo_root().join("specs/schemas")).expect("store");
    let job = store
        .create_job("review p0 output", "star-sentinel", Vec::new())
        .expect("job");
    let job_id = job["job_id"].as_str().expect("job_id");
    let task = task_with_allowed_paths(["src/allowed/**"]);
    let changed_lines = changed_lines(json!([
        file("src/allowed/index.ts", "modified", json!([])),
        file("src/other/hidden.ts", "modified", json!([]))
    ]));
    let result = scope_block_result();
    let review_pack = build_review_pack_artifact(&task, &changed_lines, &result, &[]);

    let refs =
        write_review_pack_artifacts(&store, job_id, &review_pack, schema_root()).expect("write");

    assert_eq!(
        refs.tool_json_ref["path"],
        "tool-output/star-sentinel/review_pack.json"
    );
    assert_eq!(
        refs.tool_markdown_ref["path"],
        "tool-output/star-sentinel/review_pack.md"
    );
    assert_eq!(
        refs.review_json_ref["path"],
        "review-packs/review_pack.json"
    );
    assert_eq!(
        refs.review_markdown_ref["path"],
        "review-packs/review_pack.md"
    );
    assert!(temp_project
        .join(".ai-runs/J-0001/tool-output/star-sentinel/review_pack.json")
        .is_file());
    assert!(temp_project
        .join(".ai-runs/J-0001/tool-output/star-sentinel/review_pack.md")
        .is_file());
    assert!(temp_project
        .join(".ai-runs/J-0001/review-packs/review_pack.json")
        .is_file());
    assert!(temp_project
        .join(".ai-runs/J-0001/review-packs/review_pack.md")
        .is_file());
    fs::remove_dir_all(temp_project).ok();
}

#[test]
fn review_pack_writer_refuses_to_overwrite_existing_artifacts() {
    let temp_project = temp_dir();
    let store = StateStore::open(&temp_project, repo_root().join("specs/schemas")).expect("store");
    let job = store
        .create_job("review p0 output", "star-sentinel", Vec::new())
        .expect("job");
    let job_id = job["job_id"].as_str().expect("job_id");
    let task = task_with_allowed_paths(["src/allowed/**"]);
    let changed_lines = changed_lines(json!([file("src/other/hidden.ts", "modified", json!([]))]));
    let result = scope_block_result();
    let review_pack = build_review_pack_artifact(&task, &changed_lines, &result, &[]);

    write_review_pack_artifacts(&store, job_id, &review_pack, schema_root()).expect("first write");
    let overwrite = write_review_pack_artifacts(&store, job_id, &review_pack, schema_root());

    assert!(matches!(overwrite, Err(SentinelError::State { .. })));
    fs::remove_dir_all(temp_project).ok();
}
