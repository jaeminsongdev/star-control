//! Provider-neutral Code Health evidence and observations.
//!
//! Tool-specific command lines and parsers remain outside Gateway.  These
//! contracts accept only an exact executable identity plus a declared protocol
//! and keep raw provider output behind `ArtifactRef`.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ProjectId, Sha256Hash, canonical_sha256, development_v2::CompatibilityClass,
    evidence::ArtifactRef, maintenance_v2::ExternalFreshness,
};

pub const EXTERNAL_ANALYSIS_EVIDENCE_V1_SCHEMA_ID: &str = "star.external-analysis-evidence";
pub const COVERAGE_OBSERVATION_V1_SCHEMA_ID: &str = "star.coverage-observation";
pub const FLAKY_TEST_OBSERVATION_V1_SCHEMA_ID: &str = "star.flaky-test-observation";
pub const REPRODUCIBILITY_VERIFICATION_REPORT_V1_SCHEMA_ID: &str =
    "star.reproducibility-verification-report";
pub const RUNTIME_SAFETY_OBSERVATION_V1_SCHEMA_ID: &str = "star.runtime-safety-observation";
pub const NEAR_CLONE_OBSERVATION_V1_SCHEMA_ID: &str = "star.near-clone-observation";
pub const MAX_EXTERNAL_PROVIDER_OBSERVATIONS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExternalAnalysisCompleteness {
    Complete,
    Partial,
    Unavailable,
    Unverified,
    TimedOut,
    Cancelled,
    Flaky,
    Failed,
}

