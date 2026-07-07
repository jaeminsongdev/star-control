use super::*;
use crate::ProviderRegistryLoader;
use helpers::{
    execute_with_adapter, open_store, repo_root, request_value, schema_root, temp_project,
};
use serde_json::json;
use std::fs;
use std::path::PathBuf;

mod helpers;

#[test]
fn loads_execution_request_example() {
    let request = load_execution_request(
        repo_root().join("examples/execution-contracts/execution-request.fake.example.json"),
        schema_root(),
    )
    .expect("load request example");

    assert_eq!(request.request_id(), "request-0001");
    assert_eq!(request.job_id(), "J-0001");
    assert_eq!(request.provider_instance_id(), "fake-default");
}

#[test]
fn fake_provider_writes_deterministic_success_output() {
    let project = temp_project();
    let store = open_store(&project);
    store
        .create_job("implement feature", "codex", vec![])
        .expect("create job");
    let registry = ProviderRegistryLoader::new(repo_root())
        .load_fake_default_registry()
        .expect("load fake registry");
    let request = request_value("success goal");
    let request =
        ExecutionRequest::from_value(request, "request.json", schema_root()).expect("request");
    let schemas = schema_root();
    let context = ProviderRunContext::new(&registry, &store, &schemas);

    let execution = FakeProviderAdapter::success()
        .execute(&request, &context)
        .expect("execute fake provider");

    assert_eq!(execution.result().status(), "success");
    assert_eq!(
        execution.result().value()["metrics"]["estimated_cost"],
        json!(0)
    );
    assert_eq!(
        execution.request_ref()["path"],
        "provider-output/fake-default/request.json"
    );
    assert_eq!(
        execution.response_ref()["path"],
        "provider-output/fake-default/response.json"
    );
    assert_eq!(
        execution.stdout_ref()["path"],
        "provider-output/fake-default/stdout.txt"
    );
    assert!(execution.stderr_ref().is_none());
    assert!(project
        .join(".ai-runs/J-0001/provider-output/fake-default/request.json")
        .is_file());
    assert!(project
        .join(".ai-runs/J-0001/provider-output/fake-default/response.json")
        .is_file());
    let cost_metric: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            project.join(".ai-runs/J-0001/provider-output/fake-default/cost-metric.json"),
        )
        .expect("read fake cost metric"),
    )
    .expect("parse fake cost metric");
    assert_eq!(cost_metric["estimated_cost"], json!(0));
    assert_eq!(cost_metric["currency"], "USD");
    assert_eq!(cost_metric["wall_time_ms"], json!(0));

    fs::remove_dir_all(project).ok();
}

#[test]
fn fake_provider_redacts_request_artifact_and_writes_redaction_report() {
    let project = temp_project();
    let store = open_store(&project);
    store
        .create_job("implement feature", "codex", vec![])
        .expect("create job");
    let registry = ProviderRegistryLoader::new(repo_root())
        .load_fake_default_registry()
        .expect("load fake registry");
    let request = request_value("Authorization: Bearer sk-test-secret");
    let request =
        ExecutionRequest::from_value(request, "request.json", schema_root()).expect("request");
    let schemas = schema_root();
    let context = ProviderRunContext::new(&registry, &store, &schemas);

    let execution = FakeProviderAdapter::success()
        .execute(&request, &context)
        .expect("execute fake provider");

    let request_text = fs::read_to_string(
        project.join(".ai-runs/J-0001/provider-output/fake-default/request.json"),
    )
    .expect("read request");
    assert!(!request_text.contains("sk-test-secret"));
    assert!(request_text.contains("[REDACTED]"));
    assert!(project
        .join(".ai-runs/J-0001/audit/provider-redaction-fake-default-request-json.json")
        .is_file());
    assert!(execution.result().value()["artifacts"]
        .as_array()
        .expect("artifacts")
        .iter()
        .any(|artifact| artifact == "audit/provider-redaction-fake-default-request-json.json"));

    fs::remove_dir_all(project).ok();
}

#[test]
fn fake_provider_simulates_failed_and_blocked_results() {
    let failed = execute_with_adapter(FakeProviderAdapter::failed("unit failure"));
    assert_eq!(failed.result().status(), "failed");
    assert_eq!(failed.result().value()["error"]["kind"], "fake_failed");
    assert!(failed.stderr_ref().is_some());

    let blocked = execute_with_adapter(FakeProviderAdapter::blocked("approval required"));
    assert_eq!(blocked.result().status(), "blocked");
    assert_eq!(blocked.result().value()["error"]["kind"], "fake_blocked");
    assert!(blocked.stderr_ref().is_some());
}

#[test]
fn fake_provider_refuses_to_overwrite_existing_output() {
    let project = temp_project();
    let store = open_store(&project);
    store
        .create_job("implement feature", "codex", vec![])
        .expect("create job");
    let registry = ProviderRegistryLoader::new(repo_root())
        .load_fake_default_registry()
        .expect("load fake registry");
    let request =
        ExecutionRequest::from_value(request_value("overwrite"), "request.json", schema_root())
            .expect("request");
    let schemas = schema_root();
    let context = ProviderRunContext::new(&registry, &store, &schemas);

    FakeProviderAdapter::success()
        .execute(&request, &context)
        .expect("first execute");
    let error = FakeProviderAdapter::success()
        .execute(&request, &context)
        .expect_err("second execute should fail");

    assert!(matches!(
        error,
        ProviderAdapterError::ProviderOutputAlreadyExists { .. }
    ));
    fs::remove_dir_all(project).ok();
}

#[test]
fn fake_provider_rejects_non_fake_instance() {
    let project = temp_project();
    let store = open_store(&project);
    store
        .create_job("implement feature", "codex", vec![])
        .expect("create job");
    let registry = ProviderRegistryLoader::new(repo_root())
        .load_registry(
            "configs/registries/builtin-provider-registry.yaml",
            &[PathBuf::from(
                "configs/provider-instances/codex-cli.example.yaml",
            )],
        )
        .expect("load builtin registry");
    let mut request = request_value("wrong provider");
    request["provider_instance_id"] = json!("my-codex-cli");
    let request =
        ExecutionRequest::from_value(request, "request.json", schema_root()).expect("request");
    let schemas = schema_root();
    let context = ProviderRunContext::new(&registry, &store, &schemas);

    let error = FakeProviderAdapter::success()
        .execute(&request, &context)
        .expect_err("non-fake instance should fail");
    assert!(matches!(
        error,
        ProviderAdapterError::UnsupportedProvider { .. }
    ));

    fs::remove_dir_all(project).ok();
}
