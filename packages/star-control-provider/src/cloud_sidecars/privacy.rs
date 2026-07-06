use crate::cloud_constants::REQUEST_FILE;
use crate::fake::provider_output_path;
use crate::{ExecutionRequest, ProviderManifest};
use serde_json::{json, Value};

pub(crate) fn privacy_handoff_value(
    request: &ExecutionRequest,
    manifest: &ProviderManifest,
    approved: bool,
) -> Value {
    json!({
        "schema_version": "1.0.0",
        "job_id": request.job_id(),
        "destination": manifest.id(),
        "context_paths": [
            request.workspec_path(),
            provider_output_path(request.provider_instance_id(), REQUEST_FILE)
        ],
        "redaction_required": true,
        "approved": approved,
        "notes": "Cloud provider preflight records handoff scope before any external transport execution."
    })
}
