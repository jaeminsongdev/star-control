use super::helpers::{
    approval_response, cleanup_dirs, create_job, open_daemon_queue, open_project_store, temp_dir,
};
use crate::DaemonError;

pub(super) fn waiting_approval_requires_approved_response() {
    let project = temp_dir("project");
    let config = temp_dir("config");
    let store = open_project_store(&project);
    create_job(&store, "WAITING_APPROVAL", "validate");
    let queue = open_daemon_queue(&config);

    let missing = queue
        .enqueue_project_job(&store, "J-0001")
        .expect_err("missing approval response rejected");
    assert!(matches!(missing, DaemonError::ApprovalRequired { .. }));

    store
        .write_approval_json(
            "J-0001",
            "approval-response.json",
            &approval_response("J-0001", "approved"),
        )
        .expect("write approval response");
    let entry = queue
        .enqueue_project_job(&store, "J-0001")
        .expect("enqueue approved job");
    assert_eq!(entry["run_state"], "WAITING_APPROVAL");
    assert_eq!(queue.queue_len().expect("queue len"), 1);

    cleanup_dirs(project, config);
}

pub(super) fn non_approved_response_is_not_queued() {
    let project = temp_dir("project");
    let config = temp_dir("config");
    let store = open_project_store(&project);
    create_job(&store, "WAITING_APPROVAL", "validate");
    store
        .write_approval_json(
            "J-0001",
            "approval-response.json",
            &approval_response("J-0001", "needs_changes"),
        )
        .expect("write approval response");
    let queue = open_daemon_queue(&config);

    let error = queue
        .enqueue_project_job(&store, "J-0001")
        .expect_err("needs_changes rejected");
    assert!(matches!(
        error,
        DaemonError::ApprovalResponseNotApproved { .. }
    ));
    assert_eq!(queue.queue_len().expect("queue len"), 0);

    cleanup_dirs(project, config);
}
