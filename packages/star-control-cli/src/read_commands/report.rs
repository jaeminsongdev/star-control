use super::release::release_readiness_report_command;
use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::error::CliError;
use crate::output::{status_for_report, success_envelope};
use crate::{required_job, required_project};
use serde_json::{json, Value};
use star_control_security::redact_value_with_report;
use star_control_state::{StateStore, StateStoreError};

pub(crate) fn report_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    if parsed.has_recovery_source_selection() {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "report does not accept --recovery-artifact or --recovery-source".to_string(),
        });
    }
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
    let report_path = format!(".ai-runs/{}/reports/{}.json", job_id, report_name);
    let outcome = redact_value_with_report(report);
    let mut artifacts = vec![report_path.clone()];
    if outcome.redacted() {
        let redaction_file = format!("redaction-report-{}.json", stage);
        let redaction_path = format!(".ai-runs/{}/audit/{}", job_id, redaction_file);
        let redaction_report = outcome.report(&job_id, &format!("reports/{}.json", report_name));
        match store.write_redaction_report_json(&job_id, &redaction_file, &redaction_report) {
            Ok(_) | Err(StateStoreError::ArtifactAlreadyExists { .. }) => {
                artifacts.push(redaction_path);
            }
            Err(source) => {
                return Err(CliError::State {
                    command: parsed.command.clone(),
                    source,
                });
            }
        }
    }
    let report = outcome.into_value();

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
        artifacts,
    ))
}
