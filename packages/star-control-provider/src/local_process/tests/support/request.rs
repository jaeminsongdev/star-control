use serde_json::{json, Value};

pub(crate) fn request_value() -> Value {
    json!({
        "schema_version": "1.0.0",
        "request_id": "request-0001",
        "job_id": "J-0001",
        "stage": "implement",
        "provider_instance_id": "local-default",
        "attempt_id": "attempt-0001",
        "workspec_path": "workspecs/implement.json",
        "created_at": "2026-06-28T00:00:00Z",
        "goal": "run local process provider",
        "allowed_scope": ["src/**", "tests/**"],
        "forbidden_actions": ["dependency_install", "file_delete"],
        "required_outputs": ["provider-output/local-default/response.json"],
        "validation_requirements": ["policy:p0"],
        "context_pack": { "files": [] }
    })
}

pub(crate) fn run_state(state: &str) -> Value {
    json!({
        "schema_version": "1.0.0",
        "job_id": "J-0001",
        "state": state,
        "current_stage": "implement",
        "updated_at": "test:deterministic",
        "threads": {},
        "workers": {},
        "artifacts": {},
        "latest_event_id": "",
        "active_provider": null,
        "next_action": if state == "CANCELLED" { "stop" } else { "continue" },
        "budget": {},
        "history": []
    })
}
