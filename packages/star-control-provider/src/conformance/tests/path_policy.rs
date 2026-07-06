use super::super::{check_provider_relative_path, ProviderConformanceError};

#[test]
fn provider_relative_path_accepts_canonical_provider_output() {
    check_provider_relative_path(
        "artifacts[]",
        "provider-output/cloud-default/response.json",
        "cloud-default",
    )
    .expect("canonical provider output path");
}

#[test]
fn provider_relative_path_rejects_unsafe_or_wrong_scope_paths() {
    for path in [
        "../response.json",
        "provider-output/cloud-default/../response.json",
        "provider-output/cloud-default\\response.json",
        "tool-output/cloud-default/response.json",
        "provider-output/other/response.json",
    ] {
        let error = check_provider_relative_path("artifacts[]", path, "cloud-default")
            .expect_err("unsafe provider artifact path should fail");
        assert!(matches!(
            error,
            ProviderConformanceError::InvalidArtifactPath { .. }
        ));
    }
}
