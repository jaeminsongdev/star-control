//! Persisted M2 task, scope, impact, and full validation-plan contracts.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    Sha256Hash, canonical_sha256,
    evidence::{ActorRef, ArtifactRef, DocumentRef},
    ids::{
        ChangePlanId, ChangeSetId, CheckoutId, CodeIndexSnapshotId, FindingId, ImpactAnalysisId,
        ProjectCatalogSnapshotId, ProjectId, ProjectRevisionId, ScopeRevisionId, TaskSpecId,
        ValidationPlanId, WorkspaceSnapshotId,
    },
    index::{IndexFreshnessState, IndexTier, SourceClass},
    management::{ChangePlan, ChangeRecipeRef, ProjectPathRef},
    profile::DevelopmentProfileResolutionV1,
};

pub const TASK_SPEC_SCHEMA_ID: &str = "star.task-spec";
pub const SCOPE_REVISION_SCHEMA_ID: &str = "star.scope-revision";
pub const CHANGE_SET_SCHEMA_ID: &str = "star.change-set";
pub const IMPACT_ANALYSIS_SCHEMA_ID: &str = "star.impact-analysis";
pub const RISK_PATH_DESCRIPTOR_SCHEMA_ID: &str = "star.risk-path-descriptor";
pub const FULL_VALIDATION_PLAN_SCHEMA_ID: &str = "star.validation-plan";
pub const CHANGE_PLAN_V2_SCHEMA_ID: &str = "star.change-plan";
pub const PLANNING_BUNDLE_V2_SCHEMA_ID: &str = "star.planning-bundle";
pub const CHANGE_PLAN_V1_TO_V2_MIGRATION_PLAN_SCHEMA_ID: &str =
    "star.change-plan-v1-to-v2-migration-plan";
pub const CHANGE_PLAN_V1_TO_V2_MIGRATION_RESULT_SCHEMA_ID: &str =
    "star.change-plan-v1-to-v2-migration-result";

macro_rules! string_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    };
}

