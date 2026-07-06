use crate::constants::SCHEMA_VERSION;
use crate::io::timestamp_nanos;
use crate::{DaemonConfig, DaemonQueue};
use serde_json::{json, Value};
use star_control_state::StateStore;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn schema_root() -> PathBuf {
    repo_root().join("specs/schemas")
}

pub(super) fn temp_dir(name: &str) -> PathBuf {
    let count = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!(
        "star-control-daemon-{}-{}-{}-{}",
        name,
        std::process::id(),
        timestamp_nanos(),
        count
    ));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

pub(super) fn open_project_store(project: &Path) -> StateStore {
    StateStore::open(project, schema_root()).expect("open project store")
}

pub(super) fn open_daemon_queue(config_root: &Path) -> DaemonQueue {
    DaemonQueue::open(DaemonConfig::local(config_root, schema_root())).expect("open daemon queue")
}

pub(super) fn create_job(store: &StateStore, state_name: &str, stage: &str) {
    let job = store
        .create_job("test request", "README.md", Vec::new())
        .expect("create job");
    let job_id = job["job_id"].as_str().expect("job id");
    assert_eq!(job_id, "J-0001");
    store
        .save_state(job_id, &run_state(job_id, state_name, stage))
        .expect("save run state");
}

fn run_state(job_id: &str, state_name: &str, stage: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id,
        "state": state_name,
        "current_stage": stage,
        "workers": {},
        "artifacts": {},
        "next_action": "run"
    })
}

pub(super) fn approval_response(job_id: &str, response: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id,
        "stage": "validate",
        "task_id": "approval-1",
        "response": response,
        "reviewer": "test",
        "responded_at": "unix:1",
        "reason": "test",
        "allowed_next_stage": "report",
        "constraints": []
    })
}

pub(super) fn cleanup_dirs(project: PathBuf, config: PathBuf) {
    fs::remove_dir_all(project).ok();
    fs::remove_dir_all(config).ok();
}
