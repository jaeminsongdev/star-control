use super::helpers::{assert_not_ready_blockers, assert_reserved_readiness, release_writer};
use crate::test_support::all_m9_readiness_checks_passed;
use crate::{
    M9ReadinessAuditBuilder, M9ReadinessCheck, ReleaseReadinessError, M9_REQUIRED_READINESS_CHECKS,
};

#[test]
fn m9_readiness_audit_builder_reserves_complete_audit() {
    let writer = release_writer();
    let readiness = M9ReadinessAuditBuilder.build(
        &writer,
        "m9-audit-0001",
        "star-control",
        "m9",
        all_m9_readiness_checks_passed(),
    );

    assert_reserved_readiness(
        &writer,
        &readiness,
        "schema-valid reserved M9 readiness",
        M9_REQUIRED_READINESS_CHECKS.len(),
        M9_REQUIRED_READINESS_CHECKS[0],
        "final release/deploy/publish remains reserved until explicit approval",
    );
}

#[test]
fn m9_readiness_audit_builder_blocks_missing_failed_and_duplicate_checks() {
    let writer = release_writer();
    let mut checks = all_m9_readiness_checks_passed();
    checks.retain(|check| check.name() != "release-automation-reserved");
    checks.push(
        M9ReadinessCheck::failed(
            "cost-budget-guard",
            vec!["docs/implementation/briefs/E28-cost-metric-budget-guard.md".to_string()],
            vec!["cost budget acceptance evidence is missing".to_string()],
        )
        .expect("failed M9 check"),
    );

    let readiness =
        M9ReadinessAuditBuilder.build(&writer, "m9-audit-0002", "star-control", "m9", checks);

    assert_not_ready_blockers(
        &writer,
        &readiness,
        "schema-valid not_ready M9 readiness",
        &[
            "missing M9 readiness check: release-automation-reserved",
            "duplicate M9 readiness check: cost-budget-guard",
            "cost-budget-guard: cost budget acceptance evidence is missing",
        ],
    );
}

#[test]
fn m9_readiness_check_rejects_unknown_or_unsafe_inputs() {
    let unknown_check =
        M9ReadinessCheck::passed("unknown-check", Vec::new()).expect_err("unknown check");
    assert!(matches!(
        unknown_check,
        ReleaseReadinessError::InvalidReleaseReadiness { .. }
    ));

    let unsafe_evidence = M9ReadinessCheck::passed(
        "security-redaction",
        vec!["../security-redaction.json".to_string()],
    )
    .expect_err("unsafe evidence");
    assert!(matches!(
        unsafe_evidence,
        ReleaseReadinessError::InvalidReleaseEvidence { .. }
    ));

    let empty_blocker =
        M9ReadinessCheck::failed("cost-budget-guard", Vec::new(), vec![" ".to_string()])
            .expect_err("empty blocker");
    assert!(matches!(
        empty_blocker,
        ReleaseReadinessError::InvalidReleaseReadiness { .. }
    ));
}
