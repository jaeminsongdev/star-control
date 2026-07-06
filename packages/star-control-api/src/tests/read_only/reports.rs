use super::super::{api_with_store, create_job, event, open_store, report, temp_project};
use super::helpers::{assert_redacted_text, assert_success};
use std::fs;

#[test]
fn events_and_report_endpoints_return_read_artifacts() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store, "DONE", "report");
    store
        .append_event("J-0001", &event("J-0001"))
        .expect("event");
    store
        .save_report("J-0001", "implement-report", &report("J-0001", Vec::new()))
        .expect("save report");
    let service = api_with_store(store);

    let events = service
        .handle_get("/projects/local/jobs/J-0001/events")
        .expect("events");
    assert_success(&events);
    assert_eq!(events["data"]["event_count"], 2);

    let report = service
        .handle_get("/projects/local/jobs/J-0001/report?stage=implement")
        .expect("report");
    assert_success(&report);
    assert_eq!(
        report["data"]["report_path"],
        ".ai-runs/J-0001/reports/implement-report.json"
    );
    assert_eq!(report["data"]["report"]["status"], "DONE");

    fs::remove_dir_all(project).ok();
}

#[test]
fn report_response_redacts_sensitive_values() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store, "DONE", "report");
    store
        .save_report(
            "J-0001",
            "implement-report",
            &report("J-0001", vec!["Authorization: Bearer sk-test-secret"]),
        )
        .expect("save report");
    let service = api_with_store(store);

    let response = service
        .handle_get("/projects/local/jobs/J-0001/report")
        .expect("report");
    let text = serde_json::to_string(&response).expect("response text");
    assert_redacted_text(&text, "sk-test-secret");

    fs::remove_dir_all(project).ok();
}
