use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    ApprovalId, EvaluationRunId, GateId, ProjectId, ReleaseManifestId, ScopeRevisionId, Sha256Hash,
    TaskInvocationId, TaskSpecId, ValidationPlanId, ValidationRunId,
};

pub const RELEASE_MANIFEST_V2_SCHEMA_ID: &str = "star.release-manifest";
pub const EVALUATION_RUN_V2_SCHEMA_ID: &str = "star.evaluation-run";
pub const EVALUATION_CATALOG_ITEM_SCHEMA_ID: &str = "star.evaluation-catalog-item";

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluationCaseResult {
    pub case_id: String,
    pub case_version: String,
    pub corpus_ref: String,
    pub evaluation_context: EvaluationContext,
    pub task_source_binding: Sha256Hash,
    pub baseline_run_refs: Vec<ValidationRunId>,
    pub candidate_run_refs: Vec<ValidationRunId>,
    pub adjudication: CaseAdjudication,
    pub baseline_detected: bool,
    pub candidate_detected: bool,
    pub baseline_duration_ms: u64,
    pub candidate_duration_ms: u64,
    pub baseline_rework_count: u32,
    pub candidate_rework_count: u32,
    pub baseline_outcome: EvaluationOutcome,
    pub candidate_outcome: EvaluationOutcome,
    pub candidate_flaky: bool,
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
    pub baseline: EvaluationDefinition,
    pub candidate: EvaluationDefinition,
    pub mode: EvaluationMode,
    pub corpus_ref: String,
    pub case_selection_fingerprint: Sha256Hash,
    pub measurement_protocol_fingerprint: Sha256Hash,
    pub case_results: Vec<EvaluationCaseResult>,
    pub ground_truth_summary: EvaluationMetricSummary,
    pub finding_metrics: EvaluationMetricSummary,
    pub efficiency_metrics: EvaluationMetricSummary,
    pub usage_and_cost_refs: Vec<String>,
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
