use crate::{
    CompleteImplementationAuditCheck, M9ReadinessCheck, COMPLETE_IMPLEMENTATION_REQUIRED_CHECKS,
    M9_REQUIRED_READINESS_CHECKS,
};
use star_control_state::StateStore;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_PROJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn create_job(store: &StateStore) {
    store
        .create_job("request", "cli", Vec::new())
        .expect("create job");
}

pub(crate) fn open_store(project_root: &Path) -> StateStore {
    StateStore::open(project_root, schema_root()).expect("open state store")
}

pub(crate) fn schema_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("packages dir")
        .parent()
        .expect("repo root")
        .join("specs")
        .join("schemas")
}

pub(crate) fn temp_project(label: &str) -> PathBuf {
    let counter = TEMP_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "star-control-release-{}-{}-{}",
        std::process::id(),
        counter,
        label
    ));
    fs::create_dir_all(&path).expect("create temp project");
    path
}

pub(crate) fn all_m9_readiness_checks_passed() -> Vec<M9ReadinessCheck> {
    M9_REQUIRED_READINESS_CHECKS
        .iter()
        .map(|check_name| {
            M9ReadinessCheck::passed(
                *check_name,
                vec![format!("docs/implementation/briefs/{}.md", check_name)],
            )
            .expect("M9 readiness check")
        })
        .collect()
}

pub(crate) fn all_complete_implementation_checks_passed() -> Vec<CompleteImplementationAuditCheck> {
    COMPLETE_IMPLEMENTATION_REQUIRED_CHECKS
        .iter()
        .map(|check_name| {
            CompleteImplementationAuditCheck::passed(
                *check_name,
                vec![format!("docs/implementation/audit/{}.md", check_name)],
            )
            .expect("complete implementation check")
        })
        .collect()
}
