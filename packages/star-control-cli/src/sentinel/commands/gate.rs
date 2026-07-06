use super::super::evaluation::evaluate_sentinel_job;
use super::super::options::reject_sentinel_command_options;
use super::super::paths::{sentinel_artifact_path, sentinel_schema_root};
use super::super::status::status_for_sentinel_decision;
use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::error::CliError;
use crate::output::success_envelope;
use crate::required_job;
use serde_json::{json, Value};
use star_sentinel::{write_gate_artifacts, DIAGNOSTICS_FILE};

pub(in crate::sentinel) fn sentinel_gate_command(
    parsed: &ParsedArgs,
    config: &CliConfig,
) -> Result<Value, CliError> {
    reject_sentinel_command_options(parsed, true)?;
    let job_id = required_job(parsed)?;
    let (store, task, _changed_lines, result) = evaluate_sentinel_job(parsed, config, &job_id)?;
    write_gate_artifacts(
        &store,
        &job_id,
        &task,
        &result,
        sentinel_schema_root(config),
    )
    .map_err(|source| CliError::Sentinel {
        command: parsed.command.clone(),
        source,
    })?;
    let diagnostics_path = sentinel_artifact_path(&job_id, DIAGNOSTICS_FILE);
    let approval_path = sentinel_artifact_path(&job_id, star_sentinel::APPROVAL_FILE);

    Ok(success_envelope(
        "sentinel",
        status_for_sentinel_decision(result.decision),
        json!({
            "subcommand": "gate",
            "job_id": job_id,
            "task_id": task.task_id,
            "decision": result.decision.as_str(),
            "diagnostic_count": result.diagnostics.len(),
            "diagnostics_path": diagnostics_path,
            "approval_path": approval_path,
            "actions_enabled": false
        }),
        vec![diagnostics_path, approval_path],
    ))
}
