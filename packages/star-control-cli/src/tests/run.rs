mod errors;
mod fake;
mod helpers;
mod local_process;

#[test]
fn run_status_and_report_json_work_for_fake_project() {
    fake::run_status_and_report_json_work_for_fake_project();
}

#[test]
fn run_dry_run_writes_route_without_provider_output() {
    fake::run_dry_run_writes_route_without_provider_output();
}

#[test]
fn run_with_local_process_provider_instance_executes_process() {
    local_process::run_with_local_process_provider_instance_executes_process();
}

#[test]
fn missing_job_returns_schema_valid_error() {
    errors::missing_job_returns_schema_valid_error();
}

#[test]
fn non_default_provider_requires_provider_instance_path() {
    errors::non_default_provider_requires_provider_instance_path();
}

#[test]
fn provider_instance_path_requires_explicit_provider() {
    errors::provider_instance_path_requires_explicit_provider();
}