string_enum!(ProjectTargetRole {
    PlannedChange,
    ReadOnlyImpact,
    ValidationOnly
});
string_enum!(SelectorKind {
    Path,
    Package,
    Workspace,
    Symbol,
    Contract,
    ConfigKey,
    Schema,
    ManagedDeclaration,
    SourceClass
});
string_enum!(ScopeAxis {
    Analysis,
    PlannedChange,
    Validation,
    All
});
string_enum!(IntendedChangeKind {
    Add,
    Modify,
    Delete,
    Rename,
    ContractChange
});
string_enum!(BaselinePolicyKind {
    CurrentWorkspace,
    ExplicitRevision,
    PreviousSuccess
});
string_enum!(CheckOverrideKind { Add, Promote, Omit });
string_enum!(ScopeReasonCode {
    Initial,
    UserEdit,
    UnexpectedImpact,
    NewRisk,
    SourceChanged,
    CheckFallback
});
string_enum!(ScopeApprovalState {
    Accepted,
    Proposed,
    Rejected,
    Superseded
});
string_enum!(ScopeItemSource {
    User,
    TaskDescriptor,
    Impact,
    RiskPath,
    Fallback,
    UserOverride
});
string_enum!(ChangeSetKind {
    PlanningBaseline,
    PreviousSuccessDelta,
    RecipePreview,
    ObservedAfterChange,
    MergeResult
});
string_enum!(ObservedChangeKind {
    Add,
    Modify,
    Delete,
    Rename,
    Mode,
    Binary,
    Submodule
});
string_enum!(ChangeOrigin {
    Preexisting,
    TaskDeclared,
    ToolApplied,
    Unknown
});
string_enum!(ScopeRelation {
    Planned,
    NecessaryExpansion,
    Unrelated,
    Unknown
});
string_enum!(CollectionState {
    Complete,
    Partial,
    Unverified
});
string_enum!(SeedResolution {
    Resolved,
    Ambiguous,
    Unresolved,
    Excluded,
    Stale
});
string_enum!(ImpactKind { Direct, Transitive });
string_enum!(ImpactCertainty {
    Confirmed,
    Possible
});
string_enum!(ImpactConfidence { High, Medium, Low });
string_enum!(ImpactResolution {
    Resolved,
    Ambiguous,
    Unresolved,
    External
});
string_enum!(ImpactStatus {
    Complete,
    Partial,
    Blocked,
    Invalidated
});
string_enum!(NoResultReason {
    ConfirmedEmpty,
    NotIndexed,
    UnsupportedLanguage,
    ParseFailed,
    SemanticUnavailable,
    ExcludedByPolicy,
    Stale,
    Partial,
    Ambiguous,
    LimitExceeded,
    NoSeedMapping,
    DescriptorNotFound,
    NotApplicable
});
string_enum!(RiskSeverityFloor {
    Info,
    Warning,
    Error,
    Critical
});
string_enum!(ValidationRiskLevel {
    Low,
    Medium,
    High,
    Critical
});
string_enum!(ValidationPlanV2Readiness {
    Draft,
    Ready,
    Blocked,
    Invalidated
});
string_enum!(ValidationScopeLevel {
    Package,
    Workspace,
    ProjectFull
});
string_enum!(CheckApplicability {
    Applicable,
    NotApplicable,
    Unknown
});
string_enum!(CheckResolutionOutcome {
    SelectedRequired,
    SelectedOptional,
    OmittedNotApplicable,
    UnresolvedNotFound,
    BlockedUnavailable,
    UserWaived
});
string_enum!(ReviewKind {
    None,
    HumanSemantic,
    CodexIndependent
});

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningSelector {
    pub kind: SelectorKind,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectTarget {
    pub project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub role: ProjectTargetRole,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExcludedScope {
    pub selector: PlanningSelector,
    pub applies_to: ScopeAxis,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntendedChange {
    pub change_id: String,
    pub selector: PlanningSelector,
    pub change_kind: IntendedChangeKind,
    pub intended_postcondition: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SuccessCriterion {
    pub criterion_id: String,
    pub description: String,
    pub verification: String,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BaselinePolicy {
    pub kind: BaselinePolicyKind,
    pub reference: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckOverride {
    pub family: String,
    pub kind: CheckOverrideKind,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskSpec {
    pub schema_id: String,
    pub schema_version: u32,
    pub task_spec_id: TaskSpecId,
    #[schemars(range(min = 1))]
    pub revision: u64,
    #[schemars(length(min = 1))]
    pub title: String,
    #[schemars(length(min = 1))]
    pub objective: String,
    #[schemars(length(min = 1))]
    pub project_targets: Vec<ProjectTarget>,
    #[schemars(length(min = 1))]
    pub included_scope: Vec<PlanningSelector>,
    pub excluded_scope: Vec<ExcludedScope>,
    #[schemars(length(min = 1))]
    pub intended_changes: Vec<IntendedChange>,
    #[schemars(length(min = 1))]
    pub success_criteria: Vec<SuccessCriterion>,
    pub constraints: Vec<String>,
    pub forbidden_actions: Vec<String>,
    #[serde(default)]
    pub profile_ids: Vec<String>,
    pub baseline_policy: BaselinePolicy,
    pub requested_checks: Vec<String>,
    pub check_overrides: Vec<CheckOverride>,
    pub assumptions: Vec<String>,
    pub created_by: ActorRef,
    pub created_at: DateTime<Utc>,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScopedSelector {
    pub selector: PlanningSelector,
    pub source: ScopeItemSource,
    pub reason_code: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScopeSet {
    pub project_ids: Vec<ProjectId>,
    pub selectors: Vec<ScopedSelector>,
    pub exclusions: Vec<ExcludedScope>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScopeSourceSnapshotRef {
    pub project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub project_catalog_snapshot_id: ProjectCatalogSnapshotId,
    pub project_revision_id: ProjectRevisionId,
    pub workspace_snapshot_id: WorkspaceSnapshotId,
    pub code_index_snapshot_id: CodeIndexSnapshotId,
    pub freshness: IndexFreshnessState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScopeDerivedAddition {
    pub axis: ScopeAxis,
    pub selector: PlanningSelector,
    pub source: ScopeItemSource,
    pub reason_code: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScopeUserDecision {
    pub decision_id: String,
    pub state: ScopeApprovalState,
    pub selector: PlanningSelector,
    pub reason: String,
    pub actor: ActorRef,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScopeRevision {
    pub schema_id: String,
    pub schema_version: u32,
    pub scope_revision_id: ScopeRevisionId,
    #[schemars(range(min = 1))]
    pub revision: u64,
    pub task_spec_ref: DocumentRef,
    pub previous_scope_revision_ref: Option<DocumentRef>,
    pub reason_code: ScopeReasonCode,
    pub reason: String,
    pub requested_scope: ScopeSet,
    pub analysis_scope: ScopeSet,
    pub planned_change_scope: ScopeSet,
    pub validation_scope: ScopeSet,
    #[schemars(length(min = 1))]
    pub source_snapshot_refs: Vec<ScopeSourceSnapshotRef>,
    pub derived_additions: Vec<ScopeDerivedAddition>,
    pub user_decisions: Vec<ScopeUserDecision>,
    pub changed_fields: Vec<String>,
    pub approval_state: ScopeApprovalState,
    pub scope_hash: Sha256Hash,
    pub created_by: ActorRef,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeEntry {
    pub entry_id: String,
    pub path: ProjectPathRef,
    pub rename_from: Option<ProjectPathRef>,
    pub change_kind: ObservedChangeKind,
    pub before_sha256: Option<Sha256Hash>,
    pub after_sha256: Option<Sha256Hash>,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub binary: bool,
    pub source_class: SourceClass,
    pub origin: ChangeOrigin,
    pub scope_relation: ScopeRelation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeSet {
    pub schema_id: String,
    pub schema_version: u32,
    pub change_set_id: ChangeSetId,
    pub task_spec_ref: DocumentRef,
    pub scope_revision_ref: DocumentRef,
    pub project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub change_set_kind: ChangeSetKind,
    pub base_revision_id: ProjectRevisionId,
    pub observed_workspace_snapshot_id: WorkspaceSnapshotId,
    pub comparison_scope: Vec<PlanningSelector>,
    pub entries: Vec<ChangeEntry>,
    pub collection_limits: Vec<String>,
    pub collection_state: CollectionState,
    pub change_set_fingerprint: Sha256Hash,
    pub captured_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImpactProjectInput {
    pub project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub project_catalog_snapshot_id: ProjectCatalogSnapshotId,
    pub code_index_snapshot_id: CodeIndexSnapshotId,
    pub project_revision_id: ProjectRevisionId,
    pub workspace_snapshot_id: WorkspaceSnapshotId,
    pub freshness: IndexFreshnessState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImpactSeed {
    pub seed_id: String,
    pub project_id: ProjectId,
    pub selector: PlanningSelector,
    pub entity_key: Option<String>,
    pub resolution: SeedResolution,
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImpactEdge {
    pub edge_id: String,
    pub project_id: ProjectId,
    pub from_entity_key: String,
    pub to_entity_key: String,
    pub relation: String,
    pub impact_kind: ImpactKind,
    pub distance: u32,
    pub certainty: ImpactCertainty,
    pub confidence: ImpactConfidence,
    pub resolution: ImpactResolution,
    pub tier: IndexTier,
    pub freshness: IndexFreshnessState,
    pub evidence_refs: Vec<String>,
    pub path_edge_ids: Vec<String>,
    pub limitations: Vec<String>,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImpactedNode {
    pub project_id: ProjectId,
    pub entity_key: String,
    pub kind: String,
    pub impact_kind: ImpactKind,
    pub certainty: ImpactCertainty,
    pub confidence: ImpactConfidence,
    pub minimum_distance: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RiskPathDescriptor {
    pub schema_id: String,
    pub schema_version: u32,
    pub risk_id: String,
    pub version: String,
    pub selector_kinds: Vec<SelectorKind>,
    pub source_classes: Vec<SourceClass>,
    pub entity_kinds: Vec<String>,
    #[schemars(length(min = 1))]
    pub required_check_families: Vec<String>,
    pub severity_floor: RiskSeverityFloor,
    pub fallback_floor: ValidationScopeLevel,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RiskPathFinding {
    pub finding_id: String,
    pub risk_id: String,
    pub risk_version: String,
    pub project_id: ProjectId,
    pub seed_ids: Vec<String>,
    pub impact_edge_ids: Vec<String>,
    pub certainty: ImpactCertainty,
    pub severity_floor: RiskSeverityFloor,
    pub required_check_families: Vec<String>,
    pub fallback_floor: ValidationScopeLevel,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoResult {
    pub query_kind: String,
    pub reason: NoResultReason,
    pub searched_scope: Vec<PlanningSelector>,
    pub required_tier: Option<IndexTier>,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImpactConfidenceSummary {
    pub confirmed: u64,
    pub possible: u64,
    pub high: u64,
    pub medium: u64,
    pub low: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AffectedProject {
    pub project_id: ProjectId,
    pub certainty: ImpactCertainty,
    pub closure_complete: bool,
    pub impacted_node_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImpactAnalysis {
    pub schema_id: String,
    pub schema_version: u32,
    pub impact_analysis_id: ImpactAnalysisId,
    #[schemars(range(min = 1))]
    pub revision: u64,
    pub task_spec_ref: DocumentRef,
    pub scope_revision_ref: DocumentRef,
    #[schemars(length(min = 1))]
    pub project_inputs: Vec<ImpactProjectInput>,
    #[schemars(length(min = 1))]
    pub change_set_refs: Vec<DocumentRef>,
    pub catalog_snapshot_ref: DocumentRef,
    pub effective_config_fingerprint: Sha256Hash,
    #[schemars(length(min = 1))]
    pub seeds: Vec<ImpactSeed>,
    pub impacted_nodes: Vec<ImpactedNode>,
    pub impact_edges: Vec<ImpactEdge>,
    pub risk_paths: Vec<RiskPathFinding>,
    pub affected_projects: Vec<AffectedProject>,
    pub no_results: Vec<NoResult>,
    /// Read-only advisory evidence (for example Git history observations).
    /// Unlike `limitations`, this never changes the Impact/Gate readiness.
    #[serde(default)]
    pub advisory_evidence_refs: Vec<String>,
    pub limitations: Vec<String>,
    pub confidence_summary: ImpactConfidenceSummary,
    pub calculation_fingerprint: Sha256Hash,
    pub status: ImpactStatus,
    pub generated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckDescriptor {
    pub check_id: String,
    pub family: String,
    #[serde(default)]
    pub project_ids: Vec<ProjectId>,
    pub tool_id: String,
    pub logical_executable: String,
    pub argument_template: Vec<String>,
    pub supported_scope_levels: Vec<ValidationScopeLevel>,
    pub applicable_source_classes: Vec<SourceClass>,
    pub trusted: bool,
    pub available: bool,
    #[serde(default)]
    pub output_normalizer: CheckOutputNormalizer,
    pub required_evidence: Vec<String>,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckOutputNormalizer {
    #[default]
    SafeExitV1,
    SarifV210,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckCandidate {
    pub family: String,
    pub check_id: Option<String>,
    pub applicability: CheckApplicability,
    pub outcome: CheckResolutionOutcome,
    pub evidence_refs: Vec<String>,
    pub reason_code: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AffectedScope {
    pub project_id: ProjectId,
    pub requested_level: ValidationScopeLevel,
    pub selected_level: ValidationScopeLevel,
    pub selectors: Vec<PlanningSelector>,
    pub reason_codes: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckInvocationTemplate {
    pub logical_executable: String,
    pub args: Vec<String>,
    pub timeout_ms: u64,
    pub expected_exit_codes: Vec<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckPlanV2 {
    pub plan_item_id: String,
    pub check_id: String,
    pub descriptor_ref: DocumentRef,
    pub tool_id: String,
    pub family: String,
    pub project_id: ProjectId,
    pub scope_level: ValidationScopeLevel,
    pub outcome: CheckResolutionOutcome,
    pub reason_codes: Vec<String>,
    pub impact_edge_ids: Vec<String>,
    pub risk_path_ids: Vec<String>,
    pub invocation: CheckInvocationTemplate,
    #[serde(default)]
    pub output_normalizer: CheckOutputNormalizer,
    pub fallback_floor: ValidationScopeLevel,
    pub evidence_kinds: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckGraphEdgeV2 {
    pub from_plan_item_id: String,
    pub to_plan_item_id: String,
    pub relation: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckGraphV2 {
    pub nodes: Vec<String>,
    pub edges: Vec<CheckGraphEdgeV2>,
    #[schemars(range(min = 1))]
    pub max_parallel: u32,
    pub failure_policy: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UnresolvedCheck {
    pub family: String,
    pub reason: String,
    pub searched_catalog_scope: String,
    pub required_coverage: String,
    pub readiness_impact: ValidationPlanV2Readiness,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FallbackDecision {
    pub project_id: ProjectId,
    pub from_level: ValidationScopeLevel,
    pub to_level: ValidationScopeLevel,
    pub trigger: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewRequirementV2 {
    pub required: bool,
    pub review_kind: ReviewKind,
    pub reason_codes: Vec<String>,
    pub absence_behavior: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatePolicyV2 {
    pub fail_on_required_failure: bool,
    pub fail_on_partial: bool,
    pub fail_on_unverified: bool,
    pub fail_on_flaky: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FullValidationPlan {
    pub schema_id: String,
    pub schema_version: u32,
    pub validation_plan_id: ValidationPlanId,
    #[schemars(range(min = 1))]
    pub revision: u64,
    pub task_spec_ref: DocumentRef,
    #[schemars(range(min = 1))]
    pub scope_revision: u64,
    pub scope_revision_ref: DocumentRef,
    pub phase: String,
    pub change_set_refs: Vec<DocumentRef>,
    pub impact_analysis_ref: DocumentRef,
    pub risk_level: ValidationRiskLevel,
    pub affected_scope: Vec<AffectedScope>,
    pub candidate_checks: Vec<CheckCandidate>,
    #[schemars(length(min = 1))]
    pub required_checks: Vec<CheckPlanV2>,
    pub optional_checks: Vec<CheckPlanV2>,
    pub check_graph: CheckGraphV2,
    pub omitted_checks: Vec<String>,
    pub unresolved_checks: Vec<UnresolvedCheck>,
    pub previous_success_comparisons: Vec<String>,
    pub fallback_decisions: Vec<FallbackDecision>,
    pub manual_observations: Vec<String>,
    pub independent_review: ReviewRequirementV2,
    pub gate_policy: GatePolicyV2,
    pub config_fingerprint: Sha256Hash,
    pub catalog_snapshot_ref: DocumentRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_resolution: Option<DevelopmentProfileResolutionV1>,
    pub selection_fingerprint: Sha256Hash,
    pub readiness: ValidationPlanV2Readiness,
}

string_enum!(ChangePlanOriginV2 {
    UserPlanned,
    FindingRecipe,
    Mixed
});
string_enum!(ChangePlanReadinessV2 {
    Draft,
    Ready,
    Blocked,
    Invalidated
});
string_enum!(ChangePlanStatusV2 {
    Draft,
    Ready,
    Applied,
    Validated,
    Blocked,
    Abandoned,
    Superseded
});
string_enum!(ChangeUnitSourceV2 {
    User,
    AcceptedScopeRevision
});
string_enum!(ChangeGraphRelationV2 {
    Requires,
    MustPrecede,
    SameAtomicGroup
});

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FindingRefV2 {
    pub finding_id: FindingId,
    pub finding_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlannedChangeUnitV2 {
    pub unit_id: String,
    pub target_selector: PlanningSelector,
    pub change_kind: IntendedChangeKind,
    pub intended_postcondition: String,
    pub source: ChangeUnitSourceV2,
    pub reason: String,
    pub expected_paths: Vec<ProjectPathRef>,
    pub unresolved_target: Option<String>,
    pub precondition_fingerprints: Vec<Sha256Hash>,
    pub permission_requirements: Vec<String>,
    pub risk_path_refs: Vec<String>,
    pub impact_edge_refs: Vec<String>,
    pub completion_criterion_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeGraphEdgeV2 {
    pub from_unit_id: String,
    pub to_unit_id: String,
    pub relation: ChangeGraphRelationV2,
    pub atomic_group_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RelatedProjectImpactV2 {
    pub project_id: ProjectId,
    pub impact_analysis_ref: DocumentRef,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExpectedImpactRefV2 {
    pub unit_id: String,
    pub accepted_impact_edge_ids: Vec<String>,
    pub unresolved_frontier_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompletionCriterionMappingV2 {
    pub criterion_id: String,
    pub unit_ids: Vec<String>,
    pub check_plan_item_ids: Vec<String>,
    pub manual_observation_refs: Vec<String>,
    pub explicit_user_decision_omission: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangePlanV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub change_plan_id: ChangePlanId,
    #[schemars(range(min = 1))]
    pub revision: u64,
    pub task_spec_ref: DocumentRef,
    pub scope_revision_ref: DocumentRef,
    pub impact_analysis_ref: DocumentRef,
    pub change_origin: ChangePlanOriginV2,
    pub project_id: ProjectId,
    pub target_checkout_id: CheckoutId,
    pub target_project_revision_id: ProjectRevisionId,
    pub target_workspace_snapshot_id: WorkspaceSnapshotId,
    pub change_set_ref: DocumentRef,
    pub related_project_impacts: Vec<RelatedProjectImpactV2>,
    pub planned_change_units: Vec<PlannedChangeUnitV2>,
    pub change_graph: Vec<ChangeGraphEdgeV2>,
    pub deterministic_unit_order: Vec<String>,
    pub expected_impact_refs: Vec<ExpectedImpactRefV2>,
    pub completion_criteria_mapping: Vec<CompletionCriterionMappingV2>,
    pub expected_paths: Vec<ProjectPathRef>,
    pub finding_refs: Vec<FindingRefV2>,
    pub recipe_refs: Vec<ChangeRecipeRef>,
    pub parameters: BTreeMap<String, String>,
    pub risk_path_refs: Vec<String>,
    pub preconditions: Vec<Sha256Hash>,
    pub unresolved_impacts: Vec<String>,
    pub permission_requirements: Vec<String>,
    pub permission_plan_ref: Option<DocumentRef>,
    pub validation_plan_ref: DocumentRef,
    pub readiness: ChangePlanReadinessV2,
    pub status: ChangePlanStatusV2,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub content_fingerprint: Sha256Hash,
}

impl ChangePlanV2 {
    pub fn seal(mut self) -> Result<Self, PlanningContractError> {
        self.planned_change_units
            .sort_by(|left, right| left.unit_id.cmp(&right.unit_id));
        for unit in &mut self.planned_change_units {
            normalize_nonempty_strings(&mut unit.permission_requirements)?;
            normalize_nonempty_strings(&mut unit.risk_path_refs)?;
            normalize_nonempty_strings(&mut unit.impact_edge_refs)?;
            normalize_nonempty_strings(&mut unit.completion_criterion_refs)?;
            unit.expected_paths.sort();
            unit.expected_paths.dedup();
            unit.precondition_fingerprints.sort();
            unit.precondition_fingerprints.dedup();
            if unit.unit_id.trim().is_empty()
                || unit.target_selector.value.trim().is_empty()
                || unit.intended_postcondition.trim().is_empty()
                || unit.reason.trim().is_empty()
                || unit
                    .unresolved_target
                    .as_deref()
                    .is_some_and(|value| value.trim().is_empty())
            {
                return Err(PlanningContractError::Empty);
            }
        }
        if self
            .planned_change_units
            .windows(2)
            .any(|pair| pair[0].unit_id == pair[1].unit_id)
        {
            return Err(PlanningContractError::Ordering);
        }
        self.change_graph.sort_by(|left, right| {
            (&left.from_unit_id, &left.to_unit_id, left.relation).cmp(&(
                &right.from_unit_id,
                &right.to_unit_id,
                right.relation,
            ))
        });
        self.related_project_impacts
            .sort_by(|left, right| left.project_id.cmp(&right.project_id));
        self.expected_impact_refs
            .sort_by(|left, right| left.unit_id.cmp(&right.unit_id));
        self.completion_criteria_mapping
            .sort_by(|left, right| left.criterion_id.cmp(&right.criterion_id));
        self.expected_paths.sort();
        self.expected_paths.dedup();
        self.finding_refs
            .sort_by(|left, right| left.finding_id.cmp(&right.finding_id));
        self.recipe_refs.sort_by(|left, right| {
            (
                &left.recipe_id,
                &left.recipe_version,
                &left.definition_fingerprint,
            )
                .cmp(&(
                    &right.recipe_id,
                    &right.recipe_version,
                    &right.definition_fingerprint,
                ))
        });
        normalize_nonempty_strings(&mut self.risk_path_refs)?;
        normalize_nonempty_strings(&mut self.unresolved_impacts)?;
        normalize_nonempty_strings(&mut self.permission_requirements)?;
        self.preconditions.sort();
        self.preconditions.dedup();
        let unit_ids = self
            .planned_change_units
            .iter()
            .map(|unit| unit.unit_id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        if self.schema_id != CHANGE_PLAN_V2_SCHEMA_ID
            || self.schema_version != 2
            || self.revision == 0
            || self.updated_at < self.created_at
            || self.planned_change_units.is_empty()
            || self.preconditions.is_empty()
            || self.permission_requirements.is_empty()
            || self.change_graph.iter().any(|edge| {
                !unit_ids.contains(&edge.from_unit_id)
                    || !unit_ids.contains(&edge.to_unit_id)
                    || edge.from_unit_id == edge.to_unit_id
                    || (edge.relation == ChangeGraphRelationV2::SameAtomicGroup)
                        != edge.atomic_group_id.is_some()
            })
            || self
                .deterministic_unit_order
                .iter()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>()
                != unit_ids
            || !acyclic_change_graph(&unit_ids, &self.change_graph)
            || self.expected_impact_refs.iter().any(|reference| {
                !unit_ids.contains(&reference.unit_id)
                    || reference
                        .accepted_impact_edge_ids
                        .iter()
                        .any(|value| value.trim().is_empty())
            })
            || self.completion_criteria_mapping.iter().any(|mapping| {
                mapping.criterion_id.trim().is_empty()
                    || mapping.unit_ids.iter().any(|unit| !unit_ids.contains(unit))
                    || (mapping.unit_ids.is_empty()
                        && mapping.check_plan_item_ids.is_empty()
                        && mapping.manual_observation_refs.is_empty()
                        && mapping.explicit_user_decision_omission.is_none())
            })
            || (self.change_origin == ChangePlanOriginV2::FindingRecipe
                && self.finding_refs.is_empty())
            || (self.change_origin == ChangePlanOriginV2::UserPlanned
                && !self.finding_refs.is_empty())
        {
            return Err(PlanningContractError::Identity);
        }
        if self.readiness == ChangePlanReadinessV2::Ready
            && (self.status != ChangePlanStatusV2::Ready
                || !self.unresolved_impacts.is_empty()
                || self.permission_plan_ref.is_some())
        {
            return Err(PlanningContractError::Readiness);
        }
        self.content_fingerprint = fingerprint(
            CHANGE_PLAN_V2_SCHEMA_ID,
            2,
            &serde_json::json!({
                "change_plan_id":self.change_plan_id,
                "revision":self.revision,
                "task_spec_ref":self.task_spec_ref,
                "scope_revision_ref":self.scope_revision_ref,
                "impact_analysis_ref":self.impact_analysis_ref,
                "change_origin":self.change_origin,
                "project_id":self.project_id,
                "target_checkout_id":self.target_checkout_id,
                "target_project_revision_id":self.target_project_revision_id,
                "target_workspace_snapshot_id":self.target_workspace_snapshot_id,
                "change_set_ref":self.change_set_ref,
                "related_project_impacts":self.related_project_impacts,
                "planned_change_units":self.planned_change_units,
                "change_graph":self.change_graph,
                "deterministic_unit_order":self.deterministic_unit_order,
                "expected_impact_refs":self.expected_impact_refs,
                "completion_criteria_mapping":self.completion_criteria_mapping,
                "expected_paths":self.expected_paths,
                "finding_refs":self.finding_refs,
                "recipe_refs":self.recipe_refs,
                "parameters":self.parameters,
                "risk_path_refs":self.risk_path_refs,
                "preconditions":self.preconditions,
                "unresolved_impacts":self.unresolved_impacts,
                "permission_requirements":self.permission_requirements,
                "permission_plan_ref":self.permission_plan_ref,
                "validation_plan_ref":self.validation_plan_ref,
                "readiness":self.readiness,
                "status":self.status,
                "created_at":self.created_at,
                "updated_at":self.updated_at,
            }),
        )?;
        Ok(self)
    }

    pub fn reference(&self) -> Result<DocumentRef, PlanningContractError> {
        Ok(DocumentRef {
            schema_id: CHANGE_PLAN_V2_SCHEMA_ID.to_owned(),
            document_id: self.change_plan_id.to_string(),
            revision: self.revision,
            sha256: document_hash(self)?,
        })
    }
}

/// Projects a persisted v1 ChangePlan into the v2 planning contract without
/// inventing readiness. Fields that v1 never recorded are represented as
/// explicit unresolved inputs, so a migrated document always requires M2
/// replanning before it can become executable.
pub fn migrate_change_plan_v1_to_v2(
    value: &ChangePlan,
    bundle: &PlanningBundle,
) -> Result<ChangePlanV2, PlanningContractError> {
    let candidates = bundle
        .change_sets
        .iter()
        .filter(|change_set| {
            change_set.project_id == value.project_id
                && change_set.observed_workspace_snapshot_id == value.target_workspace_snapshot_id
        })
        .collect::<Vec<_>>();
    let [change_set] = candidates.as_slice() else {
        return Err(PlanningContractError::Migration);
    };
    let task_spec_ref = document_ref(
        TASK_SPEC_SCHEMA_ID,
        bundle.task_spec.task_spec_id.as_str(),
        bundle.task_spec.revision,
        &bundle.task_spec.content_fingerprint,
    );
    let scope_revision_ref = document_ref(
        SCOPE_REVISION_SCHEMA_ID,
        bundle.scope_revision.scope_revision_id.as_str(),
        bundle.scope_revision.revision,
        &bundle.scope_revision.scope_hash,
    );
    let impact_analysis_ref = document_ref(
        IMPACT_ANALYSIS_SCHEMA_ID,
        bundle.impact_analysis.impact_analysis_id.as_str(),
        bundle.impact_analysis.revision,
        &bundle.impact_analysis.calculation_fingerprint,
    );
    let change_set_ref = document_ref(
        CHANGE_SET_SCHEMA_ID,
        change_set.change_set_id.as_str(),
        1,
        &change_set.change_set_fingerprint,
    );
    let validation_plan_ref = document_ref(
        FULL_VALIDATION_PLAN_SCHEMA_ID,
        bundle.validation_plan.validation_plan_id.as_str(),
        bundle.validation_plan.revision,
        &bundle.validation_plan.selection_fingerprint,
    );
    let fallback_precondition = document_hash(value)?;
    let preconditions = if value.preconditions.is_empty() {
        vec![fallback_precondition]
    } else {
        value.preconditions.clone()
    };
    let mut expected_paths = value.expected_paths.clone();
    expected_paths.sort();
    expected_paths.dedup();
    let mut planned_change_units = Vec::new();
    for (index, path) in expected_paths.iter().enumerate() {
        let intended = bundle.task_spec.intended_changes.iter().find(|change| {
            change.selector.kind == SelectorKind::Path && change.selector.value == path.as_str()
        });
        let observed = change_set.entries.iter().find(|entry| entry.path == *path);
        let change_kind = intended
            .map(|change| change.change_kind)
            .unwrap_or_else(|| {
                observed
                    .map(|entry| match entry.change_kind {
                        ObservedChangeKind::Add => IntendedChangeKind::Add,
                        ObservedChangeKind::Modify => IntendedChangeKind::Modify,
                        ObservedChangeKind::Delete => IntendedChangeKind::Delete,
                        ObservedChangeKind::Rename => IntendedChangeKind::Rename,
                        ObservedChangeKind::Mode
                        | ObservedChangeKind::Binary
                        | ObservedChangeKind::Submodule => IntendedChangeKind::Modify,
                    })
                    .unwrap_or(IntendedChangeKind::Modify)
            });
        planned_change_units.push(PlannedChangeUnitV2 {
            unit_id: format!("legacy-unit-{index:04}"),
            target_selector: PlanningSelector {
                kind: SelectorKind::Path,
                value: path.as_str().to_owned(),
            },
            change_kind,
            intended_postcondition: intended
                .map(|change| change.intended_postcondition.clone())
                .unwrap_or_else(|| "legacy intent requires explicit M2 replan".to_owned()),
            source: ChangeUnitSourceV2::AcceptedScopeRevision,
            reason: "migrated from persisted ChangePlan v1".to_owned(),
            expected_paths: vec![path.clone()],
            unresolved_target: Some("LEGACY_CHANGE_PLAN_REQUIRES_REPLAN".to_owned()),
            precondition_fingerprints: preconditions.clone(),
            permission_requirements: vec!["local_write".to_owned()],
            risk_path_refs: vec![format!("legacy-risk:{}", value.risk)],
            impact_edge_refs: Vec::new(),
            completion_criterion_refs: bundle
                .task_spec
                .success_criteria
                .iter()
                .map(|criterion| criterion.criterion_id.clone())
                .collect(),
        });
    }
    if planned_change_units.is_empty() {
        return Err(PlanningContractError::Migration);
    }
    let unit_ids = planned_change_units
        .iter()
        .map(|unit| unit.unit_id.clone())
        .collect::<Vec<_>>();
    let finding_refs = value
        .finding_refs
        .iter()
        .map(|finding_id| FindingRefV2 {
            finding_id: finding_id.clone(),
            finding_fingerprint: Sha256Hash::digest(
                format!("legacy-unverified-finding:{}", finding_id.as_str()).as_bytes(),
            ),
        })
        .collect::<Vec<_>>();
    ChangePlanV2 {
        schema_id: CHANGE_PLAN_V2_SCHEMA_ID.to_owned(),
        schema_version: 2,
        change_plan_id: value.change_plan_id.clone(),
        revision: value.revision.max(1),
        task_spec_ref,
        scope_revision_ref,
        impact_analysis_ref,
        change_origin: if finding_refs.is_empty() {
            ChangePlanOriginV2::UserPlanned
        } else {
            ChangePlanOriginV2::FindingRecipe
        },
        project_id: value.project_id.clone(),
        target_checkout_id: change_set.checkout_id.clone(),
        target_project_revision_id: change_set.base_revision_id.clone(),
        target_workspace_snapshot_id: value.target_workspace_snapshot_id.clone(),
        change_set_ref,
        related_project_impacts: Vec::new(),
        planned_change_units,
        change_graph: Vec::new(),
        deterministic_unit_order: unit_ids.clone(),
        expected_impact_refs: unit_ids
            .iter()
            .map(|unit_id| ExpectedImpactRefV2 {
                unit_id: unit_id.clone(),
                accepted_impact_edge_ids: Vec::new(),
                unresolved_frontier_refs: vec!["LEGACY_IMPACT_REQUIRES_REPLAN".to_owned()],
            })
            .collect(),
        completion_criteria_mapping: bundle
            .task_spec
            .success_criteria
            .iter()
            .map(|criterion| CompletionCriterionMappingV2 {
                criterion_id: criterion.criterion_id.clone(),
                unit_ids: unit_ids.clone(),
                check_plan_item_ids: bundle
                    .validation_plan
                    .required_checks
                    .iter()
                    .map(|check| check.plan_item_id.clone())
                    .collect(),
                manual_observation_refs: Vec::new(),
                explicit_user_decision_omission: None,
            })
            .collect(),
        expected_paths,
        finding_refs,
        recipe_refs: value.recipe_refs.clone(),
        parameters: value.parameters.clone(),
        risk_path_refs: vec![format!("legacy-risk:{}", value.risk)],
        preconditions,
        unresolved_impacts: vec![
            "LEGACY_CHANGE_PLAN_REQUIRES_REPLAN".to_owned(),
            "LEGACY_FINDING_FINGERPRINT_UNVERIFIED".to_owned(),
            "LEGACY_PERMISSION_DOCUMENT_UNAVAILABLE".to_owned(),
        ],
        permission_requirements: vec!["local_write".to_owned(), value.permission_plan_ref.clone()],
        permission_plan_ref: None,
        validation_plan_ref,
        readiness: ChangePlanReadinessV2::Blocked,
        status: ChangePlanStatusV2::Blocked,
        created_at: value.created_at,
        updated_at: value.updated_at,
        content_fingerprint: Sha256Hash::digest(b"unsealed"),
    }
    .seal()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangePlanV1ToV2MigrationEntry {
    pub legacy_change_plan_ref: DocumentRef,
    pub projected_change_plan: ChangePlanV2,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangePlanV1ToV2MigrationPlan {
    pub schema_id: String,
    pub schema_version: u32,
    pub project_id: ProjectId,
    pub task_spec_ref: DocumentRef,
    pub entries: Vec<ChangePlanV1ToV2MigrationEntry>,
    pub dry_run: bool,
    pub backup_required: bool,
    pub rollback_supported: bool,
    pub plan_fingerprint: Sha256Hash,
}

impl ChangePlanV1ToV2MigrationPlan {
    pub fn seal(mut self) -> Result<Self, PlanningContractError> {
        self.entries.sort_by(|left, right| {
            left.legacy_change_plan_ref
                .document_id
                .cmp(&right.legacy_change_plan_ref.document_id)
        });
        for entry in &mut self.entries {
            entry.projected_change_plan = entry.projected_change_plan.clone().seal()?;
            normalize_nonempty_strings(&mut entry.limitations)?;
        }
        if self.schema_id != CHANGE_PLAN_V1_TO_V2_MIGRATION_PLAN_SCHEMA_ID
            || self.schema_version != 1
            || self.entries.is_empty()
            || !self.dry_run
            || !self.backup_required
            || !self.rollback_supported
            || self.task_spec_ref.revision == 0
            || self.entries.iter().any(|entry| {
                entry.projected_change_plan.project_id != self.project_id
                    || entry.legacy_change_plan_ref.document_id
                        != entry.projected_change_plan.change_plan_id.as_str()
            })
            || self.entries.windows(2).any(|pair| {
                pair[0].legacy_change_plan_ref.document_id
                    == pair[1].legacy_change_plan_ref.document_id
            })
        {
            return Err(PlanningContractError::Migration);
        }
        self.plan_fingerprint = fingerprint(
            CHANGE_PLAN_V1_TO_V2_MIGRATION_PLAN_SCHEMA_ID,
            1,
            &serde_json::json!({
                "project_id":self.project_id,
                "task_spec_ref":self.task_spec_ref,
                "entries":self.entries,
                "dry_run":self.dry_run,
                "backup_required":self.backup_required,
                "rollback_supported":self.rollback_supported,
            }),
        )?;
        Ok(self)
    }
}

string_enum!(ChangePlanMigrationOutcomeV1 {
    Applied,
    RolledBack,
    Incompatible
});

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangePlanV1ToV2MigrationResult {
    pub schema_id: String,
    pub schema_version: u32,
    pub project_id: ProjectId,
    pub task_spec_ref: DocumentRef,
    pub plan_fingerprint: Sha256Hash,
    pub backup_manifest_ref: Option<ArtifactRef>,
    pub migrated_change_plan_refs: Vec<DocumentRef>,
    pub outcome: ChangePlanMigrationOutcomeV1,
    pub reason_codes: Vec<String>,
    pub completed_at: DateTime<Utc>,
    pub result_fingerprint: Sha256Hash,
}

impl ChangePlanV1ToV2MigrationResult {
    pub fn seal(mut self) -> Result<Self, PlanningContractError> {
        self.migrated_change_plan_refs.sort_by(|left, right| {
            (&left.document_id, left.revision).cmp(&(&right.document_id, right.revision))
        });
        self.migrated_change_plan_refs.dedup();
        normalize_nonempty_strings(&mut self.reason_codes)?;
        if self.schema_id != CHANGE_PLAN_V1_TO_V2_MIGRATION_RESULT_SCHEMA_ID
            || self.schema_version != 1
            || self.task_spec_ref.revision == 0
            || self
                .backup_manifest_ref
                .as_ref()
                .is_some_and(|reference| reference.validate().is_err())
            || (self.outcome == ChangePlanMigrationOutcomeV1::Applied
                && (self.backup_manifest_ref.is_none()
                    || self.migrated_change_plan_refs.is_empty()
                    || !self.reason_codes.is_empty()))
            || (self.outcome != ChangePlanMigrationOutcomeV1::Applied
                && self.reason_codes.is_empty())
        {
            return Err(PlanningContractError::Migration);
        }
        self.result_fingerprint = fingerprint(
            CHANGE_PLAN_V1_TO_V2_MIGRATION_RESULT_SCHEMA_ID,
            1,
            &serde_json::json!({
                "project_id":self.project_id,
                "task_spec_ref":self.task_spec_ref,
                "plan_fingerprint":self.plan_fingerprint,
                "backup_manifest_ref":self.backup_manifest_ref,
                "migrated_change_plan_refs":self.migrated_change_plan_refs,
                "outcome":self.outcome,
                "reason_codes":self.reason_codes,
                "completed_at":self.completed_at,
            }),
        )?;
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningBundle {
    pub schema_id: String,
    pub schema_version: u32,
    pub task_spec: TaskSpec,
    pub scope_revision: ScopeRevision,
    pub change_sets: Vec<ChangeSet>,
    pub impact_analysis: ImpactAnalysis,
    #[serde(default)]
    pub change_plans: Vec<ChangePlanV2>,
    pub validation_plan: FullValidationPlan,
    pub bundle_fingerprint: Sha256Hash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlanningContractError {
    #[error("planning document schema identity is invalid")]
    Schema,
    #[error("planning document has an empty required value")]
    Empty,
    #[error("planning document ordering or uniqueness is invalid")]
    Ordering,
    #[error("planning document identity or cross-reference is invalid")]
    Identity,
    #[error("planning document cannot claim ready under unresolved inputs")]
    Readiness,
    #[error("planning fingerprint could not be calculated")]
    Fingerprint,
    #[error("legacy planning document cannot be migrated safely")]
    Migration,
}

pub fn document_ref(
    schema_id: &str,
    document_id: &str,
    revision: u64,
    fingerprint: &Sha256Hash,
) -> DocumentRef {
    DocumentRef {
        schema_id: schema_id.to_owned(),
        document_id: document_id.to_owned(),
        revision,
        sha256: fingerprint.clone(),
    }
}

fn fingerprint<T: Serialize>(
    domain: &str,
    version: u32,
    value: &T,
) -> Result<Sha256Hash, PlanningContractError> {
    canonical_sha256(&serde_json::json!({
        "domain":domain,
        "version":version,
        "value":value,
    }))
    .map_err(|_| PlanningContractError::Fingerprint)
}

fn sorted_unique<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn non_empty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
}

fn bounded_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.contains('\0')
}

fn bounded_token(value: &str, max: usize) -> bool {
    bounded_text(value, max)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn valid_planning_selector(selector: &PlanningSelector) -> bool {
    bounded_text(&selector.value, 1_024)
        && (selector.kind != SelectorKind::Path
            || ProjectPathRef::parse(selector.value.clone()).is_ok())
}

fn valid_planning_document_ref(reference: &DocumentRef, schema_id: &str) -> bool {
    reference.schema_id == schema_id
        && bounded_token(&reference.document_id, 192)
        && reference.revision > 0
        && reference.sha256 != Sha256Hash::digest(b"")
}

fn valid_excluded_scope(excluded: &ExcludedScope) -> bool {
    valid_planning_selector(&excluded.selector) && bounded_text(&excluded.reason, 2_048)
}

fn valid_scope_set(scope: &ScopeSet) -> bool {
    !scope.project_ids.is_empty()
        && scope.project_ids.len() <= 64
        && sorted_unique(&scope.project_ids)
        && !scope.selectors.is_empty()
        && scope.selectors.len() <= 512
        && scope.exclusions.len() <= 512
        && scope.selectors.iter().all(|item| {
            valid_planning_selector(&item.selector)
                && bounded_token(&item.reason_code, 128)
                && item.evidence_refs.len() <= 128
                && sorted_unique(&item.evidence_refs)
                && item
                    .evidence_refs
                    .iter()
                    .all(|reference| bounded_text(reference, 1_024))
        })
        && scope.selectors.windows(2).all(|pair| {
            (&pair[0].selector, pair[0].source, &pair[0].reason_code)
                < (&pair[1].selector, pair[1].source, &pair[1].reason_code)
        })
        && scope.exclusions.iter().all(valid_excluded_scope)
        && scope.exclusions.windows(2).all(|pair| {
            (&pair[0].selector, pair[0].applies_to) < (&pair[1].selector, pair[1].applies_to)
        })
}

fn normalize_scope_set(scope: &mut ScopeSet) -> Result<(), PlanningContractError> {
    scope.project_ids.sort();
    scope.project_ids.dedup();
    for selector in &mut scope.selectors {
        selector.evidence_refs.sort();
        selector.evidence_refs.dedup();
    }
    scope.selectors.sort_by(|left, right| {
        (&left.selector, left.source, &left.reason_code).cmp(&(
            &right.selector,
            right.source,
            &right.reason_code,
        ))
    });
    scope.exclusions.sort_by(|left, right| {
        (&left.selector, left.applies_to, &left.reason).cmp(&(
            &right.selector,
            right.applies_to,
            &right.reason,
        ))
    });
    if scope.exclusions.windows(2).any(|pair| {
        pair[0].selector == pair[1].selector && pair[0].applies_to == pair[1].applies_to
    }) || !valid_scope_set(scope)
    {
        return Err(PlanningContractError::Ordering);
    }
    Ok(())
}

fn normalize_nonempty_strings(values: &mut Vec<String>) -> Result<(), PlanningContractError> {
    if values.iter().any(|value| value.trim().is_empty()) {
        return Err(PlanningContractError::Empty);
    }
    values.sort();
    values.dedup();
    Ok(())
}

fn document_hash<T: Serialize>(value: &T) -> Result<Sha256Hash, PlanningContractError> {
    let value = serde_json::to_value(value).map_err(|_| PlanningContractError::Fingerprint)?;
    canonical_sha256(&value).map_err(|_| PlanningContractError::Fingerprint)
}

fn acyclic_change_graph(
    unit_ids: &std::collections::BTreeSet<String>,
    edges: &[ChangeGraphEdgeV2],
) -> bool {
    let mut indegree = unit_ids
        .iter()
        .cloned()
        .map(|unit| (unit, 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = unit_ids
        .iter()
        .cloned()
        .map(|unit| (unit, Vec::<String>::new()))
        .collect::<BTreeMap<_, _>>();
    for edge in edges
        .iter()
        .filter(|edge| edge.relation != ChangeGraphRelationV2::SameAtomicGroup)
    {
        let Some(degree) = indegree.get_mut(&edge.to_unit_id) else {
            return false;
        };
        *degree += 1;
        let Some(next) = outgoing.get_mut(&edge.from_unit_id) else {
            return false;
        };
        next.push(edge.to_unit_id.clone());
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(unit, _)| unit.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut visited = 0_usize;
    while let Some(unit) = ready.pop_first() {
        visited += 1;
        for next in outgoing.get(&unit).into_iter().flatten() {
            let Some(degree) = indegree.get_mut(next) else {
                return false;
            };
            *degree -= 1;
            if *degree == 0 {
                ready.insert(next.clone());
            }
        }
    }
    visited == unit_ids.len()
}

impl TaskSpec {
    pub fn seal(mut self) -> Result<Self, PlanningContractError> {
        self.content_fingerprint = fingerprint(
            "star.task-spec",
            1,
            &serde_json::json!({
                "task_spec_id":self.task_spec_id,
                "revision":self.revision,
                "title":self.title,
                "objective":self.objective,
                "project_targets":self.project_targets,
                "included_scope":self.included_scope,
                "excluded_scope":self.excluded_scope,
                "intended_changes":self.intended_changes,
                "success_criteria":self.success_criteria,
                "constraints":self.constraints,
                "forbidden_actions":self.forbidden_actions,
                "profile_ids":self.profile_ids,
                "baseline_policy":self.baseline_policy,
                "requested_checks":self.requested_checks,
                "check_overrides":self.check_overrides,
                "assumptions":self.assumptions,
                "created_by":self.created_by,
            }),
        )?;
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), PlanningContractError> {
        if self.schema_id != TASK_SPEC_SCHEMA_ID || self.schema_version != 1 || self.revision == 0 {
            return Err(PlanningContractError::Schema);
        }
        if !bounded_text(&self.title, 256)
            || !bounded_text(&self.objective, 4_096)
            || self.project_targets.is_empty()
            || self.project_targets.len() > 64
            || !self
                .project_targets
                .iter()
                .any(|target| target.role == ProjectTargetRole::PlannedChange)
            || self.included_scope.is_empty()
            || self.included_scope.len() > 512
            || self.excluded_scope.len() > 512
            || self.intended_changes.is_empty()
            || self.intended_changes.len() > 512
            || self.success_criteria.is_empty()
            || self.success_criteria.len() > 256
            || !self
                .success_criteria
                .iter()
                .any(|criterion| criterion.required)
            || self.constraints.len() > 256
            || self.forbidden_actions.len() > 256
            || self.profile_ids.len() > 16
            || self.requested_checks.len() > 256
            || self.check_overrides.len() > 256
            || self.assumptions.len() > 256
        {
            return Err(PlanningContractError::Empty);
        }
        if !self.project_targets.iter().all(|target| {
            bounded_token(target.project_id.as_str(), 192)
                && bounded_token(target.checkout_id.as_str(), 192)
                && bounded_text(&target.reason, 2_048)
        }) || !self.project_targets.windows(2).all(|pair| {
            (&pair[0].project_id, &pair[0].checkout_id, pair[0].role)
                < (&pair[1].project_id, &pair[1].checkout_id, pair[1].role)
        }) || self.project_targets.windows(2).any(|pair| {
            pair[0].project_id == pair[1].project_id && pair[0].checkout_id == pair[1].checkout_id
        }) || !self.included_scope.iter().all(valid_planning_selector)
            || !sorted_unique(&self.included_scope)
            || !self.excluded_scope.iter().all(valid_excluded_scope)
            || !self.excluded_scope.windows(2).all(|pair| {
                (&pair[0].selector, pair[0].applies_to) < (&pair[1].selector, pair[1].applies_to)
            })
            || self
                .excluded_scope
                .iter()
                .any(|excluded| self.included_scope.contains(&excluded.selector))
            || !self.intended_changes.iter().all(|change| {
                bounded_token(&change.change_id, 128)
                    && valid_planning_selector(&change.selector)
                    && bounded_text(&change.intended_postcondition, 4_096)
            })
            || !self
                .intended_changes
                .windows(2)
                .all(|pair| pair[0].change_id < pair[1].change_id)
            || !self.success_criteria.iter().all(|criterion| {
                bounded_token(&criterion.criterion_id, 128)
                    && bounded_text(&criterion.description, 4_096)
                    && bounded_text(&criterion.verification, 4_096)
            })
            || !self
                .success_criteria
                .windows(2)
                .all(|pair| pair[0].criterion_id < pair[1].criterion_id)
            || !sorted_unique(&self.constraints)
            || !sorted_unique(&self.forbidden_actions)
            || !sorted_unique(&self.requested_checks)
            || !sorted_unique(&self.profile_ids)
            || !sorted_unique(&self.assumptions)
            || !non_empty(&self.constraints)
            || !non_empty(&self.forbidden_actions)
            || !non_empty(&self.assumptions)
            || self
                .constraints
                .iter()
                .any(|value| !bounded_text(value, 4_096))
            || self
                .forbidden_actions
                .iter()
                .any(|value| !bounded_text(value, 4_096))
            || self
                .requested_checks
                .iter()
                .any(|value| !bounded_token(value, 128))
            || self
                .profile_ids
                .iter()
                .any(|value| !bounded_token(value, 96))
            || self
                .assumptions
                .iter()
                .any(|value| !bounded_text(value, 4_096))
            || !self.check_overrides.iter().all(|override_item| {
                bounded_token(&override_item.family, 128)
                    && bounded_text(&override_item.reason, 2_048)
            })
            || !self.check_overrides.windows(2).all(|pair| {
                (&pair[0].family, pair[0].kind, &pair[0].reason)
                    < (&pair[1].family, pair[1].kind, &pair[1].reason)
                    && pair[0].family != pair[1].family
            })
            || match self.baseline_policy.kind {
                BaselinePolicyKind::CurrentWorkspace => self.baseline_policy.reference.is_some(),
                BaselinePolicyKind::ExplicitRevision | BaselinePolicyKind::PreviousSuccess => self
                    .baseline_policy
                    .reference
                    .as_deref()
                    .is_none_or(|reference| !bounded_text(reference, 1_024)),
            }
            || !bounded_token(&self.created_by.actor_id, 192)
            || !bounded_text(&self.created_by.display_name, 256)
            || !bounded_token(&self.created_by.auth_source, 192)
        {
            return Err(PlanningContractError::Ordering);
        }
        Ok(())
    }
}

impl ScopeRevision {
    pub fn seal(mut self) -> Result<Self, PlanningContractError> {
        normalize_scope_set(&mut self.requested_scope)?;
        normalize_scope_set(&mut self.analysis_scope)?;
        normalize_scope_set(&mut self.planned_change_scope)?;
        normalize_scope_set(&mut self.validation_scope)?;
        self.source_snapshot_refs.sort_by(|left, right| {
            (&left.project_id, &left.checkout_id).cmp(&(&right.project_id, &right.checkout_id))
        });
        self.derived_additions.sort_by(|left, right| {
            (left.axis, &left.selector, left.source, &left.reason_code).cmp(&(
                right.axis,
                &right.selector,
                right.source,
                &right.reason_code,
            ))
        });
        self.user_decisions
            .sort_by(|left, right| left.decision_id.cmp(&right.decision_id));
        normalize_nonempty_strings(&mut self.changed_fields)?;
        self.scope_hash = fingerprint(
            "star.scope-revision",
            1,
            &serde_json::json!({
                "scope_revision_id":self.scope_revision_id,
                "revision":self.revision,
                "task_spec_ref":self.task_spec_ref,
                "previous_scope_revision_ref":self.previous_scope_revision_ref,
                "reason_code":self.reason_code,
                "reason":self.reason,
                "requested_scope":self.requested_scope,
                "analysis_scope":self.analysis_scope,
                "planned_change_scope":self.planned_change_scope,
                "validation_scope":self.validation_scope,
                "source_snapshot_refs":self.source_snapshot_refs,
                "derived_additions":self.derived_additions,
                "user_decisions":self.user_decisions,
                "changed_fields":self.changed_fields,
                "approval_state":self.approval_state,
                "created_by":self.created_by,
            }),
        )?;
        if self.schema_id != SCOPE_REVISION_SCHEMA_ID
            || self.schema_version != 1
            || self.revision == 0
            || !bounded_text(&self.reason, 4_096)
            || !valid_planning_document_ref(&self.task_spec_ref, TASK_SPEC_SCHEMA_ID)
            || self.task_spec_ref.revision != self.revision
            || self.source_snapshot_refs.is_empty()
            || self.source_snapshot_refs.len() > 64
            || self.approval_state != ScopeApprovalState::Accepted
        {
            return Err(PlanningContractError::Schema);
        }
        let initial = self.reason_code == ScopeReasonCode::Initial;
        if initial != (self.revision == 1 && self.previous_scope_revision_ref.is_none()) {
            return Err(PlanningContractError::Identity);
        }
        if let Some(previous) = &self.previous_scope_revision_ref
            && (!valid_planning_document_ref(previous, SCOPE_REVISION_SCHEMA_ID)
                || previous.document_id != self.scope_revision_id.as_str()
                || previous.revision.checked_add(1) != Some(self.revision))
        {
            return Err(PlanningContractError::Identity);
        }
        if self.source_snapshot_refs.windows(2).any(|pair| {
            pair[0].project_id == pair[1].project_id && pair[0].checkout_id == pair[1].checkout_id
        }) || self.source_snapshot_refs.iter().any(|source| {
            !bounded_token(source.project_id.as_str(), 192)
                || !bounded_token(source.checkout_id.as_str(), 192)
                || source.freshness != IndexFreshnessState::Current
        }) {
            return Err(PlanningContractError::Ordering);
        }
        let expected_projects = self
            .source_snapshot_refs
            .iter()
            .map(|source| source.project_id.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if [
            &self.requested_scope,
            &self.analysis_scope,
            &self.planned_change_scope,
            &self.validation_scope,
        ]
        .iter()
        .any(|scope| scope.project_ids != expected_projects)
        {
            return Err(PlanningContractError::Identity);
        }
        if self.derived_additions.len() > 512
            || self.derived_additions.iter().any(|addition| {
                !valid_planning_selector(&addition.selector)
                    || !bounded_token(&addition.reason_code, 128)
                    || addition.evidence_refs.len() > 128
                    || !sorted_unique(&addition.evidence_refs)
                    || addition
                        .evidence_refs
                        .iter()
                        .any(|reference| !bounded_text(reference, 1_024))
            })
            || self
                .derived_additions
                .windows(2)
                .any(|pair| pair[0].axis == pair[1].axis && pair[0].selector == pair[1].selector)
            || self.user_decisions.len() > 512
            || self.user_decisions.iter().any(|decision| {
                !bounded_token(&decision.decision_id, 128)
                    || !valid_planning_selector(&decision.selector)
                    || !bounded_text(&decision.reason, 2_048)
                    || !bounded_token(&decision.actor.actor_id, 192)
                    || !bounded_text(&decision.actor.display_name, 256)
                    || !bounded_token(&decision.actor.auth_source, 192)
            })
            || self
                .user_decisions
                .windows(2)
                .any(|pair| pair[0].decision_id >= pair[1].decision_id)
            || self.changed_fields.len() > 128
            || !bounded_token(&self.created_by.actor_id, 192)
            || !bounded_text(&self.created_by.display_name, 256)
            || !bounded_token(&self.created_by.auth_source, 192)
        {
            return Err(PlanningContractError::Ordering);
        }
        Ok(self)
    }
}

impl ChangeSet {
    pub fn seal(mut self) -> Result<Self, PlanningContractError> {
        self.entries
            .sort_by(|left, right| left.path.cmp(&right.path));
        self.change_set_fingerprint = fingerprint(
            "star.change-set",
            1,
            &serde_json::json!({
                "change_set_id":self.change_set_id,
                "task_spec_ref":self.task_spec_ref,
                "scope_revision_ref":self.scope_revision_ref,
                "project_id":self.project_id,
                "checkout_id":self.checkout_id,
                "change_set_kind":self.change_set_kind,
                "base_revision_id":self.base_revision_id,
                "observed_workspace_snapshot_id":self.observed_workspace_snapshot_id,
                "comparison_scope":self.comparison_scope,
                "entries":self.entries,
                "collection_limits":self.collection_limits,
                "collection_state":self.collection_state,
            }),
        )?;
        if self.schema_id != CHANGE_SET_SCHEMA_ID || self.schema_version != 1 {
            return Err(PlanningContractError::Schema);
        }
        Ok(self)
    }
}

impl ImpactAnalysis {
    pub fn seal(mut self) -> Result<Self, PlanningContractError> {
        self.seeds
            .sort_by(|left, right| left.seed_id.cmp(&right.seed_id));
        self.impacted_nodes.sort_by(|left, right| {
            (&left.project_id, &left.entity_key).cmp(&(&right.project_id, &right.entity_key))
        });
        self.impact_edges
            .sort_by(|left, right| left.edge_id.cmp(&right.edge_id));
        self.risk_paths
            .sort_by(|left, right| left.finding_id.cmp(&right.finding_id));
        self.advisory_evidence_refs.sort();
        self.advisory_evidence_refs.dedup();
        self.calculation_fingerprint = fingerprint(
            "star.impact-analysis",
            1,
            &serde_json::json!({
                "impact_analysis_id":self.impact_analysis_id,
                "revision":self.revision,
                "task_spec_ref":self.task_spec_ref,
                "scope_revision_ref":self.scope_revision_ref,
                "project_inputs":self.project_inputs,
                "change_set_refs":self.change_set_refs,
                "catalog_snapshot_ref":self.catalog_snapshot_ref,
                "effective_config_fingerprint":self.effective_config_fingerprint,
                "seeds":self.seeds,
                "impacted_nodes":self.impacted_nodes,
                "impact_edges":self.impact_edges,
                "risk_paths":self.risk_paths,
                "affected_projects":self.affected_projects,
                "no_results":self.no_results,
                "advisory_evidence_refs":self.advisory_evidence_refs,
                "limitations":self.limitations,
                "confidence_summary":self.confidence_summary,
                "status":self.status,
            }),
        )?;
        if self.schema_id != IMPACT_ANALYSIS_SCHEMA_ID
            || self.schema_version != 1
            || self.revision == 0
            || self.project_inputs.is_empty()
            || self.change_set_refs.is_empty()
            || self.seeds.is_empty()
        {
            return Err(PlanningContractError::Empty);
        }
        Ok(self)
    }
}

impl RiskPathDescriptor {
    pub fn seal(mut self) -> Result<Self, PlanningContractError> {
        self.selector_kinds.sort();
        self.selector_kinds.dedup();
        self.source_classes.sort();
        self.source_classes.dedup();
        self.entity_kinds.sort();
        self.entity_kinds.dedup();
        self.required_check_families.sort();
        self.required_check_families.dedup();
        self.content_fingerprint = fingerprint(
            "star.risk-path-descriptor",
            1,
            &serde_json::json!({
                "risk_id":self.risk_id,
                "version":self.version,
                "selector_kinds":self.selector_kinds,
                "source_classes":self.source_classes,
                "entity_kinds":self.entity_kinds,
                "required_check_families":self.required_check_families,
                "severity_floor":self.severity_floor,
                "fallback_floor":self.fallback_floor,
            }),
        )?;
        if self.schema_id != RISK_PATH_DESCRIPTOR_SCHEMA_ID
            || self.schema_version != 1
            || self.risk_id.trim().is_empty()
            || self.version.trim().is_empty()
            || self.required_check_families.is_empty()
        {
            return Err(PlanningContractError::Empty);
        }
        Ok(self)
    }
}

impl FullValidationPlan {
    pub fn seal(mut self) -> Result<Self, PlanningContractError> {
        self.candidate_checks.sort_by(|left, right| {
            (&left.family, &left.check_id).cmp(&(&right.family, &right.check_id))
        });
        self.required_checks
            .sort_by(|left, right| left.plan_item_id.cmp(&right.plan_item_id));
        self.optional_checks
            .sort_by(|left, right| left.plan_item_id.cmp(&right.plan_item_id));
        self.check_graph.nodes.sort();
        self.check_graph.nodes.dedup();
        self.selection_fingerprint = fingerprint(
            "star.validation-plan",
            2,
            &serde_json::json!({
                "validation_plan_id":self.validation_plan_id,
                "revision":self.revision,
                "task_spec_ref":self.task_spec_ref,
                "scope_revision":self.scope_revision,
                "scope_revision_ref":self.scope_revision_ref,
                "phase":self.phase,
                "change_set_refs":self.change_set_refs,
                "impact_analysis_ref":self.impact_analysis_ref,
                "risk_level":self.risk_level,
                "affected_scope":self.affected_scope,
                "candidate_checks":self.candidate_checks,
                "required_checks":self.required_checks,
                "optional_checks":self.optional_checks,
                "check_graph":self.check_graph,
                "omitted_checks":self.omitted_checks,
                "unresolved_checks":self.unresolved_checks,
                "previous_success_comparisons":self.previous_success_comparisons,
                "fallback_decisions":self.fallback_decisions,
                "manual_observations":self.manual_observations,
                "independent_review":self.independent_review,
                "gate_policy":self.gate_policy,
                "config_fingerprint":self.config_fingerprint,
                "catalog_snapshot_ref":self.catalog_snapshot_ref,
                "profile_resolution":self.profile_resolution,
                "readiness":self.readiness,
            }),
        )?;
        if self.schema_id != FULL_VALIDATION_PLAN_SCHEMA_ID
            || self.schema_version != 2
            || self.revision == 0
            || self.scope_revision != self.scope_revision_ref.revision
            || self
                .profile_resolution
                .as_ref()
                .is_some_and(|resolution| resolution.validate().is_err())
            || (self.readiness == ValidationPlanV2Readiness::Ready
                && self.required_checks.is_empty())
        {
            return Err(PlanningContractError::Identity);
        }
        if self.readiness == ValidationPlanV2Readiness::Ready
            && (!self.unresolved_checks.is_empty()
                || self
                    .required_checks
                    .iter()
                    .any(|check| check.outcome != CheckResolutionOutcome::SelectedRequired))
        {
            return Err(PlanningContractError::Readiness);
        }
        Ok(self)
    }
}

impl PlanningBundle {
    pub fn migrate_v1_to_v2(mut self) -> Result<Self, PlanningContractError> {
        if self.schema_version == 2 {
            return self.seal();
        }
        if self.schema_id != PLANNING_BUNDLE_V2_SCHEMA_ID
            || self.schema_version != 1
            || !self.change_plans.is_empty()
        {
            return Err(PlanningContractError::Schema);
        }
        let task_ref = document_ref(
            TASK_SPEC_SCHEMA_ID,
            self.task_spec.task_spec_id.as_str(),
            self.task_spec.revision,
            &self.task_spec.content_fingerprint,
        );
        let scope_ref = document_ref(
            SCOPE_REVISION_SCHEMA_ID,
            self.scope_revision.scope_revision_id.as_str(),
            self.scope_revision.revision,
            &self.scope_revision.scope_hash,
        );
        let impact_ref = document_ref(
            IMPACT_ANALYSIS_SCHEMA_ID,
            self.impact_analysis.impact_analysis_id.as_str(),
            self.impact_analysis.revision,
            &self.impact_analysis.calculation_fingerprint,
        );
        let validation_ref = document_ref(
            FULL_VALIDATION_PLAN_SCHEMA_ID,
            self.validation_plan.validation_plan_id.as_str(),
            self.validation_plan.revision,
            &self.validation_plan.selection_fingerprint,
        );
        let unit_ids = self
            .task_spec
            .intended_changes
            .iter()
            .map(|change| change.change_id.clone())
            .collect::<Vec<_>>();
        let check_ids = self
            .validation_plan
            .required_checks
            .iter()
            .map(|check| check.plan_item_id.clone())
            .collect::<Vec<_>>();
        for change_set in &self.change_sets {
            let expected_paths = change_set
                .entries
                .iter()
                .map(|entry| entry.path.clone())
                .collect::<Vec<_>>();
            let target_edges = self
                .impact_analysis
                .impact_edges
                .iter()
                .filter(|edge| edge.project_id == change_set.project_id)
                .map(|edge| edge.edge_id.clone())
                .collect::<Vec<_>>();
            let target_risks = self
                .impact_analysis
                .risk_paths
                .iter()
                .filter(|risk| risk.project_id == change_set.project_id)
                .map(|risk| format!("{}@{}", risk.risk_id, risk.risk_version))
                .collect::<Vec<_>>();
            let preconditions = vec![
                self.task_spec.content_fingerprint.clone(),
                self.scope_revision.scope_hash.clone(),
                change_set.change_set_fingerprint.clone(),
                self.impact_analysis.calculation_fingerprint.clone(),
                self.validation_plan.selection_fingerprint.clone(),
            ];
            self.change_plans.push(
                ChangePlanV2 {
                    schema_id: CHANGE_PLAN_V2_SCHEMA_ID.to_owned(),
                    schema_version: 2,
                    change_plan_id: ChangePlanId::from_stable_bytes(
                        format!(
                            "planning-bundle-v1:{}:{}",
                            self.task_spec.task_spec_id, change_set.project_id
                        )
                        .as_bytes(),
                    ),
                    revision: 1,
                    task_spec_ref: task_ref.clone(),
                    scope_revision_ref: scope_ref.clone(),
                    impact_analysis_ref: impact_ref.clone(),
                    change_origin: ChangePlanOriginV2::UserPlanned,
                    project_id: change_set.project_id.clone(),
                    target_checkout_id: change_set.checkout_id.clone(),
                    target_project_revision_id: change_set.base_revision_id.clone(),
                    target_workspace_snapshot_id: change_set.observed_workspace_snapshot_id.clone(),
                    change_set_ref: document_ref(
                        CHANGE_SET_SCHEMA_ID,
                        change_set.change_set_id.as_str(),
                        1,
                        &change_set.change_set_fingerprint,
                    ),
                    related_project_impacts: self
                        .impact_analysis
                        .affected_projects
                        .iter()
                        .filter(|project| project.project_id != change_set.project_id)
                        .map(|project| RelatedProjectImpactV2 {
                            project_id: project.project_id.clone(),
                            impact_analysis_ref: impact_ref.clone(),
                        })
                        .collect(),
                    planned_change_units: self
                        .task_spec
                        .intended_changes
                        .iter()
                        .map(|change| PlannedChangeUnitV2 {
                            unit_id: change.change_id.clone(),
                            target_selector: change.selector.clone(),
                            change_kind: change.change_kind,
                            intended_postcondition: change.intended_postcondition.clone(),
                            source: ChangeUnitSourceV2::User,
                            reason: "migrated_planning_bundle_v1".to_owned(),
                            expected_paths: Vec::new(),
                            unresolved_target: Some(
                                "legacy_plan_requires_current_target_recheck".to_owned(),
                            ),
                            precondition_fingerprints: preconditions.clone(),
                            permission_requirements: vec!["source.write".to_owned()],
                            risk_path_refs: target_risks.clone(),
                            impact_edge_refs: target_edges.clone(),
                            completion_criterion_refs: self
                                .task_spec
                                .success_criteria
                                .iter()
                                .map(|criterion| criterion.criterion_id.clone())
                                .collect(),
                        })
                        .collect(),
                    change_graph: Vec::new(),
                    deterministic_unit_order: unit_ids.clone(),
                    expected_impact_refs: unit_ids
                        .iter()
                        .map(|unit_id| ExpectedImpactRefV2 {
                            unit_id: unit_id.clone(),
                            accepted_impact_edge_ids: target_edges.clone(),
                            unresolved_frontier_refs: vec![
                                "LEGACY_PLANNING_BUNDLE_REQUIRES_REPLAN".to_owned(),
                            ],
                        })
                        .collect(),
                    completion_criteria_mapping: self
                        .task_spec
                        .success_criteria
                        .iter()
                        .map(|criterion| CompletionCriterionMappingV2 {
                            criterion_id: criterion.criterion_id.clone(),
                            unit_ids: unit_ids.clone(),
                            check_plan_item_ids: check_ids.clone(),
                            manual_observation_refs: Vec::new(),
                            explicit_user_decision_omission: None,
                        })
                        .collect(),
                    expected_paths,
                    finding_refs: Vec::new(),
                    recipe_refs: Vec::new(),
                    parameters: BTreeMap::new(),
                    risk_path_refs: target_risks,
                    preconditions,
                    unresolved_impacts: vec!["LEGACY_PLANNING_BUNDLE_REQUIRES_REPLAN".to_owned()],
                    permission_requirements: vec!["source.write".to_owned()],
                    permission_plan_ref: None,
                    validation_plan_ref: validation_ref.clone(),
                    readiness: ChangePlanReadinessV2::Blocked,
                    status: ChangePlanStatusV2::Blocked,
                    created_at: self.task_spec.created_at,
                    updated_at: Utc::now(),
                    content_fingerprint: empty_fingerprint(),
                }
                .seal()?,
            );
        }
        self.schema_version = 2;
        self.seal()
    }

    pub fn seal(mut self) -> Result<Self, PlanningContractError> {
        let task_ref = document_ref(
            TASK_SPEC_SCHEMA_ID,
            self.task_spec.task_spec_id.as_str(),
            self.task_spec.revision,
            &self.task_spec.content_fingerprint,
        );
        let scope_ref = document_ref(
            SCOPE_REVISION_SCHEMA_ID,
            self.scope_revision.scope_revision_id.as_str(),
            self.scope_revision.revision,
            &self.scope_revision.scope_hash,
        );
        let change_set_refs = self
            .change_sets
            .iter()
            .map(|change_set| {
                document_ref(
                    CHANGE_SET_SCHEMA_ID,
                    change_set.change_set_id.as_str(),
                    1,
                    &change_set.change_set_fingerprint,
                )
            })
            .collect::<Vec<_>>();
        let impact_ref = document_ref(
            IMPACT_ANALYSIS_SCHEMA_ID,
            self.impact_analysis.impact_analysis_id.as_str(),
            self.impact_analysis.revision,
            &self.impact_analysis.calculation_fingerprint,
        );
        let validation_plan_ref = document_ref(
            FULL_VALIDATION_PLAN_SCHEMA_ID,
            self.validation_plan.validation_plan_id.as_str(),
            self.validation_plan.revision,
            &self.validation_plan.selection_fingerprint,
        );
        self.change_plans = self
            .change_plans
            .into_iter()
            .map(ChangePlanV2::seal)
            .collect::<Result<Vec<_>, _>>()?;
        self.change_plans
            .sort_by(|left, right| left.project_id.cmp(&right.project_id));
        let change_plan_identity_invalid = self.change_plans.len() != self.change_sets.len()
            || self.change_plans.iter().any(|plan| {
                let Some(change_set) = self
                    .change_sets
                    .iter()
                    .find(|change_set| change_set.project_id == plan.project_id)
                else {
                    return true;
                };
                plan.task_spec_ref != task_ref
                    || plan.scope_revision_ref != scope_ref
                    || plan.impact_analysis_ref != impact_ref
                    || plan.validation_plan_ref != validation_plan_ref
                    || plan.change_set_ref
                        != document_ref(
                            CHANGE_SET_SCHEMA_ID,
                            change_set.change_set_id.as_str(),
                            1,
                            &change_set.change_set_fingerprint,
                        )
                    || plan.target_checkout_id != change_set.checkout_id
                    || plan.target_project_revision_id != change_set.base_revision_id
                    || plan.target_workspace_snapshot_id
                        != change_set.observed_workspace_snapshot_id
            });
        if self.scope_revision.task_spec_ref != task_ref
            || self.impact_analysis.task_spec_ref != task_ref
            || self.impact_analysis.scope_revision_ref != scope_ref
            || self.impact_analysis.change_set_refs != change_set_refs
            || self.validation_plan.task_spec_ref != task_ref
            || self.validation_plan.scope_revision_ref != scope_ref
            || self.validation_plan.change_set_refs != change_set_refs
            || self.validation_plan.impact_analysis_ref != impact_ref
            || self
                .validation_plan
                .profile_resolution
                .as_ref()
                .map(|resolution| {
                    resolution
                        .selected_profiles
                        .iter()
                        .map(|profile| profile.profile_id.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
                != self.task_spec.profile_ids
            || (self.impact_analysis.status == ImpactStatus::Invalidated)
                != (self.validation_plan.readiness == ValidationPlanV2Readiness::Invalidated)
            || self.change_sets.iter().any(|change_set| {
                change_set.task_spec_ref != task_ref || change_set.scope_revision_ref != scope_ref
            })
            || change_plan_identity_invalid
        {
            return Err(PlanningContractError::Identity);
        }
        self.bundle_fingerprint = fingerprint(
            "star.planning-bundle",
            2,
            &serde_json::json!({
                "task_spec":self.task_spec,
                "scope_revision":self.scope_revision,
                "change_sets":self.change_sets,
                "impact_analysis":self.impact_analysis,
                "change_plans":self.change_plans,
                "validation_plan":self.validation_plan,
            }),
        )?;
        if self.schema_id != PLANNING_BUNDLE_V2_SCHEMA_ID || self.schema_version != 2 {
            return Err(PlanningContractError::Schema);
        }
        Ok(self)
    }
}

pub fn empty_fingerprint() -> Sha256Hash {
    Sha256Hash::digest(b"")
}

pub fn limitation_parameters() -> BTreeMap<String, String> {
    BTreeMap::new()
}