impl ExternalAnalysisCompleteness {
    pub fn is_terminal_success_evidence(self) -> bool {
        self == Self::Complete
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolStability {
    Stable,
    Unstable,
    HumanText,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolDetailLevel {
    Structured,
    ExitClassification,
    RawOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalAnalysisProtocolV1 {
    pub protocol_id: String,
    pub protocol_version: String,
    pub media_type: String,
    pub stability: ProtocolStability,
    pub detail_level: ProtocolDetailLevel,
    pub machine_readable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_uri: Option<String>,
}

impl ExternalAnalysisProtocolV1 {
    pub fn application_normalization_eligible(&self) -> bool {
        self.stability == ProtocolStability::Stable
            && self.machine_readable
            && self.detail_level == ProtocolDetailLevel::Structured
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalAnalysisEvidenceV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub evidence_id: String,
    pub project_id: ProjectId,
    pub provider_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable_sha256: Option<Sha256Hash>,
    pub protocol: ExternalAnalysisProtocolV1,
    pub config_fingerprint: Sha256Hash,
    pub input_fingerprint: Sha256Hash,
    pub source_fingerprint: Sha256Hash,
    pub environment_fingerprint: Sha256Hash,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_artifact_ref: Option<ArtifactRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_artifact_ref: Option<ArtifactRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub completeness: ExternalAnalysisCompleteness,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub content_fingerprint: Sha256Hash,
}

impl ExternalAnalysisEvidenceV1 {
    pub fn seal(mut self) -> Result<Self, ExternalAnalysisContractError> {
        self.content_fingerprint = self.expected_fingerprint()?;
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), ExternalAnalysisContractError> {
        if self.schema_id != EXTERNAL_ANALYSIS_EVIDENCE_V1_SCHEMA_ID
            || self.schema_version != 1
            || !bounded_token(&self.evidence_id, 256)
            || !bounded_token(&self.provider_id, 128)
            || self
                .provider_version
                .as_deref()
                .is_some_and(|version| !bounded_token(version, 256))
            || self.started_at > self.completed_at
            || !bounded_protocol(&self.protocol)
            || !bounded_limitations(&self.limitations)
            || (!self.completeness.is_terminal_success_evidence() && self.limitations.is_empty())
        {
            return Err(ExternalAnalysisContractError::InvalidEvidence);
        }
        if let Some(raw) = &self.raw_artifact_ref {
            raw.validate()
                .map_err(|_| ExternalAnalysisContractError::InvalidArtifact)?;
            if raw.project_id.as_ref() != Some(&self.project_id) {
                return Err(ExternalAnalysisContractError::CrossSubjectEvidence);
            }
        }
        if let Some(normalized) = &self.normalized_artifact_ref {
            normalized
                .validate()
                .map_err(|_| ExternalAnalysisContractError::InvalidArtifact)?;
            if normalized.project_id.as_ref() != Some(&self.project_id) {
                return Err(ExternalAnalysisContractError::CrossSubjectEvidence);
            }
            if !self.protocol.application_normalization_eligible() {
                return Err(ExternalAnalysisContractError::UnstableNormalization);
            }
            let Some(raw) = self.raw_artifact_ref.as_ref() else {
                return Err(ExternalAnalysisContractError::InvalidArtifact);
            };
            if normalized
                .source_artifact_ref
                .as_deref()
                .is_none_or(|source| {
                    source.artifact_id != raw.artifact_id || source.sha256 != raw.sha256
                })
            {
                return Err(ExternalAnalysisContractError::InvalidArtifact);
            }
        }
        if self.completeness == ExternalAnalysisCompleteness::Unavailable {
            if self.provider_version.is_some()
                || self.executable_sha256.is_some()
                || self.raw_artifact_ref.is_some()
                || self.normalized_artifact_ref.is_some()
                || self.exit_code.is_some()
            {
                return Err(ExternalAnalysisContractError::UnavailableClaim);
            }
        } else if self.provider_version.as_deref().is_none_or(str::is_empty)
            || self.executable_sha256.is_none()
            || self.raw_artifact_ref.is_none()
        {
            return Err(ExternalAnalysisContractError::MissingExecutionIdentity);
        }
        if self.expected_fingerprint()? != self.content_fingerprint {
            return Err(ExternalAnalysisContractError::Fingerprint);
        }
        Ok(())
    }

    pub fn expected_fingerprint(&self) -> Result<Sha256Hash, ExternalAnalysisContractError> {
        canonical_sha256(&serde_json::json!({
            "schema_id":self.schema_id,
            "schema_version":self.schema_version,
            "evidence_id":self.evidence_id,
            "project_id":self.project_id,
            "provider_id":self.provider_id,
            "provider_version":self.provider_version,
            "executable_sha256":self.executable_sha256,
            "protocol":self.protocol,
            "config_fingerprint":self.config_fingerprint,
            "input_fingerprint":self.input_fingerprint,
            "source_fingerprint":self.source_fingerprint,
            "environment_fingerprint":self.environment_fingerprint,
            "raw_artifact_ref":self.raw_artifact_ref,
            "normalized_artifact_ref":self.normalized_artifact_ref,
            "exit_code":self.exit_code,
            "started_at":self.started_at,
            "completed_at":self.completed_at,
            "completeness":self.completeness,
            "limitations":self.limitations,
        }))
        .map_err(|_| ExternalAnalysisContractError::Fingerprint)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BranchCoverageShadowV1 {
    pub measured_branches: u64,
    pub covered_branches: u64,
    /// Branch coverage remains informational until the selected provider
    /// protocol is declared stable by the Catalog.
    pub gate_eligible: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoverageGateStateV1 {
    Pass,
    Block,
    NotApplicable,
    Unverified,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoverageObservationV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub observation_id: String,
    pub project_id: ProjectId,
    pub evidence: ExternalAnalysisEvidenceV1,
    pub changed_scope_fingerprint: Sha256Hash,
    pub changed_line_count: u64,
    pub executable_line_count: u64,
    pub covered_executable_line_count: u64,
    pub uncovered_executable_line_count: u64,
    pub excluded_line_count: u64,
    pub line_coverage_basis_points: u32,
    pub required_line_coverage_basis_points: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_shadow: Option<BranchCoverageShadowV1>,
    pub gate_state: CoverageGateStateV1,
    #[serde(default)]
    pub limitations: Vec<String>,
}

impl CoverageObservationV1 {
    pub fn validate(&self) -> Result<(), ExternalAnalysisContractError> {
        self.evidence.validate()?;
        let Some(classified_line_count) = self
            .executable_line_count
            .checked_add(self.excluded_line_count)
        else {
            return Err(ExternalAnalysisContractError::InvalidCoverage);
        };
        if self.schema_id != COVERAGE_OBSERVATION_V1_SCHEMA_ID
            || self.schema_version != 1
            || !bounded_token(&self.observation_id, 256)
            || self.project_id != self.evidence.project_id
            || self.changed_scope_fingerprint != self.evidence.input_fingerprint
            || self.changed_line_count < classified_line_count
            || self.executable_line_count
                != self
                    .covered_executable_line_count
                    .saturating_add(self.uncovered_executable_line_count)
            || self.required_line_coverage_basis_points > 10_000
            || !bounded_limitations(&self.limitations)
        {
            return Err(ExternalAnalysisContractError::InvalidCoverage);
        }
        let expected_basis_points = if self.executable_line_count == 0 {
            0
        } else {
            u32::try_from(
                (u128::from(self.covered_executable_line_count) * 10_000)
                    / u128::from(self.executable_line_count),
            )
            .map_err(|_| ExternalAnalysisContractError::InvalidCoverage)?
        };
        if expected_basis_points != self.line_coverage_basis_points {
            return Err(ExternalAnalysisContractError::InvalidCoverage);
        }
        if let Some(branch) = &self.branch_shadow
            && (branch.gate_eligible || branch.covered_branches > branch.measured_branches)
        {
            return Err(ExternalAnalysisContractError::UnstableBranchGate);
        }
        let expected_gate = if !self.evidence.completeness.is_terminal_success_evidence()
            || !self.evidence.protocol.application_normalization_eligible()
        {
            CoverageGateStateV1::Unverified
        } else if self.executable_line_count == 0 {
            CoverageGateStateV1::NotApplicable
        } else if self.line_coverage_basis_points >= self.required_line_coverage_basis_points {
            CoverageGateStateV1::Pass
        } else {
            CoverageGateStateV1::Block
        };
        if self.gate_state != expected_gate {
            return Err(ExternalAnalysisContractError::InvalidCoverageGate);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TestAttemptOutcomeV1 {
    Pass,
    Fail,
    TimedOut,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TestAttemptObservationV1 {
    pub attempt_id: String,
    pub input_fingerprint: Sha256Hash,
    pub environment_fingerprint: Sha256Hash,
    pub outcome: TestAttemptOutcomeV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlakyQuarantineV1 {
    pub owner: String,
    pub approval_ref: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FlakyTestClassificationV1 {
    StablePass,
    StableFail,
    Flaky,
    Inconclusive,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlakyTestObservationV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub observation_id: String,
    pub project_id: ProjectId,
    pub evidence: ExternalAnalysisEvidenceV1,
    pub test_id: String,
    pub input_fingerprint: Sha256Hash,
    pub environment_fingerprint: Sha256Hash,
    pub attempts: Vec<TestAttemptObservationV1>,
    pub classification: FlakyTestClassificationV1,
    /// A retry pass is never promoted to an ordinary clean pass.
    pub retry_success_promoted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quarantine: Option<FlakyQuarantineV1>,
    #[serde(default)]
    pub limitations: Vec<String>,
}

impl FlakyTestObservationV1 {
    pub fn validate(&self) -> Result<(), ExternalAnalysisContractError> {
        self.evidence.validate()?;
        let mut attempt_ids = BTreeSet::new();
        if self.schema_id != FLAKY_TEST_OBSERVATION_V1_SCHEMA_ID
            || self.schema_version != 1
            || self.project_id != self.evidence.project_id
            || self.input_fingerprint != self.evidence.input_fingerprint
            || self.environment_fingerprint != self.evidence.environment_fingerprint
            || !bounded_token(&self.observation_id, 256)
            || !bounded_text(&self.test_id, 1_024)
            || self.retry_success_promoted
            || self.attempts.len() > 256
            || self.attempts.iter().any(|attempt| {
                !bounded_token(&attempt.attempt_id, 256)
                    || !attempt_ids.insert(attempt.attempt_id.as_str())
                    || attempt.input_fingerprint != self.input_fingerprint
                    || attempt.environment_fingerprint != self.environment_fingerprint
            })
            || !bounded_limitations(&self.limitations)
            || (self.evidence.completeness == ExternalAnalysisCompleteness::Unavailable
                && !self.attempts.is_empty())
        {
            return Err(ExternalAnalysisContractError::InvalidFlakyObservation);
        }
        if let Some(quarantine) = &self.quarantine
            && (!bounded_text(&quarantine.owner, 256)
                || !bounded_text(&quarantine.approval_ref, 256)
                || quarantine.expires_at <= self.evidence.completed_at)
        {
            return Err(ExternalAnalysisContractError::InvalidQuarantine);
        }
        let pass = self
            .attempts
            .iter()
            .any(|attempt| attempt.outcome == TestAttemptOutcomeV1::Pass);
        let fail = self
            .attempts
            .iter()
            .any(|attempt| attempt.outcome == TestAttemptOutcomeV1::Fail);
        let interrupted = self.attempts.iter().any(|attempt| {
            matches!(
                attempt.outcome,
                TestAttemptOutcomeV1::TimedOut | TestAttemptOutcomeV1::Cancelled
            )
        });
        let expected = if self.evidence.completeness == ExternalAnalysisCompleteness::Unavailable {
            FlakyTestClassificationV1::Unavailable
        } else if interrupted || self.attempts.is_empty() {
            FlakyTestClassificationV1::Inconclusive
        } else if pass && fail && self.attempts.len() >= 2 {
            FlakyTestClassificationV1::Flaky
        } else if !self.evidence.completeness.is_terminal_success_evidence() {
            FlakyTestClassificationV1::Inconclusive
        } else if pass && !fail {
            FlakyTestClassificationV1::StablePass
        } else if fail && !pass {
            FlakyTestClassificationV1::StableFail
        } else {
            FlakyTestClassificationV1::Inconclusive
        };
        if self.classification != expected {
            return Err(ExternalAnalysisContractError::InvalidFlakyClassification);
        }
        if self.quarantine.is_some() && self.classification != FlakyTestClassificationV1::Flaky {
            return Err(ExternalAnalysisContractError::InvalidQuarantine);
        }
        Ok(())
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityProviderKindV1 {
    Buf,
    Oasdiff,
    CargoSemverChecks,
    Libabigail,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityProviderObservationV1 {
    pub provider_kind: CompatibilityProviderKindV1,
    pub evidence: ExternalAnalysisEvidenceV1,
    pub classification: CompatibilityClass,
    pub machine_detail_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_exit_classification: Option<String>,
    #[serde(default)]
    pub limitations: Vec<String>,
}

impl CompatibilityProviderObservationV1 {
    pub fn validate(&self) -> Result<(), ExternalAnalysisContractError> {
        self.evidence.validate()?;
        if !compatibility_provider_matches(self.provider_kind, &self.evidence.provider_id)
            || self.machine_detail_count > 1_000_000
            || self
                .raw_exit_classification
                .as_deref()
                .is_some_and(|value| !bounded_token(value, 256))
            || !bounded_limitations(&self.limitations)
            || (self.evidence.protocol.detail_level != ProtocolDetailLevel::Structured
                && self.machine_detail_count != 0)
        {
            return Err(ExternalAnalysisContractError::InventedProviderDetail);
        }
        match self.evidence.protocol.detail_level {
            ProtocolDetailLevel::Structured
                if !self.evidence.protocol.application_normalization_eligible() =>
            {
                return Err(ExternalAnalysisContractError::UnstableNormalization);
            }
            ProtocolDetailLevel::ExitClassification
                if self.evidence.protocol.stability != ProtocolStability::Stable
                    || !self.evidence.protocol.machine_readable
                    || self.raw_exit_classification.is_none()
                    || self.evidence.exit_code.is_none() =>
            {
                return Err(ExternalAnalysisContractError::InventedProviderDetail);
            }
            ProtocolDetailLevel::RawOnly
                if self.classification != CompatibilityClass::Unknown
                    || self.machine_detail_count != 0
                    || self.raw_exit_classification.is_some() =>
            {
                return Err(ExternalAnalysisContractError::InventedProviderDetail);
            }
            _ => {}
        }
        if !self.evidence.completeness.is_terminal_success_evidence()
            && self.classification != CompatibilityClass::Unknown
        {
            return Err(ExternalAnalysisContractError::CrossSubjectEvidence);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArchitectureRuleKindV1 {
    Layer,
    AllowedEdge,
    ForbiddenEdge,
    DependencyCycle,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureRuleExceptionV1 {
    pub subject: String,
    pub owner: String,
    pub approval_ref: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureRuleV1 {
    pub kind: ArchitectureRuleKindV1,
    pub source_layer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_layer: Option<String>,
    #[serde(default)]
    pub exceptions: Vec<ArchitectureRuleExceptionV1>,
}

impl ArchitectureRuleV1 {
    pub fn validate(
        &self,
        observed_at: DateTime<Utc>,
    ) -> Result<(), ExternalAnalysisContractError> {
        let target_shape_valid = match self.kind {
            ArchitectureRuleKindV1::Layer => self.target_layer.is_none(),
            ArchitectureRuleKindV1::AllowedEdge
            | ArchitectureRuleKindV1::ForbiddenEdge
            | ArchitectureRuleKindV1::DependencyCycle => self.target_layer.is_some(),
        };
        let mut exception_subjects = BTreeSet::new();
        if !bounded_token(&self.source_layer, 256)
            || !target_shape_valid
            || self
                .target_layer
                .as_deref()
                .is_some_and(|target| !bounded_token(target, 256))
            || self.exceptions.len() > 1_024
            || self.exceptions.iter().any(|exception| {
                !bounded_text(&exception.subject, 1_024)
                    || !exception_subjects.insert(exception.subject.as_str())
                    || !bounded_text(&exception.owner, 256)
                    || !bounded_text(&exception.approval_ref, 256)
                    || exception.expires_at <= observed_at
            })
        {
            return Err(ExternalAnalysisContractError::InvalidArchitectureRule);
        }
        Ok(())
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SupplyChainProviderKindV1 {
    Sbom,
    Advisory,
    License,
    Vex,
    Provenance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReachabilityStateV1 {
    Reachable,
    NotReachable,
    Unknown,
    NotAnalyzed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VexStateV1 {
    Affected,
    NotAffected,
    Fixed,
    UnderInvestigation,
    None,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AdvisoryFreshnessObservationV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_fingerprint: Option<Sha256Hash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_updated_at: Option<DateTime<Utc>>,
    pub observed_at: DateTime<Utc>,
    pub maximum_age_seconds: u64,
    pub freshness: ExternalFreshness,
}

impl AdvisoryFreshnessObservationV1 {
    fn validate(
        &self,
        evidence: &ExternalAnalysisEvidenceV1,
    ) -> Result<(), ExternalAnalysisContractError> {
        if self.maximum_age_seconds == 0
            || self.maximum_age_seconds > 365 * 24 * 60 * 60
            || self.observed_at < evidence.started_at
            || self.observed_at > evidence.completed_at
        {
            return Err(ExternalAnalysisContractError::InvalidSupplyChainObservation);
        }
        let expected = if evidence.completeness == ExternalAnalysisCompleteness::Unavailable {
            if self.database_fingerprint.is_some() || self.database_updated_at.is_some() {
                return Err(ExternalAnalysisContractError::UnavailableClaim);
            }
            ExternalFreshness::Unavailable
        } else {
            if self.database_fingerprint.is_none() {
                return Err(ExternalAnalysisContractError::InvalidSupplyChainObservation);
            }
            match self.database_updated_at {
                Some(updated_at) if updated_at <= self.observed_at => {
                    let age = self
                        .observed_at
                        .signed_duration_since(updated_at)
                        .num_seconds();
                    if u64::try_from(age).unwrap_or(u64::MAX) <= self.maximum_age_seconds {
                        ExternalFreshness::Current
                    } else {
                        ExternalFreshness::Stale
                    }
                }
                Some(_) => {
                    return Err(ExternalAnalysisContractError::InvalidSupplyChainObservation);
                }
                None => ExternalFreshness::Unknown,
            }
        };
        if self.freshness != expected {
            return Err(ExternalAnalysisContractError::InvalidSupplyChainObservation);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SupplyChainProviderObservationV1 {
    pub provider_kind: SupplyChainProviderKindV1,
    pub evidence: ExternalAnalysisEvidenceV1,
    #[serde(default)]
    pub standards: Vec<String>,
    #[serde(default)]
    pub license_expressions: Vec<String>,
    pub reachability: ReachabilityStateV1,
    pub vex_state: VexStateV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advisory_freshness: Option<AdvisoryFreshnessObservationV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_vex_evidence_ref: Option<ArtifactRef>,
    #[serde(default)]
    pub limitations: Vec<String>,
}

impl SupplyChainProviderObservationV1 {
    pub fn validate(&self) -> Result<(), ExternalAnalysisContractError> {
        self.evidence.validate()?;
        if !self.evidence.protocol.application_normalization_eligible() {
            return Err(ExternalAnalysisContractError::UnstableNormalization);
        }
        if self.standards.len() > 64
            || self.standards.iter().collect::<BTreeSet<_>>().len() != self.standards.len()
            || self.standards.iter().any(|value| !bounded_text(value, 256))
            || self.license_expressions.len() > 4_096
            || self
                .license_expressions
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                != self.license_expressions.len()
            || self
                .license_expressions
                .iter()
                .any(|value| !bounded_text(value, 1_024))
            || !bounded_limitations(&self.limitations)
        {
            return Err(ExternalAnalysisContractError::InvalidSupplyChainObservation);
        }
        if let Some(reviewed) = &self.reviewed_vex_evidence_ref {
            reviewed
                .validate()
                .map_err(|_| ExternalAnalysisContractError::InvalidArtifact)?;
            if reviewed.project_id.as_ref() != Some(&self.evidence.project_id) {
                return Err(ExternalAnalysisContractError::CrossSubjectEvidence);
            }
        }
        match (self.provider_kind, &self.advisory_freshness) {
            (SupplyChainProviderKindV1::Advisory, Some(freshness)) => {
                freshness.validate(&self.evidence)?;
            }
            (SupplyChainProviderKindV1::Advisory, None) | (_, Some(_)) => {
                return Err(ExternalAnalysisContractError::InvalidSupplyChainObservation);
            }
            (_, None) => {}
        }
        if self.vex_state == VexStateV1::NotAffected
            && (!self.evidence.completeness.is_terminal_success_evidence()
                || self.reviewed_vex_evidence_ref.is_none()
                || matches!(
                    self.reachability,
                    ReachabilityStateV1::Unknown | ReachabilityStateV1::NotAnalyzed
                ))
        {
            return Err(ExternalAnalysisContractError::UnreviewedNotAffected);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BuildArtifactDigestV1 {
    pub logical_name: String,
    pub sha256: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IsolatedBuildObservationV1 {
    pub root_id: String,
    pub root_fingerprint: Sha256Hash,
    pub artifacts: Vec<BuildArtifactDigestV1>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReproducibilityOutcomeV1 {
    Pass,
    Mismatch,
    Unavailable,
    Unverified,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReproducibilityVerificationReportV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub report_id: String,
    pub project_id: ProjectId,
    pub source_fingerprint: Sha256Hash,
    pub config_fingerprint: Sha256Hash,
    pub toolchain_fingerprint: Sha256Hash,
    pub builds: Vec<IsolatedBuildObservationV1>,
    pub provider_evidence: Vec<ExternalAnalysisEvidenceV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slsa_provenance_ref: Option<ArtifactRef>,
    pub slsa_subject_bound: bool,
    pub slsa_materials_bound: bool,
    pub slsa_builder_bound: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffoscope_diagnostic_ref: Option<ArtifactRef>,
    pub outcome: ReproducibilityOutcomeV1,
    #[serde(default)]
    pub limitations: Vec<String>,
}

impl ReproducibilityVerificationReportV1 {
    pub fn validate(&self) -> Result<(), ExternalAnalysisContractError> {
        let mut evidence_ids = BTreeSet::new();
        if self.provider_evidence.len() > 16 || self.builds.len() > 2 {
            return Err(ExternalAnalysisContractError::InvalidReproducibility);
        }
        for evidence in &self.provider_evidence {
            evidence.validate()?;
            if !evidence.protocol.application_normalization_eligible() {
                return Err(ExternalAnalysisContractError::UnstableNormalization);
            }
            if evidence.project_id != self.project_id
                || evidence.source_fingerprint != self.source_fingerprint
                || evidence.config_fingerprint != self.config_fingerprint
                || !evidence_ids.insert((&evidence.provider_id, &evidence.evidence_id))
            {
                return Err(ExternalAnalysisContractError::CrossSubjectEvidence);
            }
        }
        if let Some(provenance) = &self.slsa_provenance_ref {
            provenance
                .validate()
                .map_err(|_| ExternalAnalysisContractError::InvalidArtifact)?;
            if provenance.project_id.as_ref() != Some(&self.project_id)
                || !self.provider_evidence.iter().any(|evidence| {
                    evidence.raw_artifact_ref.as_ref() == Some(provenance)
                        || evidence.normalized_artifact_ref.as_ref() == Some(provenance)
                })
            {
                return Err(ExternalAnalysisContractError::CrossSubjectEvidence);
            }
        } else if self.slsa_subject_bound || self.slsa_materials_bound || self.slsa_builder_bound {
            return Err(ExternalAnalysisContractError::InvalidReproducibility);
        }
        if let Some(diffoscope) = &self.diffoscope_diagnostic_ref {
            diffoscope
                .validate()
                .map_err(|_| ExternalAnalysisContractError::InvalidArtifact)?;
            if diffoscope.project_id.as_ref() != Some(&self.project_id) {
                return Err(ExternalAnalysisContractError::CrossSubjectEvidence);
            }
        }
        if !bounded_limitations(&self.limitations)
            || self.builds.iter().any(|build| {
                let unique_names = build
                    .artifacts
                    .iter()
                    .map(|artifact| artifact.logical_name.as_str())
                    .collect::<BTreeSet<_>>();
                !bounded_token(&build.root_id, 256)
                    || build.artifacts.len() > 4_096
                    || build
                        .artifacts
                        .iter()
                        .any(|artifact| !bounded_text(&artifact.logical_name, 1_024))
                    || unique_names.len() != build.artifacts.len()
            })
        {
            return Err(ExternalAnalysisContractError::InvalidReproducibility);
        }
        let two_distinct_roots = self.builds.len() == 2
            && self.builds[0].root_id != self.builds[1].root_id
            && self.builds[0].root_fingerprint != self.builds[1].root_fingerprint;
        let canonical_artifacts = |build: &IsolatedBuildObservationV1| {
            let mut artifacts = build.artifacts.clone();
            artifacts.sort();
            artifacts
        };
        let artifacts_equal = two_distinct_roots
            && !self.builds[0].artifacts.is_empty()
            && canonical_artifacts(&self.builds[0]) == canonical_artifacts(&self.builds[1]);
        let provider_evidence_complete = !self.provider_evidence.is_empty()
            && self
                .provider_evidence
                .iter()
                .all(|evidence| evidence.completeness.is_terminal_success_evidence());
        let slsa_binding_complete = self.slsa_provenance_ref.as_ref().is_some_and(|provenance| {
            self.slsa_subject_bound
                && self.slsa_materials_bound
                && self.slsa_builder_bound
                && self.provider_evidence.iter().any(|evidence| {
                    evidence.completeness.is_terminal_success_evidence()
                        && (evidence.raw_artifact_ref.as_ref() == Some(provenance)
                            || evidence.normalized_artifact_ref.as_ref() == Some(provenance))
                })
        });
        let expected = if self
            .provider_evidence
            .iter()
            .any(|evidence| evidence.completeness == ExternalAnalysisCompleteness::Unavailable)
        {
            ReproducibilityOutcomeV1::Unavailable
        } else if !two_distinct_roots || !provider_evidence_complete {
            ReproducibilityOutcomeV1::Unverified
        } else if !artifacts_equal {
            ReproducibilityOutcomeV1::Mismatch
        } else if slsa_binding_complete {
            ReproducibilityOutcomeV1::Pass
        } else {
            ReproducibilityOutcomeV1::Unverified
        };
        if self.schema_id != REPRODUCIBILITY_VERIFICATION_REPORT_V1_SCHEMA_ID
            || self.schema_version != 1
            || !bounded_token(&self.report_id, 256)
            || self.outcome != expected
        {
            return Err(ExternalAnalysisContractError::InvalidReproducibility);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSafetyProviderKindV1 {
    Sanitizer,
    Generator,
    Doctest,
    Loom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSafetyStateV1 {
    Pass,
    Fail,
    TimedOut,
    Cancelled,
    Unavailable,
    Unverified,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeSafetyObservationV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub observation_id: String,
    pub project_id: ProjectId,
    pub provider_kind: RuntimeSafetyProviderKindV1,
    pub evidence: ExternalAnalysisEvidenceV1,
    pub target: String,
    pub project_declares_toolchain_or_test: bool,
    pub timeout_ms: u64,
    pub cancelled: bool,
    pub finding_refs: Vec<String>,
    pub state: RuntimeSafetyStateV1,
    #[serde(default)]
    pub limitations: Vec<String>,
}

impl RuntimeSafetyObservationV1 {
    pub fn validate(&self) -> Result<(), ExternalAnalysisContractError> {
        self.evidence.validate()?;
        let expected = if !self.project_declares_toolchain_or_test
            || self.evidence.completeness == ExternalAnalysisCompleteness::Unavailable
        {
            RuntimeSafetyStateV1::Unavailable
        } else if self.cancelled
            || self.evidence.completeness == ExternalAnalysisCompleteness::Cancelled
        {
            RuntimeSafetyStateV1::Cancelled
        } else if self.evidence.completeness == ExternalAnalysisCompleteness::TimedOut {
            RuntimeSafetyStateV1::TimedOut
        } else if !self.evidence.completeness.is_terminal_success_evidence() {
            RuntimeSafetyStateV1::Unverified
        } else if self.finding_refs.is_empty() {
            RuntimeSafetyStateV1::Pass
        } else {
            RuntimeSafetyStateV1::Fail
        };
        if self.schema_id != RUNTIME_SAFETY_OBSERVATION_V1_SCHEMA_ID
            || self.schema_version != 1
            || self.project_id != self.evidence.project_id
            || !runtime_safety_provider_matches(self.provider_kind, &self.evidence.provider_id)
            || !bounded_token(&self.observation_id, 256)
            || !bounded_text(&self.target, 1_024)
            || self.timeout_ms == 0
            || self.timeout_ms > 86_400_000
            || (!self.project_declares_toolchain_or_test
                && self.evidence.completeness != ExternalAnalysisCompleteness::Unavailable)
            || (self.evidence.completeness == ExternalAnalysisCompleteness::Unavailable
                && !self.finding_refs.is_empty())
            || (self.cancelled
                != (self.evidence.completeness == ExternalAnalysisCompleteness::Cancelled))
            || self
                .finding_refs
                .iter()
                .any(|finding| !bounded_text(finding, 1_024))
            || self.finding_refs.len() > 10_000
            || self.finding_refs.iter().collect::<BTreeSet<_>>().len() != self.finding_refs.len()
            || !bounded_limitations(&self.limitations)
            || self.state != expected
        {
            return Err(ExternalAnalysisContractError::InvalidRuntimeSafety);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NearClonePairV1 {
    pub left_subject: String,
    pub right_subject: String,
    pub left_fingerprint: Sha256Hash,
    pub right_fingerprint: Sha256Hash,
    pub similarity_basis_points: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NearCloneObservationV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub observation_id: String,
    pub project_id: ProjectId,
    pub source_fingerprint: Sha256Hash,
    pub algorithm: String,
    pub minimum_similarity_basis_points: u32,
    pub max_candidates: u32,
    pub max_source_bytes: u64,
    pub pairs: Vec<NearClonePairV1>,
    pub advisory_only: bool,
    pub automatic_patch_set_generated: bool,
    #[serde(default)]
    pub limitations: Vec<String>,
}

impl NearCloneObservationV1 {
    pub fn validate(&self) -> Result<(), ExternalAnalysisContractError> {
        let mut pair_keys = BTreeSet::new();
        if self.schema_id != NEAR_CLONE_OBSERVATION_V1_SCHEMA_ID
            || self.schema_version != 1
            || !bounded_token(&self.observation_id, 256)
            || self.algorithm != "identifier_literal_normalized_simhash_v1"
            || !(5_000..=10_000).contains(&self.minimum_similarity_basis_points)
            || self.max_candidates == 0
            || self.max_candidates > 10_000
            || self.max_source_bytes == 0
            || self.max_source_bytes > 64 * 1024 * 1024
            || !self.advisory_only
            || self.automatic_patch_set_generated
            || self.pairs.len() > self.max_candidates as usize
            || self.pairs.iter().any(|pair| {
                let pair_key = if pair.left_subject < pair.right_subject {
                    (pair.left_subject.clone(), pair.right_subject.clone())
                } else {
                    (pair.right_subject.clone(), pair.left_subject.clone())
                };
                pair.left_subject == pair.right_subject
                    || pair.similarity_basis_points < self.minimum_similarity_basis_points
                    || pair.similarity_basis_points > 10_000
                    || !bounded_text(&pair.left_subject, 1_024)
                    || !bounded_text(&pair.right_subject, 1_024)
                    || !pair_keys.insert(pair_key)
            })
            || !bounded_limitations(&self.limitations)
        {
            return Err(ExternalAnalysisContractError::InvalidNearClone);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ExternalAnalysisContractError {
    #[error("external analysis evidence is invalid")]
    InvalidEvidence,
    #[error("external analysis artifact is invalid")]
    InvalidArtifact,
    #[error("unavailable evidence claimed execution artifacts")]
    UnavailableClaim,
    #[error("executed evidence is missing exact executable identity or raw artifact")]
    MissingExecutionIdentity,
    #[error("unstable or human protocol cannot produce normalized evidence")]
    UnstableNormalization,
    #[error("external analysis fingerprint is invalid")]
    Fingerprint,
    #[error("coverage observation is invalid")]
    InvalidCoverage,
    #[error("unstable branch coverage cannot be a gate")]
    UnstableBranchGate,
    #[error("coverage gate state is invalid")]
    InvalidCoverageGate,
    #[error("flaky-test observation is invalid")]
    InvalidFlakyObservation,
    #[error("flaky-test classification is invalid")]
    InvalidFlakyClassification,
    #[error("quarantine requires owner, approval, and future expiry")]
    InvalidQuarantine,
    #[error("provider detail was inferred from non-structured output")]
    InventedProviderDetail,
    #[error("architecture rule or exception is invalid")]
    InvalidArchitectureRule,
    #[error("supply-chain provider observation is invalid")]
    InvalidSupplyChainObservation,
    #[error("not_affected requires reviewed VEX and reachability evidence")]
    UnreviewedNotAffected,
    #[error("provider evidence is bound to another project or source")]
    CrossSubjectEvidence,
    #[error("reproducibility report is invalid")]
    InvalidReproducibility,
    #[error("runtime-safety observation is invalid")]
    InvalidRuntimeSafety,
    #[error("near-clone observation is invalid")]
    InvalidNearClone,
}

fn compatibility_provider_matches(kind: CompatibilityProviderKindV1, provider_id: &str) -> bool {
    matches!(
        (kind, provider_id),
        (CompatibilityProviderKindV1::Buf, "buf")
            | (CompatibilityProviderKindV1::Oasdiff, "oasdiff")
            | (
                CompatibilityProviderKindV1::CargoSemverChecks,
                "cargo-semver-checks"
            )
            | (CompatibilityProviderKindV1::Libabigail, "libabigail")
    )
}

fn runtime_safety_provider_matches(kind: RuntimeSafetyProviderKindV1, provider_id: &str) -> bool {
    matches!(
        (kind, provider_id),
        (RuntimeSafetyProviderKindV1::Sanitizer, "sanitizer")
            | (
                RuntimeSafetyProviderKindV1::Generator | RuntimeSafetyProviderKindV1::Doctest,
                "generator-doctest"
            )
            | (RuntimeSafetyProviderKindV1::Loom, "loom")
    )
}

fn bounded_limitations(values: &[String]) -> bool {
    values.len() <= 64 && values.iter().all(|value| bounded_text(value, 2_000))
}

fn bounded_token(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && !value.contains('\0')
        && !value.chars().any(char::is_whitespace)
}

fn bounded_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.contains('\0')
}

fn bounded_protocol(protocol: &ExternalAnalysisProtocolV1) -> bool {
    bounded_token(&protocol.protocol_id, 128)
        && bounded_token(&protocol.protocol_version, 128)
        && bounded_token(&protocol.media_type, 256)
        && protocol
            .schema_uri
            .as_deref()
            .is_none_or(|value| bounded_text(value, 2_048))
        && (protocol.machine_readable || protocol.detail_level == ProtocolDetailLevel::RawOnly)
        && (protocol.stability != ProtocolStability::HumanText || !protocol.machine_readable)
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone};

    use super::*;
    use crate::{
        evidence::{ArtifactKind, ProducerRef, RedactionStatus, RetentionClass},
        ids::ArtifactId,
    };

    fn observed_at() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 31, 0, 0, 0)
            .single()
            .expect("valid test timestamp")
    }

    fn hash(value: &str) -> Sha256Hash {
        Sha256Hash::digest(value.as_bytes())
    }

    fn artifact(project_id: &ProjectId, name: &str) -> ArtifactRef {
        ArtifactRef {
            artifact_id: ArtifactId::from_stable_bytes(name.as_bytes()),
            kind: ArtifactKind::Report,
            project_id: Some(project_id.clone()),
            relative_path: format!("external-analysis/{name}.json"),
            media_type: "application/json".to_owned(),
            size_bytes: 32,
            sha256: hash(name),
            created_at: observed_at(),
            producer: ProducerRef {
                component: "external-analysis-test".to_owned(),
                product_version: "1.0.0".to_owned(),
                build_id: "test-build".to_owned(),
                platform: "test".to_owned(),
            },
            redaction_status: RedactionStatus::NotNeeded,
            retention_class: RetentionClass::Evidence,
            source_artifact_ref: None,
        }
    }

    fn protocol(detail_level: ProtocolDetailLevel) -> ExternalAnalysisProtocolV1 {
        ExternalAnalysisProtocolV1 {
            protocol_id: "provider-json".to_owned(),
            protocol_version: "1".to_owned(),
            media_type: "application/json".to_owned(),
            stability: ProtocolStability::Stable,
            detail_level,
            machine_readable: true,
            schema_uri: Some("https://example.invalid/provider.schema.json".to_owned()),
        }
    }

    fn evidence_with(
        project_id: &ProjectId,
        provider_id: &str,
        detail_level: ProtocolDetailLevel,
        completeness: ExternalAnalysisCompleteness,
    ) -> ExternalAnalysisEvidenceV1 {
        let executed = completeness != ExternalAnalysisCompleteness::Unavailable;
        ExternalAnalysisEvidenceV1 {
            schema_id: EXTERNAL_ANALYSIS_EVIDENCE_V1_SCHEMA_ID.to_owned(),
            schema_version: 1,
            evidence_id: format!("evidence-{provider_id}"),
            project_id: project_id.clone(),
            provider_id: provider_id.to_owned(),
            provider_version: executed.then(|| "1.2.3".to_owned()),
            executable_sha256: executed.then(|| hash("executable")),
            protocol: protocol(detail_level),
            config_fingerprint: hash("config"),
            input_fingerprint: hash("input"),
            source_fingerprint: hash("source"),
            environment_fingerprint: hash("environment"),
            raw_artifact_ref: executed.then(|| artifact(project_id, provider_id)),
            normalized_artifact_ref: None,
            exit_code: executed.then_some(0),
            started_at: observed_at(),
            completed_at: observed_at() + Duration::seconds(1),
            completeness,
            limitations: (!completeness.is_terminal_success_evidence())
                .then(|| format!("provider completeness is {completeness:?}"))
                .into_iter()
                .collect(),
            content_fingerprint: hash("placeholder"),
        }
        .seal()
        .expect("valid evidence")
    }

    #[test]
    fn unavailable_evidence_cannot_claim_execution_identity() {
        let project_id = ProjectId::new();
        let evidence = evidence_with(
            &project_id,
            "missing-provider",
            ProtocolDetailLevel::Structured,
            ExternalAnalysisCompleteness::Unavailable,
        );
        evidence
            .validate()
            .expect("unavailable evidence is explicit");

        let mut invalid = evidence;
        invalid.executable_sha256 = Some(hash("invented executable"));
        invalid.content_fingerprint = invalid.expected_fingerprint().expect("fingerprint");
        assert_eq!(
            invalid.validate(),
            Err(ExternalAnalysisContractError::UnavailableClaim)
        );

        let mut invalid = evidence_with(
            &project_id,
            "missing-provider",
            ProtocolDetailLevel::Structured,
            ExternalAnalysisCompleteness::Unavailable,
        );
        invalid.provider_version = Some("invented-version".to_owned());
        invalid.content_fingerprint = invalid.expected_fingerprint().expect("fingerprint");
        assert_eq!(
            invalid.validate(),
            Err(ExternalAnalysisContractError::UnavailableClaim)
        );
    }

    #[test]
    fn normalized_evidence_is_project_bound_and_links_the_exact_raw_artifact() {
        let project_id = ProjectId::new();
        let mut evidence = evidence_with(
            &project_id,
            "structured-provider",
            ProtocolDetailLevel::Structured,
            ExternalAnalysisCompleteness::Complete,
        );
        let raw = evidence.raw_artifact_ref.clone().expect("raw artifact");
        let mut normalized = artifact(&project_id, "structured-provider-normalized");
        normalized.source_artifact_ref = Some(Box::new(raw));
        evidence.normalized_artifact_ref = Some(normalized);
        evidence = evidence.seal().expect("exact raw linkage is valid");

        let mut cross_project = evidence.clone();
        cross_project
            .normalized_artifact_ref
            .as_mut()
            .expect("normalized artifact")
            .project_id = Some(ProjectId::new());
        cross_project.content_fingerprint = cross_project.expected_fingerprint().unwrap();
        assert_eq!(
            cross_project.validate(),
            Err(ExternalAnalysisContractError::CrossSubjectEvidence)
        );

        let mut unlinked = evidence;
        unlinked
            .normalized_artifact_ref
            .as_mut()
            .expect("normalized artifact")
            .source_artifact_ref = None;
        unlinked.content_fingerprint = unlinked.expected_fingerprint().unwrap();
        assert_eq!(
            unlinked.validate(),
            Err(ExternalAnalysisContractError::InvalidArtifact)
        );
    }

    #[test]
    fn coverage_gates_changed_executable_lines_and_keeps_branches_shadow_only() {
        let project_id = ProjectId::new();
        let evidence = evidence_with(
            &project_id,
            "cargo-llvm-cov",
            ProtocolDetailLevel::Structured,
            ExternalAnalysisCompleteness::Complete,
        );
        let mut observation = CoverageObservationV1 {
            schema_id: COVERAGE_OBSERVATION_V1_SCHEMA_ID.to_owned(),
            schema_version: 1,
            observation_id: "coverage-1".to_owned(),
            project_id,
            changed_scope_fingerprint: hash("input"),
            evidence,
            changed_line_count: 12,
            executable_line_count: 10,
            covered_executable_line_count: 8,
            uncovered_executable_line_count: 2,
            excluded_line_count: 2,
            line_coverage_basis_points: 8_000,
            required_line_coverage_basis_points: 8_000,
            branch_shadow: Some(BranchCoverageShadowV1 {
                measured_branches: 4,
                covered_branches: 3,
                gate_eligible: false,
            }),
            gate_state: CoverageGateStateV1::Pass,
            limitations: vec!["branch coverage is shadow-only".to_owned()],
        };
        observation.validate().expect("line coverage can gate");
        observation
            .branch_shadow
            .as_mut()
            .expect("shadow")
            .gate_eligible = true;
        assert_eq!(
            observation.validate(),
            Err(ExternalAnalysisContractError::UnstableBranchGate)
        );
    }

    #[test]
    fn flaky_requires_mixed_results_for_the_exact_same_input_and_environment() {
        let project_id = ProjectId::new();
        let input = hash("input");
        let environment = hash("environment");
        let evidence = evidence_with(
            &project_id,
            "cargo-nextest",
            ProtocolDetailLevel::Structured,
            ExternalAnalysisCompleteness::Flaky,
        );
        let mut observation = FlakyTestObservationV1 {
            schema_id: FLAKY_TEST_OBSERVATION_V1_SCHEMA_ID.to_owned(),
            schema_version: 1,
            observation_id: "flaky-1".to_owned(),
            project_id,
            evidence,
            test_id: "tests::sometimes_fails".to_owned(),
            input_fingerprint: input.clone(),
            environment_fingerprint: environment.clone(),
            attempts: vec![
                TestAttemptObservationV1 {
                    attempt_id: "attempt-1".to_owned(),
                    input_fingerprint: input.clone(),
                    environment_fingerprint: environment.clone(),
                    outcome: TestAttemptOutcomeV1::Fail,
                },
                TestAttemptObservationV1 {
                    attempt_id: "attempt-2".to_owned(),
                    input_fingerprint: input,
                    environment_fingerprint: environment,
                    outcome: TestAttemptOutcomeV1::Pass,
                },
            ],
            classification: FlakyTestClassificationV1::Flaky,
            retry_success_promoted: false,
            quarantine: Some(FlakyQuarantineV1 {
                owner: "team-code-health".to_owned(),
                approval_ref: "approval-1".to_owned(),
                expires_at: observed_at() + Duration::days(7),
            }),
            limitations: Vec::new(),
        };
        observation
            .validate()
            .expect("mixed exact attempts are flaky");
        observation.retry_success_promoted = true;
        assert_eq!(
            observation.validate(),
            Err(ExternalAnalysisContractError::InvalidFlakyObservation)
        );

        let mut incomplete = observation;
        incomplete.retry_success_promoted = false;
        incomplete.evidence = evidence_with(
            &incomplete.project_id,
            "cargo-nextest",
            ProtocolDetailLevel::Structured,
            ExternalAnalysisCompleteness::Partial,
        );
        incomplete.input_fingerprint = incomplete.evidence.input_fingerprint.clone();
        incomplete.environment_fingerprint = incomplete.evidence.environment_fingerprint.clone();
        incomplete.attempts = vec![TestAttemptObservationV1 {
            attempt_id: "attempt-partial".to_owned(),
            input_fingerprint: incomplete.input_fingerprint.clone(),
            environment_fingerprint: incomplete.environment_fingerprint.clone(),
            outcome: TestAttemptOutcomeV1::Pass,
        }];
        incomplete.quarantine = None;
        incomplete.classification = FlakyTestClassificationV1::StablePass;
        assert_eq!(
            incomplete.validate(),
            Err(ExternalAnalysisContractError::InvalidFlakyClassification)
        );
        incomplete.classification = FlakyTestClassificationV1::Inconclusive;
        incomplete
            .validate()
            .expect("partial evidence remains inconclusive");
    }

    #[test]
    fn exit_only_cargo_semver_output_cannot_invent_machine_detail() {
        let project_id = ProjectId::new();
        let observation = CompatibilityProviderObservationV1 {
            provider_kind: CompatibilityProviderKindV1::CargoSemverChecks,
            evidence: evidence_with(
                &project_id,
                "cargo-semver-checks",
                ProtocolDetailLevel::ExitClassification,
                ExternalAnalysisCompleteness::Complete,
            ),
            classification: CompatibilityClass::Breaking,
            machine_detail_count: 1,
            raw_exit_classification: Some("nonzero".to_owned()),
            limitations: Vec::new(),
        };
        assert_eq!(
            observation.validate(),
            Err(ExternalAnalysisContractError::InventedProviderDetail)
        );
    }

    #[test]
    fn human_text_compatibility_output_remains_unknown_and_cannot_drive_a_report() {
        let project_id = ProjectId::new();
        let mut evidence = evidence_with(
            &project_id,
            "libabigail",
            ProtocolDetailLevel::RawOnly,
            ExternalAnalysisCompleteness::Complete,
        );
        evidence.protocol.stability = ProtocolStability::HumanText;
        evidence.protocol.machine_readable = false;
        evidence.protocol.media_type = "text/plain".to_owned();
        evidence.protocol.schema_uri = None;
        evidence = evidence
            .seal()
            .expect("raw human evidence remains retainable");
        let mut observation = CompatibilityProviderObservationV1 {
            provider_kind: CompatibilityProviderKindV1::Libabigail,
            evidence,
            classification: CompatibilityClass::Unknown,
            machine_detail_count: 0,
            raw_exit_classification: None,
            limitations: vec!["human output is retained without sentence parsing".to_owned()],
        };
        observation
            .validate()
            .expect("raw human evidence can remain explicitly unknown");
        observation.classification = CompatibilityClass::Breaking;
        assert_eq!(
            observation.validate(),
            Err(ExternalAnalysisContractError::InventedProviderDetail)
        );
    }

    #[test]
    fn not_affected_requires_reviewed_vex_and_reachability() {
        let project_id = ProjectId::new();
        let observation = SupplyChainProviderObservationV1 {
            provider_kind: SupplyChainProviderKindV1::Vex,
            evidence: evidence_with(
                &project_id,
                "cyclonedx-vex",
                ProtocolDetailLevel::Structured,
                ExternalAnalysisCompleteness::Complete,
            ),
            standards: vec!["CycloneDX 1.6".to_owned()],
            license_expressions: Vec::new(),
            reachability: ReachabilityStateV1::NotAnalyzed,
            vex_state: VexStateV1::NotAffected,
            advisory_freshness: None,
            reviewed_vex_evidence_ref: None,
            limitations: Vec::new(),
        };
        assert_eq!(
            observation.validate(),
            Err(ExternalAnalysisContractError::UnreviewedNotAffected)
        );
    }

    #[test]
    fn supply_chain_normalization_rejects_human_text_protocols() {
        let project_id = ProjectId::new();
        let mut evidence = evidence_with(
            &project_id,
            "syft",
            ProtocolDetailLevel::RawOnly,
            ExternalAnalysisCompleteness::Complete,
        );
        evidence.protocol.stability = ProtocolStability::HumanText;
        evidence.protocol.machine_readable = false;
        evidence.protocol.media_type = "text/plain".to_owned();
        evidence.protocol.schema_uri = None;
        evidence = evidence.seal().expect("raw evidence remains retainable");
        let observation = SupplyChainProviderObservationV1 {
            provider_kind: SupplyChainProviderKindV1::Sbom,
            evidence,
            standards: Vec::new(),
            license_expressions: Vec::new(),
            reachability: ReachabilityStateV1::NotAnalyzed,
            vex_state: VexStateV1::None,
            advisory_freshness: None,
            reviewed_vex_evidence_ref: None,
            limitations: vec!["human output is not normalized".to_owned()],
        };
        assert_eq!(
            observation.validate(),
            Err(ExternalAnalysisContractError::UnstableNormalization)
        );
    }

    #[test]
    fn advisory_freshness_is_source_snapshot_bound_and_fail_closed() {
        let project_id = ProjectId::new();
        let evidence = evidence_with(
            &project_id,
            "cargo-audit",
            ProtocolDetailLevel::Structured,
            ExternalAnalysisCompleteness::Complete,
        );
        let mut observation = SupplyChainProviderObservationV1 {
            provider_kind: SupplyChainProviderKindV1::Advisory,
            advisory_freshness: Some(AdvisoryFreshnessObservationV1 {
                database_fingerprint: Some(hash("rustsec-database")),
                database_updated_at: Some(evidence.completed_at - Duration::hours(1)),
                observed_at: evidence.completed_at,
                maximum_age_seconds: 86_400,
                freshness: ExternalFreshness::Current,
            }),
            evidence,
            standards: Vec::new(),
            license_expressions: Vec::new(),
            reachability: ReachabilityStateV1::NotAnalyzed,
            vex_state: VexStateV1::None,
            reviewed_vex_evidence_ref: None,
            limitations: Vec::new(),
        };
        observation.validate().expect("current advisory database");
        observation
            .advisory_freshness
            .as_mut()
            .expect("freshness")
            .database_updated_at = Some(observation.evidence.completed_at - Duration::days(2));
        assert_eq!(
            observation.validate(),
            Err(ExternalAnalysisContractError::InvalidSupplyChainObservation)
        );
    }

    #[test]
    fn reproducibility_pass_requires_equal_artifacts_and_all_slsa_bindings() {
        let project_id = ProjectId::new();
        let evidence = evidence_with(
            &project_id,
            "reproducible-build",
            ProtocolDetailLevel::Structured,
            ExternalAnalysisCompleteness::Complete,
        );
        let artifact_digest = BuildArtifactDigestV1 {
            logical_name: "star.exe".to_owned(),
            sha256: hash("binary"),
        };
        let slsa_provenance_ref = evidence.raw_artifact_ref.clone();
        let mut report = ReproducibilityVerificationReportV1 {
            schema_id: REPRODUCIBILITY_VERIFICATION_REPORT_V1_SCHEMA_ID.to_owned(),
            schema_version: 1,
            report_id: "repro-1".to_owned(),
            project_id,
            source_fingerprint: evidence.source_fingerprint.clone(),
            config_fingerprint: evidence.config_fingerprint.clone(),
            toolchain_fingerprint: hash("toolchain"),
            builds: vec![
                IsolatedBuildObservationV1 {
                    root_id: "root-a".to_owned(),
                    root_fingerprint: hash("root-a"),
                    artifacts: vec![artifact_digest.clone()],
                },
                IsolatedBuildObservationV1 {
                    root_id: "root-b".to_owned(),
                    root_fingerprint: hash("root-b"),
                    artifacts: vec![artifact_digest],
                },
            ],
            provider_evidence: vec![evidence],
            slsa_provenance_ref,
            slsa_subject_bound: true,
            slsa_materials_bound: true,
            slsa_builder_bound: true,
            diffoscope_diagnostic_ref: None,
            outcome: ReproducibilityOutcomeV1::Pass,
            limitations: Vec::new(),
        };
        report.validate().expect("exactly reproducible builds pass");
        let mut human_report = report.clone();
        human_report.provider_evidence[0].protocol.stability = ProtocolStability::HumanText;
        human_report.provider_evidence[0].protocol.detail_level = ProtocolDetailLevel::RawOnly;
        human_report.provider_evidence[0].protocol.machine_readable = false;
        human_report.provider_evidence[0].protocol.media_type = "text/plain".to_owned();
        human_report.provider_evidence[0].protocol.schema_uri = None;
        human_report.provider_evidence[0] = human_report.provider_evidence[0]
            .clone()
            .seal()
            .expect("raw diagnostic evidence remains retainable");
        assert_eq!(
            human_report.validate(),
            Err(ExternalAnalysisContractError::UnstableNormalization)
        );
        let verified_provenance_ref = report.slsa_provenance_ref.take();
        assert_eq!(
            report.validate(),
            Err(ExternalAnalysisContractError::InvalidReproducibility)
        );
        report.slsa_provenance_ref = verified_provenance_ref;
        report.slsa_builder_bound = false;
        assert_eq!(
            report.validate(),
            Err(ExternalAnalysisContractError::InvalidReproducibility)
        );
        report.outcome = ReproducibilityOutcomeV1::Unverified;
        report
            .validate()
            .expect("missing SLSA binding remains unverified, not mismatched");

        report.slsa_builder_bound = true;
        report.builds[1].artifacts[0].sha256 = hash("different-binary");
        report.outcome = ReproducibilityOutcomeV1::Mismatch;
        report
            .validate()
            .expect("complete distinct artifact digests are a mismatch");

        let mut partial_evidence = report.provider_evidence[0].clone();
        partial_evidence.completeness = ExternalAnalysisCompleteness::Partial;
        partial_evidence.limitations = vec!["one isolated build was incomplete".to_owned()];
        report.provider_evidence[0] = partial_evidence.seal().expect("partial evidence");
        assert_eq!(
            report.validate(),
            Err(ExternalAnalysisContractError::InvalidReproducibility)
        );
        report.outcome = ReproducibilityOutcomeV1::Unverified;
        report
            .validate()
            .expect("partial evidence cannot claim an artifact mismatch");
    }

    #[test]
    fn undeclared_runtime_provider_is_unavailable_and_near_clone_is_advisory_only() {
        let project_id = ProjectId::new();
        let runtime = RuntimeSafetyObservationV1 {
            schema_id: RUNTIME_SAFETY_OBSERVATION_V1_SCHEMA_ID.to_owned(),
            schema_version: 1,
            observation_id: "runtime-1".to_owned(),
            project_id: project_id.clone(),
            provider_kind: RuntimeSafetyProviderKindV1::Loom,
            evidence: evidence_with(
                &project_id,
                "loom",
                ProtocolDetailLevel::Structured,
                ExternalAnalysisCompleteness::Unavailable,
            ),
            target: "workspace".to_owned(),
            project_declares_toolchain_or_test: false,
            timeout_ms: 60_000,
            cancelled: false,
            finding_refs: Vec::new(),
            state: RuntimeSafetyStateV1::Unavailable,
            limitations: vec!["project does not declare a Loom test".to_owned()],
        };
        runtime
            .validate()
            .expect("undeclared provider is unavailable");
        let mut unavailable_with_findings = runtime.clone();
        unavailable_with_findings.finding_refs = vec!["finding-from-no-run".to_owned()];
        assert_eq!(
            unavailable_with_findings.validate(),
            Err(ExternalAnalysisContractError::InvalidRuntimeSafety)
        );

        let mut near_clone = NearCloneObservationV1 {
            schema_id: NEAR_CLONE_OBSERVATION_V1_SCHEMA_ID.to_owned(),
            schema_version: 1,
            observation_id: "clone-1".to_owned(),
            project_id,
            source_fingerprint: hash("source"),
            algorithm: "identifier_literal_normalized_simhash_v1".to_owned(),
            minimum_similarity_basis_points: 8_500,
            max_candidates: 100,
            max_source_bytes: 1024 * 1024,
            pairs: Vec::new(),
            advisory_only: true,
            automatic_patch_set_generated: false,
            limitations: Vec::new(),
        };
        near_clone.validate().expect("bounded advisory is valid");
        near_clone.automatic_patch_set_generated = true;
        assert_eq!(
            near_clone.validate(),
            Err(ExternalAnalysisContractError::InvalidNearClone)
        );
    }
}
