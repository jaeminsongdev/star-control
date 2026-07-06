use super::paths::{require_sentinel_input, sentinel_registry_path, sentinel_schema_root};
use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::error::CliError;
use crate::required_project;
use star_control_state::StateStore;
use star_sentinel::{
    read_changed_lines, read_p0_rule_registry, read_task, ChangedLines, EvaluationResult,
    P0Evaluator, SentinelTask, CHANGED_LINES_SCHEMA, SENTINEL_TASK_SCHEMA,
};

pub(super) fn evaluate_sentinel_job(
    parsed: &ParsedArgs,
    config: &CliConfig,
    job_id: &str,
) -> Result<(StateStore, SentinelTask, ChangedLines, EvaluationResult), CliError> {
    let project = required_project(parsed)?;
    let store =
        StateStore::open(&project, config.schema_root()).map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let task_path = require_sentinel_input(&store, job_id, "task.json", SENTINEL_TASK_SCHEMA)?;
    let changed_lines_path =
        require_sentinel_input(&store, job_id, "changed_lines.json", CHANGED_LINES_SCHEMA)?;
    let sentinel_schema_root = sentinel_schema_root(config);
    let task =
        read_task(&task_path, &sentinel_schema_root).map_err(|source| CliError::Sentinel {
            command: parsed.command.clone(),
            source,
        })?;
    let changed_lines =
        read_changed_lines(&changed_lines_path, &sentinel_schema_root).map_err(|source| {
            CliError::Sentinel {
                command: parsed.command.clone(),
                source,
            }
        })?;
    let registry = read_p0_rule_registry(sentinel_registry_path(config), &sentinel_schema_root)
        .map_err(|source| CliError::Sentinel {
            command: parsed.command.clone(),
            source,
        })?;
    let result = P0Evaluator::new(registry)
        .evaluate(&task, &changed_lines)
        .map_err(|source| CliError::Sentinel {
            command: parsed.command.clone(),
            source,
        })?;
    Ok((store, task, changed_lines, result))
}
