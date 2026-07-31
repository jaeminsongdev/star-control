//! Product-grade M7 failure, reproduction, security, dependency, and maintenance contracts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::development_v2::CoverageState;
use crate::external_analysis::{ArchitectureRuleV1, SupplyChainProviderObservationV1};
use crate::management::ProjectPathRef;
use crate::{EvaluationRunId, ProjectId, Sha256Hash};

pub const FAILURE_RECORD_SCHEMA_ID: &str = "star.failure-record";
pub const REPRODUCTION_ATTEMPT_OBSERVATION_V1_SCHEMA_ID: &str =
    "star.reproduction-attempt-observation";
pub const REPRODUCTION_PACK_V2_SCHEMA_ID: &str = "star.reproduction-pack";
pub const REGRESSION_RECORD_SCHEMA_ID: &str = "star.regression-record";
pub const RECOVERY_PLAN_V2_SCHEMA_ID: &str = "star.recovery-plan";
pub const DEPENDENCY_SNAPSHOT_SCHEMA_ID: &str = "star.dependency-snapshot";
pub const SUPPLY_CHAIN_SNAPSHOT_SCHEMA_ID: &str = "star.supply-chain-snapshot";
pub const EXTERNAL_DATA_SNAPSHOT_SCHEMA_ID: &str = "star.external-data-snapshot";
pub const STATIC_ANALYSIS_IMPORT_REPORT_SCHEMA_ID: &str = "star.static-analysis-import-report";
pub const GIT_HISTORY_RISK_SNAPSHOT_SCHEMA_ID: &str = "star.git-history-risk-snapshot";
pub const MUTATION_TESTING_SNAPSHOT_SCHEMA_ID: &str = "star.mutation-testing-snapshot";
pub const QUALITY_RULE_PACK_MANIFEST_SCHEMA_ID: &str = "star.quality-rule-pack-manifest";
pub const DEPENDENCY_UPDATE_PLAN_SCHEMA_ID: &str = "star.dependency-update-plan";
pub const MAINTENANCE_RADAR_SNAPSHOT_SCHEMA_ID: &str = "star.maintenance-radar-snapshot";

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    Compile,
    Test,
    Runtime,
    Tool,
    Environment,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FailureCausalityRole {
    RootCandidate,
    Cascade,
    Independent,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VerificationState {
    Verified,
    PartiallyVerified,
    Unverified,
    Contradicted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FailureSubjectBinding {
    pub project_id: ProjectId,
    pub checkout_ref: String,
    pub workspace_snapshot_ref: String,
    pub project_revision_ref: String,
    pub change_set_ref: Option<String>,
    pub validation_run_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrimarySymptom {
    pub producer_code: String,
    pub message_template: String,
    pub logical_owner: String,
    pub signature: String,
    pub normalization_version: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FailureInvocation {
    pub command_descriptor: String,
    pub executable_identity: String,
    pub structured_args: Vec<String>,
    pub logical_cwd: String,
    pub timeout_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RootCandidateRef {
    pub failure_record_ref: String,
    pub confidence: f64,
    pub reason: String,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FailureRecord {
    pub schema_id: String,
    pub schema_version: u32,
    pub failure_record_id: String,
    pub occurrence_id: String,
    #[serde(default)]
    pub diagnostic_refs: Vec<String>,
    #[serde(default)]
    pub finding_refs: Vec<String>,
    pub subject_binding: FailureSubjectBinding,
    pub failure_kind: FailureKind,
    pub family_fingerprint: Sha256Hash,
    pub occurrence_fingerprint: Sha256Hash,
    pub primary_symptom: PrimarySymptom,
    pub causality_role: FailureCausalityRole,
    #[serde(default)]
    pub root_candidate_refs: Vec<RootCandidateRef>,
    #[serde(default)]
    pub cascade_parent_refs: Vec<String>,
    pub invocation: FailureInvocation,
    pub environment_compatibility_class: String,
    pub environment_fingerprint: Sha256Hash,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_fingerprint: Option<Sha256Hash>,
    #[serde(default)]
    pub input_refs: Vec<String>,
    pub seed: Option<String>,
    pub stdout_ref: Option<String>,
    pub stderr_ref: Option<String>,
    #[serde(default)]
    pub artifact_refs: Vec<String>,
    pub observed_at: String,
    pub attempt_id: String,
    pub verification_state: VerificationState,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReproductionResult {
    Reproduced,
    DifferentFailure,
    NotReproduced,
    BlockedExternal,
    Incomplete,
}

/// Normalized semantic result emitted by a registered reproduction adapter.
///
/// The Controller seals the canonical fingerprint of this value into a
/// `DevelopmentEffectReceiptV1`; a caller cannot promote an unexecuted attempt
/// to a complete reproduction outcome by merely constructing a pack payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReproductionAttemptObservationV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub attempt: u32,
    pub result: ReproductionResult,
    pub family_fingerprint: Option<Sha256Hash>,
    pub occurrence_fingerprint: Option<Sha256Hash>,
    pub environment_fingerprint: Sha256Hash,
    pub input_fingerprint: Sha256Hash,
    pub duration_ms: u64,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReproductionAttemptV2 {
    pub attempt: u32,
    pub result: ReproductionResult,
    pub family_fingerprint: Option<Sha256Hash>,
    pub occurrence_fingerprint: Option<Sha256Hash>,
    pub environment_fingerprint: Sha256Hash,
    pub input_fingerprint: Sha256Hash,
    pub duration_ms: u64,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub limitations: Vec<String>,
    /// Receipt for the terminal registered ToolInvocation that produced this
    /// semantic observation. Complete outcomes require this reference.
    #[serde(default)]
    pub effect_receipt_ref: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReproductionArtifactRef {
    pub artifact_ref: String,
    pub artifact_role: String,
    pub redaction_status: String,
    pub retention_class: String,
    pub safe_for_default_report: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReproductionPackV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub reproduction_pack_id: String,
    pub failure_record_ref: String,
    pub family_fingerprint: Sha256Hash,
    pub occurrence_fingerprint: Sha256Hash,
    pub subject_binding: FailureSubjectBinding,
    pub dirty_state: String,
    pub invocation: FailureInvocation,
    pub environment_compatibility_class: String,
    pub environment_fingerprint: Sha256Hash,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_fingerprint: Option<Sha256Hash>,
    #[serde(default)]
    pub manifest_refs: Vec<String>,
    #[serde(default)]
    pub input_refs: Vec<String>,
    pub seed: Option<String>,
    pub expected_result: String,
    pub observed_result: String,
    pub attempts: Vec<ReproductionAttemptV2>,
    #[serde(default)]
    pub artifacts: Vec<ReproductionArtifactRef>,
    pub result: ReproductionResult,
    pub completeness: CoverageState,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub pack_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RegressionState {
    Fixed,
    Recurring,
    Unverified,
    Contradicted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegressionRecord {
    pub schema_id: String,
    pub schema_version: u32,
    pub regression_record_id: String,
    pub family_fingerprint: Sha256Hash,
    pub before_failure_ref: String,
    pub after_validation_ref: String,
    pub after_subject_fingerprint: Sha256Hash,
    #[serde(default)]
    pub recurrence_failure_refs: Vec<String>,
    pub state: RegressionState,
    pub verification_state: VerificationState,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    pub record_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryKind {
    Rollback,
    RollForward,
    Restore,
    Rebuild,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryPlanState {
    Planned,
    AwaitingPermission,
    Ready,
    Blocked,
    Applied,
    Validated,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecoveryStepV2 {
    pub step_id: String,
    pub order: u32,
    pub action: String,
    pub destructive_effect: bool,
    pub permission_required: bool,
    #[serde(default)]
    pub prerequisite_step_ids: Vec<String>,
    pub expected_checkpoint: String,
    pub validation_check_ref: String,
    pub stop_condition: String,
    pub fallback_step_id: Option<String>,
    #[serde(default)]
    pub evidence_slots: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPlanV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub recovery_plan_id: String,
    pub project_id: ProjectId,
    pub failure_record_ref: String,
    pub recovery_kind: RecoveryKind,
    pub exact_subject_fingerprint: Sha256Hash,
    pub steps: Vec<RecoveryStepV2>,
    pub owner: String,
    pub state: RecoveryPlanState,
    #[serde(default)]
    pub blockers: Vec<String>,
    pub plan_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExternalFreshness {
    Current,
    Stale,
    Expired,
    Unknown,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalDataSourceDescriptor {
    pub source_id: String,
    pub source_kind: String,
    pub provider: String,
    #[serde(default)]
    pub source_uri: String,
    #[serde(default)]
    pub dataset_or_query: String,
    #[serde(default)]
    pub source_schema_version: String,
    #[serde(default)]
    pub tool_identity_ref: String,
    pub retrieval_mode: String,
    #[serde(default)]
    pub network_mode: String,
    pub integrity_policy: String,
    #[serde(default = "legacy_external_coverage")]
    pub coverage: CoverageState,
    pub maximum_age_seconds: u64,
    pub license_ref: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalDataObservation {
    pub subject: String,
    pub status: String,
    pub advisory_refs: Vec<String>,
    pub license_refs: Vec<String>,
    pub source_evidence_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalDataSnapshot {
    pub schema_id: String,
    pub schema_version: u32,
    pub snapshot_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<ProjectId>,
    pub source: ExternalDataSourceDescriptor,
    pub published_at: Option<String>,
    pub modified_at: Option<String>,
    pub retrieved_at: String,
    pub valid_until: String,
    pub evaluation_time: String,
    pub source_artifact_ref: String,
    pub source_sha256: Sha256Hash,
    #[serde(default)]
    pub normalized_artifact_ref: String,
    #[serde(default = "legacy_missing_external_fingerprint")]
    pub normalized_sha256: Sha256Hash,
    pub observations: Vec<ExternalDataObservation>,
    pub freshness: ExternalFreshness,
    pub completeness: CoverageState,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub content_fingerprint: Sha256Hash,
}

/// Immutable, source-bound receipt for a static-analysis import. The raw
/// provider document is referenced as an ArtifactRef outside this contract;
/// source text, absolute paths, and provider messages are never embedded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StaticAnalysisImportReport {
    pub schema_id: String,
    pub schema_version: u32,
    pub report_id: String,
    pub project_id: ProjectId,
    pub project_revision_ref: String,
    pub workspace_snapshot_ref: String,
    pub code_index_snapshot_ref: String,
    pub tool_descriptor_ref: String,
    pub tool_descriptor_sha256: Sha256Hash,
    pub tool_identity_sha256: Sha256Hash,
    pub sarif_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_pack_digest: Option<Sha256Hash>,
    pub uri_mapping_policy: String,
    pub raw_artifact_ref: String,
    pub normalized_artifact_ref: String,
    pub imported_count: u64,
    pub rejected_count: u64,
    pub truncated_count: u64,
    pub completeness: CoverageState,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub content_fingerprint: Sha256Hash,
}

impl StaticAnalysisImportReport {
    /// Accept only the current immutable report shape at an ingestion boundary.
    /// Serde's structural decoder intentionally remains able to read a future
    /// document, but product paths must never treat it as current evidence.
    pub fn is_current_schema(&self) -> bool {
        self.schema_id == STATIC_ANALYSIS_IMPORT_REPORT_SCHEMA_ID && self.schema_version == 1
    }
}

/// Why changed code is eligible for a bounded mutation run. The scope is
/// deliberately narrower than line coverage and never authorizes a whole-tree
/// mutation sweep.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum MutationTrigger {
    Parser,
    Protocol,
    PublicContract,
    CoreCalculation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MutationTestingBudget {
    pub max_mutants: u32,
    pub max_duration_ms: u64,
    pub max_survivors: u32,
}

/// A mutation run can be useful evidence without being a passing check. In
/// particular, timeout, flakiness, partial coverage, and provider absence are
/// never represented as `Complete`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MutationTestingState {
    Complete,
    TimedOut,
    Flaky,
    Partial,
    Unavailable,
    Unverified,
}

/// Immutable normalized result of a registered mutation engine. Source text,
/// raw engine output, and line-coverage contents remain in bounded artifacts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MutationTestingSnapshot {
    pub schema_id: String,
    pub schema_version: u32,
    pub snapshot_id: String,
    pub project_id: ProjectId,
    pub project_revision_ref: String,
    pub workspace_snapshot_ref: String,
    pub code_index_snapshot_ref: String,
    pub engine_descriptor_ref: String,
    pub engine_identity_sha256: Sha256Hash,
    pub changed_paths: Vec<ProjectPathRef>,
    pub triggers: Vec<MutationTrigger>,
    pub budget: MutationTestingBudget,
    pub executed_mutants: u32,
    pub killed_mutants: u32,
    pub survivor_count: u32,
    pub timed_out_count: u32,
    pub flaky_count: u32,
    pub line_coverage_evidence_ref: Option<String>,
    #[serde(default)]
    pub mutation_evidence_refs: Vec<String>,
    pub state: MutationTestingState,
    pub completeness: CoverageState,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub content_fingerprint: Sha256Hash,
}

impl MutationTestingSnapshot {
    pub fn is_current_schema(&self) -> bool {
        self.schema_id == MUTATION_TESTING_SNAPSHOT_SCHEMA_ID && self.schema_version == 1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QualityRulePackLifecycle {
    Active,
    Deprecated,
    Retired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QualityRulePackTrust {
    Trusted,
    Untrusted,
    Unverified,
    Expired,
}

/// Query metadata is descriptive and digest-bound. The query body itself is
/// retained only by its bounded artifact so it cannot be confused with source
/// text or execute through this public manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QualityRuleQueryMetadata {
    pub query_id: String,
    pub query_digest: Sha256Hash,
    pub query_metadata_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QualityRuleDefinition {
    pub rule_id: String,
    pub rule_version: String,
    pub default_severity: String,
    pub query: QualityRuleQueryMetadata,
    pub sarif_rule_id: Option<String>,
    pub lifecycle: QualityRulePackLifecycle,
    pub replacement_rule_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub architecture_rule: Option<ArchitectureRuleV1>,
}

/// Versioned, digest-bound Rule Pack metadata. Custom analyzers may emit
/// SARIF, but this contract does not execute queries or promote a trust claim
/// to a Gate decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QualityRulePackManifest {
    pub schema_id: String,
    pub schema_version: u32,
    pub pack_id: String,
    pub pack_version: String,
    pub source_ref: String,
    pub source_digest: Sha256Hash,
    pub tool_identity_sha256: Option<Sha256Hash>,
    #[serde(default)]
    pub supported_languages: Vec<String>,
    #[serde(default)]
    pub supported_source_classes: Vec<String>,
    #[serde(default)]
    pub fixture_corpus_refs: Vec<String>,
    #[serde(default)]
    pub rules: Vec<QualityRuleDefinition>,
    pub lifecycle: QualityRulePackLifecycle,
    pub replacement_pack_ref: Option<String>,
    pub signature_ref: Option<String>,
    pub trust: QualityRulePackTrust,
    pub valid_until: Option<String>,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub content_fingerprint: Sha256Hash,
}

impl QualityRulePackManifest {
    pub fn is_current_schema(&self) -> bool {
        self.schema_id == QUALITY_RULE_PACK_MANIFEST_SCHEMA_ID && self.schema_version == 1
    }

    pub fn validate_architecture_rules(
        &self,
        observed_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), crate::external_analysis::ExternalAnalysisContractError> {
        for rule in &self.rules {
            if let Some(architecture_rule) = &rule.architecture_rule {
                architecture_rule.validate(observed_at)?;
            }
        }
        Ok(())
    }
}

fn legacy_external_coverage() -> CoverageState {
    CoverageState::Partial
}

fn legacy_missing_external_fingerprint() -> Sha256Hash {
    Sha256Hash::digest(b"legacy-missing-external-normalization")
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DependencyRecord {
    pub dependency_id: String,
    pub purpose: String,
    pub ecosystem: String,
    pub package_identity: String,
    pub requested_version: Option<String>,
    pub resolved_version: Option<String>,
    pub source: String,
    pub integrity: Option<String>,
    #[serde(default)]
    pub license_refs: Vec<String>,
    #[serde(default)]
    pub advisory_refs: Vec<String>,
    pub direct: bool,
    #[serde(default)]
    pub affected_project_ids: Vec<ProjectId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DependencySnapshot {
    pub schema_id: String,
    pub schema_version: u32,
    pub snapshot_id: String,
    pub project_id: ProjectId,
    pub subject_revision: String,
    pub package_manager_id: String,
    pub package_manager_version: Option<String>,
    pub resolver_mode: String,
    pub manifest_path: String,
    pub manifest_sha256: Sha256Hash,
    pub lockfile_path: Option<String>,
    pub lockfile_sha256: Option<Sha256Hash>,
    pub dependencies: Vec<DependencyRecord>,
    pub completeness: CoverageState,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SupplyChainObservation {
    pub observation_id: String,
    pub kind: String,
    pub subject: String,
    pub state: String,
    pub source_ref: String,
    pub source_sha256: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SupplyChainSnapshot {
    pub schema_id: String,
    pub schema_version: u32,
    pub snapshot_id: String,
    pub project_id: ProjectId,
    pub subject_revision: String,
    pub dependency_snapshot_ref: String,
    pub dependency_snapshot_fingerprint: Sha256Hash,
    #[serde(default)]
    pub external_data_snapshot_refs: Vec<String>,
    pub observations: Vec<SupplyChainObservation>,
    #[serde(default)]
    pub provider_observations: Vec<SupplyChainProviderObservationV1>,
    pub freshness: ExternalFreshness,
    pub completeness: CoverageState,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UpdateKind {
    Patch,
    Minor,
    Major,
    Security,
    Internal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VersionDelta {
    Patch,
    Minor,
    Major,
    NonSemver,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateCandidate {
    pub candidate_id: String,
    pub dependency_id: String,
    pub current_requested_version: Option<String>,
    pub current_resolved_version: Option<String>,
    pub proposed_constraint: String,
    pub proposed_resolution: Option<String>,
    pub update_kind: UpdateKind,
    pub version_delta: VersionDelta,
    pub direct: bool,
    pub source_change: bool,
    pub reason: String,
    pub source_evidence_ref: String,
    pub source_freshness: ExternalFreshness,
    #[serde(default)]
    pub affected_project_ids: Vec<ProjectId>,
    #[serde(default)]
    pub affected_surfaces: Vec<String>,
    pub package_manager_adapter_ref: String,
    #[serde(default)]
    pub required_plan_refs: Vec<String>,
    #[serde(default)]
    pub required_approval_refs: Vec<String>,
    #[serde(default)]
    pub risk_markers: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DependencyUpdateStatus {
    Observed,
    Candidate,
    AwaitingRefreshApproval,
    AwaitingPatchPreparationApproval,
    PatchPrepared,
    AwaitingApplyApproval,
    Applied,
    Validated,
    Blocked,
    RolledBack,
    Superseded,
    Unverified,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DependencyUpdatePlan {
    pub schema_id: String,
    pub schema_version: u32,
    pub plan_id: String,
    pub project_id: ProjectId,
    pub dependency_snapshot_ref: String,
    pub candidate: UpdateCandidate,
    pub expected_manifest_paths: Vec<String>,
    pub expected_lockfile_paths: Vec<String>,
    pub patch_set_ref: Option<String>,
    pub previous_lockfile_artifact_ref: Option<String>,
    pub rollback_recipe_ref: Option<String>,
    pub status: DependencyUpdateStatus,
    #[serde(default)]
    pub blockers: Vec<String>,
    pub plan_fingerprint: Sha256Hash,
}

/// Completeness of a read-only Git history observation.  A missing or
/// rewritten predecessor is never silently promoted to a complete history.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GitHistoryCompleteness {
    Complete,
    Partial,
    Unverified,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GitHistoryComponentRisk {
    /// Project-relative component key, never a native or absolute path.
    pub component: String,
    pub changed_file_count: u32,
    pub relative_churn: u32,
    pub change_burst: u32,
    /// Opaque contributor buckets, rather than author names or email hashes.
    #[serde(default)]
    pub opaque_owner_buckets: Vec<String>,
    #[serde(default)]
    pub declared_owner_count: u32,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DebtMarkerObservation {
    pub marker_id: String,
    pub project_relative_path: String,
    pub marker_kind: String,
    pub line: u32,
    pub structured: bool,
    pub owner_declared: bool,
    pub issue_declared: bool,
    pub replacement_declared: bool,
    pub expiry: Option<String>,
    pub stale: bool,
    #[serde(default)]
    pub limitations: Vec<String>,
}

/// Rebuildable and privacy-preserving Git/source observation.  This is an
/// advisory input for Radar and Impact analysis, not a person-performance
/// record and not a standalone Gate blocker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GitHistoryRiskSnapshot {
    pub schema_id: String,
    pub schema_version: u32,
    pub project_id: ProjectId,
    pub repository_identity: String,
    pub range_start: String,
    pub range_end: String,
    pub history_completeness: GitHistoryCompleteness,
    pub codeowners_fingerprint: Option<Sha256Hash>,
    #[serde(default)]
    pub components: Vec<GitHistoryComponentRisk>,
    #[serde(default)]
    pub debt_markers: Vec<DebtMarkerObservation>,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub content_fingerprint: Sha256Hash,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RadarCategory {
    Failure,
    CodeQuality,
    Suppression,
    Dependency,
    Security,
    FlakyTest,
    ContractDrift,
    Recovery,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RadarPriority {
    pub blocking_rank: u8,
    pub risk_rank: u8,
    pub freshness_rank: u8,
    pub regression_rank: u8,
    pub evidence_rank: u8,
    pub time_rank: String,
    pub stable_identity: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluationRunEvidenceRef {
    pub evaluation_run_id: EvaluationRunId,
    pub run_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceRadarItem {
    pub item_id: String,
    pub project_id: ProjectId,
    pub category: RadarCategory,
    pub subject: String,
    pub priority: RadarPriority,
    #[serde(default)]
    pub finding_refs: Vec<String>,
    #[serde(default)]
    pub diagnostic_refs: Vec<String>,
    #[serde(default)]
    pub dependency_refs: Vec<String>,
    #[serde(default)]
    pub regression_refs: Vec<String>,
    #[serde(default)]
    pub suppression_refs: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub evaluation_run_refs: Vec<EvaluationRunEvidenceRef>,
    pub blocking: bool,
    pub freshness: ExternalFreshness,
    pub completeness: CoverageState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceRadarSnapshot {
    pub schema_id: String,
    pub schema_version: u32,
    pub snapshot_id: String,
    pub evaluation_time: String,
    pub items: Vec<MaintenanceRadarItem>,
    pub valid_until: Option<String>,
    pub completeness: CoverageState,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub content_fingerprint: Sha256Hash,
}

#[cfg(test)]
mod tests {
    use super::{MutationTestingSnapshot, QualityRulePackManifest, StaticAnalysisImportReport};

    #[test]
    fn static_analysis_import_report_fixtures_are_strict_and_versioned() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../specs/fixtures/management/v1/static-analysis-import-report");
        for name in ["minimal.json", "full.json"] {
            let value = std::fs::read_to_string(root.join(name)).unwrap();
            let report: StaticAnalysisImportReport = serde_json::from_str(&value).unwrap();
            assert!(report.is_current_schema());
        }
        let invalid = std::fs::read_to_string(root.join("invalid.json")).unwrap();
        assert!(serde_json::from_str::<StaticAnalysisImportReport>(&invalid).is_err());
        let future = std::fs::read_to_string(root.join("future.json")).unwrap();
        let future: StaticAnalysisImportReport = serde_json::from_str(&future).unwrap();
        assert!(!future.is_current_schema());
    }

    #[test]
    fn mutation_testing_snapshot_fixtures_are_strict_and_versioned() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../specs/fixtures/management/v1/mutation-testing-snapshot");
        for name in ["minimal.json", "full.json"] {
            let value = std::fs::read_to_string(root.join(name)).unwrap();
            let snapshot: MutationTestingSnapshot = serde_json::from_str(&value).unwrap();
            assert!(snapshot.is_current_schema());
        }
        let invalid = std::fs::read_to_string(root.join("invalid.json")).unwrap();
        assert!(serde_json::from_str::<MutationTestingSnapshot>(&invalid).is_err());
        let future = std::fs::read_to_string(root.join("future.json")).unwrap();
        let future: MutationTestingSnapshot = serde_json::from_str(&future).unwrap();
        assert!(!future.is_current_schema());
    }

    #[test]
    fn quality_rule_pack_manifest_fixtures_are_strict_and_versioned() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../specs/fixtures/management/v1/quality-rule-pack-manifest");
        for name in ["minimal.json", "full.json"] {
            let value = std::fs::read_to_string(root.join(name)).unwrap();
            let manifest: QualityRulePackManifest = serde_json::from_str(&value).unwrap();
            assert!(manifest.is_current_schema());
        }
        let invalid = std::fs::read_to_string(root.join("invalid.json")).unwrap();
        assert!(serde_json::from_str::<QualityRulePackManifest>(&invalid).is_err());
        let future = std::fs::read_to_string(root.join("future.json")).unwrap();
        let future: QualityRulePackManifest = serde_json::from_str(&future).unwrap();
        assert!(!future.is_current_schema());
    }
}
