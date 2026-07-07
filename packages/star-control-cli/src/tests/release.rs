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

#[test]
fn release_action_dry_run_prepares_automation_without_external_effects() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_release_readiness_job(&project, true);
    let readiness_path = project.join(".ai-runs/J-0001/release/release-readiness.json");
    let before_readiness = fs::read_to_string(&readiness_path).expect("read readiness before");

    let release = run_cli(
        [
            "release",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--action",
            "prepare",
            "--dry-run",
            "--json",
        ],
        &config,
    );

    assert_eq!(release.exit_code, 0, "{}", release.stderr);
    let release_json: Value = serde_json::from_str(&release.stdout).expect("release json");
    assert_eq!(release_json["command"], "release");
    assert_eq!(release_json["status"], "success");
    assert_eq!(release_json["data"]["mode"], "dry_run");
    assert_eq!(release_json["data"]["release_actions_enabled"], true);
    assert_eq!(release_json["data"]["action_execution_enabled"], false);
    assert_eq!(
        release_json["data"]["release_automation_plan"]["action"],
        "prepare"
    );
    assert_eq!(
        release_json["data"]["release_automation_plan"]["steps"][0]["operation"],
        "signing_policy_execution"
    );
    assert_eq!(release_json["data"]["external_actions_performed"], false);
    assert_eq!(release_json["data"]["release_actions_performed"], false);
    assert_eq!(
        release_json["data"]["external_execution_policy"]["live_execution_enabled"],
        false
    );
    assert_eq!(
        release_json["data"]["external_execution_policy"]["blocked_operations"][0],
        "package_registry_publish"
    );
    assert_eq!(
        fs::read_to_string(&readiness_path).expect("read readiness after"),
        before_readiness
    );
    assert!(!project
        .join(".ai-runs/J-0001/release/release-action.json")
        .exists());

    fs::remove_dir_all(project).ok();
}

#[test]
fn release_action_requires_approval_before_execution() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_release_readiness_job(&project, true);

    let release = run_cli(
        [
            "release",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--action",
            "deploy",
            "--json",
        ],
        &config,
    );

    assert_eq!(release.exit_code, 0, "{}", release.stderr);
    let release_json: Value = serde_json::from_str(&release.stdout).expect("release json");
    assert_eq!(release_json["status"], "blocked");
    assert_eq!(release_json["data"]["mode"], "approval_required");
    assert_eq!(release_json["data"]["approval_required"], true);
    assert_eq!(
        release_json["data"]["approval_gate"]["approval_token"],
        "approve:deploy:J-0001"
    );
    assert_eq!(
        release_json["data"]["approval_gate"]["approval_provided"],
        false
    );
    assert_eq!(release_json["data"]["external_actions_performed"], false);
    assert_eq!(release_json["data"]["release_actions_performed"], false);
    assert_eq!(
        release_json["data"]["external_execution_policy"]["external_actions_allowed"],
        false
    );
    assert!(!project
        .join(".ai-runs/J-0001/release/release-action.json")
        .exists());

    fs::remove_dir_all(project).ok();
}

#[test]
fn release_action_rejects_wrong_approval_without_execution() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_release_readiness_job(&project, true);

    let release = run_cli(
        [
            "release",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--action",
            "deploy",
            "--approve-release-action",
            "approve:deploy:J-9999",
            "--json",
        ],
        &config,
    );

    assert_eq!(release.exit_code, 0, "{}", release.stderr);
    let release_json: Value = serde_json::from_str(&release.stdout).expect("release json");
    assert_eq!(release_json["status"], "blocked");
    assert_eq!(
        release_json["data"]["approval_gate"]["approval_accepted"],
        false
    );
    assert_eq!(release_json["data"]["action_execution_enabled"], false);
    assert_eq!(release_json["data"]["external_actions_performed"], false);
    assert_eq!(release_json["data"]["release_actions_performed"], false);
    assert!(!project
        .join(".ai-runs/J-0001/release/deploy-automation-result.json")
        .exists());

    fs::remove_dir_all(project).ok();
}

#[test]
fn release_action_approved_deploy_records_local_automation_without_external_effects() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_release_readiness_job(&project, true);
    let readiness_path = project.join(".ai-runs/J-0001/release/release-readiness.json");
    let before_readiness = fs::read_to_string(&readiness_path).expect("read readiness before");

    let release = run_cli(
        [
            "release",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--action",
            "deploy",
            "--approve-release-action",
            "approve:deploy:J-0001",
            "--json",
        ],
        &config,
    );

    assert_eq!(release.exit_code, 0, "{}", release.stderr);
    let release_json: Value = serde_json::from_str(&release.stdout).expect("release json");
    assert_eq!(release_json["status"], "success");
    assert_eq!(release_json["data"]["mode"], "approved_execution");
    assert_eq!(release_json["data"]["action_execution_enabled"], true);
    assert_eq!(
        release_json["data"]["approval_gate"]["execution_after_approval"],
        "performed"
    );
    assert_eq!(release_json["data"]["external_actions_performed"], false);
    assert_eq!(release_json["data"]["release_actions_performed"], true);
    assert_eq!(
        release_json["data"]["external_execution_policy"]["status"],
        "reserved"
    );
    assert_eq!(
        release_json["data"]["release_execution"]["external_execution_policy"]
            ["blocked_operations"][0],
        "deploy_flow"
    );
    assert_eq!(
        release_json["data"]["release_execution"]["executed_steps"][0]["execution_kind"],
        "local_plan_record_only"
    );
    assert_eq!(
        release_json["data"]["release_execution"]["executed_steps"][0]["external_policy_decision"],
        "record_only_reserved"
    );
    assert!(project
        .join(".ai-runs/J-0001/release/deploy-automation-result.json")
        .is_file());
    assert_eq!(
        fs::read_to_string(&readiness_path).expect("read readiness after"),
        before_readiness
    );

    fs::remove_dir_all(project).ok();
}

#[test]
fn release_action_rollback_checklist_executes_without_approval() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_release_readiness_job(&project, true);

    let release = run_cli(
        [
            "release",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--action",
            "rollback-checklist",
            "--json",
        ],
        &config,
    );

    assert_eq!(release.exit_code, 0, "{}", release.stderr);
    let release_json: Value = serde_json::from_str(&release.stdout).expect("release json");
    assert_eq!(release_json["status"], "success");
    assert_eq!(release_json["data"]["mode"], "approved_execution");
    assert_eq!(release_json["data"]["approval_required"], false);
    assert_eq!(
        release_json["data"]["approval_gate"]["execution_after_approval"],
        "not_required"
    );
    assert_eq!(release_json["data"]["external_actions_performed"], false);
    assert_eq!(release_json["data"]["release_actions_performed"], true);
    assert!(project
        .join(".ai-runs/J-0001/release/rollback-checklist-automation-result.json")
        .is_file());

    fs::remove_dir_all(project).ok();
}
