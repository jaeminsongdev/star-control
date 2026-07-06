use super::super::test_support::{execute_cloud_provider, read_json};
use crate::cloud_constants::{CLI_TRANSPORT, CLOUD_API_KIND, CLOUD_CLI_KIND, HTTP_TRANSPORT};
use serde_json::json;
use std::fs;

#[test]
fn cloud_cli_preflight_writes_privacy_and_cost_artifacts() {
    let (execution, project) = execute_cloud_provider(
        CLOUD_CLI_KIND,
        CLI_TRANSPORT,
        json!({
            "id": "cloud-default",
            "provider": "provider.cloud",
            "enabled": true,
            "limits": {
                "timeout_seconds": 300,
                "max_parallel_jobs": 1
            },
            "routing_tags": ["cloud", "cli"],
            "transport_config": {
                "auth_mode": "login_session",
                "privacy_handoff_approved": true
            },
            "budget": {
                "estimated_cost": 0.25,
                "currency": "USD"
            },
            "command": {
                "executable": "cloud-agent"
            }
        }),
    )
    .expect("execute cloud preflight");

    assert_eq!(execution.result().status(), "blocked");
    assert_eq!(
        execution.result().value()["error"]["kind"],
        "cloud_provider_transport_not_implemented"
    );
    assert_eq!(
        execution.result().value()["metrics"]["privacy_handoff_approved"],
        true
    );
    assert!(project
        .join(".ai-runs/J-0001/provider-output/cloud-default/privacy-handoff.json")
        .is_file());
    let cost_metric =
        read_json(&project.join(".ai-runs/J-0001/provider-output/cloud-default/cost-metric.json"));
    assert_eq!(cost_metric["estimated_cost"], 0.25);
    fs::remove_dir_all(project).ok();
}

#[test]
fn cloud_api_preflight_requires_credential_ref() {
    let (execution, project) = execute_cloud_provider(
        CLOUD_API_KIND,
        HTTP_TRANSPORT,
        json!({
            "id": "cloud-default",
            "provider": "provider.cloud",
            "enabled": true,
            "limits": {
                "timeout_seconds": 300,
                "max_parallel_jobs": 1
            },
            "routing_tags": ["cloud", "api"],
            "transport_config": {
                "privacy_handoff_approved": true
            },
            "endpoint": {
                "base_url": "https://api.example.invalid/v1"
            }
        }),
    )
    .expect("execute cloud preflight");

    assert_eq!(execution.result().status(), "blocked");
    assert_eq!(
        execution.result().value()["error"]["kind"],
        "cloud_api_credential_ref_required"
    );
    fs::remove_dir_all(project).ok();
}

#[test]
fn cloud_preflight_blocks_raw_credential_without_echoing_value() {
    let raw_secret = "sk-raw-secret-value";
    let (execution, project) = execute_cloud_provider(
        CLOUD_API_KIND,
        HTTP_TRANSPORT,
        json!({
            "id": "cloud-default",
            "provider": "provider.cloud",
            "enabled": true,
            "credential_ref": "env:STAR_CONTROL_TEST_TOKEN",
            "api_key": raw_secret,
            "limits": {
                "timeout_seconds": 300,
                "max_parallel_jobs": 1
            },
            "routing_tags": ["cloud", "api"],
            "transport_config": {
                "privacy_handoff_approved": true
            }
        }),
    )
    .expect("execute cloud preflight");

    assert_eq!(
        execution.result().value()["error"]["kind"],
        "cloud_provider_raw_credential"
    );
    let response_text =
        serde_json::to_string(execution.result().value()).expect("serialize response");
    assert!(!response_text.contains(raw_secret));
    fs::remove_dir_all(project).ok();
}
