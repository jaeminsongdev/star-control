use super::helpers::{cleanup_project, config, json_output, path_arg};
use crate::run_cli;
use crate::test_support::{temp_project, write_local_process_instance};

pub(super) fn missing_job_returns_schema_valid_error() {
    let project = temp_project();
    let result = run_cli(
        [
            "status",
            "--project",
            path_arg(&project),
            "--job",
            "J-9999",
            "--json",
        ],
        &config(),
    );

    assert_eq!(result.exit_code, 3);
    let error_json = json_output(&result, "error json");
    assert_eq!(error_json["command"], "status");
    assert_eq!(error_json["error"]["code"], "StateReadFailed");
    cleanup_project(project);
}

pub(super) fn non_default_provider_requires_provider_instance_path() {
    let project = temp_project();
    let result = run_cli(
        [
            "run",
            "--project",
            path_arg(&project),
            "--request",
            "runtime code 구현",
            "--provider",
            "local-default",
            "--json",
        ],
        &config(),
    );

    assert_eq!(result.exit_code, 2);
    let error_json = json_output(&result, "error json");
    assert_eq!(error_json["error"]["code"], "InvalidInput");
    cleanup_project(project);
}

pub(super) fn provider_instance_path_requires_explicit_provider() {
    let project = temp_project();
    let provider_instance = write_local_process_instance(&project, vec!["--help".to_string()]);
    let result = run_cli(
        [
            "run",
            "--project",
            path_arg(&project),
            "--request",
            "runtime code 구현",
            "--provider-instance",
            provider_instance.to_str().expect("provider instance path"),
            "--json",
        ],
        &config(),
    );

    assert_eq!(result.exit_code, 2);
    let error_json = json_output(&result, "error json");
    assert_eq!(error_json["error"]["code"], "InvalidInput");
    cleanup_project(project);
}
