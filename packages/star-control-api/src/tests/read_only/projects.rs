use super::super::{api_with_store, create_job, open_store, temp_project};
use super::helpers::{
    assert_api_response_not_written, assert_state_unchanged, assert_success, read_file_snapshot,
};
use std::fs;

#[test]
fn projects_jobs_and_job_detail_are_schema_valid_and_read_only() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store, "IMPLEMENTED", "implement");
    let state_path = project.join(".ai-runs/J-0001/run-state.json");
    let before_state = read_file_snapshot(&state_path, "read state before");
    let service = api_with_store(store);

    let projects = service.handle_get("/projects").expect("projects");
    assert_success(&projects);
    assert_eq!(projects["data"]["projects"][0]["project_id"], "local");

    let jobs = service.handle_get("/projects/local/jobs").expect("jobs");
    assert_success(&jobs);
    assert_eq!(jobs["data"]["jobs"][0]["job_id"], "J-0001");
    assert_eq!(jobs["data"]["jobs"][0]["run_dir"], ".ai-runs/J-0001");

    let detail = service
        .handle_get("/projects/local/jobs/J-0001")
        .expect("job detail");
    assert_success(&detail);
    assert_eq!(detail["data"]["state"]["state"], "IMPLEMENTED");
    assert_eq!(detail["data"]["run_dir"], ".ai-runs/J-0001");

    assert_state_unchanged(&state_path, &before_state, "read state after");
    assert_api_response_not_written(&project);

    fs::remove_dir_all(project).ok();
}
