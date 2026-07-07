use crate::test_support::*;
use crate::{run_cli, CliConfig};
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;

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

#[test]
fn recover_action_dry_run_previews_without_mutation() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_recovery_inspection_job(&project);
    let tmp_path = project.join(".ai-runs/J-0001/tmp/run-state.json.tmp-test");

    let recover = run_cli(
        [
            "recover",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--action",
            "tmp-cleanup",
            "--dry-run",
            "--json",
        ],
        &config,
    );

    assert_eq!(recover.exit_code, 0, "{}", recover.stderr);
    let recover_json: Value = serde_json::from_str(&recover.stdout).expect("recover json");
    assert_eq!(recover_json["command"], "recover");
    assert_eq!(recover_json["status"], "success");
    assert_eq!(recover_json["data"]["mode"], "dry_run");
    assert_eq!(recover_json["data"]["recovery_actions_enabled"], true);
    assert_eq!(recover_json["data"]["action_execution_enabled"], false);
    assert_eq!(
        recover_json["data"]["recovery_action"]["planned_changes"][0]["operation"],
        "delete_file"
    );
    assert_eq!(
        recover_json["data"]["recovery_action"]["planned_changes"][0]["artifact_path"],
        "tmp/run-state.json.tmp-test"
    );
    assert_eq!(recover_json["data"]["destructive_actions_performed"], false);
    assert!(tmp_path.is_file());

    fs::remove_dir_all(project).ok();
}

#[test]
fn recover_action_requires_approval_before_execution() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_recovery_inspection_job(&project);
    let tmp_path = project.join(".ai-runs/J-0001/tmp/run-state.json.tmp-test");

    let recover = run_cli(
        [
            "recover",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--action",
            "tmp-cleanup",
            "--json",
        ],
        &config,
    );

    assert_eq!(recover.exit_code, 0, "{}", recover.stderr);
    let recover_json: Value = serde_json::from_str(&recover.stdout).expect("recover json");
    assert_eq!(recover_json["status"], "blocked");
    assert_eq!(recover_json["data"]["mode"], "approval_required");
    assert_eq!(recover_json["data"]["approval_required"], true);
    assert_eq!(
        recover_json["data"]["approval_gate"]["approval_token"],
        "approve:tmp-cleanup:J-0001"
    );
    assert_eq!(
        recover_json["data"]["approval_gate"]["approval_provided"],
        false
    );
    assert_eq!(recover_json["data"]["destructive_actions_performed"], false);
    assert!(tmp_path.is_file());

    fs::remove_dir_all(project).ok();
}

#[test]
fn recover_action_rejects_wrong_approval_without_mutation() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_recovery_inspection_job(&project);
    let tmp_path = project.join(".ai-runs/J-0001/tmp/run-state.json.tmp-test");

    let recover = run_cli(
        [
            "recover",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--action",
            "tmp-cleanup",
            "--approve-recovery-action",
            "approve:tmp-cleanup:J-9999",
            "--json",
        ],
        &config,
    );

    assert_eq!(recover.exit_code, 0, "{}", recover.stderr);
    let recover_json: Value = serde_json::from_str(&recover.stdout).expect("recover json");
    assert_eq!(recover_json["status"], "blocked");
    assert_eq!(
        recover_json["data"]["approval_gate"]["approval_accepted"],
        false
    );
    assert_eq!(recover_json["data"]["action_execution_enabled"], false);
    assert_eq!(recover_json["data"]["destructive_actions_performed"], false);
    assert!(tmp_path.is_file());

    fs::remove_dir_all(project).ok();
}

