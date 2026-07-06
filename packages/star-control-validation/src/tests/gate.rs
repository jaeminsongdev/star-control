use super::*;

#[test]
fn auto_pass_maps_to_validated_and_writes_core_artifacts() {
    let fixture = Fixture::new();
    fixture.create_job_with_state();
    fixture.write_provider_response();
    fixture
        .engine()
        .ensure_provider_response("J-0001", "fake-default")
        .expect("provider response");

    let outcome = fixture
        .engine()
        .evaluate_star_sentinel_gate(&context(), &approval("AUTO_PASS"), None)
        .expect("evaluate");
    assert_eq!(outcome.next_state(), Some("VALIDATED"));

    let written = fixture
        .engine()
        .write_outcome(&context(), &outcome)
        .expect("write outcome");

    assert_eq!(
        written.decision_ref()["path"],
        "validation/validation-decision.json"
    );
    assert_eq!(written.validation_run_ref()["kind"], "tool_output");
    assert_eq!(written.state()["state"], "VALIDATED");
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/validation/validation-decision.json")
        .is_file());
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/tool-output/star-sentinel/validation_runs.json")
        .is_file());
    let events = fixture.store.read_events("J-0001").expect("events");
    assert!(events.iter().any(|event| event["type"] == "GATE_DECIDED"));
    assert!(events
        .iter()
        .any(|event| event["type"] == "VALIDATION_RECORDED"));
}

#[test]
fn human_review_maps_to_waiting_approval_and_writes_handoff() {
    let fixture = Fixture::new();
    fixture.create_job_with_state();

    let outcome = fixture
        .engine()
        .evaluate_star_sentinel_gate(
            &context(),
            &approval("HUMAN_REVIEW"),
            Some(&review_pack("HUMAN_REVIEW")),
        )
        .expect("evaluate");

    assert_eq!(outcome.next_state(), Some("WAITING_APPROVAL"));
    assert!(outcome.approval_request().is_some());
    assert!(outcome.handoff().is_some());

    let written = fixture
        .engine()
        .write_outcome(&context(), &outcome)
        .expect("write outcome");

    assert_eq!(written.state()["state"], "WAITING_APPROVAL");
    assert_eq!(
        written.approval_request_ref().expect("approval ref")["path"],
        "approvals/approval-request.json"
    );
    assert_eq!(
        written.handoff_ref().expect("handoff ref")["path"],
        "review-packs/handoff.json"
    );
}

#[test]
fn block_maps_to_blocked() {
    let fixture = Fixture::new();
    fixture.create_job_with_state();

    let outcome = fixture
        .engine()
        .evaluate_star_sentinel_gate(&context(), &approval("BLOCK"), Some(&review_pack("BLOCK")))
        .expect("evaluate");

    assert_eq!(outcome.next_state(), Some("BLOCKED"));
    assert_eq!(outcome.validation_run()["status"], "FAIL");
}

#[test]
fn invalid_approval_output_maps_to_failed() {
    let fixture = Fixture::new();
    fixture.create_job_with_state();

    let outcome = fixture
        .engine()
        .evaluate_star_sentinel_gate(
            &context(),
            &json!({
                "schema_version": "1.0.0",
                "task_id": "p0-task-demo",
                "reasons": [],
                "diagnostics": []
            }),
            None,
        )
        .expect("failed outcome");

    assert_eq!(outcome.next_state(), Some("FAILED"));
    assert_eq!(
        outcome.decision()["reasons"][0],
        "star_sentinel_output_invalid"
    );
    assert_eq!(outcome.validation_run()["status"], "ERROR");
}

#[test]
fn auto_pass_with_block_diagnostic_maps_to_failed() {
    let fixture = Fixture::new();
    fixture.create_job_with_state();
    let approval = json!({
        "schema_version": "1.0.0",
        "task_id": "p0-task-demo",
        "decision": "AUTO_PASS",
        "reasons": [],
        "diagnostics": [
            {
                "rule_id": "scope.allowed_paths",
                "severity": "block"
            }
        ]
    });

    let outcome = fixture
        .engine()
        .evaluate_star_sentinel_gate(&context(), &approval, None)
        .expect("failed outcome");

    assert_eq!(outcome.next_state(), Some("FAILED"));
    assert_eq!(
        outcome.decision()["reasons"][0],
        "star_sentinel_output_inconsistent"
    );
}
