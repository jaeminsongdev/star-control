use star_control_state::StateStore;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_PROJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("packages dir")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

pub(crate) fn schema_root() -> PathBuf {
    repo_root().join("specs").join("schemas")
}

pub(crate) fn temp_project() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let counter = TEMP_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "star-control-provider-local-process-{}-{}-{}",
        std::process::id(),
        nanos,
        counter
    ));
    fs::create_dir_all(&path).expect("create temp project");
    path
}

pub(crate) fn open_store(project_root: &Path) -> StateStore {
    StateStore::open(project_root, schema_root()).expect("open state store")
}
