#[path = "v0_fake_flow/support.rs"]
mod support;

use support::{changed_lines_for, context, SmokeFixture};

#[test]
fn v0_fake_flow_auto_pass_reaches_done_report() {
    let fixture = SmokeFixture::new();
    let run = fixture.run_fake_cli("runtime code 구현");
    assert_eq!(run["data"]["state"], "IMPLEMENTED");

    let outcome = fixture.run_validation(
        "p0-auto-pass",
        ["src/**"],
        changed_lines_for("p0-auto-pass", "src/lib.rs", "modified"),
    );

    assert_eq!(outcome["decision"]["next_state"], "VALIDATED");
    fixture.write_done_report("J-0001", "AUTO_PASS smoke reached final report");

    let report = fixture.report_json("J-0001", "report");
    assert_eq!(report["data"]["report"]["stage"], "report");
    assert_eq!(report["data"]["report"]["status"], "DONE");
    assert_eq!(
        fixture.store.load_state("J-0001").expect("state")["state"],
        "DONE"
    );
}

#[test]
fn v0_fake_flow_human_review_waits_for_matching_approval() {
    let fixture = SmokeFixture::new();
    fixture.run_fake_cli("runtime code 구현");

    let outcome = fixture.run_validation(
        "p0-human-review",
        ["**"],
        changed_lines_for("p0-human-review", "Cargo.toml", "modified"),
    );

    assert_eq!(outcome["decision"]["next_state"], "WAITING_APPROVAL");
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/approvals/approval-request.json")
        .is_file());
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/review-packs/handoff.json")
        .is_file());

    let missing = fixture
        .validation_engine()
        .ensure_approval_response_allows_next_stage(&context("p0-human-review"))
        .unwrap_err();
    assert!(missing.to_string().contains("approval response missing"));

    fixture.write_approval_response("J-0001", "p0-human-review");
    let response = fixture
        .validation_engine()
        .ensure_approval_response_allows_next_stage(&context("p0-human-review"))
        .expect("approved response");
    assert_eq!(response["response"], "approved");

    fixture.write_done_report("J-0001", "HUMAN_REVIEW smoke completed after approval");
    assert_eq!(
        fixture.store.load_state("J-0001").expect("state")["state"],
        "DONE"
    );
}

#[test]
fn v0_fake_flow_block_stops_at_blocked_state() {
    let fixture = SmokeFixture::new();
    fixture.run_fake_cli("runtime code 구현");

    let outcome = fixture.run_validation(
        "p0-block",
        ["src/allowed/**"],
        changed_lines_for("p0-block", "src/other.rs", "modified"),
    );

    assert_eq!(outcome["decision"]["next_state"], "BLOCKED");
    assert_eq!(
        fixture.store.load_state("J-0001").expect("state")["state"],
        "BLOCKED"
    );
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/tool-output/star-sentinel/approval.json")
        .is_file());
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/validation/validation-decision.json")
        .is_file());
}
