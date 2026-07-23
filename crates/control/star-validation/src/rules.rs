//! Pure M3 B01-B07 rule-family evaluation.
//!
//! Adapters observe source and tool facts. This module only maps typed facts to
//! stable, redacted diagnostics and a minimum Gate decision.

use serde::{Deserialize, Serialize};
use star_contracts::{
    Sha256Hash,
    evidence::{DiagnosticConfidence, DiagnosticSeverity, DiagnosticStatus},
    management::ProjectPathRef,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleFamilyV2 {
    B01ChangeScopeClaim,
    B02TestTrust,
    B03ValidatorSelfProtection,
    B04ArchitectureContractDrift,
    B05SecuritySupplyChain,
    B06Regression,
    B07DocsConfigEnvironment,
}

impl RuleFamilyV2 {
    pub const fn code(self) -> &'static str {
        match self {
            Self::B01ChangeScopeClaim => "B01",
            Self::B02TestTrust => "B02",
            Self::B03ValidatorSelfProtection => "B03",
            Self::B04ArchitectureContractDrift => "B04",
            Self::B05SecuritySupplyChain => "B05",
            Self::B06Regression => "B06",
            Self::B07DocsConfigEnvironment => "B07",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleDecisionFloorV2 {
    None,
    HumanReview,
    Block,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
pub enum RuleFactV2 {
    ActualChangeCollectionIncomplete,
    ActualChangeUnrelated { path: ProjectPathRef },
    ActualChangeScopeUnknown { path: ProjectPathRef },
    CompletionClaimMismatch { claim_id: String },
    RequiredTestDeleted { path: ProjectPathRef },
    AssertionCountDecreased { path: ProjectPathRef },
    TestExecutionBypassAdded { path: ProjectPathRef },
    FocusedTestOnlyAdded { path: ProjectPathRef },
    RetryOrTimeoutIncreased { path: ProjectPathRef },
    RelatedTestCheckMissing,
    ProtectedValidationSurfaceChanged,
    ValidatorSnapshotMissing,
    ValidatorFixtureCoverageMissing { missing: Vec<String> },
    ValidatorBehaviorWeakened,
    ValidatorSelfApprovalOnly,
    ArchitectureCheckMissing,
    ContractCheckMissing,
    ManagedRegistrySnapshotMissing,
    ManagedRegistrySnapshotStale,
    ManagedRegistryConsistencyDrift { subject: String },
    ManagedRegistryValidationMissing { family: String },
    GeneratedOutputDrift { path: ProjectPathRef },
    HardcodingCheckMissing,
    SecretCandidate { path: ProjectPathRef },
    DangerousCommandCandidate { path: ProjectPathRef },
    DependencyEvidenceMissing { path: ProjectPathRef },
    RegressionEvidenceMissing,
    RegressionCheckMissing,
    DocsCheckMissing,
    ConfigCheckMissing,
    DocumentationDrift,
    EnvironmentEvidenceMissing,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleDiagnosticInputV2 {
    pub family: RuleFamilyV2,
    pub rule_id: String,
    pub code: String,
    pub title: String,
    pub message: String,
    pub severity: DiagnosticSeverity,
    pub confidence: DiagnosticConfidence,
    pub status: DiagnosticStatus,
    pub decision_floor: RuleDecisionFloorV2,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleFixtureResultV2 {
    pub fixture_kind: String,
    pub previous_snapshot_passed: bool,
    pub current_snapshot_passed: bool,
    pub result_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TwoSnapshotGuardInputV2 {
    pub protected_surface_changed: bool,
    pub previous_snapshot_fingerprint: Option<Sha256Hash>,
    pub current_snapshot_fingerprint: Option<Sha256Hash>,
    pub behavior_weakened: bool,
    pub independent_previous_executor: bool,
    pub fixtures: Vec<RuleFixtureResultV2>,
}

pub fn evaluate_two_snapshot_guard(input: &TwoSnapshotGuardInputV2) -> Vec<RuleFactV2> {
    if !input.protected_surface_changed {
        return Vec::new();
    }
    let mut facts = Vec::new();
    if input.previous_snapshot_fingerprint.is_none()
        || input.current_snapshot_fingerprint.is_none()
        || input.previous_snapshot_fingerprint == input.current_snapshot_fingerprint
    {
        facts.push(RuleFactV2::ValidatorSnapshotMissing);
    }
    let required = ["positive", "negative", "edge", "regression"];
    let missing = required
        .into_iter()
        .filter(|kind| {
            !input.fixtures.iter().any(|fixture| {
                fixture.fixture_kind == *kind
                    && fixture.previous_snapshot_passed
                    && fixture.current_snapshot_passed
            })
        })
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        facts.push(RuleFactV2::ValidatorFixtureCoverageMissing { missing });
    }
    if input.behavior_weakened {
        facts.push(RuleFactV2::ValidatorBehaviorWeakened);
    }
    if !input.independent_previous_executor {
        facts.push(RuleFactV2::ValidatorSelfApprovalOnly);
    }
    facts
}

pub fn evaluate_rule_facts(facts: &[RuleFactV2]) -> Vec<RuleDiagnosticInputV2> {
    let mut diagnostics = facts.iter().map(map_fact).collect::<Vec<_>>();
    diagnostics.sort_by(|left, right| {
        (left.family, &left.rule_id, &left.code).cmp(&(right.family, &right.rule_id, &right.code))
    });
    diagnostics.dedup();
    diagnostics
}

fn map_fact(fact: &RuleFactV2) -> RuleDiagnosticInputV2 {
    use RuleDecisionFloorV2::{Block, HumanReview};
    use RuleFactV2::*;
    match fact {
        ActualChangeCollectionIncomplete => diagnostic(
            RuleFamilyV2::B01ChangeScopeClaim,
            "star.validation.change.collection-incomplete",
            "ACTUAL_CHANGE_COLLECTION_INCOMPLETE",
            "Actual change collection is incomplete",
            "The actual ChangeSet cannot be compared with the accepted task and scope.",
            DiagnosticSeverity::Error,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            Block,
        ),
        ActualChangeUnrelated { path } => path_diagnostic(
            RuleFamilyV2::B01ChangeScopeClaim,
            "star.validation.change.out-of-scope",
            "ACTUAL_CHANGE_OUT_OF_SCOPE",
            "Actual change is unrelated to the accepted scope",
            path,
            DiagnosticSeverity::Error,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            Block,
        ),
        ActualChangeScopeUnknown { path } => path_diagnostic(
            RuleFamilyV2::B01ChangeScopeClaim,
            "star.validation.change.scope-unknown",
            "ACTUAL_CHANGE_SCOPE_UNKNOWN",
            "Actual change scope could not be proven",
            path,
            DiagnosticSeverity::Warning,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            HumanReview,
        ),
        CompletionClaimMismatch { .. } => diagnostic(
            RuleFamilyV2::B01ChangeScopeClaim,
            "star.validation.claim.mismatch",
            "COMPLETION_CLAIM_MISMATCH",
            "Completion claim does not match the actual ChangeSet",
            "A reported completion claim omits or conflicts with observed changes or required criteria.",
            DiagnosticSeverity::Error,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            Block,
        ),
        RequiredTestDeleted { path } => path_diagnostic(
            RuleFamilyV2::B02TestTrust,
            "star.validation.test.case-deleted",
            "REQUIRED_TEST_DELETED",
            "A required test surface was deleted",
            path,
            DiagnosticSeverity::Error,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            Block,
        ),
        AssertionCountDecreased { path } => path_diagnostic(
            RuleFamilyV2::B02TestTrust,
            "star.validation.test.assertion-weakened",
            "TEST_ASSERTION_COUNT_DECREASED",
            "Test assertions may have been weakened",
            path,
            DiagnosticSeverity::Warning,
            DiagnosticConfidence::Medium,
            DiagnosticStatus::Suspected,
            HumanReview,
        ),
        TestExecutionBypassAdded { path } => path_diagnostic(
            RuleFamilyV2::B02TestTrust,
            "star.validation.test.execution-bypassed",
            "TEST_EXECUTION_BYPASS_ADDED",
            "A test execution bypass was added",
            path,
            DiagnosticSeverity::Error,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            Block,
        ),
        FocusedTestOnlyAdded { path } => path_diagnostic(
            RuleFamilyV2::B02TestTrust,
            "star.validation.test.focused-only",
            "FOCUSED_TEST_ONLY_ADDED",
            "A focused-only test marker was added",
            path,
            DiagnosticSeverity::Critical,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            Block,
        ),
        RetryOrTimeoutIncreased { path } => path_diagnostic(
            RuleFamilyV2::B02TestTrust,
            "star.validation.test.failure-masked",
            "TEST_RETRY_OR_TIMEOUT_INCREASED",
            "Test failure may be masked by retry or timeout changes",
            path,
            DiagnosticSeverity::Warning,
            DiagnosticConfidence::Medium,
            DiagnosticStatus::Suspected,
            HumanReview,
        ),
        RelatedTestCheckMissing => diagnostic(
            RuleFamilyV2::B02TestTrust,
            "star.validation.test.related-check-unresolved",
            "RELATED_TEST_CHECK_MISSING",
            "Related test validation is missing",
            "Changed production or test sources are not covered by a required test CheckPlan.",
            DiagnosticSeverity::Error,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            Block,
        ),
        ProtectedValidationSurfaceChanged => diagnostic(
            RuleFamilyV2::B03ValidatorSelfProtection,
            "star.validation.guard.protected-surface-changed",
            "PROTECTED_VALIDATION_SURFACE_CHANGED",
            "Protected validation behavior changed",
            "The validator, policy, Rule, normalizer, test harness, or evidence contract changed.",
            DiagnosticSeverity::Warning,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            HumanReview,
        ),
        ValidatorSnapshotMissing => diagnostic(
            RuleFamilyV2::B03ValidatorSelfProtection,
            "star.validation.guard.snapshot-missing",
            "VALIDATOR_TWO_SNAPSHOT_MISSING",
            "Validator two-snapshot evidence is missing",
            "Both exact previous and current validator snapshots are required.",
            DiagnosticSeverity::Error,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            Block,
        ),
        ValidatorFixtureCoverageMissing { .. } => diagnostic(
            RuleFamilyV2::B03ValidatorSelfProtection,
            "star.validation.guard.fixture-missing",
            "VALIDATOR_FIXTURE_COVERAGE_MISSING",
            "Required validator fixture coverage is missing",
            "Positive, negative, edge, and regression fixtures must pass on both snapshots.",
            DiagnosticSeverity::Error,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            Block,
        ),
        ValidatorBehaviorWeakened => diagnostic(
            RuleFamilyV2::B03ValidatorSelfProtection,
            "star.validation.guard.normalization-weakened",
            "VALIDATOR_BEHAVIOR_WEAKENED",
            "Validation behavior was weakened",
            "The current snapshot accepts or downgrades a protected negative fixture.",
            DiagnosticSeverity::Critical,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            Block,
        ),
        ValidatorSelfApprovalOnly => diagnostic(
            RuleFamilyV2::B03ValidatorSelfProtection,
            "star.validation.guard.self-approval",
            "VALIDATOR_SELF_APPROVAL_ONLY",
            "Changed validator cannot approve itself",
            "Previous-snapshot execution must be independent from the changed validator.",
            DiagnosticSeverity::Critical,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            Block,
        ),
        ArchitectureCheckMissing => missing_check(
            RuleFamilyV2::B04ArchitectureContractDrift,
            "architecture",
            "ARCHITECTURE_CHECK_MISSING",
        ),
        ContractCheckMissing => missing_check(
            RuleFamilyV2::B04ArchitectureContractDrift,
            "contract",
            "CONTRACT_CHECK_MISSING",
        ),
        ManagedRegistrySnapshotMissing => diagnostic(
            RuleFamilyV2::B04ArchitectureContractDrift,
            "star.validation.managed-registry.snapshot-missing",
            "MANAGED_REGISTRY_SNAPSHOT_MISSING",
            "Managed registry evidence is missing",
            "A managed declaration change requires a current source-derived registry snapshot.",
            DiagnosticSeverity::Error,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            Block,
        ),
        ManagedRegistrySnapshotStale => diagnostic(
            RuleFamilyV2::B04ArchitectureContractDrift,
            "star.validation.managed-registry.snapshot-stale",
            "MANAGED_REGISTRY_SNAPSHOT_STALE",
            "Managed registry evidence is stale or partial",
            "The registry snapshot must match the pinned Project revision, workspace, and CodeIndex snapshot.",
            DiagnosticSeverity::Error,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            Block,
        ),
        ManagedRegistryConsistencyDrift { subject } => diagnostic(
            RuleFamilyV2::B04ArchitectureContractDrift,
            "star.validation.managed-registry.consistency-drift",
            "MANAGED_REGISTRY_CONSISTENCY_DRIFT",
            "Managed registry binding or consumer drift was detected",
            &format!(
                "The source-derived registry consistency record is not current for {subject}."
            ),
            DiagnosticSeverity::Error,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            Block,
        ),
        ManagedRegistryValidationMissing { family } => diagnostic(
            RuleFamilyV2::B04ArchitectureContractDrift,
            &format!("star.validation.managed-registry.{family}.missing"),
            "MANAGED_REGISTRY_CHECK_MISSING",
            "A required managed registry validation family is missing",
            &format!("The accepted ValidationPlan does not contain required family {family}."),
            DiagnosticSeverity::Error,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            Block,
        ),
        GeneratedOutputDrift { path } => path_diagnostic(
            RuleFamilyV2::B04ArchitectureContractDrift,
            "star.validation.generated.drift",
            "GENERATED_OUTPUT_DRIFT",
            "Generated output changed without generation evidence",
            path,
            DiagnosticSeverity::Error,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            Block,
        ),
        HardcodingCheckMissing => missing_check(
            RuleFamilyV2::B04ArchitectureContractDrift,
            "hardcoding",
            "HARDCODING_CHECK_MISSING",
        ),
        SecretCandidate { path } => path_diagnostic(
            RuleFamilyV2::B05SecuritySupplyChain,
            "star.validation.security.secret-candidate",
            "SECRET_CANDIDATE",
            "Potential secret material was detected",
            path,
            DiagnosticSeverity::Critical,
            DiagnosticConfidence::Medium,
            DiagnosticStatus::Suspected,
            HumanReview,
        ),
        DangerousCommandCandidate { path } => path_diagnostic(
            RuleFamilyV2::B05SecuritySupplyChain,
            "star.validation.security.dangerous-command",
            "DANGEROUS_COMMAND_CANDIDATE",
            "Potentially destructive command was detected",
            path,
            DiagnosticSeverity::Error,
            DiagnosticConfidence::Medium,
            DiagnosticStatus::Suspected,
            HumanReview,
        ),
        DependencyEvidenceMissing { path } => path_diagnostic(
            RuleFamilyV2::B05SecuritySupplyChain,
            "star.validation.supply-chain.evidence-missing",
            "DEPENDENCY_EVIDENCE_MISSING",
            "Dependency change lacks required supply-chain validation",
            path,
            DiagnosticSeverity::Error,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            Block,
        ),
        RegressionEvidenceMissing => diagnostic(
            RuleFamilyV2::B06Regression,
            "star.validation.test.regression-evidence-missing",
            "REGRESSION_EVIDENCE_MISSING",
            "Bug-fix regression evidence is incomplete",
            "Exact before-failure and after-success evidence is not available.",
            DiagnosticSeverity::Warning,
            DiagnosticConfidence::Medium,
            DiagnosticStatus::Suspected,
            HumanReview,
        ),
        RegressionCheckMissing => missing_check(
            RuleFamilyV2::B06Regression,
            "regression",
            "REGRESSION_CHECK_MISSING",
        ),
        DocsCheckMissing => missing_check(
            RuleFamilyV2::B07DocsConfigEnvironment,
            "docs",
            "DOCS_CHECK_MISSING",
        ),
        ConfigCheckMissing => missing_check(
            RuleFamilyV2::B07DocsConfigEnvironment,
            "config",
            "CONFIG_CHECK_MISSING",
        ),
        DocumentationDrift => diagnostic(
            RuleFamilyV2::B07DocsConfigEnvironment,
            "star.validation.docs.drift",
            "DOCUMENTATION_DRIFT",
            "Contract or public behavior changed without documentation evidence",
            "The accepted change affects a public contract but no documentation change is bound.",
            DiagnosticSeverity::Warning,
            DiagnosticConfidence::Medium,
            DiagnosticStatus::Suspected,
            HumanReview,
        ),
        EnvironmentEvidenceMissing => diagnostic(
            RuleFamilyV2::B07DocsConfigEnvironment,
            "star.validation.environment.evidence-missing",
            "ENVIRONMENT_EVIDENCE_MISSING",
            "Environment change lacks compatible validation evidence",
            "CI, toolchain, or environment configuration changed without a project-full check.",
            DiagnosticSeverity::Error,
            DiagnosticConfidence::High,
            DiagnosticStatus::Confirmed,
            Block,
        ),
    }
}

fn missing_check(family: RuleFamilyV2, check: &str, code: &str) -> RuleDiagnosticInputV2 {
    diagnostic(
        family,
        &format!("star.validation.{check}.check-missing"),
        code,
        "Required validation family is missing",
        "The accepted ValidationPlan does not contain the required deterministic check family.",
        DiagnosticSeverity::Error,
        DiagnosticConfidence::High,
        DiagnosticStatus::Confirmed,
        RuleDecisionFloorV2::Block,
    )
}

#[allow(clippy::too_many_arguments)]
fn path_diagnostic(
    family: RuleFamilyV2,
    rule_id: &str,
    code: &str,
    title: &str,
    path: &ProjectPathRef,
    severity: DiagnosticSeverity,
    confidence: DiagnosticConfidence,
    status: DiagnosticStatus,
    decision_floor: RuleDecisionFloorV2,
) -> RuleDiagnosticInputV2 {
    diagnostic(
        family,
        rule_id,
        code,
        title,
        &format!(
            "The finding applies to project-relative path {}.",
            path.as_str()
        ),
        severity,
        confidence,
        status,
        decision_floor,
    )
}

#[allow(clippy::too_many_arguments)]
fn diagnostic(
    family: RuleFamilyV2,
    rule_id: &str,
    code: &str,
    title: &str,
    message: &str,
    severity: DiagnosticSeverity,
    confidence: DiagnosticConfidence,
    status: DiagnosticStatus,
    decision_floor: RuleDecisionFloorV2,
) -> RuleDiagnosticInputV2 {
    RuleDiagnosticInputV2 {
        family,
        rule_id: rule_id.to_owned(),
        code: code.to_owned(),
        title: title.to_owned(),
        message: message.to_owned(),
        severity,
        confidence,
        status,
        decision_floor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_snapshot_guard_requires_both_snapshots_and_all_fixture_classes() {
        let facts = evaluate_two_snapshot_guard(&TwoSnapshotGuardInputV2 {
            protected_surface_changed: true,
            previous_snapshot_fingerprint: Some(Sha256Hash::digest(b"before")),
            current_snapshot_fingerprint: Some(Sha256Hash::digest(b"after")),
            behavior_weakened: false,
            independent_previous_executor: true,
            fixtures: vec![RuleFixtureResultV2 {
                fixture_kind: "positive".to_owned(),
                previous_snapshot_passed: true,
                current_snapshot_passed: true,
                result_fingerprint: Sha256Hash::digest(b"positive"),
            }],
        });
        assert!(facts.iter().any(|fact| matches!(
            fact,
            RuleFactV2::ValidatorFixtureCoverageMissing { missing }
                if missing == &vec!["negative".to_owned(), "edge".to_owned(), "regression".to_owned()]
        )));
    }

    #[test]
    fn false_positive_candidates_never_become_automatic_pass_facts() {
        let diagnostics = evaluate_rule_facts(&[
            RuleFactV2::SecretCandidate {
                path: ProjectPathRef::parse("src/config.rs").unwrap(),
            },
            RuleFactV2::AssertionCountDecreased {
                path: ProjectPathRef::parse("tests/config.rs").unwrap(),
            },
        ]);
        assert!(diagnostics.iter().all(|diagnostic| {
            diagnostic.decision_floor == RuleDecisionFloorV2::HumanReview
                && diagnostic.status == DiagnosticStatus::Suspected
        }));
    }

    #[test]
    fn managed_registry_missing_stale_drift_and_family_gaps_are_fail_closed() {
        let diagnostics = evaluate_rule_facts(&[
            RuleFactV2::ManagedRegistrySnapshotMissing,
            RuleFactV2::ManagedRegistrySnapshotStale,
            RuleFactV2::ManagedRegistryConsistencyDrift {
                subject: "star.error.fixture".to_owned(),
            },
            RuleFactV2::ManagedRegistryValidationMissing {
                family: "consumer_compatibility".to_owned(),
            },
        ]);
        assert_eq!(diagnostics.len(), 4);
        assert!(diagnostics.iter().all(|diagnostic| {
            diagnostic.decision_floor == RuleDecisionFloorV2::Block
                && diagnostic.status == DiagnosticStatus::Confirmed
                && diagnostic.severity == DiagnosticSeverity::Error
        }));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "MANAGED_REGISTRY_CHECK_MISSING")
        );
    }
}
