use crate::test_support::*;
use crate::{run_cli, CliConfig};
use serde_json::{json, Value};
use std::fs;

#[test]
fn recover_list_reports_inspection_without_mutation() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_recovery_inspection_job(&project);
    let tmp_path = project.join(".ai-runs/J-0001/tmp/run-state.json.tmp-test");
    let state_path = project.join(".ai-runs/J-0001/run-state.json");
    let events_path = project.join(".ai-runs/J-0001/events.jsonl");
    let before_state = fs::read_to_string(&state_path).expect("state before");
    let before_events = fs::read_to_string(&events_path).expect("events before");

    let recover = run_cli(
        [
            "recover",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--list",
            "--json",
        ],
        &config,
    );

    assert_eq!(recover.exit_code, 0, "{}", recover.stderr);
    let recover_json: Value = serde_json::from_str(&recover.stdout).expect("recover json");
    assert_eq!(recover_json["command"], "recover");
    assert_eq!(recover_json["status"], "success");
    assert_eq!(recover_json["data"]["mode"], "inspect_only");
    assert_eq!(recover_json["data"]["recovery_actions_enabled"], false);
    assert_eq!(recover_json["data"]["recovery"]["status"], "needs_recovery");
    assert_eq!(
        recover_json["data"]["recovery"]["destructive_actions_performed"],
        false
    );
    assert_eq!(
        recover_json["data"]["recovery"]["issues"][0]["kind"],
        "partial_tmp_file"
    );
    assert_eq!(
        recover_json["data"]["recovery"]["issues"][0]["artifact_path"],
        "tmp/run-state.json.tmp-test"
    );
    assert!(recover_json["artifacts"]
        .as_array()
        .expect("artifacts")
        .contains(&json!(".ai-runs/J-0001/tmp/run-state.json.tmp-test")));
    assert_eq!(
        fs::read_to_string(&state_path).expect("state after"),
        before_state
    );
    assert_eq!(
        fs::read_to_string(&events_path).expect("events after"),
        before_events
    );
    assert!(tmp_path.is_file());
    assert!(!project.join(".ai-runs/J-0001/recovery").exists());

    fs::remove_dir_all(project).ok();
}

#[test]
fn recover_requires_list_and_rejects_non_recovery_options() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_recovery_inspection_job(&project);

    let missing_mode = run_cli(
        [
            "recover",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--json",
        ],
        &config,
    );
    assert_eq!(missing_mode.exit_code, 2);
    let missing_mode_json: Value =
        serde_json::from_str(&missing_mode.stdout).expect("missing mode json");
    assert_eq!(missing_mode_json["error"]["code"], "InvalidInput");

    let invalid_combo = run_cli(
        [
            "recover",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--list",
            "--stage",
            "implement",
            "--json",
        ],
        &config,
    );
    assert_eq!(invalid_combo.exit_code, 2);
    let invalid_combo_json: Value =
        serde_json::from_str(&invalid_combo.stdout).expect("invalid combo json");
    assert_eq!(invalid_combo_json["error"]["code"], "InvalidInput");

    fs::remove_dir_all(project).ok();
}
