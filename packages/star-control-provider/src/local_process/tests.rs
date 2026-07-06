mod cancellation;
mod execution;
mod forbidden_action;
mod policy;
mod support;

use super::constants::{FORBIDDEN_ACTION_EVIDENCE_PREFIX, STDOUT_FILE};
use super::*;
use crate::{ExecutionRequest, ProviderAdapter, ProviderAdapterError, ProviderRunContext};
use star_control_state::StateStore;
use std::thread;
use std::time::Duration;
use support::*;

#[test]
fn local_process_sleep_helper() {
    let is_child_helper = std::env::args().collect::<Vec<_>>().windows(2).any(|args| {
        args[0] == "--exact" && args[1] == "local_process::tests::local_process_sleep_helper"
    });
    if is_child_helper && std::env::var("STAR_CONTROL_LOCAL_PROCESS_SLEEP_HELPER").is_ok() {
        thread::sleep(Duration::from_secs(5));
    }
}

#[test]
fn local_process_forbidden_evidence_helper() {
    let is_child_helper = std::env::args().collect::<Vec<_>>().windows(2).any(|args| {
        args[0] == "--exact"
            && args[1] == "local_process::tests::local_process_forbidden_evidence_helper"
    });
    if is_child_helper
        && std::env::var("STAR_CONTROL_LOCAL_PROCESS_FORBIDDEN_EVIDENCE_HELPER").is_ok()
    {
        println!(
            "{}dependency_install attempted by local provider",
            FORBIDDEN_ACTION_EVIDENCE_PREFIX
        );
    }
}
