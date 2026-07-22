//! Shared CLI and future Codex management application service.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use star_contracts::evidence::GateDecisionKind;
use star_contracts::{
    Sha256Hash,
    evidence::{ActorRef, AuthoritativeGateState, GateDecision},
    ids::{
        CoordinatedOperationId, FindingId, GenerationId, PatchSetId, ProjectId, ScanRunId,
        TaskSpecId,
    },
    index::{
        CodeIndexSnapshot, IndexEdge, IndexEntity, IndexFreshnessState, IndexPartitionKind,
        IndexPartitionState, IndexTier, ProjectCatalogSnapshot,
    },
    management::{
        Baseline, CoordinatedOperation, CoordinationParticipant, CoordinationState, Disposition,
        Finding, ManagementStoreStatus, ParticipantState, PatchSet, Project, ProjectCheckout,
        ProjectPathRef, ProjectStorePoint, ScanRun, ScanStatus, StorePoint, StoreVersionVector,
        Suppression, SymbolReference, ValidationResult,
    },
    parse_no_duplicate_keys,
    planning::{CheckDescriptor, CollectionState, PlanningBundle},
    rust_style::{RustAutoPolicy, RustCompleteness},
};
use star_domain::versioned_fingerprint;
use star_execution::rust_style::{
    RustStylePatchBinding, RustStylePatchScope, apply_rust_style_patch,
    is_rust_style_patch_artifact, rust_style_patch_binding,
};
use star_execution::{
    ApplyFailure, ExecutionError, apply_patch, prepare_trailing_whitespace_patch, rollback_applied,
};
use star_planning::{
    ObservedWorkspaceChange, PlanningError, PlanningPolicy, PlanningProjectIndex, PlanningRequest,
    TaskSpecDraft, build_planning_bundle, builtin_risk_descriptors,
};
use star_ports::{
    ArtifactStore, CodeIndexCache, GlobalManagementRepository, ManagementRepositorySet,
    ProjectRootBindingStore, RepositoryError, RepositoryErrorCategory, RetentionApplyResult,
    RetentionPlan, ScanCommit, StoredCodeIndexProjection,
};
use star_project::{
    ProjectError, ProjectSeed, ScanPolicy, SharedDecisionDeclarations,
    catalog_snapshot::{CatalogSnapshotInput, DiscoveryConfig, build_project_catalog_snapshot},
    index::{
        CodeIndexBuildRequest, CodeIndexProjection, IndexPolicy, SemanticAdapter, SyntaxAdapter,
        build_code_index,
    },
    load_shared_decisions, observe_project, observe_workspace_changes,
};
pub use star_validation::planning::{
    AiEvidenceSummary, AiValidationRunSummary, CacheMissReason, CacheReuseDecision,
    CacheValidationStability, EvidenceCompressionError, UnitDependency, ValidationCacheCandidate,
    ValidationCheckDefinition, ValidationEvidenceDiagnostic, ValidationEvidenceRun,
    ValidationPlanningError, ValidationPlanningInput, build_validation_plan,
    compress_evidence_for_ai, evaluate_cache_reuse,
};
use star_validation::runner::{
    CheckExecutor, CheckGraphRunContext, CheckGraphRunResult, CheckGraphRunnerError,
    ExecutableBinding, run_check_graph,
};
use star_validation::{
    ValidationError, analyze_trailing_whitespace, apply_decision_projection, evaluate_decisions,
    validate_patch_result_with_plan,
};
use thiserror::Error;

pub mod rust_style;
pub mod rust_style_runtime;

