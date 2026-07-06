use super::super::{api_with_store, create_job, open_store, schema_root, temp_project};
use super::helpers::{
    assert_api_response_not_written, assert_state_unchanged, assert_success, read_file_snapshot,
};
use star_control_release::ReleaseReadinessWriter;
use std::fs;

#[test]
fn release_readiness_endpoint_reads_schema_valid_artifact_without_mutation() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store, "DONE", "report");
    let writer = ReleaseReadinessWriter::new(schema_root());
    let readiness = writer.reserved("release-0001", "star-control", "0.0.0-dev");
    writer
        .write(&store, "J-0001", &readiness)
        .expect("write release readiness");
    let state_path = project.join(".ai-runs/J-0001/run-state.json");
    let before_state = read_file_snapshot(&state_path, "read state before");
    let service = api_with_store(store);

    let response = service
        .handle_get("/projects/local/jobs/J-0001/release-readiness")
        .expect("release readiness response");

    assert_success(&response);
    assert_eq!(response["data"]["project_id"], "local");
    assert_eq!(response["data"]["job_id"], "J-0001");
    assert_eq!(
        response["data"]["readiness_path"],
        ".ai-runs/J-0001/release/release-readiness.json"
    );
    assert_eq!(response["data"]["readiness"]["status"], "reserved");
    assert_eq!(
        response["data"]["readiness"]["blockers"][0],
        "release automation is not implemented yet"
    );
    assert_state_unchanged(&state_path, &before_state, "read state after");
    assert_api_response_not_written(&project);

    fs::remove_dir_all(project).ok();
}
