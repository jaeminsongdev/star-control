use super::super::test_support::{
    current_test_executable, execute_cloud_cli_transport, read_json, registry_with_instance,
    schema_root, EnvVarGuard,
};
use crate::cloud_constants::{CLI_TRANSPORT, CLOUD_CLI_KIND};
use crate::{ProviderConformanceChecker, ProviderConformanceProfile, ProviderRunContext};
use serde_json::{json, Value};
use star_control_state::StateStore;
use std::fs;

#[test]
fn cloud_cli_transport_executes_command_and_writes_contract() {
    let _env = EnvVarGuard::set("STAR_CONTROL_CLOUD_CLI_SUCCESS_HELPER", "1");
    let instance_value = json!({
        "id": "cloud-default",
        "provider": "provider.cloud",
        "enabled": true,
        "limits": {
            "timeout_seconds": 10,
            "max_parallel_jobs": 1
        },
        "routing_tags": ["cloud", "cli"],
        "transport_config": {
            "auth_mode": "login_session",
            "privacy_handoff_approved": true
        },
        "command_policy": {
            "shell": false,
            "env_allowlist": ["STAR_CONTROL_CLOUD_CLI_SUCCESS_HELPER"]
        },
        "command": {
            "executable": current_test_executable(),
            "args": [
                "--exact",
                "cloud::tests::cloud_cli_success_helper",
                "--nocapture"
            ]
        }
    });
    let (execution, project) =
        execute_cloud_cli_transport(instance_value.clone()).expect("execute cloud CLI transport");

    assert_eq!(execution.result().status(), "success");
    assert_eq!(execution.result().value()["error"], Value::Null);
    assert_eq!(
        execution.result().value()["artifacts"],
        json!([
            "provider-output/cloud-default/response.json",
            "provider-output/cloud-default/stdout.txt",
            "provider-output/cloud-default/stderr.txt",
            "provider-output/cloud-default/privacy-handoff.json",
            "provider-output/cloud-default/cost-metric.json"
        ])
    );
    let schemas = schema_root();
    let store = StateStore::open(&project, &schemas).expect("open executed project");
    let registry = registry_with_instance(CLOUD_CLI_KIND, CLI_TRANSPORT, instance_value)
        .expect("reload cloud registry");
    let context = ProviderRunContext::new(&registry, &store, &schemas);
    let conformance = ProviderConformanceChecker
        .check_execution(&execution, &context, ProviderConformanceProfile::Cloud)
        .expect("cloud CLI provider conformance");
    assert_eq!(conformance.provider_instance_id(), "cloud-default");
    assert!(conformance
        .checked_artifacts()
        .contains(&"provider-output/cloud-default/privacy-handoff.json".to_string()));
    assert!(conformance
        .checked_artifacts()
        .contains(&"provider-output/cloud-default/cost-metric.json".to_string()));
    let stdout = fs::read_to_string(
        project.join(".ai-runs/J-0001/provider-output/cloud-default/stdout.txt"),
    )
    .expect("read stdout");
    assert!(stdout.contains("cloud cli success"));
    let cost_metric =
        read_json(&project.join(".ai-runs/J-0001/provider-output/cloud-default/cost-metric.json"));
    assert_eq!(cost_metric["provider_instance_id"], "cloud-default");
    assert!(cost_metric["wall_time_ms"].as_u64().is_some());
    fs::remove_dir_all(project).ok();
}

#[test]
fn cloud_cli_transport_timeout_writes_timeout_result() {
    let _env = EnvVarGuard::set("STAR_CONTROL_CLOUD_CLI_SLEEP_HELPER", "1");
    let (execution, project) = execute_cloud_cli_transport(json!({
        "id": "cloud-default",
        "provider": "provider.cloud",
        "enabled": true,
        "limits": {
            "timeout_seconds": 1,
            "max_parallel_jobs": 1
        },
        "routing_tags": ["cloud", "cli"],
        "transport_config": {
            "auth_mode": "login_session",
            "privacy_handoff_approved": true
        },
        "command_policy": {
            "shell": false,
            "env_allowlist": ["STAR_CONTROL_CLOUD_CLI_SLEEP_HELPER"]
        },
        "command": {
            "executable": current_test_executable(),
            "args": [
                "--exact",
                "cloud::tests::cloud_cli_sleep_helper",
                "--nocapture"
            ]
        }
    }))
    .expect("execute cloud CLI timeout");

    assert_eq!(execution.result().status(), "timeout");
    assert_eq!(
        execution.result().value()["error"]["kind"],
        "cloud_cli_timeout"
    );
    fs::remove_dir_all(project).ok();
}

#[test]
fn cloud_cli_hard_budget_blocks_before_process_execution() {
    let _env = EnvVarGuard::set("STAR_CONTROL_CLOUD_CLI_SUCCESS_HELPER", "1");
    let (execution, project) = execute_cloud_cli_transport(json!({
        "id": "cloud-default",
        "provider": "provider.cloud",
        "enabled": true,
        "limits": {
            "timeout_seconds": 10,
            "max_parallel_jobs": 1
        },
        "routing_tags": ["cloud", "cli"],
        "transport_config": {
            "auth_mode": "login_session",
            "privacy_handoff_approved": true
        },
        "budget": {
            "estimated_cost": 0.25,
            "max_estimated_cost": 0.10,
            "currency": "USD"
        },
        "command_policy": {
            "shell": false,
            "env_allowlist": ["STAR_CONTROL_CLOUD_CLI_SUCCESS_HELPER"]
        },
        "command": {
            "executable": current_test_executable(),
            "args": [
                "--exact",
                "cloud::tests::cloud_cli_success_helper",
                "--nocapture"
            ]
        }
    }))
    .expect("execute cloud CLI budget block");

    assert_eq!(execution.result().status(), "blocked");
    assert_eq!(
        execution.result().value()["error"]["kind"],
        "cloud_budget_estimated_cost_exceeded"
    );
    assert_eq!(
        execution.result().value()["error"]["field"],
        "budget.max_estimated_cost"
    );
    let stdout = fs::read_to_string(
        project.join(".ai-runs/J-0001/provider-output/cloud-default/stdout.txt"),
    )
    .expect("read preflight stdout");
    assert!(stdout.contains("transport_execution=false"));
    assert!(!stdout.contains("cloud cli success"));
    let cost_metric =
        read_json(&project.join(".ai-runs/J-0001/provider-output/cloud-default/cost-metric.json"));
    assert_eq!(cost_metric["estimated_cost"], 0.25);
    fs::remove_dir_all(project).ok();
}
