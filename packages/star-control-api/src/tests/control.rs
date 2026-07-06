use super::*;
use serde_json::json;

#[test]
fn control_approve_and_resume_match_cli_gate() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store, "WAITING_APPROVAL", "validate");
    store
        .write_approval_json(
            "J-0001",
            "approval-request.json",
            &approval_request("J-0001", "validate"),
        )
        .expect("write approval request");
    let service = control_with_store(store.clone());

    let approve = service
        .handle_post(
            "/projects/local/jobs/J-0001/approve",
            json!({
                "response": "approved",
                "reason": "approved by API test",
                "constraints": ["keep schema stable"]
            }),
        )
        .expect("approve response");
    assert_eq!(approve["status"], "success");
    assert_eq!(approve["data"]["command"], "approve");
    assert_eq!(approve["data"]["state"], "WAITING_APPROVAL");
    assert_eq!(approve["data"]["approval_response"], "approved");
    assert_eq!(approve["data"]["allowed_next_stage"], "report");
    assert!(project
        .join(".ai-runs/J-0001/approvals/approval-response.json")
        .is_file());

    let approved_state = store.load_state("J-0001").expect("state after approve");
    assert_eq!(approved_state["state"], "WAITING_APPROVAL");
    assert_eq!(approved_state["next_action"], "resume");
    assert_eq!(
        approved_state["artifacts"]["approval_response"]["path"],
        "approvals/approval-response.json"
    );

    let resume = service
        .handle_post("/projects/local/jobs/J-0001/resume", json!({}))
        .expect("resume response");
    assert_eq!(resume["status"], "success");
    assert_eq!(resume["data"]["command"], "resume");
    assert_eq!(resume["data"]["previous_state"], "WAITING_APPROVAL");
    assert_eq!(resume["data"]["state"], "VALIDATED");
    assert_eq!(resume["data"]["next_action"], "report");

    let resumed_state = store.load_state("J-0001").expect("state after resume");
    assert_eq!(resumed_state["state"], "VALIDATED");
    assert_eq!(resumed_state["next_action"], "report");
    let events = store.read_events("J-0001").expect("events");
    assert!(events.iter().any(|event| {
        event["type"] == "APPROVAL_RECORDED" && event["state"] == "WAITING_APPROVAL"
    }));
    assert!(events
        .iter()
        .any(|event| { event["type"] == "STATE_CHANGED" && event["state"] == "VALIDATED" }));

    fs::remove_dir_all(project).ok();
}

#[test]
fn control_cancel_updates_nonterminal_and_rejects_terminal() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store, "ROUTED", "implement");
    let service = control_with_store(store.clone());

    let cancel = service
        .handle_post("/projects/local/jobs/J-0001/cancel", json!({}))
        .expect("cancel response");
    assert_eq!(cancel["status"], "success");
    assert_eq!(cancel["data"]["command"], "cancel");
    assert_eq!(cancel["data"]["previous_state"], "ROUTED");
    assert_eq!(cancel["data"]["state"], "CANCELLED");
    assert_eq!(
        store.load_state("J-0001").expect("cancelled state")["state"],
        "CANCELLED"
    );

    let second_cancel = service
        .handle_post("/projects/local/jobs/J-0001/cancel", json!({}))
        .expect("second cancel response");
    assert_eq!(second_cancel["status"], "failed");
    assert_eq!(second_cancel["error"]["code"], "invalid_control_state");

    fs::remove_dir_all(project).ok();
}

#[test]
fn control_requires_approval_request_and_approved_response() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store, "WAITING_APPROVAL", "validate");
    let service = control_with_store(store.clone());

    let missing_request = service
        .handle_post(
            "/projects/local/jobs/J-0001/approve",
            json!({
                "response": "approved",
                "reason": "missing request"
            }),
        )
        .expect("missing request response");
    assert_eq!(missing_request["status"], "failed");
    assert_eq!(missing_request["error"]["code"], "approval_request_missing");

    store
        .write_approval_json(
            "J-0001",
            "approval-request.json",
            &approval_request("J-0001", "validate"),
        )
        .expect("write approval request");
    let missing_response = service
        .handle_post("/projects/local/jobs/J-0001/resume", json!({}))
        .expect("missing response");
    assert_eq!(missing_response["status"], "failed");
    assert_eq!(
        missing_response["error"]["code"],
        "approval_response_missing"
    );

    fs::remove_dir_all(project).ok();
}
