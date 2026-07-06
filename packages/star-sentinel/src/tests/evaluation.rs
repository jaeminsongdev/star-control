use super::*;
use serde_json::json;

#[test]
fn loads_builtin_registry() {
    let registry = builtin_registry();

    assert_eq!(registry.profile, "quick");
    assert!(registry.rule(RULE_SCOPE_ALLOWED_PATHS).is_some());
    assert_eq!(
        registry
            .rule(RULE_DEPENDENCY_REQUIRES_APPROVAL)
            .expect("dependency rule")
            .decision_effect,
        Decision::HumanReview
    );
}

#[test]
fn scope_fixture_blocks_out_of_scope_change() {
    let registry = builtin_registry();
    let evaluator = P0Evaluator::new(registry);
    let task = task_with_allowed_paths(["src/allowed/**"]);
    let changed_lines = changed_lines(json!([
        file("src/allowed/index.ts", "modified", json!([])),
        file("src/other/hidden.ts", "modified", json!([]))
    ]));

    let result = evaluator.evaluate(&task, &changed_lines).expect("evaluate");

    assert_eq!(result.decision, Decision::Block);
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.rule_id == RULE_SCOPE_ALLOWED_PATHS
            && diagnostic.severity == Severity::Block
            && diagnostic.locations[0].path == "src/other/hidden.ts"
    }));
    assert_diagnostics_schema_valid(&result.diagnostics);
}

#[test]
fn dependency_change_requires_human_review() {
    let evaluator = P0Evaluator::new(builtin_registry());
    let task = task_with_allowed_paths(["**"]);
    let changed_lines = changed_lines(json!([file("Cargo.toml", "modified", json!([]))]));

    let result = evaluator.evaluate(&task, &changed_lines).expect("evaluate");

    assert_eq!(result.decision, Decision::HumanReview);
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.rule_id == RULE_DEPENDENCY_REQUIRES_APPROVAL
            && diagnostic.severity == Severity::Warn
    }));
}

#[test]
fn test_file_deletion_blocks() {
    let evaluator = P0Evaluator::new(builtin_registry());
    let task = task_with_allowed_paths(["**"]);
    let changed_lines = changed_lines(json!([file("tests/runtime_test.rs", "deleted", json!([]))]));

    let result = evaluator.evaluate(&task, &changed_lines).expect("evaluate");

    assert_eq!(result.decision, Decision::Block);
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule_id == RULE_TEST_NO_DELETION));
}

#[test]
fn plaintext_secret_blocks_without_echoing_raw_secret() {
    let evaluator = P0Evaluator::new(builtin_registry());
    let task = task_with_allowed_paths(["**"]);
    let secret_value = format!("{}{}", "sk-test", "1234567890");
    let changed_lines = changed_lines(json!([file(
        "src/config.ts",
        "modified",
        json!([
            {
                "kind": "added",
                "old_line": null,
                "new_line": 7,
                "content": format!("const api_key = \"{secret_value}\";")
            }
        ])
    )]));

    let result = evaluator.evaluate(&task, &changed_lines).expect("evaluate");

    assert_eq!(result.decision, Decision::Block);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.rule_id == RULE_SECRET_NO_PLAINTEXT_SECRET)
        .expect("secret diagnostic");
    let rendered = diagnostic.to_value().to_string();
    assert!(!rendered.contains(&secret_value));
}

#[test]
fn validator_self_bypass_blocks() {
    let evaluator = P0Evaluator::new(builtin_registry());
    let task = task_with_allowed_paths(["**"]);
    let changed_lines = changed_lines(json!([file(
        ".github/workflows/ci.yml",
        "modified",
        json!([
            {"kind": "added", "old_line": null, "new_line": 12, "content": "continue-on-error: true"}
        ])
    )]));

    let result = evaluator.evaluate(&task, &changed_lines).expect("evaluate");

    assert_eq!(result.decision, Decision::Block);
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule_id == RULE_VALIDATOR_NO_SELF_BYPASS));
}

#[test]
fn reads_schema_valid_json_inputs() {
    let temp_dir = temp_dir();
    let task_path = temp_dir.join("task.json");
    let changed_path = temp_dir.join("changed-lines.json");
    fs::write(&task_path, task_value(["src/**"]).to_string()).expect("write task");
    fs::write(
        &changed_path,
        changed_lines_value(json!([file("src/main.rs", "modified", json!([]))])).to_string(),
    )
    .expect("write changed lines");

    let task = read_task(&task_path, schema_root()).expect("read task");
    let changed_lines = read_changed_lines(&changed_path, schema_root()).expect("read changed");

    assert_eq!(task.task_id, "p0-task-demo");
    assert_eq!(changed_lines.files[0].path, "src/main.rs");
    fs::remove_dir_all(temp_dir).ok();
}

#[test]
fn builtin_scope_fixture_outcome_matches_evaluation() {
    let outcome = read_fixture_outcome(
        repo_root().join(
            "builtin-tools/star-sentinel/examples/p0/fixture-outcome-scope-block.example.json",
        ),
        schema_root(),
    )
    .expect("fixture outcome");
    let evaluator = P0Evaluator::new(builtin_registry());
    let task = task_with_allowed_paths(["src/allowed/**"]);
    let changed_lines = changed_lines(json!([
        file("src/allowed/index.ts", "modified", json!([])),
        file("src/other/hidden.ts", "modified", json!([]))
    ]));

    let result = evaluator.evaluate(&task, &changed_lines).expect("evaluate");

    assert!(outcome.matches_result(&result));
}
