use super::builders::state;
use crate::ValidationEngine;
use serde_json::json;
use star_control_state::StateStore;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_PROJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) struct Fixture {
    pub(crate) project: PathBuf,
    pub(crate) store: StateStore,
    core_schema_root: PathBuf,
    sentinel_schema_root: PathBuf,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        let project = temp_project();
        let root = repo_root();
        let core_schema_root = root.join("specs/schemas");
        let sentinel_schema_root = root.join("builtin-tools/star-sentinel/schemas");
        let store = StateStore::open(&project, &core_schema_root).expect("store");
        Self {
            project,
            store,
            core_schema_root,
            sentinel_schema_root,
        }
    }

    pub(crate) fn engine(&self) -> ValidationEngine<'_> {
        ValidationEngine::new(
            &self.store,
            &self.core_schema_root,
            &self.sentinel_schema_root,
        )
    }

    pub(crate) fn create_job_with_state(&self) {
        self.store
            .create_job("validate p0 output", "validation-engine", Vec::new())
            .expect("job");
        self.store
            .save_state("J-0001", &state("J-0001", "VALIDATING"))
            .expect("state");
    }

    pub(crate) fn write_provider_response(&self) {
        self.store
            .write_provider_json(
                "J-0001",
                "fake-default",
                "response.json",
                &json!({ "ok": true }),
            )
            .expect("provider response");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.project).ok();
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("package parent")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn temp_project() -> PathBuf {
    let counter = TEMP_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "star-control-validation-{}-{}-{}",
        std::process::id(),
        counter,
        nanos
    ));
    fs::create_dir_all(&path).expect("create temp project");
    path
}
