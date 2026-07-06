use crate::{
    ExecutionRequest, FakeProviderAdapter, ProviderAdapter, ProviderExecution,
    ProviderRegistryLoader, ProviderRunContext,
};
use serde_json::{json, Value};
use star_control_state::StateStore;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_PROJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(super) fn execute_with_adapter(adapter: FakeProviderAdapter) -> ProviderExecution {
    let project = temp_project();
    let store = open_store(&project);
    store
        .create_job("implement feature", "codex", vec![])
        .expect("create job");
    let registry = ProviderRegistryLoader::new(repo_root())
        .load_fake_default_registry()
        .expect("load fake registry");
    let request =
        ExecutionRequest::from_value(request_value("simulate"), "request.json", schema_root())
            .expect("request");
    let schemas = schema_root();
    let context = ProviderRunContext::new(&registry, &store, &schemas);
    let execution = adapter.execute(&request, &context).expect("execute");
    fs::remove_dir_all(project).ok();
    execution
}

pub(super) fn request_value(goal: &str) -> Value {
    json!({
        "schema_version": "1.0.0",
        "request_id": "request-0001",
        "job_id": "J-0001",
        "stage": "implement",
        "provider_instance_id": "fake-default",
        "attempt_id": "attempt-0001",
        "workspec_path": "workspecs/implement.json",
        "created_at": "2026-06-28T00:00:00Z",
        "goal": goal,
        "allowed_scope": ["src/**", "tests/**"],
        "forbidden_actions": ["dependency_install", "file_delete"],
        "required_outputs": ["provider-output/fake-default/response.json"],
        "validation_requirements": ["policy:p0"],
        "context_pack": { "files": [] }
    })
}

pub(super) fn repo_root() -> PathBuf {
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

pub(super) fn temp_project() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let counter = TEMP_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "star-control-provider-fake-{}-{}-{}",
        std::process::id(),
        nanos,
        counter
    ));
    fs::create_dir_all(&path).expect("create temp project");
    path
}

pub(super) fn open_store(project_root: &Path) -> StateStore {
    StateStore::open(project_root, schema_root()).expect("open state store")
}
