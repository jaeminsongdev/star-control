use super::helpers::{assert_success, cleanup_project, config, json_output, path_arg};
use crate::run_cli;
use crate::test_support::temp_project;

pub(super) fn run_status_and_report_json_work_for_fake_project() {
    let project = temp_project();
    let run = run_cli(
        [
            "run",
            "--project",
            path_arg(&project),
            "--request",
            "runtime code 구현",
            "--provider",
            "fake-default",
            "--json",
        ],
        &config(),
    );
    assert_success(&run);
    let run_json = json_output(&run, "run json");
    assert_eq!(run_json["command"], "run");
    assert_eq!(run_json["status"], "success");
    assert_eq!(run_json["data"]["job_id"], "J-0001");
    assert_eq!(run_json["data"]["executed_stage"], "implement");
    assert!(project
        .join(".ai-runs/J-0001/provider-output/fake-default/response.json")
        .is_file());

    let status = run_cli(
        [
            "status",
            "--project",
            path_arg(&project),
            "--job",
            "J-0001",
            "--json",
        ],
        &config(),
    );
    assert_success(&status);
    let status_json = json_output(&status, "status json");
    assert_eq!(status_json["command"], "status");
    assert_eq!(status_json["data"]["state"], "IMPLEMENTED");

    let report = run_cli(
        [
            "report",
            "--project",
            path_arg(&project),
            "--job",
            "J-0001",
            "--stage",
            "implement",
            "--json",
        ],
        &config(),
    );
    assert_success(&report);
    let report_json = json_output(&report, "report json");
    assert_eq!(report_json["command"], "report");
    assert_eq!(report_json["data"]["report"]["status"], "DONE");

    cleanup_project(project);
}

pub(super) fn run_dry_run_writes_route_without_provider_output() {
    let project = temp_project();
    let run = run_cli(
        [
            "run",
            "--project",
            path_arg(&project),
            "--request",
            "README 문서 수정",
            "--dry-run",
            "--json",
        ],
        &config(),
    );

    assert_success(&run);
    let run_json = json_output(&run, "run json");
    assert_eq!(run_json["data"]["dry_run"], true);
    assert!(project.join(".ai-runs/J-0001/route.json").is_file());
    assert!(!project
        .join(".ai-runs/J-0001/provider-output/fake-default/response.json")
        .exists());
    cleanup_project(project);
}
