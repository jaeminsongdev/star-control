use crate::cloud_constants::{
    COST_METRIC_FILE, PRIVACY_HANDOFF_FILE, RESPONSE_FILE, STDERR_FILE, STDOUT_FILE,
};
use crate::cloud_policy::{currency, estimated_cost, CloudProviderPolicyDecision};
use crate::fake::provider_output_path;
use crate::{ExecutionRequest, ProviderInstance, ProviderManifest};
use serde_json::{json, Value};

pub(crate) fn response_value(
    request: &ExecutionRequest,
    manifest: &ProviderManifest,
    instance: &ProviderInstance,
    decision: &CloudProviderPolicyDecision,
    redaction_artifacts: &[String],
) -> Value {
    let mut artifacts = vec![
        provider_output_path(request.provider_instance_id(), RESPONSE_FILE),
        provider_output_path(request.provider_instance_id(), STDOUT_FILE),
        provider_output_path(request.provider_instance_id(), STDERR_FILE),
        provider_output_path(request.provider_instance_id(), PRIVACY_HANDOFF_FILE),
        provider_output_path(request.provider_instance_id(), COST_METRIC_FILE),
    ];
    artifacts.extend(redaction_artifacts.iter().cloned());

    json!({
        "schema_version": "1.0.0",
        "provider_instance_id": request.provider_instance_id(),
        "job_id": request.job_id(),
        "stage": request.stage(),
        "status": "blocked",
        "started_at": request.created_at(),
        "finished_at": request.created_at(),
        "stdout_path": provider_output_path(request.provider_instance_id(), STDOUT_FILE),
        "stderr_path": provider_output_path(request.provider_instance_id(), STDERR_FILE),
        "summary": format!("cloud provider preflight blocked: {}", decision.block.message),
        "changed_files": [],
        "artifacts": artifacts,
        "metrics": {
            "estimated_cost": estimated_cost(instance),
            "currency": currency(instance),
            "input_tokens": 0,
            "output_tokens": 0,
            "wall_time_ms": 0,
            "credential_ref_present": decision.credential_ref_present,
            "auth_mode_login_session": decision.auth_mode_login_session,
            "privacy_handoff_approved": decision.privacy_approved
        },
        "error": {
            "kind": decision.block.kind,
            "message": decision.block.message,
            "field": decision.block.field,
            "provider_id": manifest.id(),
            "provider_kind": manifest.kind(),
            "transport": manifest.transport()
        }
    })
}
