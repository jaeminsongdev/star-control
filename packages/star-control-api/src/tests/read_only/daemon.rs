use super::super::{open_daemon_queue, schema_root, temp_project};
use super::helpers::{assert_state_unchanged, assert_success, read_file_snapshot};
use crate::ApiReadOnlyService;
use std::fs;

#[test]
fn daemon_state_endpoint_reads_registered_queue_state() {
    let config = temp_project();
    let queue = open_daemon_queue(&config);
    let state_path = queue.state_path().to_path_buf();
    let before_state = read_file_snapshot(&state_path, "read daemon state before");
    let mut service = ApiReadOnlyService::new(schema_root());
    service.register_daemon_queue(queue);

    let response = service.handle_get("/daemon/state").expect("daemon state");
    assert_success(&response);
    assert_eq!(response["data"]["daemon_state"]["status"], "reserved");
    assert_eq!(
        response["data"]["daemon_state"]["queue"]
            .as_array()
            .expect("queue")
            .len(),
        0
    );

    assert_state_unchanged(&state_path, &before_state, "read daemon state after");

    fs::remove_dir_all(config).ok();
}
