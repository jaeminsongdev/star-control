use crate::{JobSpec, RouterEngine, RouterOutput};
use serde_json::{json, Value};
use star_control_provider::ProviderRegistryLoader;
use std::path::PathBuf;

pub(super) fn route_for(request_text: &str, constraints: Vec<String>) -> RouterOutput {
    let registry = ProviderRegistryLoader::new(repo_root())
        .load_fake_default_registry()
        .expect("load registry");
    let engine = RouterEngine::new(&registry, schema_root());
    let job = job_spec(request_text, constraints);
    engine.route(&job).expect("route")
}

pub(super) fn job_spec(request_text: &str, constraints: Vec<String>) -> JobSpec {
    JobSpec::from_value(
        json!({
            "schema_version": "1.0.0",
            "job_id": "J-0001",
            "project_root": "D:/work/project",
            "request_text": request_text,
            "created_at": "2026-07-01T00:00:00Z",
            "updated_at": "2026-07-01T00:00:00Z",
            "entrypoint": "codex",
            "state": "REQUESTED",
            "user_constraints": constraints
        }),
        "job.json",
        schema_root(),
    )
    .expect("job spec")
}

pub(super) fn array_contains(values: &[Value], expected: &str) -> bool {
    values.iter().any(|value| value.as_str() == Some(expected))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("packages dir")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

pub(super) fn schema_root() -> PathBuf {
    repo_root().join("specs").join("schemas")
}
