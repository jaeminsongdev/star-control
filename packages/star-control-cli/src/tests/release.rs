use crate::test_support::*;
use crate::{run_cli, CliConfig};
use serde_json::Value;
use std::fs;

#[test]
fn report_release_readiness_reads_existing_artifact_without_mutation() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_release_readiness_job(&project, true);
    let readiness_path = project.join(".ai-runs/J-0001/release/release-readiness.json");
    let before_readiness = fs::read_to_string(&readiness_path).expect("read readiness before");

    let report = run_cli(
        [
            "report",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--release-readiness",
            "--json",
        ],
        &config,
    );

    assert_eq!(report.exit_code, 0, "{}", report.stderr);
    let report_json: Value = serde_json::from_str(&report.stdout).expect("report json");
    assert_eq!(report_json["command"], "report");
    assert_eq!(report_json["data"]["report_kind"], "release_readiness");
    assert_eq!(report_json["data"]["release_actions_enabled"], false);
    assert_eq!(
        report_json["data"]["release_readiness_path"],
        ".ai-runs/J-0001/release/release-readiness.json"
    );
    assert_eq!(report_json["data"]["readiness"]["status"], "reserved");
    assert_eq!(
        report_json["artifacts"][0],
        ".ai-runs/J-0001/release/release-readiness.json"
    );
    let after_readiness = fs::read_to_string(&readiness_path).expect("read readiness after");
    assert_eq!(after_readiness, before_readiness);
    assert!(!project
        .join(".ai-runs/J-0001/release/release-action.json")
        .exists());

    fs::remove_dir_all(project).ok();
}

#[test]
fn report_release_readiness_requires_existing_artifact_and_rejects_stage() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_release_readiness_job(&project, false);

    let missing = run_cli(
        [
            "report",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--release-readiness",
            "--json",
        ],
        &config,
    );
    assert_eq!(missing.exit_code, 3);
    let missing_json: Value = serde_json::from_str(&missing.stdout).expect("missing json");
    assert_eq!(missing_json["error"]["code"], "MissingArtifact");
    assert_eq!(
        missing_json["error"]["artifact_paths"][0],
        ".ai-runs/J-0001/release/release-readiness.json"
    );

    let invalid = run_cli(
        [
            "report",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--release-readiness",
            "--stage",
            "implement",
            "--json",
        ],
        &config,
    );
    assert_eq!(invalid.exit_code, 2);
    let invalid_json: Value = serde_json::from_str(&invalid.stdout).expect("invalid json");
    assert_eq!(invalid_json["error"]["code"], "InvalidInput");

    fs::remove_dir_all(project).ok();
}
