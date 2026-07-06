use crate::constants::{
    REVIEW_PACK_JSON_PATH, REVIEW_PACK_MARKDOWN_PATH, SCHEMA_VERSION, SENTINEL_APPROVAL_PATH,
    SENTINEL_REVIEW_PACK_JSON_PATH, SENTINEL_REVIEW_PACK_MARKDOWN_PATH,
};
use crate::ValidationContext;
use serde_json::{json, Value};

pub(crate) fn build_validation_decision(
    context: &ValidationContext,
    decision: &str,
    reasons: Vec<String>,
    diagnostics: Value,
    next_state: &str,
    review_pack_path: Option<&str>,
    approval_request_path: Option<&str>,
) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": context.job_id(),
        "stage": context.stage(),
        "task_id": context.task_id(),
        "decision": decision,
        "source": "star-sentinel/gate",
        "reasons": reasons,
        "diagnostics": diagnostics,
        "next_state": next_state,
        "review_pack_path": review_pack_path,
        "approval_request_path": approval_request_path
    })
}

pub(crate) fn build_approval_request(
    context: &ValidationContext,
    decision: &str,
    reasons: Vec<String>,
    diagnostics: Value,
    review_pack: Option<&Value>,
) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": context.job_id(),
        "stage": context.stage(),
        "task_id": context.task_id(),
        "decision": decision,
        "reasons": reasons,
        "changed_files": array_of_strings_from(review_pack, "changed_files"),
        "risks": array_of_strings_from(review_pack, "risks"),
        "diagnostics": diagnostics,
        "review_pack_path": REVIEW_PACK_MARKDOWN_PATH,
        "requested_at": context.requested_at(),
        "requested_by": "validation-engine"
    })
}

pub(crate) fn build_review_pack_handoff(
    context: &ValidationContext,
    decision: &str,
    review_pack: Option<&Value>,
) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": context.job_id(),
        "stage": context.stage(),
        "task_id": context.task_id(),
        "decision": decision,
        "source_json_path": SENTINEL_REVIEW_PACK_JSON_PATH,
        "source_markdown_path": SENTINEL_REVIEW_PACK_MARKDOWN_PATH,
        "canonical_json_path": REVIEW_PACK_JSON_PATH,
        "canonical_markdown_path": REVIEW_PACK_MARKDOWN_PATH,
        "created_at": context.requested_at(),
        "questions_for_human": array_of_strings_from(review_pack, "questions_for_human")
    })
}

pub(crate) fn build_validation_run(context: &ValidationContext, next_state: &str) -> Value {
    let status = match next_state {
        "FAILED" => "ERROR",
        "BLOCKED" => "FAIL",
        _ => "PASS",
    };
    let exit_code = if status == "PASS" { 0 } else { 1 };
    json!({
        "id": format!(
            "{}-{}-star-sentinel-gate",
            context.job_id().to_lowercase(),
            context.stage()
        ),
        "command": "star-sentinel gate",
        "status": status,
        "exit_code": exit_code,
        "started_at": context.requested_at(),
        "finished_at": context.requested_at(),
        "log_path": SENTINEL_APPROVAL_PATH
    })
}

fn array_of_strings_from(value: Option<&Value>, field: &str) -> Vec<String> {
    value
        .and_then(|value| value.get(field))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}
