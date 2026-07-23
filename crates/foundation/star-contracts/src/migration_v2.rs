//! Product-grade M8 migration, performance, language, and platform contracts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::development_v2::CoverageState;
use crate::{ProjectId, Sha256Hash};

pub const PROJECT_MIGRATION_MANIFEST_SCHEMA_ID: &str = "star.project-migration-manifest";
pub const MIGRATION_PLAN_V2_SCHEMA_ID: &str = "star.migration-plan";
pub const MIGRATION_CHECKPOINT_V2_SCHEMA_ID: &str = "star.migration-checkpoint";
pub const MIGRATION_ATTEMPT_SCHEMA_ID: &str = "star.migration-attempt";
pub const MIGRATION_VALIDATION_REPORT_SCHEMA_ID: &str = "star.migration-validation-report";
pub const RESTORE_VERIFICATION_RECORD_SCHEMA_ID: &str = "star.restore-verification-record";
pub const PERFORMANCE_WORKLOAD_SPEC_SCHEMA_ID: &str = "star.performance-workload-spec";
pub const PERFORMANCE_RUN_SCHEMA_ID: &str = "star.performance-run";
pub const PERFORMANCE_COMPARISON_V2_SCHEMA_ID: &str = "star.performance-comparison";
pub const LANGUAGE_MIGRATION_PLAN_SCHEMA_ID: &str = "star.language-migration-plan";
pub const EQUIVALENCE_REPORT_SCHEMA_ID: &str = "star.equivalence-report";
pub const CROSS_PROJECT_MIGRATION_HANDOFF_SCHEMA_ID: &str = "star.cross-project-migration-handoff";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrationTargetKind {
    Data,
    Config,
    Database,
    State,
    FileFormat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrationEffectClass {
    ReadOnly,
    CopyWrite,
    LiveNondestructive,
    LiveDestructive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IdempotencyContract {
    ReplaySafe,
    DetectAlreadyApplied,
    NotReplaySafe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnknownFieldPolicy {
    Preserve,
    Opaque,
    Block,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MigrationTargetSpec {
    pub target_id: String,
    pub target_kind: MigrationTargetKind,
    pub owner: String,
    pub locator_class: String,
    pub sensitivity: String,
    pub version_source_ref: String,
    pub current_version: Option<String>,
    pub target_version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MigrationInvariantSpec {
    pub invariant_id: String,
    pub invariant_kind: String,
    pub required: bool,
    pub before_check_ref: String,
    pub after_check_ref: String,
    pub loss_budget: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MigrationStepDefinition {
    pub step_id: String,
    pub step_version: String,
    pub from_version: String,
    pub to_version: String,
    #[serde(default)]
    pub preconditions: Vec<String>,
    pub invocation_template_ref: String,
    pub effect_class: MigrationEffectClass,
    #[serde(default)]
    pub write_scope: Vec<String>,
    pub idempotency_contract: IdempotencyContract,
    pub checkpoint_policy: String,
    pub unknown_field_policy: UnknownFieldPolicy,
    #[serde(default)]
    pub invariant_refs: Vec<String>,
    pub expected_output: String,
    pub rollback_ref: String,
    pub tool_ref: String,
    pub normalizer_ref: String,
    pub definition_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MigrationChain {
    pub chain_id: String,
    pub target_id: String,
    pub steps: Vec<MigrationStepDefinition>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectMigrationManifest {
    pub schema_id: String,
    pub schema_version: u32,
    pub manifest_id: String,
    pub manifest_version: String,
    pub project_id: ProjectId,
    pub target_specs: Vec<MigrationTargetSpec>,
    pub migration_chains: Vec<MigrationChain>,
    pub invariant_specs: Vec<MigrationInvariantSpec>,
    #[serde(default)]
    pub backup_specs: Vec<String>,
    #[serde(default)]
    pub rehearsal_specs: Vec<String>,
    #[serde(default)]
    pub activation_specs: Vec<String>,
    #[serde(default)]
    pub rollback_specs: Vec<String>,
    #[serde(default)]
    pub consumer_refs: Vec<String>,
    #[serde(default)]
    pub tool_refs: Vec<String>,
    #[serde(default)]
    pub check_refs: Vec<String>,
    #[serde(default)]
    pub cross_project_relations: Vec<String>,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VersionObservationState {
    Observed,
    Unknown,
    Corrupt,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MigrationVersionEntry {
    pub axis_id: String,
    pub owner: String,
    pub observed_version: Option<String>,
    pub version_scheme: String,
    pub source_ref: String,
    pub source_fingerprint: Sha256Hash,
    pub coverage: CoverageState,
    pub observation_state: VersionObservationState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MigrationVersionVector {
    pub entries: Vec<MigrationVersionEntry>,
    pub vector_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrationSupportDecision {
    CurrentSupported,
    Migratable,
    ReadOnlySupported,
    FutureVersion,
    ChainGap,
    AmbiguousChain,
    UnknownVersion,
    Corrupt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrationStrategy {
    SideBySide,
    AtomicReplace,
    TransactionalInPlace,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolvedMigrationStep {
    pub order: u32,
    pub step_id: String,
    pub step_version: String,
    pub definition_fingerprint: Sha256Hash,
    pub from_version: String,
    pub to_version: String,
    pub effect_class: MigrationEffectClass,
    pub idempotency_contract: IdempotencyContract,
    pub invocation_template_ref: String,
    pub rollback_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MigrationPhasePlan {
    pub phase: String,
    pub required: bool,
    pub input_refs: Vec<String>,
    pub expected_output: String,
    pub stop_condition: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MigrationPlanV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub migration_plan_id: String,
    pub revision: u64,
    pub task_spec_ref: String,
    pub scope_revision_ref: String,
    pub impact_analysis_ref: String,
    pub project_id: ProjectId,
    pub checkout_id: String,
    pub source_subject_fingerprint: Sha256Hash,
    pub manifest_ref: String,
    pub manifest_fingerprint: Sha256Hash,
    pub target_id: String,
    pub observed_version_vector: MigrationVersionVector,
    pub target_version_vector: MigrationVersionVector,
    pub support_decision: MigrationSupportDecision,
    pub ordered_steps: Vec<ResolvedMigrationStep>,
    #[serde(default)]
    pub invariant_refs: Vec<String>,
    pub strategy: MigrationStrategy,
    pub resource_estimate: String,
    pub dry_run_plan: MigrationPhasePlan,
    pub backup_plan: MigrationPhasePlan,
    pub rehearsal_plan: MigrationPhasePlan,
    pub activation_plan: MigrationPhasePlan,
    pub resume_plan: MigrationPhasePlan,
    pub rollback_plan_ref: String,
    #[serde(default)]
    pub validation_plan_refs: Vec<String>,
    #[serde(default)]
    pub permission_checkpoints: Vec<String>,
    #[serde(default)]
    pub source_patch_refs: Vec<String>,
    #[serde(default)]
    pub consumer_compatibility_refs: Vec<String>,
    pub cross_project_handoff_ref: Option<String>,
    #[serde(default)]
    pub blockers: Vec<String>,
    pub plan_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrationPhase {
    DryRun,
    Backup,
    BackupVerify,
    RestoreRehearsal,
    MigrationRehearsal,
    PreExecuteGate,
    Execute,
    Resume,
    Validate,
    Activate,
    ConsumerValidate,
    Rollback,
    PostRollbackValidate,
    Reconcile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrationAttemptState {
    Planned,
    Running,
    Succeeded,
    Failed,
    Blocked,
    OutcomeUnknown,
    PartiallyApplied,
    RolledBack,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MigrationCheckpointV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub checkpoint_id: String,
    pub plan_ref: String,
    pub plan_fingerprint: Sha256Hash,
    pub completed_step_refs: Vec<String>,
    pub in_progress_step_ref: Option<String>,
    pub target_version: String,
    pub target_state_fingerprint: Sha256Hash,
    pub last_receipt_ref: Option<String>,
    pub replay_safe: bool,
    pub reconciliation_required: bool,
    pub checkpoint_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MigrationAttempt {
    pub schema_id: String,
    pub schema_version: u32,
    pub attempt_id: String,
    pub attempt_no: u32,
    pub plan_ref: String,
    pub plan_fingerprint: Sha256Hash,
    pub phase: MigrationPhase,
    pub step_ref: Option<String>,
    pub subject_binding_before: Sha256Hash,
    pub checkpoint_before_ref: Option<String>,
    pub permission_decision_ref: Option<String>,
    pub gate_decision_ref: Option<String>,
    pub invocation_ref: Option<String>,
    pub tool_observation_ref: Option<String>,
    #[serde(default)]
    pub receipt_refs: Vec<String>,
    pub subject_binding_after: Option<Sha256Hash>,
    pub checkpoint_after_ref: Option<String>,
    #[serde(default)]
    pub diagnostic_refs: Vec<String>,
    pub state: MigrationAttemptState,
    pub effect_committed: Option<bool>,
    pub loss_observed: Option<bool>,
    pub attempt_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InvariantResult {
    pub invariant_ref: String,
    pub before_fingerprint: Sha256Hash,
    pub after_fingerprint: Sha256Hash,
    pub state: String,
    pub loss_observed: bool,
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MigrationValidationReport {
    pub schema_id: String,
    pub schema_version: u32,
    pub report_id: String,
    pub plan_ref: String,
    pub attempt_ref: String,
    pub invariant_results: Vec<InvariantResult>,
    pub reference_validation_refs: Vec<String>,
    pub gate_refs: Vec<String>,
    pub target_version_observed: Option<String>,
    pub state: String,
    pub completeness: CoverageState,
    pub report_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RestoreVerificationRecord {
    pub schema_id: String,
    pub schema_version: u32,
    pub record_id: String,
    pub plan_ref: String,
    pub backup_artifact_ref: String,
    pub backup_fingerprint: Sha256Hash,
    pub restored_subject_fingerprint: Sha256Hash,
    pub integrity_verified: bool,
    pub behavior_check_refs: Vec<String>,
    pub state: String,
    pub record_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PerformanceMetricSpec {
    pub metric_id: String,
    pub unit: String,
    pub direction: String,
    pub budget_ratio: f64,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PerformanceWorkloadSpec {
    pub schema_id: String,
    pub schema_version: u32,
    pub workload_id: String,
    pub project_id: ProjectId,
    pub task_ref: String,
    pub input_fingerprint: Sha256Hash,
    pub environment_class: String,
    pub build_mode: String,
    pub warmup_count: u32,
    pub measured_count: u32,
    pub outlier_policy: String,
    pub noise_budget_ratio: f64,
    pub metrics: Vec<PerformanceMetricSpec>,
    pub correctness_check_refs: Vec<String>,
    pub specification_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceCohort {
    Baseline,
    Candidate,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PerformanceMeasurement {
    pub metric_id: String,
    pub value: f64,
    pub unit: String,
    pub collector: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PerformanceRun {
    pub schema_id: String,
    pub schema_version: u32,
    pub run_id: String,
    pub workload_ref: String,
    pub workload_fingerprint: Sha256Hash,
    pub cohort: PerformanceCohort,
    pub attempt: u32,
    pub warmup: bool,
    pub subject_fingerprint: Sha256Hash,
    pub environment_fingerprint: Sha256Hash,
    pub toolchain_fingerprint: Sha256Hash,
    pub build_mode: String,
    pub measurements: Vec<PerformanceMeasurement>,
    pub correctness_passed: bool,
    pub evidence_refs: Vec<String>,
    pub run_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceComparisonState {
    Pass,
    Regression,
    Incomparable,
    NoiseInconclusive,
    CorrectnessUnverified,
    HumanReview,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MetricComparison {
    pub metric_id: String,
    pub unit: String,
    pub baseline_median: f64,
    pub candidate_median: f64,
    pub baseline_p95: f64,
    pub candidate_p95: f64,
    pub ratio: f64,
    pub noise_ratio: f64,
    pub budget_ratio: f64,
    pub state: PerformanceComparisonState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PerformanceComparisonV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub comparison_id: String,
    pub workload_ref: String,
    pub workload_fingerprint: Sha256Hash,
    pub baseline_run_refs: Vec<String>,
    pub candidate_run_refs: Vec<String>,
    pub metric_comparisons: Vec<MetricComparison>,
    pub correctness_verified: bool,
    pub comparable: bool,
    pub state: PerformanceComparisonState,
    pub limitations: Vec<String>,
    pub comparison_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StackBinding {
    pub language: String,
    pub runtime: String,
    pub sdk: String,
    pub architecture: String,
    pub os: String,
    pub toolchain_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoexistencePhase {
    pub phase_id: String,
    pub order: u32,
    pub source_state: String,
    pub consumer_state: String,
    pub writer_state: String,
    pub reader_state: String,
    pub fallback_available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LanguageMigrationPlan {
    pub schema_id: String,
    pub schema_version: u32,
    pub plan_id: String,
    pub revision: u64,
    pub task_spec_ref: String,
    pub impact_analysis_ref: String,
    pub project_id: ProjectId,
    pub checkout_id: String,
    pub source_stack: StackBinding,
    pub target_stack: StackBinding,
    pub behavior_contract_refs: Vec<String>,
    pub boundary_adapter_specs: Vec<String>,
    pub coexistence_phases: Vec<CoexistencePhase>,
    pub consumer_transition_order: Vec<String>,
    pub recipe_refs: Vec<String>,
    pub codegen_refs: Vec<String>,
    pub comparison_plan_refs: Vec<String>,
    pub compatibility_window: String,
    pub cutover_plan: String,
    pub rollback_plan_ref: String,
    pub platform_evidence_matrix: Vec<String>,
    pub unknown_semantics: Vec<String>,
    pub state: String,
    pub plan_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EquivalenceDimensionState {
    Equivalent,
    NotEquivalent,
    Partial,
    NotRun,
    Unverified,
    HumanReview,
    NotRequired,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EquivalenceDimensionResult {
    pub dimension_id: String,
    pub required: bool,
    pub state: EquivalenceDimensionState,
    pub evidence_refs: Vec<String>,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EquivalenceState {
    NotEvaluated,
    Partial,
    Equivalent,
    NotEquivalent,
    HumanReview,
    Unverified,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EquivalenceReport {
    pub schema_id: String,
    pub schema_version: u32,
    pub equivalence_report_id: String,
    pub plan_ref: String,
    pub baseline_subject: Sha256Hash,
    pub candidate_subject: Sha256Hash,
    pub dimension_results: Vec<EquivalenceDimensionResult>,
    pub build_compile_result: String,
    pub test_contract_results: Vec<String>,
    pub performance_comparison_refs: Vec<String>,
    pub platform_matrix_results: Vec<String>,
    pub consumer_results: Vec<String>,
    pub unknown_semantics: Vec<String>,
    pub equivalence_state: EquivalenceState,
    pub gate_refs: Vec<String>,
    pub report_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MigrationParticipant {
    pub project_id: ProjectId,
    pub plan_ref: String,
    pub plan_fingerprint: Sha256Hash,
    pub state: String,
    pub ordering_hint: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CrossProjectMigrationHandoff {
    pub schema_id: String,
    pub schema_version: u32,
    pub handoff_id: String,
    pub participants: Vec<MigrationParticipant>,
    pub dependency_edges: Vec<String>,
    pub blockers: Vec<String>,
    pub ready_for_change_bundle: bool,
    pub content_fingerprint: Sha256Hash,
}
