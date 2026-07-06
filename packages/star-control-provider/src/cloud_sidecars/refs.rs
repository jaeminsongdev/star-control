use crate::cloud_constants::*;
use crate::fake::provider_output_path;
use crate::{ExecutionRequest, ProviderAdapterError, ProviderExecution, ProviderRunContext};
use serde_json::Value;
use star_control_state::ArtifactKind;

pub(crate) fn planned_output_files(provider_instance_id: &str) -> Vec<String> {
    vec![
        provider_output_path(provider_instance_id, REQUEST_FILE),
        provider_output_path(provider_instance_id, RESPONSE_FILE),
        provider_output_path(provider_instance_id, HTTP_REQUEST_FILE),
        provider_output_path(provider_instance_id, HTTP_TRANSPORT_PLAN_FILE),
        provider_output_path(provider_instance_id, LIVE_TRANSPORT_APPROVAL_FILE),
        provider_output_path(provider_instance_id, RAW_RESPONSE_FILE),
        provider_output_path(provider_instance_id, STDOUT_FILE),
        provider_output_path(provider_instance_id, STDERR_FILE),
        provider_output_path(provider_instance_id, PRIVACY_HANDOFF_FILE),
        provider_output_path(provider_instance_id, COST_METRIC_FILE),
    ]
}

pub(crate) fn artifact_ref(
    context: &ProviderRunContext<'_>,
    request: &ExecutionRequest,
    file_name: &str,
) -> Result<Value, ProviderAdapterError> {
    Ok(context.state_store().artifact_ref(
        request.job_id(),
        &provider_output_path(request.provider_instance_id(), file_name),
        ArtifactKind::Log,
        request.provider_instance_id(),
        None,
        Some("provider text output"),
    )?)
}

pub(crate) fn assert_provider_sidecar_refs(
    _execution: &ProviderExecution,
    privacy_ref: &Value,
    cost_ref: &Value,
) {
    debug_assert_eq!(privacy_ref["kind"], "provider_output");
    debug_assert_eq!(cost_ref["kind"], "provider_output");
}
