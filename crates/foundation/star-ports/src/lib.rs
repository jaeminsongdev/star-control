//! Backend-neutral ports for P0 application services.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};

use star_contracts::{
    Sha256Hash,
    evidence::{
        ArtifactKind, ArtifactRef, CatalogRef, Completeness, GateDecision, OutputLimits,
        RedactionStatus, RetentionClass, TerminationReason,
    },
    evidence_v2::{
        BaselineV2, DiagnosticV2, DispositionV2, EvidenceBundleV2, GateDecisionV2, ReviewPackV1,
        ReworkDirectiveV1, SuppressionV2, ValidationResultV2, ValidationRunV2,
    },
    ids::{
        CheckoutId, CodeIndexSnapshotId, CoordinatedOperationId, DiagnosticId, EvidenceBundleId,
        FindingId, GateId, PatchSetId, ProjectId, ProjectRevisionId, ReviewPackId, RootBindingId,
        ScanRunId, SuppressionId, TaskSpecId, ValidationResultId, ValidationRunId,
        WorkspaceSnapshotId,
    },
    index::{CodeIndexSnapshot, IndexEdge, IndexEntity, ProjectCatalogSnapshot, SourceEntry},
    maintenance_v2::{
        ExternalDataSnapshot, GitHistoryRiskSnapshot, MutationTestingBudget,
        MutationTestingSnapshot, MutationTrigger, QualityRulePackManifest,
        StaticAnalysisImportReport,
    },
    managed_registry::{
        ManagedDeclarationChangeIntent, ManagedRegistrySnapshot, RegistryConsistencyRecord,
    },
    management::{
        Baseline, CanonicalSource, ChangePlan, CoordinatedOperation, Disposition, Finding,
        ManagementStoreStatus, Occurrence, ParticipantReceipt, PatchSet, Project, ProjectCheckout,
        ProjectPathRef, ProjectRevision, ScanRun, Suppression, Symbol, SymbolReference,
        ValidationResult, WorkspaceSnapshot,
    },
    patch_v2::{ChangeRecipeV2, PatchSetV2, TargetSelector, WorktreeDecision},
    planning::PlanningBundle,
    recovery::{
        ActiveSetManifest, BackupApplyResult, BackupPlan, LocalStateBundle, LocalStateExportPlan,
        LocalStateExportResult, LocalStateImportPlan, LocalStateImportResult, RebuildApplyResult,
        RebuildPlan, RebuildProjectInput, RebuiltProjectSummary, RecoveryLossItem, RecoveryStatus,
        RestoreApplyResult, RestorePlan,
    },
};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRootAttachment {
    pub project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub root_binding_id: RootBindingId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepositoryErrorCategory {
    Unavailable,
    Busy,
    RevisionConflict,
    IdempotencyConflict,
    MigrationRequired,
    IncompatibleVersion,
    IntegrityFailed,
    ReadOnly,
    QuotaExceeded,
    Corrupt,
    NotFound,
    Invalid,
}

#[derive(Debug, Error)]
#[error("repository {category:?}: {message}")]
pub struct RepositoryError {
    pub category: RepositoryErrorCategory,
    pub message: &'static str,
}

impl RepositoryError {
    pub fn new(category: RepositoryErrorCategory, message: &'static str) -> Self {
        Self { category, message }
    }
}

#[derive(Clone, Debug)]
pub struct ScanCommit {
    pub project: Project,
    pub revision: ProjectRevision,
    pub snapshot: WorkspaceSnapshot,
    pub run: ScanRun,
    pub sources: Vec<CanonicalSource>,
    pub symbols: Vec<Symbol>,
    pub references: Vec<SymbolReference>,
    pub findings: Vec<Finding>,
    pub occurrences: Vec<Occurrence>,
    pub code_index: Option<CodeIndexSnapshot>,
    pub source_entries: Vec<SourceEntry>,
    pub index_entities: Vec<IndexEntity>,
    pub index_edges: Vec<IndexEdge>,
    pub idempotency_key: String,
    pub payload_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredCodeIndexProjection {
    pub snapshot: CodeIndexSnapshot,
    pub source_entries: Vec<SourceEntry>,
    pub entities: Vec<IndexEntity>,
    pub edges: Vec<IndexEdge>,
    pub symbols: Vec<Symbol>,
    pub references: Vec<SymbolReference>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactDiscovery {
    pub verified: Vec<ArtifactRef>,
    pub rejected_count: u64,
}

pub trait CodeIndexCache: Send + Sync {
    fn load(
        &self,
        project_id: &ProjectId,
        cache_key: &Sha256Hash,
    ) -> Result<Option<StoredCodeIndexProjection>, RepositoryError>;
    fn store(
        &self,
        project_id: &ProjectId,
        cache_key: &Sha256Hash,
        projection: &StoredCodeIndexProjection,
    ) -> Result<(), RepositoryError>;
}

/// Bounded, read-only input for Git-derived maintenance observations.  The
/// caller supplies only revisions and a commit cap; adapters must never alter
/// the checked-out repository while collecting the snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitHistoryObservationRequest {
    pub project_id: ProjectId,
    pub range_start: Option<String>,
    pub range_end: String,
    pub commit_limit: u32,
    pub evaluation_time: String,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum GitHistoryPortError {
    #[error("Git history input is invalid")]
    Invalid,
    #[error("Git history adapter is unavailable")]
    Unavailable,
    #[error("Git history observation is incomplete or unverified")]
    Unverified,
    #[error("Git history output could not be parsed safely")]
    Malformed,
}

/// Adapter boundary for Git history and source debt observations.  The output
/// contains only project-relative paths and opaque owner buckets.
pub trait GitHistoryPort: Send + Sync {
    fn observe(
        &self,
        project_root: &Path,
        request: &GitHistoryObservationRequest,
    ) -> Result<GitHistoryRiskSnapshot, GitHistoryPortError>;
}

/// Bounded, changed-code-only request for a registered mutation engine. The
/// engine may not widen the requested paths or replace line coverage evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutationTestingObservationRequest {
    pub snapshot_id: String,
    pub project_id: ProjectId,
    pub project_revision_ref: String,
    pub workspace_snapshot_ref: String,
    pub code_index_snapshot_ref: String,
    pub changed_paths: Vec<ProjectPathRef>,
    pub triggers: Vec<MutationTrigger>,
    pub budget: MutationTestingBudget,
    pub engine_descriptor_ref: String,
    pub expected_engine_identity: Sha256Hash,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum MutationTestingPortError {
    #[error("mutation testing input is invalid")]
    Invalid,
    #[error("mutation engine is unavailable")]
    Unavailable,
    #[error("mutation result is partial, flaky, or unverified")]
    Unverified,
    #[error("mutation engine output is malformed")]
    Malformed,
}

/// Registered external mutation-engine boundary. It is observation-only: the
/// engine returns a normalized snapshot and never receives a source mutation
/// capability.
pub trait MutationTestingPort: Send + Sync {
    fn observe(
        &self,
        project_root: &Path,
        request: &MutationTestingObservationRequest,
    ) -> Result<MutationTestingSnapshot, MutationTestingPortError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QualityRulePackLoadRequest {
    pub pack_id: String,
    pub pack_version: String,
    pub expected_source_digest: Sha256Hash,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum QualityRulePackPortError {
    #[error("Rule Pack input is invalid")]
    Invalid,
    #[error("Rule Pack provider is unavailable")]
    Unavailable,
    #[error("Rule Pack trust or freshness is unverified")]
    Unverified,
    #[error("Rule Pack output is malformed")]
    Malformed,
}

/// Registered Rule Pack provider boundary. Query execution remains with the
/// declared external analyzer; Star-Control only accepts digest-bound metadata.
pub trait QualityRulePackPort: Send + Sync {
    fn load(
        &self,
        request: &QualityRulePackLoadRequest,
    ) -> Result<QualityRulePackManifest, QualityRulePackPortError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepositoryPostureObservationRequest {
    pub project_id: ProjectId,
    pub project_revision_ref: String,
    pub source_url: String,
    pub query: String,
    pub source_schema_version: String,
    pub expected_tool_identity: Sha256Hash,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum RepositoryPosturePortError {
    #[error("repository posture input is invalid")]
    Invalid,
    #[error("repository posture provider is unavailable")]
    Unavailable,
    #[error("repository posture snapshot is stale or unverified")]
    Unverified,
    #[error("repository posture output is malformed")]
    Malformed,
}

/// Read-only external repository posture boundary (for example Scorecard).
/// Aggregate scores remain advisory data and are never a Gate result here.
pub trait RepositoryPosturePort: Send + Sync {
    fn observe(
        &self,
        request: &RepositoryPostureObservationRequest,
    ) -> Result<ExternalDataSnapshot, RepositoryPosturePortError>;
}

#[derive(Clone, Debug)]
pub struct ManagedRegistryConsumerProjectInput {
    pub project_id: ProjectId,
    pub project_root: PathBuf,
    pub source_entries: Vec<SourceEntry>,
    pub index_current: bool,
    pub coverage_complete: bool,
}

#[derive(Clone, Debug)]
pub struct ManagedRegistryResolveRequest {
    pub project_root: PathBuf,
    pub manifest_path: ProjectPathRef,
    pub owner_project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub project_revision_id: ProjectRevisionId,
    pub workspace_snapshot_id: WorkspaceSnapshotId,
    pub code_index_snapshot_id: CodeIndexSnapshotId,
    pub index_current: bool,
    pub coverage_complete: bool,
    pub consumer_projects: Vec<ManagedRegistryConsumerProjectInput>,
}

#[derive(Clone, Debug)]
pub struct ManagedRegistryResolveResult {
    pub snapshot: ManagedRegistrySnapshot,
    pub consistency_records: Vec<RegistryConsistencyRecord>,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ManagedRegistryResolverError {
    #[error("managed registry input is invalid")]
    Invalid,
    #[error("managed registry evidence is stale, partial, or unverified")]
    Unverified,
    #[error("managed registry identity or lifecycle conflicts")]
    Conflict,
    #[error("managed registry resolution is blocked by policy")]
    Blocked,
    #[error("managed registry adapter operation failed")]
    Adapter,
    #[error("managed registry fingerprint calculation failed")]
    Fingerprint,
}

pub trait ManagedRegistryResolverPort: Send + Sync {
    fn resolve(
        &self,
        request: ManagedRegistryResolveRequest,
    ) -> Result<ManagedRegistryResolveResult, ManagedRegistryResolverError>;
}

#[derive(Clone, Debug)]
pub struct ManagedRegistryRewriteRequest {
    pub project_root: PathBuf,
    pub snapshot: ManagedRegistrySnapshot,
    pub intent: ManagedDeclarationChangeIntent,
}

#[derive(Clone, Debug)]
pub struct ManagedRegistryRewriteResult {
    pub files: Vec<MaterializedRewrite>,
    pub replay_operation_count: u64,
    pub idempotence_proved: bool,
}

pub trait ManagedRegistryRewritePort: Send + Sync {
    fn rewrite(
        &self,
        request: ManagedRegistryRewriteRequest,
    ) -> Result<ManagedRegistryRewriteResult, ManagedRegistryResolverError>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionCandidate {
    pub project_id: ProjectId,
    pub generation_id: String,
    pub scan_run_id: ScanRunId,
    pub retention_class: String,
    pub reason_code: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub keep_latest_successful_scans: usize,
    pub incomplete_staging_retention_days: u64,
    pub scan_detail_retention_days: u64,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            keep_latest_successful_scans: 2,
            incomplete_staging_retention_days: 7,
            scan_detail_retention_days: 90,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionPlan {
    pub schema_version: u32,
    pub created_at: String,
    pub expected_store_revisions: BTreeMap<String, u64>,
    pub candidates: Vec<RetentionCandidate>,
    pub plan_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionApplyResult {
    pub applied_count: usize,
    pub plan_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentRecord {
    pub schema_version: u32,
    pub record_kind: String,
    pub record_id: String,
    pub revision: u64,
    pub project_id: Option<ProjectId>,
    pub state: String,
    pub document_schema_id: String,
    pub document_schema_version: u32,
    pub document_fingerprint: Sha256Hash,
    pub document: serde_json::Value,
    pub created_at: String,
}

pub trait GlobalManagementRepository: Send + Sync {
    fn status(&self) -> Result<ManagementStoreStatus, RepositoryError>;
    fn register_project(
        &self,
        project: &Project,
        checkout: &ProjectCheckout,
        idempotency_key: &str,
        payload_fingerprint: &Sha256Hash,
    ) -> Result<Project, RepositoryError>;
    fn get_project(&self, project_id: &ProjectId) -> Result<Option<Project>, RepositoryError>;
    fn list_projects(&self) -> Result<Vec<Project>, RepositoryError>;
    fn get_project_checkout(
        &self,
        checkout_id: &CheckoutId,
    ) -> Result<Option<ProjectCheckout>, RepositoryError>;
    fn list_project_checkouts(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<ProjectCheckout>, RepositoryError>;
    fn put_project_catalog_snapshot(
        &self,
        snapshot: &ProjectCatalogSnapshot,
    ) -> Result<(), RepositoryError>;
    fn latest_project_catalog_snapshot(
        &self,
    ) -> Result<Option<ProjectCatalogSnapshot>, RepositoryError>;
    fn put_planning_bundle(
        &self,
        bundle: &PlanningBundle,
        idempotency_key: &str,
        input_fingerprint: &Sha256Hash,
    ) -> Result<PlanningBundle, RepositoryError>;
    fn get_planning_bundle(
        &self,
        task_spec_id: &TaskSpecId,
    ) -> Result<Option<PlanningBundle>, RepositoryError>;
    fn get_planning_bundle_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<(PlanningBundle, Sha256Hash)>, RepositoryError>;
    fn list_planning_bundle_revisions(
        &self,
        task_spec_id: &TaskSpecId,
    ) -> Result<Vec<PlanningBundle>, RepositoryError>;
    fn put_coordination(&self, operation: &CoordinatedOperation) -> Result<(), RepositoryError>;
    fn get_coordination(
        &self,
        operation_id: &CoordinatedOperationId,
    ) -> Result<Option<CoordinatedOperation>, RepositoryError>;
    fn get_coordination_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<CoordinatedOperation>, RepositoryError>;
    fn list_incomplete_coordination(&self) -> Result<Vec<CoordinatedOperation>, RepositoryError>;
    fn put_development_record(&self, record: &DevelopmentRecord) -> Result<(), RepositoryError>;
    fn get_development_record(
        &self,
        record_kind: &str,
        record_id: &str,
        revision: Option<u64>,
    ) -> Result<Option<DevelopmentRecord>, RepositoryError>;
    fn list_development_records(
        &self,
        record_kind: &str,
        project_id: Option<&ProjectId>,
    ) -> Result<Vec<DevelopmentRecord>, RepositoryError>;
}

pub struct CheckGraphEvidenceTransaction<'a> {
    pub runs: &'a [ValidationRunV2],
    pub results: &'a [ValidationResultV2],
    pub diagnostics: &'a [DiagnosticV2],
    pub decision: &'a GateDecisionV2,
    pub bundle: &'a EvidenceBundleV2,
    pub review_pack: &'a ReviewPackV1,
    pub rework_directive: Option<&'a ReworkDirectiveV1>,
    pub source_bound_findings: Option<SourceBoundFindingImport<'a>>,
}

/// Additional findings attached to the current successful source generation.
/// This is deliberately not a second scanner database: the repository checks
/// the exact scan, revision, workspace, and CodeIndex generation before the
/// M3 evidence and projections are committed together.
pub struct SourceBoundFindingImport<'a> {
    pub scan_run_id: &'a ScanRunId,
    pub project_revision_id: &'a ProjectRevisionId,
    pub workspace_snapshot_id: &'a star_contracts::ids::WorkspaceSnapshotId,
    pub code_index_snapshot_id: &'a CodeIndexSnapshotId,
    pub findings: &'a [Finding],
    pub occurrences: &'a [Occurrence],
    pub reports: &'a [StaticAnalysisImportReport],
}

pub trait ProjectManagementRepository: Send + Sync {
    fn status(&self) -> Result<ManagementStoreStatus, RepositoryError>;
    fn commit_registration_participant(
        &self,
        project: &Project,
        operation_id: &CoordinatedOperationId,
        payload_fingerprint: &Sha256Hash,
        result_fingerprint: &Sha256Hash,
    ) -> Result<ParticipantReceipt, RepositoryError>;
    fn get_project(&self) -> Result<Option<Project>, RepositoryError>;
    fn replay_scan(
        &self,
        idempotency_key: &str,
        payload_fingerprint: &Sha256Hash,
    ) -> Result<Option<ScanRun>, RepositoryError>;
    fn commit_scan(&self, commit: &ScanCommit) -> Result<ScanRun, RepositoryError>;
    fn latest_scan(&self) -> Result<Option<ScanRun>, RepositoryError>;
    fn latest_code_index_projection(
        &self,
    ) -> Result<Option<StoredCodeIndexProjection>, RepositoryError>;
    fn get_code_index_snapshot(
        &self,
        snapshot_id: &CodeIndexSnapshotId,
    ) -> Result<Option<CodeIndexSnapshot>, RepositoryError>;
    fn get_workspace_snapshot(
        &self,
        workspace_snapshot_id: &star_contracts::ids::WorkspaceSnapshotId,
    ) -> Result<Option<WorkspaceSnapshot>, RepositoryError>;
    fn list_findings(&self) -> Result<Vec<Finding>, RepositoryError>;
    fn get_finding(&self, finding_id: &FindingId) -> Result<Option<Finding>, RepositoryError>;
    fn occurrences_for_finding(
        &self,
        finding_id: &FindingId,
    ) -> Result<Vec<Occurrence>, RepositoryError>;
    fn put_suppression(
        &self,
        suppression: &Suppression,
        expected_revision: u64,
    ) -> Result<(), RepositoryError>;
    fn sync_shared_decisions(
        &self,
        baselines: &[Baseline],
        suppressions: &[Suppression],
        source_fingerprint: &Sha256Hash,
    ) -> Result<(), RepositoryError>;
    fn list_suppressions(&self) -> Result<Vec<Suppression>, RepositoryError>;
    fn put_baseline(
        &self,
        baseline: &Baseline,
        expected_revision: u64,
    ) -> Result<(), RepositoryError>;
    fn list_baselines(&self) -> Result<Vec<Baseline>, RepositoryError>;
    fn put_disposition(
        &self,
        disposition: &Disposition,
        expected_revision: u64,
    ) -> Result<(), RepositoryError>;
    fn list_dispositions(&self) -> Result<Vec<Disposition>, RepositoryError>;
    fn save_patch_set(&self, patch_set: &PatchSet) -> Result<(), RepositoryError>;
    fn save_change_plan(&self, change_plan: &ChangePlan) -> Result<(), RepositoryError>;
    fn list_change_plans(&self) -> Result<Vec<ChangePlan>, RepositoryError>;
    fn import_local_state(
        &self,
        bundle: &LocalStateBundle,
        expected_store_revision: u64,
    ) -> Result<(), RepositoryError>;
    fn get_patch_set(&self, patch_set_id: &PatchSetId)
    -> Result<Option<PatchSet>, RepositoryError>;
    fn save_validation(
        &self,
        result: &ValidationResult,
        decision: &GateDecision,
    ) -> Result<(), RepositoryError>;
    fn save_check_graph_evidence(
        &self,
        evidence: CheckGraphEvidenceTransaction<'_>,
    ) -> Result<(), RepositoryError>;
    fn get_validation_run_v2(
        &self,
        validation_run_id: &ValidationRunId,
    ) -> Result<Option<ValidationRunV2>, RepositoryError>;
    fn list_validation_runs_v2(&self) -> Result<Vec<ValidationRunV2>, RepositoryError>;
    fn get_validation_result_v2(
        &self,
        validation_result_id: &ValidationResultId,
    ) -> Result<Option<ValidationResultV2>, RepositoryError>;
    fn list_validation_results_v2(&self) -> Result<Vec<ValidationResultV2>, RepositoryError>;
    fn get_diagnostic_v2(
        &self,
        diagnostic_id: &DiagnosticId,
    ) -> Result<Option<DiagnosticV2>, RepositoryError>;
    fn list_diagnostics_v2(&self) -> Result<Vec<DiagnosticV2>, RepositoryError>;
    fn list_static_analysis_import_reports(
        &self,
    ) -> Result<Vec<StaticAnalysisImportReport>, RepositoryError>;
    fn get_gate_decision_v2(
        &self,
        gate_id: &GateId,
    ) -> Result<Option<GateDecisionV2>, RepositoryError>;
    fn list_gate_decisions_v2(&self) -> Result<Vec<GateDecisionV2>, RepositoryError>;
    fn get_evidence_bundle_v2(
        &self,
        evidence_bundle_id: &EvidenceBundleId,
    ) -> Result<Option<EvidenceBundleV2>, RepositoryError>;
    fn list_evidence_bundles_v2(&self) -> Result<Vec<EvidenceBundleV2>, RepositoryError>;
    fn get_review_pack_v1(
        &self,
        review_pack_id: &ReviewPackId,
    ) -> Result<Option<ReviewPackV1>, RepositoryError>;
    fn list_review_packs_v1(&self) -> Result<Vec<ReviewPackV1>, RepositoryError>;
    fn put_baseline_v2(&self, baseline: &BaselineV2) -> Result<(), RepositoryError>;
    fn list_baselines_v2(&self) -> Result<Vec<BaselineV2>, RepositoryError>;
    fn put_suppression_v2(&self, suppression: &SuppressionV2) -> Result<(), RepositoryError>;
    fn get_suppression_v2(
        &self,
        suppression_id: &SuppressionId,
        revision: u64,
    ) -> Result<Option<SuppressionV2>, RepositoryError>;
    fn list_suppressions_v2(&self) -> Result<Vec<SuppressionV2>, RepositoryError>;
    fn put_disposition_v2(&self, disposition: &DispositionV2) -> Result<(), RepositoryError>;
    fn list_dispositions_v2(&self) -> Result<Vec<DispositionV2>, RepositoryError>;
    fn save_managed_registry_resolution(
        &self,
        snapshot: &ManagedRegistrySnapshot,
        consistency_records: &[RegistryConsistencyRecord],
    ) -> Result<(), RepositoryError>;
    fn latest_managed_registry_snapshot(
        &self,
    ) -> Result<Option<ManagedRegistrySnapshot>, RepositoryError>;
    fn get_managed_registry_snapshot(
        &self,
        snapshot_id: &star_contracts::ManagedRegistrySnapshotId,
    ) -> Result<Option<ManagedRegistrySnapshot>, RepositoryError>;
    fn list_registry_consistency_records(
        &self,
        snapshot_id: &star_contracts::ManagedRegistrySnapshotId,
    ) -> Result<Vec<RegistryConsistencyRecord>, RepositoryError>;
    fn artifact_refs_for_scan(
        &self,
        scan_run_id: &ScanRunId,
    ) -> Result<Vec<ArtifactRef>, RepositoryError>;
    fn reindex_artifact_refs(&self, artifact_refs: &[ArtifactRef]) -> Result<(), RepositoryError>;
    fn list_artifact_refs(&self) -> Result<Vec<ArtifactRef>, RepositoryError>;
}

pub trait ManagementRepositorySet: Send + Sync {
    fn global(&self) -> &dyn GlobalManagementRepository;
    fn project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Arc<dyn ProjectManagementRepository>, RepositoryError>;
    fn active_set(&self) -> Result<ActiveSetManifest, RepositoryError>;
    fn verify_all(&self) -> Result<Vec<ManagementStoreStatus>, RepositoryError>;
    fn plan_backup(&self, destination: &Path) -> Result<BackupPlan, RepositoryError>;
    fn apply_backup(
        &self,
        destination: &Path,
        plan: &BackupPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<BackupApplyResult, RepositoryError>;
    fn plan_local_state_export(
        &self,
        project_id: &ProjectId,
        destination: &Path,
    ) -> Result<LocalStateExportPlan, RepositoryError>;
    fn apply_local_state_export(
        &self,
        destination: &Path,
        plan: &LocalStateExportPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<LocalStateExportResult, RepositoryError>;
    fn plan_local_state_import(
        &self,
        source: &Path,
    ) -> Result<LocalStateImportPlan, RepositoryError>;
    fn apply_local_state_import(
        &self,
        source: &Path,
        plan: &LocalStateImportPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<LocalStateImportResult, RepositoryError>;
    fn plan_retention(&self) -> Result<RetentionPlan, RepositoryError>;
    fn plan_retention_with_policy(
        &self,
        _policy: &RetentionPolicy,
    ) -> Result<RetentionPlan, RepositoryError> {
        self.plan_retention()
    }
    fn apply_retention(
        &self,
        plan: &RetentionPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<RetentionApplyResult, RepositoryError>;
}

pub trait ManagementRecovery: Send + Sync {
    fn status(&self) -> Result<RecoveryStatus, RepositoryError>;
    fn plan_restore(&self, backup_root: &Path) -> Result<RestorePlan, RepositoryError>;
    fn apply_restore(
        &self,
        backup_root: &Path,
        plan: &RestorePlan,
        approved_plan_fingerprint: &str,
    ) -> Result<RestoreApplyResult, RepositoryError>;
    fn plan_rebuild(
        &self,
        projects: Vec<RebuildProjectInput>,
        predicted_losses: Vec<RecoveryLossItem>,
    ) -> Result<RebuildPlan, RepositoryError>;
    fn completed_rebuild(
        &self,
        plan: &RebuildPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<Option<RebuildApplyResult>, RepositoryError>;
    fn begin_rebuild(
        &self,
        plan: &RebuildPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<Arc<dyn ManagementRepositorySet>, RepositoryError>;
    fn apply_rebuild(
        &self,
        plan: &RebuildPlan,
        approved_plan_fingerprint: &str,
        rebuilt_projects: Vec<RebuiltProjectSummary>,
    ) -> Result<RebuildApplyResult, RepositoryError>;
    fn plan_local_state_export(
        &self,
        project_id: &ProjectId,
        destination: &Path,
    ) -> Result<LocalStateExportPlan, RepositoryError>;
    fn apply_local_state_export(
        &self,
        destination: &Path,
        plan: &LocalStateExportPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<LocalStateExportResult, RepositoryError>;
}

pub trait ProjectRootBindingStore: Send + Sync {
    fn list_attachments(&self) -> Result<Vec<ProjectRootAttachment>, RepositoryError>;
    fn find_by_root(&self, root: &Path) -> Result<Option<ProjectRootAttachment>, RepositoryError>;
    fn find_by_project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Option<ProjectRootAttachment>, RepositoryError>;
    fn find_by_checkout(
        &self,
        checkout_id: &CheckoutId,
    ) -> Result<Option<ProjectRootAttachment>, RepositoryError>;
    fn attach(
        &self,
        project_id: &ProjectId,
        checkout_id: &CheckoutId,
        root: &Path,
    ) -> Result<RootBindingId, RepositoryError>;
    fn resolve(&self, binding_id: &RootBindingId) -> Result<std::path::PathBuf, RepositoryError>;
}

pub trait ArtifactStore: Send + Sync {
    fn put_json(
        &self,
        project_id: &ProjectId,
        project_root: &Path,
        relative_path: &str,
        subject_kind: &str,
        subject_id: &str,
        value: &serde_json::Value,
    ) -> Result<ArtifactRef, RepositoryError> {
        self.put_json_with_policy(ArtifactWriteRequest {
            project_id,
            project_root,
            relative_path,
            subject_kind,
            subject_id,
            policy: ArtifactWritePolicy::default(),
            value,
        })
    }
    fn put_json_with_policy(
        &self,
        request: ArtifactWriteRequest<'_>,
    ) -> Result<ArtifactRef, RepositoryError>;
    fn verify(&self, project_root: &Path, artifact: &ArtifactRef) -> Result<(), RepositoryError>;
    fn read_json(
        &self,
        project_root: &Path,
        artifact: &ArtifactRef,
    ) -> Result<serde_json::Value, RepositoryError>;
    fn discover_verified(
        &self,
        project_id: &ProjectId,
        project_root: &Path,
    ) -> Result<ArtifactDiscovery, RepositoryError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtifactWritePolicy {
    pub kind: ArtifactKind,
    pub redaction_status: RedactionStatus,
    pub retention_class: RetentionClass,
}

pub struct ArtifactWriteRequest<'a> {
    pub project_id: &'a ProjectId,
    pub project_root: &'a Path,
    pub relative_path: &'a str,
    pub subject_kind: &'a str,
    pub subject_id: &'a str,
    pub policy: ArtifactWritePolicy,
    pub value: &'a serde_json::Value,
}

impl Default for ArtifactWritePolicy {
    fn default() -> Self {
        Self {
            kind: ArtifactKind::Report,
            redaction_status: RedactionStatus::NotNeeded,
            retention_class: RetentionClass::Evidence,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RewriteTransformRequest {
    pub recipe: ChangeRecipeV2,
    pub target_selector: TargetSelector,
    pub parameters: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterializedRewrite {
    pub path: star_contracts::management::ProjectPathRef,
    pub before_sha256: Sha256Hash,
    pub after_sha256: Sha256Hash,
    pub before_bytes: Vec<u8>,
    pub after_bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RewriteTransformResult {
    pub files: Vec<MaterializedRewrite>,
    pub replay_operation_count: usize,
    pub idempotence_proved: bool,
}

/// Materializes a bounded recipe against a read-only checkout view. The port
/// must not mutate `project_root`.
pub trait RewriteTransformerPort: Send + Sync {
    fn materialize(
        &self,
        project_root: &Path,
        request: &RewriteTransformRequest,
    ) -> Result<RewriteTransformResult, PatchPortError>;
}

/// The operation claimed by a registered semantic refactor provider.  Provider
/// output is still only a preview; it must be normalized through the existing
/// PatchSetV2 and Gate path before any source mutation is possible.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticRefactorOperation {
    Rename,
    CodeAction,
    AnalyzerFix,
    OpenRewriteRecipe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticProviderAvailability {
    Available,
    Unavailable,
    Unverified,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticProviderCapability {
    pub provider_id: String,
    pub executable_identity: Option<Sha256Hash>,
    pub operation: SemanticRefactorOperation,
    pub availability: SemanticProviderAvailability,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SemanticRefactorPreviewRequest {
    pub capability: SemanticProviderCapability,
    pub transform: RewriteTransformRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticRefactorPreviewResult {
    pub capability: SemanticProviderCapability,
    pub files: Vec<MaterializedRewrite>,
    pub replay_operation_count: usize,
    pub idempotence_proved: bool,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SemanticRefactorProviderError {
    #[error("semantic refactor input is invalid")]
    Invalid,
    #[error("semantic refactor provider is unavailable")]
    Unavailable,
    #[error("semantic refactor provider is unverified or partial")]
    Unverified,
    #[error("semantic refactor preview is malformed")]
    Malformed,
}

/// A registered provider may materialize an isolated preview only. It has no
/// source-mutation method and is deliberately distinct from ToolExecutorPort.
pub trait SemanticRefactorProviderPort: Send + Sync {
    fn preview(
        &self,
        isolated_project_root: &Path,
        request: &SemanticRefactorPreviewRequest,
    ) -> Result<SemanticRefactorPreviewResult, SemanticRefactorProviderError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceMutationRequest {
    pub patch_set: PatchSetV2,
    pub files: Vec<MaterializedRewrite>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceMutationState {
    AppliedExact,
    PartiallyApplied,
    OutcomeUnknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceMutationObservation {
    pub path: star_contracts::management::ProjectPathRef,
    pub observed_sha256: Option<Sha256Hash>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceMutationResult {
    pub state: SourceMutationState,
    pub observations: Vec<SourceMutationObservation>,
}

/// The concrete implementation chooses a non-serializable permit type. This
/// prevents the backend-neutral port crate from depending on a validation
/// implementation while still forcing every source write to consume proof.
pub trait SourceMutationPort: Send + Sync {
    type Permit;

    fn apply(
        &self,
        project_root: &Path,
        request: &SourceMutationRequest,
        permit: Self::Permit,
    ) -> Result<SourceMutationResult, PatchPortError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorktreeMaterialization {
    pub root: std::path::PathBuf,
    pub locator_fingerprint: Sha256Hash,
    pub evidence_refs: Vec<ArtifactRef>,
}

pub trait WorktreePort: Send + Sync {
    fn materialize(
        &self,
        repository_root: &Path,
        decision: &WorktreeDecision,
    ) -> Result<WorktreeMaterialization, PatchPortError>;

    fn discard(
        &self,
        repository_root: &Path,
        materialization: &WorktreeMaterialization,
    ) -> Result<(), PatchPortError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolExecutionRequest {
    pub tool_ref: CatalogRef,
    pub logical_executable: String,
    pub executable_binding_fingerprint: Sha256Hash,
    pub args: Vec<String>,
    pub working_directory: std::path::PathBuf,
    pub timeout_ms: u64,
    pub permission_action: String,
    pub expected_exit_codes: BTreeSet<i32>,
    pub output_limits: OutputLimits,
    pub input_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolExecutionResult {
    pub exit_code: Option<i32>,
    pub termination_reason: TerminationReason,
    pub completeness: Completeness,
    pub success: bool,
    pub output_artifact_refs: Vec<ArtifactRef>,
    pub observed_executable_fingerprint: Sha256Hash,
}

pub trait ToolExecutorPort: Send + Sync {
    fn execute(
        &self,
        request: &ToolExecutionRequest,
    ) -> Result<ToolExecutionResult, PatchPortError>;
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PatchPortError {
    #[error("patch port input is invalid or stale")]
    Invalid,
    #[error("patch port target is unsafe")]
    Unsafe,
    #[error("patch port is unavailable")]
    Unavailable,
    #[error("patch port operation was partial")]
    Partial,
    #[error("patch port outcome is unknown")]
    OutcomeUnknown,
}
