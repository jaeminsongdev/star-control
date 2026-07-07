use super::artifacts::LiveApprovalArtifacts;
use crate::cloud_api_artifacts::api_live_approval_stdout_value;
use crate::cloud_constants::{
    COST_METRIC_FILE, COST_METRIC_SCHEMA, PRIVACY_HANDOFF_FILE, PRIVACY_HANDOFF_SCHEMA,
    STDERR_FILE, STDOUT_FILE,
};
use crate::cloud_io::validate_contract;
use crate::cloud_sidecars::{cost_metric_value_with_wall_time, privacy_handoff_value};
use crate::provider_redaction::redact_provider_text_artifact;
use crate::{
    ExecutionRequest, ProviderAdapterError, ProviderInstance, ProviderManifest, ProviderRunContext,
};
use serde_json::Value;
use std::path::Path;

pub(super) struct LiveApprovalSidecarRefs {
    pub(super) privacy_ref: Value,
    pub(super) cost_ref: Value,
    pub(super) stdout_ref: Value,
    pub(super) stderr_ref: Value,
    pub(super) redaction_artifacts: Vec<String>,
}

pub(super) fn write_live_approval_sidecars(
    request: &ExecutionRequest,
    context: &ProviderRunContext<'_>,
    manifest: &ProviderManifest,
    instance: &ProviderInstance,
    artifacts: &LiveApprovalArtifacts,
) -> Result<LiveApprovalSidecarRefs, ProviderAdapterError> {
    let privacy_handoff = privacy_handoff_value(request, manifest, true);
    validate_contract(
        &privacy_handoff,
        Path::new(PRIVACY_HANDOFF_FILE),
        context.schema_root(),
        PRIVACY_HANDOFF_SCHEMA,
    )?;
    let privacy_ref = context.state_store().write_provider_json(
        request.job_id(),
        request.provider_instance_id(),
        PRIVACY_HANDOFF_FILE,
        &privacy_handoff,
    )?;

    let cost_metric = cost_metric_value_with_wall_time(request, instance, artifacts.wall_time_ms);
    validate_contract(
        &cost_metric,
        Path::new(COST_METRIC_FILE),
        context.schema_root(),
        COST_METRIC_SCHEMA,
    )?;
    let cost_ref = context.state_store().write_provider_json(
        request.job_id(),
        request.provider_instance_id(),
        COST_METRIC_FILE,
        &cost_metric,
    )?;

    let stdout = api_live_approval_stdout_value(manifest, &artifacts.prepared_request);
    let stdout_redaction = redact_provider_text_artifact(context, request, STDOUT_FILE, &stdout)?;
    let stdout_ref = context.state_store().write_provider_text(
        request.job_id(),
        request.provider_instance_id(),
        STDOUT_FILE,
        stdout_redaction.content(),
    )?;
    let stderr = "blocked kind=cloud_api_live_transport_approval_required field=transport_config.live_api_call_requested message=cloud API live HTTP transport requires explicit approval before credential lookup or external API call\n";
    let stderr_redaction = redact_provider_text_artifact(context, request, STDERR_FILE, stderr)?;
    let stderr_ref = context.state_store().write_provider_text(
        request.job_id(),
        request.provider_instance_id(),
        STDERR_FILE,
        stderr_redaction.content(),
    )?;

    Ok(LiveApprovalSidecarRefs {
        privacy_ref,
        cost_ref,
        stdout_ref,
        stderr_ref,
        redaction_artifacts: [
            stdout_redaction.report_path().map(ToString::to_string),
            stderr_redaction.report_path().map(ToString::to_string),
        ]
        .into_iter()
        .flatten()
        .collect(),
    })
}
