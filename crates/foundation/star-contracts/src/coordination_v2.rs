use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{CheckoutId, ProjectId, Sha256Hash, development_v2::CoverageState};

pub const MULTI_PROJECT_GOAL_SCHEMA_ID: &str = "star.multi-project-goal";
pub const CROSS_REPO_CHANGE_BUNDLE_SCHEMA_ID: &str = "star.cross-repo-change-bundle";
pub const CHANGE_BUNDLE_PARTICIPANT_V2_SCHEMA_ID: &str = "star.change-bundle-participant";
pub const WORKTREE_RECORD_SCHEMA_ID: &str = "star.worktree-record";
pub const OVERLAP_ANALYSIS_SCHEMA_ID: &str = "star.overlap-analysis";
pub const MERGE_PLAN_V2_SCHEMA_ID: &str = "star.merge-plan-v2";
pub const MERGE_QUEUE_RECORD_SCHEMA_ID: &str = "star.merge-queue-record";
pub const MERGE_CONFLICT_RECORD_SCHEMA_ID: &str = "star.merge-conflict-record";
pub const PROJECT_MERGE_RESULT_SCHEMA_ID: &str = "star.project-merge-result";
pub const REMOTE_STATE_SNAPSHOT_V2_SCHEMA_ID: &str = "star.remote-state-snapshot-v2";
pub const REMOTE_OPERATION_RECORD_SCHEMA_ID: &str = "star.remote-operation-record";
pub const CHANGE_BUNDLE_RELEASE_HANDOFF_SCHEMA_ID: &str = "star.change-bundle-release-handoff";

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantRole {
    Provider,
    Consumer,
    DataOwner,
    Tooling,
    ValidationOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GoalParticipant {
    pub project_id: ProjectId,
    pub required: bool,
    pub roles: Vec<ParticipantRole>,
    pub source_of_truth_refs: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjectRelationKind {
    Api,
    Schema,
    Format,
    Config,
    ErrorCode,
    Artifact,
    Dependency,
    Data,
    Tooling,
    Runtime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RelationCertainty {
    Confirmed,
    Possible,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectRelation {
    pub relation_id: String,
    pub provider_project_id: ProjectId,
    pub consumer_project_id: ProjectId,
    pub relation_kind: ProjectRelationKind,
    pub contract_refs: Vec<String>,
    pub accepted_versions: Vec<String>,
    pub minimum_provider_version: Option<String>,
    pub certainty: RelationCertainty,
    pub evidence_refs: Vec<String>,
    pub freshness: CoverageState,
    pub limitations: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BundleStepKind {
    ProviderCompatibilityOpen,
    ProjectPatchApply,
    ProjectMigration,
    ProjectValidate,
    ConsumerTransition,
    ProjectLocalIntegrate,
    ProviderCompatibilityClose,
    RemotePush,
    RemotePr,
    RemoteMerge,
    BundleGoalValidate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BundleEdgeKind {
    Requires,
    ProviderBeforeConsumer,
    SchemaBeforeCodegen,
    ReaderBeforeWriter,
    ConsumerBeforeProviderRemoval,
    ValidationBeforeIntegration,
    LocalBeforeRemote,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BundleStep {
    pub step_id: String,
    pub project_id: Option<ProjectId>,
    pub stage_ref: Option<String>,
    pub step_kind: BundleStepKind,
    pub input_refs: Vec<String>,
    pub output_refs: Vec<String>,
    pub expected_effect: String,
    pub required_gate_refs: Vec<String>,
    pub completion_condition: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BundleStepEdge {
    pub from_step_id: String,
    pub to_step_id: String,
    pub edge_kind: BundleEdgeKind,
    pub reason: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BundleStepGraph {
    pub steps: Vec<BundleStep>,
    pub edges: Vec<BundleStepEdge>,
    pub topological_order: Vec<String>,
    pub graph_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityWindowState {
    Planned,
    Open,
    ClosingReady,
    Closed,
    ExpiredUnresolved,
    Blocked,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityWindow {
    pub window_id: String,
    pub contract_ref: String,
    pub provider_project_id: ProjectId,
    pub required_consumer_project_ids: Vec<ProjectId>,
    pub open_step_ref: String,
    pub close_step_ref: String,
    pub old_accepted_version: String,
    pub new_accepted_version: String,
    pub opened_revision_ref: Option<String>,
    pub deadline: Option<String>,
    pub evidence_close_condition: Option<String>,
    pub current_consumer_state_refs: Vec<String>,
    pub rollback_trigger: String,
    pub state: CompatibilityWindowState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResourceBudget {
    pub max_parallel_projects: u32,
    pub max_active_worktrees: u32,
    pub max_concurrent_writes: u32,
    pub max_processes: u32,
    pub cpu_weight_limit: u32,
    pub memory_limit_bytes: u64,
    pub worktree_disk_limit_bytes: u64,
    pub artifact_limit_bytes: u64,
    pub wall_time_limit_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResourceBudgetSnapshot {
    pub budget_ref: String,
    pub observed: BTreeMap<String, u64>,
    pub reserved: BTreeMap<String, u64>,
    pub remaining: BTreeMap<String, u64>,
    pub unknown_dimensions: Vec<String>,
    pub captured_at: String,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CompletionLevel {
    None,
    ValidatedParticipants,
    LocalIntegrated,
    RemoteMerged,
    ReleaseHandoffReady,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MultiProjectGoal {
    pub schema_id: String,
    pub schema_version: u32,
    pub multi_project_goal_id: String,
    pub revision: u64,
    pub previous_revision_ref: Option<String>,
    pub goal_spec_ref: String,
    pub task_spec_refs: Vec<String>,
    pub scope_revision_refs: Vec<String>,
    pub participants: Vec<GoalParticipant>,
    pub project_relations: Vec<ProjectRelation>,
    pub step_graph: BundleStepGraph,
    pub compatibility_windows: Vec<CompatibilityWindow>,
    pub cross_project_invariants: Vec<String>,
    pub completion_target: CompletionLevel,
    pub resource_budget: ResourceBudget,
    pub permission_floor_ref: String,
    pub source_snapshot_refs: Vec<String>,
    pub unknowns: Vec<String>,
    pub questions: Vec<String>,
    pub goal_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RemotePolicy {
    Disabled,
    ObserveOnly,
    ApprovedActionsOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BundleAggregateState {
    Preparing,
    Prepared,
    AwaitingApply,
    Applying,
    PartiallyApplied,
    AwaitingValidation,
    Validating,
    RollbackRequired,
    Held,
    OutcomeUnknown,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CrossRepoChangeBundle {
    pub schema_id: String,
    pub schema_version: u32,
    pub change_bundle_id: String,
    pub revision: u64,
    pub previous_revision_ref: Option<String>,
    pub multi_project_goal_ref: String,
    pub task_spec_refs: Vec<String>,
    pub scope_revision_refs: Vec<String>,
    pub input_handoff_refs: Vec<String>,
    pub participant_refs: Vec<String>,
    pub step_graph: BundleStepGraph,
    pub compatibility_window_refs: Vec<String>,
    pub merge_policy: String,
    pub remote_policy: RemotePolicy,
    pub resource_budget: ResourceBudget,
    pub budget_snapshot_ref: String,
    pub permission_plan_ref: String,
    pub gate_policy_fingerprint: Sha256Hash,
    pub prepare_gate_ref: Option<String>,
    pub goal_gate_ref: Option<String>,
    pub state: BundleAggregateState,
    pub completion_target: CompletionLevel,
    pub completion_level_reached: CompletionLevel,
    pub open_effect_refs: Vec<String>,
    pub pending_approval_refs: Vec<String>,
    pub remaining_risks: Vec<String>,
    pub hold_reasons: Vec<String>,
    pub supersedes_bundle_ref: Option<String>,
    pub bundle_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DirtyState {
    Clean,
    DirtyComplete,
    DirtyPartial,
    Unverified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantState {
    Preparing,
    Prepared,
    AwaitingApply,
    Applying,
    PartiallyApplied,
    AwaitingValidation,
    Validating,
    MergeReady,
    Merging,
    LocalCompleted,
    RemotePending,
    RollbackRequired,
    Held,
    OutcomeUnknown,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeBundleParticipantV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub participant_id: String,
    pub revision: u64,
    pub previous_revision_ref: Option<String>,
    pub change_bundle_ref: String,
    pub project_id: ProjectId,
    pub required: bool,
    pub roles: Vec<ParticipantRole>,
    pub step_ids: Vec<String>,
    pub checkout_id: CheckoutId,
    pub repository_fingerprint: Sha256Hash,
    pub git_object_format: String,
    pub base_project_revision_ref: String,
    pub base_commit_oid: String,
    pub baseline_workspace_snapshot_ref: String,
    pub dirty_manifest_ref: String,
    pub dirty_state: DirtyState,
    pub preexisting_change_set_ref: String,
    pub change_plan_refs: Vec<String>,
    pub patch_set_refs: Vec<String>,
    pub migration_plan_refs: Vec<String>,
    pub worktree_record_refs: Vec<String>,
    pub merge_plan_ref: Option<String>,
    pub merge_queue_ref: Option<String>,
    pub validation_plan_refs: Vec<String>,
    pub gate_decision_refs: Vec<String>,
    pub evidence_bundle_refs: Vec<String>,
    pub project_merge_result_ref: Option<String>,
    pub remote_snapshot_refs: Vec<String>,
    pub remote_operation_refs: Vec<String>,
    pub recovery_plan_ref: String,
    pub compensation_refs: Vec<String>,
    pub state: ParticipantState,
    pub pending_action: Option<String>,
    pub actual_subject_binding_ref: Option<String>,
    pub participant_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeRole {
    ParticipantApply,
    ParticipantValidation,
    ProjectIntegration,
    ConflictResolution,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeState {
    Planned,
    Creating,
    Ready,
    Dirty,
    Validating,
    MergeReady,
    Retained,
    DiscardReady,
    Discarded,
    Orphaned,
    OwnershipUnknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorktreeRecord {
    pub schema_id: String,
    pub schema_version: u32,
    pub worktree_id: String,
    pub revision: u64,
    pub previous_revision_ref: Option<String>,
    pub project_id: ProjectId,
    pub participant_id: String,
    pub step_id: String,
    pub repository_fingerprint: Sha256Hash,
    pub base_commit_oid: String,
    pub root_binding_id: String,
    pub role: WorktreeRole,
    pub branch_ref: Option<String>,
    pub creation_receipt_ref: Option<String>,
    pub before_manifest_ref: String,
    pub current_manifest_ref: Option<String>,
    pub owner_token_fingerprint: Sha256Hash,
    pub state: WorktreeState,
    pub retention: String,
    pub evidence_hold: bool,
    pub last_probe_ref: Option<String>,
    pub record_fingerprint: Sha256Hash,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum OverlapAxis {
    File,
    Rename,
    Range,
    Symbol,
    Contract,
    Generated,
    Dependency,
    RepositoryPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OverlapDisposition {
    Disjoint,
    OrderedOverlap,
    ConflictPossible,
    ConflictConfirmed,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OverlapSubject {
    pub participant_ref: String,
    pub project_id: ProjectId,
    pub repository_fingerprint: Sha256Hash,
    pub file_refs: Vec<String>,
    pub rename_refs: Vec<String>,
    pub range_refs: Vec<String>,
    pub symbol_refs: Vec<String>,
    pub contract_refs: Vec<String>,
    pub generated_owner_refs: Vec<String>,
    pub dependency_refs: Vec<String>,
    pub repository_policy_refs: Vec<String>,
    pub coverage: CoverageState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OverlapItem {
    pub left_participant_ref: String,
    pub right_participant_ref: String,
    pub axis: OverlapAxis,
    pub subject_ref: String,
    pub disposition: OverlapDisposition,
    pub reason: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OverlapAnalysis {
    pub schema_id: String,
    pub schema_version: u32,
    pub overlap_analysis_id: String,
    pub revision: u64,
    pub change_bundle_ref: String,
    pub subjects: Vec<OverlapSubject>,
    pub items: Vec<OverlapItem>,
    pub overall: OverlapDisposition,
    pub parallel_safe: bool,
    pub merge_ready: bool,
    pub limitations: Vec<String>,
    pub analysis_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategyV2 {
    FastForwardOnly,
    MergeCommit,
    Squash,
    ApplyPatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MergePlanState {
    Draft,
    Ready,
    Queued,
    Stale,
    Integrating,
    Conflicted,
    Validating,
    Completed,
    Held,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MergePlanV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub merge_plan_id: String,
    pub revision: u64,
    pub previous_revision_ref: Option<String>,
    pub change_bundle_ref: String,
    pub participant_ref: String,
    pub project_id: ProjectId,
    pub repository_fingerprint: Sha256Hash,
    pub integration_worktree_ref: String,
    pub target_ref: String,
    pub target_base_commit_oid: String,
    pub inputs: Vec<String>,
    pub strategy: MergeStrategyV2,
    pub order: Vec<String>,
    pub dependency_refs: Vec<String>,
    pub overlap_analysis_ref: String,
    pub conflict_policy: String,
    pub validation_plan_ref: String,
    pub rollback_plan_ref: String,
    pub permission_plan_ref: String,
    pub status: MergePlanState,
    pub plan_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MergeQueueEntryState {
    Queued,
    BlockedDependency,
    Stale,
    Ready,
    Integrating,
    Conflicted,
    Validating,
    Completed,
    Held,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MergeQueueEntry {
    pub entry_id: String,
    pub merge_plan_ref: String,
    pub participant_ref: String,
    pub expected_predecessor_commit_oid: String,
    pub dependency_entry_refs: Vec<String>,
    pub state: MergeQueueEntryState,
    pub attempt_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MergeQueueRecord {
    pub schema_id: String,
    pub schema_version: u32,
    pub merge_queue_id: String,
    pub revision: u64,
    pub previous_revision_ref: Option<String>,
    pub project_id: ProjectId,
    pub repository_fingerprint: Sha256Hash,
    pub integration_target_ref: String,
    pub current_base_commit_oid: String,
    pub entries: Vec<MergeQueueEntry>,
    pub active_entry_ref: Option<String>,
    pub repository_lock_ref: Option<String>,
    pub resource_reservation_ref: Option<String>,
    pub queue_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolutionClass {
    MechanicalSafe,
    RequiresReplan,
    HumanReview,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MergeConflictState {
    Open,
    Proposed,
    ResolvedPendingValidation,
    Resolved,
    Blocked,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MergeConflictRecord {
    pub schema_id: String,
    pub schema_version: u32,
    pub conflict_id: String,
    pub revision: u64,
    pub project_id: ProjectId,
    pub merge_plan_ref: String,
    pub queue_entry_refs: Vec<String>,
    pub base_commit_oid: String,
    pub left_revision: String,
    pub right_revision: String,
    pub conflict_items: Vec<OverlapItem>,
    pub left_intent_refs: Vec<String>,
    pub right_intent_refs: Vec<String>,
    pub contract_refs: Vec<String>,
    pub raw_conflict_artifact_ref: String,
    pub resolution_class: ConflictResolutionClass,
    pub resolution_decision_ref: Option<String>,
    pub resolution_patch_set_ref: Option<String>,
    pub revalidation_refs: Vec<String>,
    pub state: MergeConflictState,
    pub conflict_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjectMergeResultState {
    ValidatedWorktree,
    IntegratedUncommitted,
    LocalCommit,
    LocalBranchUpdated,
    Conflicted,
    Failed,
    OutcomeUnknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectMergeResult {
    pub schema_id: String,
    pub schema_version: u32,
    pub project_merge_result_id: String,
    pub revision: u64,
    pub project_id: ProjectId,
    pub repository_fingerprint: Sha256Hash,
    pub merge_plan_ref: String,
    pub queue_entry_ref: String,
    pub integration_before_commit_oid: String,
    pub integration_after_commit_oid: Option<String>,
    pub working_tree_snapshot_ref: String,
    pub actual_strategy: MergeStrategyV2,
    pub commit_parent_oids: Vec<String>,
    pub adapter_receipt_ref: String,
    pub preexisting_change_preservation_ref: String,
    pub actual_change_set_ref: String,
    pub scope_deviation_refs: Vec<String>,
    pub validation_plan_ref: String,
    pub gate_decision_ref: String,
    pub evidence_bundle_ref: String,
    pub local_branch_updated: bool,
    pub branch_update_approval_ref: Option<String>,
    pub rollback_capabilities: Vec<String>,
    pub result: ProjectMergeResultState,
    pub result_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoteRefObservation {
    pub provider_ref: String,
    pub object_id: String,
    pub object_kind: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemotePullRequestObservation {
    pub provider_ref: String,
    pub head_object_id: String,
    pub base_ref: String,
    pub merge_object_id: Option<String>,
    pub state: String,
    pub updated_revision: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoteCheckObservation {
    pub check_id: String,
    pub subject_object_id: String,
    pub status: String,
    pub conclusion: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoteReleaseObservation {
    pub provider_ref: String,
    pub tag: String,
    pub source_object_id: String,
    pub artifact_refs: Vec<String>,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoteStateSnapshotV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub remote_snapshot_id: String,
    pub revision: u64,
    pub project_id: ProjectId,
    pub remote_kind: String,
    pub adapter_descriptor_ref: String,
    pub remote_identity: String,
    pub local_subject_ref: String,
    pub query_scope: Vec<String>,
    pub refs: Vec<RemoteRefObservation>,
    pub pull_requests: Vec<RemotePullRequestObservation>,
    pub checks: Vec<RemoteCheckObservation>,
    pub releases: Vec<RemoteReleaseObservation>,
    pub capabilities: BTreeMap<String, bool>,
    pub captured_at: String,
    pub valid_until: String,
    pub completeness: CoverageState,
    pub limitations: Vec<String>,
    pub raw_artifact_ref: Option<String>,
    pub snapshot_fingerprint: Sha256Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RemoteAction {
    Push,
    CreatePr,
    UpdatePr,
    MergePr,
    ClosePr,
    Publish,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RemoteOperationState {
    Planned,
    AwaitingApproval,
    Executing,
    Succeeded,
    Failed,
    OutcomeUnknown,
    Reconciled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoteOperationRecord {
    pub schema_id: String,
    pub schema_version: u32,
    pub remote_operation_id: String,
    pub revision: u64,
    pub project_id: ProjectId,
    pub change_bundle_ref: String,
    pub participant_ref: String,
    pub action: RemoteAction,
    pub before_snapshot_ref: String,
    pub local_source_revision: String,
    pub target: String,
    pub expected_remote_precondition: String,
    pub permission_plan_ref: String,
    pub approval_request_ref: Option<String>,
    pub idempotency_key: String,
    pub request_fingerprint: Sha256Hash,
    pub adapter_receipt_ref: Option<String>,
    pub after_snapshot_ref: Option<String>,
    pub state: RemoteOperationState,
    pub diagnostic_refs: Vec<String>,
    pub operation_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectReleaseInput {
    pub project_id: ProjectId,
    pub roles: Vec<ParticipantRole>,
    pub git_object_format: String,
    pub commit_oid: String,
    pub project_revision_ref: String,
    pub project_merge_result_ref: String,
    pub gate_decision_ref: String,
    pub evidence_bundle_ref: String,
    pub artifact_refs: Vec<String>,
    pub artifact_set_fingerprint: Sha256Hash,
    pub local_branch_state_ref: String,
    pub remote_merged_commit_oid: Option<String>,
    pub remote_snapshot_ref: Option<String>,
    pub migration_rollback_state: String,
    pub compatibility_state: String,
    pub unresolved_risks: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeBundleReleaseHandoff {
    pub schema_id: String,
    pub schema_version: u32,
    pub release_handoff_id: String,
    pub revision: u64,
    pub change_bundle_ref: String,
    pub multi_project_goal_ref: String,
    pub completion_target: CompletionLevel,
    pub completion_level_reached: CompletionLevel,
    pub project_inputs: Vec<ProjectReleaseInput>,
    pub dependency_order: Vec<ProjectId>,
    pub compatibility_windows: Vec<CompatibilityWindow>,
    pub overall_gate_ref: String,
    pub remaining_risks: Vec<String>,
    pub limitations: Vec<String>,
    pub ready: bool,
    pub handoff_fingerprint: Sha256Hash,
}
