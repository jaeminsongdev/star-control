use crate::{ExecutionRequest, ProviderInstance};
use serde_json::{json, Value};

mod request;
mod response;

pub(super) fn request_value(goal: &str) -> ExecutionRequest {
    ExecutionRequest::from_value(
        json!({
            "schema_version": "1.0.0",
            "request_id": "request-0001",
            "job_id": "J-0001",
            "stage": "implement",
            "provider_instance_id": "openai-default",
            "attempt_id": "attempt-0001",
            "workspec_path": "workspecs/implement.json",
            "created_at": "2026-06-28T00:00:00Z",
            "goal": goal,
            "allowed_scope": ["src/**", "tests/**"],
            "forbidden_actions": ["dependency_install", "file_delete"],
            "required_outputs": ["provider-output/openai-default/response.json"],
            "validation_requirements": ["policy:p0"],
            "context_pack": { "files": [] }
        }),
        "request.json",
        schema_root(),
    )
    .expect("execution request")
}

pub(super) fn provider_instance(value: Value) -> ProviderInstance {
    ProviderInstance {
        id: "openai-default".to_string(),
        provider_id: "provider.openai".to_string(),
        enabled: true,
        routing_tags: vec!["cloud".to_string(), "api".to_string()],
        path: "openai-default.json".into(),
        value,
    }
}

fn schema_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("packages dir")
        .parent()
        .expect("repo root")
        .join("specs")
        .join("schemas")
}
