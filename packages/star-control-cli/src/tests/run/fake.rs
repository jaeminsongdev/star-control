use super::helpers::{assert_success, cleanup_project, config, json_output, path_arg};
use crate::run_cli;
use crate::test_support::{repo_root, temp_project};
use serde_json::Value;
use star_control_state::StateStore;
use std::fs;

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

pub(super) fn report_json_redacts_sensitive_values_and_writes_redaction_report() {
    let project = temp_project();
    let config = config();
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
        &config,
    );
    assert_success(&run);

    let store = StateStore::open(&project, repo_root().join("specs/schemas")).expect("open store");
    let mut report = store
        .load_report("J-0001", "implement-report")
        .expect("load report");
    report["risks"] = serde_json::json!(["Authorization: Bearer sk-test-secret"]);
    store
        .save_report("J-0001", "implement-report", &report)
        .expect("save report with sensitive value");

    let first = run_cli(
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
        &config,
    );
    assert_success(&first);
    let first_text = first.stdout.clone();
    assert!(!first_text.contains("sk-test-secret"));
    assert!(first_text.contains("[REDACTED]"));
    let first_json = json_output(&first, "redacted report json");
    assert_eq!(first_json["data"]["report"]["risks"][0], "[REDACTED]");
    assert_eq!(
        first_json["artifacts"][1],
        ".ai-runs/J-0001/audit/redaction-report-implement.json"
    );
    let report_path = project.join(".ai-runs/J-0001/audit/redaction-report-implement.json");
    assert!(report_path.is_file());
    let redaction_report: Value =
        serde_json::from_str(&fs::read_to_string(&report_path).expect("read redaction report"))
            .expect("parse redaction report");
    assert_eq!(
        redaction_report["artifact_path"],
        "reports/implement-report.json"
    );
    assert_eq!(redaction_report["redacted"], true);
    assert_eq!(redaction_report["findings"][0]["path"], "$.risks[0]");

    let second = run_cli(
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
        &config,
    );
    assert_success(&second);
    let second_text = second.stdout.clone();
    assert!(!second_text.contains("sk-test-secret"));
    assert!(second_text.contains("[REDACTED]"));

    cleanup_project(project);
}
