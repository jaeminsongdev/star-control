use super::super::{api_with_store, create_job, open_store, schema_root, temp_project};
use super::helpers::assert_failed_code;
use crate::{ApiMethod, ApiReadOnlyService, ApiRequest};
use std::fs;

#[test]
fn missing_project_job_and_report_are_structured_errors() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store, "IMPLEMENTED", "implement");
    let service = api_with_store(store);

    let missing_project = service
        .handle_get("/projects/missing/jobs")
        .expect("missing project response");
    assert_failed_code(&missing_project, "project_not_found");

    let missing_job = service
        .handle_get("/projects/local/jobs/J-9999")
        .expect("missing job response");
    assert_failed_code(&missing_job, "job_read_failed");

    let missing_report = service
        .handle_get("/projects/local/jobs/J-0001/report?stage=implement")
        .expect("missing report response");
    assert_failed_code(&missing_report, "report_read_failed");

    let missing_readiness = service
        .handle_get("/projects/local/jobs/J-0001/release-readiness")
        .expect("missing release readiness response");
    assert_failed_code(&missing_readiness, "release_readiness_not_found");
    assert_eq!(
        missing_readiness["error"]["details"]["artifact_path"],
        ".ai-runs/J-0001/release/release-readiness.json"
    );

    let missing_job_readiness = service
        .handle_get("/projects/local/jobs/J-9999/release-readiness")
        .expect("missing job release readiness response");
    assert_failed_code(&missing_job_readiness, "job_read_failed");

    fs::remove_dir_all(project).ok();
}

#[test]
fn mutation_methods_and_unknown_paths_are_not_implemented() {
    let service = ApiReadOnlyService::new(schema_root());

    let missing_daemon = service
        .handle_get("/daemon/state")
        .expect("missing daemon response");
    assert_failed_code(&missing_daemon, "daemon_not_registered");

    let mutation = service
        .handle(ApiRequest::new(ApiMethod::Post, "/projects/local/jobs"))
        .expect("mutation response");
    assert_failed_code(&mutation, "method_not_allowed");

    let unknown = service.handle_get("/projects/local/jobs/J-0001/approve");
    let unknown = unknown.expect("unknown response");
    assert_failed_code(&unknown, "endpoint_not_found");
}
