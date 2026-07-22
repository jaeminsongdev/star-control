//! M5-M9 development maintenance and coordination contracts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{GoalId, ProjectId, Sha256Hash};

pub const MANAGED_REGISTRY_SNAPSHOT_SCHEMA_ID: &str = "star.managed-registry-snapshot";
pub const COMPATIBILITY_REPORT_SCHEMA_ID: &str = "star.compatibility-report";
pub const CLEAN_ROOM_DOCTOR_REPORT_SCHEMA_ID: &str = "star.clean-room-doctor-report";
pub const REPRODUCTION_PACK_SCHEMA_ID: &str = "star.reproduction-pack";
pub const MAINTENANCE_RADAR_SCHEMA_ID: &str = "star.maintenance-radar";
pub const MIGRATION_RUN_SCHEMA_ID: &str = "star.migration-run";
pub const PERFORMANCE_COMPARISON_SCHEMA_ID: &str = "star.performance-comparison";
pub const CHANGE_BUNDLE_SCHEMA_ID: &str = "star.change-bundle";
pub const CHANGE_BUNDLE_HANDOFF_SCHEMA_ID: &str = "star.change-bundle-handoff";

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceCompleteness {
    Complete,
    Partial,
    Unverified,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ManagedDeclarationKind {
    ErrorCode,
    SchemaId,
    ConfigKey,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ManagedLifecycle {
    Active,
    Deprecated,
    Reserved,
    Removed,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ManagedConsumerState {
    Bound,
    Alias,
    Unresolved,
    Stale,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagedDeclaration {
    pub declaration_id: String,
    pub namespace: String,
    pub kind: ManagedDeclarationKind,
    pub value: String,
    pub owner_project_id: ProjectId,
    pub source_path: String,
    pub source_sha256: Sha256Hash,
    pub lifecycle: ManagedLifecycle,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagedConsumer {
    pub declaration_id: String,
    pub project_id: ProjectId,
    pub path: String,
    pub observed_value: String,
    pub state: ManagedConsumerState,
    pub source_sha256: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagedRegistrySnapshot {
    pub schema_id: String,
    pub schema_version: u32,
    pub registry_id: String,
    pub git_revision: String,
    pub manifest_sha256: Sha256Hash,
    pub declarations: Vec<ManagedDeclaration>,
    pub consumers: Vec<ManagedConsumer>,
    pub completeness: EvidenceCompleteness,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConsumerMigrationState {
    Ready,
    Blocked,
    NoChange,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConsumerRewrite {
    pub project_id: ProjectId,
    pub path: String,
    pub expected_source_sha256: Sha256Hash,
    pub before_value: String,
    pub after_value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConsumerMigrationPlan {
    pub declaration_id: String,
    pub from_snapshot: Sha256Hash,
    pub to_snapshot: Sha256Hash,
    pub state: ConsumerMigrationState,
    pub rewrites: Vec<ConsumerRewrite>,
    pub blockers: Vec<String>,
    pub plan_fingerprint: Sha256Hash,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ContractKind {
    Api,
    Schema,
    Config,
    Docs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityOutcome {
    Compatible,
    Breaking,
    HumanReview,
    Unverified,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityFinding {
    pub code: String,
    pub subject: String,
    pub outcome: CompatibilityOutcome,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityReport {
    pub schema_id: String,
    pub schema_version: u32,
    pub kind: ContractKind,
    pub before_sha256: Sha256Hash,
    pub after_sha256: Sha256Hash,
    pub outcome: CompatibilityOutcome,
    pub findings: Vec<CompatibilityFinding>,
    pub completeness: EvidenceCompleteness,
    pub report_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DoctorCheckState {
    Pass,
    Block,
    Unverified,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DoctorCheck {
    pub check_id: String,
    pub state: DoctorCheckState,
    pub observed: String,
    pub required: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CleanRoomDoctorReport {
    pub schema_id: String,
    pub schema_version: u32,
    pub dependency_download: String,
    pub package_install: String,
    pub system_mutation: String,
    pub checks: Vec<DoctorCheck>,
    pub state: DoctorCheckState,
    pub report_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReproductionState {
    Reproduced,
    PartiallyReproduced,
    NotReproduced,
    BlockedExternal,
    Unverified,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReproductionAttempt {
    pub attempt: u32,
    pub family_fingerprint: Sha256Hash,
    pub environment_fingerprint: Sha256Hash,
    pub input_fingerprint: Sha256Hash,
    pub complete: bool,
    pub observed: bool,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReproductionPack {
    pub schema_id: String,
    pub schema_version: u32,
    pub family_fingerprint: Sha256Hash,
    pub subject_fingerprint: Sha256Hash,
    pub attempts: Vec<ReproductionAttempt>,
    pub state: ReproductionState,
    pub limitations: Vec<String>,
    pub pack_fingerprint: Sha256Hash,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RadarKind {
    Failure,
    Dependency,
    Security,
    SupplyChain,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RadarItem {
    pub item_id: String,
    pub kind: RadarKind,
    pub subject: String,
    pub priority: u32,
    pub source: String,
    pub source_fingerprint: Sha256Hash,
    pub fresh: bool,
    pub blocking: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceRadar {
    pub schema_id: String,
    pub schema_version: u32,
    pub items: Vec<RadarItem>,
    pub completeness: EvidenceCompleteness,
    pub limitations: Vec<String>,
    pub radar_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrationRunState {
    Planned,
    Running,
    Interrupted,
    Completed,
    RollbackRequired,
    RolledBack,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MigrationStep {
    pub step_id: String,
    pub from_version: u32,
    pub to_version: u32,
    pub input_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MigrationCheckpoint {
    pub completed_step_ids: Vec<String>,
    pub current_version: u32,
    pub state_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MigrationRun {
    pub schema_id: String,
    pub schema_version: u32,
    pub migration_id: String,
    pub source_version: u32,
    pub target_version: u32,
    pub steps: Vec<MigrationStep>,
    pub checkpoint: MigrationCheckpoint,
    pub state: MigrationRunState,
    pub limitations: Vec<String>,
    pub run_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceState {
    Pass,
    Regression,
    Incomparable,
    Unverified,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PerformanceComparison {
    pub schema_id: String,
    pub schema_version: u32,
    pub workload_id: String,
    pub binding_fingerprint: Sha256Hash,
    pub reference_samples_ms: Vec<f64>,
    pub candidate_samples_ms: Vec<f64>,
    pub reference_p95_ms: f64,
    pub candidate_p95_ms: f64,
    pub budget_ratio: f64,
    pub state: PerformanceState,
    pub comparison_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlatformVerificationState {
    NativeVerified,
    NativeUnverified,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlatformMigrationEvidence {
    pub target_triple: String,
    pub artifact_sha256: Sha256Hash,
    pub architecture: String,
    pub cross_build_complete: bool,
    pub simulation_checks: Vec<String>,
    pub state: PlatformVerificationState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BundleParticipantState {
    Planned,
    WorktreeReady,
    Validated,
    MergeQueued,
    MergedLocal,
    RemoteVerified,
    Blocked,
    OutcomeUnknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeBundleParticipant {
    pub participant_id: String,
    pub project_id: ProjectId,
    pub checkout_revision: String,
    pub owned_worktree: String,
    pub patch_fingerprint: Sha256Hash,
    pub gate_fingerprint: Sha256Hash,
    pub state: BundleParticipantState,
    pub resulting_commit: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BundleDependency {
    pub from_participant_id: String,
    pub to_participant_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RemoteOutcome {
    NotRequested,
    Verified,
    Failed,
    OutcomeUnknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeBundle {
    pub schema_id: String,
    pub schema_version: u32,
    pub bundle_id: String,
    pub goal_id: GoalId,
    pub revision: u64,
    pub participants: Vec<ChangeBundleParticipant>,
    pub dependencies: Vec<BundleDependency>,
    pub merge_order: Vec<String>,
    pub remote_outcome: RemoteOutcome,
    pub remote_snapshot_fingerprint: Option<Sha256Hash>,
    pub limitations: Vec<String>,
    pub bundle_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeBundleHandoff {
    pub schema_id: String,
    pub schema_version: u32,
    pub bundle_id: String,
    pub bundle_revision: u64,
    pub bundle_fingerprint: Sha256Hash,
    pub participant_commits: Vec<String>,
    pub artifact_fingerprints: Vec<Sha256Hash>,
    pub remote_outcome: RemoteOutcome,
    pub ready: bool,
    pub blockers: Vec<String>,
    pub handoff_fingerprint: Sha256Hash,
}
