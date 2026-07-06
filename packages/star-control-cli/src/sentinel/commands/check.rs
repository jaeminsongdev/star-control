use super::super::evaluation::evaluate_sentinel_job;
use super::super::options::reject_sentinel_command_options;
use super::super::paths::{sentinel_artifact_path, sentinel_schema_root};
use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::error::CliError;
use crate::output::success_envelope;
use crate::required_job;
use serde_json::{json, Value};
use star_sentinel::{
    build_diagnostics_artifact, validate_diagnostics_artifact, DIAGNOSTICS_FILE,
    STAR_SENTINEL_TOOL_OUTPUT_DIR,
};

pub(in crate::sentinel) fn sentinel_check_command(
    parsed: &ParsedArgs,
    config: &CliConfig,
) -> Result<Value, CliError> {
    reject_sentinel_command_options(parsed, true)?;
    let job_id = required_job(parsed)?;
    let (store, task, _changed_lines, result) = evaluate_sentinel_job(parsed, config, &job_id)?;
    let diagnostics = build_diagnostics_artifact(&result);
    let sentinel_schema_root = sentinel_schema_root(config);
    validate_diagnostics_artifact(&diagnostics, &sentinel_schema_root).map_err(|source| {
        CliError::Sentinel {
            command: parsed.command.clone(),
            source,
        }
    })?;
    store
        .write_tool_json(
            &job_id,
            STAR_SENTINEL_TOOL_OUTPUT_DIR,
            DIAGNOSTICS_FILE,
            &diagnostics,
        )
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let diagnostics_path = sentinel_artifact_path(&job_id, DIAGNOSTICS_FILE);

    Ok(success_envelope(
        "sentinel",
        "success",
        json!({
            "subcommand": "check",
            "job_id": job_id,
            "task_id": task.task_id,
            "decision": result.decision.as_str(),
            "diagnostic_count": result.diagnostics.len(),
            "diagnostics": diagnostics,
            "diagnostics_path": diagnostics_path,
            "actions_enabled": false
        }),
        vec![diagnostics_path],
    ))
}
