mod artifacts;
mod recovery;
mod store;

use crate::{StateStore, SCHEMA_VERSION};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_PROJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(super) fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub(super) fn schema_root() -> PathBuf {
    repo_root().join("specs/schemas")
}

pub(super) fn temp_project() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let counter = TEMP_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "star-control-state-{}-{}-{}",
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

pub(super) fn create_job(store: &StateStore) -> Value {
    store
        .create_job("implement feature", "codex", vec!["no deploy".to_string()])
        .expect("create job")
}

pub(super) fn state(job_id: &str, state: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id,
        "state": state,
        "current_stage": "route",
        "updated_at": "2026-07-01T00:00:00Z",
        "threads": {},
        "workers": {},
        "artifacts": {},
        "latest_event_id": "EV-0001",
        "active_provider": null,
        "next_action": "continue",
        "budget": {},
        "history": []
    })
}

pub(super) fn event(job_id: &str, event_id: &str, message: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "event_id": event_id,
        "job_id": job_id,
        "type": "STATE_CHANGED",
        "created_at": "2026-07-01T00:00:00Z",
        "stage": "route",
        "state": "ROUTING",
        "message": message,
        "artifact_paths": ["run-state.json"],
        "details": {}
    })
}

pub(super) fn read_example(relative_path: &str) -> Value {
    let path = repo_root().join(relative_path);
    serde_json::from_str(&fs::read_to_string(path).expect("read example")).expect("parse example")
}
