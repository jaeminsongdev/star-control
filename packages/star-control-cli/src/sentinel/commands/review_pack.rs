use super::super::evaluation::evaluate_sentinel_job;
use super::super::options::reject_sentinel_command_options;
use super::super::paths::{sentinel_artifact_path, sentinel_schema_root};
use super::super::status::{status_for_sentinel_decision, validation_result_for_sentinel_decision};
use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::error::CliError;
use crate::output::success_envelope;
use crate::required_job;
use serde_json::{json, Value};
use star_sentinel::{build_review_pack_artifact, write_review_pack_artifacts, ReviewValidation};

pub(in crate::sentinel) fn sentinel_review_pack_command(
    parsed: &ParsedArgs,
    config: &CliConfig,
) -> Result<Value, CliError> {
    reject_sentinel_command_options(parsed, true)?;
    let job_id = required_job(parsed)?;
    let (store, task, changed_lines, result) = evaluate_sentinel_job(parsed, config, &job_id)?;
    let review_pack = build_review_pack_artifact(
        &task,
        &changed_lines,
        &result,
        &[ReviewValidation::new(
            "star-control sentinel check",
            validation_result_for_sentinel_decision(result.decision),
        )],
    );
    write_review_pack_artifacts(&store, &job_id, &review_pack, sentinel_schema_root(config))
        .map_err(|source| CliError::Sentinel {
            command: parsed.command.clone(),
            source,
        })?;
    let tool_json_path = sentinel_artifact_path(&job_id, star_sentinel::REVIEW_PACK_JSON_FILE);
    let tool_markdown_path =
        sentinel_artifact_path(&job_id, star_sentinel::REVIEW_PACK_MARKDOWN_FILE);
    let review_json_path = format!(
        ".ai-runs/{}/review-packs/{}",
        job_id,
        star_sentinel::REVIEW_PACK_JSON_FILE
    );
    let review_markdown_path = format!(
        ".ai-runs/{}/review-packs/{}",
        job_id,
        star_sentinel::REVIEW_PACK_MARKDOWN_FILE
    );

    Ok(success_envelope(
        "sentinel",
        status_for_sentinel_decision(result.decision),
        json!({
            "subcommand": "review-pack",
            "job_id": job_id,
            "task_id": task.task_id,
            "decision": result.decision.as_str(),
            "review_pack_path": review_markdown_path,
            "tool_review_pack_path": tool_markdown_path,
            "actions_enabled": false
        }),
        vec![
            tool_json_path,
            tool_markdown_path,
            review_json_path,
            review_markdown_path,
        ],
    ))
}
