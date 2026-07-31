//! Backend-neutral P0 development-management contracts.

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fmt,
    str::FromStr,
};

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    Sha256Hash,
    evidence::ArtifactRef,
    ids::{
        BackupSetId, BaselineId, CanonicalSourceId, ChangePlanId, CheckoutId,
        CoordinatedOperationId, DispositionId, FindingId, GenerationId, ManagementStoreId,
        OccurrenceId, OperationId, PatchSetId, ProjectId, ProjectRevisionId, RootBindingId,
        ScanRunId, SuppressionId, SymbolId, SymbolReferenceId, ValidationResultId,
        WorkspaceSnapshotId,
    },
};

pub const MANAGEMENT_STORE_VERSION: u32 = 2;
pub const REDACTION_CONTRACT_VERSION: u32 = 1;
pub const MANAGEMENT_STATUS_PAGE_SCHEMA_ID: &str = "star.management-status-page";
pub const MANAGEMENT_STATUS_PAGE_SCHEMA_VERSION: u32 = 1;
pub const MANAGEMENT_STATUS_QUERY_SCHEMA_ID: &str = "star.management-status-query";
pub const MANAGEMENT_MAINTENANCE_STATE_SCHEMA_ID: &str = "star.management-maintenance-state";
pub const MANAGEMENT_MAINTENANCE_STATE_SCHEMA_VERSION: u32 = 1;
pub const MANAGEMENT_STATUS_DEFAULT_MAX_ITEMS: u32 = 32;
pub const MANAGEMENT_STATUS_MAX_ITEMS: u32 = 256;
pub const MANAGEMENT_STATUS_DEFAULT_MAX_BYTES: u64 = 65_536;
pub const MANAGEMENT_STATUS_MAX_BYTES: u64 = 65_536;
pub const RETENTION_PLAN_V2_SCHEMA_ID: &str = "star.management-retention-plan";
pub const RETENTION_PLAN_V2_SCHEMA_VERSION: u32 = 2;
pub const RETENTION_CHECKPOINT_SCHEMA_ID: &str = "star.management-retention-checkpoint";
pub const RETENTION_CHECKPOINT_SCHEMA_VERSION: u32 = 1;
pub const RETENTION_DEFAULT_BATCH_ROWS: u64 = 10_000;
pub const RETENTION_DEFAULT_BATCH_BYTES: u64 = 64 * 1_024 * 1_024;
pub const MANAGEMENT_COMPACTION_PLAN_SCHEMA_ID: &str = "star.management-compaction-plan";
pub const MANAGEMENT_COMPACTION_PLAN_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum ManagementDecodeError {
    #[error("management JSON is invalid or contains duplicate keys")]
    InvalidJson,
    #[error("management schema ID does not match the expected contract")]
    SchemaId,
    #[error("management schema version is unsupported")]
    SchemaVersion,
    #[error("management document shape is invalid")]
    Shape,
}

pub fn decode_current_management_document<T: serde::de::DeserializeOwned>(
    input: &str,
    expected_schema_id: &str,
) -> Result<T, ManagementDecodeError> {
    decode_management_document(input, expected_schema_id, 1)
}

pub fn decode_management_document<T: serde::de::DeserializeOwned>(
    input: &str,
    expected_schema_id: &str,
    expected_schema_version: u32,
) -> Result<T, ManagementDecodeError> {
    let value =
        crate::parse_no_duplicate_keys(input).map_err(|_| ManagementDecodeError::InvalidJson)?;
    let object = value.as_object().ok_or(ManagementDecodeError::Shape)?;
    if object.get("schema_id").and_then(serde_json::Value::as_str) != Some(expected_schema_id) {
        return Err(ManagementDecodeError::SchemaId);
    }
    if object
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        != Some(u64::from(expected_schema_version))
    {
        return Err(ManagementDecodeError::SchemaVersion);
    }
    serde_json::from_value(value).map_err(|_| ManagementDecodeError::Shape)
}

#[derive(Debug, Error)]
pub enum ProjectPathError {
    #[error(
        "project path must be non-empty, relative, slash-separated, and must not escape the project root"
    )]
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ProjectPathRef(String);

impl ProjectPathRef {
    pub fn parse(value: impl Into<String>) -> Result<Self, ProjectPathError> {
        let value = value.into();
        let invalid = value.is_empty()
            || value.starts_with('/')
            || value.ends_with('/')
            || value.contains('\\')
            || value.contains('\0')
            || value.contains(':')
            || value
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..");
        if invalid {
            return Err(ProjectPathError::Invalid);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProjectPathRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ProjectPathRef {
    type Err = ProjectPathError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl<'de> Deserialize<'de> for ProjectPathRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for ProjectPathRef {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("ProjectPathRef")
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type":"string",
            "minLength":1,
            "pattern":r"^[^/\\:\u0000]+(?:/[^/\\:\u0000]+)*$"
        })
    }
}

macro_rules! string_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    };
}

