use super::SmokeFixture;
use star_control_state::StateStore;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

impl SmokeFixture {
    pub(crate) fn new() -> Self {
        let project = temp_project();
        let repo_root = repo_root();
        let core_schema_root = repo_root.join("specs/schemas");
        let sentinel_schema_root = repo_root.join("builtin-tools/star-sentinel/schemas");
        let store = StateStore::open(&project, &core_schema_root).expect("state store");
        Self {
            project,
            repo_root,
            core_schema_root,
            sentinel_schema_root,
            store,
        }
    }
}

impl Drop for SmokeFixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.project).ok();
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("packages dir")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn temp_project() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "star-control-v0-smoke-{}-{}",
        std::process::id(),
        nanos
    ));
    fs::create_dir_all(&path).expect("create temp project");
    path
}
