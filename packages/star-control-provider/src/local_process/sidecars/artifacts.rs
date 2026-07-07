use crate::fake::provider_output_path;
use crate::local_process::constants::{STDERR_FILE, STDOUT_FILE};
use crate::provider_cost::COST_METRIC_FILE;
use crate::{ExecutionRequest, ProviderAdapterError, ProviderRunContext};
use serde_json::Value;
use star_control_state::ArtifactKind;

pub(crate) fn planned_output_files(provider_instance_id: &str) -> Vec<String> {
    vec![
        provider_output_path(provider_instance_id, "request.json"),
        provider_output_path(provider_instance_id, STDOUT_FILE),
        provider_output_path(provider_instance_id, STDERR_FILE),
        provider_output_path(provider_instance_id, "response.json"),
        provider_output_path(provider_instance_id, COST_METRIC_FILE),
    ]
}

pub(crate) fn artifact_ref(
    context: &ProviderRunContext<'_>,
    request: &ExecutionRequest,
    relative_path: &str,
) -> Result<Value, ProviderAdapterError> {
    Ok(context.state_store().artifact_ref(
        request.job_id(),
        relative_path,
        ArtifactKind::Log,
        request.provider_instance_id(),
        None,
        Some("provider text output"),
    )?)
}
