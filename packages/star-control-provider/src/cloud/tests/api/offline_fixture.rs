use super::super::super::test_support::{
    execute_cloud_api_offline, read_json, registry_with_instance, schema_root,
};
use crate::cloud_constants::{CLOUD_API_KIND, HTTP_TRANSPORT};
use crate::{ProviderConformanceChecker, ProviderConformanceProfile, ProviderRunContext};
use serde_json::json;
use star_control_state::StateStore;
use std::fs;

#[test]
fn cloud_api_offline_fixture_builds_request_and_parses_response_contract() {
    let fixture_relative_path = "fixtures/openai-response.json";
    let instance_value = json!({
        "id": "cloud-default",
        "provider": "provider.cloud",
        "enabled": true,
        "credential_ref": "env:STAR_CONTROL_TEST_TOKEN",
        "limits": {
            "timeout_seconds": 300,
            "max_parallel_jobs": 1
        },
        "routing_tags": ["cloud", "api"],
        "transport_config": {
            "privacy_handoff_approved": true,
            "offline_response_fixture": fixture_relative_path
        },
        "budget": {
            "estimated_cost": 0.03,
            "currency": "USD"
        },
        "endpoint": {
            "base_url": "https://api.openai.com/v1/",
            "model": "gpt-example"
        }
    });
    let fixture_value = json!({
        "id": "resp_fixture",
        "model": "gpt-example",
        "status": "completed",
        "output_text": "offline fixture answer",
        "usage": {
            "input_tokens": 5,
            "output_tokens": 7,
            "total_tokens": 12
        }
    });
    let (execution, project) = execute_cloud_api_offline(
        instance_value.clone(),
        fixture_relative_path,
        &fixture_value,
    )
    .expect("execute cloud API offline fixture");

    assert_eq!(execution.result().status(), "success");
    assert_eq!(
        execution.result().value()["summary"],
        "offline fixture answer"
    );
    assert_eq!(
        execution.result().value()["metrics"]["transport_execution"],
        "offline_fixture"
    );
    assert_eq!(execution.result().value()["metrics"]["input_tokens"], 5);
    assert_eq!(execution.result().value()["metrics"]["output_tokens"], 7);
    assert_eq!(execution.result().value()["metrics"]["total_tokens"], 12);
    assert_eq!(
        execution.result().value()["artifacts"],
        json!([
            "provider-output/cloud-default/response.json",
            "provider-output/cloud-default/request.json",
            "provider-output/cloud-default/http-request.json",
            "provider-output/cloud-default/http-transport-plan.json",
            "provider-output/cloud-default/raw-response.json",
            "provider-output/cloud-default/stdout.txt",
            "provider-output/cloud-default/stderr.txt",
            "provider-output/cloud-default/privacy-handoff.json",
            "provider-output/cloud-default/cost-metric.json"
        ])
    );

    let http_request =
        read_json(&project.join(".ai-runs/J-0001/provider-output/cloud-default/http-request.json"));
    assert_eq!(http_request["method"], "POST");
    assert_eq!(http_request["url"], "https://api.openai.com/v1/responses");
    assert_eq!(http_request["body"]["model"], "gpt-example");
    assert_eq!(http_request["body"]["input"], "run cloud provider");
    let http_request_text = serde_json::to_string(&http_request).expect("serialize http request");
    assert!(!http_request_text.contains("STAR_CONTROL_TEST_TOKEN"));
    assert!(!http_request_text.contains("credential_ref"));

    let transport_plan = read_json(
        &project.join(".ai-runs/J-0001/provider-output/cloud-default/http-transport-plan.json"),
    );
    assert_eq!(transport_plan["method"], "POST");
    assert_eq!(transport_plan["url"], "https://api.openai.com/v1/responses");
    assert_eq!(transport_plan["execution_mode"], "offline_fixture");
    assert_eq!(transport_plan["credential"]["reference_kind"], "env");
    assert_eq!(transport_plan["credential"]["materialized"], false);
    assert_eq!(transport_plan["credential"]["value_present"], false);
    assert_eq!(transport_plan["live_api_call"], false);
    assert_eq!(transport_plan["approval_required_for_live_call"], true);
    assert_eq!(
        transport_plan["header_policy"][1]["value_policy"],
        "deferred_credential_reference"
    );
    let transport_plan_text =
        serde_json::to_string(&transport_plan).expect("serialize transport plan");
    assert!(!transport_plan_text.contains("STAR_CONTROL_TEST_TOKEN"));
    assert!(!transport_plan_text.contains("env:STAR_CONTROL_TEST_TOKEN"));

    let raw_response =
        read_json(&project.join(".ai-runs/J-0001/provider-output/cloud-default/raw-response.json"));
    assert_eq!(raw_response, fixture_value);
    let cost_metric =
        read_json(&project.join(".ai-runs/J-0001/provider-output/cloud-default/cost-metric.json"));
    assert_eq!(cost_metric["input_tokens"], 5);
    assert_eq!(cost_metric["output_tokens"], 7);

    let schemas = schema_root();
    let store = StateStore::open(&project, &schemas).expect("open executed project");
    let registry = registry_with_instance(CLOUD_API_KIND, HTTP_TRANSPORT, instance_value)
        .expect("reload cloud API registry");
    let context = ProviderRunContext::new(&registry, &store, &schemas);
    let conformance = ProviderConformanceChecker
        .check_execution(&execution, &context, ProviderConformanceProfile::Cloud)
        .expect("cloud API offline provider conformance");
    assert!(conformance
        .checked_artifacts()
        .contains(&"provider-output/cloud-default/http-request.json".to_string()));
    assert!(conformance
        .checked_artifacts()
        .contains(&"provider-output/cloud-default/http-transport-plan.json".to_string()));
    assert!(conformance
        .checked_artifacts()
        .contains(&"provider-output/cloud-default/raw-response.json".to_string()));
    fs::remove_dir_all(project).ok();
}

