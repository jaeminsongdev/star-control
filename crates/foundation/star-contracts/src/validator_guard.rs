//! Versioned evidence for M3 validator self-protection.
//!
//! The current validator may consume this document, but it cannot manufacture
//! independence from its own output.  Callers must bind two distinct executor
//! images and immutable result artifacts; the application layer verifies those
//! artifacts before the evidence can raise the B03 decision floor.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    Sha256Hash, canonical_sha256,
    evidence::{
        ActorRef, ArtifactRef, CatalogRef, Completeness, DocumentRef, GateDecisionKind,
        ValidationOutcome,
    },
    ids::{ProjectId, ValidatorGuardEvidenceId},
};

pub const VALIDATOR_GUARD_EVIDENCE_SCHEMA_ID: &str = "star.validator-guard-evidence";
pub const VALIDATOR_GUARD_EVIDENCE_SCHEMA_VERSION: u32 = 2;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum GuardTrustedSourceV2 {
    PlanningBaseline,
    LastKnownGood,
    ReleaseMinimum,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum GuardFixtureKindV2 {
    Positive,
    Negative,
    Edge,
    Regression,
    Adversarial,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum GuardComparisonOutcomeV2 {
    Equivalent,
    Strengthened,
    Weakened,
    Unverified,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GuardExecutorIdentityV2 {
    pub tool_ref: CatalogRef,
    pub executable_image_sha256: Sha256Hash,
    pub executable_binding_fingerprint: Sha256Hash,
    pub trust_evidence_ref: ArtifactRef,
}

impl GuardExecutorIdentityV2 {
    fn valid(&self) -> bool {
        self.tool_ref.format_version > 0
            && !self.tool_ref.catalog_id.trim().is_empty()
            && !self.tool_ref.item_version.trim().is_empty()
            && self.trust_evidence_ref.validate().is_ok()
            && self.trust_evidence_ref.redaction_status != crate::evidence::RedactionStatus::Unknown
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GuardFixtureResultV2 {
    pub fixture_kind: GuardFixtureKindV2,
    pub rule_ref: CatalogRef,
    pub input_sha256: Sha256Hash,
    pub expected_diagnostic_fingerprint: Option<Sha256Hash>,
    pub expected_gate_decision: GateDecisionKind,
    pub previous_outcome: ValidationOutcome,
    pub previous_completeness: Completeness,
    pub current_outcome: ValidationOutcome,
    pub current_completeness: Completeness,
    pub previous_result_ref: ArtifactRef,
    pub current_result_ref: ArtifactRef,
    pub result_fingerprint: Sha256Hash,
}

impl GuardFixtureResultV2 {
    pub fn seal(mut self) -> Result<Self, ValidatorGuardEvidenceError> {
        if self.rule_ref.format_version == 0
            || self.rule_ref.catalog_id.trim().is_empty()
            || self.rule_ref.item_version.trim().is_empty()
            || self.previous_result_ref.redaction_status
                == crate::evidence::RedactionStatus::Unknown
            || self.current_result_ref.redaction_status == crate::evidence::RedactionStatus::Unknown
            || self.previous_result_ref.validate().is_err()
            || self.current_result_ref.validate().is_err()
        {
            return Err(ValidatorGuardEvidenceError::Fixture);
        }
        self.result_fingerprint = guard_fingerprint(
            "star.validator-guard-fixture-result",
            &serde_json::json!({
                "fixture_kind":self.fixture_kind,
                "rule_ref":self.rule_ref,
                "input_sha256":self.input_sha256,
                "expected_diagnostic_fingerprint":self.expected_diagnostic_fingerprint,
                "expected_gate_decision":self.expected_gate_decision,
                "previous_outcome":self.previous_outcome,
                "previous_completeness":self.previous_completeness,
                "current_outcome":self.current_outcome,
                "current_completeness":self.current_completeness,
                "previous_result_ref":self.previous_result_ref,
                "current_result_ref":self.current_result_ref,
            }),
        )?;
        Ok(self)
    }

    pub fn both_snapshots_passed(&self) -> bool {
        self.previous_snapshot_passed() && self.current_snapshot_passed()
    }

    pub fn previous_snapshot_passed(&self) -> bool {
        self.previous_outcome == ValidationOutcome::Pass
            && self.previous_completeness == Completeness::Complete
    }

    pub fn current_snapshot_passed(&self) -> bool {
        self.current_outcome == ValidationOutcome::Pass
            && self.current_completeness == Completeness::Complete
    }

    fn artifact_refs(&self) -> [&ArtifactRef; 2] {
        [&self.previous_result_ref, &self.current_result_ref]
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GuardComparisonV2 {
    pub protected_field_path: String,
    pub previous_value_fingerprint: Sha256Hash,
    pub current_value_fingerprint: Sha256Hash,
    pub rule_ref: CatalogRef,
    pub coverage: Vec<GuardFixtureKindV2>,
    pub outcome: GuardComparisonOutcomeV2,
    pub evidence_refs: Vec<ArtifactRef>,
    pub comparison_fingerprint: Sha256Hash,
}

impl GuardComparisonV2 {
    pub fn seal(mut self) -> Result<Self, ValidatorGuardEvidenceError> {
        self.coverage.sort();
        self.coverage.dedup();
        self.evidence_refs.sort_by(artifact_order);
        self.evidence_refs.dedup();
        if !self.protected_field_path.starts_with('/')
            || self.protected_field_path.len() > 1024
            || self.coverage.is_empty()
            || self.evidence_refs.is_empty()
            || self.rule_ref.format_version == 0
            || self.rule_ref.catalog_id.trim().is_empty()
            || self.rule_ref.item_version.trim().is_empty()
            || self.evidence_refs.iter().any(|reference| {
                reference.redaction_status == crate::evidence::RedactionStatus::Unknown
                    || reference.validate().is_err()
            })
        {
            return Err(ValidatorGuardEvidenceError::Comparison);
        }
        self.comparison_fingerprint = guard_fingerprint(
            "star.validator-guard-comparison",
            &serde_json::json!({
                "protected_field_path":self.protected_field_path,
                "previous_value_fingerprint":self.previous_value_fingerprint,
                "current_value_fingerprint":self.current_value_fingerprint,
                "rule_ref":self.rule_ref,
                "coverage":self.coverage,
                "outcome":self.outcome,
                "evidence_refs":self.evidence_refs,
            }),
        )?;
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidatorGuardEvidenceV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub guard_evidence_id: ValidatorGuardEvidenceId,
    pub revision: u64,
    pub project_id: ProjectId,
    pub task_spec_ref: DocumentRef,
    pub trusted_source: GuardTrustedSourceV2,
    pub trusted_registry_fingerprint: Sha256Hash,
    pub candidate_registry_fingerprint: Sha256Hash,
    pub trusted_executor: GuardExecutorIdentityV2,
    pub candidate_executor: GuardExecutorIdentityV2,
    pub previous_snapshot_fingerprint: Sha256Hash,
    pub current_snapshot_fingerprint: Sha256Hash,
    pub security_sensitive: bool,
    pub fixture_results: Vec<GuardFixtureResultV2>,
    pub comparisons: Vec<GuardComparisonV2>,
    pub produced_by: ActorRef,
    pub produced_at: DateTime<Utc>,
    pub evidence_fingerprint: Sha256Hash,
}

impl ValidatorGuardEvidenceV2 {
    pub fn seal(mut self) -> Result<Self, ValidatorGuardEvidenceError> {
        self.fixture_results = self
            .fixture_results
            .into_iter()
            .map(GuardFixtureResultV2::seal)
            .collect::<Result<Vec<_>, _>>()?;
        self.fixture_results.sort_by(|left, right| {
            (left.fixture_kind, &left.result_fingerprint)
                .cmp(&(right.fixture_kind, &right.result_fingerprint))
        });
        self.comparisons = self
            .comparisons
            .into_iter()
            .map(GuardComparisonV2::seal)
            .collect::<Result<Vec<_>, _>>()?;
        self.comparisons.sort_by(|left, right| {
            (&left.protected_field_path, &left.comparison_fingerprint)
                .cmp(&(&right.protected_field_path, &right.comparison_fingerprint))
        });
        let kinds = self
            .fixture_results
            .iter()
            .map(|fixture| fixture.fixture_kind)
            .collect::<BTreeSet<_>>();
        let required = [
            GuardFixtureKindV2::Positive,
            GuardFixtureKindV2::Negative,
            GuardFixtureKindV2::Edge,
            GuardFixtureKindV2::Regression,
        ];
        if self.schema_id != VALIDATOR_GUARD_EVIDENCE_SCHEMA_ID
            || self.schema_version != VALIDATOR_GUARD_EVIDENCE_SCHEMA_VERSION
            || self.revision == 0
            || self.task_spec_ref.revision == 0
            || self.previous_snapshot_fingerprint == self.current_snapshot_fingerprint
            || !self.trusted_executor.valid()
            || !self.candidate_executor.valid()
            || self.trusted_executor.executable_image_sha256
                == self.candidate_executor.executable_image_sha256
            || self.trusted_executor.executable_binding_fingerprint
                == self.candidate_executor.executable_binding_fingerprint
            || kinds.len() != self.fixture_results.len()
            || !required.into_iter().all(|kind| kinds.contains(&kind))
            || (self.security_sensitive && !kinds.contains(&GuardFixtureKindV2::Adversarial))
            || self.comparisons.is_empty()
            || self.produced_by.actor_id.trim().is_empty()
            || self.produced_by.auth_source.trim().is_empty()
        {
            return Err(ValidatorGuardEvidenceError::Evidence);
        }
        self.evidence_fingerprint = guard_fingerprint(
            "star.validator-guard-evidence",
            &serde_json::json!({
                "guard_evidence_id":self.guard_evidence_id,
                "revision":self.revision,
                "project_id":self.project_id,
                "task_spec_ref":self.task_spec_ref,
                "trusted_source":self.trusted_source,
                "trusted_registry_fingerprint":self.trusted_registry_fingerprint,
                "candidate_registry_fingerprint":self.candidate_registry_fingerprint,
                "trusted_executor":self.trusted_executor,
                "candidate_executor":self.candidate_executor,
                "previous_snapshot_fingerprint":self.previous_snapshot_fingerprint,
                "current_snapshot_fingerprint":self.current_snapshot_fingerprint,
                "security_sensitive":self.security_sensitive,
                "fixture_results":self.fixture_results,
                "comparisons":self.comparisons,
                "produced_by":self.produced_by,
                "produced_at":self.produced_at,
            }),
        )?;
        Ok(self)
    }

    pub fn independent_previous_executor(&self) -> bool {
        self.trusted_executor.executable_image_sha256
            != self.candidate_executor.executable_image_sha256
            && self.trusted_executor.executable_binding_fingerprint
                != self.candidate_executor.executable_binding_fingerprint
    }

    pub fn behavior_weakened(&self) -> bool {
        self.comparisons.iter().any(|comparison| {
            matches!(
                comparison.outcome,
                GuardComparisonOutcomeV2::Weakened | GuardComparisonOutcomeV2::Unverified
            )
        }) || self
            .fixture_results
            .iter()
            .any(|fixture| !fixture.both_snapshots_passed())
    }

    pub fn artifact_refs(&self) -> Vec<&ArtifactRef> {
        let mut references = vec![
            &self.trusted_executor.trust_evidence_ref,
            &self.candidate_executor.trust_evidence_ref,
        ];
        for fixture in &self.fixture_results {
            references.extend(fixture.artifact_refs());
        }
        for comparison in &self.comparisons {
            references.extend(comparison.evidence_refs.iter());
        }
        references.sort_by(|left, right| artifact_order(left, right));
        references.dedup();
        references
    }

    pub fn reference(&self) -> Result<DocumentRef, ValidatorGuardEvidenceError> {
        Ok(DocumentRef {
            schema_id: VALIDATOR_GUARD_EVIDENCE_SCHEMA_ID.to_owned(),
            document_id: self.guard_evidence_id.to_string(),
            revision: self.revision,
            sha256: document_hash(self)?,
        })
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ValidatorGuardEvidenceError {
    #[error("validator guard fixture result is invalid")]
    Fixture,
    #[error("validator guard comparison is invalid")]
    Comparison,
    #[error("validator guard evidence is invalid")]
    Evidence,
    #[error("validator guard fingerprint could not be calculated")]
    Fingerprint,
}

fn artifact_order(left: &ArtifactRef, right: &ArtifactRef) -> std::cmp::Ordering {
    (
        left.artifact_id.as_str(),
        left.relative_path.as_str(),
        left.sha256.as_str(),
    )
        .cmp(&(
            right.artifact_id.as_str(),
            right.relative_path.as_str(),
            right.sha256.as_str(),
        ))
}

fn guard_fingerprint<T: Serialize>(
    domain: &str,
    value: &T,
) -> Result<Sha256Hash, ValidatorGuardEvidenceError> {
    canonical_sha256(&serde_json::json!({
        "domain":domain,
        "version":VALIDATOR_GUARD_EVIDENCE_SCHEMA_VERSION,
        "value":value,
    }))
    .map_err(|_| ValidatorGuardEvidenceError::Fingerprint)
}

fn document_hash<T: Serialize>(value: &T) -> Result<Sha256Hash, ValidatorGuardEvidenceError> {
    serde_json::to_value(value)
        .map_err(|_| ValidatorGuardEvidenceError::Fingerprint)
        .and_then(|value| {
            canonical_sha256(&value).map_err(|_| ValidatorGuardEvidenceError::Fingerprint)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        evidence::{ActorType, ArtifactKind, ProducerRef, RedactionStatus, RetentionClass},
        ids::ArtifactId,
    };

    fn catalog_ref(name: &str) -> CatalogRef {
        CatalogRef {
            catalog_id: name.to_owned(),
            format_version: 1,
            item_version: "1.0.0".to_owned(),
            sha256: Sha256Hash::digest(name.as_bytes()),
        }
    }

    fn artifact(name: &str) -> ArtifactRef {
        ArtifactRef {
            artifact_id: ArtifactId::from_stable_bytes(name.as_bytes()),
            kind: ArtifactKind::Report,
            project_id: None,
            relative_path: format!("validation/guard/{name}.json"),
            media_type: "application/json".to_owned(),
            size_bytes: 2,
            sha256: Sha256Hash::digest(name.as_bytes()),
            created_at: Utc::now(),
            producer: ProducerRef {
                component: "guard-fixture".to_owned(),
                product_version: "1.0.0".to_owned(),
                build_id: "test".to_owned(),
                platform: "windows-x64".to_owned(),
            },
            redaction_status: RedactionStatus::NotNeeded,
            retention_class: RetentionClass::Evidence,
            source_artifact_ref: None,
        }
    }

    fn fixture(kind: GuardFixtureKindV2) -> GuardFixtureResultV2 {
        let name = format!("{kind:?}").to_ascii_lowercase();
        GuardFixtureResultV2 {
            fixture_kind: kind,
            rule_ref: catalog_ref("star.validation.guard"),
            input_sha256: Sha256Hash::digest(format!("{name}-input").as_bytes()),
            expected_diagnostic_fingerprint: None,
            expected_gate_decision: GateDecisionKind::AutoPass,
            previous_outcome: ValidationOutcome::Pass,
            previous_completeness: Completeness::Complete,
            current_outcome: ValidationOutcome::Pass,
            current_completeness: Completeness::Complete,
            previous_result_ref: artifact(&format!("{name}-previous")),
            current_result_ref: artifact(&format!("{name}-current")),
            result_fingerprint: Sha256Hash::digest(b""),
        }
    }

    fn evidence() -> ValidatorGuardEvidenceV2 {
        let comparison_evidence = artifact("comparison");
        ValidatorGuardEvidenceV2 {
            schema_id: VALIDATOR_GUARD_EVIDENCE_SCHEMA_ID.to_owned(),
            schema_version: VALIDATOR_GUARD_EVIDENCE_SCHEMA_VERSION,
            guard_evidence_id: ValidatorGuardEvidenceId::new(),
            revision: 1,
            project_id: ProjectId::new(),
            task_spec_ref: DocumentRef {
                schema_id: "star.task-spec".to_owned(),
                document_id: "tsk_example".to_owned(),
                revision: 1,
                sha256: Sha256Hash::digest(b"task"),
            },
            trusted_source: GuardTrustedSourceV2::LastKnownGood,
            trusted_registry_fingerprint: Sha256Hash::digest(b"registry-before"),
            candidate_registry_fingerprint: Sha256Hash::digest(b"registry-after"),
            trusted_executor: GuardExecutorIdentityV2 {
                tool_ref: catalog_ref("guard.previous"),
                executable_image_sha256: Sha256Hash::digest(b"image-before"),
                executable_binding_fingerprint: Sha256Hash::digest(b"binding-before"),
                trust_evidence_ref: artifact("trust-before"),
            },
            candidate_executor: GuardExecutorIdentityV2 {
                tool_ref: catalog_ref("guard.current"),
                executable_image_sha256: Sha256Hash::digest(b"image-current"),
                executable_binding_fingerprint: Sha256Hash::digest(b"binding-current"),
                trust_evidence_ref: artifact("trust-current"),
            },
            previous_snapshot_fingerprint: Sha256Hash::digest(b"snapshot-before"),
            current_snapshot_fingerprint: Sha256Hash::digest(b"snapshot-current"),
            security_sensitive: false,
            fixture_results: vec![
                fixture(GuardFixtureKindV2::Positive),
                fixture(GuardFixtureKindV2::Negative),
                fixture(GuardFixtureKindV2::Edge),
                fixture(GuardFixtureKindV2::Regression),
            ],
            comparisons: vec![GuardComparisonV2 {
                protected_field_path: "/rules/star.validation.guard/severity".to_owned(),
                previous_value_fingerprint: Sha256Hash::digest(b"error"),
                current_value_fingerprint: Sha256Hash::digest(b"error"),
                rule_ref: catalog_ref("star.validation.guard"),
                coverage: vec![GuardFixtureKindV2::Negative],
                outcome: GuardComparisonOutcomeV2::Equivalent,
                evidence_refs: vec![comparison_evidence],
                comparison_fingerprint: Sha256Hash::digest(b""),
            }],
            produced_by: ActorRef {
                actor_type: ActorType::Tool,
                actor_id: "last-known-good-guard".to_owned(),
                display_name: "Last known good guard".to_owned(),
                auth_source: "registered-tool".to_owned(),
            },
            produced_at: Utc::now(),
            evidence_fingerprint: Sha256Hash::digest(b""),
        }
    }

    #[test]
    fn guard_evidence_requires_distinct_executors_and_complete_fixture_kinds() {
        let sealed = evidence().seal().unwrap();
        assert!(sealed.independent_previous_executor());
        assert!(!sealed.behavior_weakened());
        assert_eq!(
            serde_json::from_value::<ValidatorGuardEvidenceV2>(
                serde_json::to_value(&sealed).unwrap()
            )
            .unwrap(),
            sealed
        );

        let mut missing = evidence();
        missing
            .fixture_results
            .retain(|fixture| fixture.fixture_kind != GuardFixtureKindV2::Regression);
        assert_eq!(missing.seal(), Err(ValidatorGuardEvidenceError::Evidence));
    }

    #[test]
    fn weakening_is_valid_evidence_but_never_auto_pass_eligible() {
        let mut changed = evidence();
        changed.fixture_results[1].current_outcome = ValidationOutcome::Fail;
        changed.comparisons[0].outcome = GuardComparisonOutcomeV2::Weakened;
        let sealed = changed.seal().unwrap();
        assert!(sealed.behavior_weakened());
    }
}
