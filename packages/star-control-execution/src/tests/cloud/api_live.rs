use crate::test_support::Fixture;
use crate::ExecutionEngine;
use serde_json::Value;
use std::fs;

#[test]
fn cloud_api_live_transport_request_blocks_pending_approval_without_live_call() {
    let mut fixture = Fixture::new();
    fixture.use_cloud_api_live_approval_registry();
    fixture.assign_implement_stage_to_cloud_provider();

    let outcome = ExecutionEngine::new(&fixture.store, &fixture.registry, &fixture.schemas)
        .execute_stage("J-0001", "implement")
        .expect("execute cloud API live approval stage");

    assert_eq!(outcome.request().provider_instance_id(), "cloud-default");
    assert_eq!(outcome.provider_execution().result().status(), "blocked");
    assert_eq!(outcome.attempt()["status"], "blocked");
    assert_eq!(outcome.state()["state"], "BLOCKED");
    assert_eq!(
        outcome.provider_execution().result().value()["error"]["kind"],
        "cloud_api_live_transport_approval_required"
    );
    assert_eq!(
        outcome.provider_execution().result().value()["metrics"]["transport_execution"],
        "approval_required"
    );
    assert_eq!(
        outcome.provider_execution().result().value()["metrics"]["live_api_call"],
        false
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
        .join(".ai-runs/J-0001/provider-output/cloud-default/live-transport-approval.json")
        .is_file());
    assert!(!fixture
        .project
        .join(".ai-runs/J-0001/provider-output/cloud-default/raw-response.json")
        .exists());

    let transport_plan: Value = serde_json::from_str(
        &fs::read_to_string(
            fixture
                .project
                .join(".ai-runs/J-0001/provider-output/cloud-default/http-transport-plan.json"),
        )
        .expect("read transport plan"),
    )
    .expect("parse transport plan");
    assert_eq!(transport_plan["execution_mode"], "live_approval_required");
    assert_eq!(transport_plan["credential"]["reference_kind"], "env");
    assert_eq!(transport_plan["credential"]["materialized"], false);
    assert_eq!(transport_plan["live_api_call"], false);
    let transport_plan_text =
        serde_json::to_string(&transport_plan).expect("serialize transport plan");
    assert!(!transport_plan_text.contains("OPENAI_API_KEY"));

    let approval: Value =
        serde_json::from_str(
            &fs::read_to_string(fixture.project.join(
                ".ai-runs/J-0001/provider-output/cloud-default/live-transport-approval.json",
            ))
            .expect("read live approval"),
        )
        .expect("parse live approval");
    assert_eq!(
        approval["kind"],
        "cloud_api_live_transport_approval_required"
    );
    assert_eq!(approval["approval_required"], true);
    assert_eq!(approval["credential"]["materialized"], false);
    assert_eq!(approval["live_api_call"], false);
    let approval_text = serde_json::to_string(&approval).expect("serialize approval");
    assert!(!approval_text.contains("OPENAI_API_KEY"));

    let events = fixture.store.read_events("J-0001").expect("events");
    assert!(events.iter().any(|event| {
        event["type"] == "PROVIDER_FINISHED" && event["details"]["status"] == "blocked"
    }));
}
