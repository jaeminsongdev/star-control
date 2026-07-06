use crate::test_support::Fixture;
use crate::ExecutionEngine;
use serde_json::{json, Value};
use std::fs;

#[test]
fn cloud_api_offline_fixture_updates_state_without_live_call() {
    let mut fixture = Fixture::new();
    fixture.write_openai_response_fixture(
        "fixtures/openai-response.json",
        &json!({
            "id": "resp_execution_fixture",
            "model": "gpt-example",
            "status": "completed",
            "output_text": "execution offline answer",
            "usage": {
                "input_tokens": 8,
                "output_tokens": 13,
                "total_tokens": 21
            }
        }),
    );
    fixture.use_cloud_api_offline_registry("fixtures/openai-response.json");
    fixture.assign_implement_stage_to_cloud_provider();

    let outcome = ExecutionEngine::new(&fixture.store, &fixture.registry, &fixture.schemas)
        .execute_stage("J-0001", "implement")
        .expect("execute cloud API offline stage");

    assert_eq!(outcome.request().provider_instance_id(), "cloud-default");
    assert_eq!(outcome.provider_execution().result().status(), "success");
    assert_eq!(outcome.attempt()["status"], "success");
    assert_eq!(outcome.state()["state"], "IMPLEMENTED");
    assert_eq!(
        outcome.provider_execution().result().value()["summary"],
        "execution offline answer"
    );
    assert_eq!(
        outcome.provider_execution().result().value()["metrics"]["transport_execution"],
        "offline_fixture"
    );
    assert_eq!(
        outcome.provider_execution().result().value()["metrics"]["input_tokens"],
        8
    );
    assert_eq!(
        outcome.state()["artifacts"]["implement_provider_request"]["path"],
        "provider-output/cloud-default/request.json"
    );
    assert_eq!(
        outcome.state()["artifacts"]["implement_provider_response"]["path"],
        "provider-output/cloud-default/response.json"
    );
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/provider-output/cloud-default/http-request.json")
        .is_file());
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/provider-output/cloud-default/http-transport-plan.json")
        .is_file());
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/provider-output/cloud-default/raw-response.json")
        .is_file());
    let http_request: Value = serde_json::from_str(
        &fs::read_to_string(
            fixture
                .project
                .join(".ai-runs/J-0001/provider-output/cloud-default/http-request.json"),
        )
        .expect("read http request"),
    )
    .expect("parse http request");
    assert_eq!(http_request["url"], "https://api.openai.com/v1/responses");
    assert_eq!(http_request["body"]["input"], "runtime code 구현");
    let http_request_text = serde_json::to_string(&http_request).expect("serialize http request");
    assert!(!http_request_text.contains("OPENAI_API_KEY"));
    let transport_plan: Value = serde_json::from_str(
        &fs::read_to_string(
            fixture
                .project
                .join(".ai-runs/J-0001/provider-output/cloud-default/http-transport-plan.json"),
        )
        .expect("read transport plan"),
    )
    .expect("parse transport plan");
    assert_eq!(transport_plan["credential"]["reference_kind"], "env");
    assert_eq!(transport_plan["credential"]["materialized"], false);
    assert_eq!(transport_plan["live_api_call"], false);
    let transport_plan_text =
        serde_json::to_string(&transport_plan).expect("serialize transport plan");
    assert!(!transport_plan_text.contains("OPENAI_API_KEY"));

    let events = fixture.store.read_events("J-0001").expect("events");
    assert!(events.iter().any(|event| {
        event["type"] == "PROVIDER_FINISHED" && event["details"]["status"] == "success"
    }));
}
