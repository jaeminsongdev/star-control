use serde_json::Value;
use star_control_state::StateStore;
use star_sentinel::{
    build_approval_artifact, build_review_pack_artifact, read_p0_rule_registry,
    write_gate_artifacts, write_review_pack_artifacts, ChangedLines, Decision, EvaluationResult,
    P0Evaluator, ReviewValidation, SentinelTask,
};
use std::path::Path;

pub(super) struct GateArtifacts {
    pub(super) approval: Value,
    pub(super) review_pack: Option<Value>,
}

pub(super) fn write_sentinel_inputs(
    store: &StateStore,
    task_value: &Value,
    changed_lines_value: &Value,
) {
    store
        .write_tool_json("J-0001", "star-sentinel", "task.json", task_value)
        .expect("write task");
    store
        .write_tool_json(
            "J-0001",
            "star-sentinel",
            "changed_lines.json",
            changed_lines_value,
        )
        .expect("write changed lines");
}

pub(super) fn evaluate_and_write(
    store: &StateStore,
    repo_root: &Path,
    sentinel_schema_root: &Path,
    task: &SentinelTask,
    changed_lines: &ChangedLines,
) -> GateArtifacts {
    let registry = read_p0_rule_registry(
        repo_root.join("builtin-tools/star-sentinel/policies/p0-rule-registry.json"),
        sentinel_schema_root,
    )
    .expect("registry");
    let result = P0Evaluator::new(registry)
        .evaluate(task, changed_lines)
        .expect("evaluate");
    write_gate_artifacts(store, "J-0001", task, &result, sentinel_schema_root)
        .expect("gate artifacts");

    GateArtifacts {
        approval: build_approval_artifact(task, &result),
        review_pack: review_pack_for_decision(
            store,
            sentinel_schema_root,
            task,
            changed_lines,
            &result,
        ),
    }
}

fn review_pack_for_decision(
    store: &StateStore,
    sentinel_schema_root: &Path,
    task: &SentinelTask,
    changed_lines: &ChangedLines,
    result: &EvaluationResult,
) -> Option<Value> {
    if result.decision == Decision::AutoPass {
        return None;
    }
    let review_pack = build_review_pack_artifact(
        task,
        changed_lines,
        result,
        &[ReviewValidation::new(
            "policy:p0",
            validation_result_for_decision(result.decision),
        )],
    );
    write_review_pack_artifacts(store, "J-0001", &review_pack, sentinel_schema_root)
        .expect("review pack artifacts");
    Some(review_pack)
}

fn validation_result_for_decision(decision: Decision) -> &'static str {
    match decision {
        Decision::AutoPass => "passed",
        Decision::HumanReview => "requires_human_review",
        Decision::Block => "blocked",
    }
}