#[test]
fn cloud_api_hard_budget_blocks_before_offline_fixture_processing() {
    let fixture_relative_path = "fixtures/openai-response.json";
    let instance_value = json!({
        "id": "cloud-default",
        "provider": "provider.cloud",
        "enabled": true,
        "credential_ref": "env:STAR_CONTROL_TEST_TOKEN",
        "limits": {
            "timeout_seconds": 300,
            "max_parallel_jobs": 1
        },
        "routing_tags": ["cloud", "api"],
        "transport_config": {
            "privacy_handoff_approved": true,
            "offline_response_fixture": fixture_relative_path
        },
        "budget": {
            "estimated_cost": 0.50,
            "max_estimated_cost": 0.10,
            "currency": "USD"
        },
        "endpoint": {
            "base_url": "https://api.openai.com/v1/",
            "model": "gpt-example"
        }
    });
    let fixture_value = json!({
        "id": "resp_fixture",
        "model": "gpt-example",
        "status": "completed",
        "output_text": "offline fixture answer",
        "usage": {
            "input_tokens": 5,
            "output_tokens": 7,
            "total_tokens": 12
        }
    });
    let (execution, project) =
        execute_cloud_api_offline(instance_value, fixture_relative_path, &fixture_value)
            .expect("execute cloud API hard budget block");

    assert_eq!(execution.result().status(), "blocked");
    assert_eq!(
        execution.result().value()["error"]["kind"],
        "cloud_budget_estimated_cost_exceeded"
    );
    assert!(!project
        .join(".ai-runs/J-0001/provider-output/cloud-default/http-request.json")
        .exists());
    assert!(!project
        .join(".ai-runs/J-0001/provider-output/cloud-default/raw-response.json")
        .exists());
    let cost_metric =
        read_json(&project.join(".ai-runs/J-0001/provider-output/cloud-default/cost-metric.json"));
    assert_eq!(cost_metric["estimated_cost"], 0.50);
    fs::remove_dir_all(project).ok();
}