string_enum!(IdentityScope { Shared, Local });
string_enum!(RepositoryKind { Git, None });
string_enum!(RegistrationState {
    Attached,
    Detached,
    Invalid
});
string_enum!(CheckoutKind {
    MainWorktree,
    LinkedWorktree,
    Clone,
    FilesystemRoot
});
string_enum!(CheckoutHeadState {
    Branch,
    Detached,
    Unborn,
    Unavailable
});
string_enum!(CheckoutAttachmentState {
    Attached,
    Detached,
    Missing,
    IdentityConflict,
    Unsupported
});
string_enum!(MigrationApplyState {
    Completed,
    Interrupted
});
string_enum!(Completeness {
    Complete,
    Partial,
    Unverified
});
string_enum!(RevisionKind {
    GitCommit,
    FilesystemManifest
});
string_enum!(ScanStatus {
    Queued,
    Running,
    Succeeded,
    Incomplete,
    Failed,
    Cancelled
});
string_enum!(SourceKind {
    File,
    GeneratedUnit,
    Virtual
});
string_enum!(Sensitivity {
    Public,
    Internal,
    Sensitive
});
string_enum!(RuleLifecycle {
    Active,
    Deprecated,
    Disabled
});
string_enum!(Severity {
    Info,
    Warning,
    Error,
    Critical
});
string_enum!(Confidence { Low, Medium, High });
string_enum!(SymbolResolution {
    Resolved,
    Ambiguous,
    Unresolved,
    External
});
string_enum!(FindingLifecycle {
    Open,
    NotObserved,
    Resolved
});
string_enum!(SuppressionScope { Shared, Local });
string_enum!(SuppressionStatus {
    Active,
    Expired,
    Revoked,
    Stale
});
string_enum!(BaselineScope { Shared, Local });
string_enum!(BaselineStatus {
    Active,
    Superseded,
    Invalid
});
string_enum!(DispositionDecision {
    NeedsAction,
    AcceptedRisk,
    FalsePositive,
    Deferred,
    Duplicate,
    Fixed
});
string_enum!(DispositionStatus {
    Active,
    Stale,
    Revoked
});
string_enum!(ChangePlanStatus {
    Draft,
    Ready,
    Applied,
    Validated,
    Blocked,
    Abandoned
});
string_enum!(FileOperationKind {
    Add,
    Modify,
    Delete,
    Rename
});
string_enum!(PatchSetStatus {
    Proposed,
    Applied,
    PartiallyApplied,
    Failed,
    Reverted
});
string_enum!(ValidationOutcome {
    Pass,
    Fail,
    Incomplete,
    Error,
    Cancelled
});
string_enum!(IntegrityState {
    Healthy,
    Suspect,
    Corrupt
});
string_enum!(StoreOpenMode {
    ReadWrite,
    MigrationRequired,
    ReadOnlyRecovery,
    Quarantined
});
string_enum!(CoordinationState {
    Prepared,
    Applying,
    Completed,
    Blocked,
    OutcomeUnknown
});
string_enum!(ParticipantState {
    Pending,
    Committed,
    Blocked,
    OutcomeUnknown
});
string_enum!(RedactionState {
    NotNeeded,
    Redacted,
    Quarantined,
    Rejected
});

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceRange {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub project_id: ProjectId,
    pub identity_scope: IdentityScope,
    pub display_name: String,
    pub repository_kind: RepositoryKind,
    pub source_of_truth: Vec<String>,
    pub declaration_fingerprint: Sha256Hash,
    pub registration_state: RegistrationState,
    pub root_binding_id: Option<RootBindingId>,
    pub latest_revision_id: Option<ProjectRevisionId>,
    pub latest_workspace_snapshot_id: Option<WorkspaceSnapshotId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub schema_id: String,
    pub schema_version: u32,
    pub project_id: ProjectId,
    pub identity_scope: IdentityScope,
    pub display_name: String,
    pub repository_kind: RepositoryKind,
    pub source_of_truth: Vec<String>,
    pub declaration_fingerprint: Sha256Hash,
    pub registration_state: RegistrationState,
    pub attached_checkout_ids: Vec<CheckoutId>,
    pub latest_revision_id: Option<ProjectRevisionId>,
    pub latest_workspace_snapshot_id: Option<WorkspaceSnapshotId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoteIdentity {
    pub host: String,
    pub owner: String,
    pub repository: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectCheckout {
    pub schema_id: String,
    pub schema_version: u32,
    pub checkout_id: CheckoutId,
    pub project_id: ProjectId,
    pub root_binding_id: Option<RootBindingId>,
    pub repository_kind: RepositoryKind,
    pub checkout_kind: CheckoutKind,
    pub repository_binding_id: Option<String>,
    pub worktree_binding_id: Option<String>,
    pub object_format: Option<String>,
    pub head_state: CheckoutHeadState,
    pub head_ref: Option<String>,
    pub head_commit_id: Option<String>,
    pub head_tree_id: Option<String>,
    pub upstream_ref: Option<String>,
    pub default_branch_hint: Option<String>,
    pub remote_identity: Option<RemoteIdentity>,
    pub attachment_state: CheckoutAttachmentState,
    pub last_observed_at: DateTime<Utc>,
    pub limitations: Vec<String>,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectV1ToV2MigrationEntry {
    pub project_id: ProjectId,
    pub source_project: ProjectV1,
    pub source_project_fingerprint: Sha256Hash,
    pub source_updated_at: String,
    pub source_root_binding_id: Option<RootBindingId>,
    pub project: Project,
    pub checkout: Option<ProjectCheckout>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectV1ToV2MigrationPlan {
    pub schema_id: String,
    pub schema_version: u32,
    pub source_store_version: u32,
    pub target_store_version: u32,
    pub entries: Vec<ProjectV1ToV2MigrationEntry>,
    pub limitations: Vec<String>,
    pub plan_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectV1ToV2MigrationResult {
    pub schema_id: String,
    pub schema_version: u32,
    pub state: MigrationApplyState,
    pub completed_steps: usize,
    pub total_steps: usize,
    pub plan_fingerprint: Sha256Hash,
    pub backup_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectRevision {
    pub schema_id: String,
    pub schema_version: u32,
    pub project_revision_id: ProjectRevisionId,
    pub project_id: ProjectId,
    pub revision_kind: RevisionKind,
    pub vcs_object_format: Option<String>,
    pub commit_id: Option<String>,
    pub tree_id: Option<String>,
    pub manifest_fingerprint: Option<Sha256Hash>,
    pub captured_at: DateTime<Utc>,
    pub completeness: Completeness,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSnapshot {
    pub schema_id: String,
    pub schema_version: u32,
    pub workspace_snapshot_id: WorkspaceSnapshotId,
    pub project_id: ProjectId,
    pub project_revision_id: ProjectRevisionId,
    pub scope: Vec<String>,
    pub entries_manifest_ref: ArtifactRef,
    pub entries_fingerprint: Sha256Hash,
    pub dirty_summary: BTreeMap<String, u64>,
    pub ignored_policy: String,
    pub symlink_policy: String,
    pub captured_at: DateTime<Utc>,
    pub completeness: Completeness,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub schema_id: String,
    pub schema_version: u32,
    pub rule_id: String,
    pub rule_version: String,
    pub definition_fingerprint: Sha256Hash,
    pub title: String,
    pub category: String,
    pub default_severity: Severity,
    pub default_confidence: Confidence,
    pub supported_languages: Vec<String>,
    pub source_kinds: Vec<SourceKind>,
    pub analyzer_ref: String,
    pub parameter_schema_ref: String,
    pub identity_contract_version: u32,
    pub identity_anchor: String,
    pub redaction_contract_version: u32,
    pub remediation_recipe_refs: Vec<String>,
    pub lifecycle: RuleLifecycle,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScanRun {
    pub schema_id: String,
    pub schema_version: u32,
    pub scan_run_id: ScanRunId,
    pub project_id: ProjectId,
    pub project_revision_id: ProjectRevisionId,
    pub workspace_snapshot_id: WorkspaceSnapshotId,
    pub effective_config_fingerprint: Sha256Hash,
    pub scan_config_fingerprint: Sha256Hash,
    pub rule_set_fingerprint: Sha256Hash,
    pub input_fingerprint: Sha256Hash,
    pub status: ScanStatus,
    pub generation_id: GenerationId,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub reused_from_scan_run_id: Option<ScanRunId>,
    pub counts: BTreeMap<String, u64>,
    pub limitations: Vec<String>,
    pub artifact_refs: Vec<ArtifactRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CanonicalSource {
    pub schema_id: String,
    pub schema_version: u32,
    pub canonical_source_id: CanonicalSourceId,
    pub project_id: ProjectId,
    pub path: Option<ProjectPathRef>,
    pub source_kind: SourceKind,
    pub language_id: Option<String>,
    pub content_sha256: Option<Sha256Hash>,
    pub project_revision_id: Option<ProjectRevisionId>,
    pub workspace_snapshot_id: Option<WorkspaceSnapshotId>,
    pub generated_from_refs: Vec<CanonicalSourceId>,
    pub sensitivity: Sensitivity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Symbol {
    pub schema_id: String,
    pub schema_version: u32,
    pub symbol_id: SymbolId,
    pub project_id: ProjectId,
    pub canonical_source_id: CanonicalSourceId,
    pub language_id: String,
    pub symbol_kind: String,
    pub qualified_name: String,
    pub signature_fingerprint: Option<Sha256Hash>,
    pub declaration_range: SourceRange,
    pub visibility: Option<String>,
    pub workspace_snapshot_id: WorkspaceSnapshotId,
    pub scan_run_id: ScanRunId,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SymbolReference {
    pub schema_id: String,
    pub schema_version: u32,
    pub symbol_reference_id: SymbolReferenceId,
    pub project_id: ProjectId,
    pub from_symbol_id: Option<SymbolId>,
    pub from_source_id: CanonicalSourceId,
    pub from_range: SourceRange,
    pub to_symbol_id: Option<SymbolId>,
    pub unresolved_target: Option<String>,
    pub reference_kind: String,
    pub resolution: SymbolResolution,
    pub workspace_snapshot_id: WorkspaceSnapshotId,
    pub scan_run_id: ScanRunId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Occurrence {
    pub schema_id: String,
    pub schema_version: u32,
    pub occurrence_id: OccurrenceId,
    pub occurrence_fingerprint: Sha256Hash,
    pub finding_id: FindingId,
    pub scan_run_id: ScanRunId,
    pub project_revision_id: ProjectRevisionId,
    pub workspace_snapshot_id: WorkspaceSnapshotId,
    pub canonical_source_id: CanonicalSourceId,
    pub source_content_sha256: Sha256Hash,
    pub location_path: ProjectPathRef,
    pub location_range: SourceRange,
    pub symbol_id: Option<SymbolId>,
    pub message_parameters: BTreeMap<String, String>,
    pub evidence_refs: Vec<ArtifactRef>,
    pub observed_at: DateTime<Utc>,
    pub redaction_state: RedactionState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub schema_id: String,
    pub schema_version: u32,
    pub finding_id: FindingId,
    pub finding_fingerprint: Sha256Hash,
    pub project_id: ProjectId,
    pub rule_id: String,
    pub rule_version: String,
    pub identity_anchor: String,
    pub identity_tokens: Vec<String>,
    pub title_code: String,
    pub message_code: String,
    pub severity: Severity,
    pub confidence: Confidence,
    pub lifecycle: FindingLifecycle,
    pub first_observed_scan_id: ScanRunId,
    pub last_observed_scan_id: ScanRunId,
    pub current_occurrence_ids: Vec<OccurrenceId>,
    pub active_disposition_id: Option<DispositionId>,
    pub active_suppression_ids: Vec<SuppressionId>,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Suppression {
    pub schema_id: String,
    pub schema_version: u32,
    pub suppression_id: SuppressionId,
    pub revision: u64,
    pub scope_kind: SuppressionScope,
    pub project_id: ProjectId,
    pub selector: String,
    pub reason_code: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub permanent: bool,
    pub justification: Option<String>,
    pub source_revision_constraint: Option<ProjectRevisionId>,
    pub config_fingerprint_constraint: Option<Sha256Hash>,
    pub status: SuppressionStatus,
    pub provenance: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Baseline {
    pub schema_id: String,
    pub schema_version: u32,
    pub baseline_id: BaselineId,
    pub revision: u64,
    pub scope_kind: BaselineScope,
    pub project_id: ProjectId,
    pub project_revision_id: ProjectRevisionId,
    pub workspace_snapshot_id: WorkspaceSnapshotId,
    pub scan_config_fingerprint: Sha256Hash,
    pub rule_set_fingerprint: Sha256Hash,
    pub finding_fingerprints: Vec<Sha256Hash>,
    pub set_fingerprint: Sha256Hash,
    pub created_at: DateTime<Utc>,
    pub reason: String,
    pub reviewed: bool,
    pub status: BaselineStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Disposition {
    pub schema_id: String,
    pub schema_version: u32,
    pub disposition_id: DispositionId,
    pub revision: u64,
    pub finding_id: FindingId,
    pub finding_fingerprint: Sha256Hash,
    pub decision: DispositionDecision,
    pub reason_code: String,
    pub reason: String,
    pub scope_revision: Option<ProjectRevisionId>,
    pub expires_at: Option<DateTime<Utc>>,
    pub duplicate_of_finding_id: Option<FindingId>,
    pub decided_at: DateTime<Utc>,
    pub provenance: String,
    pub status: DispositionStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeRecipe {
    pub schema_id: String,
    pub schema_version: u32,
    pub recipe_id: String,
    pub recipe_version: String,
    pub definition_fingerprint: Sha256Hash,
    pub finding_selectors: Vec<String>,
    pub preconditions: Vec<String>,
    pub parameter_schema_ref: String,
    pub transformer_ref: String,
    pub allowed_path_scope: Vec<String>,
    pub idempotency_contract: String,
    pub validation_requirements: Vec<String>,
    pub risk_class: String,
    pub permission_actions: Vec<String>,
    pub rollback_contract: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeRecipeRef {
    pub recipe_id: String,
    pub recipe_version: String,
    pub definition_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangePlan {
    pub schema_id: String,
    pub schema_version: u32,
    pub change_plan_id: ChangePlanId,
    pub revision: u64,
    pub project_id: ProjectId,
    pub target_workspace_snapshot_id: WorkspaceSnapshotId,
    pub finding_refs: Vec<FindingId>,
    pub recipe_refs: Vec<ChangeRecipeRef>,
    pub parameters: BTreeMap<String, String>,
    pub expected_paths: Vec<ProjectPathRef>,
    pub preconditions: Vec<Sha256Hash>,
    pub risk: String,
    pub permission_plan_ref: String,
    pub validation_plan_ref: String,
    pub status: ChangePlanStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PatchFileOperation {
    pub kind: FileOperationKind,
    pub path: ProjectPathRef,
    pub rename_from: Option<ProjectPathRef>,
    pub before_sha256: Option<Sha256Hash>,
    pub after_sha256: Option<Sha256Hash>,
    pub before_mode: Option<u32>,
    pub after_mode: Option<u32>,
    pub operation_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PatchSet {
    pub schema_id: String,
    pub schema_version: u32,
    pub patch_set_id: PatchSetId,
    pub change_plan_id: ChangePlanId,
    pub change_plan_revision: u64,
    pub project_id: ProjectId,
    pub base_workspace_snapshot_id: WorkspaceSnapshotId,
    pub patch_fingerprint: Sha256Hash,
    pub operations: Vec<PatchFileOperation>,
    pub patch_artifact_refs: Vec<ArtifactRef>,
    pub affected_finding_ids: Vec<FindingId>,
    pub expected_result_fingerprint: Option<Sha256Hash>,
    pub status: PatchSetStatus,
    pub applied_workspace_snapshot_id: Option<WorkspaceSnapshotId>,
    pub rollback_artifact_refs: Vec<ArtifactRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidationResult {
    pub schema_id: String,
    pub schema_version: u32,
    pub validation_result_id: ValidationResultId,
    pub subject_kind: String,
    pub subject_id: String,
    pub project_id: ProjectId,
    pub project_revision_id: ProjectRevisionId,
    pub workspace_snapshot_id: WorkspaceSnapshotId,
    pub validation_plan_ref: String,
    pub validation_run_refs: Vec<String>,
    pub effective_config_fingerprint: Sha256Hash,
    pub outcome: ValidationOutcome,
    pub completeness: Completeness,
    pub finding_refs: Vec<FindingId>,
    pub diagnostic_refs: Vec<String>,
    pub artifact_refs: Vec<ArtifactRef>,
    pub result_fingerprint: Sha256Hash,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StoreScope {
    Global,
    Project { project_id: ProjectId },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StorePoint {
    pub store_id: ManagementStoreId,
    pub generation: u64,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectStorePoint {
    pub project_id: ProjectId,
    pub point: StorePoint,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StoreVersionVector {
    pub global: StorePoint,
    pub projects: Vec<ProjectStorePoint>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagementStoreStatus {
    pub schema_id: String,
    pub schema_version: u32,
    pub store_id: ManagementStoreId,
    pub store_scope: StoreScope,
    pub management_store_version: u32,
    pub min_reader_version: u32,
    pub writer_version: u32,
    pub store_revision: u64,
    pub generation: u64,
    pub created_by_product_version: String,
    pub last_opened_by_product_version: String,
    pub last_clean_shutdown: bool,
    pub integrity_state: IntegrityState,
    pub open_mode: StoreOpenMode,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub last_backup_ref: Option<ArtifactRef>,
    pub redaction_contract_version: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagementStatusQueryV1 {
    pub cursor: Option<String>,
    pub max_items: u32,
    pub max_bytes: u64,
}

impl Default for ManagementStatusQueryV1 {
    fn default() -> Self {
        Self {
            cursor: None,
            max_items: MANAGEMENT_STATUS_DEFAULT_MAX_ITEMS,
            max_bytes: MANAGEMENT_STATUS_DEFAULT_MAX_BYTES,
        }
    }
}

impl ManagementStatusQueryV1 {
    pub fn validate(&self) -> Result<(), ManagementDecodeError> {
        if self.max_items == 0
            || self.max_items > MANAGEMENT_STATUS_MAX_ITEMS
            || self.max_bytes == 0
            || self.max_bytes > MANAGEMENT_STATUS_MAX_BYTES
            || self
                .cursor
                .as_ref()
                .is_some_and(|value| value.is_empty() || value.len() > 1_024 || !value.is_ascii())
        {
            return Err(ManagementDecodeError::Shape);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagementStatusPageV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub snapshot_revision: u64,
    pub snapshot_hash: Sha256Hash,
    pub items: Vec<ManagementStoreStatus>,
    pub returned_count: u32,
    pub total_count: u64,
    pub truncated: bool,
    pub next_cursor: Option<String>,
    pub max_items: u32,
    pub max_bytes: u64,
}

impl ManagementStatusPageV1 {
    pub fn validate(&self) -> Result<(), ManagementDecodeError> {
        let serialized_len = serde_json::to_vec(self)
            .map_err(|_| ManagementDecodeError::Shape)?
            .len() as u64;
        if self.schema_id != MANAGEMENT_STATUS_PAGE_SCHEMA_ID
            || self.schema_version != MANAGEMENT_STATUS_PAGE_SCHEMA_VERSION
            || self.returned_count as usize != self.items.len()
            || self.returned_count as u64 > self.total_count
            || self.returned_count > self.max_items
            || self.max_items == 0
            || self.max_items > MANAGEMENT_STATUS_MAX_ITEMS
            || self.max_bytes == 0
            || self.max_bytes > MANAGEMENT_STATUS_MAX_BYTES
            || serialized_len > self.max_bytes
            || self
                .next_cursor
                .as_ref()
                .is_some_and(|value| value.is_empty() || value.len() > 1_024 || !value.is_ascii())
            || (self.truncated != self.next_cursor.is_some())
        {
            return Err(ManagementDecodeError::Shape);
        }
        Ok(())
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ManagementMaintenancePhase {
    Idle,
    Planning,
    ApprovalRequired,
    Applying,
    Checkpointing,
    Complete,
    Cancelled,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagementMaintenanceStateV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub operation_id: Option<OperationId>,
    pub phase: ManagementMaintenancePhase,
    pub policy_fingerprint: Option<Sha256Hash>,
    pub plan_fingerprint: Option<Sha256Hash>,
    pub checkpoint_cursor: Option<String>,
    pub planned_rows: u64,
    pub planned_bytes: u64,
    pub applied_rows: u64,
    pub applied_bytes: u64,
    pub started_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub last_error_code: Option<String>,
}

impl ManagementMaintenanceStateV1 {
    pub fn idle(now: DateTime<Utc>) -> Self {
        Self {
            schema_id: MANAGEMENT_MAINTENANCE_STATE_SCHEMA_ID.to_owned(),
            schema_version: MANAGEMENT_MAINTENANCE_STATE_SCHEMA_VERSION,
            operation_id: None,
            phase: ManagementMaintenancePhase::Idle,
            policy_fingerprint: None,
            plan_fingerprint: None,
            checkpoint_cursor: None,
            planned_rows: 0,
            planned_bytes: 0,
            applied_rows: 0,
            applied_bytes: 0,
            started_at: None,
            updated_at: now,
            last_error_code: None,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.phase,
            ManagementMaintenancePhase::Planning
                | ManagementMaintenancePhase::Applying
                | ManagementMaintenancePhase::Checkpointing
        )
    }
}

string_enum!(RetentionTargetKindV2 {
    ScanGeneration,
    ResolvedFinding,
    LocalSuppression,
    LocalDisposition
});

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RetentionPolicyV2 {
    pub keep_latest_successful_scans: u64,
    pub incomplete_staging_retention_days: u64,
    pub scan_detail_retention_days: u64,
    pub resolved_finding_retention_days: u64,
    pub local_decision_retention_days: u64,
    pub migration_backup_min_count: u64,
    pub max_batch_rows: u64,
    pub max_batch_bytes: u64,
}

impl Default for RetentionPolicyV2 {
    fn default() -> Self {
        Self {
            keep_latest_successful_scans: 2,
            incomplete_staging_retention_days: 7,
            scan_detail_retention_days: 90,
            resolved_finding_retention_days: 180,
            local_decision_retention_days: 180,
            migration_backup_min_count: 2,
            max_batch_rows: RETENTION_DEFAULT_BATCH_ROWS,
            max_batch_bytes: RETENTION_DEFAULT_BATCH_BYTES,
        }
    }
}

impl RetentionPolicyV2 {
    pub fn validate(&self) -> Result<(), ManagementDecodeError> {
        if self.keep_latest_successful_scans == 0
            || self.incomplete_staging_retention_days == 0
            || self.scan_detail_retention_days == 0
            || self.resolved_finding_retention_days == 0
            || self.local_decision_retention_days == 0
            || self.migration_backup_min_count == 0
            || self.max_batch_rows == 0
            || self.max_batch_rows > RETENTION_DEFAULT_BATCH_ROWS
            || self.max_batch_bytes == 0
            || self.max_batch_bytes > RETENTION_DEFAULT_BATCH_BYTES
        {
            return Err(ManagementDecodeError::Shape);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RetentionCandidateV2 {
    pub candidate_id: String,
    pub project_id: ProjectId,
    pub target_kind: RetentionTargetKindV2,
    pub generation_id: Option<String>,
    pub entity_id: Option<String>,
    pub entity_revision: Option<u64>,
    pub scan_run_id: Option<ScanRunId>,
    pub retention_class: String,
    pub reason_code: String,
    pub estimated_rows: u64,
    pub estimated_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RetentionProtectedTargetV1 {
    pub project_id: ProjectId,
    pub target_kind: RetentionTargetKindV2,
    pub target_ref: String,
    pub reason_code: String,
    pub estimated_rows: u64,
    pub estimated_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RetentionMigrationBackupCandidateV1 {
    pub candidate_id: String,
    pub backup_locator: String,
    pub backup_fingerprint: Sha256Hash,
    pub manifest_fingerprint: Sha256Hash,
    pub last_modified_at: DateTime<Utc>,
    pub reason_code: String,
    pub estimated_rows: u64,
    pub estimated_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RetentionProtectedMigrationBackupV1 {
    pub backup_locator: String,
    pub backup_fingerprint: Option<Sha256Hash>,
    pub manifest_fingerprint: Option<Sha256Hash>,
    pub last_modified_at: Option<DateTime<Utc>>,
    pub reason_code: String,
    pub estimated_rows: u64,
    pub estimated_bytes: u64,
}

fn valid_backup_locator(value: &str, require_migration_name: bool) -> bool {
    if value.is_empty()
        || value.len() > 255
        || value == "."
        || value == ".."
        || value.contains(['/', '\\', '\0'])
    {
        return false;
    }
    if !require_migration_name {
        return true;
    }
    value
        .strip_prefix("project-v1-to-v2-")
        .is_some_and(|suffix| {
            suffix.len() == 64 && suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

fn bounded_retention_text(value: &str, max_len: usize) -> bool {
    !value.is_empty() && value.len() <= max_len && !value.contains('\0')
}

fn retention_candidate_shape_valid(candidate: &RetentionCandidateV2) -> bool {
    if Sha256Hash::from_str(&candidate.candidate_id).is_err()
        || !bounded_retention_text(&candidate.retention_class, 128)
        || !bounded_retention_text(&candidate.reason_code, 128)
        || candidate
            .generation_id
            .as_deref()
            .is_some_and(|value| !bounded_retention_text(value, 256))
        || candidate
            .entity_id
            .as_deref()
            .is_some_and(|value| !bounded_retention_text(value, 256))
    {
        return false;
    }
    match candidate.target_kind {
        RetentionTargetKindV2::ScanGeneration => {
            candidate.generation_id.is_some()
                && candidate.entity_id.is_none()
                && candidate.entity_revision.is_none()
                && candidate.scan_run_id.is_some()
                && matches!(
                    candidate.retention_class.as_str(),
                    "incomplete_staging" | "successful_scan_detail"
                )
        }
        RetentionTargetKindV2::ResolvedFinding => {
            candidate.generation_id.is_some()
                && candidate.entity_id.is_some()
                && candidate.entity_revision.is_none()
                && candidate.scan_run_id.is_some()
                && candidate.retention_class == "resolved_finding"
        }
        RetentionTargetKindV2::LocalSuppression => {
            candidate.generation_id.is_none()
                && candidate.entity_id.is_some()
                && candidate.entity_revision.is_some()
                && candidate.scan_run_id.is_none()
                && matches!(
                    candidate.retention_class.as_str(),
                    "local_suppression_v1" | "local_suppression_v2"
                )
        }
        RetentionTargetKindV2::LocalDisposition => {
            candidate.generation_id.is_none()
                && candidate.entity_id.is_some()
                && candidate.entity_revision.is_some()
                && candidate.scan_run_id.is_none()
                && matches!(
                    candidate.retention_class.as_str(),
                    "local_disposition_v1" | "local_disposition_v2"
                )
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RetentionPlanV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub created_at: DateTime<Utc>,
    pub policy: RetentionPolicyV2,
    pub policy_fingerprint: Sha256Hash,
    pub source_fingerprint: Sha256Hash,
    pub expected_store_revisions: BTreeMap<String, u64>,
    pub candidates: Vec<RetentionCandidateV2>,
    pub protected_targets: Vec<RetentionProtectedTargetV1>,
    pub migration_backup_candidates: Vec<RetentionMigrationBackupCandidateV1>,
    pub protected_migration_backups: Vec<RetentionProtectedMigrationBackupV1>,
    pub estimated_rows: u64,
    pub estimated_bytes: u64,
    pub plan_fingerprint: Sha256Hash,
}

impl RetentionPlanV2 {
    pub fn total_candidate_count(&self) -> usize {
        self.candidates
            .len()
            .saturating_add(self.migration_backup_candidates.len())
    }

    pub fn validate(&self) -> Result<(), ManagementDecodeError> {
        self.policy.validate()?;
        let totals = self
            .candidates
            .iter()
            .map(|candidate| (candidate.estimated_rows, candidate.estimated_bytes))
            .chain(
                self.migration_backup_candidates
                    .iter()
                    .map(|candidate| (candidate.estimated_rows, candidate.estimated_bytes)),
            )
            .try_fold(
                (0_u64, 0_u64),
                |(rows, bytes), (candidate_rows, candidate_bytes)| {
                    Some((
                        rows.checked_add(candidate_rows)?,
                        bytes.checked_add(candidate_bytes)?,
                    ))
                },
            );
        let mut candidate_ids = BTreeSet::new();
        let mut backup_locators = BTreeSet::new();
        let mut protected_target_refs = BTreeSet::new();
        if self.schema_id != RETENTION_PLAN_V2_SCHEMA_ID
            || self.schema_version != RETENTION_PLAN_V2_SCHEMA_VERSION
            || !self.expected_store_revisions.contains_key("global")
            || totals != Some((self.estimated_rows, self.estimated_bytes))
            || self.candidates.iter().any(|candidate| {
                !retention_candidate_shape_valid(candidate)
                    || !candidate_ids.insert(candidate.candidate_id.as_str())
                    || candidate.estimated_rows == 0
                    || candidate.estimated_bytes == 0
            })
            || self.protected_targets.iter().any(|target| {
                Sha256Hash::from_str(&target.target_ref).is_err()
                    || !protected_target_refs.insert(target.target_ref.as_str())
                    || !bounded_retention_text(&target.reason_code, 128)
                    || target.estimated_rows == 0
            })
            || self.migration_backup_candidates.iter().any(|candidate| {
                Sha256Hash::from_str(&candidate.candidate_id).is_err()
                    || !candidate_ids.insert(candidate.candidate_id.as_str())
                    || !valid_backup_locator(&candidate.backup_locator, true)
                    || !backup_locators.insert(candidate.backup_locator.as_str())
                    || !bounded_retention_text(&candidate.reason_code, 128)
                    || candidate.estimated_rows == 0
                    || candidate.estimated_rows > self.policy.max_batch_rows
                    || candidate.estimated_bytes == 0
                    || candidate.estimated_bytes > self.policy.max_batch_bytes
            })
            || self.protected_migration_backups.iter().any(|backup| {
                let metadata_present = (
                    backup.backup_fingerprint.is_some(),
                    backup.manifest_fingerprint.is_some(),
                    backup.last_modified_at.is_some(),
                );
                !valid_backup_locator(&backup.backup_locator, false)
                    || !backup_locators.insert(backup.backup_locator.as_str())
                    || !bounded_retention_text(&backup.reason_code, 128)
                    || !matches!(metadata_present, (true, true, true) | (false, false, false))
                    || (metadata_present == (false, false, false)
                        && (backup.estimated_rows != 0 || backup.estimated_bytes != 0))
                    || (metadata_present == (true, true, true) && backup.estimated_rows == 0)
            })
        {
            return Err(ManagementDecodeError::Shape);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RetentionCheckpointV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub operation_id: OperationId,
    pub plan_fingerprint: Sha256Hash,
    pub expected_store_revisions: BTreeMap<String, u64>,
    pub next_candidate_index: u64,
    pub committed_batch_count: u64,
    pub applied_rows: u64,
    pub applied_bytes: u64,
    pub last_candidate_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_candidate_id: Option<String>,
    #[serde(default)]
    pub active_candidate_applied_rows: u64,
    #[serde(default)]
    pub active_candidate_applied_bytes: u64,
    pub complete: bool,
    pub cancelled: bool,
    pub updated_at: DateTime<Utc>,
}

impl RetentionCheckpointV1 {
    pub fn start(
        operation_id: OperationId,
        plan_fingerprint: Sha256Hash,
        expected_store_revisions: BTreeMap<String, u64>,
    ) -> Self {
        Self {
            schema_id: RETENTION_CHECKPOINT_SCHEMA_ID.to_owned(),
            schema_version: RETENTION_CHECKPOINT_SCHEMA_VERSION,
            operation_id,
            plan_fingerprint,
            expected_store_revisions,
            next_candidate_index: 0,
            committed_batch_count: 0,
            applied_rows: 0,
            applied_bytes: 0,
            last_candidate_id: None,
            active_candidate_id: None,
            active_candidate_applied_rows: 0,
            active_candidate_applied_bytes: 0,
            complete: false,
            cancelled: false,
            updated_at: Utc::now(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RetentionBatchResultV1 {
    pub processed_candidates: u64,
    pub applied_rows: u64,
    pub applied_bytes: u64,
    pub checkpoint: RetentionCheckpointV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagementCompactionPlanV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub store_id: ManagementStoreId,
    pub store_scope: StoreScope,
    pub store_revision: u64,
    pub source_fingerprint: Sha256Hash,
    pub store_fingerprint: Sha256Hash,
    pub database_bytes: u64,
    pub estimated_reclaimable_bytes: u64,
    pub required_free_bytes: u64,
    pub available_free_bytes_at_plan: u64,
    pub backup_binding: Option<ManagementCompactionBackupBindingV1>,
    pub created_at: DateTime<Utc>,
    pub plan_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagementCompactionBackupBindingV1 {
    pub backup_set_id: BackupSetId,
    pub approved_backup_plan_fingerprint: Sha256Hash,
    pub backup_result_fingerprint: Sha256Hash,
    pub backup_set_fingerprint: Sha256Hash,
    pub backup_destination_fingerprint: Sha256Hash,
    pub source_active_set_fingerprint: Sha256Hash,
    pub store_id: ManagementStoreId,
    pub store_revision: u64,
    pub store_size_bytes: u64,
    pub store_byte_sha256: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagementCompactionApplyResultV1 {
    pub plan_fingerprint: Sha256Hash,
    pub before_bytes: u64,
    pub after_bytes: u64,
    pub completed_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ParticipantReceipt {
    pub project_id: ProjectId,
    pub operation_id: CoordinatedOperationId,
    pub payload_fingerprint: Sha256Hash,
    pub result_fingerprint: Sha256Hash,
    pub committed_store_revision: u64,
    pub local_event_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoordinationParticipant {
    pub project_id: ProjectId,
    pub required: bool,
    pub payload_fingerprint: Sha256Hash,
    pub state: ParticipantState,
    pub receipt: Option<ParticipantReceipt>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoordinatedOperation {
    pub schema_id: String,
    pub schema_version: u32,
    pub coordinated_operation_id: CoordinatedOperationId,
    pub idempotency_key: String,
    pub command_kind: String,
    pub input_fingerprint: Sha256Hash,
    pub permission_scope_fingerprint: Sha256Hash,
    pub expected_version_vector: StoreVersionVector,
    pub participants: Vec<CoordinationParticipant>,
    pub state: CoordinationState,
    pub result_fingerprint: Option<Sha256Hash>,
    pub committed_version_vector: Option<StoreVersionVector>,
    pub diagnostic_refs: Vec<String>,
    pub artifact_refs: Vec<ArtifactRef>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::{ArtifactRef, GateDecision};

    fn assert_fixture<T: serde::de::DeserializeOwned>(
        root: &std::path::Path,
        directory: &str,
        schema_id: &str,
    ) {
        assert_fixture_version::<T>(root, directory, schema_id, 1);
    }

    fn assert_fixture_version<T: serde::de::DeserializeOwned>(
        root: &std::path::Path,
        directory: &str,
        schema_id: &str,
        schema_version: u32,
    ) {
        for name in ["minimal.json", "full.json"] {
            let source = std::fs::read_to_string(root.join(directory).join(name)).unwrap();
            decode_management_document::<T>(&source, schema_id, schema_version).unwrap();
        }
        for name in ["invalid.json", "future.json"] {
            let source = std::fs::read_to_string(root.join(directory).join(name)).unwrap();
            assert!(decode_management_document::<T>(&source, schema_id, schema_version).is_err());
        }
    }

    fn assert_strict_artifact_fixture(root: &std::path::Path) {
        for name in ["minimal.json", "full.json"] {
            let source = std::fs::read_to_string(root.join("artifact-ref").join(name)).unwrap();
            let artifact: ArtifactRef = serde_json::from_str(&source).unwrap();
            artifact.validate().unwrap();
        }
        for name in ["invalid.json", "future.json"] {
            let source = std::fs::read_to_string(root.join("artifact-ref").join(name)).unwrap();
            assert!(serde_json::from_str::<ArtifactRef>(&source).is_err());
        }
    }

    #[test]
    fn project_path_rejects_absolute_escape_and_windows_special_forms() {
        for invalid in ["", "/root", "a\\b", "a/../b", "./a", "C:/a", "a//b", "a/"] {
            assert!(ProjectPathRef::parse(invalid).is_err(), "{invalid}");
        }
        assert_eq!(
            ProjectPathRef::parse("src/lib.rs").unwrap().as_str(),
            "src/lib.rs"
        );
    }

    #[test]
    fn all_contract_enums_use_snake_case() {
        assert_eq!(
            serde_json::to_string(&CoordinationState::OutcomeUnknown).unwrap(),
            "\"outcome_unknown\""
        );
    }

    #[test]
    fn strict_management_decoder_rejects_future_duplicate_and_unknown_fields() {
        let project_id = ProjectId::new();
        let hash = Sha256Hash::digest(b"project");
        let minimal = serde_json::json!({
            "schema_id":"star.project",
            "schema_version":2,
            "project_id":project_id,
            "identity_scope":"local",
            "display_name":"Local project",
            "repository_kind":"none",
            "source_of_truth":["source"],
            "declaration_fingerprint":hash,
            "registration_state":"detached",
            "attached_checkout_ids":[],
            "latest_revision_id":null,
            "latest_workspace_snapshot_id":null
        });
        let encoded = serde_json::to_string(&minimal).unwrap();
        assert!(decode_management_document::<Project>(&encoded, "star.project", 2).is_ok());
        let future = encoded.replace("\"schema_version\":2", "\"schema_version\":3");
        assert!(matches!(
            decode_management_document::<Project>(&future, "star.project", 2),
            Err(ManagementDecodeError::SchemaVersion)
        ));
        let duplicate = encoded.replacen("{", "{\"schema_id\":\"star.project\",", 1);
        assert!(matches!(
            decode_management_document::<Project>(&duplicate, "star.project", 2),
            Err(ManagementDecodeError::InvalidJson)
        ));
        let mut unknown = minimal;
        unknown
            .as_object_mut()
            .unwrap()
            .insert("unexpected".to_owned(), true.into());
        assert!(matches!(
            decode_management_document::<Project>(
                &serde_json::to_string(&unknown).unwrap(),
                "star.project",
                2
            ),
            Err(ManagementDecodeError::Shape)
        ));
    }

    #[test]
    fn generated_management_fixtures_round_trip_through_strict_rust_types() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../specs/fixtures/management/v1");
        assert_fixture_version::<Project>(&root, "project", "star.project", 2);
        assert_fixture::<ProjectV1>(&root, "project-v1", "star.project");
        assert_fixture::<ProjectCheckout>(&root, "project-checkout", "star.project-checkout");
        assert_fixture::<ProjectV1ToV2MigrationPlan>(
            &root,
            "project-v1-to-v2-migration-plan",
            "star.management.project-v1-to-v2-migration-plan",
        );
        assert_fixture::<ProjectV1ToV2MigrationResult>(
            &root,
            "project-v1-to-v2-migration-result",
            "star.management.project-v1-to-v2-migration-result",
        );
        assert_fixture::<ProjectRevision>(&root, "project-revision", "star.project-revision");
        assert_fixture::<WorkspaceSnapshot>(&root, "workspace-snapshot", "star.workspace-snapshot");
        assert_fixture::<ScanRun>(&root, "scan-run", "star.scan-run");
        assert_fixture::<Rule>(&root, "rule", "star.rule");
        assert_fixture::<Finding>(&root, "finding", "star.finding");
        assert_fixture::<Occurrence>(&root, "occurrence", "star.occurrence");
        assert_fixture::<Symbol>(&root, "symbol", "star.symbol");
        assert_fixture::<SymbolReference>(&root, "symbol-reference", "star.symbol-reference");
        assert_fixture::<CanonicalSource>(&root, "canonical-source", "star.canonical-source");
        assert_fixture::<Suppression>(&root, "suppression", "star.suppression");
        assert_fixture::<Baseline>(&root, "baseline", "star.baseline");
        assert_fixture::<Disposition>(&root, "disposition", "star.disposition");
        assert_fixture::<ChangePlan>(&root, "change-plan", "star.change-plan");
        assert_fixture::<PatchSet>(&root, "patch-set", "star.patch-set");
        assert_fixture::<ChangeRecipe>(&root, "change-recipe", "star.change-recipe");
        assert_fixture::<ValidationResult>(&root, "validation-result", "star.validation-result");
        assert_fixture::<GateDecision>(&root, "gate-decision", "star.gate-decision");
        assert_strict_artifact_fixture(&root);
        assert_fixture::<ManagementStoreStatus>(
            &root,
            "management-store-status",
            "star.management-store-status",
        );
        assert_fixture::<CoordinatedOperation>(
            &root,
            "coordinated-operation",
            "star.coordinated-operation",
        );
    }

    #[test]
    fn management_fingerprint_golden_recomputes_to_the_checked_in_ids() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../specs/fixtures/management/v1/fingerprint-golden.json");
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let fingerprint = crate::canonical_sha256(&serde_json::json!({
            "algorithm":value["algorithm"],
            "contract_version":value["contract_version"],
            "payload":value["payload"],
        }))
        .unwrap();
        assert_eq!(
            value["identity_fingerprint"].as_str(),
            Some(fingerprint.as_str())
        );
        assert_eq!(
            value["derived_ids"]["canonical_source_id"].as_str(),
            Some(CanonicalSourceId::from_fingerprint(&fingerprint).as_str())
        );
        assert_eq!(
            value["derived_ids"]["symbol_id"].as_str(),
            Some(SymbolId::from_fingerprint(&fingerprint).as_str())
        );
        assert_eq!(
            value["derived_ids"]["finding_id"].as_str(),
            Some(FindingId::from_fingerprint(&fingerprint).as_str())
        );
        assert_eq!(
            value["derived_ids"]["occurrence_id"].as_str(),
            Some(OccurrenceId::from_fingerprint(&fingerprint).as_str())
        );
    }
}
