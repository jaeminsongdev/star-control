use super::{create_job, open_store, state, temp_project};
use crate::StateStoreError;
use std::fs::{self, OpenOptions};
use std::io::Write;

#[test]
fn recovery_inspection_reports_ok_for_complete_job() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store);
    store
        .save_state("J-0001", &state("J-0001", "REQUESTED"))
        .expect("save state");

    let inspection = store.inspect_recovery("J-0001").expect("inspect recovery");
    assert_eq!(inspection.status, "ok");
    assert_eq!(inspection.mode, "inspect_only");
    assert!(!inspection.manual_followup_required);
    assert!(!inspection.destructive_actions_performed);
    assert!(inspection.issues.is_empty());
    assert_eq!(inspection.to_value()["status"], "ok");

    fs::remove_dir_all(project).ok();
}

#[test]
fn recovery_inspection_reports_missing_required_files_and_tmp_without_mutation() {
    let project = temp_project();
    let store = open_store(&project);
    let corrupt_dir = project.join(".ai-runs/J-0001");
    fs::create_dir_all(corrupt_dir.join("tmp/nested")).expect("create corrupt job dirs");
    let tmp_file = corrupt_dir.join("tmp/nested/run-state.json.tmp-test");
    fs::write(&tmp_file, "{ partial json").expect("write tmp file");

    let inspection = store
        .inspect_recovery("J-0001")
        .expect("inspect corrupt job");
    let kinds: Vec<_> = inspection
        .issues
        .iter()
        .map(|issue| issue.kind.as_str())
        .collect();

    assert_eq!(inspection.status, "needs_recovery");
    assert!(inspection.manual_followup_required);
    assert!(!inspection.destructive_actions_performed);
    assert!(kinds.contains(&"missing_required_file"));
    assert!(kinds.contains(&"partial_tmp_file"));
    assert!(inspection
        .issues
        .iter()
        .any(|issue| issue.artifact_path == "tmp/nested/run-state.json.tmp-test"));
    assert!(tmp_file.is_file(), "inspection must not delete tmp files");

    fs::remove_dir_all(project).ok();
}

#[test]
fn recovery_inspection_reports_invalid_state_and_corrupt_event_log() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store);
    fs::write(
        project.join(".ai-runs/J-0001/run-state.json"),
        "{ invalid json",
    )
    .expect("write invalid state");
    let mut events = OpenOptions::new()
        .append(true)
        .open(project.join(".ai-runs/J-0001/events.jsonl"))
        .expect("open events");
    writeln!(events, "{{ invalid event").expect("append corrupt event");

    let inspection = store
        .inspect_recovery("J-0001")
        .expect("inspect corrupt artifacts");
    assert!(inspection
        .issues
        .iter()
        .any(|issue| issue.artifact_path == "run-state.json" && issue.kind == "invalid_json"));
    assert!(inspection
        .issues
        .iter()
        .any(|issue| issue.artifact_path == "events.jsonl" && issue.kind == "corrupt_event_log"));
    assert_eq!(
        inspection.to_value()["destructive_actions_performed"],
        false
    );

    fs::remove_dir_all(project).ok();
}

#[test]
fn recovery_inspection_rejects_unsafe_job_id() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store);

    assert!(matches!(
        store.inspect_recovery("../J-0001"),
        Err(StateStoreError::InvalidJobId { .. })
    ));

    fs::remove_dir_all(project).ok();
}
