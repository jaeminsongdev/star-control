use super::helpers::{cleanup_dirs, open_daemon_queue, temp_dir};
use crate::constants::{DEFAULT_DAEMON_ID, SCHEMA_VERSION};

pub(super) fn opens_default_state_under_config_root_not_project_root() {
    let project = temp_dir("project");
    let config = temp_dir("config");
    let queue = open_daemon_queue(&config);

    assert_eq!(
        queue.state_path(),
        config.join("daemon/state.json").as_path()
    );
    assert!(queue.state_path().is_file());
    assert!(!project.join("daemon/state.json").exists());
    assert!(!project.join(".star-control/daemon/state.json").exists());

    let state = queue.load_state().expect("load daemon state");
    assert_eq!(state["schema_version"], SCHEMA_VERSION);
    assert_eq!(state["daemon_id"], DEFAULT_DAEMON_ID);
    assert_eq!(state["status"], "reserved");
    assert_eq!(state["queue"].as_array().expect("queue").len(), 0);

    cleanup_dirs(project, config);
}
