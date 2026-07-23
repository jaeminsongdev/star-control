//! Product-grade M7 failure, reproduction, security, dependency, and maintenance contracts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::development_v2::CoverageState;
use crate::{ProjectId, Sha256Hash};

pub const FAILURE_RECORD_SCHEMA_ID: &str = "star.failure-record";
pub const REPRODUCTION_PACK_V2_SCHEMA_ID: &str = "star.reproduction-pack";
pub const REGRESSION_RECORD_SCHEMA_ID: &str = "star.regression-record";
pub const RECOVERY_PLAN_V2_SCHEMA_ID: &str = "star.recovery-plan";
pub const DEPENDENCY_SNAPSHOT_SCHEMA_ID: &str = "star.dependency-snapshot";
pub const SUPPLY_CHAIN_SNAPSHOT_SCHEMA_ID: &str = "star.supply-chain-snapshot";
pub const EXTERNAL_DATA_SNAPSHOT_SCHEMA_ID: &str = "star.external-data-snapshot";
pub const DEPENDENCY_UPDATE_PLAN_SCHEMA_ID: &str = "star.dependency-update-plan";
pub const MAINTENANCE_RADAR_SNAPSHOT_SCHEMA_ID: &str = "star.maintenance-radar-snapshot";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
    pub retrieval_mode: String,
    pub integrity_policy: String,
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
    pub source: ExternalDataSourceDescriptor,
    pub retrieved_at: String,
    pub valid_until: String,
    pub evaluation_time: String,
    pub source_artifact_ref: String,
    pub source_sha256: Sha256Hash,
    pub observations: Vec<ExternalDataObservation>,
    pub freshness: ExternalFreshness,
    pub completeness: CoverageState,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub content_fingerprint: Sha256Hash,
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

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RadarCategory {
    Failure,
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
