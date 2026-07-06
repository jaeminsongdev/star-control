mod helpers;

use crate::test_support::*;
use crate::CliConfig;
use helpers::{
    assert_error_code, parse_stdout_json, run_approve_with_constraint,
    run_approve_without_constraint, run_cancel, run_dry_run, run_resume,
};
use star_control_state::StateStore;
use std::fs;

#[test]
fn approve_writes_response_and_resume_advances_waiting_approval_gate() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_waiting_approval_job(&project, true);

    let approve = run_approve_with_constraint(&project, &config);
    assert_eq!(approve.exit_code, 0, "{}", approve.stderr);
    let approve_json = parse_stdout_json(&approve, "approve json");
    assert_eq!(approve_json["command"], "approve");
    assert_eq!(approve_json["status"], "success");
    assert_eq!(approve_json["data"]["state"], "WAITING_APPROVAL");
    assert_eq!(approve_json["data"]["approval_response"], "approved");
    assert_eq!(approve_json["data"]["allowed_next_stage"], "report");
    assert!(project
        .join(".ai-runs/J-0001/approvals/approval-response.json")
        .is_file());

    let store = StateStore::open(&project, repo_root().join("specs/schemas")).expect("store");
    let approved_state = store.load_state("J-0001").expect("state after approve");
    assert_eq!(approved_state["state"], "WAITING_APPROVAL");
    assert_eq!(approved_state["next_action"], "resume");
    assert_eq!(
        approved_state["artifacts"]["approval_response"]["path"],
        "approvals/approval-response.json"
    );

    let resume = run_resume(&project, &config);
    assert_eq!(resume.exit_code, 0, "{}", resume.stderr);
    let resume_json = parse_stdout_json(&resume, "resume json");
    assert_eq!(resume_json["command"], "resume");
    assert_eq!(resume_json["data"]["previous_state"], "WAITING_APPROVAL");
    assert_eq!(resume_json["data"]["state"], "VALIDATED");
    assert_eq!(resume_json["data"]["next_action"], "report");
    let resumed_state = store.load_state("J-0001").expect("state after resume");
    assert_eq!(resumed_state["state"], "VALIDATED");
    assert_eq!(resumed_state["next_action"], "report");
    let events = store.read_events("J-0001").expect("events");
    assert!(events
        .iter()
        .any(|event| event["type"] == "APPROVAL_RECORDED"));
    assert!(events
        .iter()
        .any(|event| { event["type"] == "STATE_CHANGED" && event["state"] == "VALIDATED" }));

    fs::remove_dir_all(project).ok();
}

#[test]
fn approve_requires_approval_request_artifact() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_waiting_approval_job(&project, false);

    let approve = run_approve_without_constraint(&project, &config);
    assert_eq!(approve.exit_code, 3);
    let error_json = assert_error_code(&approve, "approve error json", "MissingArtifact");
    assert_eq!(
        error_json["error"]["artifact_paths"][0],
        ".ai-runs/J-0001/approvals/approval-request.json"
    );

    fs::remove_dir_all(project).ok();
}

#[test]
fn resume_waiting_approval_requires_approved_response() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    write_waiting_approval_job(&project, true);

    let resume = run_resume(&project, &config);
    assert_eq!(resume.exit_code, 3);
    let error_json = assert_error_code(&resume, "resume error json", "MissingArtifact");
    assert_eq!(
        error_json["error"]["artifact_paths"][0],
        ".ai-runs/J-0001/approvals/approval-response.json"
    );

    fs::remove_dir_all(project).ok();
}

#[test]
fn cancel_updates_nonterminal_state_and_rejects_terminal_cancel() {
    let project = temp_project();
    let config = CliConfig::new(repo_root());
    let run = run_dry_run(&project, &config);
    assert_eq!(run.exit_code, 0, "{}", run.stderr);

    let cancel = run_cancel(&project, &config);
    assert_eq!(cancel.exit_code, 0, "{}", cancel.stderr);
    let cancel_json = parse_stdout_json(&cancel, "cancel json");
    assert_eq!(cancel_json["command"], "cancel");
    assert_eq!(cancel_json["data"]["previous_state"], "ROUTED");
    assert_eq!(cancel_json["data"]["state"], "CANCELLED");
    let store = StateStore::open(&project, repo_root().join("specs/schemas")).expect("store");
    assert_eq!(
        store.load_state("J-0001").expect("state")["state"],
        "CANCELLED"
    );

    let second_cancel = run_cancel(&project, &config);
    assert_eq!(second_cancel.exit_code, 2);
    assert_error_code(&second_cancel, "cancel error json", "InvalidInput");

    fs::remove_dir_all(project).ok();
}
