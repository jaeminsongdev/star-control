//! Backend-neutral ports for P0 application services.

use std::{collections::BTreeMap, path::Path, sync::Arc};

use serde::{Deserialize, Serialize};

use star_contracts::{
    Sha256Hash,
    evidence::{ArtifactRef, GateDecision},
    evidence_v2::{DiagnosticV2, EvidenceBundleV2, GateDecisionV2, ValidationRunV2},
    ids::{
        CheckoutId, CodeIndexSnapshotId, CoordinatedOperationId, EvidenceBundleId, FindingId,
        PatchSetId, ProjectId, RootBindingId, ScanRunId, TaskSpecId,
    },
    index::{CodeIndexSnapshot, IndexEdge, IndexEntity, ProjectCatalogSnapshot, SourceEntry},
    management::{
        Baseline, CanonicalSource, ChangePlan, CoordinatedOperation, Disposition, Finding,
        ManagementStoreStatus, Occurrence, ParticipantReceipt, PatchSet, Project, ProjectCheckout,
        ProjectRevision, ScanRun, Suppression, Symbol, SymbolReference, ValidationResult,
        WorkspaceSnapshot,
    },
    planning::PlanningBundle,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionCandidate {
    pub project_id: ProjectId,
    pub generation_id: String,
    pub scan_run_id: ScanRunId,
    pub retention_class: String,
    pub reason_code: String,
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
    fn get_patch_set(&self, patch_set_id: &PatchSetId)
    -> Result<Option<PatchSet>, RepositoryError>;
    fn save_validation(
        &self,
        result: &ValidationResult,
        decision: &GateDecision,
    ) -> Result<(), RepositoryError>;
    fn save_check_graph_evidence(
        &self,
        runs: &[ValidationRunV2],
        diagnostics: &[DiagnosticV2],
        decision: &GateDecisionV2,
        bundle: &EvidenceBundleV2,
    ) -> Result<(), RepositoryError>;
    fn get_evidence_bundle_v2(
        &self,
        evidence_bundle_id: &EvidenceBundleId,
    ) -> Result<Option<EvidenceBundleV2>, RepositoryError>;
    fn artifact_refs_for_scan(
        &self,
        scan_run_id: &ScanRunId,
    ) -> Result<Vec<ArtifactRef>, RepositoryError>;
}

pub trait ManagementRepositorySet: Send + Sync {
    fn global(&self) -> &dyn GlobalManagementRepository;
    fn project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Arc<dyn ProjectManagementRepository>, RepositoryError>;
    fn verify_all(&self) -> Result<Vec<ManagementStoreStatus>, RepositoryError>;
    fn backup_all(&self, destination: &Path)
    -> Result<Vec<ManagementStoreStatus>, RepositoryError>;
    fn plan_retention(&self) -> Result<RetentionPlan, RepositoryError>;
    fn apply_retention(
        &self,
        plan: &RetentionPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<RetentionApplyResult, RepositoryError>;
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
    ) -> Result<ArtifactRef, RepositoryError>;
    fn verify(&self, project_root: &Path, artifact: &ArtifactRef) -> Result<(), RepositoryError>;
    fn read_json(
        &self,
        project_root: &Path,
        artifact: &ArtifactRef,
    ) -> Result<serde_json::Value, RepositoryError>;
}
