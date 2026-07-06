use super::helpers::{cleanup_dirs, create_job, open_daemon_queue, open_project_store, temp_dir};
use crate::constants::{DEFAULT_PRIORITY, QUEUED_STATE};
use crate::DaemonError;

pub(super) fn enqueue_nonterminal_job_records_project_reference_without_copying_artifacts() {
    let project = temp_dir("project");
    let config = temp_dir("config");
    let store = open_project_store(&project);
    create_job(&store, "ROUTED", "implement");
    let queue = open_daemon_queue(&config);

    let entry = queue
        .enqueue_project_job(&store, "J-0001")
        .expect("enqueue job");
    assert_eq!(entry["job_id"], "J-0001");
    assert_eq!(entry["priority"], DEFAULT_PRIORITY);
    assert_eq!(entry["state"], QUEUED_STATE);
    assert_eq!(
        entry["project_root"],
        store.project_root().display().to_string()
    );
    assert_eq!(entry["current_stage"], "implement");
    assert_eq!(entry["run_state"], "ROUTED");
    assert_eq!(entry["run_dir"], ".ai-runs/J-0001");

    let daemon_state = queue.load_state().expect("load daemon state");
    assert_eq!(daemon_state["queue"].as_array().expect("queue").len(), 1);
    assert!(project.join(".ai-runs/J-0001/job.json").is_file());
    assert!(project.join(".ai-runs/J-0001/run-state.json").is_file());
    assert!(!queue.daemon_dir().join(".ai-runs").exists());

    cleanup_dirs(project, config);
}

pub(super) fn terminal_job_is_not_queued() {
    let project = temp_dir("project");
    let config = temp_dir("config");
    let store = open_project_store(&project);
    create_job(&store, "DONE", "report");
    let queue = open_daemon_queue(&config);

    let error = queue
        .enqueue_project_job(&store, "J-0001")
        .expect_err("terminal job rejected");
    assert!(matches!(error, DaemonError::TerminalJobRejected { .. }));
    assert_eq!(queue.queue_len().expect("queue len"), 0);

    cleanup_dirs(project, config);
}

pub(super) fn duplicate_queue_entry_is_rejected() {
    let project = temp_dir("project");
    let config = temp_dir("config");
    let store = open_project_store(&project);
    create_job(&store, "VALIDATED", "report");
    let queue = open_daemon_queue(&config);

    queue
        .enqueue_project_job(&store, "J-0001")
        .expect("first enqueue");
    let error = queue
        .enqueue_project_job(&store, "J-0001")
        .expect_err("duplicate rejected");
    assert!(matches!(error, DaemonError::DuplicateQueuedJob { .. }));
    assert_eq!(queue.queue_len().expect("queue len"), 1);

    cleanup_dirs(project, config);
}
