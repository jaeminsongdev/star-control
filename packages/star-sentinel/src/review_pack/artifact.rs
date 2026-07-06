use super::markdown::render_review_pack_markdown;
use super::signals::{
    changed_file_paths, review_questions, review_risks, review_summary, review_validations,
};
use crate::changed_lines::ChangedLines;
use crate::constants::{
    REVIEW_PACK_JSON_FILE, REVIEW_PACK_MARKDOWN_FILE, REVIEW_PACK_SCHEMA,
    STAR_SENTINEL_TOOL_OUTPUT_DIR,
};
use crate::model::{EvaluationResult, ReviewValidation};
use crate::schema_io::validate_against_schema;
use crate::{SentinelError, SentinelTask};
use serde_json::{json, Value};
use std::path::Path;

pub fn build_review_pack_artifact(
    task: &SentinelTask,
    changed_lines: &ChangedLines,
    result: &EvaluationResult,
    validations: &[ReviewValidation],
) -> Value {
    let changed_files = changed_file_paths(changed_lines);
    let risks = review_risks(result);
    let validations = review_validations(result, validations);
    let unverified_claims: Vec<String> = Vec::new();
    let questions_for_human = review_questions(result);
    let generated_artifacts = vec![
        format!(
            "tool-output/{}/{}",
            STAR_SENTINEL_TOOL_OUTPUT_DIR, REVIEW_PACK_JSON_FILE
        ),
        format!(
            "tool-output/{}/{}",
            STAR_SENTINEL_TOOL_OUTPUT_DIR, REVIEW_PACK_MARKDOWN_FILE
        ),
        format!("review-packs/{}", REVIEW_PACK_JSON_FILE),
        format!("review-packs/{}", REVIEW_PACK_MARKDOWN_FILE),
    ];
    let summary = review_summary(result.decision);
    let markdown = render_review_pack_markdown(
        result.decision,
        &summary,
        &changed_files,
        &risks,
        &validations,
        &questions_for_human,
    );

    json!({
        "schema_version": "1.0.0",
        "task_id": task.task_id,
        "decision": result.decision.as_str(),
        "summary": summary,
        "changed_files": changed_files,
        "risks": risks,
        "validations": validations.iter().map(|validation| {
            json!({
                "command": validation.command,
                "result": validation.result
            })
        }).collect::<Vec<_>>(),
        "unverified_claims": unverified_claims,
        "questions_for_human": questions_for_human,
        "generated_artifacts": generated_artifacts,
        "review_pack_markdown": markdown
    })
}

pub fn validate_review_pack_artifact(
    review_pack: &Value,
    schema_root: impl AsRef<Path>,
) -> Result<(), SentinelError> {
    validate_against_schema(
        review_pack,
        schema_root.as_ref(),
        REVIEW_PACK_SCHEMA,
        REVIEW_PACK_JSON_FILE,
    )
}
