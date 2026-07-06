use super::super::{create_job, open_store, temp_project};
use crate::StateStoreError;
use std::fs;

#[test]
fn reports_missing_job_and_invalid_json() {
    let project = temp_project();
    let store = open_store(&project);
    assert!(matches!(
        store.load_job("J-9999"),
        Err(StateStoreError::JobNotFound { .. })
    ));

    create_job(&store);
    fs::write(
        project.join(".ai-runs/J-0001/run-state.json"),
        "{ invalid json",
    )
    .expect("write invalid state");

    assert!(matches!(
        store.load_state("J-0001"),
        Err(StateStoreError::InvalidJson { .. })
    ));

    fs::remove_dir_all(project).ok();
}

#[test]
fn blocks_path_traversal_absolute_paths_and_git_paths() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store);

    assert!(matches!(
        store.resolve_job_path("J-0001", "../outside.json"),
        Err(StateStoreError::PathTraversalBlocked { .. })
    ));
    assert!(matches!(
        store.resolve_job_path("J-0001", "C:\\temp\\file.json"),
        Err(StateStoreError::PathTraversalBlocked { .. })
    ));
    assert!(matches!(
        store.resolve_job_path("J-0001", ".git/config"),
        Err(StateStoreError::PathTraversalBlocked { .. })
    ));

    fs::remove_dir_all(project).ok();
}

#[test]
fn tmp_files_are_not_read_as_artifacts() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store);
    fs::write(
        project.join(".ai-runs/J-0001/tmp/run-state.json.tmp-test"),
        "{ invalid json",
    )
    .expect("write tmp file");

    assert!(matches!(
        store.load_state("J-0001"),
        Err(StateStoreError::ArtifactNotFound { .. })
    ));

    fs::remove_dir_all(project).ok();
}
