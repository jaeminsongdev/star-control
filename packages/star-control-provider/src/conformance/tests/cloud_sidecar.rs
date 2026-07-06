use super::super::{
    ProviderConformanceChecker, ProviderConformanceError, ProviderConformanceProfile,
    COST_METRIC_SCHEMA,
};
use super::fixture::{execution_from_value, result_value, Fixture};
use serde_json::json;

#[test]
fn cloud_conformance_validates_sidecar_schema() {
    let fixture = Fixture::new("invalid-cloud-sidecar");
    let response = result_value(
        "cloud-default",
        "success",
        vec![
            "provider-output/cloud-default/response.json",
            "provider-output/cloud-default/stdout.txt",
            "provider-output/cloud-default/privacy-handoff.json",
            "provider-output/cloud-default/cost-metric.json",
        ],
    );
    fixture.write_provider_text("request.json", "{}");
    fixture.write_provider_text("stdout.txt", "ok\n");
    fixture.write_provider_json("response.json", &response);
    fixture.write_provider_json(
        "privacy-handoff.json",
        &json!({
            "schema_version": "1.0.0",
            "job_id": "J-0001",
            "destination": "cloud-default",
            "context_paths": ["workspecs/implement.json"],
            "redaction_required": true,
            "approved": true
        }),
    );
    fixture.write_provider_json(
        "cost-metric.json",
        &json!({
            "schema_version": "1.0.0",
            "job_id": "J-0001",
            "stage": "implement",
            "provider_instance_id": "cloud-default",
            "estimated_cost": 0,
            "currency": "USD"
        }),
    );

    let execution = execution_from_value(response);
    let error = ProviderConformanceChecker
        .check_execution(
            &execution,
            &fixture.context(),
            ProviderConformanceProfile::Cloud,
        )
        .expect_err("schema-invalid cost sidecar should fail");

    assert!(matches!(
        error,
        ProviderConformanceError::SchemaValidationFailed { schema_path, .. }
            if schema_path.ends_with(COST_METRIC_SCHEMA)
    ));
}
