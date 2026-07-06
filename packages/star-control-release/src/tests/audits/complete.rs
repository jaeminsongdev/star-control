use super::helpers::{assert_not_ready_blockers, assert_reserved_readiness, release_writer};
use crate::test_support::all_complete_implementation_checks_passed;
use crate::{
    CompleteImplementationAuditBuilder, CompleteImplementationAuditCheck, ReleaseReadinessError,
    COMPLETE_IMPLEMENTATION_REQUIRED_CHECKS,
};

#[test]
fn complete_implementation_audit_builder_reserves_complete_audit() {
    let writer = release_writer();
    let readiness = CompleteImplementationAuditBuilder.build(
        &writer,
        "completion-audit-0001",
        "star-control",
        "m0-m9",
        all_complete_implementation_checks_passed(),
    );

    assert_reserved_readiness(
        &writer,
        &readiness,
        "schema-valid reserved complete implementation readiness",
        COMPLETE_IMPLEMENTATION_REQUIRED_CHECKS.len(),
        COMPLETE_IMPLEMENTATION_REQUIRED_CHECKS[0],
        "release/deploy/publish and external repository settings remain reserved until explicit approval",
    );
}

#[test]
fn complete_implementation_audit_builder_blocks_missing_failed_and_duplicate_checks() {
    let writer = release_writer();
    let mut checks = all_complete_implementation_checks_passed();
    checks.retain(|check| check.name() != "remote-ci-evidence");
    checks.push(
        CompleteImplementationAuditCheck::failed(
            "m6-cloud-provider",
            vec!["docs/implementation/cloud-provider-policy.md".to_string()],
            vec!["cloud API live transport remains approval-gated".to_string()],
        )
        .expect("failed complete implementation check"),
    );

    let readiness = CompleteImplementationAuditBuilder.build(
        &writer,
        "completion-audit-0002",
        "star-control",
        "m0-m9",
        checks,
    );

    assert_not_ready_blockers(
        &writer,
        &readiness,
        "schema-valid not_ready complete implementation readiness",
        &[
            "missing complete implementation check: remote-ci-evidence",
            "duplicate complete implementation check: m6-cloud-provider",
            "m6-cloud-provider: cloud API live transport remains approval-gated",
        ],
    );
}

#[test]
fn complete_implementation_check_rejects_unknown_or_unsafe_inputs() {
    let unknown_check = CompleteImplementationAuditCheck::passed("m10-extra", Vec::new())
        .expect_err("unknown completion check");
    assert!(matches!(
        unknown_check,
        ReleaseReadinessError::InvalidReleaseReadiness { .. }
    ));

    let unsafe_evidence = CompleteImplementationAuditCheck::passed(
        "m0-docs-decisions",
        vec!["../complete-implementation-roadmap.md".to_string()],
    )
    .expect_err("unsafe evidence");
    assert!(matches!(
        unsafe_evidence,
        ReleaseReadinessError::InvalidReleaseEvidence { .. }
    ));

    let empty_blocker = CompleteImplementationAuditCheck::failed(
        "stacked-prs-clean",
        Vec::new(),
        vec![" ".to_string()],
    )
    .expect_err("empty blocker");
    assert!(matches!(
        empty_blocker,
        ReleaseReadinessError::InvalidReleaseReadiness { .. }
    ));
}
