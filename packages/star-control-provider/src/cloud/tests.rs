use super::test_support::is_child_helper;
use std::time::Duration;

mod api;
mod cli;
mod preflight;

#[test]
fn cloud_cli_success_helper() {
    if is_child_helper("cloud::tests::cloud_cli_success_helper")
        && std::env::var("STAR_CONTROL_CLOUD_CLI_SUCCESS_HELPER").is_ok()
    {
        println!("cloud cli success");
    }
}

#[test]
fn cloud_cli_sleep_helper() {
    if is_child_helper("cloud::tests::cloud_cli_sleep_helper")
        && std::env::var("STAR_CONTROL_CLOUD_CLI_SLEEP_HELPER").is_ok()
    {
        std::thread::sleep(Duration::from_secs(5));
    }
}
