//! Shared CLI and future Codex management application service.

use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use chrono::Utc;
use serde::Serialize;
#[cfg(test)]
use star_contracts::evidence::GateDecisionKind;
use star_contracts::{
    Sha256Hash,
    evidence::{AuthoritativeGateState, GateDecision},
    ids::{CoordinatedOperationId, FindingId, GenerationId, PatchSetId, ProjectId, ScanRunId},
    management::{
        Baseline, CoordinatedOperation, CoordinationParticipant, CoordinationState, Disposition,
        Finding, ManagementStoreStatus, ParticipantState, PatchSet, Project, ProjectStorePoint,
        ScanRun, ScanStatus, StorePoint, StoreVersionVector, Suppression, ValidationResult,
    },
};
use star_domain::versioned_fingerprint;
use star_execution::{
    ApplyFailure, ExecutionError, apply_patch, prepare_trailing_whitespace_patch, rollback_applied,
};
use star_ports::{
    ArtifactStore, GlobalManagementRepository, ManagementRepositorySet, ProjectRootBindingStore,
    RepositoryError, RepositoryErrorCategory, RetentionApplyResult, RetentionPlan, ScanCommit,
};
use star_project::{
    ProjectError, ProjectSeed, ScanPolicy, SharedDecisionDeclarations, load_shared_decisions,
    observe_project,
};
pub use star_validation::planning::{
    AiEvidenceSummary, AiValidationRunSummary, CacheMissReason, CacheReuseDecision,
    CacheValidationStability, EvidenceCompressionError, UnitDependency, ValidationCacheCandidate,
    ValidationCheckDefinition, ValidationEvidenceDiagnostic, ValidationEvidenceRun,
    ValidationPlanningError, ValidationPlanningInput, build_validation_plan,
    compress_evidence_for_ai, evaluate_cache_reuse,
};
use star_validation::{
    ValidationError, analyze_trailing_whitespace, apply_decision_projection, evaluate_decisions,
    validate_patch_result,
};
use thiserror::Error;

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
    #[error("finding or gate evaluation failed")]
    Validation(#[from] ValidationError),
    #[error("patch preparation failed")]
    Execution(#[from] ExecutionError),
    #[error("patch apply failed: {0}")]
    Apply(String),
}

#[derive(Clone, Debug, Serialize)]
pub struct RegisterProjectResult {
    pub project: Project,
    pub coordinated_operation: CoordinatedOperation,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScanProjectResult {
    pub scan_run: ScanRun,
    pub finding_count: usize,
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

pub struct ManagementApplicationService {
    repositories: Arc<dyn ManagementRepositorySet>,
    root_bindings: Arc<dyn ProjectRootBindingStore>,
    artifacts: Arc<dyn ArtifactStore>,
    scan_policy: ScanPolicy,
    command_lock: Mutex<()>,
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
            command_lock: Mutex::new(()),
        }
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
        let binding_id = self
            .root_bindings
            .attach(&seed.project_id, &canonical_root)?;
        let project = seed.attach(binding_id);
        let global_before = self.repositories.global().status()?;
        let input_fingerprint = versioned_fingerprint("star.command.project-register", 1, &project)
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
            1,
            &project,
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
        let participant_result =
            versioned_fingerprint("star.coordination.project-register.result", 1, &project)
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
        let candidate = seed.attach(attachment.root_binding_id);
        let input_fingerprint =
            versioned_fingerprint("star.command.project-register", 1, &candidate)
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
        Ok(RegisterProjectResult {
            project,
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
            let project = if let Some(project) = project_repository.get_project()? {
                project
            } else {
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
                seed.attach(attachment.root_binding_id)
            };
            let input_fingerprint =
                versioned_fingerprint("star.command.project-register", 1, &project)
                    .map_err(|_| ApplicationError::Invalid)?;
            let participant_payload = versioned_fingerprint(
                "star.coordination.project-register.participant",
                1,
                &project,
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
            let participant_result =
                versioned_fingerprint("star.coordination.project-register.result", 1, &project)
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
        let binding_id = project
            .root_binding_id
            .as_ref()
            .ok_or(ApplicationError::Invalid)?;
        let root = self.root_bindings.resolve(binding_id)?;
        let observation = observe_project(&project, &root, &self.scan_policy)?;
        let mut scan_complete =
            observation.completeness == star_contracts::management::Completeness::Complete;
        let mut scan_limitations = observation.limitations.clone();
        let scan_run_id = ScanRunId::new();
        let workspace_snapshot_id = observation.workspace_snapshot_id(project_id)?;
        let (sources, symbols) =
            observation.source_graph(project_id, &workspace_snapshot_id, &scan_run_id)?;
        let mut projection = analyze_trailing_whitespace(
            project_id,
            &observation.revision,
            &workspace_snapshot_id,
            &scan_run_id,
            &observation.files,
            &sources,
            &symbols,
        )?;
        let repository = self.repositories.project(project_id)?;
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
                "scanner_contract_version":1,
            }),
        )
        .map_err(|_| ApplicationError::Invalid)?;
        if let Some(scan_run) = repository.replay_scan(idempotency_key, &input_fingerprint)? {
            return Ok(ScanProjectResult {
                scan_run,
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
        let status = if scan_complete {
            ScanStatus::Succeeded
        } else {
            ScanStatus::Incomplete
        };
        let mut counts = std::collections::BTreeMap::new();
        counts.insert("source".to_owned(), sources.len() as u64);
        counts.insert("symbol".to_owned(), symbols.len() as u64);
        counts.insert("reference".to_owned(), 0);
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
            generation_id: GenerationId::new(),
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
        let commit = ScanCommit {
            project,
            revision: observation.revision,
            snapshot,
            run: scan_run.clone(),
            sources,
            symbols,
            references: vec![],
            findings: projection.findings,
            occurrences: projection.occurrences,
            idempotency_key: idempotency_key.to_owned(),
            payload_fingerprint: input_fingerprint,
        };
        let committed_run = repository.commit_scan(&commit)?;
        Ok(ScanProjectResult {
            scan_run: committed_run,
            finding_count: repository.list_findings()?.len().max(finding_count),
        })
    }

    pub fn list_findings(&self, project_id: &ProjectId) -> Result<Vec<Finding>, ApplicationError> {
        Ok(self.repositories.project(project_id)?.list_findings()?)
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
        let root = self.root_bindings.resolve(
            project
                .root_binding_id
                .as_ref()
                .ok_or(ApplicationError::Invalid)?,
        )?;
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
        let root = self.root_bindings.resolve(
            project
                .root_binding_id
                .as_ref()
                .ok_or(ApplicationError::Invalid)?,
        )?;
        let repository = self.repositories.project(project_id)?;
        let patch_set = repository
            .get_patch_set(patch_set_id)?
            .ok_or(ApplicationError::NotFound)?;
        let patch_artifact = patch_set
            .patch_artifact_refs
            .first()
            .ok_or(ApplicationError::Invalid)?;
        let recipe = self.artifacts.read_json(&root, patch_artifact)?;
        let applied = match apply_patch(patch_set, &root, &recipe, approved_patch_fingerprint) {
            Ok(applied) => applied,
            Err(failure) => {
                repository.save_patch_set(&failure.patch_set)?;
                return Err(apply_failure(failure));
            }
        };
        repository.save_patch_set(&applied.patch_set)?;
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
        let (validation_result, gate_decision) =
            validate_patch_result(&applied.patch_set, &scan, &findings, &decisions)?;
        repository.save_validation(&validation_result, &gate_decision)?;
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

fn apply_failure(failure: Box<ApplyFailure>) -> ApplicationError {
    ApplicationError::Apply(failure.code.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use star_contracts::{
        ids::{BaselineId, DispositionId, SuppressionId},
        management::{
            BaselineScope, BaselineStatus, DispositionDecision, DispositionStatus, PatchSetStatus,
            SuppressionScope, SuppressionStatus,
        },
    };
    use star_evidence::LocalArtifactStore;
    use star_state::{SqliteManagementRepositorySet, WindowsProjectRootBindingStore};

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
        let service = ManagementApplicationService::new(
            repositories,
            bindings.clone(),
            Arc::new(LocalArtifactStore::default()),
        );
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
}
