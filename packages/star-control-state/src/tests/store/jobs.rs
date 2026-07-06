use super::super::{create_job, open_store, state, temp_project};
use crate::StateStoreError;
use std::fs;

#[test]
fn creates_ai_runs_and_first_job_directory() {
    let project = temp_project();
    let store = open_store(&project);
    let job = create_job(&store);

    assert_eq!(job["job_id"], "J-0001");
    assert!(project.join(".ai-runs/J-0001/job.json").is_file());
    assert!(project.join(".ai-runs/J-0001/workspecs").is_dir());
    assert!(project.join(".ai-runs/J-0001/tmp").is_dir());

    fs::remove_dir_all(project).ok();
}

#[test]
fn allocates_next_job_id_from_existing_jobs() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store);
    let second = create_job(&store);

    assert_eq!(second["job_id"], "J-0002");

    fs::remove_dir_all(project).ok();
}

#[test]
fn job_and_state_roundtrip_with_schema_validation() {
    let project = temp_project();
    let store = open_store(&project);
    let job = create_job(&store);
    let job_id = job["job_id"].as_str().unwrap();

    assert_eq!(store.load_job(job_id).expect("load job"), job);

    let state = state(job_id, "REQUESTED");
    store.save_state(job_id, &state).expect("save state");
    assert_eq!(store.load_state(job_id).expect("load state"), state);

    fs::remove_dir_all(project).ok();
}

#[test]
fn terminal_state_blocks_resume_but_preserves_state() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store);

    let done = state("J-0001", "DONE");
    store
        .save_state("J-0001", &done)
        .expect("save terminal state");

    assert!(matches!(
        store.ensure_resume_allowed("J-0001"),
        Err(StateStoreError::TerminalStateBlocked { .. })
    ));
    assert_eq!(store.load_state("J-0001").expect("load terminal"), done);

    fs::remove_dir_all(project).ok();
}

#[test]
fn list_jobs_includes_corrupt_jobs() {
    let project = temp_project();
    let store = open_store(&project);
    let corrupt_dir = project.join(".ai-runs/J-0001");
    fs::create_dir_all(&corrupt_dir).expect("create corrupt job");
    fs::write(corrupt_dir.join("job.json"), "{ invalid json").expect("write corrupt job");

    let jobs = store.list_jobs().expect("list jobs");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].job_id, "J-0001");
    assert!(jobs[0].corrupt);

    fs::remove_dir_all(project).ok();
}
