//! Backend-neutral ports for P0 application services.

use std::{collections::BTreeMap, path::Path, sync::Arc};

use serde::{Deserialize, Serialize};

use star_contracts::{
    Sha256Hash,
    evidence::{ArtifactRef, GateDecision},
    ids::{CoordinatedOperationId, FindingId, PatchSetId, ProjectId, RootBindingId, ScanRunId},
    management::{
        Baseline, CanonicalSource, ChangePlan, CoordinatedOperation, Disposition, Finding,
        ManagementStoreStatus, Occurrence, ParticipantReceipt, PatchSet, Project, ProjectRevision,
        ScanRun, Suppression, Symbol, SymbolReference, ValidationResult, WorkspaceSnapshot,
    },
};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRootAttachment {
    pub project_id: ProjectId,
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
    pub idempotency_key: String,
    pub payload_fingerprint: Sha256Hash,
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
        idempotency_key: &str,
        payload_fingerprint: &Sha256Hash,
    ) -> Result<Project, RepositoryError>;
    fn get_project(&self, project_id: &ProjectId) -> Result<Option<Project>, RepositoryError>;
    fn list_projects(&self) -> Result<Vec<Project>, RepositoryError>;
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
    fn attach(&self, project_id: &ProjectId, root: &Path)
    -> Result<RootBindingId, RepositoryError>;
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