#[test]
fn recover_action_approved_tmp_cleanup_executes_and_records_result() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_recovery_inspection_job(&project);
    let tmp_path = project.join(".ai-runs/J-0001/tmp/run-state.json.tmp-test");

    let recover = run_cli(
        [
            "recover",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--action",
            "tmp-cleanup",
            "--approve-recovery-action",
            "approve:tmp-cleanup:J-0001",
            "--json",
        ],
        &config,
    );

    assert_eq!(recover.exit_code, 0, "{}", recover.stderr);
    let recover_json: Value = serde_json::from_str(&recover.stdout).expect("recover json");
    assert_eq!(recover_json["status"], "success");
    assert_eq!(recover_json["data"]["mode"], "approved_execution");
    assert_eq!(recover_json["data"]["action_execution_enabled"], true);
    assert_eq!(
        recover_json["data"]["approval_gate"]["execution_after_approval"],
        "performed"
    );
    assert_eq!(recover_json["data"]["destructive_actions_performed"], true);
    assert_eq!(
        recover_json["data"]["recovery_execution"]["executed_changes"][0]["operation"],
        "delete_file"
    );
    assert!(!tmp_path.exists());
    assert!(project
        .join(".ai-runs/J-0001/recovery/tmp-cleanup-result.json")
        .is_file());

    fs::remove_dir_all(project).ok();
}

#[test]
fn recover_action_recovered_copy_executes_without_approval() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_recovery_inspection_job(&project);
    let state_path = project.join(".ai-runs/J-0001/run-state.json");
    fs::write(&state_path, "{ invalid state").expect("write corrupt state");

    let recover = run_cli(
        [
            "recover",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--action",
            "recovered-copy",
            "--json",
        ],
        &config,
    );

    assert_eq!(recover.exit_code, 0, "{}", recover.stderr);
    let recover_json: Value = serde_json::from_str(&recover.stdout).expect("recover json");
    assert_eq!(recover_json["status"], "success");
    assert_eq!(recover_json["data"]["mode"], "approved_execution");
    assert_eq!(recover_json["data"]["approval_required"], false);
    assert_eq!(
        recover_json["data"]["approval_gate"]["execution_after_approval"],
        "not_required"
    );
    assert_eq!(recover_json["data"]["action_execution_enabled"], true);
    assert_eq!(recover_json["data"]["destructive_actions_performed"], false);
    assert!(project
        .join(".ai-runs/J-0001/recovery/run-state.json.recovered-copy")
        .is_file());

    fs::remove_dir_all(project).ok();
}

#[test]
fn recover_action_approved_event_log_trim_replaces_corrupt_log() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_recovery_inspection_job(&project);
    let events_path = project.join(".ai-runs/J-0001/events.jsonl");
    let mut events = OpenOptions::new()
        .append(true)
        .open(&events_path)
        .expect("open events");
    writeln!(events, "{{ corrupt event").expect("append corrupt event");
    drop(events);

    let recover = run_cli(
        [
            "recover",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--action",
            "event-log-trim",
            "--approve-recovery-action",
            "approve:event-log-trim:J-0001",
            "--json",
        ],
        &config,
    );

    assert_eq!(recover.exit_code, 0, "{}", recover.stderr);
    let recover_json: Value = serde_json::from_str(&recover.stdout).expect("recover json");
    assert_eq!(recover_json["status"], "success");
    assert_eq!(recover_json["data"]["action_execution_enabled"], true);
    assert_eq!(recover_json["data"]["destructive_actions_performed"], true);
    assert!(project
        .join(".ai-runs/J-0001/recovery/events.trimmed.jsonl")
        .is_file());
    assert!(project
        .join(".ai-runs/J-0001/recovery/event-log-trim-result.json")
        .is_file());
    let trimmed_events = fs::read_to_string(&events_path).expect("trimmed events");
    assert!(!trimmed_events.contains("corrupt event"));

    fs::remove_dir_all(project).ok();
}

#[test]
fn recover_action_artifact_replace_requires_explicit_source_for_execution() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_recovery_inspection_job(&project);
    let state_path = project.join(".ai-runs/J-0001/run-state.json");
    fs::write(&state_path, "{ invalid state").expect("write corrupt state");

    let recover = run_cli(
        [
            "recover",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--action",
            "artifact-replace",
            "--approve-recovery-action",
            "approve:artifact-replace:J-0001",
            "--json",
        ],
        &config,
    );

    assert_eq!(recover.exit_code, 2);
    let recover_json: Value = serde_json::from_str(&recover.stdout).expect("recover json");
    assert_eq!(recover_json["error"]["code"], "InvalidInput");
    assert_eq!(
        fs::read_to_string(&state_path).expect("read corrupt state"),
        "{ invalid state"
    );
    assert!(!project
        .join(".ai-runs/J-0001/recovery/artifact-replace-result.json")
        .exists());

    fs::remove_dir_all(project).ok();
}

