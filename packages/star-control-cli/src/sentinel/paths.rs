use crate::config::CliConfig;
use crate::error::CliError;
use star_control_state::StateStore;
use star_sentinel::STAR_SENTINEL_TOOL_OUTPUT_DIR;
use std::path::PathBuf;

pub(super) fn require_sentinel_input(
    store: &StateStore,
    job_id: &str,
    file_name: &str,
    schema_name: &str,
) -> Result<PathBuf, CliError> {
    let relative_path = format!(
        "tool-output/{}/{}",
        STAR_SENTINEL_TOOL_OUTPUT_DIR, file_name
    );
    let path = store
        .resolve_job_path(job_id, &relative_path)
        .map_err(|source| CliError::State {
            command: "sentinel".to_string(),
            source,
        })?;
    if path.is_file() {
        Ok(path)
    } else {
        Err(CliError::MissingArtifact {
            command: "sentinel".to_string(),
            message: format!(
                "required Star Sentinel input not found: {} ({})",
                relative_path, schema_name
            ),
            artifact_paths: vec![format!(".ai-runs/{}/{}", job_id, relative_path)],
        })
    }
}

pub(super) fn sentinel_schema_root(config: &CliConfig) -> PathBuf {
    config
        .repo_root()
        .join("builtin-tools")
        .join("star-sentinel")
        .join("schemas")
}

pub(super) fn sentinel_registry_path(config: &CliConfig) -> PathBuf {
    config
        .repo_root()
        .join("builtin-tools")
        .join("star-sentinel")
        .join("policies")
        .join("p0-rule-registry.json")
}

pub(super) fn sentinel_artifact_path(job_id: &str, file_name: &str) -> String {
    format!(
        ".ai-runs/{}/tool-output/{}/{}",
        job_id, STAR_SENTINEL_TOOL_OUTPUT_DIR, file_name
    )
}
