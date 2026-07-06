mod cloud;
mod fake;
mod local_process;

#[test]
fn execution_sleep_helper() {
    let is_child_helper = std::env::args()
        .collect::<Vec<_>>()
        .windows(2)
        .any(|args| args[0] == "--exact" && args[1] == "tests::execution_sleep_helper");
    if is_child_helper && std::env::var("STAR_CONTROL_EXECUTION_SLEEP_HELPER").is_ok() {
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}

#[test]
fn execution_forbidden_evidence_helper() {
    let is_child_helper = std::env::args().collect::<Vec<_>>().windows(2).any(|args| {
        args[0] == "--exact" && args[1] == "tests::execution_forbidden_evidence_helper"
    });
    if is_child_helper && std::env::var("STAR_CONTROL_EXECUTION_FORBIDDEN_EVIDENCE_HELPER").is_ok()
    {
        println!("STAR_CONTROL_FORBIDDEN_ACTION_EVIDENCE:dependency_install from execution helper");
    }
}

#[test]
fn execution_cloud_cli_success_helper() {
    let is_child_helper = std::env::args()
        .collect::<Vec<_>>()
        .windows(2)
        .any(|args| args[0] == "--exact" && args[1] == "tests::execution_cloud_cli_success_helper");
    if is_child_helper && std::env::var("STAR_CONTROL_EXECUTION_CLOUD_CLI_HELPER").is_ok() {
        println!("cloud cli execution helper completed");
    }
}
