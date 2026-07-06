use super::super::super::test_support::{
    execute_cloud_api_live_approval, read_json, registry_with_instance, schema_root,
};
use crate::cloud_constants::{CLOUD_API_KIND, HTTP_TRANSPORT};
use crate::{ProviderConformanceChecker, ProviderConformanceProfile, ProviderRunContext};
use serde_json::{json, Value};
use star_control_state::StateStore;
use std::fs;

#[test]
fn cloud_api_live_transport_request_blocks_with_approval_artifacts() {
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
            "live_api_call_requested": true
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
    let (execution, project) =
        execute_cloud_api_live_approval(instance_value.clone()).expect("execute live approval");

    assert_eq!(execution.result().status(), "blocked");
    assert_eq!(
        execution.result().value()["error"]["kind"],
        "cloud_api_live_transport_approval_required"
    );
    assert_eq!(
        execution.result().value()["metrics"]["transport_execution"],
        "approval_required"
    );
    assert_eq!(
        execution.result().value()["metrics"]["live_api_call"],
        false
    );
    assert_eq!(
        execution.result().value()["artifacts"],
        json!([
            "provider-output/cloud-default/response.json",
            "provider-output/cloud-default/request.json",
            "provider-output/cloud-default/http-request.json",
            "provider-output/cloud-default/http-transport-plan.json",
            "provider-output/cloud-default/live-transport-approval.json",
            "provider-output/cloud-default/stdout.txt",
            "provider-output/cloud-default/stderr.txt",
            "provider-output/cloud-default/privacy-handoff.json",
            "provider-output/cloud-default/cost-metric.json"
        ])
    );
    assert!(!project
        .join(".ai-runs/J-0001/provider-output/cloud-default/raw-response.json")
        .exists());

    let http_request =
        read_json(&project.join(".ai-runs/J-0001/provider-output/cloud-default/http-request.json"));
    assert_eq!(http_request["method"], "POST");
    assert_eq!(http_request["url"], "https://api.openai.com/v1/responses");
    assert_eq!(http_request["body"]["model"], "gpt-example");
    let http_request_text = serde_json::to_string(&http_request).expect("serialize http request");
    assert!(!http_request_text.contains("STAR_CONTROL_TEST_TOKEN"));
    assert!(!http_request_text.contains("credential_ref"));

    let transport_plan = read_json(
        &project.join(".ai-runs/J-0001/provider-output/cloud-default/http-transport-plan.json"),
    );
    assert_eq!(transport_plan["execution_mode"], "live_approval_required");
    assert_eq!(transport_plan["credential"]["reference_kind"], "env");
    assert_eq!(transport_plan["credential"]["materialized"], false);
    assert_eq!(transport_plan["credential"]["value_present"], false);
    assert_eq!(transport_plan["raw_response_path"], Value::Null);
    assert_eq!(transport_plan["raw_response_expected"], false);
    assert_eq!(transport_plan["live_api_call"], false);
    assert_eq!(transport_plan["approval_required_for_live_call"], true);

    let approval = read_json(
        &project.join(".ai-runs/J-0001/provider-output/cloud-default/live-transport-approval.json"),
    );
    assert_eq!(
        approval["kind"],
        "cloud_api_live_transport_approval_required"
    );
    assert_eq!(approval["approval_required"], true);
    assert_eq!(
        approval["approval_required_actions"],
        json!([
            "credential_lookup",
            "authorization_header_value_construction",
            "live_http_request",
            "paid_api_call"
        ])
    );
    assert_eq!(approval["credential"]["reference_kind"], "env");
    assert_eq!(approval["credential"]["materialized"], false);
    assert_eq!(approval["live_api_call"], false);
    let approval_text = serde_json::to_string(&approval).expect("serialize approval");
    assert!(!approval_text.contains("STAR_CONTROL_TEST_TOKEN"));
    assert!(!approval_text.contains("env:STAR_CONTROL_TEST_TOKEN"));
    let transport_plan_text =
        serde_json::to_string(&transport_plan).expect("serialize transport plan");
    assert!(!transport_plan_text.contains("STAR_CONTROL_TEST_TOKEN"));
    assert!(!transport_plan_text.contains("env:STAR_CONTROL_TEST_TOKEN"));

    let schemas = schema_root();
    let store = StateStore::open(&project, &schemas).expect("open executed project");
    let registry = registry_with_instance(CLOUD_API_KIND, HTTP_TRANSPORT, instance_value)
        .expect("reload cloud API registry");
    let context = ProviderRunContext::new(&registry, &store, &schemas);
    let conformance = ProviderConformanceChecker
        .check_execution(&execution, &context, ProviderConformanceProfile::Cloud)
        .expect("cloud API live approval provider conformance");
    assert!(conformance
        .checked_artifacts()
        .contains(&"provider-output/cloud-default/live-transport-approval.json".to_string()));
    fs::remove_dir_all(project).ok();
}
