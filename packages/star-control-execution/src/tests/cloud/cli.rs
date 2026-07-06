use crate::test_support::{EnvVarGuard, Fixture};
use crate::ExecutionEngine;
use serde_json::Value;

#[test]
fn cloud_cli_transport_records_handoff_and_updates_state() {
    let mut fixture = Fixture::new();
    let _env = EnvVarGuard::set("STAR_CONTROL_EXECUTION_CLOUD_CLI_HELPER", "1");
    fixture.use_cloud_cli_registry(
        vec![
            "--exact".to_string(),
            "tests::execution_cloud_cli_success_helper".to_string(),
            "--nocapture".to_string(),
        ],
        vec!["STAR_CONTROL_EXECUTION_CLOUD_CLI_HELPER".to_string()],
        10,
    );
    fixture.assign_implement_stage_to_cloud_provider();

    let outcome = ExecutionEngine::new(&fixture.store, &fixture.registry, &fixture.schemas)
        .execute_stage("J-0001", "implement")
        .expect("execute cloud CLI stage");

    assert_eq!(outcome.request().provider_instance_id(), "cloud-default");
    assert_eq!(outcome.provider_execution().result().status(), "success");
    assert_eq!(outcome.state()["state"], "IMPLEMENTED");
    assert_eq!(
        outcome.provider_execution().result().value()["error"],
        Value::Null
    );
    assert_eq!(
        outcome.state()["artifacts"]["implement_provider_request"]["path"],
        "provider-output/cloud-default/request.json"
    );
    assert_eq!(
        outcome.state()["artifacts"]["implement_provider_response"]["path"],
        "provider-output/cloud-default/response.json"
    );
    assert_eq!(
        outcome.state()["artifacts"]["implement_provider_stdout"]["path"],
        "provider-output/cloud-default/stdout.txt"
    );
    assert_eq!(
        outcome.state()["artifacts"]["implement_provider_stderr"]["path"],
        "provider-output/cloud-default/stderr.txt"
    );
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/provider-output/cloud-default/privacy-handoff.json")
        .is_file());
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/provider-output/cloud-default/cost-metric.json")
        .is_file());

    let result = outcome.provider_execution().result().value();
    assert!(result["artifacts"]
        .as_array()
        .expect("artifacts")
        .iter()
        .any(|path| path == "provider-output/cloud-default/privacy-handoff.json"));
    let events = fixture.store.read_events("J-0001").expect("events");
    assert!(events.iter().any(|event| {
        event["type"] == "PROVIDER_FINISHED" && event["details"]["status"] == "success"
    }));
}
