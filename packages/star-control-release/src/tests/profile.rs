use crate::test_support::schema_root;
use crate::{
    ReleaseConsistencyChecker, ReleaseProfileReadinessBuilder, ReleaseProfileValidation,
    ReleaseReadinessError, ReleaseReadinessWriter,
};
use serde_json::json;

#[test]
fn release_profile_readiness_builder_reserves_status_after_all_checks_pass() {
    let writer = ReleaseReadinessWriter::new(schema_root());
    let profile = ReleaseProfileValidation::passed(
        "star-sentinel-release",
        vec![".ai-runs/J-0001/review-packs/release-profile.json".to_string()],
    )
    .expect("profile validation");
    let consistency = ReleaseConsistencyChecker::check(
        "1.2.3",
        "1.2.3",
        "## 1.2.3\n- release notes\n",
        "VERSION",
        "CHANGELOG.md",
    );

    let readiness = ReleaseProfileReadinessBuilder.build(
        &writer,
        "release-0005",
        "star-control",
        "1.2.3",
        profile,
        consistency,
    );

    writer
        .validate_readiness(&readiness)
        .expect("schema-valid reserved readiness");
    assert_eq!(readiness["status"], "reserved");
    assert_eq!(readiness["checks"][0]["name"], "release-profile-passed");
    assert_eq!(readiness["checks"][0]["status"], "pass");
    assert!(readiness["blockers"]
        .as_array()
        .expect("blockers")
        .contains(&json!(
            "release approval/signing/publish/deploy automation remains reserved"
        )));
}

#[test]
fn release_profile_readiness_builder_blocks_profile_and_consistency_failures() {
    let writer = ReleaseReadinessWriter::new(schema_root());
    let profile = ReleaseProfileValidation::failed(
        "star-sentinel-release",
        vec![".ai-runs/J-0001/tool-output/star-sentinel/gate.json".to_string()],
        vec!["release profile blocked unresolved BLOCK diagnostic".to_string()],
    )
    .expect("profile validation");
    let consistency = ReleaseConsistencyChecker::check(
        "1.2.3",
        "1.2.2",
        "## 1.2.2\n- previous release\n",
        "VERSION",
        "CHANGELOG.md",
    );

    let readiness = ReleaseProfileReadinessBuilder.build(
        &writer,
        "release-0006",
        "star-control",
        "1.2.3",
        profile,
        consistency,
    );

    writer
        .validate_readiness(&readiness)
        .expect("schema-valid not_ready readiness");
    assert_eq!(readiness["status"], "not_ready");
    assert_eq!(readiness["checks"][0]["status"], "fail");
    let blockers = readiness["blockers"].as_array().expect("blockers");
    assert!(blockers.contains(&json!(
        "release profile blocked unresolved BLOCK diagnostic"
    )));
    assert!(blockers.contains(&json!("version mismatch: expected 1.2.3, found 1.2.2")));
    assert!(blockers.contains(&json!("changelog does not mention version 1.2.3")));
}

#[test]
fn release_profile_validation_rejects_unsafe_evidence_and_empty_failure() {
    let unsafe_error = ReleaseProfileValidation::passed(
        "star-sentinel-release",
        vec!["../release-profile.json".to_string()],
    )
    .expect_err("unsafe release profile evidence");
    assert!(matches!(
        unsafe_error,
        ReleaseReadinessError::InvalidReleaseEvidence { .. }
    ));

    let empty_blocker_error = ReleaseProfileValidation::failed(
        "star-sentinel-release",
        Vec::new(),
        vec![" ".to_string()],
    )
    .expect_err("empty blocker");
    assert!(matches!(
        empty_blocker_error,
        ReleaseReadinessError::InvalidReleaseReadiness { .. }
    ));

    let empty_profile_error =
        ReleaseProfileValidation::passed(" ", Vec::new()).expect_err("empty profile name");
    assert!(matches!(
        empty_profile_error,
        ReleaseReadinessError::InvalidReleaseReadiness { .. }
    ));
}