use rust_style_runtime::{
    RustStyleCheckResult, RustStyleInspection, RustStyleRuntimeError, RustStyleScope,
    check_rust_style, inspect_rust_style, prepare_rust_style,
};

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("management input is invalid")]
    Invalid,
    #[error("management object was not found")]
    NotFound,
    #[error("management repository failed")]
    Repository(#[from] RepositoryError),
    #[error("project observation failed")]
    Project(#[from] ProjectError),
    #[error("code index is not current")]
    IndexNotCurrent,
    #[error("code index analysis input produced conflicting content")]
    IndexIdentityConflict,
    #[error("task planning failed")]
    Planning(#[from] PlanningError),
    #[error("check graph execution failed")]
    CheckGraph(#[from] CheckGraphRunnerError),
    #[error("finding or gate evaluation failed")]
    Validation(#[from] ValidationError),
    #[error("patch preparation failed")]
    Execution(#[from] ExecutionError),
    #[error("patch apply failed: {0}")]
    Apply(String),
    #[error("Rust style workflow failed: {0}")]
    RustStyle(#[from] RustStyleRuntimeError),
}

#[derive(Clone, Debug, Serialize)]
pub struct RegisterProjectResult {
    pub project: Project,
    pub checkout: ProjectCheckout,
    pub coordinated_operation: CoordinatedOperation,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScanProjectResult {
    pub scan_run: ScanRun,
    pub code_index_snapshot: Option<CodeIndexSnapshot>,
    pub finding_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct IndexStatusResult {
    pub snapshot: CodeIndexSnapshot,
    pub current: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct IndexQueryResult<T> {
    pub snapshot_id: star_contracts::ids::CodeIndexSnapshotId,
    pub requested_tier: IndexTier,
    pub used_tier: IndexTier,
    pub current: bool,
    pub confirmed_empty: bool,
    pub limitations: Vec<String>,
    pub items: Vec<T>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PreparedPatchResult {
    pub patch_set: PatchSet,
    pub change_plan_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct AppliedPatchResult {
    pub patch_set: PatchSet,
    pub validation_result: ValidationResult,
    pub gate_decision: GateDecision,
    pub automatic_rollback: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PreparedRustStyleResult {
    pub inspection: RustStyleInspection,
    pub state: String,
    pub candidate_fingerprint: Sha256Hash,
    pub before_fingerprint: Sha256Hash,
    pub expected_after_fingerprint: Sha256Hash,
    pub idempotence_proved: bool,
    pub changed_paths: Vec<ProjectPathRef>,
    pub patch_set: Option<PatchSet>,
    pub pre_apply_validation_result: Option<ValidationResult>,
    pub pre_apply_gate_decision: Option<GateDecision>,
    pub isolation_ref: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RustStyleAutoApplyResult {
    pub prepared: PreparedRustStyleResult,
    pub applied: Option<AppliedPatchResult>,
    pub permit_automatic: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SourceRebuildPlan {
    pub schema_version: u32,
    pub project_ids: Vec<ProjectId>,
    pub rebuildable_categories: Vec<String>,
    pub not_rebuildable_without_backup: Vec<String>,
    pub plan_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, Serialize)]
pub struct SourceRebuildProjectResult {
    pub project_id: ProjectId,
    pub scan_run_id: ScanRunId,
    pub scan_status: ScanStatus,
    pub finding_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct SourceRebuildResult {
    pub plan_fingerprint: Sha256Hash,
    pub projects: Vec<SourceRebuildProjectResult>,
    pub not_rebuildable_without_backup: Vec<String>,
}

struct AttachedCatalogEntry {
    project: Project,
    checkout: ProjectCheckout,
    root: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RustStyleAutoGrantSource {
    schema_version: u32,
    action: String,
    project_id: ProjectId,
    profile_ref: String,
    pipeline_ref: String,
    toolchain_fingerprint: Sha256Hash,
    style_policy_fingerprint: Sha256Hash,
    coverage_fingerprint: Sha256Hash,
    scope_paths: Vec<ProjectPathRef>,
    max_files: u32,
    max_changed_bytes: u64,
    expires_at: String,
}

pub struct ManagementApplicationService {
    repositories: Arc<dyn ManagementRepositorySet>,
    root_bindings: Arc<dyn ProjectRootBindingStore>,
    artifacts: Arc<dyn ArtifactStore>,
    scan_policy: ScanPolicy,
    index_policy: IndexPolicy,
    index_cache: Option<Arc<dyn CodeIndexCache>>,
    syntax_adapters: Vec<Arc<dyn SyntaxAdapter>>,
    semantic_adapters: Vec<Arc<dyn SemanticAdapter>>,
    rust_style_runtime_root: Option<PathBuf>,
    rust_style_policy_path: Option<PathBuf>,
    command_lock: Mutex<()>,
}

struct ManagedRustSourceMutationPort<'a> {
    service: &'a ManagementApplicationService,
    project_id: &'a ProjectId,
    patch_set_id: PatchSetId,
    approved_patch_fingerprint: String,
    result: Option<Result<AppliedPatchResult, ApplicationError>>,
}

impl rust_style::RustSourceMutationPort for ManagedRustSourceMutationPort<'_> {
    fn apply_exact(
        &mut self,
        _candidate: &rust_style::RustStyleCandidate,
    ) -> rust_style::SourceMutationObservation {
        let result = self.service.apply_patch_inner(
            self.project_id,
            &self.patch_set_id,
            &self.approved_patch_fingerprint,
        );
        let observation = match &result {
            Ok(applied)
                if !applied.automatic_rollback
                    && applied.gate_decision.authoritative_state()
                        == AuthoritativeGateState::Passed =>
            {
                rust_style::SourceMutationObservation::Applied {
                    post_gate_auto_pass: true,
                    evidence_complete: true,
                }
            }
            Ok(_) => rust_style::SourceMutationObservation::Partial,
            Err(ApplicationError::Apply(code)) if code.contains("STALE") => {
                rust_style::SourceMutationObservation::Stale
            }
            Err(_) => rust_style::SourceMutationObservation::OutcomeUnknown,
        };
        self.result = Some(result);
        observation
    }
}

impl ManagementApplicationService {
    pub fn new(
        repositories: Arc<dyn ManagementRepositorySet>,
        root_bindings: Arc<dyn ProjectRootBindingStore>,
        artifacts: Arc<dyn ArtifactStore>,
    ) -> Self {
        Self {
            repositories,
            root_bindings,
            artifacts,
            scan_policy: ScanPolicy::default(),
            index_policy: IndexPolicy::default(),
            index_cache: None,
            syntax_adapters: Vec::new(),
            semantic_adapters: Vec::new(),
            rust_style_runtime_root: None,
            rust_style_policy_path: None,
            command_lock: Mutex::new(()),
        }
    }

    pub fn with_index_cache(mut self, cache: Arc<dyn CodeIndexCache>) -> Self {
        self.index_cache = Some(cache);
        self
    }

    pub fn with_rust_style_runtime(
        mut self,
        runtime_root: PathBuf,
        release_policy_path: PathBuf,
    ) -> Self {
        self.rust_style_runtime_root = Some(runtime_root);
        self.rust_style_policy_path = Some(release_policy_path);
        self
    }

    pub fn with_syntax_adapter(mut self, adapter: Arc<dyn SyntaxAdapter>) -> Self {
        self.syntax_adapters.push(adapter);
        self
    }

    pub fn with_semantic_adapter(mut self, adapter: Arc<dyn SemanticAdapter>) -> Self {
        self.semantic_adapters.push(adapter);
        self
    }

    pub fn register_project(
        &self,
        project_root: &Path,
        idempotency_key: &str,
    ) -> Result<RegisterProjectResult, ApplicationError> {
        let _guard = self.command_guard()?;
        self.register_project_inner(project_root, idempotency_key)
    }

    fn register_project_inner(
        &self,
        project_root: &Path,
        idempotency_key: &str,
    ) -> Result<RegisterProjectResult, ApplicationError> {
        if !valid_idempotency_key(idempotency_key) {
            return Err(ApplicationError::Invalid);
        }
        let canonical_root = project_root
            .canonicalize()
            .map_err(|_| ApplicationError::Invalid)?;
        if let Some(existing) = self
            .repositories
            .global()
            .get_coordination_by_idempotency_key(idempotency_key)?
        {
            return self.replay_registration(existing, &canonical_root);
        }
        let attachment = self.root_bindings.find_by_root(&canonical_root)?;
        let seed = ProjectSeed::discover_with_local_project_id(
            &canonical_root,
            attachment.as_ref().map(|value| value.project_id.clone()),
        )?;
        if attachment
            .as_ref()
            .is_some_and(|value| value.project_id != seed.project_id)
        {
            return Err(ApplicationError::Invalid);
        }
        let checkout_id = attachment
            .as_ref()
            .map(|value| value.checkout_id.clone())
            .unwrap_or_default();
        let binding_id =
            self.root_bindings
                .attach(&seed.project_id, &checkout_id, &canonical_root)?;
        let attached = seed.attach(checkout_id, binding_id, &canonical_root)?;
        let project = attached.project;
        let checkout = attached.checkout;
        let global_before = self.repositories.global().status()?;
        let registration_payload = registration_fingerprint_payload(&project, &checkout);
        let input_fingerprint =
            versioned_fingerprint("star.command.project-register", 2, &registration_payload)
                .map_err(|_| ApplicationError::Invalid)?;
        let permission_scope_fingerprint = versioned_fingerprint(
            "star.permission-scope",
            1,
            &serde_json::json!({
                "action":"local_write",
                "project_id":project.project_id,
                "command":"project.register",
            }),
        )
        .map_err(|_| ApplicationError::Invalid)?;
        let operation_id = CoordinatedOperationId::new();
        let participant_payload = versioned_fingerprint(
            "star.coordination.project-register.participant",
            2,
            &registration_payload,
        )
        .map_err(|_| ApplicationError::Invalid)?;
        let now = Utc::now();
        let mut operation = CoordinatedOperation {
            schema_id: "star.coordinated-operation".to_owned(),
            schema_version: 1,
            coordinated_operation_id: operation_id.clone(),
            idempotency_key: idempotency_key.to_owned(),
            command_kind: "project.register".to_owned(),
            input_fingerprint: input_fingerprint.clone(),
            permission_scope_fingerprint,
            expected_version_vector: StoreVersionVector {
                global: store_point(&global_before),
                projects: vec![],
            },
            participants: vec![CoordinationParticipant {
                project_id: project.project_id.clone(),
                required: true,
                payload_fingerprint: participant_payload.clone(),
                state: ParticipantState::Pending,
                receipt: None,
            }],
            state: CoordinationState::Prepared,
            result_fingerprint: None,
            committed_version_vector: None,
            diagnostic_refs: vec![],
            artifact_refs: vec![],
            created_at: now,
            updated_at: now,
        };
        self.repositories.global().put_coordination(&operation)?;

        let project_repository = self.repositories.project(&project.project_id)?;
        let participant_result = versioned_fingerprint(
            "star.coordination.project-register.result",
            2,
            &registration_payload,
        )
        .map_err(|_| ApplicationError::Invalid)?;
        let receipt = project_repository.commit_registration_participant(
            &project,
            &operation_id,
            &participant_payload,
            &participant_result,
        )?;
        operation.participants[0].state = ParticipantState::Committed;
        operation.participants[0].receipt = Some(receipt);
        operation.state = CoordinationState::Applying;
        operation.updated_at = Utc::now();
        self.repositories.global().put_coordination(&operation)?;

        self.repositories.global().register_project(
            &project,
            &checkout,
            idempotency_key,
            &input_fingerprint,
        )?;
        let global_before_completion = self.repositories.global().status()?;
        let project_after = project_repository.status()?;
        let committed = StoreVersionVector {
            global: StorePoint {
                revision: global_before_completion.store_revision + 1,
                ..store_point(&global_before_completion)
            },
            projects: vec![ProjectStorePoint {
                project_id: project.project_id.clone(),
                point: store_point(&project_after),
            }],
        };
        let result_fingerprint = versioned_fingerprint(
            "star.coordination.completed",
            1,
            &serde_json::json!({
                "project":project,
                "checkout":checkout,
                "store_version_vector":committed,
            }),
        )
        .map_err(|_| ApplicationError::Invalid)?;
        operation.state = CoordinationState::Completed;
        operation.result_fingerprint = Some(result_fingerprint);
        operation.committed_version_vector = Some(committed);
        operation.updated_at = Utc::now();
        self.repositories.global().put_coordination(&operation)?;
        let _ = self.repositories.verify_all()?;
        Ok(RegisterProjectResult {
            project,
            checkout,
            coordinated_operation: operation,
        })
    }

    fn replay_registration(
        &self,
        operation: CoordinatedOperation,
        requested_root: &Path,
    ) -> Result<RegisterProjectResult, ApplicationError> {
        if operation.command_kind != "project.register" || operation.participants.len() != 1 {
            return Err(ApplicationError::Invalid);
        }
        let project_id = operation.participants[0].project_id.clone();
        let attachment = self
            .root_bindings
            .find_by_project(&project_id)?
            .ok_or(ApplicationError::NotFound)?;
        let attached_root = self.root_bindings.resolve(&attachment.root_binding_id)?;
        if attached_root != requested_root {
            return Err(ApplicationError::Repository(RepositoryError::new(
                RepositoryErrorCategory::IdempotencyConflict,
                "registration idempotency key belongs to another project root",
            )));
        }
        let seed =
            ProjectSeed::discover_with_local_project_id(&attached_root, Some(project_id.clone()))?;
        if seed.project_id != project_id {
            return Err(ApplicationError::Repository(RepositoryError::new(
                RepositoryErrorCategory::IdempotencyConflict,
                "registration declaration changed for an idempotent retry",
            )));
        }
        let candidate = seed.attach(
            attachment.checkout_id.clone(),
            attachment.root_binding_id,
            &attached_root,
        )?;
        let registration_payload =
            registration_fingerprint_payload(&candidate.project, &candidate.checkout);
        let input_fingerprint =
            versioned_fingerprint("star.command.project-register", 2, &registration_payload)
                .map_err(|_| ApplicationError::Invalid)?;
        if input_fingerprint != operation.input_fingerprint {
            return Err(ApplicationError::Repository(RepositoryError::new(
                RepositoryErrorCategory::IdempotencyConflict,
                "registration payload changed for an idempotent retry",
            )));
        }
        if operation.state != CoordinationState::Completed {
            self.recover_incomplete_registrations_inner()?;
        }
        let completed = self
            .repositories
            .global()
            .get_coordination(&operation.coordinated_operation_id)?
            .ok_or(ApplicationError::NotFound)?;
        if completed.state != CoordinationState::Completed {
            return Err(ApplicationError::Invalid);
        }
        let project = self
            .repositories
            .global()
            .get_project(&project_id)?
            .ok_or(ApplicationError::NotFound)?;
        let checkout = self
            .repositories
            .global()
            .get_project_checkout(&attachment.checkout_id)?
            .ok_or(ApplicationError::NotFound)?;
        Ok(RegisterProjectResult {
            project,
            checkout,
            coordinated_operation: completed,
        })
    }

    pub fn recover_incomplete_registrations(&self) -> Result<usize, ApplicationError> {
        let _guard = self.command_guard()?;
        self.recover_incomplete_registrations_inner()
    }

    fn recover_incomplete_registrations_inner(&self) -> Result<usize, ApplicationError> {
        let mut recovered = 0;
        for mut operation in self.repositories.global().list_incomplete_coordination()? {
            if operation.command_kind != "project.register" || operation.participants.len() != 1 {
                continue;
            }
            let project_id = operation.participants[0].project_id.clone();
            let project_repository = self.repositories.project(&project_id)?;
            let Some(attachment) = self.root_bindings.find_by_project(&project_id)? else {
                block_coordination(
                    self.repositories.global(),
                    &mut operation,
                    "PROJECT_ROOT_BINDING_MISSING",
                )?;
                continue;
            };
            let root = match self.root_bindings.resolve(&attachment.root_binding_id) {
                Ok(root) => root,
                Err(_) => {
                    block_coordination(
                        self.repositories.global(),
                        &mut operation,
                        "PROJECT_ROOT_BINDING_DETACHED",
                    )?;
                    continue;
                }
            };
            let seed = match ProjectSeed::discover_with_local_project_id(
                &root,
                Some(project_id.clone()),
            ) {
                Ok(seed) if seed.project_id == project_id => seed,
                _ => {
                    block_coordination(
                        self.repositories.global(),
                        &mut operation,
                        "PROJECT_DECLARATION_CHANGED",
                    )?;
                    continue;
                }
            };
            let attached =
                match seed.attach(attachment.checkout_id, attachment.root_binding_id, &root) {
                    Ok(attached) => attached,
                    Err(_) => {
                        block_coordination(
                            self.repositories.global(),
                            &mut operation,
                            "PROJECT_CHECKOUT_OBSERVATION_FAILED",
                        )?;
                        continue;
                    }
                };
            let project = attached.project;
            let checkout = attached.checkout;
            if project_repository
                .get_project()?
                .is_some_and(|stored| stored != project)
            {
                block_coordination(
                    self.repositories.global(),
                    &mut operation,
                    "PROJECT_REGISTRATION_INPUT_CHANGED",
                )?;
                continue;
            }
            let registration_payload = registration_fingerprint_payload(&project, &checkout);
            let input_fingerprint =
                versioned_fingerprint("star.command.project-register", 2, &registration_payload)
                    .map_err(|_| ApplicationError::Invalid)?;
            let participant_payload = versioned_fingerprint(
                "star.coordination.project-register.participant",
                2,
                &registration_payload,
            )
            .map_err(|_| ApplicationError::Invalid)?;
            if input_fingerprint != operation.input_fingerprint
                || participant_payload != operation.participants[0].payload_fingerprint
            {
                block_coordination(
                    self.repositories.global(),
                    &mut operation,
                    "PROJECT_REGISTRATION_INPUT_CHANGED",
                )?;
                continue;
            }
            let participant_result = versioned_fingerprint(
                "star.coordination.project-register.result",
                2,
                &registration_payload,
            )
            .map_err(|_| ApplicationError::Invalid)?;
            let receipt = project_repository.commit_registration_participant(
                &project,
                &operation.coordinated_operation_id,
                &participant_payload,
                &participant_result,
            )?;
            operation.participants[0].state = ParticipantState::Committed;
            operation.participants[0].receipt = Some(receipt);
            operation.state = CoordinationState::Applying;
            operation.updated_at = Utc::now();
            self.repositories.global().put_coordination(&operation)?;

            self.repositories.global().register_project(
                &project,
                &checkout,
                &operation.idempotency_key,
                &operation.input_fingerprint,
            )?;
            let global = self.repositories.global().status()?;
            let local = project_repository.status()?;
            let committed = StoreVersionVector {
                global: StorePoint {
                    revision: global.store_revision + 1,
                    ..store_point(&global)
                },
                projects: vec![ProjectStorePoint {
                    project_id: project_id.clone(),
                    point: store_point(&local),
                }],
            };
            operation.state = CoordinationState::Completed;
            operation.committed_version_vector = Some(committed.clone());
            operation.result_fingerprint = Some(
                versioned_fingerprint(
                    "star.coordination.completed",
                    1,
                    &serde_json::json!({
                        "project":project,
                        "checkout":checkout,
                        "store_version_vector":committed,
                    }),
                )
                .map_err(|_| ApplicationError::Invalid)?,
            );
            operation.updated_at = Utc::now();
            self.repositories.global().put_coordination(&operation)?;
            recovered += 1;
        }
        Ok(recovered)
    }

    pub fn list_projects(&self) -> Result<Vec<Project>, ApplicationError> {
        Ok(self.repositories.global().list_projects()?)
    }

    fn primary_project_root(&self, project: &Project) -> Result<PathBuf, ApplicationError> {
        let checkout_id = project
            .attached_checkout_ids
            .first()
            .ok_or(ApplicationError::Invalid)?;
        let attachment = self
            .root_bindings
            .find_by_checkout(checkout_id)?
            .ok_or(ApplicationError::NotFound)?;
        if attachment.project_id != project.project_id {
            return Err(ApplicationError::Invalid);
        }
        Ok(self.root_bindings.resolve(&attachment.root_binding_id)?)
    }

    fn attached_catalog_entries(&self) -> Result<Vec<AttachedCatalogEntry>, ApplicationError> {
        let mut entries = Vec::new();
        for project in self.repositories.global().list_projects()? {
            for checkout in self
                .repositories
                .global()
                .list_project_checkouts(&project.project_id)?
            {
                let attachment = self
                    .root_bindings
                    .find_by_checkout(&checkout.checkout_id)?
                    .ok_or(ApplicationError::NotFound)?;
                if attachment.project_id != project.project_id {
                    return Err(ApplicationError::Invalid);
                }
                entries.push(AttachedCatalogEntry {
                    project: project.clone(),
                    checkout,
                    root: self.root_bindings.resolve(&attachment.root_binding_id)?,
                });
            }
        }
        entries.sort_by(|left, right| left.checkout.checkout_id.cmp(&right.checkout.checkout_id));
        Ok(entries)
    }

    fn refresh_project_catalog(
        &self,
    ) -> Result<(ProjectCatalogSnapshot, Vec<AttachedCatalogEntry>), ApplicationError> {
        let entries = self.attached_catalog_entries()?;
        let inputs: Vec<_> = entries
            .iter()
            .map(|entry| CatalogSnapshotInput {
                project: &entry.project,
                checkout: &entry.checkout,
                root: &entry.root,
            })
            .collect();
        let snapshot = build_project_catalog_snapshot(&inputs, &DiscoveryConfig::default())?;
        self.repositories
            .global()
            .put_project_catalog_snapshot(&snapshot)?;
        let persisted = self
            .repositories
            .global()
            .latest_project_catalog_snapshot()?
            .filter(|persisted| {
                persisted.project_catalog_snapshot_id == snapshot.project_catalog_snapshot_id
            })
            .unwrap_or(snapshot);
        Ok((persisted, entries))
    }

    pub fn discover_projects(&self) -> Result<ProjectCatalogSnapshot, ApplicationError> {
        let _guard = self.command_guard()?;
        self.refresh_project_catalog().map(|(snapshot, _)| snapshot)
    }

    pub fn scan_project(
        &self,
        project_id: &ProjectId,
        idempotency_key: &str,
    ) -> Result<ScanProjectResult, ApplicationError> {
        let _guard = self.command_guard()?;
        self.scan_project_inner(project_id, idempotency_key)
    }

    fn scan_project_inner(
        &self,
        project_id: &ProjectId,
        idempotency_key: &str,
    ) -> Result<ScanProjectResult, ApplicationError> {
        if !valid_idempotency_key(idempotency_key) {
            return Err(ApplicationError::Invalid);
        }
        let mut project = self
            .repositories
            .global()
            .get_project(project_id)?
            .ok_or(ApplicationError::NotFound)?;
        let root = self.primary_project_root(&project)?;
        let checkout_id = project
            .attached_checkout_ids
            .first()
            .ok_or(ApplicationError::Invalid)?;
        let checkout = self
            .repositories
            .global()
            .get_project_checkout(checkout_id)?
            .ok_or(ApplicationError::NotFound)?;
        let (catalog_snapshot, catalog_entries) = self.refresh_project_catalog()?;
        let mut scan_policy = self.scan_policy.clone();
        scan_policy.excluded_relative_roots = catalog_entries
            .iter()
            .filter(|entry| entry.checkout.checkout_id != checkout.checkout_id)
            .filter_map(|entry| entry.root.strip_prefix(&root).ok())
            .filter(|relative| !relative.as_os_str().is_empty())
            .filter_map(|relative| {
                let value = relative
                    .components()
                    .filter_map(|component| component.as_os_str().to_str())
                    .collect::<Vec<_>>()
                    .join("/");
                ProjectPathRef::parse(value).ok()
            })
            .collect();
        let observation = observe_project(&project, &root, &scan_policy)?;
        let mut scan_complete =
            observation.completeness == star_contracts::management::Completeness::Complete;
        let mut scan_limitations = observation.limitations.clone();
        let scan_run_id = ScanRunId::new();
        let generation_id = GenerationId::new();
        let workspace_snapshot_id = observation.workspace_snapshot_id(project_id)?;
        let mut adapter_cache_fingerprints = self
            .syntax_adapters
            .iter()
            .map(|adapter| {
                serde_json::json!({
                    "language_id":adapter.language_id(),
                    "tier":"syntax",
                    "fingerprint":adapter.fingerprint(),
                })
            })
            .chain(self.semantic_adapters.iter().map(|adapter| {
                serde_json::json!({
                    "language_id":adapter.language_id(),
                    "tier":"semantic",
                    "fingerprint":adapter.fingerprint(),
                })
            }))
            .collect::<Vec<_>>();
        adapter_cache_fingerprints.sort_by_key(serde_json::Value::to_string);
        let index_cache_key = versioned_fingerprint(
            "star.code-index-cache-key",
            1,
            &serde_json::json!({
                "project_id":project_id,
                "checkout_id":checkout.checkout_id,
                "checkout_observation_fingerprint":checkout.content_fingerprint,
                "workspace_snapshot_id":workspace_snapshot_id,
                "scan_config_fingerprint":observation.scan_config_fingerprint,
                "index_policy":self.index_policy,
                "adapters":adapter_cache_fingerprints,
            }),
        )
        .map_err(|_| ApplicationError::Invalid)?;
        let (sources, mut symbols) =
            observation.source_graph(project_id, &workspace_snapshot_id, &scan_run_id)?;
        let repository = self.repositories.project(project_id)?;
        let mut stored_previous = repository.latest_code_index_projection()?;
        if stored_previous.is_none()
            && let Some(cache) = &self.index_cache
        {
            match cache.load(project_id, &index_cache_key) {
                Ok(cached) => stored_previous = cached,
                Err(_) => stored_previous = None,
            }
        }
        let previous = stored_previous.map(|stored| {
            let index_symbol_ids: BTreeSet<_> = stored
                .entities
                .iter()
                .filter_map(|entity| entity.symbol_id.clone())
                .collect();
            CodeIndexProjection {
                snapshot: stored.snapshot,
                source_entries: stored.source_entries,
                entities: stored.entities,
                edges: stored.edges,
                symbols: stored
                    .symbols
                    .into_iter()
                    .filter(|symbol| index_symbol_ids.contains(&symbol.symbol_id))
                    .collect(),
                references: stored.references,
            }
        });
        let syntax_adapters = self
            .syntax_adapters
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>();
        let semantic_adapters = self
            .semantic_adapters
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>();
        let mut code_index = build_code_index(&CodeIndexBuildRequest {
            project_root: Some(&root),
            project: &project,
            checkout: &checkout,
            catalog_snapshot: &catalog_snapshot,
            observation: &observation,
            scan_run_id: &scan_run_id,
            generation_id: &generation_id,
            policy: &self.index_policy,
            syntax_adapters: &syntax_adapters,
            semantic_adapters: &semantic_adapters,
            previous: previous.as_ref(),
        })?;
        if previous.as_ref().is_some_and(|previous| {
            index_identity_conflicts(&previous.snapshot, &code_index.snapshot)
        }) {
            return Err(ApplicationError::IndexIdentityConflict);
        }
        if code_index.snapshot.partitions.iter().any(|partition| {
            partition.required
                && !matches!(
                    partition.state,
                    IndexPartitionState::Succeeded | IndexPartitionState::Reused
                )
        }) {
            scan_complete = false;
        }
        scan_limitations.extend(
            code_index
                .snapshot
                .limitations
                .iter()
                .map(|item| item.code.clone()),
        );
        symbols.extend(code_index.symbols.clone());
        symbols.sort_by(|left, right| left.symbol_id.cmp(&right.symbol_id));
        symbols.dedup_by(|left, right| left.symbol_id == right.symbol_id);
        let mut projection = analyze_trailing_whitespace(
            project_id,
            &observation.revision,
            &workspace_snapshot_id,
            &scan_run_id,
            &observation.files,
            &sources,
            &symbols,
        )?;
        let shared_decisions = match load_shared_decisions(&project, &root) {
            Ok(declarations) => declarations,
            Err(_) => {
                scan_complete = false;
                scan_limitations.push("shared_decision_declaration_invalid".to_owned());
                SharedDecisionDeclarations {
                    baselines: vec![],
                    suppressions: vec![],
                    source_fingerprint: versioned_fingerprint(
                        "star.shared-decision-declarations-invalid",
                        1,
                        &serde_json::json!({"project_id":project_id,"error_code":"INVALID"}),
                    )
                    .map_err(|_| ApplicationError::Invalid)?,
                }
            }
        };
        repository.sync_shared_decisions(
            &shared_decisions.baselines,
            &shared_decisions.suppressions,
            &shared_decisions.source_fingerprint,
        )?;
        let baselines = repository.list_baselines()?;
        let suppressions = repository.list_suppressions()?;
        let dispositions = repository.list_dispositions()?;
        let decisions = evaluate_decisions(
            project_id,
            &observation.revision.project_revision_id,
            &observation.scan_config_fingerprint,
            &projection.rule_set_fingerprint,
            &projection.findings,
            &projection.occurrences,
            &baselines,
            &suppressions,
            &dispositions,
            Utc::now(),
        );
        apply_decision_projection(&mut projection.findings, &decisions);
        let decision_set_fingerprint = versioned_fingerprint(
            "star.scan-decision-inputs",
            1,
            &serde_json::json!({
                "baselines":baselines,
                "suppressions":suppressions,
                "dispositions":dispositions,
                "shared_source_fingerprint":shared_decisions.source_fingerprint,
            }),
        )
        .map_err(|_| ApplicationError::Invalid)?;
        let effective_config_fingerprint = versioned_fingerprint(
            "star.effective-config",
            1,
            &serde_json::json!({
                "scan_config_fingerprint":observation.scan_config_fingerprint,
                "require_complete_for_gate":true,
                "suppression_default_expiry_days":90,
                "decision_set_fingerprint":decision_set_fingerprint,
            }),
        )
        .map_err(|_| ApplicationError::Invalid)?;
        scan_limitations.sort();
        scan_limitations.dedup();
        let input_fingerprint = versioned_fingerprint(
            "star.scan-input",
            1,
            &serde_json::json!({
                "workspace_snapshot_id":workspace_snapshot_id,
                "scan_config_fingerprint":observation.scan_config_fingerprint,
                "rule_set_fingerprint":projection.rule_set_fingerprint,
                "decision_set_fingerprint":decision_set_fingerprint,
                "scan_complete":scan_complete,
                "scan_limitations":scan_limitations,
                "code_index_analysis_input_fingerprint":code_index.snapshot.analysis_input_fingerprint,
                "code_index_content_fingerprint":code_index.snapshot.content_fingerprint,
                "scanner_contract_version":2,
            }),
        )
        .map_err(|_| ApplicationError::Invalid)?;
        if let Some(scan_run) = repository.replay_scan(idempotency_key, &input_fingerprint)? {
            return Ok(ScanProjectResult {
                scan_run,
                code_index_snapshot: repository
                    .latest_code_index_projection()?
                    .map(|projection| projection.snapshot),
                finding_count: repository.list_findings()?.len(),
            });
        }
        let manifest_artifact = self.artifacts.put_json(
            project_id,
            &root,
            &format!(
                "management/snapshots/{}/workspace-manifest.json",
                workspace_snapshot_id.as_str()
            ),
            "workspace_snapshot",
            workspace_snapshot_id.as_str(),
            &observation.entries_manifest,
        )?;
        let snapshot = observation.workspace_snapshot(project_id, manifest_artifact.clone())?;
        code_index.snapshot.artifact_refs = vec![manifest_artifact.clone()];
        let status = if scan_complete {
            ScanStatus::Succeeded
        } else {
            ScanStatus::Incomplete
        };
        let mut counts = std::collections::BTreeMap::new();
        counts.insert("source".to_owned(), sources.len() as u64);
        counts.insert("symbol".to_owned(), symbols.len() as u64);
        counts.insert("reference".to_owned(), code_index.references.len() as u64);
        counts.insert("index_entity".to_owned(), code_index.entities.len() as u64);
        counts.insert("index_edge".to_owned(), code_index.edges.len() as u64);
        counts.insert("occurrence".to_owned(), projection.occurrences.len() as u64);
        counts.insert("finding".to_owned(), projection.findings.len() as u64);
        let scan_run = ScanRun {
            schema_id: "star.scan-run".to_owned(),
            schema_version: 1,
            scan_run_id: scan_run_id.clone(),
            project_id: project_id.clone(),
            project_revision_id: observation.revision.project_revision_id.clone(),
            workspace_snapshot_id: snapshot.workspace_snapshot_id.clone(),
            effective_config_fingerprint,
            scan_config_fingerprint: observation.scan_config_fingerprint.clone(),
            rule_set_fingerprint: projection.rule_set_fingerprint,
            input_fingerprint: input_fingerprint.clone(),
            status,
            generation_id,
            started_at: Utc::now(),
            finished_at: Some(Utc::now()),
            reused_from_scan_run_id: None,
            counts,
            limitations: scan_limitations,
            artifact_refs: vec![manifest_artifact],
        };
        project.latest_revision_id = Some(observation.revision.project_revision_id.clone());
        project.latest_workspace_snapshot_id = Some(snapshot.workspace_snapshot_id.clone());
        let finding_count = projection.findings.len();
        let cache_projection = StoredCodeIndexProjection {
            snapshot: code_index.snapshot.clone(),
            source_entries: code_index.source_entries.clone(),
            entities: code_index.entities.clone(),
            edges: code_index.edges.clone(),
            symbols: code_index.symbols.clone(),
            references: code_index.references.clone(),
        };
        let commit = ScanCommit {
            project,
            revision: observation.revision,
            snapshot,
            run: scan_run.clone(),
            sources,
            symbols,
            references: code_index.references.clone(),
            findings: projection.findings,
            occurrences: projection.occurrences,
            code_index: Some(code_index.snapshot.clone()),
            source_entries: code_index.source_entries,
            index_entities: code_index.entities,
            index_edges: code_index.edges,
            idempotency_key: idempotency_key.to_owned(),
            payload_fingerprint: input_fingerprint,
        };
        let committed_run = repository.commit_scan(&commit)?;
        if let Some(cache) = &self.index_cache {
            let _ = cache.store(project_id, &index_cache_key, &cache_projection);
        }
        Ok(ScanProjectResult {
            scan_run: committed_run,
            code_index_snapshot: Some(cache_projection.snapshot),
            finding_count: repository.list_findings()?.len().max(finding_count),
        })
    }

    pub fn index_status(
        &self,
        project_id: &ProjectId,
    ) -> Result<IndexStatusResult, ApplicationError> {
        let (projection, current) = self.load_index_projection_with_freshness(project_id)?;
        Ok(IndexStatusResult {
            snapshot: projection.snapshot,
            current,
        })
    }

    pub fn index_search(
        &self,
        project_id: &ProjectId,
        query: &str,
        requested_tier: IndexTier,
        require_current: bool,
    ) -> Result<IndexQueryResult<IndexEntity>, ApplicationError> {
        if query.trim().is_empty() || query.chars().count() > 256 {
            return Err(ApplicationError::Invalid);
        }
        let (projection, current) = self.load_index_projection_with_freshness(project_id)?;
        if require_current && !current {
            return Err(ApplicationError::IndexNotCurrent);
        }
        let query = query.to_lowercase();
        let items = projection
            .entities
            .iter()
            .filter(|entity| {
                entity.tier >= requested_tier
                    && entity.qualified_name.to_lowercase().contains(&query)
            })
            .take(256)
            .cloned()
            .collect();
        let required_partition_kind = match requested_tier {
            IndexTier::Text => IndexPartitionKind::Text,
            IndexTier::Syntax => IndexPartitionKind::Syntax,
            IndexTier::Semantic => IndexPartitionKind::Semantic,
        };
        Ok(index_query_result(
            &projection.snapshot,
            requested_tier,
            current,
            items,
            required_partition_kind,
        ))
    }

    pub fn index_definitions(
        &self,
        project_id: &ProjectId,
        query: &str,
        require_current: bool,
    ) -> Result<IndexQueryResult<IndexEntity>, ApplicationError> {
        if query.trim().is_empty() || query.chars().count() > 256 {
            return Err(ApplicationError::Invalid);
        }
        let (projection, current) = self.load_index_projection_with_freshness(project_id)?;
        if require_current && !current {
            return Err(ApplicationError::IndexNotCurrent);
        }
        let query = query.to_lowercase();
        let items = projection
            .entities
            .iter()
            .filter(|entity| {
                entity.kind == star_contracts::index::IndexEntityKind::Symbol
                    && entity.tier >= IndexTier::Syntax
                    && entity.qualified_name.to_lowercase().contains(&query)
            })
            .take(256)
            .cloned()
            .collect();
        Ok(index_query_result(
            &projection.snapshot,
            IndexTier::Syntax,
            current,
            items,
            IndexPartitionKind::Syntax,
        ))
    }

    pub fn index_references(
        &self,
        project_id: &ProjectId,
        symbol_id: &star_contracts::ids::SymbolId,
        require_current: bool,
    ) -> Result<IndexQueryResult<SymbolReference>, ApplicationError> {
        let (projection, current) = self.load_index_projection_with_freshness(project_id)?;
        if require_current && !current {
            return Err(ApplicationError::IndexNotCurrent);
        }
        let items = projection
            .references
            .iter()
            .filter(|reference| {
                reference.from_symbol_id.as_ref() == Some(symbol_id)
                    || reference.to_symbol_id.as_ref() == Some(symbol_id)
            })
            .take(256)
            .cloned()
            .collect();
        Ok(index_query_result(
            &projection.snapshot,
            IndexTier::Syntax,
            current,
            items,
            IndexPartitionKind::Syntax,
        ))
    }

    pub fn graph_neighbors(
        &self,
        project_id: &ProjectId,
        entity_key: &str,
        require_current: bool,
    ) -> Result<IndexQueryResult<IndexEdge>, ApplicationError> {
        if entity_key.is_empty() || entity_key.chars().count() > 512 {
            return Err(ApplicationError::Invalid);
        }
        let (projection, current) = self.load_index_projection_with_freshness(project_id)?;
        if require_current && !current {
            return Err(ApplicationError::IndexNotCurrent);
        }
        let items = projection
            .edges
            .iter()
            .filter(|edge| {
                edge.from_entity_key == entity_key
                    || edge.to_entity_key.as_deref() == Some(entity_key)
            })
            .take(256)
            .cloned()
            .collect();
        Ok(index_query_result(
            &projection.snapshot,
            IndexTier::Text,
            current,
            items,
            IndexPartitionKind::Text,
        ))
    }

    fn load_index_projection_with_freshness(
        &self,
        project_id: &ProjectId,
    ) -> Result<(CodeIndexProjection, bool), ApplicationError> {
        let project = self
            .repositories
            .global()
            .get_project(project_id)?
            .ok_or(ApplicationError::NotFound)?;
        let checkout_id = project
            .attached_checkout_ids
            .first()
            .ok_or(ApplicationError::Invalid)?;
        let checkout = self
            .repositories
            .global()
            .get_project_checkout(checkout_id)?
            .ok_or(ApplicationError::NotFound)?;
        let root = self.primary_project_root(&project)?;
        let (catalog_snapshot, catalog_entries) = self.refresh_project_catalog()?;
        let mut scan_policy = self.scan_policy.clone();
        scan_policy.excluded_relative_roots = catalog_entries
            .iter()
            .filter(|entry| entry.checkout.checkout_id != checkout.checkout_id)
            .filter_map(|entry| entry.root.strip_prefix(&root).ok())
            .filter(|relative| !relative.as_os_str().is_empty())
            .filter_map(|relative| {
                ProjectPathRef::parse(
                    relative
                        .components()
                        .filter_map(|component| component.as_os_str().to_str())
                        .collect::<Vec<_>>()
                        .join("/"),
                )
                .ok()
            })
            .collect();
        let observation = observe_project(&project, &root, &scan_policy)?;
        let current_workspace_snapshot_id = observation.workspace_snapshot_id(project_id)?;
        let stored = self
            .repositories
            .project(project_id)?
            .latest_code_index_projection()?
            .ok_or(ApplicationError::NotFound)?;
        let mut snapshot = stored.snapshot;
        let state = if checkout.content_fingerprint != snapshot.checkout_observation_fingerprint
            || catalog_snapshot.project_catalog_snapshot_id != snapshot.project_catalog_snapshot_id
        {
            IndexFreshnessState::StaleCatalog
        } else if current_workspace_snapshot_id != snapshot.workspace_snapshot_id {
            IndexFreshnessState::StaleSource
        } else if observation.scan_config_fingerprint != snapshot.scan_config_fingerprint {
            IndexFreshnessState::StaleConfig
        } else if observation.completeness != star_contracts::management::Completeness::Complete {
            IndexFreshnessState::Unverified
        } else {
            IndexFreshnessState::Current
        };
        for proof in &mut snapshot.freshness {
            let partition = snapshot
                .partitions
                .iter()
                .find(|partition| partition.partition_key == proof.partition_key);
            let partition_is_usable = partition.is_some_and(|partition| {
                matches!(
                    partition.state,
                    IndexPartitionState::Succeeded | IndexPartitionState::Reused
                )
            });
            proof.state = if state == IndexFreshnessState::Current && !partition_is_usable {
                if partition.is_some_and(|partition| {
                    partition.limitations.iter().any(|limitation| {
                        matches!(
                            limitation.code.as_str(),
                            "INDEX_LANGUAGE_UNSUPPORTED" | "INDEX_SEMANTIC_UNAVAILABLE"
                        )
                    })
                }) {
                    IndexFreshnessState::Unavailable
                } else {
                    IndexFreshnessState::Partial
                }
            } else {
                state
            };
            proof.observed_source_fingerprint = Some(observation.entries_fingerprint.clone());
            proof.probe_method = "bounded_content_sha256".to_owned();
            proof.probed_at = Utc::now();
            proof.stale_reason_codes = match proof.state {
                IndexFreshnessState::Current => Vec::new(),
                IndexFreshnessState::StaleCatalog => vec!["INDEX_STALE_CATALOG".to_owned()],
                IndexFreshnessState::StaleSource => vec!["INDEX_STALE_SOURCE".to_owned()],
                IndexFreshnessState::StaleConfig => vec!["INDEX_STALE_CONFIG".to_owned()],
                IndexFreshnessState::StaleAdapter => vec!["INDEX_STALE_ADAPTER".to_owned()],
                IndexFreshnessState::Partial => vec!["INDEX_RESULT_PARTIAL".to_owned()],
                IndexFreshnessState::Unverified => {
                    vec!["INDEX_FRESHNESS_UNVERIFIED".to_owned()]
                }
                IndexFreshnessState::Unavailable => {
                    vec!["INDEX_PARTITION_UNAVAILABLE".to_owned()]
                }
            };
        }
        let required_partitions_current = snapshot
            .partitions
            .iter()
            .filter(|partition| partition.required)
            .all(|partition| {
                matches!(
                    partition.state,
                    IndexPartitionState::Succeeded | IndexPartitionState::Reused
                )
            });
        Ok((
            CodeIndexProjection {
                snapshot,
                source_entries: stored.source_entries,
                entities: stored.entities,
                edges: stored.edges,
                symbols: stored.symbols,
                references: stored.references,
            },
            state == IndexFreshnessState::Current && required_partitions_current,
        ))
    }

    pub fn list_findings(&self, project_id: &ProjectId) -> Result<Vec<Finding>, ApplicationError> {
        Ok(self.repositories.project(project_id)?.list_findings()?)
    }

    pub fn create_planning_bundle(
        &self,
        task: TaskSpecDraft,
        actor: ActorRef,
        check_descriptors: Vec<CheckDescriptor>,
        idempotency_key: &str,
    ) -> Result<PlanningBundle, ApplicationError> {
        let _guard = self.command_guard()?;
        if !valid_idempotency_key(idempotency_key) || task.project_targets.is_empty() {
            return Err(ApplicationError::Invalid);
        }
        let input_fingerprint = versioned_fingerprint(
            "star.command.planning-create",
            1,
            &serde_json::json!({
                "task":task,
                "actor":actor,
                "check_descriptors":check_descriptors,
                "policy":PlanningPolicy::default(),
            }),
        )
        .map_err(|_| ApplicationError::Invalid)?;
        if let Some((existing, stored_input)) = self
            .repositories
            .global()
            .get_planning_bundle_by_idempotency_key(idempotency_key)?
        {
            if stored_input != input_fingerprint {
                return Err(ApplicationError::Repository(RepositoryError::new(
                    RepositoryErrorCategory::IdempotencyConflict,
                    "planning idempotency key was already used for different input",
                )));
            }
            return Ok(existing);
        }
        let mut targets = task.project_targets.clone();
        targets.sort_by(|left, right| left.project_id.cmp(&right.project_id));
        if targets
            .windows(2)
            .any(|pair| pair[0].project_id == pair[1].project_id)
        {
            return Err(ApplicationError::Invalid);
        }
        let (catalog, _) = self.refresh_project_catalog()?;
        let mut projects = Vec::with_capacity(targets.len());
        let mut pinned_snapshots = Vec::with_capacity(targets.len());
        for target in &targets {
            let project = self
                .repositories
                .global()
                .get_project(&target.project_id)?
                .ok_or(ApplicationError::NotFound)?;
            let checkout = self
                .repositories
                .global()
                .get_project_checkout(&target.checkout_id)?
                .ok_or(ApplicationError::NotFound)?;
            if checkout.project_id != project.project_id
                || !project
                    .attached_checkout_ids
                    .contains(&checkout.checkout_id)
            {
                return Err(ApplicationError::Invalid);
            }
            let attachment = self
                .root_bindings
                .find_by_checkout(&target.checkout_id)?
                .ok_or(ApplicationError::NotFound)?;
            if attachment.project_id != target.project_id {
                return Err(ApplicationError::Invalid);
            }
            let root = self.root_bindings.resolve(&attachment.root_binding_id)?;
            let (projection, current) =
                self.load_index_projection_with_freshness(&target.project_id)?;
            if !current
                || projection.snapshot.checkout_id != target.checkout_id
                || projection.snapshot.project_catalog_snapshot_id
                    != catalog.project_catalog_snapshot_id
            {
                return Err(ApplicationError::IndexNotCurrent);
            }
            let observed = observe_workspace_changes(&project, &root, &projection.source_entries)?;
            let collection_state = match observed.completeness {
                star_contracts::management::Completeness::Complete => CollectionState::Complete,
                star_contracts::management::Completeness::Partial => CollectionState::Partial,
                star_contracts::management::Completeness::Unverified => CollectionState::Unverified,
            };
            pinned_snapshots.push((
                target.project_id.clone(),
                projection.snapshot.code_index_snapshot_id.clone(),
            ));
            projects.push(PlanningProjectIndex {
                snapshot: projection.snapshot,
                source_entries: projection.source_entries,
                entities: projection.entities,
                edges: projection.edges,
                observed_changes: observed
                    .entries
                    .into_iter()
                    .map(|change| ObservedWorkspaceChange {
                        path: change.path,
                        rename_from: change.rename_from,
                        change_kind: change.change_kind,
                        before_sha256: change.before_sha256,
                        after_sha256: change.after_sha256,
                        staged: change.staged,
                        unstaged: change.unstaged,
                        untracked: change.untracked,
                        binary: change.binary,
                    })
                    .collect(),
                collection_state,
                collection_limits: observed.limitations,
            });
        }
        let bundle = build_planning_bundle(PlanningRequest {
            task,
            actor,
            catalog,
            projects,
            risk_descriptors: builtin_risk_descriptors()?,
            check_descriptors,
            policy: PlanningPolicy::default(),
        })?;
        for (project_id, snapshot_id) in pinned_snapshots {
            let (projection, current) = self.load_index_projection_with_freshness(&project_id)?;
            if !current || projection.snapshot.code_index_snapshot_id != snapshot_id {
                return Err(ApplicationError::IndexNotCurrent);
            }
        }
        Ok(self.repositories.global().put_planning_bundle(
            &bundle,
            idempotency_key,
            &input_fingerprint,
        )?)
    }

    pub fn get_planning_bundle(
        &self,
        task_spec_id: &TaskSpecId,
    ) -> Result<PlanningBundle, ApplicationError> {
        self.repositories
            .global()
            .get_planning_bundle(task_spec_id)?
            .ok_or(ApplicationError::NotFound)
    }

    pub fn execute_planning_bundle(
        &self,
        task_spec_id: &TaskSpecId,
        bindings: &[ExecutableBinding],
        context: CheckGraphRunContext,
        executor: &mut dyn CheckExecutor,
    ) -> Result<CheckGraphRunResult, ApplicationError> {
        let _guard = self.command_guard()?;
        let bundle = self
            .repositories
            .global()
            .get_planning_bundle(task_spec_id)?
            .ok_or(ApplicationError::NotFound)?;
        let project_ids = bundle
            .validation_plan
            .required_checks
            .iter()
            .map(|check| check.project_id.clone())
            .collect::<BTreeSet<_>>();
        if project_ids.len() != 1 {
            return Err(ApplicationError::Invalid);
        }
        let mut pinned = Vec::new();
        for source in &bundle.scope_revision.source_snapshot_refs {
            let (projection, current) =
                self.load_index_projection_with_freshness(&source.project_id)?;
            if !current
                || projection.snapshot.checkout_id != source.checkout_id
                || projection.snapshot.code_index_snapshot_id != source.code_index_snapshot_id
                || projection.snapshot.workspace_snapshot_id != source.workspace_snapshot_id
            {
                return Err(ApplicationError::IndexNotCurrent);
            }
            pinned.push((
                source.project_id.clone(),
                source.code_index_snapshot_id.clone(),
            ));
        }
        let result = run_check_graph(&bundle.validation_plan, bindings, context, executor)?;
        for (project_id, snapshot_id) in &pinned {
            let (projection, current) = self.load_index_projection_with_freshness(project_id)?;
            if !current || projection.snapshot.code_index_snapshot_id != *snapshot_id {
                return Err(ApplicationError::IndexNotCurrent);
            }
        }
        let project_id = project_ids
            .into_iter()
            .next()
            .ok_or(ApplicationError::Invalid)?;
        self.repositories
            .project(&project_id)?
            .save_check_graph_evidence(
                &result.validation_runs,
                &result.diagnostics,
                &result.gate_decision,
                &result.evidence_bundle,
            )?;
        Ok(result)
    }

    pub fn put_suppression(
        &self,
        suppression: &Suppression,
        expected_revision: u64,
    ) -> Result<(), ApplicationError> {
        let _guard = self.command_guard()?;
        self.repositories
            .project(&suppression.project_id)?
            .put_suppression(suppression, expected_revision)?;
        Ok(())
    }

    pub fn put_baseline(
        &self,
        baseline: &Baseline,
        expected_revision: u64,
    ) -> Result<(), ApplicationError> {
        let _guard = self.command_guard()?;
        self.repositories
            .project(&baseline.project_id)?
            .put_baseline(baseline, expected_revision)?;
        Ok(())
    }

    pub fn put_disposition(
        &self,
        project_id: &ProjectId,
        disposition: &Disposition,
        expected_revision: u64,
    ) -> Result<(), ApplicationError> {
        let _guard = self.command_guard()?;
        self.repositories
            .project(project_id)?
            .put_disposition(disposition, expected_revision)?;
        Ok(())
    }

    pub fn inspect_rust_style(
        &self,
        project_id: &ProjectId,
        scope: RustStyleScope,
        auto_policy: RustAutoPolicy,
    ) -> Result<RustStyleInspection, ApplicationError> {
        let project = self
            .repositories
            .global()
            .get_project(project_id)?
            .ok_or(ApplicationError::NotFound)?;
        let root = self.primary_project_root(&project)?;
        let (runtime_root, policy_path) = self.rust_style_runtime_paths()?;
        Ok(inspect_rust_style(
            project_id,
            &root,
            runtime_root,
            policy_path,
            scope,
            auto_policy,
        )?)
    }

    pub fn check_rust_style(
        &self,
        project_id: &ProjectId,
        scope: RustStyleScope,
        auto_policy: RustAutoPolicy,
    ) -> Result<RustStyleCheckResult, ApplicationError> {
        let _guard = self.command_guard()?;
        let project = self
            .repositories
            .global()
            .get_project(project_id)?
            .ok_or(ApplicationError::NotFound)?;
        let root = self.primary_project_root(&project)?;
        let (runtime_root, policy_path) = self.rust_style_runtime_paths()?;
        Ok(check_rust_style(
            project_id,
            &root,
            runtime_root,
            policy_path,
            scope,
            auto_policy,
        )?)
    }

    pub fn prepare_rust_style(
        &self,
        project_id: &ProjectId,
        scope: RustStyleScope,
        auto_policy: RustAutoPolicy,
    ) -> Result<PreparedRustStyleResult, ApplicationError> {
        let _guard = self.command_guard()?;
        self.prepare_rust_style_inner(project_id, scope, auto_policy)
    }

    fn prepare_rust_style_inner(
        &self,
        project_id: &ProjectId,
        scope: RustStyleScope,
        auto_policy: RustAutoPolicy,
    ) -> Result<PreparedRustStyleResult, ApplicationError> {
        self.prepare_rust_style_persisted(project_id, scope, auto_policy)
            .map(|(result, _)| result)
    }

    fn prepare_rust_style_persisted(
        &self,
        project_id: &ProjectId,
        scope: RustStyleScope,
        auto_policy: RustAutoPolicy,
    ) -> Result<(PreparedRustStyleResult, rust_style::RustStyleCandidate), ApplicationError> {
        let project = self
            .repositories
            .global()
            .get_project(project_id)?
            .ok_or(ApplicationError::NotFound)?;
        let root = self.primary_project_root(&project)?;
        let scan_key = format!(
            "rust-style-prepare-{}",
            star_contracts::ArtifactId::new().as_str()
        );
        let scan = self.scan_project_inner(project_id, &scan_key)?.scan_run;
        if scan.status != ScanStatus::Succeeded {
            return Err(ApplicationError::Invalid);
        }
        let (runtime_root, policy_path) = self.rust_style_runtime_paths()?;
        let prepared = prepare_rust_style(
            project_id,
            scan.workspace_snapshot_id.clone(),
            &root,
            runtime_root,
            policy_path,
            scope,
            auto_policy,
        )?;
        let candidate = prepared.candidate;
        let mut patch_set = candidate.patch_set.clone();
        let mut pre_apply_validation_result = None;
        let mut pre_apply_gate_decision = None;
        if let Some(patch) = patch_set.as_mut() {
            let forward = candidate
                .forward_artifact
                .as_ref()
                .ok_or(ApplicationError::Invalid)?;
            let reverse = candidate
                .reverse_artifact
                .as_ref()
                .ok_or(ApplicationError::Invalid)?;
            let forward_ref = self.artifacts.put_json(
                project_id,
                &root,
                &format!(
                    "management/rust-style/{}/forward.json",
                    patch.patch_set_id.as_str()
                ),
                "rust_style_patch_set",
                patch.patch_set_id.as_str(),
                forward,
            )?;
            let reverse_ref = self.artifacts.put_json(
                project_id,
                &root,
                &format!(
                    "management/rust-style/{}/reverse.json",
                    patch.patch_set_id.as_str()
                ),
                "rust_style_reverse_patch",
                patch.patch_set_id.as_str(),
                reverse,
            )?;
            if patch
                .patch_artifact_refs
                .first()
                .is_none_or(|expected| expected.sha256 != forward_ref.sha256)
                || patch
                    .rollback_artifact_refs
                    .first()
                    .is_none_or(|expected| expected.sha256 != reverse_ref.sha256)
            {
                return Err(ApplicationError::Invalid);
            }
            patch.patch_artifact_refs = vec![forward_ref];
            patch.rollback_artifact_refs = vec![reverse_ref];
            self.repositories
                .project(project_id)?
                .save_patch_set(patch)?;
            let (validation_result, gate_decision) = self.evaluate_and_save_patch_gate(
                project_id,
                patch,
                &scan,
                "star.validation.rust-style-pre-apply-v1",
            )?;
            pre_apply_validation_result = Some(validation_result);
            pre_apply_gate_decision = Some(gate_decision);
        }
        let result = PreparedRustStyleResult {
            inspection: prepared.inspection,
            state: format!("{:?}", candidate.state).to_ascii_lowercase(),
            candidate_fingerprint: candidate.candidate_fingerprint.clone(),
            before_fingerprint: candidate.before_fingerprint.clone(),
            expected_after_fingerprint: candidate.expected_after_fingerprint.clone(),
            idempotence_proved: candidate.idempotence_proved,
            changed_paths: candidate
                .changes
                .iter()
                .map(|change| change.path.clone())
                .collect(),
            patch_set,
            pre_apply_validation_result,
            pre_apply_gate_decision,
            isolation_ref: prepared.isolation_ref,
        };
        Ok((result, candidate))
    }

    pub fn auto_apply_rust_style(
        &self,
        project_id: &ProjectId,
        scope: RustStyleScope,
    ) -> Result<RustStyleAutoApplyResult, ApplicationError> {
        let _guard = self.command_guard()?;
        let (prepared, candidate) =
            self.prepare_rust_style_persisted(project_id, scope, RustAutoPolicy::PersonalAuto)?;
        let Some(patch_set) = prepared.patch_set.as_ref() else {
            return Ok(RustStyleAutoApplyResult {
                prepared,
                applied: None,
                permit_automatic: true,
            });
        };
        let project = self
            .repositories
            .global()
            .get_project(project_id)?
            .ok_or(ApplicationError::NotFound)?;
        let root = self.primary_project_root(&project)?;
        let grant = load_rust_style_auto_grant(&root)?;
        let pre_gate = match prepared
            .pre_apply_gate_decision
            .as_ref()
            .map(GateDecision::authoritative_state)
        {
            Some(AuthoritativeGateState::Passed) => rust_style::PreApplyGateVerdict::AutoPass,
            Some(AuthoritativeGateState::AwaitingHumanReview) => {
                rust_style::PreApplyGateVerdict::HumanReview
            }
            _ => rust_style::PreApplyGateVerdict::Block,
        };
        let mut permit = rust_style::authorize_personal_auto(
            &candidate,
            &prepared.inspection.policy,
            &grant,
            pre_gate,
            Utc::now(),
        )
        .map_err(|error| ApplicationError::RustStyle(error.into()))?;
        let mut port = ManagedRustSourceMutationPort {
            service: self,
            project_id,
            patch_set_id: patch_set.patch_set_id.clone(),
            approved_patch_fingerprint: patch_set.patch_fingerprint.as_str().to_owned(),
            result: None,
        };
        let state = rust_style::apply_with_permit(&candidate, &mut permit, &mut port)
            .map_err(|error| ApplicationError::RustStyle(error.into()))?;
        if state != rust_style::RustApplyState::Applied {
            return Err(ApplicationError::Apply(format!(
                "RUST_STYLE_AUTO_APPLY_{state:?}"
            )));
        }
        let applied = port.result.take().ok_or_else(|| {
            ApplicationError::Apply("RUST_STYLE_APPLY_OUTCOME_UNKNOWN".to_owned())
        })??;
        Ok(RustStyleAutoApplyResult {
            prepared,
            applied: Some(applied),
            permit_automatic: permit.automatic,
        })
    }

    fn rust_style_runtime_paths(&self) -> Result<(&Path, &Path), ApplicationError> {
        Ok((
            self.rust_style_runtime_root
                .as_deref()
                .ok_or(ApplicationError::Invalid)?,
            self.rust_style_policy_path
                .as_deref()
                .ok_or(ApplicationError::Invalid)?,
        ))
    }

    fn evaluate_and_save_patch_gate(
        &self,
        project_id: &ProjectId,
        patch_set: &PatchSet,
        scan: &ScanRun,
        validation_plan_ref: &str,
    ) -> Result<(ValidationResult, GateDecision), ApplicationError> {
        let repository = self.repositories.project(project_id)?;
        let findings = repository.list_findings()?;
        let mut occurrences = Vec::new();
        for finding in &findings {
            occurrences.extend(repository.occurrences_for_finding(&finding.finding_id)?);
        }
        let decisions = evaluate_decisions(
            project_id,
            &scan.project_revision_id,
            &scan.scan_config_fingerprint,
            &scan.rule_set_fingerprint,
            &findings,
            &occurrences,
            &repository.list_baselines()?,
            &repository.list_suppressions()?,
            &repository.list_dispositions()?,
            Utc::now(),
        );
        let (validation_result, gate_decision) = validate_patch_result_with_plan(
            patch_set,
            scan,
            &findings,
            &decisions,
            validation_plan_ref,
        )?;
        repository.save_validation(&validation_result, &gate_decision)?;
        Ok((validation_result, gate_decision))
    }

    pub fn prepare_patch(
        &self,
        project_id: &ProjectId,
        finding_id: &FindingId,
    ) -> Result<PreparedPatchResult, ApplicationError> {
        let _guard = self.command_guard()?;
        self.prepare_patch_inner(project_id, finding_id)
    }

    fn prepare_patch_inner(
        &self,
        project_id: &ProjectId,
        finding_id: &FindingId,
    ) -> Result<PreparedPatchResult, ApplicationError> {
        let project = self
            .repositories
            .global()
            .get_project(project_id)?
            .ok_or(ApplicationError::NotFound)?;
        let root = self.primary_project_root(&project)?;
        let repository = self.repositories.project(project_id)?;
        let finding = repository
            .get_finding(finding_id)?
            .ok_or(ApplicationError::NotFound)?;
        let occurrences = repository.occurrences_for_finding(finding_id)?;
        let latest_scan = repository
            .latest_scan()?
            .ok_or(ApplicationError::NotFound)?;
        let snapshot = repository
            .get_workspace_snapshot(&latest_scan.workspace_snapshot_id)?
            .ok_or(ApplicationError::NotFound)?;
        let prepared = prepare_trailing_whitespace_patch(&root, &finding, &occurrences, &snapshot)?;
        let artifact = self.artifacts.put_json(
            project_id,
            &root,
            &format!(
                "management/patches/{}/recipe.json",
                prepared.patch_set.patch_set_id.as_str()
            ),
            "patch_set",
            prepared.patch_set.patch_set_id.as_str(),
            &prepared.recipe_artifact,
        )?;
        let prepared = prepared.attach_artifact(artifact)?;
        repository.save_change_plan(&prepared.change_plan)?;
        repository.save_patch_set(&prepared.patch_set)?;
        Ok(PreparedPatchResult {
            change_plan_id: prepared.change_plan.change_plan_id.as_str().to_owned(),
            patch_set: prepared.patch_set,
        })
    }

    pub fn apply_patch(
        &self,
        project_id: &ProjectId,
        patch_set_id: &PatchSetId,
        approved_patch_fingerprint: &str,
    ) -> Result<AppliedPatchResult, ApplicationError> {
        let _guard = self.command_guard()?;
        self.apply_patch_inner(project_id, patch_set_id, approved_patch_fingerprint)
    }

    fn apply_patch_inner(
        &self,
        project_id: &ProjectId,
        patch_set_id: &PatchSetId,
        approved_patch_fingerprint: &str,
    ) -> Result<AppliedPatchResult, ApplicationError> {
        let project = self
            .repositories
            .global()
            .get_project(project_id)?
            .ok_or(ApplicationError::NotFound)?;
        let root = self.primary_project_root(&project)?;
        let repository = self.repositories.project(project_id)?;
        let patch_set = repository
            .get_patch_set(patch_set_id)?
            .ok_or(ApplicationError::NotFound)?;
        let patch_artifact = patch_set
            .patch_artifact_refs
            .first()
            .ok_or(ApplicationError::Invalid)?;
        let recipe = self.artifacts.read_json(&root, patch_artifact)?;
        let rust_style_patch = is_rust_style_patch_artifact(&recipe);
        let rust_style_binding = if rust_style_patch {
            let binding = rust_style_patch_binding(&recipe).map_err(|_| {
                ApplicationError::Apply("RUST_STYLE_PATCH_BINDING_INVALID".to_owned())
            })?;
            let (runtime_root, policy_path) = self.rust_style_runtime_paths()?;
            let scope = runtime_scope_from_patch(&binding.scope)?;
            let inspection = inspect_rust_style(
                project_id,
                &root,
                runtime_root,
                policy_path,
                scope.clone(),
                binding.auto_policy,
            )?;
            if !rust_style_binding_matches(&inspection, &binding) {
                return Err(ApplicationError::Apply(
                    "RUST_STYLE_PRE_GATE_STALE".to_owned(),
                ));
            }
            Some((binding, scope))
        } else {
            None
        };
        let applied = match if rust_style_patch {
            apply_rust_style_patch(patch_set, &root, &recipe, approved_patch_fingerprint)
        } else {
            apply_patch(patch_set, &root, &recipe, approved_patch_fingerprint)
        } {
            Ok(applied) => applied,
            Err(failure) => {
                repository.save_patch_set(&failure.patch_set)?;
                return Err(apply_failure(failure));
            }
        };
        repository.save_patch_set(&applied.patch_set)?;
        if let Some((binding, scope)) = rust_style_binding.as_ref() {
            let (runtime_root, policy_path) = match self.rust_style_runtime_paths() {
                Ok(paths) => paths,
                Err(_) => {
                    let reverted = rollback_applied(applied).map_err(apply_failure)?;
                    repository.save_patch_set(&reverted)?;
                    return Err(ApplicationError::Apply(
                        "RUST_STYLE_POST_GATE_UNAVAILABLE".to_owned(),
                    ));
                }
            };
            let post_check = check_rust_style(
                project_id,
                &root,
                runtime_root,
                policy_path,
                scope.clone(),
                binding.auto_policy,
            );
            let post_gate_passed = post_check.as_ref().is_ok_and(|check| {
                check.rustfmt.success
                    && check.clippy.success
                    && check.source_unchanged
                    && check.inspection.binding.completeness == RustCompleteness::Complete
                    && check.inspection.policy.policy_completeness == RustCompleteness::Complete
                    && check.inspection.coverage.completeness == RustCompleteness::Complete
                    && check.inspection.limitations.is_empty()
                    && rust_style_binding_matches(&check.inspection, binding)
            });
            if !post_gate_passed {
                let reverted = rollback_applied(applied).map_err(apply_failure)?;
                repository.save_patch_set(&reverted)?;
                return Err(ApplicationError::Apply(
                    "RUST_STYLE_POST_GATE_BLOCKED".to_owned(),
                ));
            }
        }
        let validation_scan_key = format!(
            "patch-validation-{}-{}",
            patch_set_id.as_str(),
            applied
                .patch_set
                .patch_fingerprint
                .as_str()
                .trim_start_matches("sha256:")
        );
        let scan = match self.scan_project_inner(project_id, &validation_scan_key) {
            Ok(scan) => scan.scan_run,
            Err(error) => {
                let reverted = rollback_applied(applied).map_err(apply_failure)?;
                repository.save_patch_set(&reverted)?;
                return Err(error);
            }
        };
        let validation_plan_ref = if rust_style_patch {
            "star.validation.rust-style-v1"
        } else {
            "star.validation.trailing-whitespace.v1"
        };
        let (validation_result, gate_decision) = self.evaluate_and_save_patch_gate(
            project_id,
            &applied.patch_set,
            &scan,
            validation_plan_ref,
        )?;
        let mut patch_set = applied.patch_set.clone();
        patch_set.applied_workspace_snapshot_id = Some(scan.workspace_snapshot_id.clone());
        let automatic_rollback =
            gate_decision.authoritative_state() != AuthoritativeGateState::Passed;
        if automatic_rollback {
            patch_set = rollback_applied(applied).map_err(apply_failure)?;
            repository.save_patch_set(&patch_set)?;
            let rollback_scan_key = format!(
                "patch-rollback-{}-{}",
                patch_set_id.as_str(),
                patch_set
                    .patch_fingerprint
                    .as_str()
                    .trim_start_matches("sha256:")
            );
            let _ = self.scan_project_inner(project_id, &rollback_scan_key);
        } else {
            repository.save_patch_set(&patch_set)?;
        }
        Ok(AppliedPatchResult {
            patch_set,
            validation_result,
            gate_decision,
            automatic_rollback,
        })
    }

    pub fn verify_stores(&self) -> Result<Vec<ManagementStoreStatus>, ApplicationError> {
        let _guard = self.command_guard()?;
        Ok(self.repositories.verify_all()?)
    }

    pub fn backup_stores(
        &self,
        destination: &Path,
    ) -> Result<Vec<ManagementStoreStatus>, ApplicationError> {
        let _guard = self.command_guard()?;
        Ok(self.repositories.backup_all(destination)?)
    }

    pub fn plan_retention(&self) -> Result<RetentionPlan, ApplicationError> {
        Ok(self.repositories.plan_retention()?)
    }

    pub fn apply_retention(
        &self,
        plan: &RetentionPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<RetentionApplyResult, ApplicationError> {
        let _guard = self.command_guard()?;
        Ok(self
            .repositories
            .apply_retention(plan, approved_plan_fingerprint)?)
    }

    pub fn apply_current_retention(
        &self,
        approved_plan_fingerprint: &str,
    ) -> Result<RetentionApplyResult, ApplicationError> {
        let plan = self.plan_retention()?;
        self.apply_retention(&plan, approved_plan_fingerprint)
    }

    pub fn plan_source_rebuild(&self) -> Result<SourceRebuildPlan, ApplicationError> {
        self.plan_source_rebuild_inner()
    }

    fn plan_source_rebuild_inner(&self) -> Result<SourceRebuildPlan, ApplicationError> {
        let mut project_ids = Vec::new();
        for attachment in self.root_bindings.list_attachments()? {
            if self
                .repositories
                .global()
                .get_project(&attachment.project_id)?
                .is_none()
            {
                project_ids.push(attachment.project_id);
            }
        }
        project_ids.sort();
        project_ids.dedup();
        let rebuildable_categories = vec![
            "project_directory".to_owned(),
            "project_revision".to_owned(),
            "workspace_snapshot".to_owned(),
            "source_graph".to_owned(),
            "scan_finding_projection".to_owned(),
            "shared_baseline_suppression_projection".to_owned(),
        ];
        let not_rebuildable_without_backup = vec![
            "local_suppression".to_owned(),
            "local_disposition".to_owned(),
            "decision_revision_history".to_owned(),
            "idempotency_history".to_owned(),
            "actor_and_event_timestamps".to_owned(),
            "in_progress_change_state".to_owned(),
        ];
        let plan_fingerprint = versioned_fingerprint(
            "star.source-rebuild-plan",
            1,
            &serde_json::json!({
                "project_ids":project_ids,
                "rebuildable_categories":rebuildable_categories,
                "not_rebuildable_without_backup":not_rebuildable_without_backup,
            }),
        )
        .map_err(|_| ApplicationError::Invalid)?;
        Ok(SourceRebuildPlan {
            schema_version: 1,
            project_ids,
            rebuildable_categories,
            not_rebuildable_without_backup,
            plan_fingerprint,
        })
    }

    pub fn apply_source_rebuild(
        &self,
        approved_plan_fingerprint: &str,
    ) -> Result<SourceRebuildResult, ApplicationError> {
        let _guard = self.command_guard()?;
        let plan = self.plan_source_rebuild_inner()?;
        if plan.plan_fingerprint.as_str() != approved_plan_fingerprint {
            return Err(ApplicationError::Repository(RepositoryError::new(
                RepositoryErrorCategory::RevisionConflict,
                "source rebuild approval is stale",
            )));
        }
        let mut projects = Vec::new();
        for project_id in &plan.project_ids {
            let attachment = self
                .root_bindings
                .find_by_project(project_id)?
                .ok_or(ApplicationError::NotFound)?;
            let root = self.root_bindings.resolve(&attachment.root_binding_id)?;
            let registration_key = format!("source-rebuild-register-{}", project_id.as_str());
            let registration = self.register_project_inner(&root, &registration_key)?;
            if registration.project.project_id != *project_id {
                return Err(ApplicationError::Invalid);
            }
            let scan_key = format!(
                "source-rebuild-scan-{}-{}",
                project_id.as_str(),
                plan.plan_fingerprint.as_str().trim_start_matches("sha256:")
            );
            let scan = self.scan_project_inner(project_id, &scan_key)?;
            projects.push(SourceRebuildProjectResult {
                project_id: project_id.clone(),
                scan_run_id: scan.scan_run.scan_run_id,
                scan_status: scan.scan_run.status,
                finding_count: scan.finding_count,
            });
        }
        Ok(SourceRebuildResult {
            plan_fingerprint: plan.plan_fingerprint,
            projects,
            not_rebuildable_without_backup: plan.not_rebuildable_without_backup,
        })
    }

    fn command_guard(&self) -> Result<std::sync::MutexGuard<'_, ()>, ApplicationError> {
        self.command_lock.lock().map_err(|_| {
            ApplicationError::Repository(RepositoryError::new(
                RepositoryErrorCategory::Unavailable,
                "management application writer lock is unavailable",
            ))
        })
    }
}

fn valid_idempotency_key(value: &str) -> bool {
    !value.trim().is_empty() && value.chars().count() <= 128 && !value.contains('\0')
}

fn runtime_scope_from_patch(
    scope: &RustStylePatchScope,
) -> Result<RustStyleScope, ApplicationError> {
    match scope {
        RustStylePatchScope::Workspace => Ok(RustStyleScope::workspace()),
        RustStylePatchScope::Package { package } => Ok(RustStyleScope::package(package.clone())?),
    }
}

fn rust_style_binding_matches(
    inspection: &RustStyleInspection,
    binding: &RustStylePatchBinding,
) -> bool {
    inspection.binding.completeness == RustCompleteness::Complete
        && inspection.policy.policy_completeness == RustCompleteness::Complete
        && inspection.coverage.completeness == RustCompleteness::Complete
        && inspection.limitations.is_empty()
        && inspection.binding.binding_fingerprint == binding.toolchain_fingerprint
        && inspection.policy.policy_fingerprint == binding.policy_fingerprint
        && inspection.coverage.coverage_fingerprint == binding.coverage_fingerprint
        && inspection.policy.fixed_adapter_definition_fingerprint
            == binding.fixed_adapter_fingerprint
        && inspection.policy.auto_policy == binding.auto_policy
}

fn load_rust_style_auto_grant(
    project_root: &Path,
) -> Result<rust_style::RustAutoApplyGrant, ApplicationError> {
    const MAX_GRANT_BYTES: u64 = 64 * 1024;
    let canonical_root = project_root
        .canonicalize()
        .map_err(|_| ApplicationError::Invalid)?;
    let path = canonical_root.join(".star-control/rust-style-auto-grant.json");
    let metadata = std::fs::symlink_metadata(&path).map_err(|_| ApplicationError::Invalid)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_GRANT_BYTES
    {
        return Err(ApplicationError::Invalid);
    }
    let canonical = path.canonicalize().map_err(|_| ApplicationError::Invalid)?;
    if !canonical.starts_with(&canonical_root) {
        return Err(ApplicationError::Invalid);
    }
    let bytes = std::fs::read(canonical).map_err(|_| ApplicationError::Invalid)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| ApplicationError::Invalid)?;
    let value = parse_no_duplicate_keys(text).map_err(|_| ApplicationError::Invalid)?;
    let source: RustStyleAutoGrantSource =
        serde_json::from_value(value).map_err(|_| ApplicationError::Invalid)?;
    if source.schema_version != 1
        || source.action != "apply_rust_style_patch"
        || source.scope_paths.is_empty()
        || source.max_files == 0
        || source.max_changed_bytes == 0
    {
        return Err(ApplicationError::Invalid);
    }
    let grant_fingerprint = versioned_fingerprint(
        "star.rust-style-auto-grant",
        1,
        &serde_json::json!({
            "project_id":source.project_id,
            "profile_ref":source.profile_ref,
            "pipeline_ref":source.pipeline_ref,
            "toolchain_fingerprint":source.toolchain_fingerprint,
            "style_policy_fingerprint":source.style_policy_fingerprint,
            "coverage_fingerprint":source.coverage_fingerprint,
            "scope_paths":source.scope_paths,
            "max_files":source.max_files,
            "max_changed_bytes":source.max_changed_bytes,
            "expires_at":source.expires_at,
        }),
    )
    .map_err(|_| ApplicationError::Invalid)?;
    Ok(rust_style::RustAutoApplyGrant {
        project_id: source.project_id,
        profile_ref: source.profile_ref,
        pipeline_ref: source.pipeline_ref,
        toolchain_fingerprint: source.toolchain_fingerprint,
        style_policy_fingerprint: source.style_policy_fingerprint,
        coverage_fingerprint: source.coverage_fingerprint,
        scope_paths: source.scope_paths,
        max_files: source.max_files,
        max_changed_bytes: source.max_changed_bytes,
        expires_at: source.expires_at,
        grant_fingerprint,
    })
}

fn registration_fingerprint_payload(
    project: &Project,
    checkout: &ProjectCheckout,
) -> serde_json::Value {
    serde_json::json!({
        "project": project,
        "checkout_id": checkout.checkout_id,
        "checkout_content_fingerprint": checkout.content_fingerprint,
    })
}

fn block_coordination(
    repository: &dyn GlobalManagementRepository,
    operation: &mut CoordinatedOperation,
    diagnostic: &str,
) -> Result<(), ApplicationError> {
    operation.state = CoordinationState::Blocked;
    if !operation
        .diagnostic_refs
        .iter()
        .any(|existing| existing == diagnostic)
    {
        operation.diagnostic_refs.push(diagnostic.to_owned());
    }
    operation.updated_at = Utc::now();
    repository.put_coordination(operation)?;
    Ok(())
}

fn store_point(status: &ManagementStoreStatus) -> StorePoint {
    StorePoint {
        store_id: status.store_id.clone(),
        generation: status.generation,
        revision: status.store_revision,
    }
}

fn index_query_result<T>(
    snapshot: &CodeIndexSnapshot,
    requested_tier: IndexTier,
    current: bool,
    items: Vec<T>,
    required_partition_kind: IndexPartitionKind,
) -> IndexQueryResult<T> {
    let relevant: Vec<_> = snapshot
        .partitions
        .iter()
        .filter(|partition| partition.kind == required_partition_kind)
        .collect();
    let tier_is_complete = !relevant.is_empty()
        && relevant.iter().all(|partition| {
            matches!(
                partition.state,
                IndexPartitionState::Succeeded | IndexPartitionState::Reused
            ) && partition.excluded_count == 0
                && partition
                    .used_tier
                    .is_some_and(|tier| tier >= requested_tier)
        });
    let used_tier = [IndexTier::Semantic, IndexTier::Syntax, IndexTier::Text]
        .into_iter()
        .find(|tier| {
            *tier <= requested_tier
                && snapshot.partitions.iter().any(|partition| {
                    partition.used_tier == Some(*tier)
                        && matches!(
                            partition.state,
                            IndexPartitionState::Succeeded | IndexPartitionState::Reused
                        )
                })
        })
        .unwrap_or(IndexTier::Text);
    let mut limitations: Vec<_> = relevant
        .iter()
        .flat_map(|partition| partition.limitations.iter())
        .map(|limitation| limitation.code.clone())
        .collect();
    limitations.sort();
    limitations.dedup();
    IndexQueryResult {
        snapshot_id: snapshot.code_index_snapshot_id.clone(),
        requested_tier,
        used_tier,
        current,
        confirmed_empty: items.is_empty() && current && tier_is_complete,
        limitations,
        items,
    }
}

fn index_identity_conflicts(previous: &CodeIndexSnapshot, current: &CodeIndexSnapshot) -> bool {
    previous.analysis_input_fingerprint == current.analysis_input_fingerprint
        && previous.content_fingerprint != current.content_fingerprint
}

fn apply_failure(failure: Box<ApplyFailure>) -> ApplicationError {
    ApplicationError::Apply(failure.code.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use star_contracts::{
        evidence::{
            ActorType, ArtifactKind, ArtifactManifest, ArtifactRef, GateScope, ObservedTool,
            OutputLimits, ProducerRef, RedactionStatus, RetentionClass,
        },
        evidence_v2::ValidationStabilityV2,
        ids::{ArtifactId, BaselineId, DispositionId, GoalId, RunId, SuppressionId},
        management::{
            BaselineScope, BaselineStatus, DispositionDecision, DispositionStatus, PatchSetStatus,
            SuppressionScope, SuppressionStatus,
        },
        planning::{
            BaselinePolicy, BaselinePolicyKind, IntendedChange, IntendedChangeKind,
            PlanningSelector, ProjectTarget, ProjectTargetRole, SelectorKind, SuccessCriterion,
            ValidationPlanV2Readiness, ValidationScopeLevel,
        },
    };
    use star_evidence::LocalArtifactStore;
    use star_state::{SqliteManagementRepositorySet, WindowsProjectRootBindingStore};

    struct FailingIndexCache;

    struct FixtureRustSyntaxAdapter;

    impl SyntaxAdapter for FixtureRustSyntaxAdapter {
        fn language_id(&self) -> &'static str {
            "rust"
        }

        fn fingerprint(&self) -> Sha256Hash {
            Sha256Hash::digest(b"fixture-rust-syntax-adapter")
        }

        fn analyze(
            &self,
            _source: &star_project::FileObservation,
        ) -> Result<star_project::index::SyntaxAnalysis, star_project::index::AdapterFailure>
        {
            Ok(star_project::index::SyntaxAnalysis::default())
        }
    }

    #[derive(Default)]
    struct PassingCheckExecutor {
        calls: usize,
    }

    impl CheckExecutor for PassingCheckExecutor {
        fn execute(
            &mut self,
            _invocation: &star_contracts::evidence_v2::TaskInvocationV2,
        ) -> Result<
            star_validation::runner::CheckExecutionObservation,
            star_validation::runner::CheckExecutorError,
        > {
            self.calls += 1;
            let now = Utc::now();
            Ok(star_validation::runner::CheckExecutionObservation {
                started_at: now,
                finished_at: now,
                exit_code: Some(0),
                termination_reason: star_contracts::evidence::TerminationReason::Exited,
                completeness: star_contracts::evidence::Completeness::Complete,
                stability: ValidationStabilityV2::Stable,
                artifact_refs: vec![],
                observed_tool: Some(ObservedTool {
                    executable_path: "registered://fixture-validator".to_owned(),
                    version: "1.0.0".to_owned(),
                    sha256: Sha256Hash::digest(b"fixture-validator"),
                }),
                diagnostics: vec![],
            })
        }
    }

    impl CodeIndexCache for FailingIndexCache {
        fn load(
            &self,
            _project_id: &ProjectId,
            _cache_key: &Sha256Hash,
        ) -> Result<Option<StoredCodeIndexProjection>, RepositoryError> {
            Err(RepositoryError::new(
                RepositoryErrorCategory::Corrupt,
                "fixture cache is corrupt",
            ))
        }

        fn store(
            &self,
            _project_id: &ProjectId,
            _cache_key: &Sha256Hash,
            _projection: &StoredCodeIndexProjection,
        ) -> Result<(), RepositoryError> {
            Err(RepositoryError::new(
                RepositoryErrorCategory::QuotaExceeded,
                "fixture cache is full",
            ))
        }
    }

    #[test]
    fn same_index_analysis_input_with_different_content_is_a_conflict() {
        let previous: CodeIndexSnapshot = serde_json::from_str(include_str!(
            "../../../../specs/fixtures/management/v1/code-index-snapshot/minimal.json"
        ))
        .unwrap();
        let mut current = previous.clone();
        current.content_fingerprint = Sha256Hash::digest(b"nondeterministic-content");
        assert!(index_identity_conflicts(&previous, &current));
        current.analysis_input_fingerprint = Sha256Hash::digest(b"different-input");
        assert!(!index_identity_conflicts(&previous, &current));
    }

    #[test]
    fn cli_only_service_runs_register_scan_patch_validation_without_ai_dependencies() {
        let root = std::env::temp_dir().join(format!(
            "star-application-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        let source = root.join("source");
        std::fs::create_dir_all(source.join("src")).unwrap();
        std::fs::create_dir_all(source.join(".star-control")).unwrap();
        let declared_project_id = ProjectId::new();
        std::fs::write(
            source.join(".star-control/project.toml"),
            format!(
                "schema_version = 1\nproject_id = \"{}\"\ndisplay_name = \"fixture-project\"\nrepository_kind = \"none\"\nsource_of_truth = [\"source\"]\n",
                declared_project_id.as_str()
            ),
        )
        .unwrap();
        std::fs::write(source.join("src/lib.rs"), b"fn main() {}  \n").unwrap();
        std::fs::write(source.join("user-change.txt"), b"preserve\n").unwrap();
        let repositories =
            Arc::new(SqliteManagementRepositorySet::open(root.join("management"), "test").unwrap());
        let bindings =
            Arc::new(WindowsProjectRootBindingStore::open(root.join("root-bindings")).unwrap());
        let mut service = ManagementApplicationService::new(
            repositories,
            bindings.clone(),
            Arc::new(LocalArtifactStore::default()),
        )
        .with_index_cache(Arc::new(FailingIndexCache));
        let registration = service
            .register_project(&source.canonicalize().unwrap(), "register-test")
            .unwrap();
        assert_eq!(
            registration.coordinated_operation.state,
            CoordinationState::Completed
        );
        let replayed_registration = service
            .register_project(&source.canonicalize().unwrap(), "register-test")
            .unwrap();
        assert_eq!(
            replayed_registration.project.project_id,
            registration.project.project_id
        );
        assert_eq!(
            replayed_registration
                .coordinated_operation
                .coordinated_operation_id,
            registration.coordinated_operation.coordinated_operation_id
        );
        assert_eq!(
            std::fs::read_dir(root.join("root-bindings"))
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry
                    .path()
                    .extension()
                    .is_some_and(|value| value == "binding"))
                .count(),
            1
        );
        let other_source = root.join("other-source");
        std::fs::create_dir_all(&other_source).unwrap();
        let conflict = service
            .register_project(&other_source.canonicalize().unwrap(), "register-test")
            .unwrap_err();
        assert!(matches!(
            conflict,
            ApplicationError::Repository(RepositoryError {
                category: RepositoryErrorCategory::IdempotencyConflict,
                ..
            })
        ));
        let project_id = registration.project.project_id;
        assert!(matches!(
            service.scan_project(&project_id, &"x".repeat(129)),
            Err(ApplicationError::Invalid)
        ));
        let scan = service.scan_project(&project_id, "scan-test").unwrap();
        assert_eq!(scan.scan_run.status, ScanStatus::Succeeded);
        let replayed_scan = service.scan_project(&project_id, "scan-test").unwrap();
        assert_eq!(
            replayed_scan.scan_run.scan_run_id,
            scan.scan_run.scan_run_id
        );
        let cache_scan = service.scan_project(&project_id, "scan-cache").unwrap();
        assert_eq!(cache_scan.scan_run.status, ScanStatus::Succeeded);
        assert_eq!(
            cache_scan
                .code_index_snapshot
                .as_ref()
                .unwrap()
                .code_index_snapshot_id,
            scan.code_index_snapshot
                .as_ref()
                .unwrap()
                .code_index_snapshot_id
        );
        assert!(
            cache_scan
                .code_index_snapshot
                .as_ref()
                .unwrap()
                .partitions
                .iter()
                .filter(|partition| {
                    partition.kind == star_contracts::index::IndexPartitionKind::Text
                })
                .all(|partition| partition.state == IndexPartitionState::Reused)
        );
        assert!(service.index_status(&project_id).unwrap().current);
        let text_result = service
            .index_search(&project_id, "main", IndexTier::Text, true)
            .unwrap();
        assert!(!text_result.items.is_empty());
        let definition_result = service
            .index_definitions(&project_id, "main", true)
            .unwrap();
        assert!(definition_result.items.is_empty());
        assert!(!definition_result.confirmed_empty);
        assert!(
            definition_result
                .limitations
                .iter()
                .any(|code| code == "INDEX_LANGUAGE_UNSUPPORTED")
        );
        let semantic_result = service
            .index_search(&project_id, "missing", IndexTier::Semantic, true)
            .unwrap();
        assert!(semantic_result.items.is_empty());
        assert!(!semantic_result.confirmed_empty);
        let index_status = service.index_status(&project_id).unwrap();
        assert!(index_status.current);
        assert!(index_status.snapshot.freshness.iter().any(|proof| {
            proof.partition_key.ends_with(":semantic")
                && proof.state == IndexFreshnessState::Unavailable
        }));
        let task = TaskSpecDraft {
            title: "Update the fixture source".to_owned(),
            objective: "Apply a bounded source change and validate it".to_owned(),
            project_targets: vec![ProjectTarget {
                project_id: project_id.clone(),
                checkout_id: registration.checkout.checkout_id.clone(),
                role: ProjectTargetRole::PlannedChange,
                reason: "fixture target".to_owned(),
            }],
            included_scope: vec![PlanningSelector {
                kind: SelectorKind::Path,
                value: "src/lib.rs".to_owned(),
            }],
            excluded_scope: vec![],
            intended_changes: vec![IntendedChange {
                change_id: "change-source".to_owned(),
                selector: PlanningSelector {
                    kind: SelectorKind::Path,
                    value: "src/lib.rs".to_owned(),
                },
                change_kind: IntendedChangeKind::Modify,
                intended_postcondition: "source remains valid".to_owned(),
            }],
            success_criteria: vec![SuccessCriterion {
                criterion_id: "validated".to_owned(),
                description: "all affected checks pass".to_owned(),
                verification: "sealed validation plan".to_owned(),
                required: true,
            }],
            constraints: vec!["project relative only".to_owned()],
            forbidden_actions: vec!["remote publish".to_owned()],
            baseline_policy: BaselinePolicy {
                kind: BaselinePolicyKind::CurrentWorkspace,
                reference: None,
            },
            requested_checks: vec![],
            check_overrides: vec![],
            assumptions: vec![],
        };
        let check_descriptors = ["format", "lint", "build", "test", "project_full"]
            .into_iter()
            .map(|family| {
                star_planning::descriptor(
                    &format!("fixture.{family}"),
                    family,
                    vec![
                        ValidationScopeLevel::Package,
                        ValidationScopeLevel::Workspace,
                        ValidationScopeLevel::ProjectFull,
                    ],
                    vec![star_contracts::index::SourceClass::Source],
                    vec!["--scope".to_owned(), "{scope}".to_owned()],
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let actor = ActorRef {
            actor_type: ActorType::User,
            actor_id: "fixture-user".to_owned(),
            display_name: "Fixture User".to_owned(),
            auth_source: "fixture".to_owned(),
        };
        let planning = service
            .create_planning_bundle(
                task.clone(),
                actor.clone(),
                check_descriptors.clone(),
                "planning-test",
            )
            .unwrap();
        assert_eq!(
            planning.validation_plan.readiness,
            ValidationPlanV2Readiness::Ready
        );
        assert!(!planning.validation_plan.required_checks.is_empty());
        let replayed = service
            .create_planning_bundle(
                task.clone(),
                actor.clone(),
                check_descriptors.clone(),
                "planning-test",
            )
            .unwrap();
        assert_eq!(replayed.bundle_fingerprint, planning.bundle_fingerprint);
        assert_eq!(
            service
                .get_planning_bundle(&planning.task_spec.task_spec_id)
                .unwrap()
                .bundle_fingerprint,
            planning.bundle_fingerprint
        );
        let executable_bindings = planning
            .validation_plan
            .required_checks
            .iter()
            .map(|check| ExecutableBinding {
                check_id: check.check_id.clone(),
                check_ref: star_contracts::evidence::CatalogRef {
                    catalog_id: check.check_id.clone(),
                    format_version: 1,
                    item_version: "1.0.0".to_owned(),
                    sha256: Sha256Hash::digest(check.check_id.as_bytes()),
                },
                tool_ref: star_contracts::evidence::CatalogRef {
                    catalog_id: "fixture.validator".to_owned(),
                    format_version: 1,
                    item_version: "1.0.0".to_owned(),
                    sha256: Sha256Hash::digest(b"fixture.validator"),
                },
                logical_executable: check.invocation.logical_executable.clone(),
                executable_binding_fingerprint: Sha256Hash::digest(b"fixture-binding"),
                cwd: ProjectPathRef::parse("src").unwrap(),
                permission_action: "local_write".to_owned(),
                output_limits: OutputLimits {
                    stdout_bytes: 1024,
                    stderr_bytes: 1024,
                    artifact_bytes: 4096,
                },
            })
            .collect::<Vec<_>>();
        let manifest_ref = ArtifactRef {
            artifact_id: ArtifactId::new(),
            kind: ArtifactKind::Manifest,
            project_id: Some(project_id.clone()),
            relative_path: ".ai-runs/star-control/fixture/manifest.json".to_owned(),
            media_type: "application/json".to_owned(),
            size_bytes: 2,
            sha256: Sha256Hash::digest(b"{}"),
            created_at: Utc::now(),
            producer: ProducerRef {
                component: "fixture".to_owned(),
                product_version: "0.1.0".to_owned(),
                build_id: "fixture".to_owned(),
                platform: "windows-x64".to_owned(),
            },
            redaction_status: RedactionStatus::NotNeeded,
            retention_class: RetentionClass::Evidence,
            source_artifact_ref: None,
        };
        let mut check_executor = PassingCheckExecutor::default();
        let execution = service
            .execute_planning_bundle(
                &planning.task_spec.task_spec_id,
                &executable_bindings,
                CheckGraphRunContext {
                    gate_scope: GateScope::Goal {
                        goal_id: GoalId::new(),
                        run_id: RunId::new(),
                        revision: 1,
                    },
                    decided_by: actor.clone(),
                    artifact_manifest: ArtifactManifest {
                        manifest_ref,
                        artifacts: vec![],
                    },
                    force_human_review: false,
                },
                &mut check_executor,
            )
            .unwrap();
        assert_eq!(
            execution.gate_decision.authoritative_state(),
            AuthoritativeGateState::Passed
        );
        assert_eq!(
            service
                .repositories
                .project(&project_id)
                .unwrap()
                .get_evidence_bundle_v2(&execution.evidence_bundle.evidence_bundle_id)
                .unwrap()
                .unwrap()
                .bundle_fingerprint,
            execution.evidence_bundle.bundle_fingerprint
        );
        let mut conflicting_task = task;
        conflicting_task.objective = "different idempotency input".to_owned();
        assert!(matches!(
            service.create_planning_bundle(
                conflicting_task,
                actor,
                check_descriptors,
                "planning-test"
            ),
            Err(ApplicationError::Repository(RepositoryError {
                category: RepositoryErrorCategory::IdempotencyConflict,
                ..
            }))
        ));
        std::fs::write(source.join("stale-probe.txt"), b"new source\n").unwrap();
        assert!(!service.index_status(&project_id).unwrap().current);
        std::fs::remove_file(source.join("stale-probe.txt")).unwrap();
        assert!(service.index_status(&project_id).unwrap().current);
        service.index_policy.required_tier = IndexTier::Semantic;
        let incomplete = service
            .scan_project(&project_id, "scan-required-semantic")
            .unwrap();
        assert_eq!(incomplete.scan_run.status, ScanStatus::Incomplete);
        assert_eq!(
            service
                .repositories
                .project(&project_id)
                .unwrap()
                .latest_scan()
                .unwrap()
                .unwrap()
                .scan_run_id,
            cache_scan.scan_run.scan_run_id
        );
        service.index_policy.required_tier = IndexTier::Text;
        let findings = service.list_findings(&project_id).unwrap();
        assert_eq!(findings.len(), 1);
        let now = Utc::now();
        let baseline_id = BaselineId::new();
        let finding_fingerprints = vec![findings[0].finding_fingerprint.clone()];
        let baseline = Baseline {
            schema_id: "star.baseline".to_owned(),
            schema_version: 1,
            baseline_id: baseline_id.clone(),
            revision: 1,
            scope_kind: BaselineScope::Shared,
            project_id: project_id.clone(),
            project_revision_id: scan.scan_run.project_revision_id.clone(),
            workspace_snapshot_id: scan.scan_run.workspace_snapshot_id.clone(),
            scan_config_fingerprint: scan.scan_run.scan_config_fingerprint.clone(),
            rule_set_fingerprint: scan.scan_run.rule_set_fingerprint.clone(),
            finding_fingerprints: finding_fingerprints.clone(),
            set_fingerprint: versioned_fingerprint(
                "star.baseline-finding-set",
                1,
                &finding_fingerprints,
            )
            .unwrap(),
            created_at: now,
            reason: "reviewed-existing-finding".to_owned(),
            reviewed: true,
            status: BaselineStatus::Active,
        };
        let suppression_id = SuppressionId::new();
        let suppression = Suppression {
            schema_id: "star.suppression".to_owned(),
            schema_version: 1,
            suppression_id: suppression_id.clone(),
            revision: 1,
            scope_kind: SuppressionScope::Shared,
            project_id: project_id.clone(),
            selector: format!("finding:{}", findings[0].finding_fingerprint),
            reason_code: "REVIEWED_LOCAL_EXCEPTION".to_owned(),
            reason: "temporary-local-review".to_owned(),
            created_at: now,
            expires_at: Some(now + Duration::days(90)),
            permanent: false,
            justification: None,
            source_revision_constraint: None,
            config_fingerprint_constraint: Some(scan.scan_run.scan_config_fingerprint.clone()),
            status: SuppressionStatus::Active,
            provenance: "git:.star-control/suppressions.toml".to_owned(),
        };
        #[derive(Serialize)]
        struct SharedSuppressionsFixture {
            schema_version: u32,
            suppressions: Vec<Suppression>,
        }
        std::fs::write(
            source.join(".star-control/suppressions.toml"),
            toml::to_string(&SharedSuppressionsFixture {
                schema_version: 1,
                suppressions: vec![suppression],
            })
            .unwrap(),
        )
        .unwrap();
        std::fs::create_dir_all(source.join(".star-control/baselines")).unwrap();
        std::fs::write(
            source.join(".star-control/baselines/reviewed.toml"),
            toml::to_string(&baseline).unwrap(),
        )
        .unwrap();
        let disposition_id = DispositionId::new();
        service
            .put_disposition(
                &project_id,
                &Disposition {
                    schema_id: "star.disposition".to_owned(),
                    schema_version: 1,
                    disposition_id: disposition_id.clone(),
                    revision: 1,
                    finding_id: findings[0].finding_id.clone(),
                    finding_fingerprint: findings[0].finding_fingerprint.clone(),
                    decision: DispositionDecision::NeedsAction,
                    reason_code: "LOCAL_TRIAGE".to_owned(),
                    reason: "confirmed-action-needed".to_owned(),
                    scope_revision: None,
                    expires_at: None,
                    duplicate_of_finding_id: None,
                    decided_at: now,
                    provenance: "local:event".to_owned(),
                    status: DispositionStatus::Active,
                },
                0,
            )
            .unwrap();
        let decision_scan = service.scan_project(&project_id, "scan-decisions").unwrap();
        assert_eq!(decision_scan.scan_run.status, ScanStatus::Succeeded);
        let findings = service.list_findings(&project_id).unwrap();
        assert_eq!(findings[0].active_suppression_ids, vec![suppression_id]);
        assert_eq!(findings[0].active_disposition_id, Some(disposition_id));
        let prepared = service
            .prepare_patch(&project_id, &findings[0].finding_id)
            .unwrap();
        let approval = prepared.patch_set.patch_fingerprint.as_str().to_owned();
        let applied = service
            .apply_patch(&project_id, &prepared.patch_set.patch_set_id, &approval)
            .unwrap();
        assert_eq!(applied.patch_set.status, PatchSetStatus::Applied);
        assert_eq!(applied.gate_decision.decision, GateDecisionKind::AutoPass);
        assert_eq!(
            applied.gate_decision.authoritative_state(),
            AuthoritativeGateState::Passed
        );
        let management_gate = applied
            .gate_decision
            .extensions
            .get("star.management")
            .and_then(serde_json::Value::as_object)
            .unwrap();
        assert_eq!(
            management_gate["baseline_ids"][0].as_str(),
            Some(baseline_id.as_str())
        );
        assert!(
            management_gate["reason_codes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|reason| reason.as_str() == Some("STALE_DECISION_IGNORED"))
        );
        assert!(!applied.automatic_rollback);
        assert_eq!(
            std::fs::read(source.join("src/lib.rs")).unwrap(),
            b"fn main() {}\n"
        );
        assert_eq!(
            std::fs::read(source.join("user-change.txt")).unwrap(),
            b"preserve\n"
        );
        drop(service);
        let rebuilt = ManagementApplicationService::new(
            Arc::new(
                SqliteManagementRepositorySet::open(root.join("rebuilt-management"), "test")
                    .unwrap(),
            ),
            Arc::new(WindowsProjectRootBindingStore::open(root.join("root-bindings")).unwrap()),
            Arc::new(LocalArtifactStore::default()),
        );
        let rebuild_plan = rebuilt.plan_source_rebuild().unwrap();
        assert_eq!(rebuild_plan.project_ids, vec![project_id.clone()]);
        let rebuild_result = rebuilt
            .apply_source_rebuild(rebuild_plan.plan_fingerprint.as_str())
            .unwrap();
        assert_eq!(rebuild_result.projects.len(), 1);
        assert_eq!(rebuild_result.projects[0].project_id, project_id);
        assert_eq!(rebuild_result.projects[0].finding_count, 0);
        assert!(
            rebuild_result
                .not_rebuildable_without_backup
                .contains(&"local_disposition".to_owned())
        );
    }

    #[test]
    fn personal_auto_rust_style_uses_persisted_pre_and_post_gates() {
        let root =
            std::env::temp_dir().join(format!("rsa-{}-{}", std::process::id(), ProjectId::new()));
        let source = root.join("source");
        std::fs::create_dir_all(source.join("src")).unwrap();
        std::fs::create_dir_all(source.join(".star-control")).unwrap();
        let declared_project_id = ProjectId::new();
        std::fs::write(
            source.join(".star-control/project.toml"),
            format!(
                "schema_version = 1\nproject_id = \"{}\"\ndisplay_name = \"rust-style-fixture\"\nrepository_kind = \"none\"\nsource_of_truth = [\"source\"]\n",
                declared_project_id.as_str()
            ),
        )
        .unwrap();
        std::fs::write(
            source.join("Cargo.toml"),
            "[package]\nname = \"rust-style-service-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\nrust-version = \"1.96\"\n",
        )
        .unwrap();
        std::fs::write(
            source.join("Cargo.lock"),
            "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n[[package]]\nname = \"rust-style-service-fixture\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::write(
            source.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.96.0\"\nprofile = \"minimal\"\ncomponents = [\"rustfmt\", \"clippy\"]\n",
        )
        .unwrap();
        let original = b"pub fn answer( )->u32{42}\n";
        std::fs::write(source.join("src/lib.rs"), original).unwrap();
        std::fs::write(source.join("user-change.txt"), b"preserve\n").unwrap();

        let policy_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .unwrap()
            .join("catalog/policies/rust-style.toml");
        let service = ManagementApplicationService::new(
            Arc::new(SqliteManagementRepositorySet::open(root.join("management"), "test").unwrap()),
            Arc::new(WindowsProjectRootBindingStore::open(root.join("root-bindings")).unwrap()),
            Arc::new(LocalArtifactStore::default()),
        )
        .with_syntax_adapter(Arc::new(FixtureRustSyntaxAdapter))
        .with_rust_style_runtime(root.join("runtime"), policy_path);
        let registration = service
            .register_project(&source.canonicalize().unwrap(), "register-rust-style")
            .unwrap();
        let project_id = registration.project.project_id;
        let scope = RustStyleScope::package("rust-style-service-fixture".to_owned()).unwrap();
        let inspection = service
            .inspect_rust_style(&project_id, scope.clone(), RustAutoPolicy::PersonalAuto)
            .unwrap();
        let mut grant = inspection.standing_grant_template.unwrap();
        grant["expires_at"] =
            serde_json::Value::String((Utc::now() + Duration::hours(1)).to_rfc3339());
        std::fs::write(
            source.join(".star-control/rust-style-auto-grant.json"),
            serde_json::to_vec_pretty(&grant).unwrap(),
        )
        .unwrap();

        let scan = service
            .scan_project(&project_id, "rust-style-preflight")
            .unwrap();
        assert_eq!(scan.scan_run.status, ScanStatus::Succeeded);
        assert!(
            scan.scan_run
                .limitations
                .iter()
                .any(|limitation| limitation == "INDEX_SEMANTIC_UNAVAILABLE")
        );
        let result = service
            .auto_apply_rust_style(&project_id, scope.clone())
            .unwrap();
        assert!(result.permit_automatic);
        assert_eq!(
            result
                .prepared
                .pre_apply_validation_result
                .as_ref()
                .unwrap()
                .validation_plan_ref,
            "star.validation.rust-style-pre-apply-v1"
        );
        assert_eq!(
            result
                .prepared
                .pre_apply_gate_decision
                .as_ref()
                .unwrap()
                .authoritative_state(),
            AuthoritativeGateState::Passed
        );
        let applied = result.applied.unwrap();
        assert_eq!(applied.patch_set.status, PatchSetStatus::Applied);
        assert_eq!(
            applied.validation_result.validation_plan_ref,
            "star.validation.rust-style-v1"
        );
        assert_eq!(
            applied.gate_decision.authoritative_state(),
            AuthoritativeGateState::Passed
        );
        assert!(!applied.automatic_rollback);
        assert_eq!(
            std::fs::read_to_string(source.join("src/lib.rs")).unwrap(),
            "pub fn answer() -> u32 {\n    42\n}\n"
        );
        assert_eq!(
            std::fs::read(source.join("user-change.txt")).unwrap(),
            b"preserve\n"
        );

        let second = service.auto_apply_rust_style(&project_id, scope).unwrap();
        assert!(second.applied.is_none());
        assert!(second.prepared.idempotence_proved);
        assert_eq!(second.prepared.state, "succeedednochange");

        std::fs::write(source.join("src/lib.rs"), original).unwrap();
        let stale_candidate = service
            .prepare_rust_style(
                &project_id,
                RustStyleScope::package("rust-style-service-fixture".to_owned()).unwrap(),
                RustAutoPolicy::SafeDefault,
            )
            .unwrap();
        let stale_patch = stale_candidate.patch_set.unwrap();
        std::fs::create_dir_all(source.join(".cargo")).unwrap();
        std::fs::write(source.join(".cargo/config.toml"), "[net]\noffline = true\n").unwrap();
        assert!(matches!(
            service.apply_patch(
                &project_id,
                &stale_patch.patch_set_id,
                stale_patch.patch_fingerprint.as_str(),
            ),
            Err(ApplicationError::Apply(code)) if code == "RUST_STYLE_PRE_GATE_STALE"
        ));
        assert_eq!(std::fs::read(source.join("src/lib.rs")).unwrap(), original);
    }
}
