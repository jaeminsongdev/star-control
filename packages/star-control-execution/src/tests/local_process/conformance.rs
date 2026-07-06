use crate::test_support::{
    local_process_forbidden_evidence_args, local_process_sleep_args,
    run_local_process_conformance_case, LocalProcessConformanceCase,
};
use std::time::Duration;

#[test]
fn local_process_provider_conformance_fixture_covers_m5_runtime_contract() {
    let cases = vec![
        LocalProcessConformanceCase {
            id: "allowed_command_success",
            args: vec!["--help".to_string()],
            env_name: None,
            timeout_seconds: 10,
            cancel_after: None,
            expected_status: "success",
            expected_state: "IMPLEMENTED",
            expected_error_kind: None,
            expected_error_action: None,
        },
        LocalProcessConformanceCase {
            id: "timeout_failed_state",
            args: local_process_sleep_args(),
            env_name: Some("STAR_CONTROL_EXECUTION_SLEEP_HELPER"),
            timeout_seconds: 1,
            cancel_after: None,
            expected_status: "timeout",
            expected_state: "FAILED",
            expected_error_kind: Some("local_process_timeout"),
            expected_error_action: None,
        },
        LocalProcessConformanceCase {
            id: "cancelled_state",
            args: local_process_sleep_args(),
            env_name: Some("STAR_CONTROL_EXECUTION_SLEEP_HELPER"),
            timeout_seconds: 10,
            cancel_after: Some(Duration::from_millis(150)),
            expected_status: "cancelled",
            expected_state: "CANCELLED",
            expected_error_kind: Some("local_process_cancelled"),
            expected_error_action: None,
        },
        LocalProcessConformanceCase {
            id: "forbidden_action_blocked_state",
            args: local_process_forbidden_evidence_args(),
            env_name: Some("STAR_CONTROL_EXECUTION_FORBIDDEN_EVIDENCE_HELPER"),
            timeout_seconds: 10,
            cancel_after: None,
            expected_status: "blocked",
            expected_state: "BLOCKED",
            expected_error_kind: Some("local_process_forbidden_action"),
            expected_error_action: Some("dependency_install"),
        },
    ];

    for case in cases {
        run_local_process_conformance_case(case);
    }
}
