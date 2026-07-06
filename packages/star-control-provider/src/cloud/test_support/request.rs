use serde_json::{json, Value};

pub(super) fn request_value() -> Value {
    json!({
        "schema_version": "1.0.0",
        "request_id": "request-0001",
        "job_id": "J-0001",
        "stage": "implement",
        "provider_instance_id": "cloud-default",
        "attempt_id": "attempt-0001",
        "workspec_path": "workspecs/implement.json",
        "created_at": "2026-06-28T00:00:00Z",
        "goal": "run cloud provider",
        "allowed_scope": ["src/**", "tests/**"],
        "forbidden_actions": ["dependency_install", "file_delete"],
        "required_outputs": ["provider-output/cloud-default/response.json"],
        "validation_requirements": ["policy:p0"],
        "context_pack": { "files": [] }
    })
}
