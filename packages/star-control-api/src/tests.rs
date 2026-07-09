mod control;
mod read_only;

use super::*;
use serde_json::{json, Value};
use star_control_daemon::{DaemonConfig, DaemonQueue};
use star_control_state::StateStore;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn schema_root() -> PathBuf {
    repo_root().join("specs/schemas")
}

fn temp_project() -> PathBuf {
    let count = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!(
        "star-control-api-{}-{}-{}",
        std::process::id(),
        timestamp_nanos(),
        count
    ));
    fs::create_dir_all(&path).expect("create temp project");
    path
}

fn timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn open_store(project: &Path) -> StateStore {
    StateStore::open(project, schema_root()).expect("open state store")
}

fn api_with_store(store: StateStore) -> ApiReadOnlyService {
    let mut service = ApiReadOnlyService::new(schema_root());
    service
        .register_project_store("local", store)
        .expect("register project");
    service
}

fn control_with_store(store: StateStore) -> ApiControlService {
    let mut service = ApiControlService::new(schema_root());
    service
        .register_project_store("local", store)
        .expect("register project");
    service
}

fn control_with_config_root(config_root: &Path) -> ApiControlService {
    let mut service = ApiControlService::new(schema_root());
    service.register_config_root(config_root.to_path_buf());
    service
}

fn open_daemon_queue(config_root: &Path) -> DaemonQueue {
    DaemonQueue::open(DaemonConfig::local(config_root, schema_root())).expect("open daemon queue")
}

fn create_job(store: &StateStore, state_name: &str, stage: &str) {
    let job = store
        .create_job("implement API", "README.md", Vec::new())
        .expect("create job");
    let job_id = job["job_id"].as_str().expect("job id");
    store
        .save_state(job_id, &run_state(job_id, state_name, stage))
        .expect("save state");
}

fn run_state(job_id: &str, state_name: &str, stage: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id,
        "state": state_name,
        "current_stage": stage,
        "updated_at": "unix:1",
        "workers": {},
        "artifacts": {},
        "next_action": "report"
    })
}

fn event(job_id: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "event_id": "J-0001-api-test",
        "job_id": job_id,
        "type": "STATE_CHANGED",
        "created_at": "unix:2",
        "stage": "implement",
        "state": "IMPLEMENTED",
        "message": "implemented",
        "artifact_paths": ["run-state.json"],
        "details": {}
    })
}

fn report(job_id: &str, risks: Vec<&str>) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id,
        "stage": "implement",
        "status": "DONE",
        "changed_files": [],
        "commands_run": [],
        "validation": [],
        "risks": risks,
        "blocked_reason": null,
        "next_step": "done",
        "artifacts": []
    })
}

fn approval_request(job_id: &str, stage: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id,
        "stage": stage,
        "task_id": "approval-1",
        "decision": "HUMAN_REVIEW",
        "reasons": ["test approval"],
        "changed_files": ["src/lib.rs"],
        "risks": ["requires human review"],
        "diagnostics": [],
        "review_pack_path": "review-packs/review_pack.md",
        "requested_at": "unix:1",
        "requested_by": "star-control-test"
    })
}
