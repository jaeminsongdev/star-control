use crate::test_support::{repo_root, temp_project, write_sentinel_input_job};
use crate::{run_cli, CliConfig};
use serde_json::Value;
use std::fs;

pub(super) fn sentinel_commands_wrap_star_sentinel_artifacts() {
    let config = CliConfig::new(repo_root());

    let selfcheck = run_cli(["sentinel", "selfcheck", "--json"], &config);
    assert_eq!(selfcheck.exit_code, 0, "{}", selfcheck.stderr);
    let selfcheck_json: Value = serde_json::from_str(&selfcheck.stdout).expect("selfcheck json");
    assert_eq!(selfcheck_json["command"], "sentinel");
    assert_eq!(selfcheck_json["data"]["subcommand"], "selfcheck");
    assert_eq!(selfcheck_json["data"]["ok"], true);
    assert_eq!(selfcheck_json["data"]["actions_enabled"], false);

    let check_project = temp_project();
    write_sentinel_input_job(&check_project, "p0-auto-pass", vec!["src/**"], "src/lib.rs");
    let check = run_cli(
        [
            "sentinel",
            "check",
            "--project",
            check_project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--json",
        ],
        &config,
    );
    assert_eq!(check.exit_code, 0, "{}", check.stderr);
    let check_json: Value = serde_json::from_str(&check.stdout).expect("check json");
    assert_eq!(check_json["data"]["subcommand"], "check");
    assert_eq!(check_json["data"]["decision"], "AUTO_PASS");
    assert_eq!(check_json["data"]["actions_enabled"], false);
    assert!(check_project
        .join(".ai-runs/J-0001/tool-output/star-sentinel/diagnostics.json")
        .is_file());
    assert!(!check_project
        .join(".ai-runs/J-0001/tool-output/star-sentinel/approval.json")
        .exists());

    let gate_project = temp_project();
    write_sentinel_input_job(&gate_project, "p0-human-review", vec!["**"], "Cargo.toml");
    let gate = run_cli(
        [
            "sentinel",
            "gate",
            "--project",
            gate_project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--json",
        ],
        &config,
    );
    assert_eq!(gate.exit_code, 0, "{}", gate.stderr);
    let gate_json: Value = serde_json::from_str(&gate.stdout).expect("gate json");
    assert_eq!(gate_json["status"], "waiting_approval");
    assert_eq!(gate_json["data"]["decision"], "HUMAN_REVIEW");
    assert!(gate_project
        .join(".ai-runs/J-0001/tool-output/star-sentinel/approval.json")
        .is_file());

    let review_project = temp_project();
    write_sentinel_input_job(
        &review_project,
        "p0-block",
        vec!["src/allowed/**"],
        "src/other.rs",
    );
    let review = run_cli(
        [
            "sentinel",
            "review-pack",
            "--project",
            review_project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--json",
        ],
        &config,
    );
    assert_eq!(review.exit_code, 0, "{}", review.stderr);
    let review_json: Value = serde_json::from_str(&review.stdout).expect("review json");
    assert_eq!(review_json["status"], "blocked");
    assert_eq!(review_json["data"]["decision"], "BLOCK");
    assert!(review_project
        .join(".ai-runs/J-0001/review-packs/review_pack.md")
        .is_file());

    fs::remove_dir_all(check_project).ok();
    fs::remove_dir_all(gate_project).ok();
    fs::remove_dir_all(review_project).ok();
}