#[test]
fn recover_action_artifact_replace_rejects_non_matching_target() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_recovery_inspection_job(&project);
    fs::write(
        project.join(".ai-runs/J-0001/run-state.json"),
        "{ invalid state",
    )
    .expect("write corrupt state");
    let source_path = project.join(".ai-runs/J-0001/recovery/run-state.replacement.json");
    fs::create_dir_all(source_path.parent().expect("source parent")).expect("create recovery dir");
    fs::write(&source_path, replacement_state_json()).expect("write source");

    let recover = run_cli(
        [
            "recover",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--action",
            "artifact-replace",
            "--recovery-artifact",
            "reports/missing-report.json",
            "--recovery-source",
            "recovery/run-state.replacement.json",
            "--approve-recovery-action",
            "approve:artifact-replace:J-0001",
            "--json",
        ],
        &config,
    );

    assert_eq!(recover.exit_code, 2);
    let recover_json: Value = serde_json::from_str(&recover.stdout).expect("recover json");
    assert_eq!(recover_json["error"]["code"], "InvalidInput");

    fs::remove_dir_all(project).ok();
}

#[test]
fn recover_action_approved_artifact_replace_uses_explicit_source() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_recovery_inspection_job(&project);
    let state_path = project.join(".ai-runs/J-0001/run-state.json");
    fs::write(&state_path, "{ invalid state").expect("write corrupt state");
    let source_path = project.join(".ai-runs/J-0001/recovery/run-state.replacement.json");
    fs::create_dir_all(source_path.parent().expect("source parent")).expect("create recovery dir");
    let replacement = replacement_state_json();
    fs::write(&source_path, &replacement).expect("write source");

    let recover = run_cli(
        [
            "recover",
            "--project",
            project.to_str().expect("project path"),
            "--job",
            "J-0001",
            "--action",
            "artifact-replace",
            "--recovery-artifact",
            "run-state.json",
            "--recovery-source",
            "recovery/run-state.replacement.json",
            "--approve-recovery-action",
            "approve:artifact-replace:J-0001",
            "--json",
        ],
        &config,
    );

    assert_eq!(recover.exit_code, 0, "{}", recover.stderr);
    let recover_json: Value = serde_json::from_str(&recover.stdout).expect("recover json");
    assert_eq!(recover_json["status"], "success");
    assert_eq!(recover_json["data"]["mode"], "approved_execution");
    assert_eq!(recover_json["data"]["action_execution_enabled"], true);
    assert_eq!(recover_json["data"]["destructive_actions_performed"], true);
    assert_eq!(
        recover_json["data"]["recovery_execution"]["executed_changes"][0]["operation"],
        "replace_artifact_from_approved_source"
    );
    assert_eq!(
        recover_json["data"]["recovery_execution"]["executed_changes"][0]["source_path"],
        "recovery/run-state.replacement.json"
    );
    assert_eq!(
        fs::read_to_string(&state_path).expect("read replaced state"),
        replacement
    );
    assert!(project
        .join(".ai-runs/J-0001/recovery/artifact-replace-result.json")
        .is_file());

    fs::remove_dir_all(project).ok();
}

fn replacement_state_json() -> String {
    serde_json::to_string_pretty(&json!({
        "schema_version": "1.0.0",
        "job_id": "J-0001",
        "state": "DONE",
        "current_stage": "report",
        "updated_at": "test:artifact-replace",
        "threads": {},
        "workers": {},
        "artifacts": {},
        "latest_event_id": "J-0001-0001",
        "active_provider": null,
        "next_action": "none",
        "budget": {},
        "history": []
    }))
    .expect("replacement json")
}
