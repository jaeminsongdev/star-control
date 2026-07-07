use super::ProviderAdapterError;
use crate::provider_cost::COST_METRIC_FILE;
use star_control_state::StateStore;

pub(super) fn planned_output_files(
    provider_instance_id: &str,
    include_stderr: bool,
) -> Vec<String> {
    let mut files = vec![
        provider_output_path(provider_instance_id, "request.json"),
        provider_output_path(provider_instance_id, "stdout.txt"),
        provider_output_path(provider_instance_id, "response.json"),
        provider_output_path(provider_instance_id, COST_METRIC_FILE),
    ];
    if include_stderr {
        files.push(provider_output_path(provider_instance_id, "stderr.txt"));
    }
    files
}

pub(crate) fn provider_output_path(provider_instance_id: &str, file_name: &str) -> String {
    format!("provider-output/{}/{}", provider_instance_id, file_name)
}

pub(crate) fn ensure_output_files_absent(
    state_store: &StateStore,
    job_id: &str,
    relative_paths: &[String],
) -> Result<(), ProviderAdapterError> {
    for relative_path in relative_paths {
        let path = state_store.resolve_job_path(job_id, relative_path)?;
        if path.exists() {
            return Err(ProviderAdapterError::ProviderOutputAlreadyExists { path });
        }
    }
    Ok(())
}
