use super::release::release_readiness_report_command;
use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::error::CliError;
use crate::output::{status_for_report, success_envelope};
use crate::{required_job, required_project};
use serde_json::{json, Value};
use star_control_state::{StateStore, StateStoreError};

pub(crate) fn report_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    let project = required_project(parsed)?;
    let job_id = required_job(parsed)?;
    if parsed.release_readiness {
        return release_readiness_report_command(parsed, config, project, job_id);
    }
    let stage = parsed.stage.as_deref().unwrap_or("implement");
    let store =
        StateStore::open(&project, config.schema_root()).map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let report_name = format!("{}-report", stage);
    let report = store
        .load_report(&job_id, &report_name)
        .map_err(|source| match source {
            StateStoreError::ArtifactNotFound { .. } => CliError::MissingArtifact {
                command: parsed.command.clone(),
                message: format!("report artifact not found for stage {}", stage),
                artifact_paths: vec![format!(".ai-runs/{}/reports/{}.json", job_id, report_name)],
            },
            source => CliError::State {
                command: parsed.command.clone(),
                source,
            },
        })?;

    Ok(success_envelope(
        "report",
        status_for_report(
            report
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("FAILED"),
        ),
        json!({
            "job_id": job_id,
            "stage": stage,
            "report": report
        }),
        vec![format!(".ai-runs/{}/reports/{}.json", job_id, report_name)],
    ))
}
