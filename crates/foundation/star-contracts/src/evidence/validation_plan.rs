//! Immutable validation-selection and cache-key contract.
//!
//! This v1 surface is deliberately bounded to the tracked-path precursor. It
//! records what the planner selected; a runner must consume the selected
//! checks without reclassifying the change.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::ValidationRunRef;
use crate::{Sha256Hash, canonical::CanonicalError, canonical_sha256, ids::ValidationPlanId};

pub const VALIDATION_PLAN_SCHEMA_ID: &str = "star.validation-plan";
pub const VALIDATION_PLAN_SCHEMA_VERSION: u32 = 1;
pub const VALIDATION_POLICY_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ValidationPlanSchemaId {
    #[default]
    #[serde(rename = "star.validation-plan")]
    ValidationPlan,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ValidationCapabilityLevel {
    TrackedPathPrecursor,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ValidationProfile {
    Quick,
    Target,
    Full,
    Release,
}

impl ValidationProfile {
    pub const fn rank(self) -> u8 {
        match self {
            Self::Quick => 0,
            Self::Target => 1,
            Self::Full => 2,
            Self::Release => 3,
        }
    }

    pub const fn max(self, other: Self) -> Self {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }
}

pub const fn resolve_validation_profile(
    required: ValidationProfile,
    requested: Option<ValidationProfile>,
) -> ValidationProfile {
    match requested {
        Some(ValidationProfile::Full) => required.max(ValidationProfile::Full),
        Some(ValidationProfile::Release) => ValidationProfile::Release,
        Some(ValidationProfile::Quick | ValidationProfile::Target) | None => required,
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ValidationChangeSource {
    Staged,
    Unstaged,
    Untracked,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ValidationChangeClass {
    Documentation,
    InternalCode,
    PublicContract,
    Configuration,
    Toolchain,
    Lockfile,
    ValidatorPolicy,
    Security,
    DataMigration,
    WorkflowRelease,
    Unknown,
}

impl ValidationChangeClass {
    pub const fn required_profile(self) -> ValidationProfile {
        match self {
            Self::Documentation | Self::Configuration => ValidationProfile::Quick,
            Self::InternalCode => ValidationProfile::Target,
            Self::PublicContract
            | Self::Toolchain
            | Self::Lockfile
            | Self::ValidatorPolicy
            | Self::Security
            | Self::DataMigration
            | Self::WorkflowRelease
            | Self::Unknown => ValidationProfile::Full,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidationChangedFile {
    pub path: String,
    pub sources: Vec<ValidationChangeSource>,
    pub change_class: ValidationChangeClass,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_unit: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AffectedUnitKind {
    Unit,
    Workspace,
    Project,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AffectedUnit {
    pub unit_id: String,
    pub kind: AffectedUnitKind,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReverseConsumer {
    pub provider_unit_id: String,
    pub consumer_unit_id: String,
    pub dependency_path: Vec<String>,
    pub reason: String,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ProfileReasonCode {
    ExplicitQuick,
    ExplicitTarget,
    ExplicitFull,
    ExplicitRelease,
    DocumentationOnly,
    InternalUnitChange,
    PublicContractChange,
    ValidatorOrPolicyChange,
    ToolchainOrLockfileChange,
    SecurityOrDataChange,
    WorkflowOrReleaseChange,
    MissingUnitMapping,
    ImpactUncertain,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidationProfileSelection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested: Option<ValidationProfile>,
    pub required: ValidationProfile,
    pub selected: ValidationProfile,
    pub reasons: Vec<ProfileReasonCode>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidationCommand {
    pub executable: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub working_directory: String,
    pub expected_exit_codes: BTreeSet<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidationInputFingerprintComponents {
    pub revision: String,
    pub staged_diff: Sha256Hash,
    pub unstaged_diff: Sha256Hash,
    pub untracked_content: Sha256Hash,
    pub toolchain: Sha256Hash,
    pub lockfile: Sha256Hash,
    pub project_manifest: Sha256Hash,
    pub validation_scripts: Sha256Hash,
    pub config: Sha256Hash,
    pub policy_schema_version: u32,
    pub evidence_schema_version: u32,
}

impl ValidationInputFingerprintComponents {
    pub fn fingerprint(&self) -> Result<Sha256Hash, ValidationFingerprintError> {
        let value = serde_json::to_value(self)?;
        Ok(canonical_sha256(&value)?)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidationCacheKeyInputs {
    pub inputs: ValidationInputFingerprintComponents,
    pub command: Sha256Hash,
}

impl ValidationCacheKeyInputs {
    pub fn fingerprint(&self) -> Result<Sha256Hash, ValidationFingerprintError> {
        let value = serde_json::to_value(self)?;
        Ok(canonical_sha256(&value)?)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlannedCheckDisposition {
    Execute,
    Reuse,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlannedCheck {
    pub check_id: String,
    pub unit_id: String,
    pub command: ValidationCommand,
    pub disposition: PlannedCheckDisposition,
    pub selection_reason: String,
    pub cache_key_inputs: ValidationCacheKeyInputs,
    pub cache_key: Sha256Hash,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_validation_run_ref: Option<ValidationRunRef>,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ValidationUncertaintyCode {
    ImpactUnavailable,
    UnitMappingMissing,
    ReverseConsumerGraphIncomplete,
    UntrackedContentUnavailable,
    FingerprintInputUnavailable,
    ToolchainUnavailable,
    ReleaseEnvironmentUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ValidationEscalation {
    ExpandToWorkspace,
    ExpandToProjectFull,
    HumanReview,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidationUncertainty {
    pub code: ValidationUncertaintyCode,
    pub summary: String,
    pub escalation: ValidationEscalation,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum IndependentReviewTrigger {
    Security,
    DataMigration,
    PublicContract,
    Release,
    RepeatedFailure,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IndependentReviewRequirement {
    pub required: bool,
    #[serde(default)]
    pub triggers: Vec<IndependentReviewTrigger>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ValidationPlanReadiness {
    Ready,
    HumanReview,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceFlowStage {
    ValidationPlan,
    ValidationRunDiagnosticResult,
    GateDecision,
    EvidenceBundle,
    AiCompressedSummary,
}

pub const EVIDENCE_FLOW: [EvidenceFlowStage; 5] = [
    EvidenceFlowStage::ValidationPlan,
    EvidenceFlowStage::ValidationRunDiagnosticResult,
    EvidenceFlowStage::GateDecision,
    EvidenceFlowStage::EvidenceBundle,
    EvidenceFlowStage::AiCompressedSummary,
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidationPlan {
    pub schema_id: ValidationPlanSchemaId,
    #[schemars(range(min = 1, max = 1))]
    pub schema_version: u32,
    pub validation_plan_id: ValidationPlanId,
    pub capability_level: ValidationCapabilityLevel,
    pub project_key: String,
    pub revision: String,
    #[serde(default)]
    pub changed_files: Vec<ValidationChangedFile>,
    pub direct_units: Vec<AffectedUnit>,
    #[serde(default)]
    pub reverse_consumers: Vec<ReverseConsumer>,
    pub profile: ValidationProfileSelection,
    pub checks: Vec<PlannedCheck>,
    #[serde(default)]
    pub uncertainties: Vec<ValidationUncertainty>,
    pub independent_review: IndependentReviewRequirement,
    pub readiness: ValidationPlanReadiness,
    pub evidence_flow: Vec<EvidenceFlowStage>,
    pub input_fingerprint: Sha256Hash,
    pub plan_fingerprint: Sha256Hash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationPlanInvariantError {
    #[error("validation plan schema version is not supported")]
    SchemaVersion,
    #[error("validation plan contains an empty required value")]
    EmptyValue,
    #[error("validation plan collections are not sorted and unique")]
    Ordering,
    #[error("validation profile selection is inconsistent")]
    Profile,
    #[error("validation cache key is inconsistent")]
    CacheKey,
    #[error("validation cache disposition and source reference disagree")]
    CacheSource,
    #[error("validation evidence flow is not canonical")]
    EvidenceFlow,
    #[error("validation independent review requirement is inconsistent")]
    IndependentReview,
    #[error("validation plan readiness does not match its uncertainties")]
    Readiness,
    #[error("validation plan fingerprint or identifier is inconsistent")]
    Fingerprint,
}

#[derive(Debug, Error)]
pub enum ValidationFingerprintError {
    #[error("validation fingerprint serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("validation fingerprint canonicalization failed: {0}")]
    Canonical(#[from] CanonicalError),
}

impl ValidationPlan {
    pub fn seal(mut self) -> Result<Self, ValidationPlanInvariantError> {
        let fingerprint = self
            .expected_plan_fingerprint()
            .map_err(|_| ValidationPlanInvariantError::Fingerprint)?;
        self.validation_plan_id =
            ValidationPlanId::from_stable_bytes(fingerprint.as_str().as_bytes());
        self.plan_fingerprint = fingerprint;
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), ValidationPlanInvariantError> {
        if self.schema_version != VALIDATION_PLAN_SCHEMA_VERSION {
            return Err(ValidationPlanInvariantError::SchemaVersion);
        }
        if self.project_key.trim().is_empty()
            || self.revision.trim().is_empty()
            || self.direct_units.is_empty()
            || self.checks.is_empty()
            || self.profile.reasons.is_empty()
        {
            return Err(ValidationPlanInvariantError::EmptyValue);
        }
        if !sorted_unique_by(&self.changed_files, |item| item.path.as_str())
            || !sorted_unique_by(&self.direct_units, |item| item.unit_id.as_str())
            || !sorted_unique_by(&self.checks, |item| item.check_id.as_str())
            || self.reverse_consumers.windows(2).any(|pair| {
                (&pair[0].provider_unit_id, &pair[0].consumer_unit_id)
                    >= (&pair[1].provider_unit_id, &pair[1].consumer_unit_id)
            })
            || !sorted_unique_copy(&self.profile.reasons)
            || !sorted_unique_copy(&self.independent_review.triggers)
            || !sorted_unique_copy_by(&self.uncertainties, |item| item.code)
            || self.changed_files.iter().any(|file| {
                file.path.trim().is_empty()
                    || file.path.contains('\\')
                    || file.sources.is_empty()
                    || file
                        .direct_unit
                        .as_deref()
                        .is_some_and(|unit| unit.trim().is_empty())
                    || !sorted_unique_copy(&file.sources)
            })
            || self
                .direct_units
                .iter()
                .any(|unit| unit.unit_id.trim().is_empty() || unit.reason.trim().is_empty())
            || self.reverse_consumers.iter().any(|consumer| {
                consumer.provider_unit_id.trim().is_empty()
                    || consumer.consumer_unit_id.trim().is_empty()
                    || consumer.reason.trim().is_empty()
                    || consumer.dependency_path.first() != Some(&consumer.provider_unit_id)
                    || consumer.dependency_path.last() != Some(&consumer.consumer_unit_id)
            })
            || self
                .uncertainties
                .iter()
                .any(|uncertainty| uncertainty.summary.trim().is_empty())
        {
            return Err(ValidationPlanInvariantError::Ordering);
        }
        let affected_units: BTreeSet<_> = self
            .direct_units
            .iter()
            .map(|unit| unit.unit_id.as_str())
            .collect();
        if self.changed_files.iter().any(|file| {
            file.direct_unit
                .as_deref()
                .is_some_and(|unit| !affected_units.contains(unit))
        }) || self.reverse_consumers.iter().any(|consumer| {
            !affected_units.contains(consumer.provider_unit_id.as_str())
                || !affected_units.contains(consumer.consumer_unit_id.as_str())
        }) {
            return Err(ValidationPlanInvariantError::Ordering);
        }
        if self.profile.selected
            != resolve_validation_profile(self.profile.required, self.profile.requested)
        {
            return Err(ValidationPlanInvariantError::Profile);
        }
        for check in &self.checks {
            if check.check_id.trim().is_empty()
                || check.unit_id.trim().is_empty()
                || check.command.executable.trim().is_empty()
                || check.command.working_directory.trim().is_empty()
                || check.command.expected_exit_codes.is_empty()
                || check.selection_reason.trim().is_empty()
                || check
                    .cache_key_inputs
                    .fingerprint()
                    .map_err(|_| ValidationPlanInvariantError::CacheKey)?
                    != check.cache_key
            {
                return Err(ValidationPlanInvariantError::CacheKey);
            }
            let source_matches = match check.disposition {
                PlannedCheckDisposition::Execute => check.source_validation_run_ref.is_none(),
                PlannedCheckDisposition::Reuse => check.source_validation_run_ref.is_some(),
            };
            if !source_matches {
                return Err(ValidationPlanInvariantError::CacheSource);
            }
        }
        let inputs = &self.checks[0].cache_key_inputs.inputs;
        if inputs.revision != self.revision
            || inputs
                .fingerprint()
                .map_err(|_| ValidationPlanInvariantError::Fingerprint)?
                != self.input_fingerprint
            || self
                .checks
                .iter()
                .any(|check| &check.cache_key_inputs.inputs != inputs)
        {
            return Err(ValidationPlanInvariantError::Fingerprint);
        }
        if self.evidence_flow != EVIDENCE_FLOW {
            return Err(ValidationPlanInvariantError::EvidenceFlow);
        }
        if self.independent_review.required == self.independent_review.triggers.is_empty() {
            return Err(ValidationPlanInvariantError::IndependentReview);
        }
        let human_review = self
            .uncertainties
            .iter()
            .any(|item| item.escalation == ValidationEscalation::HumanReview);
        if (self.readiness == ValidationPlanReadiness::Ready && human_review)
            || (self.readiness == ValidationPlanReadiness::HumanReview && !human_review)
        {
            return Err(ValidationPlanInvariantError::Readiness);
        }
        let expected = self
            .expected_plan_fingerprint()
            .map_err(|_| ValidationPlanInvariantError::Fingerprint)?;
        if self.plan_fingerprint != expected
            || self.validation_plan_id
                != ValidationPlanId::from_stable_bytes(expected.as_str().as_bytes())
        {
            return Err(ValidationPlanInvariantError::Fingerprint);
        }
        Ok(())
    }

    fn expected_plan_fingerprint(&self) -> Result<Sha256Hash, ValidationFingerprintError> {
        let mut value = serde_json::to_value(self)?;
        if let Some(object) = value.as_object_mut() {
            object.remove("validation_plan_id");
            object.remove("plan_fingerprint");
        }
        Ok(canonical_sha256(&value)?)
    }
}

fn sorted_unique_by<T>(items: &[T], value: impl Fn(&T) -> &str) -> bool {
    items
        .windows(2)
        .all(|pair| value(&pair[0]) < value(&pair[1]))
}

fn sorted_unique_copy<T: Copy + Ord>(items: &[T]) -> bool {
    items.windows(2).all(|pair| pair[0] < pair[1])
}

fn sorted_unique_copy_by<T, K: Copy + Ord>(items: &[T], value: impl Fn(&T) -> K) -> bool {
    items
        .windows(2)
        .all(|pair| value(&pair[0]) < value(&pair[1]))
}
