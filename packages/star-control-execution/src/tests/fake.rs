use crate::test_support::Fixture;
use crate::ExecutionError;
use serde_json::json;
use star_control_provider::{FakeProviderAdapter, ProviderRegistryError};

#[test]
fn fake_provider_workspec_execution_writes_artifacts_and_state() {
    let fixture = Fixture::new();
    let outcome = fixture
        .engine(FakeProviderAdapter::success())
        .execute_stage("J-0001", "implement")
        .expect("execute stage");

    assert_eq!(outcome.request().provider_instance_id(), "fake-default");
    assert_eq!(outcome.provider_execution().result().status(), "success");
    assert_eq!(outcome.attempt()["status"], "success");
    assert_eq!(outcome.state()["state"], "IMPLEMENTED");
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/provider-output/fake-default/request.json")
        .is_file());
    assert!(fixture
        .project
        .join(".ai-runs/J-0001/provider-output/fake-default/response.json")
        .is_file());

    let events = fixture.store.read_events("J-0001").expect("events");
    assert!(events
        .iter()
        .any(|event| event["type"] == "PROVIDER_STARTED"));
    assert!(events
        .iter()
        .any(|event| event["type"] == "PROVIDER_FINISHED"));
}

#[test]
fn execution_refuses_to_overwrite_existing_provider_output() {
    let fixture = Fixture::new();
    let engine = fixture.engine(FakeProviderAdapter::success());
    engine
        .execute_stage("J-0001", "implement")
        .expect("first execute");
    let error = engine
        .execute_stage("J-0001", "implement")
        .expect_err("second execute should fail");

    assert!(matches!(error, ExecutionError::StageAlreadyExecuted { .. }));
}

#[test]
fn failed_and_blocked_provider_results_update_state() {
    let failed = Fixture::new();
    let failed_outcome = failed
        .engine(FakeProviderAdapter::failed("unit failure"))
        .execute_stage("J-0001", "implement")
        .expect("failed execution");
    assert_eq!(
        failed_outcome.provider_execution().result().status(),
        "failed"
    );
    assert_eq!(failed_outcome.state()["state"], "FAILED");

    let blocked = Fixture::new();
    let blocked_outcome = blocked
        .engine(FakeProviderAdapter::blocked("approval required"))
        .execute_stage("J-0001", "implement")
        .expect("blocked execution");
    assert_eq!(
        blocked_outcome.provider_execution().result().status(),
        "blocked"
    );
    assert_eq!(blocked_outcome.state()["state"], "BLOCKED");
}

#[test]
fn unknown_provider_instance_fails_before_writing_output() {
    let fixture = Fixture::new();
    let mut workspec = fixture
        .store
        .load_workspec("J-0001", "implement")
        .expect("workspec");
    workspec["provider"] = json!("missing-provider");
    workspec["provider_instance"] = json!("missing-provider");
    fixture
        .store
        .save_workspec("J-0001", "implement", &workspec)
        .expect("save unknown provider workspec");

    let error = fixture
        .engine(FakeProviderAdapter::success())
        .execute_stage("J-0001", "implement")
        .expect_err("unknown provider should fail");

    assert!(matches!(
        error,
        ExecutionError::ProviderRegistry(ProviderRegistryError::InstanceNotFound { .. })
    ));
    assert!(!fixture
        .project
        .join(".ai-runs/J-0001/provider-output/missing-provider/response.json")
        .exists());
}
