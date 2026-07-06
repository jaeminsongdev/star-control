use star_control_state::StateStore;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn schema_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../specs/schemas")
}

pub(super) fn temp_project(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "star-control-observability-{}-{}-{}",
        label,
        std::process::id(),
        nanos
    ));
    fs::create_dir_all(&path).expect("create temp project");
    path
}

pub(super) fn open_store(project: &Path) -> StateStore {
    StateStore::open(project, schema_root()).expect("open state store")
}

pub(super) fn create_job(store: &StateStore) {
    store
        .create_job("Audit event writer", ".", Vec::new())
        .expect("create job");
}
