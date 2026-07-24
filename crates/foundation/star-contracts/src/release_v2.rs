use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    ApprovalId, EvaluationRunId, GateId, ProjectId, ReleaseManifestId, ScopeRevisionId, Sha256Hash,
    TaskInvocationId, TaskSpecId, ValidationPlanId, ValidationRunId,
};

pub const RELEASE_MANIFEST_V2_SCHEMA_ID: &str = "star.release-manifest";
pub const RELEASE_ASSET_BINDING_V1_SCHEMA_ID: &str = "star.release-asset-binding";
pub const EVALUATION_RUN_V2_SCHEMA_ID: &str = "star.evaluation-run";
pub const EVALUATION_CATALOG_ITEM_SCHEMA_ID: &str = "star.evaluation-catalog-item";
pub const EVALUATION_CASE_DEFINITION_V1_SCHEMA_ID: &str = "star.evaluation-case-definition";
pub const EVALUATION_POLICY_V1_SCHEMA_ID: &str = "star.evaluation-policy";
pub const COST_RECORD_V1_SCHEMA_ID: &str = "star.cost-record";
pub const BUDGET_SNAPSHOT_V1_SCHEMA_ID: &str = "star.budget-snapshot";
pub const FINAL_PRODUCT_AUDIT_V1_SCHEMA_ID: &str = "star.final-product-audit";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseStatus {
    Draft,
    Candidate,
    Blocked,
    BlockedExternal,
    Ready,
    Approved,
    Publishing,
    PublishOutcomeUnknown,
    Published,
    RollbackRequired,
    Withdrawn,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseArchitecture {
    X64,
    Arm64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseSupportTier {
    Stable,
    Preview,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeVerificationState {
    NativeVerified,
    NativeUnverified,
    Failed,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum VerificationLayerKind {
    LocalQuick,
    Target,
    Full,
    Release,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceCompleteness {
    Complete,
    Partial,
    Unverified,
    NotRun,
    Flaky,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SupplyChainKind {
    Sbom,
    Provenance,
    Signing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SupplyChainState {
    NotRequired,
    RequiredUnavailable,
    RequiredIncomplete,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RemoteActionKind {
    Publish,
    Deploy,
    Withdraw,
    Rollback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RemoteActionState {
    Planned,
    Approved,
    Running,
    Verified,
    OutcomeUnknown,
    RollbackRequired,
    RolledBack,
    Withdrawn,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseSourceRevision {
    pub project_id: ProjectId,
    pub revision: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseIdentityBinding {
    pub config_fingerprint: Sha256Hash,
    pub catalog_fingerprint: Sha256Hash,
    pub tool_descriptor_fingerprints: Vec<Sha256Hash>,
    pub profile_fingerprint: Sha256Hash,
    pub environment_fingerprints: Vec<Sha256Hash>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseVerificationLayer {
    pub layer: VerificationLayerKind,
    pub validation_plan_ref: ValidationPlanId,
    pub validation_run_ref: Option<ValidationRunId>,
    pub gate_ref: Option<GateId>,
    pub completeness: EvidenceCompleteness,
    pub artifact_set_digest: Option<Sha256Hash>,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseArtifactV2 {
    pub logical_name: String,
    pub role: String,
    pub architecture: ReleaseArchitecture,
    pub size: u64,
    pub media_type: String,
    pub sha256: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAssetSourceV1 {
    pub logical_name: String,
    pub remote_name: String,
    pub role: String,
    pub architecture: ReleaseArchitecture,
    pub media_type: String,
    pub relative_path: String,
    pub size: u64,
    pub sha256: Sha256Hash,
}

/// Controller-owned local path binding for one immutable release manifest.
///
/// The public `ReleaseManifestV2` remains backend neutral and contains no
/// machine-local paths. This companion document binds those exact bytes to a
/// GitHub destination only inside the management store so publication can
/// re-read and verify every byte immediately before an external effect.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAssetBindingV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub release_manifest_id: ReleaseManifestId,
    pub project_id: ProjectId,
    pub artifact_set_digest: Sha256Hash,
    pub assets: Vec<ReleaseAssetSourceV1>,
    pub repository: String,
    pub tag: String,
    pub target_commitish: String,
    pub title: String,
    pub notes_relative_path: String,
    pub prerelease: bool,
    pub binding_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SupplyChainDecision {
    pub kind: SupplyChainKind,
    pub state: SupplyChainState,
    pub policy_ref: String,
    pub evidence_ref: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseCompatibilityTarget {
    pub architecture: ReleaseArchitecture,
    pub support_tier: ReleaseSupportTier,
    pub runtime_verification: RuntimeVerificationState,
    pub minimum_windows_build: u32,
    pub evidence_refs: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseRemoteAction {
    pub action_id: String,
    pub kind: RemoteActionKind,
    pub provider: String,
    pub destination: String,
    pub immutable_subject_digest: Sha256Hash,
    pub state: RemoteActionState,
    pub approval_request_ref: Option<ApprovalId>,
    pub before_snapshot_ref: Option<String>,
    pub after_snapshot_ref: Option<String>,
    pub receipt_ref: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseManifestV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub release_manifest_id: ReleaseManifestId,
    pub revision: u64,
    pub supersedes: Option<String>,
    pub product_id: String,
    pub version: String,
    pub channel: String,
    pub task_spec_ref: TaskSpecId,
    pub scope_revision_ref: ScopeRevisionId,
    pub source_revisions: Vec<ReleaseSourceRevision>,
    pub identity_binding: ReleaseIdentityBinding,
    pub verification_layers: Vec<ReleaseVerificationLayer>,
    pub build_invocation_refs: Vec<TaskInvocationId>,
    pub artifacts: Vec<ReleaseArtifactV2>,
    pub artifact_set_digest: Option<Sha256Hash>,
    pub included_files_manifest_ref: Option<String>,
    pub metadata_refs: Vec<String>,
    pub supply_chain_applicability: Vec<SupplyChainDecision>,
    pub sbom_ref: Option<String>,
    pub provenance_ref: Option<String>,
    pub signature_refs: Vec<String>,
    pub compatibility: Vec<ReleaseCompatibilityTarget>,
    pub validation_refs: Vec<String>,
    pub release_gate_refs: Vec<GateId>,
    pub remote_actions: Vec<ReleaseRemoteAction>,
    pub approval_request_refs: Vec<ApprovalId>,
    pub remote_operation_refs: Vec<String>,
    pub before_remote_snapshot_refs: Vec<String>,
    pub after_remote_snapshot_refs: Vec<String>,
    pub rollback_plan_ref: String,
    pub rollback_artifact_ref: Option<String>,
    pub user_data_policy: String,
    pub remaining_risks: Vec<String>,
    pub external_gates: Vec<String>,
    pub status: ReleaseStatus,
    pub manifest_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProductAuditStatusV1 {
    Conformant,
    BlockedExternal,
    Blocked,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductFeatureOwnershipStatusV1 {
    pub feature_id: String,
    pub semantic_owner_ref: String,
    pub physical_owner: String,
    pub command_surfaces: Vec<String>,
    #[serde(default)]
    pub missing_command_surfaces: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductProfileConformanceStatusV1 {
    pub profile_id: String,
    pub profile_version: String,
    pub definition_hash: Sha256Hash,
    pub resolution_fingerprint: Sha256Hash,
    pub conformant: bool,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductLifecycleEvidenceStatusV1 {
    pub architecture: ReleaseArchitecture,
    pub support_tier: ReleaseSupportTier,
    pub runtime_verification: RuntimeVerificationState,
    pub evidence_record_id: String,
    pub evidence_fingerprint: Sha256Hash,
    pub candidate_artifact_set_digest: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FinalProductAuditV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub release_manifest_id: ReleaseManifestId,
    pub release_manifest_fingerprint: Sha256Hash,
    pub artifact_set_digest: Sha256Hash,
    pub profile_catalog_fingerprint: Sha256Hash,
    pub feature_statuses: Vec<ProductFeatureOwnershipStatusV1>,
    pub profile_statuses: Vec<ProductProfileConformanceStatusV1>,
    pub m11_profile_conformant: bool,
    pub lifecycle_statuses: Vec<ProductLifecycleEvidenceStatusV1>,
    pub internal_conformance: bool,
    pub release_status: ReleaseStatus,
    #[serde(default)]
    pub external_gate_reasons: Vec<String>,
    pub status: ProductAuditStatusV1,
    pub audit_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationSubjectKind {
    RoutePolicy,
    Rule,
    Check,
    Profile,
    Recipe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationContext {
    CliOnly,
    CodexIntegrated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationMode {
    Offline,
    Replay,
    Shadow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CaseAdjudication {
    ConfirmedDefect,
    FalsePositive,
    Unresolved,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationOutcome {
    Success,
    Failure,
    Rollback,
    Accepted,
    Rejected,
    Reverted,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ComparabilityState {
    Compatible,
    NotComparable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationRecommendation {
    Keep,
    Trial,
    Accept,
    Reject,
    NeedsReview,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluationSubject {
    pub kind: EvaluationSubjectKind,
    pub item_id: String,
    pub version: String,
    pub definition_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluationDefinition {
    pub subject: EvaluationSubject,
    pub resolved_closure_fingerprint: Sha256Hash,
    pub policy_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluationCaseDefinitionRefV1 {
    pub case_id: String,
    pub case_version: String,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluationCaseDefinitionV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub revision: u64,
    pub project_id: ProjectId,
    pub case_id: String,
    pub case_version: String,
    pub corpus_ref: String,
    pub evaluation_context: EvaluationContext,
    pub adjudication: CaseAdjudication,
    pub ground_truth_evidence_refs: Vec<String>,
    pub source_ref: String,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluationPolicyRefV1 {
    pub policy_id: String,
    pub policy_version: String,
    pub revision: u64,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluationPolicyV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub revision: u64,
    pub project_id: ProjectId,
    pub policy_id: String,
    pub policy_version: String,
    pub subject_kind: EvaluationSubjectKind,
    pub evaluation_context: EvaluationContext,
    pub mode: EvaluationMode,
    pub corpus_ref: String,
    pub case_refs: Vec<EvaluationCaseDefinitionRefV1>,
    pub minimum_sample_count: u32,
    pub max_attempts_per_case: u32,
    pub comparability_dimensions: Vec<String>,
    pub protected_metric_ids: Vec<String>,
    pub require_provider_cost: bool,
    pub source_ref: String,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CostRecordRefV1 {
    pub cost_record_id: String,
    pub revision: u64,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluationQuantityV1 {
    pub unit: String,
    pub quantity: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluationQuantityComparisonV1 {
    pub unit: String,
    pub baseline_quantity: u64,
    pub candidate_quantity: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluationSuppressionSummary {
    pub active: u32,
    pub expired: u32,
    pub stale: u32,
    pub revoked: u32,
    pub invalid: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluationCaseResult {
    pub case_id: String,
    pub case_version: String,
    pub corpus_ref: String,
    pub evaluation_context: EvaluationContext,
    pub case_definition_ref: EvaluationCaseDefinitionRefV1,
    pub task_source_binding: Sha256Hash,
    pub baseline_run_refs: Vec<ValidationRunId>,
    pub candidate_run_refs: Vec<ValidationRunId>,
    pub adjudication: CaseAdjudication,
    #[serde(default)]
    pub adjudication_evidence_refs: Vec<String>,
    pub baseline_detected: bool,
    pub candidate_detected: bool,
    pub baseline_duration_ms: u64,
    pub candidate_duration_ms: u64,
    pub baseline_rework_count: u32,
    pub candidate_rework_count: u32,
    pub baseline_outcome: EvaluationOutcome,
    pub candidate_outcome: EvaluationOutcome,
    pub candidate_flaky: bool,
    #[serde(default)]
    pub baseline_finding_count: u32,
    #[serde(default)]
    pub candidate_finding_count: u32,
    #[serde(default)]
    pub baseline_new_or_worsened_count: u32,
    #[serde(default)]
    pub candidate_new_or_worsened_count: u32,
    #[serde(default)]
    pub baseline_existing_debt_count: u32,
    #[serde(default)]
    pub candidate_existing_debt_count: u32,
    #[serde(default)]
    pub baseline_suppressions: EvaluationSuppressionSummary,
    #[serde(default)]
    pub candidate_suppressions: EvaluationSuppressionSummary,
    #[serde(default)]
    pub suppression_newly_added_count: u32,
    #[serde(default)]
    pub suppression_broadened_count: u32,
    #[serde(default)]
    pub suppression_removed_count: u32,
    #[serde(default)]
    pub baseline_cost_refs: Vec<CostRecordRefV1>,
    #[serde(default)]
    pub candidate_cost_refs: Vec<CostRecordRefV1>,
    #[serde(default)]
    pub baseline_usage_and_cost: Vec<EvaluationQuantityV1>,
    #[serde(default)]
    pub candidate_usage_and_cost: Vec<EvaluationQuantityV1>,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluationComparability {
    pub dimension: String,
    pub state: ComparabilityState,
    pub evidence_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProtectedMetricResult {
    pub metric_id: String,
    pub weakened: bool,
    pub evidence_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluationMetricSummary {
    pub sample_count: u32,
    pub confirmed_defects: u32,
    pub candidate_false_negatives: u32,
    pub candidate_false_positives: u32,
    pub unresolved: u32,
    pub candidate_flaky: u32,
    pub baseline_total_duration_ms: u64,
    pub candidate_total_duration_ms: u64,
    pub baseline_rework_count: u32,
    pub candidate_rework_count: u32,
    pub candidate_rollbacks: u32,
    pub baseline_finding_count: u32,
    pub candidate_finding_count: u32,
    pub baseline_new_or_worsened_count: u32,
    pub candidate_new_or_worsened_count: u32,
    pub baseline_existing_debt_count: u32,
    pub candidate_existing_debt_count: u32,
    pub baseline_suppressions: EvaluationSuppressionSummary,
    pub candidate_suppressions: EvaluationSuppressionSummary,
    pub suppression_newly_added_count: u32,
    pub suppression_broadened_count: u32,
    pub suppression_removed_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluationRunV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub evaluation_run_id: EvaluationRunId,
    pub subject_kind: EvaluationSubjectKind,
    pub subject: EvaluationSubject,
    pub evaluation_context: EvaluationContext,
    pub evaluation_policy_ref: EvaluationPolicyRefV1,
    pub baseline: EvaluationDefinition,
    pub candidate: EvaluationDefinition,
    pub mode: EvaluationMode,
    pub corpus_ref: String,
    pub case_selection_fingerprint: Sha256Hash,
    pub measurement_protocol_fingerprint: Sha256Hash,
    pub minimum_sample_count: u32,
    pub case_results: Vec<EvaluationCaseResult>,
    pub ground_truth_summary: EvaluationMetricSummary,
    pub finding_metrics: EvaluationMetricSummary,
    pub efficiency_metrics: EvaluationMetricSummary,
    pub usage_and_cost_refs: Vec<CostRecordRefV1>,
    pub usage_and_cost_metrics: Vec<EvaluationQuantityComparisonV1>,
    pub comparability: Vec<EvaluationComparability>,
    pub protected_metric_results: Vec<ProtectedMetricResult>,
    pub limitations: Vec<String>,
    pub comparison: Vec<String>,
    pub recommendation: EvaluationRecommendation,
    pub decision_ref: Option<String>,
    pub radar_item_refs: Vec<String>,
    pub run_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CostScopeKindV1 {
    Goal,
    Stage,
    Attempt,
    ValidationRun,
    ExternalAction,
    Evaluation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CostUsageV1 {
    pub unit: String,
    pub quantity: u64,
    pub provider_evidence_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerifiedMonetaryCostV1 {
    pub amount_microunits: u64,
    pub currency: String,
    pub price_source_ref: String,
    pub provider_statement_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CostRecordV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub cost_record_id: String,
    pub revision: u64,
    pub project_id: ProjectId,
    pub scope_kind: CostScopeKindV1,
    pub scope_ref: String,
    #[serde(default)]
    pub validation_run_refs: Vec<ValidationRunId>,
    pub source: String,
    #[serde(default)]
    pub usage: Vec<CostUsageV1>,
    pub monetary_cost: Option<VerifiedMonetaryCostV1>,
    pub estimated: bool,
    pub paid_action: bool,
    pub measured_at: DateTime<Utc>,
    #[serde(default)]
    pub measurement_unavailable: Vec<String>,
    pub provider_evidence_refs: Vec<String>,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BudgetQuantityV1 {
    pub unit: String,
    pub quantity: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BudgetDecisionV1 {
    WithinBudget,
    ApprovalRequired,
    Exhausted,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BudgetSnapshotV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub snapshot_id: String,
    pub revision: u64,
    pub project_id: ProjectId,
    pub scope_ref: String,
    pub limits: Vec<BudgetQuantityV1>,
    pub observed: Vec<BudgetQuantityV1>,
    pub reserved: Vec<BudgetQuantityV1>,
    pub remaining: Vec<BudgetQuantityV1>,
    #[serde(default)]
    pub unknown_measurements: Vec<String>,
    pub decision: BudgetDecisionV1,
    pub cost_record_refs: Vec<CostRecordRefV1>,
    #[serde(default)]
    pub permission_approval_refs: Vec<String>,
    pub paid_action_pending: bool,
    pub config_fingerprint: Sha256Hash,
    pub evaluated_at: DateTime<Utc>,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationCatalogLifecycle {
    Active,
    Deprecated,
    Retired,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluationCatalogItem {
    pub schema_id: String,
    pub schema_version: u32,
    pub item_id: String,
    pub item_version: String,
    pub definition_fingerprint: Sha256Hash,
    pub trial_candidate: bool,
    pub lifecycle: EvaluationCatalogLifecycle,
    pub owner: String,
    pub corpus_ref: String,
    pub replacement_ref: Option<String>,
    pub migration_guide_ref: Option<String>,
    pub compatibility_deadline: Option<String>,
    pub last_evaluation_run_ref: Option<EvaluationRunId>,
    pub tombstone_ref: Option<String>,
    pub item_fingerprint: Sha256Hash,
}
