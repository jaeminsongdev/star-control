use super::super::super::test_support::execute_cloud_api_offline;
use crate::ProviderAdapterError::CommandPolicyDenied;
use serde_json::json;

#[test]
fn cloud_api_offline_fixture_rejects_unsafe_fixture_path() {
    let error = execute_cloud_api_offline(
        json!({
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
                "offline_response_fixture": "../outside.json"
            },
            "endpoint": {
                "base_url": "https://api.openai.com/v1",
                "model": "gpt-example"
            }
        }),
        "fixtures/openai-response.json",
        &json!({
            "id": "resp_fixture",
            "output_text": "unused"
        }),
    )
    .expect_err("unsafe fixture path should fail");

    assert!(matches!(
        error,
        CommandPolicyDenied { reason, .. }
            if reason.contains("must not traverse outside the project")
    ));
}
