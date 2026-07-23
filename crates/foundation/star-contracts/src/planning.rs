//! Persisted M2 task, scope, impact, and full validation-plan contracts.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    Sha256Hash, canonical_sha256,
    evidence::{ActorRef, DocumentRef},
    ids::{
        ChangeSetId, CheckoutId, CodeIndexSnapshotId, ImpactAnalysisId, ProjectCatalogSnapshotId,
        ProjectId, ProjectRevisionId, ScopeRevisionId, TaskSpecId, ValidationPlanId,
        WorkspaceSnapshotId,
    },
    index::{IndexFreshnessState, IndexTier, SourceClass},
    management::ProjectPathRef,
    profile::DevelopmentProfileResolutionV1,
};

pub const TASK_SPEC_SCHEMA_ID: &str = "star.task-spec";
pub const SCOPE_REVISION_SCHEMA_ID: &str = "star.scope-revision";
pub const CHANGE_SET_SCHEMA_ID: &str = "star.change-set";
pub const IMPACT_ANALYSIS_SCHEMA_ID: &str = "star.impact-analysis";
pub const RISK_PATH_DESCRIPTOR_SCHEMA_ID: &str = "star.risk-path-descriptor";
pub const FULL_VALIDATION_PLAN_SCHEMA_ID: &str = "star.validation-plan";

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
    pub required_evidence: Vec<String>,
    pub content_fingerprint: Sha256Hash,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningBundle {
    pub schema_id: String,
    pub schema_version: u32,
    pub task_spec: TaskSpec,
    pub scope_revision: ScopeRevision,
    pub change_sets: Vec<ChangeSet>,
    pub impact_analysis: ImpactAnalysis,
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
        if self.title.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.project_targets.is_empty()
            || !self
                .project_targets
                .iter()
                .any(|target| target.role == ProjectTargetRole::PlannedChange)
            || self.included_scope.is_empty()
            || self.intended_changes.is_empty()
            || !self
                .success_criteria
                .iter()
                .any(|criterion| criterion.required)
        {
            return Err(PlanningContractError::Empty);
        }
        if !sorted_unique(&self.included_scope)
            || !sorted_unique(&self.requested_checks)
            || !sorted_unique(&self.profile_ids)
            || !non_empty(&self.constraints)
            || !non_empty(&self.forbidden_actions)
            || !non_empty(&self.assumptions)
        {
            return Err(PlanningContractError::Ordering);
        }
        Ok(())
    }
}

impl ScopeRevision {
    pub fn seal(mut self) -> Result<Self, PlanningContractError> {
        self.scope_hash = fingerprint(
            "star.scope-revision",
            1,
            &serde_json::json!({
                "scope_revision_id":self.scope_revision_id,
                "revision":self.revision,
                "task_spec_ref":self.task_spec_ref,
                "previous_scope_revision_ref":self.previous_scope_revision_ref,
                "reason_code":self.reason_code,
                "requested_scope":self.requested_scope,
                "analysis_scope":self.analysis_scope,
                "planned_change_scope":self.planned_change_scope,
                "validation_scope":self.validation_scope,
                "source_snapshot_refs":self.source_snapshot_refs,
                "derived_additions":self.derived_additions,
                "user_decisions":self.user_decisions,
                "changed_fields":self.changed_fields,
                "approval_state":self.approval_state,
            }),
        )?;
        if self.schema_id != SCOPE_REVISION_SCHEMA_ID
            || self.schema_version != 1
            || self.revision == 0
            || self.reason.trim().is_empty()
            || self.source_snapshot_refs.is_empty()
            || self.approval_state != ScopeApprovalState::Accepted
        {
            return Err(PlanningContractError::Schema);
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
        {
            return Err(PlanningContractError::Identity);
        }
        self.bundle_fingerprint = fingerprint(
            "star.planning-bundle",
            1,
            &serde_json::json!({
                "task_spec":self.task_spec,
                "scope_revision":self.scope_revision,
                "change_sets":self.change_sets,
                "impact_analysis":self.impact_analysis,
                "validation_plan":self.validation_plan,
            }),
        )?;
        if self.schema_id != "star.planning-bundle" || self.schema_version != 1 {
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
