use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::error::CliError;
use crate::output::success_envelope;
use serde_json::{json, Value};
use star_control_release::{ReleaseReadinessWriter, RELEASE_READINESS_PATH};
use star_control_state::StateStore;
use std::path::PathBuf;

pub(super) fn release_readiness_report_command(
    parsed: &ParsedArgs,
    config: &CliConfig,
    project: PathBuf,
    job_id: String,
) -> Result<Value, CliError> {
    if parsed.stage.is_some() {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "--stage cannot be combined with --release-readiness".to_string(),
        });
    }
    let store =
        StateStore::open(&project, config.schema_root()).map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    store.load_job(&job_id).map_err(|source| CliError::State {
        command: parsed.command.clone(),
        source,
    })?;
    let writer = ReleaseReadinessWriter::new(config.schema_root());
    let readiness = writer
        .read(&store, &job_id)
        .map_err(|source| CliError::ReleaseReadiness {
            command: parsed.command.clone(),
            source,
        })?
        .ok_or_else(|| CliError::MissingArtifact {
            command: parsed.command.clone(),
            message: "release readiness artifact not found".to_string(),
            artifact_paths: vec![format!(".ai-runs/{}/{}", job_id, RELEASE_READINESS_PATH)],
        })?;

    Ok(success_envelope(
        "report",
        "success",
        json!({
            "job_id": job_id,
            "report_kind": "release_readiness",
            "release_readiness_path": format!(".ai-runs/{}/{}", job_id, RELEASE_READINESS_PATH),
            "release_actions_enabled": false,
            "readiness": readiness
        }),
        vec![format!(".ai-runs/{}/{}", job_id, RELEASE_READINESS_PATH)],
    ))
}
