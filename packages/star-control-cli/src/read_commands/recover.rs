use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::error::CliError;
use crate::output::success_envelope;
use crate::{required_job, required_project};
use serde_json::{json, Value};
use star_control_state::StateStore;

pub(crate) fn recover_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    let project = required_project(parsed)?;
    let job_id = required_job(parsed)?;
    if !parsed.recovery_list {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "recover currently supports --list only".to_string(),
        });
    }
    if parsed.release_readiness
        || parsed.stage.is_some()
        || parsed.markdown
        || parsed.dry_run
        || parsed.request.is_some()
        || parsed.entrypoint.is_some()
        || parsed.provider.is_some()
        || !parsed.provider_instances.is_empty()
        || parsed.response.is_some()
        || parsed.reason.is_some()
        || !parsed.constraints.is_empty()
    {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "recover --list only accepts --project, --job, --list, and --json".to_string(),
        });
    }

    let store =
        StateStore::open(&project, config.schema_root()).map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let inspection = store
        .inspect_recovery(&job_id)
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let inspection_value = inspection.to_value();
    let mut artifacts = vec![
        format!(".ai-runs/{}/job.json", job_id),
        format!(".ai-runs/{}/run-state.json", job_id),
        format!(".ai-runs/{}/events.jsonl", job_id),
    ];
    artifacts.extend(
        inspection
            .issues
            .iter()
            .map(|issue| format!(".ai-runs/{}/{}", job_id, issue.artifact_path)),
    );
    artifacts.sort();
    artifacts.dedup();

    Ok(success_envelope(
        "recover",
        "success",
        json!({
            "job_id": job_id,
            "mode": "inspect_only",
            "recovery_actions_enabled": false,
            "recovery": inspection_value
        }),
        artifacts,
    ))
}
