use super::super::{
    ProviderConformanceChecker, ProviderConformanceError, ProviderConformanceProfile,
};
use super::fixture::{execution_from_value, result_value, Fixture};
use serde_json::json;

#[test]
fn conformance_rejects_unsafe_provider_instance_id() {
    let fixture = Fixture::new("unsafe-provider-id");
    let response = result_value(
        "bad/segment",
        "success",
        vec!["provider-output/bad/segment/response.json"],
    );
    let execution = execution_from_value(response);

    let error = ProviderConformanceChecker
        .check_execution(
            &execution,
            &fixture.context(),
            ProviderConformanceProfile::Basic,
        )
        .expect_err("unsafe provider instance id should fail");

    assert!(matches!(
        error,
        ProviderConformanceError::InvalidArtifactPath { field, .. } if field == "provider_instance_id"
    ));
}

#[test]
fn conformance_rejects_stored_response_mismatch() {
    let fixture = Fixture::new("response-mismatch");
    let response = result_value(
        "cloud-default",
        "success",
        vec![
            "provider-output/cloud-default/response.json",
            "provider-output/cloud-default/stdout.txt",
        ],
    );
    fixture.write_provider_text("request.json", "{}");
    fixture.write_provider_text("stdout.txt", "ok\n");
    let mut stored_response = response.clone();
    stored_response["status"] = json!("failed");
    fixture.write_provider_json("response.json", &stored_response);

    let execution = execution_from_value(response);
    let error = ProviderConformanceChecker
        .check_execution(
            &execution,
            &fixture.context(),
            ProviderConformanceProfile::Basic,
        )
        .expect_err("stored response mismatch should fail");

    assert!(matches!(
        error,
        ProviderConformanceError::FieldMismatch { field, .. } if field == "provider-output response.json"
    ));
}
