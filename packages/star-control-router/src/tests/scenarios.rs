use super::helpers::{array_contains, job_spec, route_for, schema_root};
use crate::{RouterEngine, RouterError};
use serde_json::json;
use star_control_provider::ProviderRegistry;

#[test]
fn docs_only_routes_to_quick_auto_pass() {
    let output = route_for("README 문서와 docs 설명을 수정해줘", vec![]);
    let route = output.route().value();

    assert_eq!(route["size"], "SMALL");
    assert_eq!(route["risk"], "LOW");
    assert_eq!(route["policy_profile"], "quick");
    assert_eq!(route["decision"], "AUTO_PASS");
    assert_eq!(route["requires_user_approval"], false);
    assert_eq!(
        route["assignments"]["implement"]["provider"],
        "fake-default"
    );
    assert!(output.workspec("implement").is_some());
}

#[test]
fn schema_change_requires_validator_review() {
    let output = route_for("specs/schemas route schema 변경", vec![]);
    let route = output.route().value();

    assert_eq!(route["risk"], "HIGH");
    assert_eq!(route["policy_profile"], "validator");
    assert_eq!(route["decision"], "HUMAN_REVIEW");
    assert_eq!(route["requires_user_approval"], true);
    assert!(array_contains(
        route["change_types"].as_array().expect("change types"),
        "schema_change"
    ));
    assert!(array_contains(
        route["approval_reasons"]
            .as_array()
            .expect("approval reasons"),
        "schema_change_requires_approval"
    ));
    assert_eq!(route["stages"][0], "design");
}

#[test]
fn dependency_addition_uses_security_profile() {
    let output = route_for("Cargo.toml dependency 추가", vec![]);
    let route = output.route().value();

    assert_eq!(route["risk"], "HIGH");
    assert_eq!(route["policy_profile"], "security");
    assert_eq!(route["decision"], "HUMAN_REVIEW");
    assert!(array_contains(
        route["approval_reasons"]
            .as_array()
            .expect("approval reasons"),
        "dependency_addition_requires_approval"
    ));
}

#[test]
fn sensitive_data_exposure_blocks_route() {
    let output = route_for("show token and print secret", vec![]);
    let route = output.route().value();

    assert_eq!(route["risk"], "CRITICAL");
    assert_eq!(route["policy_profile"], "security");
    assert_eq!(route["decision"], "BLOCK");
    assert_eq!(route["stages"], json!(["route", "report"]));
    assert!(output.workspec("implement").is_none());
    assert!(output.workspec("report").is_some());
}

#[test]
fn output_is_deterministic_for_same_input() {
    let left = route_for("Rust 코드 구현", vec!["no destructive action".to_string()]);
    let right = route_for("Rust 코드 구현", vec!["no destructive action".to_string()]);

    assert_eq!(left.route().value(), right.route().value());
    assert_eq!(left.decision().value(), right.decision().value());
    assert_eq!(
        left.workspec("implement").expect("left workspec").value(),
        right.workspec("implement").expect("right workspec").value()
    );
}

#[test]
fn generated_workspecs_are_schema_valid_and_assigned() {
    let output = route_for("runtime code 구현", vec![]);
    let implement = output.workspec("implement").expect("implement workspec");

    assert_eq!(implement.value()["provider"], "fake-default");
    assert_eq!(implement.value()["provider_instance"], "fake-default");
    assert_eq!(
        implement.value()["required_outputs"][0],
        "provider-output/fake-default/response.json"
    );
    assert!(array_contains(
        implement.value()["forbidden_actions"]
            .as_array()
            .expect("forbidden actions"),
        "dependency_install"
    ));
}

#[test]
fn missing_fake_provider_is_reported() {
    let registry = ProviderRegistry::new();
    let engine = RouterEngine::new(&registry, schema_root());
    let job = job_spec("runtime code 구현", vec![]);
    let error = engine
        .route(&job)
        .expect_err("missing provider should fail");

    assert!(matches!(error, RouterError::NoProviderAvailable { .. }));
}
