//! Shared CLI and future Codex management application service.

use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use star_contracts::{
    Sha256Hash, canonical_sha256,
    evidence::{
        ActorRef, ActorType, ArtifactKind, ArtifactManifest, ArtifactManifestEntry, ArtifactRef,
        AuthoritativeGateState, CatalogRef, DocumentRef, GateDecision, GateDecisionKind, GateScope,
        OutputLimits, RedactionStatus, RetentionClass, TerminationReason,
    },
    evidence_v2::{
        BaselineV2, CompletionClaimRefV2, CompletionClaimSubjectV2, CompletionClaimV2,
        DiagnosticV2, DispositionV2, EVIDENCE_V2_SCHEMA_VERSION, EvidenceBundleV2,
        EvidenceFreshnessV2, EvidenceSubjectBinding, GateDecisionV2, GatePhaseV2,
        InvocationWorkingDirectoryV2, ReviewPackV1, SuppressionV2, ValidationResultV2,
        ValidationRunV2,
    },
    ids::{
        CheckoutId, CodeIndexSnapshotId, CoordinatedOperationId, DiagnosticId, EvidenceBundleId,
        FindingId, GateId, GenerationId, PatchApplicationId, PatchSetId, ProjectId,
        RecipeExecutionId, RequestId, ReviewPackId, ScanRunId, TaskSpecId, WorktreeDecisionId,
    },
    index::{
        CodeIndexSnapshot, HardcodingCandidate, IndexEdge, IndexEntity, IndexFreshnessState,
        IndexPartitionKind, IndexPartitionState, IndexScanMode, IndexTier, ProjectCatalogSnapshot,
        SourceClass, SourceEntry, ToolchainCommandKind,
    },
    managed_registry::{
        ManagedDeclarationChangeIntent, ManagedRegistrySnapshot, RegistryConsistencyRecord,
    },
    management::{
        Baseline, CoordinatedOperation, CoordinationParticipant, CoordinationState, Disposition,
        Finding, ManagementStoreStatus, ParticipantState, PatchSet, PatchSetStatus, Project,
        ProjectCheckout, ProjectPathRef, ProjectStorePoint, ScanRun, ScanStatus, StorePoint,
        StoreVersionVector, Suppression, SymbolReference, ValidationResult,
    },
    parse_no_duplicate_keys,
    patch_v2::{
        ChangeRecipeV2, PatchApplication, PatchApplicationStateV1, PatchMigrationOutcomeV1,
        PatchOperation, PatchOperationKindV2, PatchOperationReceiptStateV1,
        PatchOperationReceiptV1, PatchPermitKindRecordV1, PatchRecoveryStrategyV1, PatchSetStateV2,
        PatchSetV2, PatchV1ToV2MigrationEntry, PatchV1ToV2MigrationPlan,
        PatchV1ToV2MigrationResult, RecipeExecution, RecipeExecutionStateV1, TargetSelector,
        WorktreeDecision, WorktreeDecisionStateV1, WorktreeStrategyV1,
    },
    planning::{
        BaselinePolicy, BaselinePolicyKind, CheckCandidate, CheckDescriptor, CheckOverride,
        CheckPlanV2, CollectionState, FallbackDecision, ImpactAnalysis, ImpactStatus,
        IntendedChange, IntendedChangeKind, ObservedChangeKind, PlanningBundle, PlanningSelector,
        ProjectTarget, ProjectTargetRole, ScopeReasonCode, ScopeRelation, ScopeSourceSnapshotRef,
        ScopeUserDecision, SelectorKind, SuccessCriterion, UnresolvedCheck,
        ValidationPlanV2Readiness,
    },
    recovery::{
        BackupApplyResult, BackupPlan, LocalStateExportPlan, LocalStateExportResult,
        LocalStateImportPlan, LocalStateImportResult, RebuildApplyResult, RebuildPlan,
        RebuildProjectInput, RebuiltProjectSummary, RecoveryLossItem, RecoveryLossKind,
        RecoveryLossState,
    },
    rust_style::{
        RustAutoPolicy, RustCompleteness, RustStylePolicyApprovalDecision,
        RustStylePolicyApprovalRequest,
    },
    validator_guard::{GuardFixtureKindV2, ValidatorGuardEvidenceV2},
};
use star_domain::{PersistenceRedactor, versioned_fingerprint};
use star_execution::rust_style::{
    RustStylePatchBinding, RustStylePatchScope, apply_owned_preview_changes,
    apply_rust_style_patch, is_rust_style_patch_artifact, rust_style_patch_binding,
    validate_owned_preview_root,
};
use star_execution::{
    ApplyFailure, ExactFileSourceMutationAdapter, ExecutionError, GitWorktreeAdapter,
    MaterializedPatchFile, PatchFilesystemStateV2, PreparedPatchTransformerAdapter,
    ReversePatchMaterialV2, apply_patch, exact_reverse_recipe_v2, managed_declaration_recipe_v2,
    observe_patch_set_v2, prepare_exact_materialized_patch, prepare_trailing_whitespace_patch,
    prepare_trailing_whitespace_paths, recover_patch_set_v2, rollback_applied,
    rust_style_recipe_v2, trailing_whitespace_recipe_v2,
};
use star_planning::{
    ObservedWorkspaceChange, PlanningError, PlanningPolicy, PlanningProjectIndex, PlanningRequest,
    PlanningRevisionRequest, PreviousSuccessEvidence, TaskSpecDraft,
    build_planning_bundle_for_phase, builtin_risk_descriptors,
    invalidate_planning_bundle as build_invalidated_planning_bundle, planning_bundle_revision,
    revise_planning_bundle as build_revised_planning_bundle, task_spec_to_draft,
};
use star_ports::{
    ArtifactDiscovery, ArtifactStore, ArtifactWritePolicy, ArtifactWriteRequest,
    CheckGraphEvidenceTransaction, CodeIndexCache, GlobalManagementRepository, ManagementRecovery,
    ManagementRepositorySet, PatchPortError, ProjectRootBindingStore, RepositoryError,
    RepositoryErrorCategory, RetentionApplyResult, RetentionPlan, RewriteTransformRequest,
    RewriteTransformerPort, ScanCommit, SourceMutationPort, SourceMutationRequest,
    SourceMutationState, StoredCodeIndexProjection, WorktreeMaterialization, WorktreePort,
};
pub use star_ports::{
    DevelopmentRecord, ManagedRegistryConsumerProjectInput, ManagedRegistryResolveRequest,
    ManagedRegistryResolveResult, ManagedRegistryResolverError, ManagedRegistryResolverPort,
    ManagedRegistryRewritePort, ManagedRegistryRewriteRequest, ManagedRegistryRewriteResult,
    MaterializedRewrite,
};
use star_project::{
    ProjectError, ProjectSeed, ScanPolicy, SharedDecisionDeclarations,
    catalog_snapshot::{CatalogSnapshotInput, DiscoveryConfig, build_project_catalog_snapshot},
    git_common_directory,
    index::{
        CodeIndexBuildRequest, CodeIndexProjection, IndexPolicy, SemanticAdapter, SyntaxAdapter,
        build_code_index,
    },
    load_shared_decisions, observe_project, observe_workspace_changes,
};
use star_validation::permit::{
    ManualPatchApprovalV2, PatchApplicationStateV2, PatchPermitKindV2, PatchPostApplyDispositionV2,
    VerifiedPatchGateV2, evaluate_patch_post_apply, issue_patch_apply_permit,
};
pub use star_validation::planning::{
    AiEvidenceSummary, AiValidationRunSummary, CacheMissReason, CacheReuseDecision,
    CacheValidationStability, EvidenceCompressionError, UnitDependency, ValidationCacheCandidate,
    ValidationCheckDefinition, ValidationEvidenceDiagnostic, ValidationEvidenceRun,
    ValidationPlanningError, ValidationPlanningInput, build_validation_plan,
    compress_evidence_for_ai, evaluate_cache_reuse,
};
pub use star_validation::process_executor::{
    CheckOutputArtifactError, CheckOutputArtifactInput, CheckOutputArtifactSink,
    ProcessExecutorError, RegisteredProcessCheckExecutor, ResolvedExecutableV2,
    SafeExitDiagnosticNormalizer,
};
pub use star_validation::rules::{
    RuleDecisionFloorV2, RuleDiagnosticInputV2, RuleFactV2, RuleFamilyV2, RuleFixtureResultV2,
    TwoSnapshotGuardInputV2, evaluate_rule_facts, evaluate_two_snapshot_guard,
};
pub use star_validation::runner::{
    ArtifactManifestFinalizationError, ArtifactManifestFinalizer, CheckExecutor,
    CheckGraphRunContext, CheckGraphRunResult, CheckGraphRunnerError, ExecutableBinding,
    run_check_graph, run_check_graph_with_artifact_finalizer,
};
use star_validation::rust_style::RustFileChange;
use star_validation::{
    ValidationError, analyze_builtin_findings, apply_decision_projection, evaluate_decisions,
    validate_patch_result_with_plan,
};
use thiserror::Error;

pub mod profile_catalog;
pub mod rust_style;
pub mod rust_style_runtime;

pub use profile_catalog::{
    ProfileCatalogLoadError, load_development_profile_catalog, resolve_loaded_development_profiles,
    show_development_profile,
};

use rust_style_runtime::{
    RustStyleCheckResult, RustStyleGatePhase, RustStyleInspection, RustStyleRuntimeError,
    RustStyleScope, check_rust_style, inspect_rust_style, materialize_rust_style_gate_preview,
    prepare_rust_style,
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
    #[error("registered process executor preparation failed")]
    ProcessExecutor(#[from] ProcessExecutorError),
    #[error("finding or gate evaluation failed")]
    Validation(#[from] ValidationError),
    #[error("patch preparation failed")]
    Execution(#[from] ExecutionError),
    #[error("patch apply failed: {0}")]
    Apply(String),
    #[error("Rust style workflow failed: {0}")]
    RustStyle(#[from] RustStyleRuntimeError),
    #[error("development profile catalog failed: {0}")]
    ProfileCatalog(#[from] ProfileCatalogLoadError),
    #[error("development profile contract failed: {0}")]
    ProfileContract(#[from] star_contracts::profile::DevelopmentProfileContractError),
}

#[derive(Clone, Debug, Serialize)]
pub struct RegisterProjectResult {
    pub project: Project,
    pub checkout: ProjectCheckout,
    pub coordinated_operation: CoordinatedOperation,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlanningBundleStatus {
    pub task_spec_id: TaskSpecId,
    pub bundle_revision: u64,
    pub task_revision: u64,
    pub scope_revision: u64,
    pub impact_revision: u64,
    pub validation_revision: u64,
    pub scope_reason_code: ScopeReasonCode,
    pub impact_status: ImpactStatus,
    pub validation_readiness: ValidationPlanV2Readiness,
    pub source_snapshot_refs: Vec<ScopeSourceSnapshotRef>,
    pub bundle_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, Serialize)]
pub struct ValidationExecutionPreflightItem {
    pub plan_item_id: String,
    pub check_id: String,
    pub project_id: ProjectId,
    pub logical_executable: String,
    pub executable_binding_fingerprint: Sha256Hash,
    pub descriptor_ref: DocumentRef,
    pub subject_binding_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, Serialize)]
pub struct ValidationExecutionPreflight {
    pub task_spec_id: TaskSpecId,
    pub validation_plan_ref: DocumentRef,
    pub project_id: ProjectId,
    pub project_root_fingerprint: Sha256Hash,
    pub execution_root_kind: String,
    pub execution_root_binding_fingerprint: Sha256Hash,
    pub items: Vec<ValidationExecutionPreflightItem>,
    pub rule_diagnostics: Vec<RuleDiagnosticInputV2>,
    pub decision_floor: RuleDecisionFloorV2,
    pub completion_claim_refs: Vec<CompletionClaimRefV2>,
    pub validator_guard_evidence_ref: Option<DocumentRef>,
    pub readiness: ValidationPlanV2Readiness,
}

#[derive(Clone, Debug)]
pub struct RegisteredValidationExecutionEvidence {
    pub completion_claims: Vec<CompletionClaimV2>,
    pub validator_guard_evidence: Option<ValidatorGuardEvidenceV2>,
}

struct PreparedValidationExecution {
    preflight: ValidationExecutionPreflight,
    bindings: Vec<ExecutableBinding>,
    resolved_executables: Vec<ResolvedExecutableV2>,
    change_sets: Vec<star_contracts::planning::ChangeSet>,
    completion_claims: Vec<CompletionClaimV2>,
    validator_guard_evidence: Option<ValidatorGuardEvidenceV2>,
}

#[derive(Clone, Debug)]
struct ValidationExecutionRootBinding {
    root: PathBuf,
    kind: &'static str,
    binding_fingerprint: Sha256Hash,
}

struct VerifiedValidatorGuardInput<'a> {
    evidence: &'a ValidatorGuardEvidenceV2,
    artifacts_verified: bool,
    expected_candidate_registry_fingerprint: &'a Sha256Hash,
}

struct ValidationOutputArtifactSink {
    artifacts: Arc<dyn ArtifactStore>,
    project_id: ProjectId,
    project_root: PathBuf,
    task_spec_id: TaskSpecId,
    artifact_set_id: RequestId,
    redactor: PersistenceRedactor,
}

struct ValidationOutputStreamInput<'a> {
    invocation_id: &'a str,
    stream: &'a str,
    bytes: &'a [u8],
    truncated: bool,
    output_read_failed: bool,
    exit_code: Option<i32>,
    termination_reason: TerminationReason,
}

impl ValidationOutputArtifactSink {
    fn persist_stream(
        &self,
        input: ValidationOutputStreamInput<'_>,
    ) -> Result<ArtifactRef, CheckOutputArtifactError> {
        let decoded = std::str::from_utf8(input.bytes).ok();
        let content_safe = decoded.is_some_and(|text| {
            self.redactor.validate(text).is_ok() && !m3_contains_secret_candidate(text)
        });
        let redaction_status = if content_safe {
            RedactionStatus::NotNeeded
        } else {
            RedactionStatus::Redacted
        };
        let content = content_safe.then(|| decoded.unwrap_or_default());
        let content_sha256 = content_safe.then(|| Sha256Hash::digest(input.bytes));
        self.artifacts
            .put_json_with_policy(ArtifactWriteRequest {
                project_id: &self.project_id,
                project_root: &self.project_root,
                relative_path: &format!(
                    "validation/m3/{}/{}/attempts/{}/{}.json",
                    self.task_spec_id.as_str(),
                    self.artifact_set_id.as_str(),
                    input.invocation_id,
                    input.stream
                ),
                subject_kind: "validation_process_output",
                subject_id: input.invocation_id,
                policy: ArtifactWritePolicy {
                    kind: ArtifactKind::Log,
                    redaction_status,
                    retention_class: RetentionClass::Evidence,
                },
                value: &serde_json::json!({
                    "schema_id":"star.validation-process-output",
                    "schema_version":1,
                    "invocation_id":input.invocation_id,
                    "stream":input.stream,
                    "captured_bytes":input.bytes.len(),
                    "truncated":input.truncated,
                    "output_read_failed":input.output_read_failed,
                    "exit_code":input.exit_code,
                    "termination_reason":input.termination_reason,
                    "content_status":if content_safe { "retained" } else { "redacted" },
                    "content_sha256":content_sha256,
                    "content":content,
                }),
            })
            .map_err(|error| CheckOutputArtifactError {
                code: format!("CHECK_OUTPUT_ARTIFACT_STORE_FAILED_{:?}", error.category)
                    .to_ascii_uppercase(),
            })
    }
}

impl CheckOutputArtifactSink for ValidationOutputArtifactSink {
    fn persist(
        &mut self,
        input: CheckOutputArtifactInput<'_>,
    ) -> Result<Vec<ArtifactRef>, CheckOutputArtifactError> {
        let invocation_id = input.invocation.invocation_id.as_str();
        let stdout = self.persist_stream(ValidationOutputStreamInput {
            invocation_id,
            stream: "stdout",
            bytes: input.stdout,
            truncated: input.stdout_truncated,
            output_read_failed: input.output_read_failed,
            exit_code: input.exit_code,
            termination_reason: input.termination_reason,
        })?;
        let stderr = self.persist_stream(ValidationOutputStreamInput {
            invocation_id,
            stream: "stderr",
            bytes: input.stderr,
            truncated: input.stderr_truncated,
            output_read_failed: input.output_read_failed,
            exit_code: input.exit_code,
            termination_reason: input.termination_reason,
        })?;
        Ok(vec![stdout, stderr])
    }
}

struct ValidationArtifactManifestFinalizer {
    artifacts: Arc<dyn ArtifactStore>,
    project_id: ProjectId,
    project_root: PathBuf,
    task_spec_id: TaskSpecId,
    artifact_set_id: RequestId,
    preflight_ref: ArtifactRef,
    initial_artifacts: Vec<ArtifactRef>,
}

impl ArtifactManifestFinalizer for ValidationArtifactManifestFinalizer {
    fn finalize(
        &mut self,
        validation_plan_ref: &DocumentRef,
        runs: &[ValidationRunV2],
        diagnostics: &[DiagnosticV2],
    ) -> Result<ArtifactManifest, ArtifactManifestFinalizationError> {
        let mut artifacts = BTreeMap::new();
        artifacts.insert(
            self.preflight_ref.artifact_id.clone(),
            self.preflight_ref.clone(),
        );
        for artifact in &self.initial_artifacts {
            if artifacts
                .insert(artifact.artifact_id.clone(), artifact.clone())
                .is_some_and(|existing| existing != *artifact)
            {
                return Err(ArtifactManifestFinalizationError {
                    code: "ARTIFACT_IDENTITY_CONFLICT".to_owned(),
                });
            }
        }
        for artifact in runs.iter().flat_map(|run| run.artifact_refs.iter()).chain(
            diagnostics
                .iter()
                .flat_map(|diagnostic| diagnostic.evidence_refs.iter()),
        ) {
            if artifacts
                .insert(artifact.artifact_id.clone(), artifact.clone())
                .is_some_and(|existing| existing != *artifact)
            {
                return Err(ArtifactManifestFinalizationError {
                    code: "ARTIFACT_IDENTITY_CONFLICT".to_owned(),
                });
            }
        }
        let entries = artifacts
            .values()
            .map(|artifact| ArtifactManifestEntry {
                artifact_id: artifact.artifact_id.clone(),
                sha256: artifact.sha256.clone(),
                size_bytes: artifact.size_bytes,
                redaction_status: artifact.redaction_status,
            })
            .collect::<Vec<_>>();
        let artifact_refs = artifacts.into_values().collect::<Vec<_>>();
        let manifest_ref = self
            .artifacts
            .put_json_with_policy(ArtifactWriteRequest {
                project_id: &self.project_id,
                project_root: &self.project_root,
                relative_path: &format!(
                    "validation/m3/{}/{}/artifact-manifest.json",
                    self.task_spec_id.as_str(),
                    self.artifact_set_id.as_str()
                ),
                subject_kind: "validation_artifact_manifest",
                subject_id: self.task_spec_id.as_str(),
                policy: ArtifactWritePolicy {
                    kind: ArtifactKind::Manifest,
                    redaction_status: RedactionStatus::NotNeeded,
                    retention_class: RetentionClass::Evidence,
                },
                value: &serde_json::json!({
                    "schema_id":"star.validation-artifact-manifest",
                    "schema_version":1,
                    "artifact_set_id":self.artifact_set_id,
                    "validation_plan_ref":validation_plan_ref,
                    "artifacts":artifact_refs,
                }),
            })
            .map_err(|_| ArtifactManifestFinalizationError {
                code: "ARTIFACT_MANIFEST_STORE_FAILED".to_owned(),
            })?;
        Ok(ArtifactManifest {
            manifest_ref,
            artifacts: entries,
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ValidationExecutionStatus {
    pub project_id: ProjectId,
    pub run_count: usize,
    pub result_count: usize,
    pub diagnostic_count: usize,
    pub gate_count: usize,
    pub evidence_bundle_count: usize,
    pub review_pack_count: usize,
    pub latest_result: Option<ValidationResultV2>,
    pub latest_gate: Option<GateDecisionV2>,
    pub latest_evidence_bundle: Option<EvidenceBundleV2>,
    pub latest_review_pack: Option<ReviewPackV1>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ValidationDecisionInspection {
    pub project_id: ProjectId,
    pub baselines: Vec<BaselineV2>,
    pub suppressions: Vec<SuppressionV2>,
    pub dispositions: Vec<DispositionV2>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AffectedChecksView {
    pub candidate_checks: Vec<CheckCandidate>,
    pub required_checks: Vec<CheckPlanV2>,
    pub optional_checks: Vec<CheckPlanV2>,
    pub omitted_checks: Vec<String>,
    pub unresolved_checks: Vec<UnresolvedCheck>,
    pub fallback_decisions: Vec<FallbackDecision>,
    pub readiness: ValidationPlanV2Readiness,
}

struct PlanningInputSnapshot {
    catalog: ProjectCatalogSnapshot,
    projects: Vec<PlanningProjectIndex>,
    pinned_snapshots: Vec<(ProjectId, CodeIndexSnapshotId)>,
}

struct LoadedPatchV2 {
    project_root: PathBuf,
    patch_set: PatchSetV2,
    recipe_execution: RecipeExecution,
    worktree_decision: WorktreeDecision,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScanProjectResult {
    pub scan_run: ScanRun,
    pub code_index_snapshot: Option<CodeIndexSnapshot>,
    pub finding_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct MultiRootDiscoveryResult {
    pub registrations: Vec<RegisterProjectResult>,
    pub catalog_snapshot: ProjectCatalogSnapshot,
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
pub struct ManagedRegistryResolutionContext {
    pub project_root: PathBuf,
    pub owner_project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub project_revision_id: star_contracts::ids::ProjectRevisionId,
    pub workspace_snapshot_id: star_contracts::ids::WorkspaceSnapshotId,
    pub code_index_snapshot_id: CodeIndexSnapshotId,
    pub index_current: bool,
    pub coverage_complete: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PublishedManagedRegistryResolution {
    pub snapshot: ManagedRegistrySnapshot,
    pub consistency_records: Vec<RegistryConsistencyRecord>,
    pub artifact_refs: Vec<ArtifactRef>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PreparedPatchResult {
    pub patch_set: PatchSet,
    pub change_plan_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecipeCatalogResult {
    pub items: Vec<ChangeRecipeV2>,
    pub confirmed_empty: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PreparedChangeV2Result {
    pub recipe: ChangeRecipeV2,
    pub planning_bundle: PlanningBundle,
    pub worktree_decision: WorktreeDecision,
    pub recipe_execution: RecipeExecution,
    pub patch_set: PatchSetV2,
    pub compatibility_patch_set: PatchSet,
}

#[derive(Clone, Debug, Serialize)]
pub struct PatchShowV2Result {
    pub patch_set: PatchSetV2,
    pub recipe_execution: RecipeExecution,
    pub worktree_decision: WorktreeDecision,
    pub forward_artifact_refs: Vec<ArtifactRef>,
    pub reverse_artifact_refs: Vec<ArtifactRef>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PatchStatusV2Result {
    pub application: PatchApplication,
    pub observed_state: PatchApplicationStateV1,
    pub reconciliation_reason_codes: Vec<String>,
    pub recovery_strategies: Vec<PatchRecoveryStrategyV1>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PatchRecoverV2Result {
    pub application: PatchApplication,
    pub recovered: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PatchApplyV2Result {
    pub application: PatchApplication,
    pub pre_gate_decision: GateDecisionV2,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_gate_decision: Option<GateDecisionV2>,
    pub source_effect_started: bool,
    pub recovered: bool,
    pub compatibility_patch_set: PatchSet,
}

#[derive(Clone, Debug, Serialize)]
pub struct AppliedPatchResult {
    pub patch_set: PatchSet,
    pub validation_result: ValidationResult,
    pub gate_decision: GateDecision,
    pub automatic_rollback: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch_application: Option<PatchApplication>,
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
    pub candidate_build: Option<rust_style_runtime::RustToolRunSummary>,
    pub candidate_test_compile: Option<rust_style_runtime::RustToolRunSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepared_change_v2: Option<PreparedChangeV2Result>,
    pub isolation_ref: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RustStyleAutoApplyResult {
    pub prepared: PreparedRustStyleResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied: Option<AppliedPatchResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied_v2: Option<PatchApplyV2Result>,
    pub permit_automatic: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_approval_request: Option<RustStylePolicyApprovalRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_approval_decision: Option<RustStylePolicyApprovalDecision>,
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
    managed_registry_resolver: Option<Arc<dyn ManagedRegistryResolverPort>>,
    managed_registry_rewriter: Option<Arc<dyn ManagedRegistryRewritePort>>,
    rust_style_runtime_root: Option<PathBuf>,
    rust_style_policy_path: Option<PathBuf>,
    profile_catalog_root: Option<PathBuf>,
    command_lock: Mutex<()>,
}

pub struct ManagementRecoveryApplicationService<'a> {
    recovery: &'a dyn ManagementRecovery,
    root_bindings: Arc<dyn ProjectRootBindingStore>,
    artifacts: Arc<dyn ArtifactStore>,
    scan_policy: ScanPolicy,
    index_policy: IndexPolicy,
    index_cache: Option<Arc<dyn CodeIndexCache>>,
    syntax_adapters: Vec<Arc<dyn SyntaxAdapter>>,
    semantic_adapters: Vec<Arc<dyn SemanticAdapter>>,
    command_lock: Mutex<()>,
}

struct ManagedRustSourceMutationPortV2<'a> {
    service: &'a ManagementApplicationService,
    patch_set_id: PatchSetId,
    approved_patch_fingerprint: String,
    requested_by: ActorRef,
    result: Option<Result<PatchApplyV2Result, ApplicationError>>,
}

impl rust_style::RustSourceMutationPort for ManagedRustSourceMutationPortV2<'_> {
    fn apply_exact(
        &mut self,
        _candidate: &rust_style::RustStyleCandidate,
    ) -> rust_style::SourceMutationObservation {
        let result = self.service.apply_patch_v2_inner(
            &self.patch_set_id,
            &self.approved_patch_fingerprint,
            self.requested_by.clone(),
            None,
            None,
        );
        let observation = match &result {
            Ok(applied)
                if !applied.recovered
                    && applied.application.state == PatchApplicationStateV1::Applied
                    && applied.pre_gate_decision.decision == GateDecisionKind::AutoPass
                    && applied.post_gate_decision.as_ref().is_some_and(|decision| {
                        decision.decision == GateDecisionKind::AutoPass
                    }) =>
            {
                rust_style::SourceMutationObservation::Applied {
                    post_gate_auto_pass: true,
                    evidence_complete: true,
                }
            }
            Ok(_) => rust_style::SourceMutationObservation::Partial,
            Err(ApplicationError::Apply(code))
                if code.contains("STALE") || code.contains("STATE_MISMATCH") =>
            {
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
            managed_registry_resolver: None,
            managed_registry_rewriter: None,
            rust_style_runtime_root: None,
            rust_style_policy_path: None,
            profile_catalog_root: None,
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

    pub fn with_profile_catalog_root(mut self, profile_catalog_root: PathBuf) -> Self {
        self.profile_catalog_root = Some(profile_catalog_root);
        self
    }

    pub fn development_profile_catalog(
        &self,
    ) -> Result<star_contracts::profile::DevelopmentProfileCatalogSnapshotV1, ApplicationError>
    {
        let root = self
            .profile_catalog_root
            .as_deref()
            .ok_or(ApplicationError::Invalid)?;
        Ok(load_development_profile_catalog(root)?)
    }

    pub fn development_profile(
        &self,
        profile_id: &str,
    ) -> Result<star_contracts::profile::DevelopmentProfileCatalogEntryV1, ApplicationError> {
        let catalog = self.development_profile_catalog()?;
        Ok(show_development_profile(&catalog, profile_id)?.clone())
    }

    pub fn resolve_development_profiles(
        &self,
        profile_ids: &[String],
    ) -> Result<star_contracts::profile::DevelopmentProfileResolutionV1, ApplicationError> {
        let catalog = self.development_profile_catalog()?;
        Ok(resolve_loaded_development_profiles(&catalog, profile_ids)?)
    }

    pub fn with_syntax_adapter(mut self, adapter: Arc<dyn SyntaxAdapter>) -> Self {
        self.syntax_adapters.push(adapter);
        self
    }

    pub fn with_semantic_adapter(mut self, adapter: Arc<dyn SemanticAdapter>) -> Self {
        self.semantic_adapters.push(adapter);
        self
    }

    pub fn with_managed_registry_resolver(
        mut self,
        resolver: Arc<dyn ManagedRegistryResolverPort>,
    ) -> Self {
        self.managed_registry_resolver = Some(resolver);
        self
    }

    pub fn with_managed_registry_rewriter(
        mut self,
        rewriter: Arc<dyn ManagedRegistryRewritePort>,
    ) -> Self {
        self.managed_registry_rewriter = Some(rewriter);
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
        let repository_match = if attachment.is_none() {
            self.matching_git_repository(&canonical_root)?
        } else {
            None
        };
        let seed = ProjectSeed::discover_with_local_project_id(
            &canonical_root,
            attachment
                .as_ref()
                .map(|value| value.project_id.clone())
                .or_else(|| {
                    repository_match
                        .as_ref()
                        .map(|(project_id, _)| project_id.clone())
                }),
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
        let binding_id = self
            .root_bindings
            .attach(&seed.project_id, &checkout_id, &canonical_root)
            .map_err(|error| {
                RepositoryError::new(error.category, "project root binding attach failed")
            })?;
        let attached = seed.attach_with_repository_binding(
            checkout_id,
            binding_id,
            &canonical_root,
            repository_match.map(|(_, binding_id)| binding_id),
        )?;
        let project = merge_project_attachment(
            self.repositories
                .global()
                .get_project(&attached.project.project_id)?,
            attached.project,
        )?;
        let checkout = attached.checkout;
        let global_before = self.repositories.global().status().map_err(|error| {
            RepositoryError::new(
                error.category,
                "global store status before registration failed",
            )
        })?;
        let registration_payload = registration_fingerprint_payload(&project, &checkout);
        let input_fingerprint =
            versioned_fingerprint("star.command.project-register", 3, &registration_payload)
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
            3,
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
        self.repositories
            .global()
            .put_coordination(&operation)
            .map_err(|error| {
                RepositoryError::new(error.category, "registration prepare record failed")
            })?;

        let project_repository =
            self.repositories
                .project(&project.project_id)
                .map_err(|error| {
                    RepositoryError::new(error.category, "registration project store open failed")
                })?;
        let participant_result = versioned_fingerprint(
            "star.coordination.project-register.result",
            3,
            &registration_payload,
        )
        .map_err(|_| ApplicationError::Invalid)?;
        let receipt = project_repository
            .commit_registration_participant(
                &project,
                &operation_id,
                &participant_payload,
                &participant_result,
            )
            .map_err(|error| {
                RepositoryError::new(error.category, "registration participant commit failed")
            })?;
        operation.participants[0].state = ParticipantState::Committed;
        operation.participants[0].receipt = Some(receipt);
        operation.state = CoordinationState::Applying;
        operation.updated_at = Utc::now();
        self.repositories
            .global()
            .put_coordination(&operation)
            .map_err(|error| {
                RepositoryError::new(error.category, "registration applying record failed")
            })?;

        self.repositories
            .global()
            .register_project(&project, &checkout, idempotency_key, &input_fingerprint)
            .map_err(|error| {
                RepositoryError::new(error.category, "global project registration failed")
            })?;
        self.repositories.verify_all().map_err(|error| {
            RepositoryError::new(
                error.category,
                "active store set refresh after registration failed",
            )
        })?;
        let global_before_completion = self.repositories.global().status().map_err(|error| {
            RepositoryError::new(
                error.category,
                "global store status before registration completion failed",
            )
        })?;
        let project_after = project_repository.status().map_err(|error| {
            RepositoryError::new(
                error.category,
                "project store status before registration completion failed",
            )
        })?;
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
            .find_by_root(requested_root)?
            .ok_or_else(|| {
                ApplicationError::Repository(RepositoryError::new(
                    RepositoryErrorCategory::IdempotencyConflict,
                    "registration idempotency key belongs to another project root",
                ))
            })?;
        if attachment.project_id != project_id {
            return Err(ApplicationError::Repository(RepositoryError::new(
                RepositoryErrorCategory::IdempotencyConflict,
                "registration idempotency key belongs to another project",
            )));
        }
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
        let existing_repository_binding_id = self
            .repositories
            .global()
            .get_project_checkout(&attachment.checkout_id)?
            .and_then(|checkout| checkout.repository_binding_id);
        let candidate = seed.attach_with_repository_binding(
            attachment.checkout_id.clone(),
            attachment.root_binding_id,
            &attached_root,
            existing_repository_binding_id,
        )?;
        let registration_payload =
            registration_fingerprint_payload(&candidate.project, &candidate.checkout);
        let input_fingerprint =
            versioned_fingerprint("star.command.project-register", 3, &registration_payload)
                .map_err(|_| ApplicationError::Invalid)?;
        let legacy_payload =
            legacy_registration_fingerprint_payload(&candidate.project, &candidate.checkout);
        let legacy_input_fingerprint =
            versioned_fingerprint("star.command.project-register", 2, &legacy_payload)
                .map_err(|_| ApplicationError::Invalid)?;
        if input_fingerprint != operation.input_fingerprint
            && legacy_input_fingerprint != operation.input_fingerprint
        {
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
            let existing_repository_binding_id = self
                .repositories
                .global()
                .get_project_checkout(&attachment.checkout_id)?
                .and_then(|checkout| checkout.repository_binding_id);
            let attached = match seed.attach_with_repository_binding(
                attachment.checkout_id,
                attachment.root_binding_id,
                &root,
                existing_repository_binding_id,
            ) {
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
            let legacy_project = attached.project;
            let checkout = attached.checkout;
            let project = merge_project_attachment(
                self.repositories.global().get_project(&project_id)?,
                legacy_project.clone(),
            )?;
            let stored_project = project_repository.get_project()?;
            if stored_project
                .as_ref()
                .is_some_and(|stored| stored != &project && stored != &legacy_project)
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
                versioned_fingerprint("star.command.project-register", 3, &registration_payload)
                    .map_err(|_| ApplicationError::Invalid)?;
            let participant_payload = versioned_fingerprint(
                "star.coordination.project-register.participant",
                3,
                &registration_payload,
            )
            .map_err(|_| ApplicationError::Invalid)?;
            let legacy_payload =
                legacy_registration_fingerprint_payload(&legacy_project, &checkout);
            let legacy_input_fingerprint =
                versioned_fingerprint("star.command.project-register", 2, &legacy_payload)
                    .map_err(|_| ApplicationError::Invalid)?;
            let legacy_participant_payload = versioned_fingerprint(
                "star.coordination.project-register.participant",
                2,
                &legacy_payload,
            )
            .map_err(|_| ApplicationError::Invalid)?;
            let (project, registration_payload, participant_payload, contract_version) =
                if input_fingerprint == operation.input_fingerprint
                    && participant_payload == operation.participants[0].payload_fingerprint
                {
                    (project, registration_payload, participant_payload, 3)
                } else if legacy_input_fingerprint == operation.input_fingerprint
                    && legacy_participant_payload == operation.participants[0].payload_fingerprint
                {
                    (
                        legacy_project,
                        legacy_payload,
                        legacy_participant_payload,
                        2,
                    )
                } else {
                    block_coordination(
                        self.repositories.global(),
                        &mut operation,
                        "PROJECT_REGISTRATION_INPUT_CHANGED",
                    )?;
                    continue;
                };
            let participant_result = versioned_fingerprint(
                "star.coordination.project-register.result",
                contract_version,
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
            self.repositories.verify_all()?;
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

    pub fn list_project_checkouts(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<ProjectCheckout>, ApplicationError> {
        if self
            .repositories
            .global()
            .get_project(project_id)?
            .is_none()
        {
            return Err(ApplicationError::NotFound);
        }
        Ok(self
            .repositories
            .global()
            .list_project_checkouts(project_id)?)
    }

    pub fn get_project_checkout(
        &self,
        checkout_id: &CheckoutId,
    ) -> Result<ProjectCheckout, ApplicationError> {
        self.repositories
            .global()
            .get_project_checkout(checkout_id)?
            .ok_or(ApplicationError::NotFound)
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

    fn matching_git_repository(
        &self,
        candidate_root: &Path,
    ) -> Result<Option<(ProjectId, String)>, ApplicationError> {
        let Some(candidate_common) = git_common_directory(candidate_root)? else {
            return Ok(None);
        };
        let mut matched: Option<(ProjectId, String)> = None;
        for attachment in self.root_bindings.list_attachments()? {
            let attached_root = self.root_bindings.resolve(&attachment.root_binding_id)?;
            let Some(attached_common) = git_common_directory(&attached_root)? else {
                continue;
            };
            if attached_common != candidate_common {
                continue;
            }
            let checkout = self
                .repositories
                .global()
                .get_project_checkout(&attachment.checkout_id)?
                .ok_or(ApplicationError::NotFound)?;
            if checkout.project_id != attachment.project_id {
                return Err(ApplicationError::Invalid);
            }
            let binding_id = checkout.repository_binding_id.unwrap_or_else(|| {
                format!("repository-binding:{}", attachment.root_binding_id.as_str())
            });
            if matched.as_ref().is_some_and(|(project_id, existing)| {
                project_id != &attachment.project_id || existing != &binding_id
            }) {
                return Err(ApplicationError::Invalid);
            }
            matched = Some((attachment.project_id, binding_id));
        }
        Ok(matched)
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

    pub fn discover_project_roots(
        &self,
        roots: &[PathBuf],
        idempotency_key: &str,
    ) -> Result<MultiRootDiscoveryResult, ApplicationError> {
        let _guard = self.command_guard()?;
        if roots.is_empty() || roots.len() > 64 || !valid_idempotency_key(idempotency_key) {
            return Err(ApplicationError::Invalid);
        }
        let mut canonical_roots = roots
            .iter()
            .map(|root| root.canonicalize().map_err(|_| ApplicationError::Invalid))
            .collect::<Result<Vec<_>, _>>()?;
        canonical_roots.sort();
        if canonical_roots.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ApplicationError::Invalid);
        }
        // 모든 root를 먼저 probe해 중간 입력 오류로 일부만 attach되는 경우를 막는다.
        for root in &canonical_roots {
            if !root.is_dir() {
                return Err(ApplicationError::Invalid);
            }
            let existing = self.root_bindings.find_by_root(root)?;
            let seed = ProjectSeed::discover_with_local_project_id(
                root,
                existing.as_ref().map(|value| value.project_id.clone()),
            )?;
            if existing
                .as_ref()
                .is_some_and(|value| value.project_id != seed.project_id)
            {
                return Err(ApplicationError::Invalid);
            }
        }
        let mut registrations = Vec::with_capacity(canonical_roots.len());
        for (index, root) in canonical_roots.iter().enumerate() {
            let child_key = format!("{idempotency_key}:{}", index + 1);
            if !valid_idempotency_key(&child_key) {
                return Err(ApplicationError::Invalid);
            }
            registrations.push(self.register_project_inner(root, &child_key)?);
        }
        let (catalog_snapshot, _) = self.refresh_project_catalog()?;
        Ok(MultiRootDiscoveryResult {
            registrations,
            catalog_snapshot,
        })
    }

    pub fn scan_project(
        &self,
        project_id: &ProjectId,
        idempotency_key: &str,
    ) -> Result<ScanProjectResult, ApplicationError> {
        let _guard = self.command_guard()?;
        self.scan_project_inner(project_id, idempotency_key)
    }

    pub fn scan_project_with_mode(
        &self,
        project_id: &ProjectId,
        idempotency_key: &str,
        scan_mode: IndexScanMode,
    ) -> Result<ScanProjectResult, ApplicationError> {
        let _guard = self.command_guard()?;
        self.scan_project_inner_with_mode(project_id, idempotency_key, scan_mode)
    }

    fn scan_project_inner(
        &self,
        project_id: &ProjectId,
        idempotency_key: &str,
    ) -> Result<ScanProjectResult, ApplicationError> {
        self.scan_project_inner_with_mode(project_id, idempotency_key, IndexScanMode::Incremental)
    }

    fn scan_project_inner_with_mode(
        &self,
        project_id: &ProjectId,
        idempotency_key: &str,
        scan_mode: IndexScanMode,
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
                "scan_mode":scan_mode,
                "adapters":adapter_cache_fingerprints,
            }),
        )
        .map_err(|_| ApplicationError::Invalid)?;
        let (sources, mut symbols) =
            observation.source_graph(project_id, &workspace_snapshot_id, &scan_run_id)?;
        let repository = self.repositories.project(project_id)?;
        let mut stored_previous = if scan_mode == IndexScanMode::Incremental {
            repository.latest_code_index_projection()?
        } else {
            None
        };
        if scan_mode == IndexScanMode::Incremental
            && stored_previous.is_none()
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
            scan_mode,
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
        let mut projection = analyze_builtin_findings(
            project_id,
            &observation.revision,
            &workspace_snapshot_id,
            &scan_run_id,
            &observation.files,
            &sources,
            &symbols,
            &code_index.snapshot.hardcoding_candidates,
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

    pub fn index_files(
        &self,
        project_id: &ProjectId,
        query: Option<&str>,
        require_current: bool,
    ) -> Result<IndexQueryResult<SourceEntry>, ApplicationError> {
        if query.is_some_and(|query| query.trim().is_empty() || query.chars().count() > 256) {
            return Err(ApplicationError::Invalid);
        }
        let (projection, current) = self.load_index_projection_with_freshness(project_id)?;
        if require_current && !current {
            return Err(ApplicationError::IndexNotCurrent);
        }
        let query = query.map(str::to_lowercase);
        let items = projection
            .source_entries
            .iter()
            .filter(|source| {
                query
                    .as_ref()
                    .is_none_or(|query| source.path.as_str().to_lowercase().contains(query))
            })
            .take(256)
            .cloned()
            .collect();
        Ok(index_query_result(
            &projection.snapshot,
            IndexTier::Text,
            current,
            items,
            IndexPartitionKind::Inventory,
        ))
    }

    pub fn index_hardcoding_candidates(
        &self,
        project_id: &ProjectId,
        require_current: bool,
    ) -> Result<IndexQueryResult<HardcodingCandidate>, ApplicationError> {
        let (projection, current) = self.load_index_projection_with_freshness(project_id)?;
        if require_current && !current {
            return Err(ApplicationError::IndexNotCurrent);
        }
        let items = projection
            .snapshot
            .hardcoding_candidates
            .iter()
            .take(256)
            .cloned()
            .collect();
        Ok(index_query_result(
            &projection.snapshot,
            IndexTier::Text,
            current,
            items,
            IndexPartitionKind::Finding,
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

    pub fn managed_registry_resolution_context(
        &self,
        project_id: &ProjectId,
    ) -> Result<ManagedRegistryResolutionContext, ApplicationError> {
        let project = self
            .repositories
            .global()
            .get_project(project_id)?
            .ok_or(ApplicationError::NotFound)?;
        let project_root = self.primary_project_root(&project)?;
        let (projection, index_current) = self.load_index_projection_with_freshness(project_id)?;
        let snapshot = &projection.snapshot;
        let coverage_complete = !snapshot.coverage.is_empty()
            && snapshot.coverage.iter().all(|coverage| {
                coverage.failed_count == 0
                    && coverage
                        .succeeded_count
                        .saturating_add(coverage.excluded_count)
                        == coverage.target_count
            });
        Ok(ManagedRegistryResolutionContext {
            project_root,
            owner_project_id: project_id.clone(),
            checkout_id: snapshot.checkout_id.clone(),
            project_revision_id: snapshot.project_revision_id.clone(),
            workspace_snapshot_id: snapshot.workspace_snapshot_id.clone(),
            code_index_snapshot_id: snapshot.code_index_snapshot_id.clone(),
            index_current,
            coverage_complete,
        })
    }

    pub fn publish_managed_registry_resolution(
        &self,
        snapshot: &ManagedRegistrySnapshot,
        consistency_records: &[RegistryConsistencyRecord],
    ) -> Result<PublishedManagedRegistryResolution, ApplicationError> {
        let _guard = self
            .command_lock
            .lock()
            .map_err(|_| ApplicationError::Apply("application command lock is poisoned".into()))?;
        self.publish_managed_registry_resolution_inner(snapshot, consistency_records)
    }

    fn publish_managed_registry_resolution_inner(
        &self,
        snapshot: &ManagedRegistrySnapshot,
        consistency_records: &[RegistryConsistencyRecord],
    ) -> Result<PublishedManagedRegistryResolution, ApplicationError> {
        let project = self
            .repositories
            .global()
            .get_project(&snapshot.owner_project_id)?
            .ok_or(ApplicationError::NotFound)?;
        let project_root = self.primary_project_root(&project)?;
        let snapshot_id = snapshot.managed_registry_snapshot_id.as_str();
        let snapshot_value =
            serde_json::to_value(snapshot).map_err(|_| ApplicationError::Invalid)?;
        let consistency_value =
            serde_json::to_value(consistency_records).map_err(|_| ApplicationError::Invalid)?;
        let artifact_refs = vec![
            self.artifacts.put_json_with_policy(ArtifactWriteRequest {
                project_id: &snapshot.owner_project_id,
                project_root: &project_root,
                relative_path: &format!("managed-registry/{snapshot_id}/snapshot.json"),
                subject_kind: "managed_registry_snapshot",
                subject_id: snapshot_id,
                policy: ArtifactWritePolicy {
                    kind: ArtifactKind::Manifest,
                    redaction_status: RedactionStatus::NotNeeded,
                    retention_class: RetentionClass::Evidence,
                },
                value: &snapshot_value,
            })?,
            self.artifacts.put_json_with_policy(ArtifactWriteRequest {
                project_id: &snapshot.owner_project_id,
                project_root: &project_root,
                relative_path: &format!("managed-registry/{snapshot_id}/consistency.json"),
                subject_kind: "registry_consistency_records",
                subject_id: snapshot_id,
                policy: ArtifactWritePolicy {
                    kind: ArtifactKind::Report,
                    redaction_status: RedactionStatus::NotNeeded,
                    retention_class: RetentionClass::Evidence,
                },
                value: &consistency_value,
            })?,
        ];
        self.repositories
            .project(&snapshot.owner_project_id)?
            .save_managed_registry_resolution(snapshot, consistency_records)?;
        Ok(PublishedManagedRegistryResolution {
            snapshot: snapshot.clone(),
            consistency_records: consistency_records.to_vec(),
            artifact_refs,
        })
    }

    pub fn refresh_managed_registry_resolution(
        &self,
        project_id: &ProjectId,
        manifest_path: &ProjectPathRef,
    ) -> Result<PublishedManagedRegistryResolution, ApplicationError> {
        let _guard = self
            .command_lock
            .lock()
            .map_err(|_| ApplicationError::Apply("application command lock is poisoned".into()))?;
        self.refresh_managed_registry_resolution_inner(project_id, manifest_path)
    }

    fn refresh_managed_registry_resolution_inner(
        &self,
        project_id: &ProjectId,
        manifest_path: &ProjectPathRef,
    ) -> Result<PublishedManagedRegistryResolution, ApplicationError> {
        let resolver = self.managed_registry_resolver.as_ref().ok_or_else(|| {
            ApplicationError::Apply("MANAGED_REGISTRY_RESOLVER_UNAVAILABLE".to_owned())
        })?;
        let context = self.managed_registry_resolution_context(project_id)?;
        let consumer_projects = self.managed_registry_consumer_projects()?;
        let resolved = resolver
            .resolve(ManagedRegistryResolveRequest {
                project_root: context.project_root,
                manifest_path: manifest_path.clone(),
                owner_project_id: context.owner_project_id,
                checkout_id: context.checkout_id,
                project_revision_id: context.project_revision_id,
                workspace_snapshot_id: context.workspace_snapshot_id,
                code_index_snapshot_id: context.code_index_snapshot_id,
                index_current: context.index_current,
                coverage_complete: context.coverage_complete,
                consumer_projects,
            })
            .map_err(|error| ApplicationError::Apply(error.to_string()))?;
        self.publish_managed_registry_resolution_inner(
            &resolved.snapshot,
            &resolved.consistency_records,
        )
    }

    fn managed_registry_consumer_projects(
        &self,
    ) -> Result<Vec<ManagedRegistryConsumerProjectInput>, ApplicationError> {
        let mut inputs = Vec::new();
        let mut projects = self.repositories.global().list_projects()?;
        projects.sort_by(|left, right| left.project_id.cmp(&right.project_id));
        for project in projects {
            let Ok(project_root) = self.primary_project_root(&project) else {
                continue;
            };
            let Ok((projection, index_current)) =
                self.load_index_projection_with_freshness(&project.project_id)
            else {
                continue;
            };
            let coverage_complete = !projection.snapshot.coverage.is_empty()
                && projection.snapshot.coverage.iter().all(|coverage| {
                    coverage.failed_count == 0
                        && coverage
                            .succeeded_count
                            .saturating_add(coverage.excluded_count)
                            == coverage.target_count
                });
            let mut source_entries = projection
                .source_entries
                .into_iter()
                .filter(|entry| entry.analysis_eligible)
                .collect::<Vec<_>>();
            source_entries.sort_by(|left, right| left.path.cmp(&right.path));
            inputs.push(ManagedRegistryConsumerProjectInput {
                project_id: project.project_id,
                project_root,
                source_entries,
                index_current,
                coverage_complete,
            });
        }
        Ok(inputs)
    }

    pub fn latest_managed_registry_snapshot(
        &self,
        project_id: &ProjectId,
    ) -> Result<Option<ManagedRegistrySnapshot>, ApplicationError> {
        Ok(self
            .repositories
            .project(project_id)?
            .latest_managed_registry_snapshot()?)
    }

    pub fn development_project_root(
        &self,
        project_id: &ProjectId,
    ) -> Result<PathBuf, ApplicationError> {
        let project = self
            .repositories
            .global()
            .get_project(project_id)?
            .ok_or(ApplicationError::NotFound)?;
        self.primary_project_root(&project)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "single-writer publication keeps identity, revision, state, and schema fields explicit"
    )]
    pub fn publish_development_document<T: Serialize>(
        &self,
        record_kind: &str,
        record_id: &str,
        revision: u64,
        project_id: Option<ProjectId>,
        state: &str,
        document_schema_id: &str,
        document_schema_version: u32,
        document: &T,
    ) -> Result<DevelopmentRecord, ApplicationError> {
        let _guard = self.command_guard()?;
        let document = serde_json::to_value(document).map_err(|_| ApplicationError::Invalid)?;
        if document
            .get("schema_id")
            .and_then(serde_json::Value::as_str)
            != Some(document_schema_id)
            || document
                .get("schema_version")
                .and_then(serde_json::Value::as_u64)
                != Some(u64::from(document_schema_version))
        {
            return Err(ApplicationError::Invalid);
        }
        let record = DevelopmentRecord {
            schema_version: 1,
            record_kind: record_kind.to_owned(),
            record_id: record_id.to_owned(),
            revision,
            project_id,
            state: state.to_owned(),
            document_schema_id: document_schema_id.to_owned(),
            document_schema_version,
            document_fingerprint: canonical_sha256(&document)
                .map_err(|_| ApplicationError::Invalid)?,
            document,
            created_at: Utc::now().to_rfc3339(),
        };
        self.repositories.global().put_development_record(&record)?;
        Ok(record)
    }

    pub fn get_development_record(
        &self,
        record_kind: &str,
        record_id: &str,
        revision: Option<u64>,
    ) -> Result<Option<DevelopmentRecord>, ApplicationError> {
        let _guard = self.command_guard()?;
        Ok(self
            .repositories
            .global()
            .get_development_record(record_kind, record_id, revision)?)
    }

    pub fn list_development_records(
        &self,
        record_kind: &str,
        project_id: Option<&ProjectId>,
    ) -> Result<Vec<DevelopmentRecord>, ApplicationError> {
        let _guard = self.command_guard()?;
        Ok(self
            .repositories
            .global()
            .list_development_records(record_kind, project_id)?)
    }

    pub fn get_managed_registry_snapshot(
        &self,
        project_id: &ProjectId,
        snapshot_id: &star_contracts::ManagedRegistrySnapshotId,
    ) -> Result<Option<ManagedRegistrySnapshot>, ApplicationError> {
        Ok(self
            .repositories
            .project(project_id)?
            .get_managed_registry_snapshot(snapshot_id)?)
    }

    pub fn list_registry_consistency_records(
        &self,
        project_id: &ProjectId,
        snapshot_id: &star_contracts::ManagedRegistrySnapshotId,
    ) -> Result<Vec<RegistryConsistencyRecord>, ApplicationError> {
        Ok(self
            .repositories
            .project(project_id)?
            .list_registry_consistency_records(snapshot_id)?)
    }

    pub fn list_findings(&self, project_id: &ProjectId) -> Result<Vec<Finding>, ApplicationError> {
        Ok(self.repositories.project(project_id)?.list_findings()?)
    }

    fn collect_planning_inputs(
        &self,
        task: &TaskSpecDraft,
    ) -> Result<PlanningInputSnapshot, ApplicationError> {
        let mut targets = task.project_targets.clone();
        targets.sort_by(|left, right| left.project_id.cmp(&right.project_id));
        if targets.is_empty()
            || targets
                .windows(2)
                .any(|pair| pair[0].project_id == pair[1].project_id)
        {
            return Err(ApplicationError::Invalid);
        }
        let (catalog, _) = self.refresh_project_catalog()?;
        let managed_declaration_ids = task
            .included_scope
            .iter()
            .chain(task.intended_changes.iter().map(|change| &change.selector))
            .filter(|selector| selector.kind == SelectorKind::ManagedDeclaration)
            .map(|selector| selector.value.as_str())
            .collect::<BTreeSet<_>>();
        let mut task_registries = Vec::new();
        if !managed_declaration_ids.is_empty() {
            for target in targets
                .iter()
                .filter(|target| target.role == ProjectTargetRole::PlannedChange)
            {
                if let Some(registry) = self
                    .repositories
                    .project(&target.project_id)?
                    .latest_managed_registry_snapshot()?
                    .filter(|registry| {
                        registry.declarations.iter().any(|declaration| {
                            managed_declaration_ids
                                .contains(declaration.managed_declaration_id.as_str())
                        })
                    })
                {
                    task_registries.push(registry);
                }
            }
        }
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
            let own_managed_registry_snapshot = self
                .repositories
                .project(&target.project_id)?
                .latest_managed_registry_snapshot()?
                .filter(|registry| {
                    registry.project_revision_id == projection.snapshot.project_revision_id
                        && registry.workspace_snapshot_id
                            == projection.snapshot.workspace_snapshot_id
                        && registry.code_index_snapshot_id.as_ref()
                            == Some(&projection.snapshot.code_index_snapshot_id)
                });
            let mut consumer_registries = task_registries.iter().filter(|registry| {
                registry.owner_project_id != target.project_id
                    && registry.declarations.iter().any(|declaration| {
                        managed_declaration_ids
                            .contains(declaration.managed_declaration_id.as_str())
                            && declaration
                                .consumer_contracts
                                .iter()
                                .any(|contract| contract.project_id == target.project_id)
                    })
            });
            let external_managed_registry_snapshot = consumer_registries.next().cloned();
            if consumer_registries.next().is_some() {
                return Err(ApplicationError::Invalid);
            }
            let managed_registry_snapshot =
                own_managed_registry_snapshot.or(external_managed_registry_snapshot);
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
                managed_registry_snapshot,
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
        Ok(PlanningInputSnapshot {
            catalog,
            projects,
            pinned_snapshots,
        })
    }

    fn expand_managed_registry_impact_targets(
        &self,
        mut task: TaskSpecDraft,
    ) -> Result<TaskSpecDraft, ApplicationError> {
        let declaration_ids = task
            .included_scope
            .iter()
            .chain(task.intended_changes.iter().map(|change| &change.selector))
            .filter(|selector| selector.kind == SelectorKind::ManagedDeclaration)
            .map(|selector| selector.value.clone())
            .collect::<BTreeSet<_>>();
        if declaration_ids.is_empty() {
            return Ok(task);
        }
        let existing = task
            .project_targets
            .iter()
            .map(|target| target.project_id.clone())
            .collect::<BTreeSet<_>>();
        let mut additions = BTreeMap::new();
        for target in task
            .project_targets
            .iter()
            .filter(|target| target.role == ProjectTargetRole::PlannedChange)
        {
            let Some(registry) = self
                .repositories
                .project(&target.project_id)?
                .latest_managed_registry_snapshot()?
            else {
                continue;
            };
            for declaration in registry.declarations.iter().filter(|declaration| {
                declaration_ids.contains(declaration.managed_declaration_id.as_str())
            }) {
                for contract in &declaration.consumer_contracts {
                    if existing.contains(&contract.project_id)
                        || additions.contains_key(&contract.project_id)
                    {
                        continue;
                    }
                    self.repositories
                        .global()
                        .get_project(&contract.project_id)?
                        .ok_or(ApplicationError::NotFound)?;
                    let (projection, current) =
                        self.load_index_projection_with_freshness(&contract.project_id)?;
                    if !current {
                        return Err(ApplicationError::IndexNotCurrent);
                    }
                    additions.insert(
                        contract.project_id.clone(),
                        ProjectTarget {
                            project_id: contract.project_id.clone(),
                            checkout_id: projection.snapshot.checkout_id,
                            role: ProjectTargetRole::ReadOnlyImpact,
                            reason: format!(
                                "managed declaration consumer {}",
                                declaration.managed_declaration_id
                            ),
                        },
                    );
                }
            }
        }
        task.project_targets.extend(additions.into_values());
        task.project_targets.sort_by(|left, right| {
            (&left.project_id, &left.checkout_id).cmp(&(&right.project_id, &right.checkout_id))
        });
        Ok(task)
    }

    fn verify_planning_pins(
        &self,
        pinned_snapshots: &[(ProjectId, CodeIndexSnapshotId)],
    ) -> Result<(), ApplicationError> {
        for (project_id, snapshot_id) in pinned_snapshots {
            let (projection, current) = self.load_index_projection_with_freshness(project_id)?;
            if !current || projection.snapshot.code_index_snapshot_id != *snapshot_id {
                return Err(ApplicationError::IndexNotCurrent);
            }
        }
        Ok(())
    }

    fn resolve_planning_check_descriptors(
        &self,
        projects: &[PlanningProjectIndex],
        mut explicit: Vec<CheckDescriptor>,
    ) -> Result<Vec<CheckDescriptor>, ApplicationError> {
        let global_families = explicit
            .iter()
            .filter(|descriptor| descriptor.project_ids.is_empty())
            .map(|descriptor| descriptor.family.clone())
            .collect::<BTreeSet<_>>();
        let mut discovered = BTreeMap::new();
        for project in projects {
            for toolchain in &project.snapshot.toolchains {
                for command in &toolchain.commands {
                    let Some(family) = toolchain_check_family(&command.command_id) else {
                        continue;
                    };
                    if global_families.contains(family)
                        || explicit.iter().any(|descriptor| {
                            descriptor.family == family
                                && descriptor
                                    .project_ids
                                    .contains(&project.snapshot.project_id)
                        })
                    {
                        continue;
                    }
                    let key = (project.snapshot.project_id.clone(), family.to_owned());
                    if discovered.contains_key(&key) {
                        continue;
                    }
                    let content_fingerprint = versioned_fingerprint(
                        "star.toolchain-check-descriptor",
                        1,
                        &serde_json::json!({
                            "project_id":project.snapshot.project_id,
                            "checkout_id":project.snapshot.checkout_id,
                            "toolchain":toolchain.content_fingerprint,
                            "command_id":command.command_id,
                            "executable_hint":command.executable_hint,
                            "args":command.args,
                            "cwd_scope":command.cwd_scope,
                            "declaration_kind":command.declaration_kind,
                            "family":family,
                        }),
                    )
                    .map_err(|_| ApplicationError::Invalid)?;
                    let short = &content_fingerprint.as_str()[7..23];
                    let mut supported_scope_levels = vec![
                        star_contracts::planning::ValidationScopeLevel::Workspace,
                        star_contracts::planning::ValidationScopeLevel::ProjectFull,
                    ];
                    if command.cwd_scope.is_some() {
                        supported_scope_levels
                            .insert(0, star_contracts::planning::ValidationScopeLevel::Package);
                    }
                    discovered.insert(
                        key,
                        CheckDescriptor {
                            check_id: format!("star.toolchain.{family}.{short}"),
                            family: family.to_owned(),
                            project_ids: vec![project.snapshot.project_id.clone()],
                            tool_id: format!(
                                "star.project.toolchain.{}",
                                toolchain
                                    .build_system
                                    .as_deref()
                                    .unwrap_or("declared-command")
                            ),
                            logical_executable: command.executable_hint.clone(),
                            argument_template: command.args.clone(),
                            supported_scope_levels,
                            applicable_source_classes: toolchain_check_source_classes(family),
                            trusted: matches!(
                                command.declaration_kind,
                                ToolchainCommandKind::Declared | ToolchainCommandKind::Suggested
                            ),
                            available: logical_executable_available(&command.executable_hint),
                            required_evidence: vec![
                                "validation_result".to_owned(),
                                "observed_tool_identity".to_owned(),
                            ],
                            content_fingerprint,
                        },
                    );
                }
            }
            let checkout = self
                .repositories
                .global()
                .get_project_checkout(&project.snapshot.checkout_id)?
                .ok_or(ApplicationError::NotFound)?;
            let Some(root_binding_id) = checkout.root_binding_id.as_ref() else {
                continue;
            };
            let root = self.root_bindings.resolve(root_binding_id)?;
            let project_manifest = root.join(".star-control/project.toml");
            let validation_entrypoint = root.join("scripts/validate.ps1");
            if !project_manifest.is_file() || !validation_entrypoint.is_file() {
                continue;
            }
            let manifest_bytes = std::fs::read(&project_manifest)
                .ok()
                .filter(|bytes| bytes.len() <= 8 * 1024 * 1024)
                .ok_or(ApplicationError::Invalid)?;
            let entrypoint_bytes = std::fs::read(&validation_entrypoint)
                .ok()
                .filter(|bytes| bytes.len() <= 8 * 1024 * 1024)
                .ok_or(ApplicationError::Invalid)?;
            let source_classes = vec![
                SourceClass::Source,
                SourceClass::Test,
                SourceClass::Docs,
                SourceClass::Config,
                SourceClass::Schema,
                SourceClass::Migration,
                SourceClass::Generated,
                SourceClass::Unknown,
            ];
            for family in [
                "format",
                "lint",
                "build",
                "test",
                "docs",
                "config",
                "contract",
                "migration",
                "generation",
                "architecture",
                "hardcoding",
                "security",
                "dependency",
                "regression",
                "validator_guard",
                "project_full",
            ] {
                if global_families.contains(family)
                    || explicit.iter().any(|descriptor| {
                        descriptor.family == family
                            && descriptor
                                .project_ids
                                .contains(&project.snapshot.project_id)
                    })
                    || discovered
                        .contains_key(&(project.snapshot.project_id.clone(), family.to_owned()))
                {
                    continue;
                }
                let (tool_id, logical_executable, args) = match family {
                    "format" => (
                        "star.tool.cargo",
                        "cargo",
                        vec!["fmt", "--all", "--", "--check"],
                    ),
                    "lint" => (
                        "star.tool.cargo",
                        "cargo",
                        vec![
                            "clippy",
                            "--workspace",
                            "--all-targets",
                            "--all-features",
                            "--locked",
                            "--",
                            "-D",
                            "warnings",
                        ],
                    ),
                    "build" => (
                        "star.tool.cargo",
                        "cargo",
                        vec!["check", "--workspace", "--all-targets", "--locked"],
                    ),
                    "test" => (
                        "star.tool.cargo",
                        "cargo",
                        vec!["test", "--workspace", "--locked"],
                    ),
                    "docs" | "config" => (
                        "star.project.validation-entrypoint",
                        "pwsh",
                        vec![
                            "-NoLogo",
                            "-NoProfile",
                            "-File",
                            "scripts/validate.ps1",
                            "-Profile",
                            "quick",
                            "-OutputFormat",
                            "json",
                        ],
                    ),
                    _ => (
                        "star.project.validation-entrypoint",
                        "pwsh",
                        vec![
                            "-NoLogo",
                            "-NoProfile",
                            "-File",
                            "scripts/validate.ps1",
                            "-Profile",
                            "target",
                            "-OutputFormat",
                            "json",
                        ],
                    ),
                };
                let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
                let content_fingerprint = versioned_fingerprint(
                    "star.project-check-descriptor",
                    1,
                    &serde_json::json!({
                        "project_id":project.snapshot.project_id,
                        "checkout_id":project.snapshot.checkout_id,
                        "family":family,
                        "tool_id":tool_id,
                        "logical_executable":logical_executable,
                        "args":args,
                        "project_manifest_sha256":Sha256Hash::digest(&manifest_bytes),
                        "validation_entrypoint_sha256":Sha256Hash::digest(&entrypoint_bytes),
                    }),
                )
                .map_err(|_| ApplicationError::Invalid)?;
                let short = &content_fingerprint.as_str()[7..23];
                discovered.insert(
                    (project.snapshot.project_id.clone(), family.to_owned()),
                    CheckDescriptor {
                        check_id: format!("star.project.{family}.{short}"),
                        family: family.to_owned(),
                        project_ids: vec![project.snapshot.project_id.clone()],
                        tool_id: tool_id.to_owned(),
                        logical_executable: logical_executable.to_owned(),
                        argument_template: args,
                        supported_scope_levels: vec![
                            star_contracts::planning::ValidationScopeLevel::Package,
                            star_contracts::planning::ValidationScopeLevel::Workspace,
                            star_contracts::planning::ValidationScopeLevel::ProjectFull,
                        ],
                        applicable_source_classes: source_classes.clone(),
                        trusted: true,
                        available: logical_executable_available(logical_executable),
                        required_evidence: vec![
                            "validation_result".to_owned(),
                            "observed_tool_identity".to_owned(),
                        ],
                        content_fingerprint,
                    },
                );
            }
        }
        explicit.extend(discovered.into_values());
        explicit.sort_by(|left, right| {
            (&left.family, &left.project_ids, &left.check_id).cmp(&(
                &right.family,
                &right.project_ids,
                &right.check_id,
            ))
        });
        Ok(explicit)
    }

    fn replay_planning_command(
        &self,
        idempotency_key: &str,
        input_fingerprint: &Sha256Hash,
    ) -> Result<Option<PlanningBundle>, ApplicationError> {
        let Some((existing, stored_input)) = self
            .repositories
            .global()
            .get_planning_bundle_by_idempotency_key(idempotency_key)?
        else {
            return Ok(None);
        };
        if &stored_input != input_fingerprint {
            return Err(ApplicationError::Repository(RepositoryError::new(
                RepositoryErrorCategory::IdempotencyConflict,
                "planning idempotency key was already used for different input",
            )));
        }
        Ok(Some(existing))
    }

    fn collect_previous_success_evidence(
        &self,
        task_spec_id: &TaskSpecId,
        targets: &[star_contracts::planning::ProjectTarget],
    ) -> Result<Vec<PreviousSuccessEvidence>, ApplicationError> {
        let history = self
            .repositories
            .global()
            .list_planning_bundle_revisions(task_spec_id)?;
        let mut candidates = Vec::new();
        for target in targets.iter().filter(|target| {
            target.role != star_contracts::planning::ProjectTargetRole::ReadOnlyImpact
        }) {
            for evidence in self
                .repositories
                .project(&target.project_id)?
                .list_evidence_bundles_v2()?
            {
                if evidence.authoritative_gate_state != AuthoritativeGateState::Passed
                    || evidence.completeness != star_contracts::evidence::Completeness::Complete
                {
                    continue;
                }
                let Some(previous) = history.iter().find(|bundle| {
                    bundle.validation_plan.validation_plan_id.as_str()
                        == evidence.validation_plan_ref.document_id
                        && bundle.validation_plan.revision == evidence.validation_plan_ref.revision
                }) else {
                    continue;
                };
                let plan_value = serde_json::to_value(&previous.validation_plan)
                    .map_err(|_| ApplicationError::Invalid)?;
                let plan_hash = star_contracts::canonical_sha256(&plan_value)
                    .map_err(|_| ApplicationError::Invalid)?;
                if plan_hash != evidence.validation_plan_ref.sha256 {
                    continue;
                }
                candidates.push(PreviousSuccessEvidence {
                    project_id: target.project_id.clone(),
                    evidence_bundle_id: evidence.evidence_bundle_id.to_string(),
                    bundle_fingerprint: evidence.bundle_fingerprint,
                    validation_plan: previous.validation_plan.clone(),
                    source_snapshot_refs: previous.scope_revision.source_snapshot_refs.clone(),
                });
            }
        }
        candidates.sort_by(|left, right| {
            (&left.project_id, &left.evidence_bundle_id)
                .cmp(&(&right.project_id, &right.evidence_bundle_id))
        });
        candidates.dedup_by(|left, right| {
            left.project_id == right.project_id
                && left.evidence_bundle_id == right.evidence_bundle_id
        });
        Ok(candidates)
    }

    pub fn create_planning_bundle(
        &self,
        task: TaskSpecDraft,
        actor: ActorRef,
        check_descriptors: Vec<CheckDescriptor>,
        idempotency_key: &str,
    ) -> Result<PlanningBundle, ApplicationError> {
        let _guard = self.command_guard()?;
        self.create_planning_bundle_for_phase_inner(
            task,
            actor,
            check_descriptors,
            idempotency_key,
            "during_stage",
            None,
        )
    }

    fn resolve_planning_profiles(
        &self,
        task: &TaskSpecDraft,
    ) -> Result<Option<star_contracts::profile::DevelopmentProfileResolutionV1>, ApplicationError>
    {
        if task.profile_ids.is_empty() {
            return Ok(None);
        }
        Ok(Some(self.resolve_development_profiles(&task.profile_ids)?))
    }

    fn create_planning_bundle_for_phase_inner(
        &self,
        task: TaskSpecDraft,
        actor: ActorRef,
        check_descriptors: Vec<CheckDescriptor>,
        idempotency_key: &str,
        validation_phase: &str,
        observed_change_override: Option<(ProjectId, Vec<ObservedWorkspaceChange>)>,
    ) -> Result<PlanningBundle, ApplicationError> {
        let task = self.expand_managed_registry_impact_targets(task)?;
        let profile_resolution = self.resolve_planning_profiles(&task)?;
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
                "validation_phase":validation_phase,
                "observed_change_override":observed_change_override,
                "profile_resolution":profile_resolution,
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
        let mut inputs = self.collect_planning_inputs(&task)?;
        if let Some((override_project_id, observed_changes)) = observed_change_override {
            let project = inputs
                .projects
                .iter_mut()
                .find(|project| project.snapshot.project_id == override_project_id)
                .ok_or(ApplicationError::Invalid)?;
            project.observed_changes = observed_changes;
            project.collection_state = CollectionState::Complete;
            project.collection_limits.clear();
        }
        let check_descriptors =
            self.resolve_planning_check_descriptors(&inputs.projects, check_descriptors)?;
        let bundle = build_planning_bundle_for_phase(
            PlanningRequest {
                task,
                actor,
                catalog: inputs.catalog,
                projects: inputs.projects,
                risk_descriptors: builtin_risk_descriptors()?,
                check_descriptors,
                previous_success_evidence: vec![],
                profile_resolution,
                policy: PlanningPolicy::default(),
            },
            validation_phase,
        )?;
        self.verify_planning_pins(&inputs.pinned_snapshots)?;
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

    #[allow(clippy::too_many_arguments)]
    pub fn revise_planning_bundle(
        &self,
        task_spec_id: &TaskSpecId,
        task: TaskSpecDraft,
        actor: ActorRef,
        check_descriptors: Vec<CheckDescriptor>,
        reason_code: ScopeReasonCode,
        reason: &str,
        user_decisions: Vec<ScopeUserDecision>,
        idempotency_key: &str,
    ) -> Result<PlanningBundle, ApplicationError> {
        let _guard = self.command_guard()?;
        if !valid_idempotency_key(idempotency_key)
            || task.project_targets.is_empty()
            || reason.trim().is_empty()
        {
            return Err(ApplicationError::Invalid);
        }
        let input_fingerprint = versioned_fingerprint(
            "star.command.planning-revise",
            1,
            &serde_json::json!({
                "task_spec_id":task_spec_id,
                "task":task,
                "actor":actor,
                "check_descriptors":check_descriptors,
                "reason_code":reason_code,
                "reason":reason,
                "user_decisions":user_decisions,
                "policy":PlanningPolicy::default(),
            }),
        )
        .map_err(|_| ApplicationError::Invalid)?;
        if let Some(existing) = self.replay_planning_command(idempotency_key, &input_fingerprint)? {
            return Ok(existing);
        }
        let previous = self
            .repositories
            .global()
            .get_planning_bundle(task_spec_id)?
            .ok_or(ApplicationError::NotFound)?;
        let previous_success_evidence =
            self.collect_previous_success_evidence(task_spec_id, &task.project_targets)?;
        let profile_resolution = self.resolve_planning_profiles(&task)?;
        let inputs = self.collect_planning_inputs(&task)?;
        let check_descriptors =
            self.resolve_planning_check_descriptors(&inputs.projects, check_descriptors)?;
        let bundle = build_revised_planning_bundle(PlanningRevisionRequest {
            previous,
            request: PlanningRequest {
                task,
                actor,
                catalog: inputs.catalog,
                projects: inputs.projects,
                risk_descriptors: builtin_risk_descriptors()?,
                check_descriptors,
                previous_success_evidence,
                profile_resolution,
                policy: PlanningPolicy::default(),
            },
            reason_code,
            reason: reason.to_owned(),
            user_decisions,
        })?;
        self.verify_planning_pins(&inputs.pinned_snapshots)?;
        Ok(self.repositories.global().put_planning_bundle(
            &bundle,
            idempotency_key,
            &input_fingerprint,
        )?)
    }

    pub fn replan_planning_bundle(
        &self,
        task_spec_id: &TaskSpecId,
        actor: ActorRef,
        check_descriptors: Vec<CheckDescriptor>,
        reason: &str,
        idempotency_key: &str,
    ) -> Result<PlanningBundle, ApplicationError> {
        let _guard = self.command_guard()?;
        if !valid_idempotency_key(idempotency_key) || reason.trim().is_empty() {
            return Err(ApplicationError::Invalid);
        }
        let input_fingerprint = versioned_fingerprint(
            "star.command.planning-replan",
            1,
            &serde_json::json!({
                "task_spec_id":task_spec_id,
                "actor":actor,
                "check_descriptors":check_descriptors,
                "reason":reason,
                "policy":PlanningPolicy::default(),
            }),
        )
        .map_err(|_| ApplicationError::Invalid)?;
        if let Some(existing) = self.replay_planning_command(idempotency_key, &input_fingerprint)? {
            return Ok(existing);
        }
        let previous = self
            .repositories
            .global()
            .get_planning_bundle(task_spec_id)?
            .ok_or(ApplicationError::NotFound)?;
        let task = task_spec_to_draft(&previous.task_spec);
        let profile_resolution = self.resolve_planning_profiles(&task)?;
        let previous_success_evidence =
            self.collect_previous_success_evidence(task_spec_id, &task.project_targets)?;
        let inputs = self.collect_planning_inputs(&task)?;
        let check_descriptors =
            self.resolve_planning_check_descriptors(&inputs.projects, check_descriptors)?;
        let bundle = build_revised_planning_bundle(PlanningRevisionRequest {
            previous,
            request: PlanningRequest {
                task,
                actor,
                catalog: inputs.catalog,
                projects: inputs.projects,
                risk_descriptors: builtin_risk_descriptors()?,
                check_descriptors,
                previous_success_evidence,
                profile_resolution,
                policy: PlanningPolicy::default(),
            },
            reason_code: ScopeReasonCode::SourceChanged,
            reason: reason.to_owned(),
            user_decisions: vec![],
        })?;
        self.verify_planning_pins(&inputs.pinned_snapshots)?;
        Ok(self.repositories.global().put_planning_bundle(
            &bundle,
            idempotency_key,
            &input_fingerprint,
        )?)
    }

    pub fn set_planning_check_override(
        &self,
        task_spec_id: &TaskSpecId,
        check_override: CheckOverride,
        actor: ActorRef,
        check_descriptors: Vec<CheckDescriptor>,
        idempotency_key: &str,
    ) -> Result<PlanningBundle, ApplicationError> {
        let _guard = self.command_guard()?;
        if !valid_idempotency_key(idempotency_key)
            || check_override.family.trim().is_empty()
            || check_override.reason.trim().is_empty()
        {
            return Err(ApplicationError::Invalid);
        }
        let input_fingerprint = versioned_fingerprint(
            "star.command.planning-check-override",
            1,
            &serde_json::json!({
                "task_spec_id":task_spec_id,
                "check_override":check_override,
                "actor":actor,
                "check_descriptors":check_descriptors,
                "policy":PlanningPolicy::default(),
            }),
        )
        .map_err(|_| ApplicationError::Invalid)?;
        if let Some(existing) = self.replay_planning_command(idempotency_key, &input_fingerprint)? {
            return Ok(existing);
        }
        let previous = self
            .repositories
            .global()
            .get_planning_bundle(task_spec_id)?
            .ok_or(ApplicationError::NotFound)?;
        let mut task = task_spec_to_draft(&previous.task_spec);
        task.check_overrides
            .retain(|existing| existing.family != check_override.family);
        task.check_overrides.push(check_override.clone());
        let profile_resolution = self.resolve_planning_profiles(&task)?;
        let previous_success_evidence =
            self.collect_previous_success_evidence(task_spec_id, &task.project_targets)?;
        let inputs = self.collect_planning_inputs(&task)?;
        let check_descriptors =
            self.resolve_planning_check_descriptors(&inputs.projects, check_descriptors)?;
        let bundle = build_revised_planning_bundle(PlanningRevisionRequest {
            previous,
            request: PlanningRequest {
                task,
                actor,
                catalog: inputs.catalog,
                projects: inputs.projects,
                risk_descriptors: builtin_risk_descriptors()?,
                check_descriptors,
                previous_success_evidence,
                profile_resolution,
                policy: PlanningPolicy::default(),
            },
            reason_code: ScopeReasonCode::UserEdit,
            reason: format!(
                "check_override:{}:{:?}",
                check_override.family, check_override.kind
            ),
            user_decisions: vec![],
        })?;
        self.verify_planning_pins(&inputs.pinned_snapshots)?;
        Ok(self.repositories.global().put_planning_bundle(
            &bundle,
            idempotency_key,
            &input_fingerprint,
        )?)
    }

    pub fn invalidate_planning_bundle(
        &self,
        task_spec_id: &TaskSpecId,
        actor: ActorRef,
        reason: &str,
        idempotency_key: &str,
    ) -> Result<PlanningBundle, ApplicationError> {
        let _guard = self.command_guard()?;
        if !valid_idempotency_key(idempotency_key) || reason.trim().is_empty() {
            return Err(ApplicationError::Invalid);
        }
        let input_fingerprint = versioned_fingerprint(
            "star.command.planning-invalidate",
            1,
            &serde_json::json!({
                "task_spec_id":task_spec_id,
                "actor":actor,
                "reason":reason,
            }),
        )
        .map_err(|_| ApplicationError::Invalid)?;
        if let Some(existing) = self.replay_planning_command(idempotency_key, &input_fingerprint)? {
            return Ok(existing);
        }
        let previous = self
            .repositories
            .global()
            .get_planning_bundle(task_spec_id)?
            .ok_or(ApplicationError::NotFound)?;
        let bundle = build_invalidated_planning_bundle(
            previous,
            &format!("{}:{}", actor.actor_id, reason.trim()),
        )?;
        Ok(self.repositories.global().put_planning_bundle(
            &bundle,
            idempotency_key,
            &input_fingerprint,
        )?)
    }

    pub fn planning_bundle_status(
        &self,
        task_spec_id: &TaskSpecId,
    ) -> Result<PlanningBundleStatus, ApplicationError> {
        let bundle = self.get_planning_bundle(task_spec_id)?;
        Ok(PlanningBundleStatus {
            task_spec_id: bundle.task_spec.task_spec_id.clone(),
            bundle_revision: planning_bundle_revision(&bundle),
            task_revision: bundle.task_spec.revision,
            scope_revision: bundle.scope_revision.revision,
            impact_revision: bundle.impact_analysis.revision,
            validation_revision: bundle.validation_plan.revision,
            scope_reason_code: bundle.scope_revision.reason_code,
            impact_status: bundle.impact_analysis.status,
            validation_readiness: bundle.validation_plan.readiness,
            source_snapshot_refs: bundle.scope_revision.source_snapshot_refs,
            bundle_fingerprint: bundle.bundle_fingerprint,
        })
    }

    pub fn planning_impact(
        &self,
        task_spec_id: &TaskSpecId,
    ) -> Result<ImpactAnalysis, ApplicationError> {
        Ok(self.get_planning_bundle(task_spec_id)?.impact_analysis)
    }

    pub fn planning_affected_checks(
        &self,
        task_spec_id: &TaskSpecId,
    ) -> Result<AffectedChecksView, ApplicationError> {
        let plan = self.get_planning_bundle(task_spec_id)?.validation_plan;
        Ok(AffectedChecksView {
            candidate_checks: plan.candidate_checks,
            required_checks: plan.required_checks,
            optional_checks: plan.optional_checks,
            omitted_checks: plan.omitted_checks,
            unresolved_checks: plan.unresolved_checks,
            fallback_decisions: plan.fallback_decisions,
            readiness: plan.readiness,
        })
    }

    pub fn list_planning_bundle_revisions(
        &self,
        task_spec_id: &TaskSpecId,
    ) -> Result<Vec<PlanningBundle>, ApplicationError> {
        Ok(self
            .repositories
            .global()
            .list_planning_bundle_revisions(task_spec_id)?)
    }

    pub fn preflight_planning_bundle_execution(
        &self,
        task_spec_id: &TaskSpecId,
        project_root: &Path,
    ) -> Result<ValidationExecutionPreflight, ApplicationError> {
        self.preflight_planning_bundle_execution_with_evidence(
            task_spec_id,
            project_root,
            vec![],
            None,
        )
    }

    pub fn preflight_planning_bundle_execution_with_claims(
        &self,
        task_spec_id: &TaskSpecId,
        project_root: &Path,
        completion_claims: Vec<CompletionClaimV2>,
    ) -> Result<ValidationExecutionPreflight, ApplicationError> {
        self.preflight_planning_bundle_execution_with_evidence(
            task_spec_id,
            project_root,
            completion_claims,
            None,
        )
    }

    pub fn preflight_planning_bundle_execution_with_evidence(
        &self,
        task_spec_id: &TaskSpecId,
        project_root: &Path,
        completion_claims: Vec<CompletionClaimV2>,
        validator_guard_evidence: Option<ValidatorGuardEvidenceV2>,
    ) -> Result<ValidationExecutionPreflight, ApplicationError> {
        Ok(self
            .prepare_planning_bundle_execution(
                task_spec_id,
                project_root,
                completion_claims,
                validator_guard_evidence,
                None,
            )?
            .preflight)
    }

    pub fn execute_planning_bundle_registered(
        &self,
        task_spec_id: &TaskSpecId,
        project_root: &Path,
        gate_scope: GateScope,
        decided_by: ActorRef,
        force_human_review: bool,
    ) -> Result<CheckGraphRunResult, ApplicationError> {
        self.execute_planning_bundle_registered_with_claims(
            task_spec_id,
            project_root,
            gate_scope,
            decided_by,
            force_human_review,
            vec![],
        )
    }

    pub fn execute_planning_bundle_registered_with_claims(
        &self,
        task_spec_id: &TaskSpecId,
        project_root: &Path,
        gate_scope: GateScope,
        decided_by: ActorRef,
        force_human_review: bool,
        completion_claims: Vec<CompletionClaimV2>,
    ) -> Result<CheckGraphRunResult, ApplicationError> {
        self.execute_planning_bundle_registered_with_evidence(
            task_spec_id,
            project_root,
            gate_scope,
            decided_by,
            force_human_review,
            RegisteredValidationExecutionEvidence {
                completion_claims,
                validator_guard_evidence: None,
            },
        )
    }

    pub fn execute_planning_bundle_registered_with_evidence(
        &self,
        task_spec_id: &TaskSpecId,
        project_root: &Path,
        gate_scope: GateScope,
        decided_by: ActorRef,
        force_human_review: bool,
        evidence: RegisteredValidationExecutionEvidence,
    ) -> Result<CheckGraphRunResult, ApplicationError> {
        let _guard = self.command_guard()?;
        let RegisteredValidationExecutionEvidence {
            completion_claims,
            validator_guard_evidence,
        } = evidence;
        self.execute_planning_bundle_registered_with_evidence_inner(
            task_spec_id,
            project_root,
            gate_scope,
            decided_by,
            force_human_review,
            completion_claims,
            validator_guard_evidence,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_planning_bundle_registered_with_evidence_inner(
        &self,
        task_spec_id: &TaskSpecId,
        project_root: &Path,
        gate_scope: GateScope,
        decided_by: ActorRef,
        force_human_review: bool,
        completion_claims: Vec<CompletionClaimV2>,
        validator_guard_evidence: Option<ValidatorGuardEvidenceV2>,
        execution_root_binding: Option<ValidationExecutionRootBinding>,
    ) -> Result<CheckGraphRunResult, ApplicationError> {
        let prepared = self.prepare_planning_bundle_execution(
            task_spec_id,
            project_root,
            completion_claims,
            validator_guard_evidence,
            execution_root_binding.as_ref(),
        )?;
        let project_id = prepared.preflight.project_id.clone();
        let repository = self.repositories.project(&project_id)?;
        let artifact_set_id = RequestId::new();
        let preflight_ref = self.artifacts.put_json_with_policy(ArtifactWriteRequest {
            project_id: &project_id,
            project_root,
            relative_path: &format!(
                "validation/m3/{}/{}/preflight.json",
                task_spec_id.as_str(),
                artifact_set_id.as_str()
            ),
            subject_kind: "validation_execution_preflight",
            subject_id: task_spec_id.as_str(),
            policy: ArtifactWritePolicy {
                kind: ArtifactKind::Input,
                redaction_status: RedactionStatus::NotNeeded,
                retention_class: RetentionClass::Evidence,
            },
            value: &serde_json::json!({
                "schema_id":"star.validation-execution-preflight",
                "schema_version":1,
                "artifact_set_id":artifact_set_id,
                "validation_plan_ref":prepared.preflight.validation_plan_ref,
                "items":prepared.preflight.items,
                "rule_diagnostics":prepared.preflight.rule_diagnostics,
                "decision_floor":prepared.preflight.decision_floor,
                "completion_claim_refs":prepared.preflight.completion_claim_refs,
                "validator_guard_evidence_ref":prepared.preflight.validator_guard_evidence_ref,
                "execution_root_kind":prepared.preflight.execution_root_kind,
                "execution_root_binding_fingerprint":prepared.preflight.execution_root_binding_fingerprint,
            }),
        })?;
        let mut guard_artifacts = prepared
            .validator_guard_evidence
            .as_ref()
            .map(|evidence| {
                evidence
                    .artifact_refs()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if let Some(evidence) = prepared.validator_guard_evidence.as_ref() {
            guard_artifacts.push(self.artifacts.put_json_with_policy(ArtifactWriteRequest {
                project_id: &project_id,
                project_root,
                relative_path: &format!(
                    "validation/m3/{}/{}/validator-guard-evidence.json",
                    task_spec_id.as_str(),
                    artifact_set_id.as_str()
                ),
                subject_kind: "validator_guard_evidence",
                subject_id: evidence.guard_evidence_id.as_str(),
                policy: ArtifactWritePolicy {
                    kind: ArtifactKind::Input,
                    redaction_status: RedactionStatus::NotNeeded,
                    retention_class: RetentionClass::Evidence,
                },
                value: &serde_json::to_value(evidence).map_err(|_| ApplicationError::Invalid)?,
            })?);
        }
        let context = CheckGraphRunContext {
            gate_scope,
            decided_by,
            artifact_manifest: ArtifactManifest {
                manifest_ref: preflight_ref.clone(),
                artifacts: vec![],
            },
            force_human_review,
            baselines: repository.list_baselines_v2()?,
            suppressions: repository.list_suppressions_v2()?,
            dispositions: repository.list_dispositions_v2()?,
            evaluation_time: Utc::now(),
            max_attempts_per_check: 2,
            preflight_diagnostics: prepared.preflight.rule_diagnostics.clone(),
            completion_claims: prepared.completion_claims.clone(),
            change_sets: prepared.change_sets.clone(),
        };
        let output_sink = ValidationOutputArtifactSink {
            artifacts: Arc::clone(&self.artifacts),
            project_id: project_id.clone(),
            project_root: project_root.to_path_buf(),
            task_spec_id: task_spec_id.clone(),
            artifact_set_id: artifact_set_id.clone(),
            redactor: PersistenceRedactor::for_current_user(),
        };
        let mut executor = RegisteredProcessCheckExecutor::new(prepared.resolved_executables)?
            .with_output_sink(Box::new(output_sink));
        let mut artifact_finalizer = ValidationArtifactManifestFinalizer {
            artifacts: Arc::clone(&self.artifacts),
            project_id,
            project_root: project_root.to_path_buf(),
            task_spec_id: task_spec_id.clone(),
            artifact_set_id,
            preflight_ref,
            initial_artifacts: guard_artifacts,
        };
        self.execute_planning_bundle_with_artifact_finalizer(
            task_spec_id,
            &prepared.bindings,
            context,
            &mut executor,
            &mut artifact_finalizer,
        )
    }

    fn prepare_planning_bundle_execution(
        &self,
        task_spec_id: &TaskSpecId,
        project_root: &Path,
        completion_claims: Vec<CompletionClaimV2>,
        validator_guard_evidence: Option<ValidatorGuardEvidenceV2>,
        execution_root_binding: Option<&ValidationExecutionRootBinding>,
    ) -> Result<PreparedValidationExecution, ApplicationError> {
        let bundle = self
            .repositories
            .global()
            .get_planning_bundle(task_spec_id)?
            .ok_or(ApplicationError::NotFound)?;
        if bundle.validation_plan.readiness != ValidationPlanV2Readiness::Ready
            || bundle.validation_plan.required_checks.is_empty()
            || !bundle.validation_plan.unresolved_checks.is_empty()
        {
            return Err(ApplicationError::Invalid);
        }
        let project_ids = bundle
            .validation_plan
            .required_checks
            .iter()
            .map(|check| check.project_id.clone())
            .collect::<BTreeSet<_>>();
        if project_ids.len() != 1 {
            return Err(ApplicationError::Invalid);
        }
        let project_id = project_ids
            .into_iter()
            .next()
            .ok_or(ApplicationError::Invalid)?;
        let source = bundle
            .scope_revision
            .source_snapshot_refs
            .iter()
            .find(|source| source.project_id == project_id)
            .ok_or(ApplicationError::Invalid)?;
        let checkout = self
            .repositories
            .global()
            .get_project_checkout(&source.checkout_id)?
            .ok_or(ApplicationError::NotFound)?;
        let root_binding_id = checkout
            .root_binding_id
            .as_ref()
            .ok_or(ApplicationError::Invalid)?;
        let expected_root = std::fs::canonicalize(self.root_bindings.resolve(root_binding_id)?)
            .map_err(|_| ApplicationError::Invalid)?;
        let actual_root =
            std::fs::canonicalize(project_root).map_err(|_| ApplicationError::Invalid)?;
        if expected_root != actual_root {
            return Err(ApplicationError::Invalid);
        }
        let project_root_fingerprint = Sha256Hash::digest(
            actual_root
                .as_os_str()
                .to_string_lossy()
                .replace('/', "\\")
                .to_ascii_lowercase()
                .as_bytes(),
        );
        let (execution_root, execution_root_kind, execution_root_binding_fingerprint) =
            if let Some(binding) = execution_root_binding {
                let runtime_root = self
                    .rust_style_runtime_root
                    .as_ref()
                    .ok_or(ApplicationError::Invalid)?
                    .canonicalize()
                    .map_err(|_| ApplicationError::Invalid)?;
                let root = binding
                    .root
                    .canonicalize()
                    .map_err(|_| ApplicationError::Invalid)?;
                if !matches!(
                    binding.kind,
                    "rust_style_candidate_preview" | "rust_style_actual_after_preview"
                ) || !root.starts_with(&runtime_root)
                    || root.starts_with(&actual_root)
                    || actual_root.starts_with(&root)
                    || validate_owned_preview_root(&root).is_err()
                {
                    return Err(ApplicationError::Invalid);
                }
                (
                    root,
                    binding.kind.to_owned(),
                    binding.binding_fingerprint.clone(),
                )
            } else {
                (
                    actual_root.clone(),
                    "project_root".to_owned(),
                    project_root_fingerprint.clone(),
                )
            };
        let (projection, current) = self.load_index_projection_with_freshness(&project_id)?;
        if !current
            || projection.snapshot.checkout_id != source.checkout_id
            || projection.snapshot.code_index_snapshot_id != source.code_index_snapshot_id
            || projection.snapshot.workspace_snapshot_id != source.workspace_snapshot_id
        {
            return Err(ApplicationError::IndexNotCurrent);
        }
        let workspace = self
            .repositories
            .project(&project_id)?
            .get_workspace_snapshot(&source.workspace_snapshot_id)?
            .ok_or(ApplicationError::NotFound)?;
        let plan_ref = DocumentRef {
            schema_id: star_contracts::planning::FULL_VALIDATION_PLAN_SCHEMA_ID.to_owned(),
            document_id: bundle.validation_plan.validation_plan_id.to_string(),
            revision: bundle.validation_plan.revision,
            sha256: application_document_hash(&bundle.validation_plan)?,
        };
        let task_spec_ref = DocumentRef {
            schema_id: star_contracts::planning::TASK_SPEC_SCHEMA_ID.to_owned(),
            document_id: bundle.task_spec.task_spec_id.to_string(),
            revision: bundle.task_spec.revision,
            sha256: application_document_hash(&bundle.task_spec)?,
        };
        let gate_phase = match bundle.validation_plan.phase.as_str() {
            "during_stage" => GatePhaseV2::DuringStage,
            "goal_exit" => GatePhaseV2::GoalExit,
            "patch_pre_apply" | "pre_apply" => GatePhaseV2::PatchPreApply,
            "patch_post_apply" | "post_apply" => GatePhaseV2::PatchPostApply,
            _ => return Err(ApplicationError::Invalid),
        };
        let gate_policy_fingerprint = versioned_fingerprint(
            "star.gate-policy-v2",
            EVIDENCE_V2_SCHEMA_VERSION,
            &bundle.validation_plan.gate_policy,
        )
        .map_err(|_| ApplicationError::Invalid)?;
        let validator_registry_fingerprint = versioned_fingerprint(
            "star.validator-registry-selection",
            EVIDENCE_V2_SCHEMA_VERSION,
            &bundle
                .validation_plan
                .required_checks
                .iter()
                .map(|check| (&check.check_id, &check.descriptor_ref))
                .collect::<Vec<_>>(),
        )
        .map_err(|_| ApplicationError::Invalid)?;
        let normalizer_fingerprint = versioned_fingerprint(
            "star.external-diagnostic-normalizer",
            EVIDENCE_V2_SCHEMA_VERSION,
            &"safe-exit-v1",
        )
        .map_err(|_| ApplicationError::Invalid)?;
        let mut bindings = Vec::new();
        let mut resolved_by_fingerprint = BTreeMap::new();
        let mut items = Vec::new();
        for check in &bundle.validation_plan.required_checks {
            let executable_path =
                resolve_logical_executable_path(&check.invocation.logical_executable)
                    .ok_or(ApplicationError::Invalid)?;
            let resolved = ResolvedExecutableV2::resolve(
                &check.invocation.logical_executable,
                &executable_path,
                &execution_root,
                "file-hash-bound",
            )?;
            let tool_ref = CatalogRef {
                catalog_id: check.tool_id.clone(),
                format_version: 1,
                item_version: "1.0.0".to_owned(),
                sha256: check.descriptor_ref.sha256.clone(),
            };
            let normalizer_rule_ref = CatalogRef {
                catalog_id: format!("star.normalizer.{}", check.family),
                format_version: 1,
                item_version: "1.0.0".to_owned(),
                sha256: normalizer_fingerprint.clone(),
            };
            let environment_fingerprint = versioned_fingerprint(
                "star.validation-execution-environment",
                EVIDENCE_V2_SCHEMA_VERSION,
                &serde_json::json!({
                    "os":std::env::consts::OS,
                    "arch":std::env::consts::ARCH,
                    "executable_binding":resolved.executable_binding_fingerprint,
                    "execution_root_kind":execution_root_kind,
                    "execution_root_binding_fingerprint":execution_root_binding_fingerprint,
                }),
            )
            .map_err(|_| ApplicationError::Invalid)?;
            let subject_binding = EvidenceSubjectBinding {
                project_id: project_id.clone(),
                checkout_id: source.checkout_id.clone(),
                project_revision_id: source.project_revision_id.clone(),
                workspace_snapshot_id: source.workspace_snapshot_id.clone(),
                workspace_content_fingerprint: workspace.entries_fingerprint.clone(),
                task_spec_ref: bundle.validation_plan.task_spec_ref.clone(),
                scope_revision_ref: bundle.validation_plan.scope_revision_ref.clone(),
                impact_analysis_ref: bundle.validation_plan.impact_analysis_ref.clone(),
                change_set_refs: bundle.validation_plan.change_set_refs.clone(),
                change_plan_refs: vec![],
                patch_set_ref: None,
                validation_plan_ref: plan_ref.clone(),
                gate_phase,
                profile_resolution_fingerprint: bundle
                    .validation_plan
                    .profile_resolution
                    .as_ref()
                    .map(|resolution| resolution.profile_resolution_fingerprint.clone())
                    .unwrap_or_else(|| bundle.validation_plan.selection_fingerprint.clone()),
                effective_config_fingerprint: bundle.validation_plan.config_fingerprint.clone(),
                gate_policy_fingerprint: gate_policy_fingerprint.clone(),
                catalog_snapshot_ref: bundle.validation_plan.catalog_snapshot_ref.clone(),
                validator_registry_fingerprint: validator_registry_fingerprint.clone(),
                check_descriptor_ref: Some(check.descriptor_ref.clone()),
                rule_refs: vec![normalizer_rule_ref.clone()],
                tool_registry_snapshot_ref: None,
                tool_descriptor_ref: Some(tool_ref.clone()),
                observed_tool_fingerprint: None,
                invocation_fingerprint: None,
                execution_environment_fingerprint: environment_fingerprint,
                normalizer_fingerprint: normalizer_fingerprint.clone(),
                freshness: EvidenceFreshnessV2::Current,
                stale_reasons: vec![],
                binding_fingerprint: Sha256Hash::digest(b""),
                probed_at: Utc::now(),
            }
            .seal()
            .map_err(|_| ApplicationError::Invalid)?;
            bindings.push(ExecutableBinding {
                check_id: check.check_id.clone(),
                check_ref: normalizer_rule_ref,
                tool_ref,
                logical_executable: check.invocation.logical_executable.clone(),
                executable_binding_fingerprint: resolved.executable_binding_fingerprint.clone(),
                cwd: InvocationWorkingDirectoryV2::ProjectRoot,
                permission_action: "local_validation".to_owned(),
                output_limits: OutputLimits {
                    stdout_bytes: 4 * 1024 * 1024,
                    stderr_bytes: 4 * 1024 * 1024,
                    artifact_bytes: 16 * 1024 * 1024,
                },
                subject_binding: subject_binding.clone(),
            });
            items.push(ValidationExecutionPreflightItem {
                plan_item_id: check.plan_item_id.clone(),
                check_id: check.check_id.clone(),
                project_id: project_id.clone(),
                logical_executable: check.invocation.logical_executable.clone(),
                executable_binding_fingerprint: resolved.executable_binding_fingerprint.clone(),
                descriptor_ref: check.descriptor_ref.clone(),
                subject_binding_fingerprint: subject_binding.binding_fingerprint,
            });
            resolved_by_fingerprint
                .entry(resolved.executable_binding_fingerprint.clone())
                .or_insert(resolved);
        }
        bindings.sort_by(|left, right| left.check_id.cmp(&right.check_id));
        items.sort_by(|left, right| left.plan_item_id.cmp(&right.plan_item_id));
        let validator_guard_evidence = validator_guard_evidence
            .map(|evidence| {
                let sealed = evidence
                    .clone()
                    .seal()
                    .map_err(|_| ApplicationError::Invalid)?;
                if sealed != evidence
                    || sealed.project_id != project_id
                    || sealed.task_spec_ref != task_spec_ref
                {
                    return Err(ApplicationError::Invalid);
                }
                Ok(sealed)
            })
            .transpose()?;
        let guard_artifacts_verified = validator_guard_evidence.as_ref().is_some_and(|evidence| {
            evidence
                .artifact_refs()
                .into_iter()
                .all(|reference| self.artifacts.verify(&actual_root, reference).is_ok())
        });
        let guard_input =
            validator_guard_evidence
                .as_ref()
                .map(|evidence| VerifiedValidatorGuardInput {
                    evidence,
                    artifacts_verified: guard_artifacts_verified,
                    expected_candidate_registry_fingerprint: &validator_registry_fingerprint,
                });
        let registry_snapshot = self
            .repositories
            .project(&project_id)?
            .latest_managed_registry_snapshot()?;
        let registry_records = match registry_snapshot.as_ref() {
            Some(snapshot) => self
                .repositories
                .project(&project_id)?
                .list_registry_consistency_records(&snapshot.managed_registry_snapshot_id)?,
            None => Vec::new(),
        };
        let rule_diagnostics = collect_m3_rule_diagnostics(
            &bundle,
            &execution_root,
            guard_input.as_ref(),
            registry_snapshot.as_ref(),
            &registry_records,
        )?;
        let decision_floor = rule_diagnostics
            .iter()
            .map(|diagnostic| diagnostic.decision_floor)
            .max()
            .unwrap_or(RuleDecisionFloorV2::None);
        let mut completion_claims = completion_claims
            .into_iter()
            .map(CompletionClaimV2::seal)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ApplicationError::Invalid)?;
        completion_claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
        if completion_claims
            .windows(2)
            .any(|pair| pair[0].claim_id == pair[1].claim_id)
            || completion_claims
                .iter()
                .any(|claim| claim.subject.project_id() != &project_id)
            || completion_claims.iter().any(|claim| {
                let CompletionClaimSubjectV2::CheckPlan {
                    plan_item_id,
                    descriptor_ref,
                    ..
                } = &claim.subject
                else {
                    return false;
                };
                !bundle.validation_plan.required_checks.iter().any(|check| {
                    &check.plan_item_id == plan_item_id && &check.descriptor_ref == descriptor_ref
                })
            })
        {
            return Err(ApplicationError::Invalid);
        }
        let completion_claim_refs = completion_claims
            .iter()
            .map(CompletionClaimV2::reference)
            .collect();
        let validator_guard_evidence_ref = validator_guard_evidence
            .as_ref()
            .map(ValidatorGuardEvidenceV2::reference)
            .transpose()
            .map_err(|_| ApplicationError::Invalid)?;
        Ok(PreparedValidationExecution {
            preflight: ValidationExecutionPreflight {
                task_spec_id: task_spec_id.clone(),
                validation_plan_ref: plan_ref,
                project_id,
                project_root_fingerprint,
                execution_root_kind,
                execution_root_binding_fingerprint,
                items,
                rule_diagnostics,
                decision_floor,
                completion_claim_refs,
                validator_guard_evidence_ref,
                readiness: bundle.validation_plan.readiness,
            },
            bindings,
            resolved_executables: resolved_by_fingerprint.into_values().collect(),
            change_sets: bundle.change_sets,
            completion_claims,
            validator_guard_evidence,
        })
    }

    pub fn execute_planning_bundle(
        &self,
        task_spec_id: &TaskSpecId,
        bindings: &[ExecutableBinding],
        context: CheckGraphRunContext,
        executor: &mut dyn CheckExecutor,
    ) -> Result<CheckGraphRunResult, ApplicationError> {
        let _guard = self.command_guard()?;
        self.execute_planning_bundle_inner(task_spec_id, bindings, context, executor, None)
    }

    fn execute_planning_bundle_with_artifact_finalizer(
        &self,
        task_spec_id: &TaskSpecId,
        bindings: &[ExecutableBinding],
        context: CheckGraphRunContext,
        executor: &mut dyn CheckExecutor,
        artifact_finalizer: &mut dyn ArtifactManifestFinalizer,
    ) -> Result<CheckGraphRunResult, ApplicationError> {
        self.execute_planning_bundle_inner(
            task_spec_id,
            bindings,
            context,
            executor,
            Some(artifact_finalizer),
        )
    }

    fn execute_planning_bundle_inner(
        &self,
        task_spec_id: &TaskSpecId,
        bindings: &[ExecutableBinding],
        context: CheckGraphRunContext,
        executor: &mut dyn CheckExecutor,
        artifact_finalizer: Option<&mut dyn ArtifactManifestFinalizer>,
    ) -> Result<CheckGraphRunResult, ApplicationError> {
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
        let result = if let Some(artifact_finalizer) = artifact_finalizer {
            run_check_graph_with_artifact_finalizer(
                &bundle.validation_plan,
                bindings,
                context,
                executor,
                artifact_finalizer,
            )?
        } else {
            run_check_graph(&bundle.validation_plan, bindings, context, executor)?
        };
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
            .save_check_graph_evidence(CheckGraphEvidenceTransaction {
                runs: &result.validation_runs,
                results: &result.validation_results,
                diagnostics: &result.diagnostics,
                decision: &result.gate_decision,
                bundle: &result.evidence_bundle,
                review_pack: &result.review_pack,
                rework_directive: result.rework_directive.as_ref(),
            })?;
        Ok(result)
    }

    pub fn validation_execution_status(
        &self,
        project_id: &ProjectId,
    ) -> Result<ValidationExecutionStatus, ApplicationError> {
        let repository = self.repositories.project(project_id)?;
        let runs = repository.list_validation_runs_v2()?;
        let results = repository.list_validation_results_v2()?;
        let diagnostics = repository.list_diagnostics_v2()?;
        let gates = repository.list_gate_decisions_v2()?;
        let bundles = repository.list_evidence_bundles_v2()?;
        let review_packs = repository.list_review_packs_v1()?;
        Ok(ValidationExecutionStatus {
            project_id: project_id.clone(),
            run_count: runs.len(),
            result_count: results.len(),
            diagnostic_count: diagnostics.len(),
            gate_count: gates.len(),
            evidence_bundle_count: bundles.len(),
            review_pack_count: review_packs.len(),
            latest_result: results.into_iter().max_by_key(|result| result.created_at),
            latest_gate: gates.into_iter().max_by_key(|gate| gate.decided_at),
            latest_evidence_bundle: bundles.into_iter().max_by_key(|bundle| bundle.created_at),
            latest_review_pack: review_packs.into_iter().max_by_key(|pack| pack.created_at),
        })
    }

    pub fn list_validation_runs_v2(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<ValidationRunV2>, ApplicationError> {
        Ok(self
            .repositories
            .project(project_id)?
            .list_validation_runs_v2()?)
    }

    pub fn list_validation_diagnostics_v2(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<DiagnosticV2>, ApplicationError> {
        Ok(self
            .repositories
            .project(project_id)?
            .list_diagnostics_v2()?)
    }

    pub fn get_validation_diagnostic_v2(
        &self,
        project_id: &ProjectId,
        diagnostic_id: &DiagnosticId,
    ) -> Result<DiagnosticV2, ApplicationError> {
        self.repositories
            .project(project_id)?
            .get_diagnostic_v2(diagnostic_id)?
            .ok_or(ApplicationError::NotFound)
    }

    pub fn get_gate_decision_v2(
        &self,
        project_id: &ProjectId,
        gate_id: &GateId,
    ) -> Result<GateDecisionV2, ApplicationError> {
        self.repositories
            .project(project_id)?
            .get_gate_decision_v2(gate_id)?
            .ok_or(ApplicationError::NotFound)
    }

    pub fn get_evidence_bundle_v2(
        &self,
        project_id: &ProjectId,
        evidence_bundle_id: &EvidenceBundleId,
    ) -> Result<EvidenceBundleV2, ApplicationError> {
        self.repositories
            .project(project_id)?
            .get_evidence_bundle_v2(evidence_bundle_id)?
            .ok_or(ApplicationError::NotFound)
    }

    pub fn get_review_pack_v1(
        &self,
        project_id: &ProjectId,
        review_pack_id: &ReviewPackId,
    ) -> Result<ReviewPackV1, ApplicationError> {
        self.repositories
            .project(project_id)?
            .get_review_pack_v1(review_pack_id)?
            .ok_or(ApplicationError::NotFound)
    }

    pub fn validation_decision_inspection(
        &self,
        project_id: &ProjectId,
    ) -> Result<ValidationDecisionInspection, ApplicationError> {
        let repository = self.repositories.project(project_id)?;
        Ok(ValidationDecisionInspection {
            project_id: project_id.clone(),
            baselines: repository.list_baselines_v2()?,
            suppressions: repository.list_suppressions_v2()?,
            dispositions: repository.list_dispositions_v2()?,
        })
    }

    pub fn put_validation_baseline_v2(
        &self,
        baseline: &BaselineV2,
    ) -> Result<(), ApplicationError> {
        let _guard = self.command_guard()?;
        self.repositories
            .project(&baseline.project_id)?
            .put_baseline_v2(baseline)?;
        Ok(())
    }

    pub fn put_validation_suppression_v2(
        &self,
        suppression: &SuppressionV2,
    ) -> Result<(), ApplicationError> {
        let _guard = self.command_guard()?;
        self.repositories
            .project(&suppression.project_id)?
            .put_suppression_v2(suppression)?;
        Ok(())
    }

    pub fn put_validation_disposition_v2(
        &self,
        disposition: &DispositionV2,
    ) -> Result<(), ApplicationError> {
        let _guard = self.command_guard()?;
        self.repositories
            .project(&disposition.project_id)?
            .put_disposition_v2(disposition)?;
        Ok(())
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
        let prepared_change_v2 = if candidate.patch_set.is_some() {
            Some(self.prepare_rust_style_change_v2(
                &project,
                &root,
                &scan,
                &candidate,
                ActorRef {
                    actor_type: ActorType::System,
                    actor_id: "star-rust-style-profile".to_owned(),
                    display_name: "Star Rust Style Profile".to_owned(),
                    auth_source: "controller".to_owned(),
                },
            )?)
        } else {
            None
        };
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
            candidate_build: prepared.candidate_build,
            candidate_test_compile: prepared.candidate_test_compile,
            prepared_change_v2,
            isolation_ref: prepared.isolation_ref,
        };
        Ok((result, candidate))
    }

    fn prepare_rust_style_change_v2(
        &self,
        project: &Project,
        root: &Path,
        scan: &ScanRun,
        candidate: &rust_style::RustStyleCandidate,
        requested_by: ActorRef,
    ) -> Result<PreparedChangeV2Result, ApplicationError> {
        let project_id = &project.project_id;
        let checkout_id = project
            .attached_checkout_ids
            .first()
            .ok_or(ApplicationError::Invalid)?;
        let repository = self.repositories.project(project_id)?;
        let snapshot = repository
            .get_workspace_snapshot(&scan.workspace_snapshot_id)?
            .ok_or(ApplicationError::NotFound)?;
        let recipe = rust_style_recipe_v2()?;
        let mut materialized = candidate
            .changes
            .iter()
            .map(|change| MaterializedPatchFile {
                path: change.path.clone(),
                before_sha256: change.before_sha256.clone(),
                after_sha256: change.after_sha256.clone(),
                before_bytes: change.before_bytes.clone(),
                after_bytes: change.after_bytes.clone(),
            })
            .collect::<Vec<_>>();
        materialized.sort_by(|left, right| left.path.cmp(&right.path));
        if materialized.is_empty()
            || materialized.iter().any(|file| {
                patch_source_bytes_are_sensitive(&file.before_bytes)
                    || patch_source_bytes_are_sensitive(&file.after_bytes)
            })
        {
            return Err(ApplicationError::Apply(
                "RUST_STYLE_PATCH_REDACTION_REQUIRED".to_owned(),
            ));
        }
        let paths = materialized
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        let expected_content_fingerprints = materialized
            .iter()
            .map(|file| (file.path.clone(), file.before_sha256.clone()))
            .collect::<BTreeMap<_, _>>();
        let target_selector = TargetSelector::Path {
            project_id: project_id.clone(),
            paths,
            expected_content_fingerprints,
        };
        target_selector
            .validate()
            .map_err(|_| ApplicationError::Invalid)?;
        let prepared = prepare_exact_materialized_patch(
            project_id,
            &snapshot,
            &recipe,
            &materialized,
            BTreeMap::new(),
        )?;
        let observed_changes = materialized
            .iter()
            .map(|file| ObservedWorkspaceChange {
                path: file.path.clone(),
                rename_from: None,
                change_kind: ObservedChangeKind::Modify,
                before_sha256: Some(file.before_sha256.clone()),
                after_sha256: Some(file.after_sha256.clone()),
                staged: false,
                unstaged: false,
                untracked: false,
                binary: false,
            })
            .collect::<Vec<_>>();
        let planning_bundle = self.create_planning_bundle_for_phase_inner(
            patch_preview_task(
                project_id,
                checkout_id,
                &recipe,
                &materialized,
                Some(&target_selector),
                PatchPreviewValidationPhase::PreApply,
            ),
            requested_by,
            rust_style_gate_check_descriptors(project_id)?,
            &format!(
                "rust-style-patch-preview-{}",
                prepared.patch_set.patch_set_id.as_str()
            ),
            "patch_pre_apply",
            Some((project_id.clone(), observed_changes)),
        )?;
        if planning_bundle.validation_plan.readiness != ValidationPlanV2Readiness::Ready {
            return Err(ApplicationError::Apply(
                "RUST_STYLE_AFFECTED_CHECKS_UNRESOLVED".to_owned(),
            ));
        }
        let preview_change_set = planning_bundle
            .change_sets
            .iter()
            .find(|change_set| change_set.project_id == *project_id)
            .ok_or(ApplicationError::Invalid)?;
        let preview_change_set_ref = DocumentRef {
            schema_id: star_contracts::planning::CHANGE_SET_SCHEMA_ID.to_owned(),
            document_id: preview_change_set.change_set_id.to_string(),
            revision: 1,
            sha256: preview_change_set.change_set_fingerprint.clone(),
        };
        let preview_impact_analysis_ref = DocumentRef {
            schema_id: star_contracts::planning::IMPACT_ANALYSIS_SCHEMA_ID.to_owned(),
            document_id: planning_bundle
                .impact_analysis
                .impact_analysis_id
                .to_string(),
            revision: planning_bundle.impact_analysis.revision,
            sha256: planning_bundle
                .impact_analysis
                .calculation_fingerprint
                .clone(),
        };
        let preview_validation_plan_ref = DocumentRef {
            schema_id: star_contracts::planning::FULL_VALIDATION_PLAN_SCHEMA_ID.to_owned(),
            document_id: planning_bundle
                .validation_plan
                .validation_plan_id
                .to_string(),
            revision: planning_bundle.validation_plan.revision,
            sha256: application_document_hash(&planning_bundle.validation_plan)?,
        };
        let now = Utc::now();
        let patch_set_id = prepared.patch_set.patch_set_id.clone();
        let worktree_decision = WorktreeDecision {
            schema_id: star_contracts::patch_v2::WORKTREE_DECISION_SCHEMA_ID.to_owned(),
            schema_version: 1,
            worktree_decision_id: WorktreeDecisionId::new(),
            revision: 1,
            project_id: project_id.clone(),
            checkout_id: checkout_id.clone(),
            base_workspace_snapshot_id: snapshot.workspace_snapshot_id.clone(),
            strategy: WorktreeStrategyV1::Current,
            reason_codes: vec!["RUST_STYLE_OWNED_PREVIEW_VERIFIED".to_owned()],
            isolated_locator_fingerprint: None,
            materialization_artifact_refs: vec![],
            state: WorktreeDecisionStateV1::Selected,
            created_at: now,
            updated_at: now,
            decision_fingerprint: Sha256Hash::digest(b""),
        }
        .seal()
        .map_err(|_| ApplicationError::Invalid)?;
        self.persist_patch_document(
            project_id,
            root,
            &format!(
                "management/patches-v2/{}/worktree-decision-r1.json",
                patch_set_id.as_str()
            ),
            "worktree_decision",
            worktree_decision.worktree_decision_id.as_str(),
            &worktree_decision,
        )?;
        let mut operation_artifacts = Vec::new();
        for (index, file) in materialized.iter().enumerate() {
            let forward = self.persist_patch_bytes(
                project_id,
                root,
                &patch_set_id,
                index,
                "forward",
                &file.path,
                &file.after_bytes,
                &file.after_sha256,
            )?;
            let reverse = self.persist_patch_bytes(
                project_id,
                root,
                &patch_set_id,
                index,
                "reverse",
                &file.path,
                &file.before_bytes,
                &file.before_sha256,
            )?;
            operation_artifacts.push((file.clone(), forward, reverse));
        }
        let replay_ref = self.artifacts.put_json_with_policy(ArtifactWriteRequest {
            project_id,
            project_root: root,
            relative_path: &format!(
                "management/patches-v2/{}/replay.json",
                patch_set_id.as_str()
            ),
            subject_kind: "recipe_replay",
            subject_id: patch_set_id.as_str(),
            policy: ArtifactWritePolicy {
                kind: ArtifactKind::Report,
                redaction_status: RedactionStatus::NotNeeded,
                retention_class: RetentionClass::Evidence,
            },
            value: &serde_json::json!({
                "schema_id":"star.recipe-replay-result",
                "schema_version":1,
                "patch_set_id":patch_set_id,
                "operation_count":0,
                "idempotence_proved":candidate.idempotence_proved,
                "candidate_fingerprint":candidate.candidate_fingerprint,
            }),
        })?;
        let recipe_execution = RecipeExecution {
            schema_id: star_contracts::patch_v2::RECIPE_EXECUTION_SCHEMA_ID.to_owned(),
            schema_version: 1,
            recipe_execution_id: RecipeExecutionId::new(),
            revision: 1,
            recipe_ref: recipe.reference(),
            project_id: project_id.clone(),
            checkout_id: checkout_id.clone(),
            base_workspace_snapshot_id: snapshot.workspace_snapshot_id.clone(),
            target_selector: target_selector.clone(),
            target_selector_fingerprint: Sha256Hash::digest(b""),
            parameters: serde_json::json!({}),
            parameter_fingerprint: Sha256Hash::digest(b""),
            worktree_decision_ref: worktree_decision
                .reference()
                .map_err(|_| ApplicationError::Invalid)?,
            first_preview_artifact_refs: operation_artifacts
                .iter()
                .flat_map(|(_, forward, reverse)| [forward.clone(), reverse.clone()])
                .collect(),
            replay_preview_artifact_refs: vec![replay_ref],
            preview_change_set_ref: Some(preview_change_set_ref.clone()),
            replan_bundle_ref: Some(DocumentRef {
                schema_id: "star.planning-bundle".to_owned(),
                document_id: planning_bundle.task_spec.task_spec_id.to_string(),
                revision: planning_bundle.task_spec.revision,
                sha256: planning_bundle.bundle_fingerprint.clone(),
            }),
            idempotence_proved: candidate.idempotence_proved,
            completeness: star_contracts::evidence::Completeness::Complete,
            limitations: vec![],
            state: RecipeExecutionStateV1::Previewed,
            started_at: now,
            finished_at: Some(Utc::now()),
            execution_fingerprint: Sha256Hash::digest(b""),
        }
        .seal()
        .map_err(|_| ApplicationError::Invalid)?;
        self.persist_patch_document(
            project_id,
            root,
            &format!(
                "management/patches-v2/{}/recipe-execution-r1.json",
                patch_set_id.as_str()
            ),
            "recipe_execution",
            recipe_execution.recipe_execution_id.as_str(),
            &recipe_execution,
        )?;
        let operations = operation_artifacts
            .into_iter()
            .enumerate()
            .map(|(index, (file, forward, reverse))| PatchOperation {
                operation_id: format!("op-{index:04}"),
                kind: PatchOperationKindV2::Modify,
                path: file.path,
                rename_from: None,
                before_sha256: Some(file.before_sha256),
                after_sha256: Some(file.after_sha256),
                before_mode: None,
                after_mode: None,
                forward_artifact_ref: forward,
                reverse_artifact_ref: reverse,
                operation_fingerprint: Sha256Hash::digest(b""),
            })
            .collect();
        let patch_set_v2 = PatchSetV2 {
            schema_id: star_contracts::patch_v2::PATCH_SET_V2_SCHEMA_ID.to_owned(),
            schema_version: 2,
            patch_set_id: patch_set_id.clone(),
            revision: 1,
            recipe_ref: recipe.reference(),
            recipe_execution_ref: recipe_execution
                .reference()
                .map_err(|_| ApplicationError::Invalid)?,
            project_id: project_id.clone(),
            checkout_id: checkout_id.clone(),
            change_plan_id: prepared.change_plan.change_plan_id.clone(),
            change_plan_revision: prepared.change_plan.revision,
            base_workspace_snapshot_id: snapshot.workspace_snapshot_id.clone(),
            target_selector_fingerprint: recipe_execution.target_selector_fingerprint.clone(),
            parameter_fingerprint: recipe_execution.parameter_fingerprint.clone(),
            operations,
            preview_change_set_ref,
            preview_impact_analysis_ref,
            preview_validation_plan_ref,
            expected_operation_set_fingerprint: Sha256Hash::digest(b""),
            completeness: star_contracts::evidence::Completeness::Complete,
            limitations: vec![],
            state: PatchSetStateV2::Ready,
            created_at: now,
            patch_fingerprint: Sha256Hash::digest(b""),
        }
        .seal()
        .map_err(|_| ApplicationError::Invalid)?;
        self.persist_patch_document(
            project_id,
            root,
            &format!(
                "management/patches-v2/{}/patch-set-r1.json",
                patch_set_id.as_str()
            ),
            "patch_set_v2",
            patch_set_id.as_str(),
            &patch_set_v2,
        )?;
        let compatibility_artifact = self.artifacts.put_json(
            project_id,
            root,
            &format!("management/patches/{}/recipe.json", patch_set_id.as_str()),
            "patch_set",
            patch_set_id.as_str(),
            &prepared.recipe_artifact,
        )?;
        let prepared = prepared.attach_artifact(compatibility_artifact)?;
        repository.save_change_plan(&prepared.change_plan)?;
        repository.save_patch_set(&prepared.patch_set)?;
        Ok(PreparedChangeV2Result {
            recipe,
            planning_bundle,
            worktree_decision,
            recipe_execution,
            patch_set: patch_set_v2,
            compatibility_patch_set: prepared.patch_set,
        })
    }

    pub fn auto_apply_rust_style<F>(
        &self,
        project_id: &ProjectId,
        scope: RustStyleScope,
        resolve_policy_approval: F,
    ) -> Result<RustStyleAutoApplyResult, ApplicationError>
    where
        F: FnOnce(
            &RustStylePolicyApprovalRequest,
        ) -> Result<RustStylePolicyApprovalDecision, ApplicationError>,
    {
        let _guard = self.command_guard()?;
        let (prepared, candidate) =
            self.prepare_rust_style_persisted(project_id, scope, RustAutoPolicy::PersonalAuto)?;
        if prepared.patch_set.is_none() {
            return Ok(RustStyleAutoApplyResult {
                prepared,
                applied: None,
                applied_v2: None,
                permit_automatic: true,
                policy_approval_request: None,
                policy_approval_decision: None,
            });
        }
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
        let gate_decision = prepared
            .pre_apply_gate_decision
            .as_ref()
            .ok_or(ApplicationError::Invalid)?;
        let pre_gate_value =
            serde_json::to_value(gate_decision).map_err(|_| ApplicationError::Invalid)?;
        let pre_gate_fingerprint =
            canonical_sha256(&pre_gate_value).map_err(|_| ApplicationError::Invalid)?;
        let prepared_change_v2 = prepared
            .prepared_change_v2
            .as_ref()
            .ok_or(ApplicationError::Invalid)?;
        let patch_set_v2 = &prepared_change_v2.patch_set;
        let policy_approval_request = rust_style::prepare_personal_auto_approval_request_for_patch(
            &candidate,
            &prepared.inspection.policy,
            &grant,
            &patch_set_v2.patch_set_id,
            &patch_set_v2.patch_fingerprint,
            pre_gate,
            gate_decision.gate_id.clone(),
            gate_decision.revision,
            pre_gate_fingerprint,
            Utc::now(),
        )
        .map_err(|error| ApplicationError::RustStyle(error.into()))?;
        let policy_approval_decision = resolve_policy_approval(&policy_approval_request)?;
        let mut permit =
            rust_style::authorize_personal_auto(rust_style::PersonalAutoAuthorization {
                candidate: &candidate,
                policy: &prepared.inspection.policy,
                grant: &grant,
                approved_patch_set_id: &patch_set_v2.patch_set_id,
                approved_patch_fingerprint: &patch_set_v2.patch_fingerprint,
                approval_request: &policy_approval_request,
                approval_decision: &policy_approval_decision,
                pre_gate,
                now: Utc::now(),
            })
            .map_err(|error| ApplicationError::RustStyle(error.into()))?;
        let mut port = ManagedRustSourceMutationPortV2 {
            service: self,
            patch_set_id: patch_set_v2.patch_set_id.clone(),
            approved_patch_fingerprint: patch_set_v2.patch_fingerprint.as_str().to_owned(),
            requested_by: ActorRef {
                actor_type: ActorType::System,
                actor_id: "rust-style-personal-auto-policy".to_owned(),
                display_name: "Rust Style Personal Auto Policy".to_owned(),
                auth_source: "durable-policy-approval".to_owned(),
            },
            result: None,
        };
        let state = rust_style::apply_with_permit(&candidate, &mut permit, &mut port)
            .map_err(|error| ApplicationError::RustStyle(error.into()))?;
        if state != rust_style::RustApplyState::Applied {
            let detail = match port.result.as_ref() {
                Some(Ok(result)) => result
                    .application
                    .reason_codes
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "PATCH_APPLICATION_NOT_APPLIED".to_owned()),
                Some(Err(_)) => "PATCH_APPLICATION_FAILED".to_owned(),
                None => "OUTCOME_MISSING".to_owned(),
            };
            return Err(ApplicationError::Apply(format!(
                "RUST_STYLE_AUTO_APPLY_{detail}"
            )));
        }
        let applied = port.result.take().ok_or_else(|| {
            ApplicationError::Apply("RUST_STYLE_APPLY_OUTCOME_UNKNOWN".to_owned())
        })??;
        Ok(RustStyleAutoApplyResult {
            prepared,
            applied: None,
            applied_v2: Some(applied),
            permit_automatic: permit.automatic,
            policy_approval_request: Some(policy_approval_request),
            policy_approval_decision: Some(policy_approval_decision),
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

    pub fn list_change_recipes(
        &self,
        language: Option<&str>,
        rewrite_kind: Option<star_contracts::patch_v2::RewriteAssuranceV2>,
    ) -> Result<RecipeCatalogResult, ApplicationError> {
        let _guard = self.command_guard()?;
        let mut items = vec![
            trailing_whitespace_recipe_v2()?,
            managed_declaration_recipe_v2()?,
            exact_reverse_recipe_v2()?,
            rust_style_recipe_v2()?,
        ];
        items.retain(|recipe| {
            language.is_none_or(|language| recipe.language.as_deref() == Some(language))
                && rewrite_kind.is_none_or(|kind| recipe.rewrite_assurance == kind)
        });
        items.sort_by(|left, right| {
            (&left.recipe_id, &left.recipe_version).cmp(&(&right.recipe_id, &right.recipe_version))
        });
        Ok(RecipeCatalogResult {
            confirmed_empty: items.is_empty(),
            items,
        })
    }

    pub fn describe_change_recipe(
        &self,
        recipe_spec: &str,
    ) -> Result<ChangeRecipeV2, ApplicationError> {
        let _guard = self.command_guard()?;
        [
            trailing_whitespace_recipe_v2()?,
            managed_declaration_recipe_v2()?,
            exact_reverse_recipe_v2()?,
            rust_style_recipe_v2()?,
        ]
        .into_iter()
        .find(|recipe| recipe_spec == format!("{}@{}", recipe.recipe_id, recipe.recipe_version))
        .ok_or(ApplicationError::NotFound)
    }

    pub fn validate_change_recipe(
        &self,
        recipe: ChangeRecipeV2,
    ) -> Result<ChangeRecipeV2, ApplicationError> {
        let _guard = self.command_guard()?;
        let sealed = recipe
            .clone()
            .seal()
            .map_err(|_| ApplicationError::Invalid)?;
        if sealed != recipe {
            return Err(ApplicationError::Invalid);
        }
        Ok(sealed)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_change_v2(
        &self,
        project_id: &ProjectId,
        checkout_id: &CheckoutId,
        recipe_spec: &str,
        target_selector: TargetSelector,
        parameters: serde_json::Value,
        worktree_strategy: WorktreeStrategyV1,
        requested_by: ActorRef,
    ) -> Result<PreparedChangeV2Result, ApplicationError> {
        let _guard = self.command_guard()?;
        self.prepare_change_v2_inner(
            project_id,
            checkout_id,
            recipe_spec,
            target_selector,
            parameters,
            worktree_strategy,
            requested_by,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_change_v2_inner(
        &self,
        project_id: &ProjectId,
        checkout_id: &CheckoutId,
        recipe_spec: &str,
        target_selector: TargetSelector,
        parameters: serde_json::Value,
        worktree_strategy: WorktreeStrategyV1,
        requested_by: ActorRef,
    ) -> Result<PreparedChangeV2Result, ApplicationError> {
        let trailing_recipe = trailing_whitespace_recipe_v2()?;
        let managed_recipe = managed_declaration_recipe_v2()?;
        let (recipe, managed_intent) = if recipe_spec
            == format!(
                "{}@{}",
                trailing_recipe.recipe_id, trailing_recipe.recipe_version
            ) {
            if parameters
                .as_object()
                .is_none_or(|object| !object.is_empty())
            {
                return Err(ApplicationError::Invalid);
            }
            (trailing_recipe, None)
        } else if recipe_spec
            == format!(
                "{}@{}",
                managed_recipe.recipe_id, managed_recipe.recipe_version
            )
        {
            let object = parameters
                .as_object()
                .filter(|object| object.len() == 1 && object.contains_key("intent"))
                .ok_or(ApplicationError::Invalid)?;
            let intent = serde_json::from_value::<ManagedDeclarationChangeIntent>(
                object
                    .get("intent")
                    .cloned()
                    .ok_or(ApplicationError::Invalid)?,
            )
            .map_err(|_| ApplicationError::Invalid)?;
            if intent.clone().seal().as_ref() != Ok(&intent) {
                return Err(ApplicationError::Invalid);
            }
            (managed_recipe, Some(intent))
        } else {
            return Err(ApplicationError::NotFound);
        };
        if target_selector.project_id() != project_id
            || target_selector.validate().is_err()
            || !recipe.selector_kinds.contains(&target_selector.kind())
            || requested_by.actor_id.trim().is_empty()
            || requested_by.auth_source.trim().is_empty()
        {
            return Err(ApplicationError::Invalid);
        }
        let project = self
            .repositories
            .global()
            .get_project(project_id)?
            .ok_or(ApplicationError::NotFound)?;
        let checkout = self
            .repositories
            .global()
            .get_project_checkout(checkout_id)?
            .ok_or(ApplicationError::NotFound)?;
        if checkout.project_id != *project_id {
            return Err(ApplicationError::Invalid);
        }
        let root = self.primary_project_root(&project)?;
        let repository = self.repositories.project(project_id)?;
        let (index_projection, index_current) =
            self.load_index_projection_with_freshness(project_id)?;
        if !index_current || index_projection.snapshot.checkout_id != *checkout_id {
            return Err(ApplicationError::IndexNotCurrent);
        }
        let latest_scan = repository
            .latest_scan()?
            .ok_or(ApplicationError::NotFound)?;
        if latest_scan.status != ScanStatus::Succeeded {
            return Err(ApplicationError::IndexNotCurrent);
        }
        let snapshot = repository
            .get_workspace_snapshot(&latest_scan.workspace_snapshot_id)?
            .ok_or(ApplicationError::NotFound)?;
        let (prepared, materialized, registry_preview) = if let Some(intent) = managed_intent {
            let manifest_path =
                ProjectPathRef::parse(".star-control/registry/manifest.toml".to_owned())
                    .map_err(|_| ApplicationError::Invalid)?;
            let registry =
                self.refresh_managed_registry_resolution_inner(project_id, &manifest_path)?;
            let rewriter = self.managed_registry_rewriter.as_ref().ok_or_else(|| {
                ApplicationError::Apply("MANAGED_REGISTRY_REWRITER_UNAVAILABLE".to_owned())
            })?;
            let rewrite = rewriter
                .rewrite(ManagedRegistryRewriteRequest {
                    project_root: root.clone(),
                    snapshot: registry.snapshot.clone(),
                    intent: intent.clone(),
                })
                .map_err(|error| ApplicationError::Apply(error.to_string()))?;
            if !rewrite.idempotence_proved || rewrite.replay_operation_count != 0 {
                return Err(ApplicationError::Apply(
                    "PATCH_RECIPE_REPLAY_NOT_IDEMPOTENT".to_owned(),
                ));
            }
            let mut materialized = rewrite
                .files
                .into_iter()
                .map(|file| MaterializedPatchFile {
                    path: file.path,
                    before_sha256: file.before_sha256,
                    after_sha256: file.after_sha256,
                    before_bytes: file.before_bytes,
                    after_bytes: file.after_bytes,
                })
                .collect::<Vec<_>>();
            materialized.sort_by(|left, right| left.path.cmp(&right.path));
            match &target_selector {
                TargetSelector::ManagedDeclaration {
                    declaration_ids,
                    expected_declaration_fingerprints,
                    ..
                } => {
                    let reference = intent
                        .declaration_ref
                        .as_ref()
                        .ok_or(ApplicationError::Invalid)?;
                    if declaration_ids.len() != 1
                        || declaration_ids[0] != reference.managed_declaration_id.as_str()
                        || expected_declaration_fingerprints
                            .get(reference.managed_declaration_id.as_str())
                            != Some(&reference.definition_fingerprint)
                    {
                        return Err(ApplicationError::Invalid);
                    }
                }
                TargetSelector::Path {
                    paths,
                    expected_content_fingerprints,
                    ..
                } => {
                    if paths
                        != &materialized
                            .iter()
                            .map(|file| file.path.clone())
                            .collect::<Vec<_>>()
                        || materialized.iter().any(|file| {
                            expected_content_fingerprints.get(&file.path)
                                != Some(&file.before_sha256)
                        })
                    {
                        return Err(ApplicationError::Invalid);
                    }
                }
                _ => return Err(ApplicationError::Invalid),
            }
            let prepared = prepare_exact_materialized_patch(
                project_id,
                &snapshot,
                &recipe,
                &materialized,
                BTreeMap::from([(
                    "managed_registry_intent_fingerprint".to_owned(),
                    intent.intent_fingerprint.to_string(),
                )]),
            )?;
            (prepared, materialized, Some((registry.snapshot, intent)))
        } else {
            let prepared = match &target_selector {
                TargetSelector::Finding {
                    finding_ids,
                    expected_finding_fingerprints,
                    ..
                } if finding_ids.len() == 1 => {
                    let finding_id = FindingId::parse(finding_ids[0].clone())
                        .map_err(|_| ApplicationError::Invalid)?;
                    let finding = repository
                        .get_finding(&finding_id)?
                        .ok_or(ApplicationError::NotFound)?;
                    if expected_finding_fingerprints.get(&finding_ids[0])
                        != Some(&finding.finding_fingerprint)
                    {
                        return Err(ApplicationError::Invalid);
                    }
                    let occurrences = repository.occurrences_for_finding(&finding_id)?;
                    prepare_trailing_whitespace_patch(&root, &finding, &occurrences, &snapshot)?
                }
                TargetSelector::Path {
                    paths,
                    expected_content_fingerprints,
                    ..
                } if paths.len() == expected_content_fingerprints.len() => {
                    prepare_trailing_whitespace_paths(
                        &root,
                        project_id,
                        expected_content_fingerprints,
                        &snapshot,
                    )?
                }
                _ => return Err(ApplicationError::Invalid),
            };
            let transform = PreparedPatchTransformerAdapter::new(&prepared)
                .materialize(
                    &root,
                    &RewriteTransformRequest {
                        recipe: recipe.clone(),
                        target_selector: target_selector.clone(),
                        parameters: parameters.clone(),
                    },
                )
                .map_err(|_| ApplicationError::Apply("PATCH_TRANSFORMER_REJECTED".to_owned()))?;
            if !transform.idempotence_proved || transform.replay_operation_count != 0 {
                return Err(ApplicationError::Apply(
                    "PATCH_RECIPE_REPLAY_NOT_IDEMPOTENT".to_owned(),
                ));
            }
            let materialized = transform
                .files
                .into_iter()
                .map(|file| MaterializedPatchFile {
                    path: file.path,
                    before_sha256: file.before_sha256,
                    after_sha256: file.after_sha256,
                    before_bytes: file.before_bytes,
                    after_bytes: file.after_bytes,
                })
                .collect::<Vec<_>>();
            (prepared, materialized, None)
        };
        if materialized.is_empty()
            || materialized.iter().any(|file| {
                patch_source_bytes_are_sensitive(&file.before_bytes)
                    || patch_source_bytes_are_sensitive(&file.after_bytes)
            })
        {
            return Err(ApplicationError::Apply(
                "PATCH_PREVIEW_REDACTION_REQUIRED".to_owned(),
            ));
        }
        let observed_changes = materialized
            .iter()
            .map(|file| ObservedWorkspaceChange {
                path: file.path.clone(),
                rename_from: None,
                change_kind: ObservedChangeKind::Modify,
                before_sha256: Some(file.before_sha256.clone()),
                after_sha256: Some(file.after_sha256.clone()),
                staged: false,
                unstaged: false,
                untracked: false,
                binary: false,
            })
            .collect::<Vec<_>>();
        let task = patch_preview_task(
            project_id,
            checkout_id,
            &recipe,
            &materialized,
            Some(&target_selector),
            PatchPreviewValidationPhase::PreApply,
        );
        let planning_key = format!("patch-preview-{}", prepared.patch_set.patch_set_id.as_str());
        let planning_bundle = self.create_planning_bundle_for_phase_inner(
            task,
            requested_by.clone(),
            vec![],
            &planning_key,
            "patch_pre_apply",
            Some((project_id.clone(), observed_changes)),
        )?;
        let planning_ready =
            planning_bundle.validation_plan.readiness == ValidationPlanV2Readiness::Ready;
        let replan_limitations = if planning_ready {
            vec![]
        } else {
            let mut limitations = planning_bundle
                .validation_plan
                .unresolved_checks
                .iter()
                .map(|check| format!("UNRESOLVED_CHECK:{}", check.family))
                .collect::<Vec<_>>();
            limitations.push("PATCH_REPLAN_REQUIRED".to_owned());
            limitations.sort();
            limitations.dedup();
            limitations
        };
        let preview_change_set = planning_bundle
            .change_sets
            .iter()
            .find(|change_set| change_set.project_id == *project_id)
            .ok_or(ApplicationError::Invalid)?;
        let preview_change_set_ref = DocumentRef {
            schema_id: star_contracts::planning::CHANGE_SET_SCHEMA_ID.to_owned(),
            document_id: preview_change_set.change_set_id.to_string(),
            revision: 1,
            sha256: preview_change_set.change_set_fingerprint.clone(),
        };
        let preview_impact_analysis_ref = DocumentRef {
            schema_id: star_contracts::planning::IMPACT_ANALYSIS_SCHEMA_ID.to_owned(),
            document_id: planning_bundle
                .impact_analysis
                .impact_analysis_id
                .to_string(),
            revision: planning_bundle.impact_analysis.revision,
            sha256: planning_bundle
                .impact_analysis
                .calculation_fingerprint
                .clone(),
        };
        let preview_validation_plan_ref = DocumentRef {
            schema_id: star_contracts::planning::FULL_VALIDATION_PLAN_SCHEMA_ID.to_owned(),
            document_id: planning_bundle
                .validation_plan
                .validation_plan_id
                .to_string(),
            revision: planning_bundle.validation_plan.revision,
            sha256: application_document_hash(&planning_bundle.validation_plan)?,
        };
        let now = Utc::now();
        let patch_set_id = prepared.patch_set.patch_set_id.clone();
        let selected_worktree = WorktreeDecision {
            schema_id: star_contracts::patch_v2::WORKTREE_DECISION_SCHEMA_ID.to_owned(),
            schema_version: 1,
            worktree_decision_id: WorktreeDecisionId::new(),
            revision: 1,
            project_id: project_id.clone(),
            checkout_id: checkout_id.clone(),
            base_workspace_snapshot_id: snapshot.workspace_snapshot_id.clone(),
            strategy: worktree_strategy,
            reason_codes: vec![match worktree_strategy {
                WorktreeStrategyV1::Current => {
                    "BUILTIN_READ_ONLY_PREVIEW_CURRENT_WORKTREE".to_owned()
                }
                WorktreeStrategyV1::Isolated => "ISOLATED_GIT_WORKTREE_SELECTED".to_owned(),
            }],
            isolated_locator_fingerprint: None,
            materialization_artifact_refs: vec![],
            state: WorktreeDecisionStateV1::Selected,
            created_at: now,
            updated_at: now,
            decision_fingerprint: Sha256Hash::digest(b""),
        }
        .seal()
        .map_err(|_| ApplicationError::Invalid)?;
        let worktree_decision = if worktree_strategy == WorktreeStrategyV1::Isolated {
            let adapter = GitWorktreeAdapter::new(
                std::env::temp_dir()
                    .join("Star-Control")
                    .join("isolated-worktrees"),
            )
            .map_err(|_| ApplicationError::Apply("PATCH_WORKTREE_UNAVAILABLE".to_owned()))?;
            let materialization = adapter
                .materialize(&root, &selected_worktree)
                .map_err(|_| ApplicationError::Apply("PATCH_WORKTREE_UNAVAILABLE".to_owned()))?;
            let preview_inputs = materialized
                .iter()
                .map(|file| MaterializedRewrite {
                    path: file.path.clone(),
                    before_sha256: file.before_sha256.clone(),
                    after_sha256: file.after_sha256.clone(),
                    before_bytes: file.before_bytes.clone(),
                    after_bytes: file.after_bytes.clone(),
                })
                .collect::<Vec<_>>();
            adapter
                .synchronize_preview_inputs(&materialization, &preview_inputs)
                .map_err(|_| {
                    ApplicationError::Apply("PATCH_WORKTREE_SYNCHRONIZE_FAILED".to_owned())
                })?;
            let (isolated_files, isolated_replay_count, isolated_idempotence) =
                if let Some((registry_snapshot, intent)) = registry_preview.as_ref() {
                    let rewriter = self.managed_registry_rewriter.as_ref().ok_or_else(|| {
                        ApplicationError::Apply("MANAGED_REGISTRY_REWRITER_UNAVAILABLE".to_owned())
                    })?;
                    let preview = rewriter
                        .rewrite(ManagedRegistryRewriteRequest {
                            project_root: materialization.root.clone(),
                            snapshot: registry_snapshot.clone(),
                            intent: intent.clone(),
                        })
                        .map_err(|_| {
                            ApplicationError::Apply("PATCH_ISOLATED_PREVIEW_REJECTED".to_owned())
                        })?;
                    (
                        preview.files,
                        preview.replay_operation_count,
                        preview.idempotence_proved,
                    )
                } else {
                    let preview = PreparedPatchTransformerAdapter::new(&prepared)
                        .materialize(
                            &materialization.root,
                            &RewriteTransformRequest {
                                recipe: recipe.clone(),
                                target_selector: target_selector.clone(),
                                parameters: parameters.clone(),
                            },
                        )
                        .map_err(|_| {
                            ApplicationError::Apply("PATCH_ISOLATED_PREVIEW_REJECTED".to_owned())
                        })?;
                    (
                        preview.files,
                        u64::try_from(preview.replay_operation_count)
                            .map_err(|_| ApplicationError::Invalid)?,
                        preview.idempotence_proved,
                    )
                };
            if !isolated_idempotence
                || isolated_replay_count != 0
                || isolated_files != preview_inputs
            {
                return Err(ApplicationError::Apply(
                    "PATCH_ISOLATED_PREVIEW_MISMATCH".to_owned(),
                ));
            }
            let materialization_ref =
                self.artifacts.put_json_with_policy(ArtifactWriteRequest {
                    project_id,
                    project_root: &root,
                    relative_path: &format!(
                        "management/patches-v2/{}/worktree-materialization.json",
                        patch_set_id.as_str()
                    ),
                    subject_kind: "git_worktree_materialization",
                    subject_id: selected_worktree.worktree_decision_id.as_str(),
                    policy: ArtifactWritePolicy {
                        kind: ArtifactKind::Checkpoint,
                        redaction_status: RedactionStatus::NotNeeded,
                        retention_class: RetentionClass::Hold,
                    },
                    value: &serde_json::json!({
                        "schema_id":"star.git-worktree-materialization",
                        "schema_version":1,
                        "worktree_decision_id":selected_worktree.worktree_decision_id,
                        "base_kind":"system_temp_star_control",
                        "worktree_name":selected_worktree.worktree_decision_id.as_str(),
                        "locator_fingerprint":materialization.locator_fingerprint,
                        "state":"retained_for_recovery",
                    }),
                })?;
            let mut materialized_worktree = selected_worktree;
            materialized_worktree.reason_codes =
                vec!["ISOLATED_GIT_WORKTREE_PREVIEW_RETAINED".to_owned()];
            materialized_worktree.isolated_locator_fingerprint =
                Some(materialization.locator_fingerprint);
            materialized_worktree.materialization_artifact_refs = vec![materialization_ref];
            materialized_worktree.state = WorktreeDecisionStateV1::RetainedForRecovery;
            materialized_worktree.updated_at = Utc::now();
            materialized_worktree
                .seal()
                .map_err(|_| ApplicationError::Invalid)?
        } else {
            selected_worktree
        };
        self.persist_patch_document(
            project_id,
            &root,
            &format!(
                "management/patches-v2/{}/worktree-decision-r1.json",
                patch_set_id.as_str()
            ),
            "worktree_decision",
            worktree_decision.worktree_decision_id.as_str(),
            &worktree_decision,
        )?;
        let mut operation_artifacts = Vec::new();
        for (index, file) in materialized.iter().enumerate() {
            let forward = self.persist_patch_bytes(
                project_id,
                &root,
                &patch_set_id,
                index,
                "forward",
                &file.path,
                &file.after_bytes,
                &file.after_sha256,
            )?;
            let reverse = self.persist_patch_bytes(
                project_id,
                &root,
                &patch_set_id,
                index,
                "reverse",
                &file.path,
                &file.before_bytes,
                &file.before_sha256,
            )?;
            operation_artifacts.push((file.clone(), forward, reverse));
        }
        let replay_ref = self.artifacts.put_json_with_policy(ArtifactWriteRequest {
            project_id,
            project_root: &root,
            relative_path: &format!(
                "management/patches-v2/{}/replay.json",
                patch_set_id.as_str()
            ),
            subject_kind: "recipe_replay",
            subject_id: patch_set_id.as_str(),
            policy: ArtifactWritePolicy {
                kind: ArtifactKind::Report,
                redaction_status: RedactionStatus::NotNeeded,
                retention_class: RetentionClass::Evidence,
            },
            value: &serde_json::json!({
                "schema_id":"star.recipe-replay-result",
                "schema_version":1,
                "patch_set_id":patch_set_id,
                "operation_count":0,
                "idempotence_proved":true,
            }),
        })?;
        let recipe_execution = RecipeExecution {
            schema_id: star_contracts::patch_v2::RECIPE_EXECUTION_SCHEMA_ID.to_owned(),
            schema_version: 1,
            recipe_execution_id: RecipeExecutionId::new(),
            revision: 1,
            recipe_ref: recipe.reference(),
            project_id: project_id.clone(),
            checkout_id: checkout_id.clone(),
            base_workspace_snapshot_id: snapshot.workspace_snapshot_id.clone(),
            target_selector: target_selector.clone(),
            target_selector_fingerprint: Sha256Hash::digest(b""),
            parameters: parameters.clone(),
            parameter_fingerprint: Sha256Hash::digest(b""),
            worktree_decision_ref: worktree_decision
                .reference()
                .map_err(|_| ApplicationError::Invalid)?,
            first_preview_artifact_refs: operation_artifacts
                .iter()
                .flat_map(|(_, forward, reverse)| [forward.clone(), reverse.clone()])
                .collect(),
            replay_preview_artifact_refs: vec![replay_ref],
            preview_change_set_ref: Some(preview_change_set_ref.clone()),
            replan_bundle_ref: Some(DocumentRef {
                schema_id: "star.planning-bundle".to_owned(),
                document_id: planning_bundle.task_spec.task_spec_id.to_string(),
                revision: planning_bundle.task_spec.revision,
                sha256: planning_bundle.bundle_fingerprint.clone(),
            }),
            idempotence_proved: true,
            completeness: if planning_ready {
                star_contracts::evidence::Completeness::Complete
            } else {
                star_contracts::evidence::Completeness::Partial
            },
            limitations: replan_limitations.clone(),
            state: if planning_ready {
                RecipeExecutionStateV1::Previewed
            } else {
                RecipeExecutionStateV1::ReplanRequired
            },
            started_at: now,
            finished_at: Some(Utc::now()),
            execution_fingerprint: Sha256Hash::digest(b""),
        }
        .seal()
        .map_err(|_| ApplicationError::Invalid)?;
        self.persist_patch_document(
            project_id,
            &root,
            &format!(
                "management/patches-v2/{}/recipe-execution-r1.json",
                patch_set_id.as_str()
            ),
            "recipe_execution",
            recipe_execution.recipe_execution_id.as_str(),
            &recipe_execution,
        )?;
        let operations = operation_artifacts
            .into_iter()
            .enumerate()
            .map(|(index, (file, forward, reverse))| PatchOperation {
                operation_id: format!("op-{index:04}"),
                kind: PatchOperationKindV2::Modify,
                path: file.path,
                rename_from: None,
                before_sha256: Some(file.before_sha256),
                after_sha256: Some(file.after_sha256),
                before_mode: None,
                after_mode: None,
                forward_artifact_ref: forward,
                reverse_artifact_ref: reverse,
                operation_fingerprint: Sha256Hash::digest(b""),
            })
            .collect();
        let patch_set_v2 = PatchSetV2 {
            schema_id: star_contracts::patch_v2::PATCH_SET_V2_SCHEMA_ID.to_owned(),
            schema_version: 2,
            patch_set_id: patch_set_id.clone(),
            revision: 1,
            recipe_ref: recipe.reference(),
            recipe_execution_ref: recipe_execution
                .reference()
                .map_err(|_| ApplicationError::Invalid)?,
            project_id: project_id.clone(),
            checkout_id: checkout_id.clone(),
            change_plan_id: prepared.change_plan.change_plan_id.clone(),
            change_plan_revision: prepared.change_plan.revision,
            base_workspace_snapshot_id: snapshot.workspace_snapshot_id.clone(),
            target_selector_fingerprint: recipe_execution.target_selector_fingerprint.clone(),
            parameter_fingerprint: recipe_execution.parameter_fingerprint.clone(),
            operations,
            preview_change_set_ref,
            preview_impact_analysis_ref,
            preview_validation_plan_ref,
            expected_operation_set_fingerprint: Sha256Hash::digest(b""),
            completeness: if planning_ready {
                star_contracts::evidence::Completeness::Complete
            } else {
                star_contracts::evidence::Completeness::Partial
            },
            limitations: replan_limitations,
            state: if planning_ready {
                PatchSetStateV2::Ready
            } else {
                PatchSetStateV2::ReplanRequired
            },
            created_at: now,
            patch_fingerprint: Sha256Hash::digest(b""),
        }
        .seal()
        .map_err(|_| ApplicationError::Invalid)?;
        self.persist_patch_document(
            project_id,
            &root,
            &format!(
                "management/patches-v2/{}/patch-set-r1.json",
                patch_set_id.as_str()
            ),
            "patch_set_v2",
            patch_set_id.as_str(),
            &patch_set_v2,
        )?;
        let compatibility_artifact = self.artifacts.put_json(
            project_id,
            &root,
            &format!("management/patches/{}/recipe.json", patch_set_id.as_str()),
            "patch_set",
            patch_set_id.as_str(),
            &prepared.recipe_artifact,
        )?;
        let prepared = prepared.attach_artifact(compatibility_artifact)?;
        repository.save_change_plan(&prepared.change_plan)?;
        repository.save_patch_set(&prepared.patch_set)?;
        Ok(PreparedChangeV2Result {
            recipe,
            planning_bundle,
            worktree_decision,
            recipe_execution,
            patch_set: patch_set_v2,
            compatibility_patch_set: prepared.patch_set,
        })
    }

    fn persist_patch_document<T: Serialize>(
        &self,
        project_id: &ProjectId,
        project_root: &Path,
        relative_path: &str,
        subject_kind: &str,
        subject_id: &str,
        value: &T,
    ) -> Result<ArtifactRef, ApplicationError> {
        let value = serde_json::to_value(value).map_err(|_| ApplicationError::Invalid)?;
        Ok(self.artifacts.put_json_with_policy(ArtifactWriteRequest {
            project_id,
            project_root,
            relative_path,
            subject_kind,
            subject_id,
            policy: ArtifactWritePolicy {
                kind: ArtifactKind::Manifest,
                redaction_status: RedactionStatus::NotNeeded,
                retention_class: RetentionClass::Evidence,
            },
            value: &value,
        })?)
    }

    #[allow(clippy::too_many_arguments)]
    fn persist_patch_bytes(
        &self,
        project_id: &ProjectId,
        project_root: &Path,
        patch_set_id: &PatchSetId,
        index: usize,
        direction: &str,
        path: &ProjectPathRef,
        bytes: &[u8],
        content_sha256: &Sha256Hash,
    ) -> Result<ArtifactRef, ApplicationError> {
        if patch_source_bytes_are_sensitive(bytes) || Sha256Hash::digest(bytes) != *content_sha256 {
            return Err(ApplicationError::Apply(
                "PATCH_PREVIEW_REDACTION_REQUIRED".to_owned(),
            ));
        }
        Ok(self.artifacts.put_json_with_policy(ArtifactWriteRequest {
            project_id,
            project_root,
            relative_path: &format!(
                "management/patches-v2/{}/operations/{index:04}-{direction}.json",
                patch_set_id.as_str()
            ),
            subject_kind: "patch_operation_bytes",
            subject_id: patch_set_id.as_str(),
            policy: ArtifactWritePolicy {
                kind: ArtifactKind::ChangeSet,
                redaction_status: RedactionStatus::NotNeeded,
                retention_class: RetentionClass::Hold,
            },
            value: &serde_json::json!({
                "schema_id":"star.patch-operation-bytes",
                "schema_version":1,
                "patch_set_id":patch_set_id,
                "direction":direction,
                "path":path,
                "encoding":"hex",
                "content_sha256":content_sha256,
                "bytes":hex_encode(bytes),
            }),
        })?)
    }

    fn read_patch_document<T: serde::de::DeserializeOwned>(
        &self,
        project_id: &ProjectId,
        project_root: &Path,
        relative_path: &str,
    ) -> Result<T, ApplicationError> {
        let expected = format!(
            ".ai-runs/star-control/{}",
            relative_path.trim_start_matches('/')
        );
        let discovery = self.artifacts.discover_verified(project_id, project_root)?;
        let artifact = discovery
            .verified
            .iter()
            .find(|artifact| artifact.relative_path == expected)
            .ok_or_else(|| {
                if discovery.rejected_count > 0 {
                    ApplicationError::Repository(RepositoryError::new(
                        RepositoryErrorCategory::IntegrityFailed,
                        "patch lifecycle artifact discovery rejected one or more artifacts",
                    ))
                } else {
                    ApplicationError::NotFound
                }
            })?;
        let value = self.artifacts.read_json(project_root, artifact)?;
        serde_json::from_value(value).map_err(|_| ApplicationError::Invalid)
    }

    fn load_latest_worktree_decision(
        &self,
        project_id: &ProjectId,
        project_root: &Path,
        patch_set_id: &PatchSetId,
        bound_ref: &DocumentRef,
    ) -> Result<WorktreeDecision, ApplicationError> {
        let discovery = self.artifacts.discover_verified(project_id, project_root)?;
        let marker = format!(
            "/management/patches-v2/{}/worktree-decision-r",
            patch_set_id.as_str()
        );
        let mut candidates = Vec::new();
        let mut bound_found = false;
        for artifact in discovery
            .verified
            .iter()
            .filter(|artifact| artifact.relative_path.contains(&marker))
        {
            let value = self.artifacts.read_json(project_root, artifact)?;
            let decision = serde_json::from_value::<WorktreeDecision>(value)
                .map_err(|_| ApplicationError::Invalid)?;
            let sealed = decision
                .clone()
                .seal()
                .map_err(|_| ApplicationError::Invalid)?;
            let reference = decision
                .reference()
                .map_err(|_| ApplicationError::Invalid)?;
            if sealed != decision
                || reference.document_id != bound_ref.document_id
                || decision.revision < bound_ref.revision
            {
                return Err(ApplicationError::Invalid);
            }
            if reference == *bound_ref {
                bound_found = true;
            }
            candidates.push(decision);
        }
        candidates.sort_by_key(|decision| decision.revision);
        if !bound_found
            || candidates.is_empty()
            || candidates
                .windows(2)
                .any(|pair| pair[0].revision == pair[1].revision)
        {
            return Err(if discovery.rejected_count > 0 {
                ApplicationError::Repository(RepositoryError::new(
                    RepositoryErrorCategory::IntegrityFailed,
                    "worktree decision discovery rejected an artifact",
                ))
            } else {
                ApplicationError::Invalid
            });
        }
        candidates.pop().ok_or(ApplicationError::NotFound)
    }

    fn load_patch_v2(&self, patch_set_id: &PatchSetId) -> Result<LoadedPatchV2, ApplicationError> {
        for project in self.repositories.global().list_projects()? {
            let root = self.primary_project_root(&project)?;
            let patch_set: PatchSetV2 = match self.read_patch_document(
                &project.project_id,
                &root,
                &format!(
                    "management/patches-v2/{}/patch-set-r1.json",
                    patch_set_id.as_str()
                ),
            ) {
                Ok(patch_set) => patch_set,
                Err(ApplicationError::NotFound) => continue,
                Err(error) => return Err(error),
            };
            let sealed_patch = patch_set
                .clone()
                .seal()
                .map_err(|_| ApplicationError::Invalid)?;
            if sealed_patch != patch_set || patch_set.project_id != project.project_id {
                return Err(ApplicationError::Invalid);
            }
            let recipe_execution: RecipeExecution = self.read_patch_document(
                &project.project_id,
                &root,
                &format!(
                    "management/patches-v2/{}/recipe-execution-r1.json",
                    patch_set_id.as_str()
                ),
            )?;
            let sealed_execution = recipe_execution
                .clone()
                .seal()
                .map_err(|_| ApplicationError::Invalid)?;
            let worktree_decision = self.load_latest_worktree_decision(
                &project.project_id,
                &root,
                patch_set_id,
                &recipe_execution.worktree_decision_ref,
            )?;
            let sealed_worktree = worktree_decision
                .clone()
                .seal()
                .map_err(|_| ApplicationError::Invalid)?;
            if sealed_execution != recipe_execution
                || sealed_worktree != worktree_decision
                || patch_set.recipe_execution_ref
                    != recipe_execution
                        .reference()
                        .map_err(|_| ApplicationError::Invalid)?
            {
                return Err(ApplicationError::Invalid);
            }
            return Ok(LoadedPatchV2 {
                project_root: root,
                patch_set,
                recipe_execution,
                worktree_decision,
            });
        }
        Err(ApplicationError::NotFound)
    }

    pub fn show_patch_v2(
        &self,
        patch_set_id: &PatchSetId,
    ) -> Result<PatchShowV2Result, ApplicationError> {
        let _guard = self.command_guard()?;
        let loaded = self.load_patch_v2(patch_set_id)?;
        let mut forward_artifact_refs = loaded
            .patch_set
            .operations
            .iter()
            .map(|operation| operation.forward_artifact_ref.clone())
            .collect::<Vec<_>>();
        let mut reverse_artifact_refs = loaded
            .patch_set
            .operations
            .iter()
            .map(|operation| operation.reverse_artifact_ref.clone())
            .collect::<Vec<_>>();
        forward_artifact_refs.sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
        reverse_artifact_refs.sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
        Ok(PatchShowV2Result {
            patch_set: loaded.patch_set,
            recipe_execution: loaded.recipe_execution,
            worktree_decision: loaded.worktree_decision,
            forward_artifact_refs,
            reverse_artifact_refs,
        })
    }

    fn materialize_loaded_patch_v2(
        &self,
        loaded: &LoadedPatchV2,
    ) -> Result<Vec<MaterializedPatchFile>, ApplicationError> {
        loaded
            .patch_set
            .operations
            .iter()
            .map(|operation| {
                if operation.kind != PatchOperationKindV2::Modify {
                    return Err(ApplicationError::Invalid);
                }
                let before_sha256 = operation
                    .before_sha256
                    .clone()
                    .ok_or(ApplicationError::Invalid)?;
                let after_sha256 = operation
                    .after_sha256
                    .clone()
                    .ok_or(ApplicationError::Invalid)?;
                let before_bytes = self.read_patch_bytes(
                    &loaded.project_root,
                    &operation.reverse_artifact_ref,
                    "reverse",
                    &operation.path,
                )?;
                let after_bytes = self.read_patch_bytes(
                    &loaded.project_root,
                    &operation.forward_artifact_ref,
                    "forward",
                    &operation.path,
                )?;
                if Sha256Hash::digest(&before_bytes) != before_sha256
                    || Sha256Hash::digest(&after_bytes) != after_sha256
                {
                    return Err(ApplicationError::Invalid);
                }
                Ok(MaterializedPatchFile {
                    path: operation.path.clone(),
                    before_sha256,
                    after_sha256,
                    before_bytes,
                    after_bytes,
                })
            })
            .collect()
    }

    fn persist_patch_operation_receipts(
        &self,
        loaded: &LoadedPatchV2,
        state_when_after: PatchOperationReceiptStateV1,
        state_when_before: PatchOperationReceiptStateV1,
        failure_reason: Option<&str>,
    ) -> Result<Vec<PatchOperationReceiptV1>, ApplicationError> {
        let mut receipts = Vec::with_capacity(loaded.patch_set.operations.len());
        for operation in &loaded.patch_set.operations {
            let observed = observe_project_path_sha256(&loaded.project_root, &operation.path)?;
            let state = if observed.as_ref() == operation.after_sha256.as_ref() {
                state_when_after
            } else if observed.as_ref() == operation.before_sha256.as_ref() {
                state_when_before
            } else {
                PatchOperationReceiptStateV1::OutcomeUnknown
            };
            let reason_code = match state {
                PatchOperationReceiptStateV1::FailedBeforeEffect
                | PatchOperationReceiptStateV1::FailedAfterEffect
                | PatchOperationReceiptStateV1::OutcomeUnknown
                | PatchOperationReceiptStateV1::RecoveryBlocked => Some(
                    failure_reason
                        .unwrap_or("PATCH_OPERATION_OUTCOME_UNKNOWN")
                        .to_owned(),
                ),
                PatchOperationReceiptStateV1::NotStarted
                | PatchOperationReceiptStateV1::AppliedExact
                | PatchOperationReceiptStateV1::RevertedExact => None,
            };
            let effect_receipt_ref = if matches!(
                state,
                PatchOperationReceiptStateV1::NotStarted
                    | PatchOperationReceiptStateV1::FailedBeforeEffect
            ) {
                None
            } else {
                Some(self.artifacts.put_json_with_policy(ArtifactWriteRequest {
                    project_id: &loaded.patch_set.project_id,
                    project_root: &loaded.project_root,
                    relative_path: &format!(
                        "management/patches-v2/{}/receipts/{}-{}.json",
                        loaded.patch_set.patch_set_id.as_str(),
                        operation.operation_id,
                        RequestId::new().as_str()
                    ),
                    subject_kind: "patch_operation_effect_receipt",
                    subject_id: &operation.operation_id,
                    policy: ArtifactWritePolicy {
                        kind: ArtifactKind::Checkpoint,
                        redaction_status: RedactionStatus::NotNeeded,
                        retention_class: RetentionClass::Evidence,
                    },
                    value: &serde_json::json!({
                        "schema_id":"star.patch-operation-effect-receipt",
                        "schema_version":1,
                        "patch_set_id":loaded.patch_set.patch_set_id,
                        "operation_id":operation.operation_id,
                        "operation_fingerprint":operation.operation_fingerprint,
                        "observed_sha256":observed,
                        "state":state,
                        "recorded_at":Utc::now(),
                    }),
                })?)
            };
            receipts.push(
                PatchOperationReceiptV1 {
                    operation_id: operation.operation_id.clone(),
                    operation_fingerprint: operation.operation_fingerprint.clone(),
                    state,
                    observed_before_sha256: operation.before_sha256.clone(),
                    observed_after_sha256: observed,
                    effect_receipt_ref,
                    reason_code,
                    recorded_at: Utc::now(),
                    receipt_fingerprint: Sha256Hash::digest(b""),
                }
                .seal()
                .map_err(|_| ApplicationError::Invalid)?,
            );
        }
        Ok(receipts)
    }

    fn persist_patch_application(
        &self,
        project_root: &Path,
        patch_set_id: &PatchSetId,
        application: &PatchApplication,
    ) -> Result<ArtifactRef, ApplicationError> {
        self.persist_patch_document(
            &application.project_id,
            project_root,
            &format!(
                "management/patches-v2/{}/applications/{}-r{}.json",
                patch_set_id.as_str(),
                application.patch_application_id.as_str(),
                application.revision
            ),
            "patch_application",
            application.patch_application_id.as_str(),
            application,
        )
    }

    fn requested_patch_application(
        &self,
        loaded: &LoadedPatchV2,
        requested_by: ActorRef,
        permission_fingerprint: Sha256Hash,
    ) -> Result<PatchApplication, ApplicationError> {
        let now = Utc::now();
        PatchApplication {
            schema_id: star_contracts::patch_v2::PATCH_APPLICATION_SCHEMA_ID.to_owned(),
            schema_version: 1,
            patch_application_id: PatchApplicationId::new(),
            revision: 1,
            patch_set_ref: loaded
                .patch_set
                .reference()
                .map_err(|_| ApplicationError::Invalid)?,
            project_id: loaded.patch_set.project_id.clone(),
            checkout_id: loaded.patch_set.checkout_id.clone(),
            worktree_decision_ref: loaded
                .worktree_decision
                .reference()
                .map_err(|_| ApplicationError::Invalid)?,
            requested_patch_fingerprint: loaded.patch_set.patch_fingerprint.clone(),
            permission_fingerprint,
            pre_gate_decision_ref: None,
            permit_kind: None,
            operation_receipts: vec![],
            actual_operation_set_fingerprint: None,
            observed_after_change_set_ref: None,
            post_gate_decision_ref: None,
            reverse_patch_set_ref: None,
            recovery_strategy: None,
            state: PatchApplicationStateV1::Requested,
            reason_codes: vec![],
            requested_by,
            requested_at: now,
            updated_at: now,
            application_fingerprint: Sha256Hash::digest(b""),
        }
        .seal()
        .map_err(|_| ApplicationError::Invalid)
    }

    fn create_reverse_patch_v2(
        &self,
        loaded: &LoadedPatchV2,
        materialized: &[MaterializedPatchFile],
        after_workspace_snapshot_id: &star_contracts::ids::WorkspaceSnapshotId,
        requested_by: ActorRef,
        patch_application_id: &PatchApplicationId,
    ) -> Result<PatchSetV2, ApplicationError> {
        let recipe = exact_reverse_recipe_v2()?;
        let reversed = materialized
            .iter()
            .map(|file| MaterializedPatchFile {
                path: file.path.clone(),
                before_sha256: file.after_sha256.clone(),
                after_sha256: file.before_sha256.clone(),
                before_bytes: file.after_bytes.clone(),
                after_bytes: file.before_bytes.clone(),
            })
            .collect::<Vec<_>>();
        let observed_changes = reversed
            .iter()
            .map(|file| ObservedWorkspaceChange {
                path: file.path.clone(),
                rename_from: None,
                change_kind: ObservedChangeKind::Modify,
                before_sha256: Some(file.before_sha256.clone()),
                after_sha256: Some(file.after_sha256.clone()),
                staged: false,
                unstaged: false,
                untracked: false,
                binary: false,
            })
            .collect::<Vec<_>>();
        let reverse_task = patch_preview_task(
            &loaded.patch_set.project_id,
            &loaded.patch_set.checkout_id,
            &recipe,
            &reversed,
            None,
            PatchPreviewValidationPhase::PreApply,
        );
        let reverse_bundle = self.create_planning_bundle_for_phase_inner(
            reverse_task,
            requested_by,
            if loaded.patch_set.recipe_ref == rust_style_recipe_v2()?.reference() {
                rust_style_gate_check_descriptors(&loaded.patch_set.project_id)?
            } else {
                vec![]
            },
            &format!("patch-reverse-preview-{}", patch_application_id.as_str()),
            "patch_pre_apply",
            Some((loaded.patch_set.project_id.clone(), observed_changes)),
        )?;
        if reverse_bundle.validation_plan.readiness != ValidationPlanV2Readiness::Ready {
            return Err(ApplicationError::Apply(
                "PATCH_REVERSE_REPLAN_REQUIRED".to_owned(),
            ));
        }
        let change_set = reverse_bundle
            .change_sets
            .iter()
            .find(|change_set| change_set.project_id == loaded.patch_set.project_id)
            .ok_or(ApplicationError::Invalid)?;
        let preview_change_set_ref = DocumentRef {
            schema_id: star_contracts::planning::CHANGE_SET_SCHEMA_ID.to_owned(),
            document_id: change_set.change_set_id.to_string(),
            revision: 1,
            sha256: change_set.change_set_fingerprint.clone(),
        };
        let preview_impact_analysis_ref = DocumentRef {
            schema_id: star_contracts::planning::IMPACT_ANALYSIS_SCHEMA_ID.to_owned(),
            document_id: reverse_bundle
                .impact_analysis
                .impact_analysis_id
                .to_string(),
            revision: reverse_bundle.impact_analysis.revision,
            sha256: reverse_bundle
                .impact_analysis
                .calculation_fingerprint
                .clone(),
        };
        let preview_validation_plan_ref = DocumentRef {
            schema_id: star_contracts::planning::FULL_VALIDATION_PLAN_SCHEMA_ID.to_owned(),
            document_id: reverse_bundle
                .validation_plan
                .validation_plan_id
                .to_string(),
            revision: reverse_bundle.validation_plan.revision,
            sha256: application_document_hash(&reverse_bundle.validation_plan)?,
        };
        let reverse_patch_set_id = PatchSetId::new();
        let replay_ref = self.artifacts.put_json_with_policy(ArtifactWriteRequest {
            project_id: &loaded.patch_set.project_id,
            project_root: &loaded.project_root,
            relative_path: &format!(
                "management/patches-v2/{}/replay.json",
                reverse_patch_set_id.as_str()
            ),
            subject_kind: "recipe_replay",
            subject_id: reverse_patch_set_id.as_str(),
            policy: ArtifactWritePolicy {
                kind: ArtifactKind::Report,
                redaction_status: RedactionStatus::NotNeeded,
                retention_class: RetentionClass::Evidence,
            },
            value: &serde_json::json!({
                "schema_id":"star.recipe-replay-result",
                "schema_version":1,
                "patch_set_id":reverse_patch_set_id,
                "operation_count":0,
                "idempotence_proved":true,
                "already_satisfied_hashes":reversed
                    .iter()
                    .map(|file| (&file.path, &file.after_sha256))
                    .collect::<Vec<_>>(),
            }),
        })?;
        let mut paths = reversed
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        paths.sort();
        let expected_content_fingerprints = reversed
            .iter()
            .map(|file| (file.path.clone(), file.before_sha256.clone()))
            .collect::<BTreeMap<_, _>>();
        let recipe_execution = RecipeExecution {
            schema_id: star_contracts::patch_v2::RECIPE_EXECUTION_SCHEMA_ID.to_owned(),
            schema_version: 1,
            recipe_execution_id: RecipeExecutionId::new(),
            revision: 1,
            recipe_ref: recipe.reference(),
            project_id: loaded.patch_set.project_id.clone(),
            checkout_id: loaded.patch_set.checkout_id.clone(),
            base_workspace_snapshot_id: after_workspace_snapshot_id.clone(),
            target_selector: TargetSelector::Path {
                project_id: loaded.patch_set.project_id.clone(),
                paths,
                expected_content_fingerprints,
            },
            target_selector_fingerprint: Sha256Hash::digest(b""),
            parameters: serde_json::json!({}),
            parameter_fingerprint: Sha256Hash::digest(b""),
            worktree_decision_ref: loaded
                .worktree_decision
                .reference()
                .map_err(|_| ApplicationError::Invalid)?,
            first_preview_artifact_refs: loaded
                .patch_set
                .operations
                .iter()
                .flat_map(|operation| {
                    [
                        operation.reverse_artifact_ref.clone(),
                        operation.forward_artifact_ref.clone(),
                    ]
                })
                .collect(),
            replay_preview_artifact_refs: vec![replay_ref],
            preview_change_set_ref: Some(preview_change_set_ref.clone()),
            replan_bundle_ref: Some(DocumentRef {
                schema_id: "star.planning-bundle".to_owned(),
                document_id: reverse_bundle.task_spec.task_spec_id.to_string(),
                revision: reverse_bundle.task_spec.revision,
                sha256: reverse_bundle.bundle_fingerprint.clone(),
            }),
            idempotence_proved: true,
            completeness: star_contracts::evidence::Completeness::Complete,
            limitations: vec![],
            state: RecipeExecutionStateV1::Previewed,
            started_at: Utc::now(),
            finished_at: Some(Utc::now()),
            execution_fingerprint: Sha256Hash::digest(b""),
        }
        .seal()
        .map_err(|_| ApplicationError::Invalid)?;
        self.persist_patch_document(
            &loaded.patch_set.project_id,
            &loaded.project_root,
            &format!(
                "management/patches-v2/{}/worktree-decision-r{}.json",
                reverse_patch_set_id.as_str(),
                loaded.worktree_decision.revision
            ),
            "worktree_decision",
            loaded.worktree_decision.worktree_decision_id.as_str(),
            &loaded.worktree_decision,
        )?;
        self.persist_patch_document(
            &loaded.patch_set.project_id,
            &loaded.project_root,
            &format!(
                "management/patches-v2/{}/recipe-execution-r1.json",
                reverse_patch_set_id.as_str()
            ),
            "recipe_execution",
            recipe_execution.recipe_execution_id.as_str(),
            &recipe_execution,
        )?;
        let operations = loaded
            .patch_set
            .operations
            .iter()
            .enumerate()
            .map(|(index, operation)| PatchOperation {
                operation_id: format!("reverse-op-{index:04}"),
                kind: operation.kind,
                path: operation.path.clone(),
                rename_from: operation.rename_from.clone(),
                before_sha256: operation.after_sha256.clone(),
                after_sha256: operation.before_sha256.clone(),
                before_mode: operation.after_mode,
                after_mode: operation.before_mode,
                forward_artifact_ref: operation.reverse_artifact_ref.clone(),
                reverse_artifact_ref: operation.forward_artifact_ref.clone(),
                operation_fingerprint: Sha256Hash::digest(b""),
            })
            .collect();
        let reverse_patch = PatchSetV2 {
            schema_id: star_contracts::patch_v2::PATCH_SET_V2_SCHEMA_ID.to_owned(),
            schema_version: 2,
            patch_set_id: reverse_patch_set_id.clone(),
            revision: 1,
            recipe_ref: recipe.reference(),
            recipe_execution_ref: recipe_execution
                .reference()
                .map_err(|_| ApplicationError::Invalid)?,
            project_id: loaded.patch_set.project_id.clone(),
            checkout_id: loaded.patch_set.checkout_id.clone(),
            change_plan_id: loaded.patch_set.change_plan_id.clone(),
            change_plan_revision: loaded.patch_set.change_plan_revision,
            base_workspace_snapshot_id: after_workspace_snapshot_id.clone(),
            target_selector_fingerprint: recipe_execution.target_selector_fingerprint.clone(),
            parameter_fingerprint: recipe_execution.parameter_fingerprint.clone(),
            operations,
            preview_change_set_ref,
            preview_impact_analysis_ref,
            preview_validation_plan_ref,
            expected_operation_set_fingerprint: Sha256Hash::digest(b""),
            completeness: star_contracts::evidence::Completeness::Complete,
            limitations: vec![],
            state: PatchSetStateV2::Ready,
            created_at: Utc::now(),
            patch_fingerprint: Sha256Hash::digest(b""),
        }
        .seal()
        .map_err(|_| ApplicationError::Invalid)?;
        self.persist_patch_document(
            &loaded.patch_set.project_id,
            &loaded.project_root,
            &format!(
                "management/patches-v2/{}/patch-set-r1.json",
                reverse_patch_set_id.as_str()
            ),
            "patch_set_v2",
            reverse_patch_set_id.as_str(),
            &reverse_patch,
        )?;
        Ok(reverse_patch)
    }

    fn reverse_patch_materials_v2(
        &self,
        loaded: &LoadedPatchV2,
    ) -> Result<Vec<ReversePatchMaterialV2>, ApplicationError> {
        loaded
            .patch_set
            .operations
            .iter()
            .map(|operation| {
                Ok(ReversePatchMaterialV2 {
                    path: operation.path.clone(),
                    expected_after_sha256: operation
                        .after_sha256
                        .clone()
                        .ok_or(ApplicationError::Invalid)?,
                    after_bytes: self.read_patch_bytes(
                        &loaded.project_root,
                        &operation.forward_artifact_ref,
                        "forward",
                        &operation.path,
                    )?,
                    restore_before_sha256: operation
                        .before_sha256
                        .clone()
                        .ok_or(ApplicationError::Invalid)?,
                    restore_before_bytes: self.read_patch_bytes(
                        &loaded.project_root,
                        &operation.reverse_artifact_ref,
                        "reverse",
                        &operation.path,
                    )?,
                })
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn recover_applied_patch_v2(
        &self,
        loaded: &LoadedPatchV2,
        mut application: PatchApplication,
        mut compatibility_patch_set: PatchSet,
        pre_gate_decision: GateDecisionV2,
        post_gate_decision: Option<GateDecisionV2>,
        observed_after_change_set_ref: Option<DocumentRef>,
        reverse_patch_set_ref: Option<DocumentRef>,
        reason_code: &str,
    ) -> Result<PatchApplyV2Result, ApplicationError> {
        let repository = self.repositories.project(&loaded.patch_set.project_id)?;
        let materials = self.reverse_patch_materials_v2(loaded)?;
        let (recovered, state) =
            match recover_patch_set_v2(&loaded.project_root, &loaded.patch_set, &materials) {
                Ok(_) => {
                    compatibility_patch_set.status = PatchSetStatus::Reverted;
                    repository.save_patch_set(&compatibility_patch_set)?;
                    let _ = self.scan_project_inner(
                        &loaded.patch_set.project_id,
                        &format!(
                            "patch-v2-recovery-{}-r{}",
                            application.patch_application_id.as_str(),
                            application.revision + 1
                        ),
                    );
                    (true, PatchApplicationStateV1::Reverted)
                }
                Err(failure) => {
                    compatibility_patch_set.status = if failure.partial {
                        PatchSetStatus::PartiallyApplied
                    } else {
                        PatchSetStatus::Applied
                    };
                    repository.save_patch_set(&compatibility_patch_set)?;
                    (false, PatchApplicationStateV1::RecoveryBlocked)
                }
            };
        let receipts = self.persist_patch_operation_receipts(
            loaded,
            PatchOperationReceiptStateV1::RecoveryBlocked,
            if recovered {
                PatchOperationReceiptStateV1::RevertedExact
            } else {
                PatchOperationReceiptStateV1::RecoveryBlocked
            },
            Some(reason_code),
        )?;
        application.revision += 1;
        application.operation_receipts = receipts;
        application.actual_operation_set_fingerprint = None;
        application.observed_after_change_set_ref = observed_after_change_set_ref;
        application.post_gate_decision_ref = post_gate_decision
            .as_ref()
            .map(GateDecisionV2::reference)
            .transpose()
            .map_err(|_| ApplicationError::Invalid)?;
        application.reverse_patch_set_ref = reverse_patch_set_ref;
        application.recovery_strategy = Some(PatchRecoveryStrategyV1::ReversePatch);
        application.state = state;
        application.reason_codes = vec![reason_code.to_owned()];
        application.updated_at = Utc::now();
        application = application.seal().map_err(|_| ApplicationError::Invalid)?;
        self.persist_patch_application(
            &loaded.project_root,
            &loaded.patch_set.patch_set_id,
            &application,
        )?;
        Ok(PatchApplyV2Result {
            application,
            pre_gate_decision,
            post_gate_decision,
            source_effect_started: true,
            recovered,
            compatibility_patch_set,
        })
    }

    fn prepare_rust_style_validation_root(
        &self,
        loaded: &LoadedPatchV2,
        materialized: &[MaterializedPatchFile],
        phase: RustStyleGatePhase,
    ) -> Result<ValidationExecutionRootBinding, ApplicationError> {
        let runtime_root = self
            .rust_style_runtime_root
            .as_ref()
            .ok_or(ApplicationError::Invalid)?;
        let preview = materialize_rust_style_gate_preview(
            &loaded.patch_set.project_id,
            &loaded.project_root,
            runtime_root,
            phase,
        )?;
        if phase == RustStyleGatePhase::PreApply {
            let changes = materialized
                .iter()
                .map(|file| RustFileChange {
                    path: file.path.clone(),
                    before_sha256: file.before_sha256.clone(),
                    after_sha256: file.after_sha256.clone(),
                    before_bytes: file.before_bytes.clone(),
                    after_bytes: file.after_bytes.clone(),
                })
                .collect::<Vec<_>>();
            apply_owned_preview_changes(&preview.root, &changes)
                .map_err(|error| ApplicationError::RustStyle(error.into()))?;
        }
        if materialized.iter().any(|file| {
            observe_project_path_sha256(&preview.root, &file.path)
                .ok()
                .flatten()
                .as_ref()
                != Some(&file.after_sha256)
        }) {
            return Err(ApplicationError::Apply(
                "RUST_STYLE_VALIDATION_PREVIEW_MISMATCH".to_owned(),
            ));
        }
        let (kind, phase_name) = match phase {
            RustStyleGatePhase::PreApply => ("rust_style_candidate_preview", "patch_pre_apply"),
            RustStyleGatePhase::PostApply => {
                ("rust_style_actual_after_preview", "patch_post_apply")
            }
        };
        let binding_fingerprint = versioned_fingerprint(
            "star.rust-style-validation-root-binding",
            1,
            &serde_json::json!({
                "project_id":loaded.patch_set.project_id,
                "checkout_id":loaded.patch_set.checkout_id,
                "patch_set_id":loaded.patch_set.patch_set_id,
                "patch_fingerprint":loaded.patch_set.patch_fingerprint,
                "phase":phase_name,
                "isolation_ref":preview.isolation_ref,
                "expected_after":materialized.iter().map(|file| (&file.path, &file.after_sha256)).collect::<Vec<_>>(),
            }),
        )
        .map_err(|_| ApplicationError::Invalid)?;
        Ok(ValidationExecutionRootBinding {
            root: preview.root,
            kind,
            binding_fingerprint,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn apply_patch_v2(
        &self,
        patch_set_id: &PatchSetId,
        approved_patch_fingerprint: &str,
        requested_by: ActorRef,
        manual_approval_id: Option<&str>,
        validator_guard_evidence: Option<ValidatorGuardEvidenceV2>,
    ) -> Result<PatchApplyV2Result, ApplicationError> {
        let _guard = self.command_guard()?;
        self.apply_patch_v2_inner(
            patch_set_id,
            approved_patch_fingerprint,
            requested_by,
            manual_approval_id,
            validator_guard_evidence,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_patch_v2_inner(
        &self,
        patch_set_id: &PatchSetId,
        approved_patch_fingerprint: &str,
        requested_by: ActorRef,
        manual_approval_id: Option<&str>,
        validator_guard_evidence: Option<ValidatorGuardEvidenceV2>,
    ) -> Result<PatchApplyV2Result, ApplicationError> {
        let loaded = self.load_patch_v2(patch_set_id)?;
        if loaded.patch_set.state != PatchSetStateV2::Ready
            || loaded.patch_set.patch_fingerprint.as_str() != approved_patch_fingerprint
            || !matches!(
                loaded.worktree_decision.state,
                WorktreeDecisionStateV1::Selected
                    | WorktreeDecisionStateV1::Materialized
                    | WorktreeDecisionStateV1::RetainedForRecovery
            )
            || requested_by.actor_id.trim().is_empty()
            || requested_by.auth_source.trim().is_empty()
            || observe_patch_set_v2(&loaded.project_root, &loaded.patch_set)
                != PatchFilesystemStateV2::Before
        {
            return Err(ApplicationError::Apply(
                "PATCH_V2_APPROVAL_OR_STATE_MISMATCH".to_owned(),
            ));
        }
        let recipe = [
            trailing_whitespace_recipe_v2()?,
            managed_declaration_recipe_v2()?,
            exact_reverse_recipe_v2()?,
            rust_style_recipe_v2()?,
        ]
        .into_iter()
        .find(|recipe| loaded.patch_set.recipe_ref == recipe.reference())
        .ok_or_else(|| ApplicationError::Apply("PATCH_V2_RECIPE_UNKNOWN".to_owned()))?;
        if loaded.recipe_execution.recipe_ref != loaded.patch_set.recipe_ref {
            return Err(ApplicationError::Apply(
                "PATCH_V2_RECIPE_BINDING_MISMATCH".to_owned(),
            ));
        }
        let rust_style_gate_descriptors = if recipe.recipe_id == "rust_style_v1" {
            Some(rust_style_gate_check_descriptors(
                &loaded.patch_set.project_id,
            )?)
        } else {
            None
        };
        let repository = self.repositories.project(&loaded.patch_set.project_id)?;
        let mut compatibility_patch_set = repository
            .get_patch_set(patch_set_id)?
            .ok_or(ApplicationError::NotFound)?;
        let materialized = self.materialize_loaded_patch_v2(&loaded)?;
        let pre_execution_root = if recipe.recipe_id == "rust_style_v1" {
            Some(self.prepare_rust_style_validation_root(
                &loaded,
                &materialized,
                RustStyleGatePhase::PreApply,
            )?)
        } else {
            None
        };
        let permission_fingerprint = versioned_fingerprint(
            "star.patch-permission-binding",
            2,
            &serde_json::json!({
                "project_id":loaded.patch_set.project_id,
                "checkout_id":loaded.patch_set.checkout_id,
                "worktree_decision_ref":loaded.worktree_decision.reference()
                    .map_err(|_| ApplicationError::Invalid)?,
                "recipe_ref":loaded.patch_set.recipe_ref,
                "permission_actions":recipe.permission_actions,
                "target_selector_fingerprint":loaded.patch_set.target_selector_fingerprint,
            }),
        )
        .map_err(|_| ApplicationError::Invalid)?;
        let mut application = self.requested_patch_application(
            &loaded,
            requested_by.clone(),
            permission_fingerprint.clone(),
        )?;
        self.persist_patch_application(
            &loaded.project_root,
            &loaded.patch_set.patch_set_id,
            &application,
        )?;

        let planning_ref = loaded
            .recipe_execution
            .replan_bundle_ref
            .as_ref()
            .ok_or(ApplicationError::Invalid)?;
        let task_spec_id = TaskSpecId::parse(planning_ref.document_id.clone())
            .map_err(|_| ApplicationError::Invalid)?;
        let planning_bundle = self
            .repositories
            .global()
            .get_planning_bundle(&task_spec_id)?
            .ok_or(ApplicationError::NotFound)?;
        if planning_ref.revision != planning_bundle.task_spec.revision
            || planning_ref.sha256 != planning_bundle.bundle_fingerprint
            || planning_bundle.validation_plan.phase != "patch_pre_apply"
        {
            return Err(ApplicationError::Apply(
                "PATCH_V2_PRE_GATE_LINEAGE_STALE".to_owned(),
            ));
        }
        let pre_gate_run = match self.execute_planning_bundle_registered_with_evidence_inner(
            &task_spec_id,
            &loaded.project_root,
            GateScope::Merge {
                project_id: loaded.patch_set.project_id.clone(),
                revision: loaded.patch_set.revision,
            },
            requested_by.clone(),
            false,
            vec![],
            validator_guard_evidence.clone(),
            pre_execution_root,
        ) {
            Ok(run) => run,
            Err(error) => {
                application.revision += 1;
                application.reason_codes = vec!["PATCH_PRE_GATE_EXECUTION_FAILED".to_owned()];
                application.updated_at = Utc::now();
                application = application.seal().map_err(|_| ApplicationError::Invalid)?;
                self.persist_patch_application(
                    &loaded.project_root,
                    &loaded.patch_set.patch_set_id,
                    &application,
                )?;
                return Err(error);
            }
        };
        let pre_gate_decision = pre_gate_run.gate_decision;
        let pre_gate_ref = pre_gate_decision
            .reference()
            .map_err(|_| ApplicationError::Invalid)?;
        let pending_reason = match pre_gate_decision.decision {
            GateDecisionKind::Block => Some("PATCH_PRE_GATE_BLOCKED"),
            GateDecisionKind::HumanReview
                if manual_approval_id.is_none() || requested_by.actor_type != ActorType::User =>
            {
                Some("PATCH_PRE_GATE_HUMAN_APPROVAL_REQUIRED")
            }
            GateDecisionKind::AutoPass | GateDecisionKind::HumanReview => None,
        };
        if let Some(reason) = pending_reason {
            application.revision += 1;
            application.pre_gate_decision_ref = Some(pre_gate_ref);
            application.reason_codes = vec![reason.to_owned()];
            application.updated_at = Utc::now();
            application = application.seal().map_err(|_| ApplicationError::Invalid)?;
            self.persist_patch_application(
                &loaded.project_root,
                &loaded.patch_set.patch_set_id,
                &application,
            )?;
            return Ok(PatchApplyV2Result {
                application,
                pre_gate_decision,
                post_gate_decision: None,
                source_effect_started: false,
                recovered: false,
                compatibility_patch_set,
            });
        }
        let before_binding_set_fingerprint =
            pre_gate_decision.subject_binding_set_fingerprint.clone();
        let verified_pre_gate = VerifiedPatchGateV2::from_persisted_gate(
            &pre_gate_decision,
            GatePhaseV2::PatchPreApply,
        )
        .map_err(|_| ApplicationError::Apply("PATCH_PRE_GATE_INVALID".to_owned()))?;
        let manual_approval = if pre_gate_decision.decision == GateDecisionKind::HumanReview {
            Some(
                ManualPatchApprovalV2::seal(
                    manual_approval_id
                        .ok_or(ApplicationError::Invalid)?
                        .to_owned(),
                    requested_by.clone(),
                    pre_gate_ref.clone(),
                    loaded.patch_set.patch_fingerprint.clone(),
                    before_binding_set_fingerprint.clone(),
                    permission_fingerprint.clone(),
                    Utc::now(),
                )
                .map_err(|_| ApplicationError::Apply("PATCH_MANUAL_APPROVAL_INVALID".to_owned()))?,
            )
        } else {
            None
        };
        let mut permit = issue_patch_apply_permit(
            &verified_pre_gate,
            loaded.patch_set.patch_fingerprint.clone(),
            before_binding_set_fingerprint.clone(),
            permission_fingerprint.clone(),
            manual_approval.as_ref(),
            Utc::now(),
        )
        .map_err(|_| ApplicationError::Apply("PATCH_APPLY_PERMIT_REJECTED".to_owned()))?;
        let permit_kind = match permit.kind() {
            PatchPermitKindV2::Automatic => PatchPermitKindRecordV1::Automatic,
            PatchPermitKindV2::ManualApproved => PatchPermitKindRecordV1::ManualApproved,
        };
        application.revision += 1;
        application.pre_gate_decision_ref = Some(pre_gate_ref);
        application.permit_kind = Some(permit_kind);
        application.state = PatchApplicationStateV1::Preflighted;
        application.reason_codes.clear();
        application.updated_at = Utc::now();
        application = application.seal().map_err(|_| ApplicationError::Invalid)?;
        self.persist_patch_application(
            &loaded.project_root,
            &loaded.patch_set.patch_set_id,
            &application,
        )?;
        let permit_use = permit
            .consume(
                &loaded.patch_set.patch_fingerprint,
                &before_binding_set_fingerprint,
                &permission_fingerprint,
            )
            .map_err(|_| ApplicationError::Apply("PATCH_APPLY_PERMIT_CONSUME_FAILED".to_owned()))?;
        application.revision += 1;
        application.state = PatchApplicationStateV1::Applying;
        application.updated_at = Utc::now();
        application = application.seal().map_err(|_| ApplicationError::Invalid)?;
        self.persist_patch_application(
            &loaded.project_root,
            &loaded.patch_set.patch_set_id,
            &application,
        )?;

        let mutation_request = SourceMutationRequest {
            patch_set: loaded.patch_set.clone(),
            files: materialized
                .iter()
                .map(|file| MaterializedRewrite {
                    path: file.path.clone(),
                    before_sha256: file.before_sha256.clone(),
                    after_sha256: file.after_sha256.clone(),
                    before_bytes: file.before_bytes.clone(),
                    after_bytes: file.after_bytes.clone(),
                })
                .collect(),
        };
        let mutation = ExactFileSourceMutationAdapter.apply(
            &loaded.project_root,
            &mutation_request,
            permit_use,
        );
        let mutation_result = match mutation {
            Ok(result) => result,
            Err(error) => {
                let filesystem_state =
                    observe_patch_set_v2(&loaded.project_root, &loaded.patch_set);
                compatibility_patch_set.status = match filesystem_state {
                    PatchFilesystemStateV2::Before => PatchSetStatus::Failed,
                    PatchFilesystemStateV2::After => PatchSetStatus::Applied,
                    PatchFilesystemStateV2::Mixed | PatchFilesystemStateV2::Unknown => {
                        PatchSetStatus::PartiallyApplied
                    }
                };
                repository.save_patch_set(&compatibility_patch_set)?;
                let (state, recovered) = match filesystem_state {
                    PatchFilesystemStateV2::Before => (PatchApplicationStateV1::Reverted, true),
                    PatchFilesystemStateV2::After => {
                        (PatchApplicationStateV1::RecoveryRequired, false)
                    }
                    PatchFilesystemStateV2::Mixed => {
                        (PatchApplicationStateV1::PartiallyApplied, false)
                    }
                    PatchFilesystemStateV2::Unknown => {
                        (PatchApplicationStateV1::OutcomeUnknown, false)
                    }
                };
                let reason_code = match error {
                    PatchPortError::Invalid => "PATCH_SOURCE_MUTATION_INVALID",
                    PatchPortError::Unsafe => "PATCH_SOURCE_MUTATION_UNSAFE",
                    PatchPortError::Unavailable => "PATCH_SOURCE_MUTATION_UNAVAILABLE",
                    PatchPortError::Partial => "PATCH_SOURCE_MUTATION_PARTIAL",
                    PatchPortError::OutcomeUnknown => "PATCH_SOURCE_MUTATION_OUTCOME_UNKNOWN",
                };
                application.revision += 1;
                application.operation_receipts = self.persist_patch_operation_receipts(
                    &loaded,
                    PatchOperationReceiptStateV1::FailedAfterEffect,
                    PatchOperationReceiptStateV1::FailedBeforeEffect,
                    Some(reason_code),
                )?;
                application.state = state;
                application.reason_codes = vec![reason_code.to_owned()];
                application.recovery_strategy =
                    (!recovered).then_some(PatchRecoveryStrategyV1::ReversePatch);
                application.updated_at = Utc::now();
                application = application.seal().map_err(|_| ApplicationError::Invalid)?;
                self.persist_patch_application(
                    &loaded.project_root,
                    &loaded.patch_set.patch_set_id,
                    &application,
                )?;
                return Ok(PatchApplyV2Result {
                    application,
                    pre_gate_decision,
                    post_gate_decision: None,
                    source_effect_started: filesystem_state != PatchFilesystemStateV2::Before,
                    recovered,
                    compatibility_patch_set,
                });
            }
        };
        compatibility_patch_set.status = PatchSetStatus::Applied;
        repository.save_patch_set(&compatibility_patch_set)?;
        if mutation_result.state != SourceMutationState::AppliedExact
            || mutation_result.observations.len() != loaded.patch_set.operations.len()
            || mutation_result.observations.iter().any(|observation| {
                loaded
                    .patch_set
                    .operations
                    .iter()
                    .find(|operation| operation.path == observation.path)
                    .is_none_or(|operation| {
                        operation.after_sha256.as_ref() != observation.observed_sha256.as_ref()
                    })
            })
            || observe_patch_set_v2(&loaded.project_root, &loaded.patch_set)
                != PatchFilesystemStateV2::After
        {
            return self.recover_applied_patch_v2(
                &loaded,
                application,
                compatibility_patch_set,
                pre_gate_decision,
                None,
                None,
                None,
                "PATCH_ACTUAL_OPERATION_SET_MISMATCH",
            );
        }
        let post_scan = match self.scan_project_inner(
            &loaded.patch_set.project_id,
            &format!(
                "patch-v2-post-{}",
                application.patch_application_id.as_str()
            ),
        ) {
            Ok(scan) => scan,
            Err(_) => {
                return self.recover_applied_patch_v2(
                    &loaded,
                    application,
                    compatibility_patch_set.clone(),
                    pre_gate_decision,
                    None,
                    None,
                    None,
                    "PATCH_POST_SCAN_FAILED",
                );
            }
        };
        if loaded.patch_set.operations.iter().any(|operation| {
            operation
                .path
                .as_str()
                .starts_with(".star-control/registry/")
        }) {
            let manifest_path =
                ProjectPathRef::parse(".star-control/registry/manifest.toml".to_owned())
                    .map_err(|_| ApplicationError::Invalid)?;
            if self
                .refresh_managed_registry_resolution_inner(
                    &loaded.patch_set.project_id,
                    &manifest_path,
                )
                .is_err()
            {
                return self.recover_applied_patch_v2(
                    &loaded,
                    application,
                    compatibility_patch_set.clone(),
                    pre_gate_decision,
                    None,
                    None,
                    None,
                    "PATCH_POST_REGISTRY_RESOLUTION_FAILED",
                );
            }
        }
        let reverse_patch = match self.create_reverse_patch_v2(
            &loaded,
            &materialized,
            &post_scan.scan_run.workspace_snapshot_id,
            requested_by.clone(),
            &application.patch_application_id,
        ) {
            Ok(reverse) => reverse,
            Err(_) => {
                return self.recover_applied_patch_v2(
                    &loaded,
                    application,
                    compatibility_patch_set.clone(),
                    pre_gate_decision,
                    None,
                    None,
                    None,
                    "PATCH_REVERSE_EVIDENCE_FAILED",
                );
            }
        };
        let reverse_patch_set_ref = match reverse_patch.reference() {
            Ok(reference) => reference,
            Err(_) => {
                return self.recover_applied_patch_v2(
                    &loaded,
                    application,
                    compatibility_patch_set.clone(),
                    pre_gate_decision,
                    None,
                    None,
                    None,
                    "PATCH_REVERSE_REFERENCE_INVALID",
                );
            }
        };
        let post_observed_changes = materialized
            .iter()
            .map(|file| ObservedWorkspaceChange {
                path: file.path.clone(),
                rename_from: None,
                change_kind: ObservedChangeKind::Modify,
                before_sha256: Some(file.before_sha256.clone()),
                after_sha256: Some(file.after_sha256.clone()),
                staged: false,
                unstaged: false,
                untracked: false,
                binary: false,
            })
            .collect::<Vec<_>>();
        let post_bundle = match self.create_planning_bundle_for_phase_inner(
            patch_preview_task(
                &loaded.patch_set.project_id,
                &loaded.patch_set.checkout_id,
                &recipe,
                &materialized,
                Some(&loaded.recipe_execution.target_selector),
                PatchPreviewValidationPhase::PostApply,
            ),
            requested_by.clone(),
            rust_style_gate_descriptors.clone().unwrap_or_default(),
            &format!("patch-post-{}", application.patch_application_id.as_str()),
            "patch_post_apply",
            Some((loaded.patch_set.project_id.clone(), post_observed_changes)),
        ) {
            Ok(bundle) if bundle.validation_plan.readiness == ValidationPlanV2Readiness::Ready => {
                bundle
            }
            Ok(_) | Err(_) => {
                return self.recover_applied_patch_v2(
                    &loaded,
                    application,
                    compatibility_patch_set.clone(),
                    pre_gate_decision,
                    None,
                    None,
                    Some(reverse_patch_set_ref),
                    "PATCH_POST_REPLAN_FAILED",
                );
            }
        };
        let observed_after_change_set = match post_bundle
            .change_sets
            .iter()
            .find(|change_set| change_set.project_id == loaded.patch_set.project_id)
        {
            Some(change_set) => change_set,
            None => {
                return self.recover_applied_patch_v2(
                    &loaded,
                    application,
                    compatibility_patch_set.clone(),
                    pre_gate_decision,
                    None,
                    None,
                    Some(reverse_patch_set_ref),
                    "PATCH_POST_CHANGE_SET_MISSING",
                );
            }
        };
        let observed_after_change_set_ref = DocumentRef {
            schema_id: star_contracts::planning::CHANGE_SET_SCHEMA_ID.to_owned(),
            document_id: observed_after_change_set.change_set_id.to_string(),
            revision: 1,
            sha256: observed_after_change_set.change_set_fingerprint.clone(),
        };
        let post_execution_root = if recipe.recipe_id == "rust_style_v1" {
            match self.prepare_rust_style_validation_root(
                &loaded,
                &materialized,
                RustStyleGatePhase::PostApply,
            ) {
                Ok(binding) => Some(binding),
                Err(_) => {
                    return self.recover_applied_patch_v2(
                        &loaded,
                        application,
                        compatibility_patch_set.clone(),
                        pre_gate_decision,
                        None,
                        Some(observed_after_change_set_ref),
                        Some(reverse_patch_set_ref),
                        "PATCH_POST_VALIDATION_ISOLATION_FAILED",
                    );
                }
            }
        } else {
            None
        };
        let post_gate_run = match self.execute_planning_bundle_registered_with_evidence_inner(
            &post_bundle.task_spec.task_spec_id,
            &loaded.project_root,
            GateScope::Merge {
                project_id: loaded.patch_set.project_id.clone(),
                revision: application.revision + 1,
            },
            requested_by,
            false,
            vec![],
            validator_guard_evidence,
            post_execution_root,
        ) {
            Ok(run) => run,
            Err(_) => {
                return self.recover_applied_patch_v2(
                    &loaded,
                    application,
                    compatibility_patch_set.clone(),
                    pre_gate_decision,
                    None,
                    Some(observed_after_change_set_ref),
                    Some(reverse_patch_set_ref),
                    "PATCH_POST_GATE_EXECUTION_FAILED",
                );
            }
        };
        let post_gate_decision = post_gate_run.gate_decision;
        let verified_post_gate = match VerifiedPatchGateV2::from_persisted_gate(
            &post_gate_decision,
            GatePhaseV2::PatchPostApply,
        ) {
            Ok(gate) => gate,
            Err(_) => {
                return self.recover_applied_patch_v2(
                    &loaded,
                    application,
                    compatibility_patch_set.clone(),
                    pre_gate_decision,
                    Some(post_gate_decision),
                    Some(observed_after_change_set_ref),
                    Some(reverse_patch_set_ref),
                    "PATCH_POST_GATE_INVALID",
                );
            }
        };
        let actual_operation_set_fingerprint = match canonical_sha256(&serde_json::json!({
            "domain":"star.patch-operation-set",
            "version":2,
            "value":loaded
                .patch_set
                .operations
                .iter()
                .map(|operation| &operation.operation_fingerprint)
                .collect::<Vec<_>>(),
        })) {
            Ok(fingerprint) => fingerprint,
            Err(_) => {
                return self.recover_applied_patch_v2(
                    &loaded,
                    application,
                    compatibility_patch_set.clone(),
                    pre_gate_decision,
                    Some(post_gate_decision),
                    Some(observed_after_change_set_ref),
                    Some(reverse_patch_set_ref),
                    "PATCH_ACTUAL_OPERATION_FINGERPRINT_FAILED",
                );
            }
        };
        let disposition = evaluate_patch_post_apply(
            PatchApplicationStateV2::AppliedExact,
            &loaded.patch_set.expected_operation_set_fingerprint,
            &actual_operation_set_fingerprint,
            &before_binding_set_fingerprint,
            &post_gate_decision.subject_binding_set_fingerprint,
            &verified_post_gate,
            Utc::now(),
        );
        if disposition == PatchPostApplyDispositionV2::RecoveryRequired {
            return self.recover_applied_patch_v2(
                &loaded,
                application,
                compatibility_patch_set.clone(),
                pre_gate_decision,
                Some(post_gate_decision),
                Some(observed_after_change_set_ref),
                Some(reverse_patch_set_ref),
                "PATCH_POST_GATE_RECOVERY_REQUIRED",
            );
        }
        compatibility_patch_set.applied_workspace_snapshot_id =
            Some(post_scan.scan_run.workspace_snapshot_id);
        repository.save_patch_set(&compatibility_patch_set)?;
        application.revision += 1;
        application.operation_receipts = match self.persist_patch_operation_receipts(
            &loaded,
            PatchOperationReceiptStateV1::AppliedExact,
            PatchOperationReceiptStateV1::OutcomeUnknown,
            Some("PATCH_EXPECTED_AFTER_NOT_OBSERVED"),
        ) {
            Ok(receipts) => receipts,
            Err(_) => {
                return self.recover_applied_patch_v2(
                    &loaded,
                    application,
                    compatibility_patch_set.clone(),
                    pre_gate_decision,
                    Some(post_gate_decision),
                    Some(observed_after_change_set_ref),
                    Some(reverse_patch_set_ref),
                    "PATCH_EFFECT_RECEIPT_PERSISTENCE_FAILED",
                );
            }
        };
        application.actual_operation_set_fingerprint = Some(actual_operation_set_fingerprint);
        application.observed_after_change_set_ref = Some(observed_after_change_set_ref);
        application.post_gate_decision_ref = match post_gate_decision.reference() {
            Ok(reference) => Some(reference),
            Err(_) => {
                let recovery_observed = application.observed_after_change_set_ref.clone();
                return self.recover_applied_patch_v2(
                    &loaded,
                    application,
                    compatibility_patch_set.clone(),
                    pre_gate_decision,
                    Some(post_gate_decision),
                    recovery_observed,
                    Some(reverse_patch_set_ref),
                    "PATCH_POST_GATE_REFERENCE_INVALID",
                );
            }
        };
        application.reverse_patch_set_ref = Some(reverse_patch_set_ref.clone());
        application.state = match disposition {
            PatchPostApplyDispositionV2::Complete => PatchApplicationStateV1::Applied,
            PatchPostApplyDispositionV2::AwaitingHumanReview => {
                PatchApplicationStateV1::AwaitingHumanReview
            }
            PatchPostApplyDispositionV2::RecoveryRequired => unreachable!(),
        };
        application.reason_codes =
            if disposition == PatchPostApplyDispositionV2::AwaitingHumanReview {
                vec!["PATCH_POST_GATE_HUMAN_REVIEW_REQUIRED".to_owned()]
            } else {
                vec![]
            };
        application.updated_at = Utc::now();
        let recovery_application = application.clone();
        let recovery_observed = application.observed_after_change_set_ref.clone();
        application = match application.seal() {
            Ok(application) => application,
            Err(_) => {
                return self.recover_applied_patch_v2(
                    &loaded,
                    recovery_application,
                    compatibility_patch_set.clone(),
                    pre_gate_decision,
                    Some(post_gate_decision),
                    recovery_observed,
                    Some(reverse_patch_set_ref),
                    "PATCH_APPLICATION_SEAL_FAILED",
                );
            }
        };
        if self
            .persist_patch_application(
                &loaded.project_root,
                &loaded.patch_set.patch_set_id,
                &application,
            )
            .is_err()
        {
            let recovery_observed = application.observed_after_change_set_ref.clone();
            let recovery_reverse = application.reverse_patch_set_ref.clone();
            return self.recover_applied_patch_v2(
                &loaded,
                application,
                compatibility_patch_set.clone(),
                pre_gate_decision,
                Some(post_gate_decision),
                recovery_observed,
                recovery_reverse,
                "PATCH_APPLICATION_PERSISTENCE_FAILED",
            );
        }
        Ok(PatchApplyV2Result {
            application,
            pre_gate_decision,
            post_gate_decision: Some(post_gate_decision),
            source_effect_started: true,
            recovered: false,
            compatibility_patch_set,
        })
    }

    fn load_patch_application(
        &self,
        patch_application_id: &PatchApplicationId,
    ) -> Result<(LoadedPatchV2, PatchApplication), ApplicationError> {
        for project in self.repositories.global().list_projects()? {
            let root = self.primary_project_root(&project)?;
            let discovery = self
                .artifacts
                .discover_verified(&project.project_id, &root)?;
            let marker = format!("/applications/{}-r", patch_application_id.as_str());
            let mut candidates = Vec::new();
            for artifact in discovery
                .verified
                .iter()
                .filter(|artifact| artifact.relative_path.contains(&marker))
            {
                let value = self.artifacts.read_json(&root, artifact)?;
                let application = serde_json::from_value::<PatchApplication>(value)
                    .map_err(|_| ApplicationError::Invalid)?;
                let sealed = application
                    .clone()
                    .seal()
                    .map_err(|_| ApplicationError::Invalid)?;
                if sealed != application || application.project_id != project.project_id {
                    return Err(ApplicationError::Invalid);
                }
                candidates.push(application);
            }
            candidates.sort_by_key(|application| application.revision);
            if let Some(application) = candidates.pop() {
                if candidates
                    .last()
                    .is_some_and(|prior| prior.revision == application.revision)
                {
                    return Err(ApplicationError::Invalid);
                }
                let patch_set_id = PatchSetId::parse(application.patch_set_ref.document_id.clone())
                    .map_err(|_| ApplicationError::Invalid)?;
                let loaded = self.load_patch_v2(&patch_set_id)?;
                if application.patch_set_ref
                    != loaded
                        .patch_set
                        .reference()
                        .map_err(|_| ApplicationError::Invalid)?
                {
                    return Err(ApplicationError::Invalid);
                }
                return Ok((loaded, application));
            }
            if discovery.rejected_count > 0 {
                return Err(ApplicationError::Repository(RepositoryError::new(
                    RepositoryErrorCategory::IntegrityFailed,
                    "patch application discovery rejected an artifact",
                )));
            }
        }
        Err(ApplicationError::NotFound)
    }

    pub fn patch_status_v2(
        &self,
        patch_application_id: &PatchApplicationId,
    ) -> Result<PatchStatusV2Result, ApplicationError> {
        let _guard = self.command_guard()?;
        let (loaded, application) = self.load_patch_application(patch_application_id)?;
        let filesystem_state = observe_patch_set_v2(&loaded.project_root, &loaded.patch_set);
        let (observed_state, reconciliation_reason_codes, mut recovery_strategies) =
            match filesystem_state {
                PatchFilesystemStateV2::Before => (
                    if matches!(
                        application.state,
                        PatchApplicationStateV1::Requested | PatchApplicationStateV1::Preflighted
                    ) {
                        application.state
                    } else {
                        PatchApplicationStateV1::Reverted
                    },
                    vec!["PATCH_FILES_MATCH_EXACT_BEFORE".to_owned()],
                    vec![],
                ),
                PatchFilesystemStateV2::After => (
                    match application.state {
                        PatchApplicationStateV1::Applied
                        | PatchApplicationStateV1::AwaitingHumanReview
                        | PatchApplicationStateV1::RecoveryRequired => application.state,
                        PatchApplicationStateV1::Requested
                        | PatchApplicationStateV1::Preflighted
                        | PatchApplicationStateV1::Applying
                        | PatchApplicationStateV1::PartiallyApplied
                        | PatchApplicationStateV1::OutcomeUnknown
                        | PatchApplicationStateV1::Reverted
                        | PatchApplicationStateV1::IsolatedDiscarded
                        | PatchApplicationStateV1::RecoveryBlocked => {
                            PatchApplicationStateV1::RecoveryRequired
                        }
                    },
                    vec!["PATCH_FILES_MATCH_EXACT_AFTER".to_owned()],
                    vec![PatchRecoveryStrategyV1::ReversePatch],
                ),
                PatchFilesystemStateV2::Mixed => (
                    PatchApplicationStateV1::PartiallyApplied,
                    vec!["PATCH_FILES_MIXED_BEFORE_AFTER".to_owned()],
                    vec![],
                ),
                PatchFilesystemStateV2::Unknown => (
                    PatchApplicationStateV1::OutcomeUnknown,
                    vec!["PATCH_FILES_DO_NOT_MATCH_BOUND_HASHES".to_owned()],
                    vec![],
                ),
            };
        if loaded.worktree_decision.strategy == WorktreeStrategyV1::Isolated
            && loaded.worktree_decision.state == WorktreeDecisionStateV1::RetainedForRecovery
            && filesystem_state == PatchFilesystemStateV2::Before
        {
            recovery_strategies.push(PatchRecoveryStrategyV1::DiscardIsolated);
            recovery_strategies.sort();
            recovery_strategies.dedup();
        }
        Ok(PatchStatusV2Result {
            application,
            observed_state,
            reconciliation_reason_codes,
            recovery_strategies,
        })
    }

    pub fn recover_patch_v2(
        &self,
        patch_application_id: &PatchApplicationId,
        strategy: PatchRecoveryStrategyV1,
        requested_by: ActorRef,
    ) -> Result<PatchRecoverV2Result, ApplicationError> {
        let _guard = self.command_guard()?;
        let (loaded, mut application) = self.load_patch_application(patch_application_id)?;
        if requested_by.actor_id.trim().is_empty() || requested_by.auth_source.trim().is_empty() {
            return Err(ApplicationError::Apply(
                "PATCH_RECOVERY_PRECONDITION_FAILED".to_owned(),
            ));
        }
        if strategy == PatchRecoveryStrategyV1::DiscardIsolated {
            if loaded.worktree_decision.strategy != WorktreeStrategyV1::Isolated
                || loaded.worktree_decision.state != WorktreeDecisionStateV1::RetainedForRecovery
                || observe_patch_set_v2(&loaded.project_root, &loaded.patch_set)
                    != PatchFilesystemStateV2::Before
            {
                return Err(ApplicationError::Apply(
                    "PATCH_RECOVERY_PRECONDITION_FAILED".to_owned(),
                ));
            }
            let adapter = GitWorktreeAdapter::new(
                std::env::temp_dir()
                    .join("Star-Control")
                    .join("isolated-worktrees"),
            )
            .map_err(|_| ApplicationError::Apply("PATCH_WORKTREE_UNAVAILABLE".to_owned()))?;
            let materialization = WorktreeMaterialization {
                root: std::env::temp_dir()
                    .join("Star-Control")
                    .join("isolated-worktrees")
                    .join(loaded.worktree_decision.worktree_decision_id.as_str()),
                locator_fingerprint: loaded
                    .worktree_decision
                    .isolated_locator_fingerprint
                    .clone()
                    .ok_or(ApplicationError::Invalid)?,
                evidence_refs: loaded
                    .worktree_decision
                    .materialization_artifact_refs
                    .clone(),
            };
            adapter
                .discard(&loaded.project_root, &materialization)
                .map_err(|_| ApplicationError::Apply("PATCH_WORKTREE_DISCARD_FAILED".to_owned()))?;
            let mut worktree_decision = loaded.worktree_decision.clone();
            worktree_decision.revision += 1;
            worktree_decision.state = WorktreeDecisionStateV1::Discarded;
            worktree_decision.reason_codes = vec!["ISOLATED_GIT_WORKTREE_DISCARDED".to_owned()];
            worktree_decision.updated_at = Utc::now();
            worktree_decision = worktree_decision
                .seal()
                .map_err(|_| ApplicationError::Invalid)?;
            self.persist_patch_document(
                &loaded.patch_set.project_id,
                &loaded.project_root,
                &format!(
                    "management/patches-v2/{}/worktree-decision-r{}.json",
                    loaded.patch_set.patch_set_id.as_str(),
                    worktree_decision.revision
                ),
                "worktree_decision",
                worktree_decision.worktree_decision_id.as_str(),
                &worktree_decision,
            )?;
            application.revision += 1;
            application.worktree_decision_ref = worktree_decision
                .reference()
                .map_err(|_| ApplicationError::Invalid)?;
            application.state = if application.operation_receipts.is_empty()
                && application.actual_operation_set_fingerprint.is_none()
                && application.observed_after_change_set_ref.is_none()
                && application.post_gate_decision_ref.is_none()
            {
                PatchApplicationStateV1::IsolatedDiscarded
            } else {
                PatchApplicationStateV1::Reverted
            };
            application.recovery_strategy = Some(strategy);
            application.reason_codes.clear();
            application.updated_at = Utc::now();
            application.requested_by = requested_by;
            application = application.seal().map_err(|_| ApplicationError::Invalid)?;
            self.persist_patch_application(
                &loaded.project_root,
                &loaded.patch_set.patch_set_id,
                &application,
            )?;
            return Ok(PatchRecoverV2Result {
                application,
                recovered: true,
            });
        }
        if strategy != PatchRecoveryStrategyV1::ReversePatch
            || observe_patch_set_v2(&loaded.project_root, &loaded.patch_set)
                != PatchFilesystemStateV2::After
            || application.pre_gate_decision_ref.is_none()
            || application.permit_kind.is_none()
        {
            return Err(ApplicationError::Apply(
                "PATCH_RECOVERY_PRECONDITION_FAILED".to_owned(),
            ));
        }
        let materials = self.reverse_patch_materials_v2(&loaded)?;
        recover_patch_set_v2(&loaded.project_root, &loaded.patch_set, &materials)
            .map_err(apply_failure)?;
        if let Some(mut compatibility_patch_set) = self
            .repositories
            .project(&loaded.patch_set.project_id)?
            .get_patch_set(&loaded.patch_set.patch_set_id)?
        {
            compatibility_patch_set.status = PatchSetStatus::Reverted;
            self.repositories
                .project(&loaded.patch_set.project_id)?
                .save_patch_set(&compatibility_patch_set)?;
        }
        application.revision += 1;
        application.operation_receipts = self.persist_patch_operation_receipts(
            &loaded,
            PatchOperationReceiptStateV1::RecoveryBlocked,
            PatchOperationReceiptStateV1::RevertedExact,
            Some("PATCH_EXPLICIT_RECOVERY_FAILED"),
        )?;
        application.state = PatchApplicationStateV1::Reverted;
        application.recovery_strategy = Some(strategy);
        application.reason_codes.clear();
        application.updated_at = Utc::now();
        application.requested_by = requested_by;
        application = application.seal().map_err(|_| ApplicationError::Invalid)?;
        self.persist_patch_application(
            &loaded.project_root,
            &loaded.patch_set.patch_set_id,
            &application,
        )?;
        Ok(PatchRecoverV2Result {
            application,
            recovered: true,
        })
    }

    pub fn plan_patch_v1_to_v2_migration(
        &self,
        project_id: &ProjectId,
    ) -> Result<PatchV1ToV2MigrationPlan, ApplicationError> {
        let _guard = self.command_guard()?;
        self.plan_patch_v1_to_v2_migration_inner(project_id)
    }

    fn plan_patch_v1_to_v2_migration_inner(
        &self,
        project_id: &ProjectId,
    ) -> Result<PatchV1ToV2MigrationPlan, ApplicationError> {
        let project = self
            .repositories
            .global()
            .get_project(project_id)?
            .ok_or(ApplicationError::NotFound)?;
        let root = self.primary_project_root(&project)?;
        let repository = self.repositories.project(project_id)?;
        let discovery = self.artifacts.discover_verified(project_id, &root)?;
        if discovery.rejected_count > 0 {
            return Err(ApplicationError::Repository(RepositoryError::new(
                RepositoryErrorCategory::IntegrityFailed,
                "patch migration discovery rejected an artifact",
            )));
        }
        let mut patch_set_ids = discovery
            .verified
            .iter()
            .filter_map(|artifact| legacy_patch_set_id_from_artifact_path(&artifact.relative_path))
            .collect::<Vec<_>>();
        patch_set_ids.sort();
        patch_set_ids.dedup();
        let mut entries = Vec::new();
        for patch_set_id in patch_set_ids {
            let legacy = repository
                .get_patch_set(&patch_set_id)?
                .ok_or(ApplicationError::Invalid)?;
            let legacy_ref = DocumentRef {
                schema_id: "star.patch-set".to_owned(),
                document_id: patch_set_id.to_string(),
                revision: 1,
                sha256: application_document_hash(&legacy)?,
            };
            let (projected_patch_set_ref, limitations) = match self
                .read_patch_document::<PatchSetV2>(
                    project_id,
                    &root,
                    &format!(
                        "management/patches-v2/{}/patch-set-r1.json",
                        patch_set_id.as_str()
                    ),
                ) {
                Ok(projected) => {
                    let sealed = projected
                        .clone()
                        .seal()
                        .map_err(|_| ApplicationError::Invalid)?;
                    if sealed != projected
                        || projected.project_id != *project_id
                        || projected.patch_set_id != patch_set_id
                    {
                        return Err(ApplicationError::Invalid);
                    }
                    (
                        projected
                            .reference()
                            .map_err(|_| ApplicationError::Invalid)?,
                        vec![],
                    )
                }
                Err(ApplicationError::NotFound) => (
                    DocumentRef {
                        schema_id: star_contracts::patch_v2::PATCH_SET_V2_SCHEMA_ID.to_owned(),
                        document_id: patch_set_id.to_string(),
                        revision: 1,
                        sha256: versioned_fingerprint(
                            "star.patch-v1-unavailable-v2-projection",
                            1,
                            &legacy_ref,
                        )
                        .map_err(|_| ApplicationError::Invalid)?,
                    },
                    vec!["LEGACY_PATCH_LACKS_EXACT_FORWARD_REVERSE_ARTIFACTS".to_owned()],
                ),
                Err(error) => return Err(error),
            };
            entries.push(PatchV1ToV2MigrationEntry {
                legacy_patch_set_ref: legacy_ref,
                projected_patch_set_ref,
                limitations,
            });
        }
        if entries.is_empty() {
            return Err(ApplicationError::NotFound);
        }
        PatchV1ToV2MigrationPlan {
            schema_id: star_contracts::patch_v2::PATCH_V1_TO_V2_MIGRATION_PLAN_SCHEMA_ID.to_owned(),
            schema_version: 1,
            project_id: project_id.clone(),
            entries,
            dry_run: true,
            backup_required: true,
            rollback_supported: true,
            plan_fingerprint: Sha256Hash::digest(b""),
        }
        .seal()
        .map_err(|_| ApplicationError::Invalid)
    }

    pub fn apply_patch_v1_to_v2_migration(
        &self,
        plan: PatchV1ToV2MigrationPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<PatchV1ToV2MigrationResult, ApplicationError> {
        let _guard = self.command_guard()?;
        let sealed = plan.clone().seal().map_err(|_| ApplicationError::Invalid)?;
        if sealed != plan || plan.plan_fingerprint.as_str() != approved_plan_fingerprint {
            return Err(ApplicationError::Invalid);
        }
        let current = self.plan_patch_v1_to_v2_migration_inner(&plan.project_id)?;
        if current.plan_fingerprint != plan.plan_fingerprint {
            return Err(ApplicationError::Apply(
                "PATCH_MIGRATION_PLAN_STALE".to_owned(),
            ));
        }
        let project = self
            .repositories
            .global()
            .get_project(&plan.project_id)?
            .ok_or(ApplicationError::NotFound)?;
        let root = self.primary_project_root(&project)?;
        let fingerprint_key = plan.plan_fingerprint.as_str().trim_start_matches("sha256:");
        let result_path =
            format!("management/migrations/patch-v1-v2/{fingerprint_key}/applied.json");
        match self.read_patch_document::<PatchV1ToV2MigrationResult>(
            &plan.project_id,
            &root,
            &result_path,
        ) {
            Ok(result)
                if result.plan_fingerprint == plan.plan_fingerprint
                    && result.outcome == PatchMigrationOutcomeV1::Applied =>
            {
                return Ok(result);
            }
            Ok(_) => return Err(ApplicationError::Invalid),
            Err(ApplicationError::NotFound) => {}
            Err(error) => return Err(error),
        }
        let limitations = plan
            .entries
            .iter()
            .flat_map(|entry| entry.limitations.iter().cloned())
            .collect::<BTreeSet<_>>();
        if !limitations.is_empty() {
            return PatchV1ToV2MigrationResult {
                schema_id: star_contracts::patch_v2::PATCH_V1_TO_V2_MIGRATION_RESULT_SCHEMA_ID
                    .to_owned(),
                schema_version: 1,
                project_id: plan.project_id,
                plan_fingerprint: plan.plan_fingerprint,
                backup_manifest_ref: None,
                migrated_patch_set_refs: vec![],
                outcome: PatchMigrationOutcomeV1::Incompatible,
                reason_codes: limitations.into_iter().collect(),
                completed_at: Utc::now(),
                result_fingerprint: Sha256Hash::digest(b""),
            }
            .seal()
            .map_err(|_| ApplicationError::Invalid);
        }
        let repository = self.repositories.project(&plan.project_id)?;
        let mut legacy_snapshots = Vec::new();
        for entry in &plan.entries {
            let patch_set_id = PatchSetId::parse(entry.legacy_patch_set_ref.document_id.clone())
                .map_err(|_| ApplicationError::Invalid)?;
            let legacy = repository
                .get_patch_set(&patch_set_id)?
                .ok_or(ApplicationError::NotFound)?;
            if application_document_hash(&legacy)? != entry.legacy_patch_set_ref.sha256 {
                return Err(ApplicationError::Apply(
                    "PATCH_MIGRATION_PLAN_STALE".to_owned(),
                ));
            }
            legacy_snapshots.push(legacy);
        }
        let backup_manifest_ref = self.artifacts.put_json_with_policy(ArtifactWriteRequest {
            project_id: &plan.project_id,
            project_root: &root,
            relative_path: &format!(
                "management/migrations/patch-v1-v2/{fingerprint_key}/backup.json"
            ),
            subject_kind: "patch_v1_to_v2_backup",
            subject_id: fingerprint_key,
            policy: ArtifactWritePolicy {
                kind: ArtifactKind::Checkpoint,
                redaction_status: RedactionStatus::NotNeeded,
                retention_class: RetentionClass::Hold,
            },
            value: &serde_json::json!({
                "schema_id":"star.patch-v1-to-v2-backup",
                "schema_version":1,
                "plan_fingerprint":plan.plan_fingerprint,
                "legacy_patch_sets":legacy_snapshots,
                "created_at":Utc::now(),
            }),
        })?;
        self.persist_patch_document(
            &plan.project_id,
            &root,
            &format!("management/migrations/patch-v1-v2/{fingerprint_key}/activation.json"),
            "patch_v2_projection_activation",
            fingerprint_key,
            &serde_json::json!({
                "schema_id":"star.patch-v2-projection-activation",
                "schema_version":1,
                "plan_fingerprint":plan.plan_fingerprint,
                "projected_patch_set_refs":plan.entries.iter()
                    .map(|entry| &entry.projected_patch_set_ref)
                    .collect::<Vec<_>>(),
                "activated_at":Utc::now(),
            }),
        )?;
        let result = PatchV1ToV2MigrationResult {
            schema_id: star_contracts::patch_v2::PATCH_V1_TO_V2_MIGRATION_RESULT_SCHEMA_ID
                .to_owned(),
            schema_version: 1,
            project_id: plan.project_id.clone(),
            plan_fingerprint: plan.plan_fingerprint.clone(),
            backup_manifest_ref: Some(backup_manifest_ref),
            migrated_patch_set_refs: plan
                .entries
                .iter()
                .map(|entry| entry.projected_patch_set_ref.clone())
                .collect(),
            outcome: PatchMigrationOutcomeV1::Applied,
            reason_codes: vec![],
            completed_at: Utc::now(),
            result_fingerprint: Sha256Hash::digest(b""),
        }
        .seal()
        .map_err(|_| ApplicationError::Invalid)?;
        self.persist_patch_document(
            &plan.project_id,
            &root,
            &result_path,
            "patch_v1_to_v2_migration_result",
            fingerprint_key,
            &result,
        )?;
        Ok(result)
    }

    pub fn rollback_patch_v1_to_v2_migration(
        &self,
        plan: PatchV1ToV2MigrationPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<PatchV1ToV2MigrationResult, ApplicationError> {
        let _guard = self.command_guard()?;
        let sealed = plan.clone().seal().map_err(|_| ApplicationError::Invalid)?;
        if sealed != plan || plan.plan_fingerprint.as_str() != approved_plan_fingerprint {
            return Err(ApplicationError::Invalid);
        }
        let project = self
            .repositories
            .global()
            .get_project(&plan.project_id)?
            .ok_or(ApplicationError::NotFound)?;
        let root = self.primary_project_root(&project)?;
        let fingerprint_key = plan.plan_fingerprint.as_str().trim_start_matches("sha256:");
        let applied: PatchV1ToV2MigrationResult = self.read_patch_document(
            &plan.project_id,
            &root,
            &format!("management/migrations/patch-v1-v2/{fingerprint_key}/applied.json"),
        )?;
        if applied.plan_fingerprint != plan.plan_fingerprint
            || applied.outcome != PatchMigrationOutcomeV1::Applied
        {
            return Err(ApplicationError::Invalid);
        }
        let backup_ref = applied
            .backup_manifest_ref
            .clone()
            .ok_or(ApplicationError::Invalid)?;
        self.artifacts.verify(&root, &backup_ref)?;
        let backup = self.artifacts.read_json(&root, &backup_ref)?;
        if backup
            .get("plan_fingerprint")
            .and_then(serde_json::Value::as_str)
            != Some(plan.plan_fingerprint.as_str())
        {
            return Err(ApplicationError::Invalid);
        }
        let legacy_patch_sets = serde_json::from_value::<Vec<PatchSet>>(
            backup
                .get("legacy_patch_sets")
                .cloned()
                .ok_or(ApplicationError::Invalid)?,
        )
        .map_err(|_| ApplicationError::Invalid)?;
        let repository = self.repositories.project(&plan.project_id)?;
        for patch_set in &legacy_patch_sets {
            repository.save_patch_set(patch_set)?;
        }
        let result = PatchV1ToV2MigrationResult {
            schema_id: star_contracts::patch_v2::PATCH_V1_TO_V2_MIGRATION_RESULT_SCHEMA_ID
                .to_owned(),
            schema_version: 1,
            project_id: plan.project_id.clone(),
            plan_fingerprint: plan.plan_fingerprint.clone(),
            backup_manifest_ref: Some(backup_ref),
            migrated_patch_set_refs: vec![],
            outcome: PatchMigrationOutcomeV1::RolledBack,
            reason_codes: vec!["PATCH_V2_PROJECTION_ACTIVATION_ROLLED_BACK".to_owned()],
            completed_at: Utc::now(),
            result_fingerprint: Sha256Hash::digest(b""),
        }
        .seal()
        .map_err(|_| ApplicationError::Invalid)?;
        self.persist_patch_document(
            &plan.project_id,
            &root,
            &format!("management/migrations/patch-v1-v2/{fingerprint_key}/rollback.json"),
            "patch_v1_to_v2_migration_result",
            fingerprint_key,
            &result,
        )?;
        Ok(result)
    }

    fn read_patch_bytes(
        &self,
        project_root: &Path,
        artifact: &ArtifactRef,
        expected_direction: &str,
        expected_path: &ProjectPathRef,
    ) -> Result<Vec<u8>, ApplicationError> {
        self.artifacts.verify(project_root, artifact)?;
        let value = self.artifacts.read_json(project_root, artifact)?;
        if value.get("schema_id").and_then(serde_json::Value::as_str)
            != Some("star.patch-operation-bytes")
            || value
                .get("schema_version")
                .and_then(serde_json::Value::as_u64)
                != Some(1)
            || value.get("direction").and_then(serde_json::Value::as_str)
                != Some(expected_direction)
            || serde_json::from_value::<ProjectPathRef>(
                value
                    .get("path")
                    .cloned()
                    .ok_or(ApplicationError::Invalid)?,
            )
            .map_err(|_| ApplicationError::Invalid)?
                != *expected_path
            || value.get("encoding").and_then(serde_json::Value::as_str) != Some("hex")
        {
            return Err(ApplicationError::Invalid);
        }
        let bytes = hex_decode(
            value
                .get("bytes")
                .and_then(serde_json::Value::as_str)
                .ok_or(ApplicationError::Invalid)?,
        )?;
        let content_sha256 = serde_json::from_value::<Sha256Hash>(
            value
                .get("content_sha256")
                .cloned()
                .ok_or(ApplicationError::Invalid)?,
        )
        .map_err(|_| ApplicationError::Invalid)?;
        if Sha256Hash::digest(&bytes) != content_sha256 || patch_source_bytes_are_sensitive(&bytes)
        {
            return Err(ApplicationError::Invalid);
        }
        Ok(bytes)
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
            patch_application: None,
        })
    }

    pub fn verify_stores(&self) -> Result<Vec<ManagementStoreStatus>, ApplicationError> {
        let _guard = self.command_guard()?;
        Ok(self.repositories.verify_all()?)
    }

    pub fn plan_backup(&self, destination: &Path) -> Result<BackupPlan, ApplicationError> {
        let _guard = self.command_guard()?;
        Ok(self.repositories.plan_backup(destination)?)
    }

    pub fn apply_backup(
        &self,
        destination: &Path,
        plan: &BackupPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<BackupApplyResult, ApplicationError> {
        let _guard = self.command_guard()?;
        Ok(self
            .repositories
            .apply_backup(destination, plan, approved_plan_fingerprint)?)
    }

    pub fn plan_local_state_export(
        &self,
        project_id: &ProjectId,
        destination: &Path,
    ) -> Result<LocalStateExportPlan, ApplicationError> {
        let _guard = self.command_guard()?;
        Ok(self
            .repositories
            .plan_local_state_export(project_id, destination)?)
    }

    pub fn apply_local_state_export(
        &self,
        destination: &Path,
        plan: &LocalStateExportPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<LocalStateExportResult, ApplicationError> {
        let _guard = self.command_guard()?;
        Ok(self.repositories.apply_local_state_export(
            destination,
            plan,
            approved_plan_fingerprint,
        )?)
    }

    pub fn plan_local_state_import(
        &self,
        source: &Path,
    ) -> Result<LocalStateImportPlan, ApplicationError> {
        let _guard = self.command_guard()?;
        Ok(self.repositories.plan_local_state_import(source)?)
    }

    pub fn apply_local_state_import(
        &self,
        source: &Path,
        plan: &LocalStateImportPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<LocalStateImportResult, ApplicationError> {
        let _guard = self.command_guard()?;
        Ok(self
            .repositories
            .apply_local_state_import(source, plan, approved_plan_fingerprint)?)
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

impl<'a> ManagementRecoveryApplicationService<'a> {
    pub fn new(
        recovery: &'a dyn ManagementRecovery,
        root_bindings: Arc<dyn ProjectRootBindingStore>,
        artifacts: Arc<dyn ArtifactStore>,
    ) -> Self {
        Self {
            recovery,
            root_bindings,
            artifacts,
            scan_policy: ScanPolicy::default(),
            index_policy: IndexPolicy::default(),
            index_cache: None,
            syntax_adapters: Vec::new(),
            semantic_adapters: Vec::new(),
            command_lock: Mutex::new(()),
        }
    }

    pub fn with_index_cache(mut self, cache: Arc<dyn CodeIndexCache>) -> Self {
        self.index_cache = Some(cache);
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

    pub fn plan_source_rebuild(&self) -> Result<RebuildPlan, ApplicationError> {
        let _guard = self.command_lock.lock().map_err(|_| {
            ApplicationError::Repository(RepositoryError::new(
                RepositoryErrorCategory::Unavailable,
                "management recovery application lock is unavailable",
            ))
        })?;
        let projects = self.rebuild_inputs()?;
        let predicted_losses = predicted_rebuild_losses(&projects);
        Ok(self.recovery.plan_rebuild(projects, predicted_losses)?)
    }

    pub fn apply_source_rebuild(
        &self,
        plan: &RebuildPlan,
        approved_plan_fingerprint: &str,
    ) -> Result<RebuildApplyResult, ApplicationError> {
        let _guard = self.command_lock.lock().map_err(|_| {
            ApplicationError::Repository(RepositoryError::new(
                RepositoryErrorCategory::Unavailable,
                "management recovery application lock is unavailable",
            ))
        })?;
        if let Some(completed) = self
            .recovery
            .completed_rebuild(plan, approved_plan_fingerprint)?
        {
            return Ok(completed);
        }
        if self.rebuild_inputs()? != plan.projects {
            return Err(ApplicationError::Repository(RepositoryError::new(
                RepositoryErrorCategory::RevisionConflict,
                "source-derived rebuild inputs changed after planning",
            )));
        }
        let repositories = self
            .recovery
            .begin_rebuild(plan, approved_plan_fingerprint)
            .map_err(|error| {
                ApplicationError::Repository(RepositoryError::new(
                    error.category,
                    "source rebuild candidate repository could not be created",
                ))
            })?;
        let mut candidate = ManagementApplicationService::new(
            repositories,
            Arc::clone(&self.root_bindings),
            Arc::clone(&self.artifacts),
        );
        candidate.scan_policy = self.scan_policy.clone();
        candidate.index_policy = self.index_policy.clone();
        candidate.index_cache = self.index_cache.clone();
        candidate.syntax_adapters.clone_from(&self.syntax_adapters);
        candidate
            .semantic_adapters
            .clone_from(&self.semantic_adapters);

        let mut rebuilt_projects = Vec::with_capacity(plan.projects.len());
        for input in &plan.projects {
            let attachment = self
                .root_bindings
                .find_by_project(&input.project_id)?
                .ok_or(ApplicationError::NotFound)?;
            if attachment.checkout_id != input.checkout_id
                || attachment.root_binding_id != input.root_binding_id
            {
                return Err(ApplicationError::Repository(RepositoryError::new(
                    RepositoryErrorCategory::RevisionConflict,
                    "protected root binding changed after rebuild planning",
                )));
            }
            let root = self.root_bindings.resolve(&input.root_binding_id)?;
            let artifact_inventory = self.artifacts.discover_verified(&input.project_id, &root)?;
            let verified_artifact_count = u64::try_from(artifact_inventory.verified.len())
                .map_err(|_| {
                    ApplicationError::Repository(RepositoryError::new(
                        RepositoryErrorCategory::QuotaExceeded,
                        "verified artifact count exceeds its supported range",
                    ))
                })?;
            if verified_artifact_count != input.verified_artifact_count
                || artifact_inventory.rejected_count != input.rejected_artifact_count
                || recovery_artifact_inventory_fingerprint(&artifact_inventory)?
                    != input.artifact_inventory_fingerprint
            {
                return Err(ApplicationError::Repository(RepositoryError::new(
                    RepositoryErrorCategory::RevisionConflict,
                    "verified artifact inventory changed after rebuild planning",
                )));
            }
            let registration = candidate.register_project(
                &root,
                &format!(
                    "recovery-rebuild-register-{}-{}",
                    plan.recovery_plan_id.as_str(),
                    input.project_id.as_str()
                ),
            )?;
            if registration.project.project_id != input.project_id
                || registration.checkout.checkout_id != input.checkout_id
            {
                return Err(ApplicationError::Invalid);
            }
            let scan = candidate
                .scan_project(
                    &input.project_id,
                    &format!(
                        "recovery-rebuild-scan-{}-{}",
                        plan.recovery_plan_id.as_str(),
                        input.project_id.as_str()
                    ),
                )
                .map_err(|error| {
                    recovery_application_stage(error, "source rebuild candidate scan failed")
                })?;
            if scan.scan_run.status != ScanStatus::Succeeded
                || scan.scan_run.project_revision_id != input.source_revision_id
                || scan.scan_run.effective_config_fingerprint != input.effective_config_fingerprint
            {
                return Err(ApplicationError::Repository(RepositoryError::new(
                    RepositoryErrorCategory::IntegrityFailed,
                    "source rebuild scan did not reproduce the planned source identity",
                )));
            }
            candidate
                .repositories
                .project(&input.project_id)?
                .reindex_artifact_refs(&artifact_inventory.verified)?;
            rebuilt_projects.push(RebuiltProjectSummary {
                project_id: input.project_id.clone(),
                project_revision_id: scan.scan_run.project_revision_id,
                workspace_snapshot_id: scan.scan_run.workspace_snapshot_id,
                scan_run_id: scan.scan_run.scan_run_id,
                canonical_source_count: scan.scan_run.counts.get("source").copied().unwrap_or(0),
                symbol_count: scan.scan_run.counts.get("symbol").copied().unwrap_or(0),
                finding_count: scan.finding_count as u64,
                reindexed_artifact_count: verified_artifact_count,
                rejected_artifact_count: artifact_inventory.rejected_count,
            });
        }
        let _ = candidate.verify_stores().map_err(|error| {
            recovery_application_stage(
                error,
                "source rebuild candidate application verification failed",
            )
        })?;
        drop(candidate);
        Ok(self
            .recovery
            .apply_rebuild(plan, approved_plan_fingerprint, rebuilt_projects)?)
    }

    fn rebuild_inputs(&self) -> Result<Vec<RebuildProjectInput>, ApplicationError> {
        let mut attachments = self.root_bindings.list_attachments()?;
        attachments.sort_by(|left, right| left.project_id.cmp(&right.project_id));
        let mut projects = Vec::with_capacity(attachments.len());
        for attachment in attachments {
            let root = self.root_bindings.resolve(&attachment.root_binding_id)?;
            let seed = ProjectSeed::discover_with_local_project_id(
                &root,
                Some(attachment.project_id.clone()),
            )?;
            let attached = seed.attach(
                attachment.checkout_id.clone(),
                attachment.root_binding_id.clone(),
                &root,
            )?;
            if attached.project.project_id != attachment.project_id
                || attached.checkout.checkout_id != attachment.checkout_id
            {
                return Err(ApplicationError::Invalid);
            }
            let observation = observe_project(&attached.project, &root, &self.scan_policy)?;
            let shared_decisions = load_shared_decisions(&attached.project, &root)?;
            let artifact_inventory = self
                .artifacts
                .discover_verified(&attachment.project_id, &root)?;
            projects.push(RebuildProjectInput {
                project_id: attachment.project_id,
                checkout_id: attachment.checkout_id,
                root_binding_id: attachment.root_binding_id,
                source_revision_id: observation.revision.project_revision_id.clone(),
                effective_config_fingerprint: rebuild_effective_config_fingerprint(
                    &observation,
                    &shared_decisions,
                )?,
                artifact_inventory_fingerprint: recovery_artifact_inventory_fingerprint(
                    &artifact_inventory,
                )?,
                verified_artifact_count: u64::try_from(artifact_inventory.verified.len()).map_err(
                    |_| {
                        ApplicationError::Repository(RepositoryError::new(
                            RepositoryErrorCategory::QuotaExceeded,
                            "verified artifact count exceeds its supported range",
                        ))
                    },
                )?,
                rejected_artifact_count: artifact_inventory.rejected_count,
            });
        }
        Ok(projects)
    }
}

fn recovery_application_stage(error: ApplicationError, message: &'static str) -> ApplicationError {
    match error {
        ApplicationError::Repository(error) => {
            ApplicationError::Repository(RepositoryError::new(error.category, message))
        }
        error => error,
    }
}

fn rebuild_effective_config_fingerprint(
    observation: &star_project::ProjectObservation,
    shared_decisions: &SharedDecisionDeclarations,
) -> Result<Sha256Hash, ApplicationError> {
    let decision_set_fingerprint = versioned_fingerprint(
        "star.scan-decision-inputs",
        1,
        &serde_json::json!({
            "baselines":shared_decisions.baselines,
            "suppressions":shared_decisions.suppressions,
            "dispositions":Vec::<Disposition>::new(),
            "shared_source_fingerprint":shared_decisions.source_fingerprint,
        }),
    )
    .map_err(|_| ApplicationError::Invalid)?;
    versioned_fingerprint(
        "star.effective-config",
        1,
        &serde_json::json!({
            "scan_config_fingerprint":observation.scan_config_fingerprint,
            "require_complete_for_gate":true,
            "suppression_default_expiry_days":90,
            "decision_set_fingerprint":decision_set_fingerprint,
        }),
    )
    .map_err(|_| ApplicationError::Invalid)
}

fn recovery_artifact_inventory_fingerprint(
    inventory: &ArtifactDiscovery,
) -> Result<Sha256Hash, ApplicationError> {
    versioned_fingerprint(
        "star.recovery-artifact-inventory",
        1,
        &serde_json::json!({
            "verified":inventory.verified,
            "rejected_count":inventory.rejected_count,
        }),
    )
    .map_err(|_| ApplicationError::Invalid)
}

fn predicted_rebuild_losses(projects: &[RebuildProjectInput]) -> Vec<RecoveryLossItem> {
    let mut losses = Vec::with_capacity(projects.len() * 8);
    for project in projects {
        for kind in [
            RecoveryLossKind::LocalSuppression,
            RecoveryLossKind::LocalBaseline,
            RecoveryLossKind::LocalDisposition,
            RecoveryLossKind::ActiveChangePlan,
            RecoveryLossKind::IdempotencyHistory,
            RecoveryLossKind::ActorHistory,
            RecoveryLossKind::EventTimestamp,
        ] {
            let reason_code = match kind {
                RecoveryLossKind::LocalSuppression
                | RecoveryLossKind::LocalBaseline
                | RecoveryLossKind::LocalDisposition
                | RecoveryLossKind::ActiveChangePlan => "LOCAL_STATE_NOT_SOURCE_DERIVED",
                RecoveryLossKind::IdempotencyHistory
                | RecoveryLossKind::ActorHistory
                | RecoveryLossKind::EventTimestamp => "HISTORICAL_EVENT_NOT_RECREATED",
                RecoveryLossKind::ArtifactReference => "ARTIFACT_REF_VERIFICATION_FAILED",
            };
            losses.push(RecoveryLossItem {
                project_id: Some(project.project_id.clone()),
                kind,
                state: RecoveryLossState::Lost,
                count: None,
                reason_code: reason_code.to_owned(),
            });
        }
        losses.push(RecoveryLossItem {
            project_id: Some(project.project_id.clone()),
            kind: RecoveryLossKind::ArtifactReference,
            state: if project.rejected_artifact_count == 0 {
                RecoveryLossState::Preserved
            } else {
                RecoveryLossState::Lost
            },
            count: Some(if project.rejected_artifact_count == 0 {
                project.verified_artifact_count
            } else {
                project.rejected_artifact_count
            }),
            reason_code: if project.rejected_artifact_count == 0 {
                "ARTIFACT_REF_HASH_VERIFIED_FOR_REINDEX"
            } else {
                "ARTIFACT_REF_VERIFICATION_FAILED"
            }
            .to_owned(),
        });
    }
    losses
}

fn valid_idempotency_key(value: &str) -> bool {
    !value.trim().is_empty() && value.chars().count() <= 128 && !value.contains('\0')
}

fn toolchain_check_family(command_id: &str) -> Option<&'static str> {
    let command = command_id
        .strip_prefix("script:")
        .unwrap_or(command_id)
        .to_ascii_lowercase();
    if command.contains("format") || command.contains("fmt") || command.contains("prettier") {
        Some("format")
    } else if command.contains("lint") || command.contains("clippy") {
        Some("lint")
    } else if command.contains("test") {
        Some("test")
    } else if command.contains("build") || command.contains("check") || command.contains("compile")
    {
        Some("build")
    } else if command.contains("doc") {
        Some("docs")
    } else if command.contains("migrat") {
        Some("migration")
    } else if command.contains("generat") || command.contains("codegen") {
        Some("generation")
    } else {
        None
    }
}

fn toolchain_check_source_classes(family: &str) -> Vec<SourceClass> {
    match family {
        "format" | "lint" | "build" => vec![
            SourceClass::Source,
            SourceClass::Test,
            SourceClass::Config,
            SourceClass::Generated,
        ],
        "test" => vec![
            SourceClass::Source,
            SourceClass::Test,
            SourceClass::Config,
            SourceClass::Schema,
            SourceClass::Migration,
        ],
        "docs" => vec![SourceClass::Docs, SourceClass::Source],
        "migration" => vec![SourceClass::Migration, SourceClass::Schema],
        "generation" => vec![SourceClass::Generated, SourceClass::Schema],
        _ => vec![SourceClass::Unknown],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PatchPreviewValidationPhase {
    PreApply,
    PostApply,
}

fn patch_preview_task(
    project_id: &ProjectId,
    checkout_id: &CheckoutId,
    recipe: &ChangeRecipeV2,
    files: &[MaterializedPatchFile],
    target_selector: Option<&TargetSelector>,
    phase: PatchPreviewValidationPhase,
) -> TaskSpecDraft {
    let managed_selectors = match target_selector {
        Some(TargetSelector::ManagedDeclaration {
            declaration_ids, ..
        }) => declaration_ids
            .iter()
            .map(|declaration_id| PlanningSelector {
                kind: SelectorKind::ManagedDeclaration,
                value: declaration_id.clone(),
            })
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    let path_selectors = files
        .iter()
        .map(|file| PlanningSelector {
            kind: SelectorKind::Path,
            value: file.path.as_str().to_owned(),
        })
        .collect::<Vec<_>>();
    let mut included_scope = managed_selectors.clone();
    included_scope.extend(path_selectors.clone());
    let intended_selectors = if managed_selectors.is_empty() {
        path_selectors
    } else {
        managed_selectors
    };
    TaskSpecDraft {
        title: format!("Preview {}", recipe.display_name),
        objective: format!(
            "Apply {}@{} to the exact selected paths and validate the observed preview",
            recipe.recipe_id, recipe.recipe_version
        ),
        project_targets: vec![ProjectTarget {
            project_id: project_id.clone(),
            checkout_id: checkout_id.clone(),
            role: ProjectTargetRole::PlannedChange,
            reason: "M4 recipe target".to_owned(),
        }],
        included_scope,
        excluded_scope: vec![],
        intended_changes: intended_selectors
            .into_iter()
            .enumerate()
            .map(|(index, selector)| IntendedChange {
                change_id: format!("recipe-operation-{index:04}"),
                selector,
                change_kind: IntendedChangeKind::Modify,
                intended_postcondition: recipe.intended_postconditions.join(";"),
            })
            .collect(),
        success_criteria: vec![SuccessCriterion {
            criterion_id: "preview-and-post-gate-current".to_owned(),
            description: "preview is exact, idempotent, and all affected checks pass after apply"
                .to_owned(),
            verification: "M3 patch_pre_apply and patch_post_apply EvidenceBundle".to_owned(),
            required: true,
        }],
        constraints: vec![
            "single_project".to_owned(),
            "exact_before_hash".to_owned(),
            "no_live_mutation_during_prepare".to_owned(),
        ],
        forbidden_actions: vec![
            "cross_project_write".to_owned(),
            "remote_write".to_owned(),
            "raw_global_literal_replace".to_owned(),
        ],
        profile_ids: if recipe.recipe_id == "rust_style_v1" {
            vec!["rust_style_auto_fix".to_owned()]
        } else {
            vec![]
        },
        baseline_policy: BaselinePolicy {
            kind: BaselinePolicyKind::CurrentWorkspace,
            reference: None,
        },
        requested_checks: if recipe.recipe_id == "rust_style_v1" {
            let mut families = recipe.validation_families.clone();
            families.extend([
                "architecture".to_owned(),
                "hardcoding".to_owned(),
                "security".to_owned(),
            ]);
            families.sort();
            families.dedup();
            families
        } else if phase == PatchPreviewValidationPhase::PreApply {
            vec![
                "architecture".to_owned(),
                "hardcoding".to_owned(),
                "security".to_owned(),
            ]
        } else {
            recipe.validation_families.clone()
        },
        check_overrides: vec![],
        assumptions: vec![],
    }
}

fn rust_style_gate_check_descriptors(
    project_id: &ProjectId,
) -> Result<Vec<CheckDescriptor>, ApplicationError> {
    [
        (
            "architecture",
            "star.rust-style.architecture",
            vec!["check", "--locked"],
        ),
        (
            "hardcoding",
            "star.rust-style.hardcoding",
            vec!["clippy", "--locked"],
        ),
        (
            "security",
            "star.rust-style.security",
            vec!["clippy", "--locked"],
        ),
    ]
    .into_iter()
    .map(|(family, tool_id, args)| {
        let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
        let content_fingerprint = versioned_fingerprint(
            "star.rust-style-gate-check-descriptor",
            1,
            &serde_json::json!({
                "project_id":project_id,
                "family":family,
                "tool_id":tool_id,
                "logical_executable":"cargo",
                "args":args,
                "role":"m11_candidate_static_analysis",
            }),
        )
        .map_err(|_| ApplicationError::Invalid)?;
        let short = &content_fingerprint.as_str()[7..23];
        Ok(CheckDescriptor {
            check_id: format!("star.rust-style.{family}.{short}"),
            family: family.to_owned(),
            project_ids: vec![project_id.clone()],
            tool_id: tool_id.to_owned(),
            logical_executable: "cargo".to_owned(),
            argument_template: args,
            supported_scope_levels: vec![
                star_contracts::planning::ValidationScopeLevel::Package,
                star_contracts::planning::ValidationScopeLevel::Workspace,
                star_contracts::planning::ValidationScopeLevel::ProjectFull,
            ],
            applicable_source_classes: vec![
                SourceClass::Source,
                SourceClass::Test,
                SourceClass::Config,
            ],
            trusted: true,
            available: logical_executable_available("cargo"),
            required_evidence: vec![
                "validation_result".to_owned(),
                "observed_tool_identity".to_owned(),
                "rust_style_candidate_binding".to_owned(),
            ],
            content_fingerprint,
        })
    })
    .collect()
}

fn patch_source_bytes_are_sensitive(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes)
        .map(m3_contains_secret_candidate)
        .unwrap_or(true)
}

fn observe_project_path_sha256(
    project_root: &Path,
    path: &ProjectPathRef,
) -> Result<Option<Sha256Hash>, ApplicationError> {
    let canonical_root = std::fs::canonicalize(project_root)
        .map_err(|_| ApplicationError::Apply("PATCH_PROJECT_ROOT_UNAVAILABLE".to_owned()))?;
    let candidate = project_root.join(path.as_str());
    let metadata = match std::fs::symlink_metadata(&candidate) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => {
            return Err(ApplicationError::Apply(
                "PATCH_PATH_OBSERVATION_FAILED".to_owned(),
            ));
        }
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(ApplicationError::Apply(
            "PATCH_PATH_OBSERVATION_UNSAFE".to_owned(),
        ));
    }
    let canonical_candidate = std::fs::canonicalize(&candidate)
        .map_err(|_| ApplicationError::Apply("PATCH_PATH_OBSERVATION_FAILED".to_owned()))?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(ApplicationError::Apply(
            "PATCH_PATH_OBSERVATION_UNSAFE".to_owned(),
        ));
    }
    let bytes = std::fs::read(canonical_candidate)
        .map_err(|_| ApplicationError::Apply("PATCH_PATH_OBSERVATION_FAILED".to_owned()))?;
    Ok(Some(Sha256Hash::digest(&bytes)))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    encoded
}

fn hex_decode(value: &str) -> Result<Vec<u8>, ApplicationError> {
    if !value.len().is_multiple_of(2) || value.len() > 32 * 1024 * 1024 {
        return Err(ApplicationError::Invalid);
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_nibble(pair[0]).ok_or(ApplicationError::Invalid)?;
            let low = hex_nibble(pair[1]).ok_or(ApplicationError::Invalid)?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn legacy_patch_set_id_from_artifact_path(relative_path: &str) -> Option<PatchSetId> {
    let parts = relative_path.split('/').collect::<Vec<_>>();
    parts.windows(4).find_map(|window| {
        (window[0] == "management" && window[1] == "patches" && window[3] == "recipe.json")
            .then(|| PatchSetId::parse(window[2].to_owned()).ok())
            .flatten()
    })
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn logical_executable_available(executable: &str) -> bool {
    resolve_logical_executable_path(executable).is_some()
}

const M3_RULE_TEXT_LIMIT: u64 = 1024 * 1024;

fn collect_m3_rule_diagnostics(
    bundle: &PlanningBundle,
    project_root: &Path,
    validator_guard: Option<&VerifiedValidatorGuardInput<'_>>,
    registry_snapshot: Option<&ManagedRegistrySnapshot>,
    registry_records: &[RegistryConsistencyRecord],
) -> Result<Vec<RuleDiagnosticInputV2>, ApplicationError> {
    let patch_pre_apply = bundle.validation_plan.phase == "patch_pre_apply";
    let families = bundle
        .validation_plan
        .required_checks
        .iter()
        .map(|check| check.family.as_str())
        .collect::<BTreeSet<_>>();
    let mut facts = Vec::new();
    let mut protected_before = Vec::new();
    let mut protected_after = Vec::new();
    let mut protected_changed = false;
    let mut has_source_change = false;
    let mut has_test_change = false;
    let mut has_docs_change = false;
    let mut has_config_change = false;
    let mut has_contract_change = false;
    let mut has_environment_change = false;
    let mut has_managed_registry_change = bundle
        .task_spec
        .included_scope
        .iter()
        .chain(
            bundle
                .task_spec
                .intended_changes
                .iter()
                .map(|change| &change.selector),
        )
        .any(|selector| selector.kind == SelectorKind::ManagedDeclaration);
    let mut seen = BTreeSet::new();
    for change_set in &bundle.change_sets {
        if change_set.collection_state != CollectionState::Complete {
            facts.push(RuleFactV2::ActualChangeCollectionIncomplete);
        }
        for entry in &change_set.entries {
            let entry_key = (
                change_set.project_id.clone(),
                entry.path.clone(),
                entry.before_sha256.clone(),
                entry.after_sha256.clone(),
            );
            if !seen.insert(entry_key) {
                continue;
            }
            match entry.scope_relation {
                ScopeRelation::Unrelated => facts.push(RuleFactV2::ActualChangeUnrelated {
                    path: entry.path.clone(),
                }),
                ScopeRelation::Unknown => facts.push(RuleFactV2::ActualChangeScopeUnknown {
                    path: entry.path.clone(),
                }),
                ScopeRelation::Planned | ScopeRelation::NecessaryExpansion => {}
            }
            let path = entry.path.as_str();
            has_managed_registry_change |= path.starts_with(".star-control/registry/");
            let protected = m3_validation_protected_path(path);
            if protected {
                protected_changed = true;
                protected_before.push((path.to_owned(), entry.before_sha256.clone()));
                protected_after.push((path.to_owned(), entry.after_sha256.clone()));
            }
            has_source_change |= entry.source_class == SourceClass::Source;
            has_test_change |= entry.source_class == SourceClass::Test;
            has_docs_change |= entry.source_class == SourceClass::Docs;
            has_config_change |= entry.source_class == SourceClass::Config;
            has_contract_change |= entry.source_class == SourceClass::Schema
                || path.starts_with("crates/foundation/star-contracts/")
                || path.starts_with("docs/contracts/");
            has_environment_change |= m3_environment_path(path);
            if entry.source_class == SourceClass::Test
                && entry.change_kind == ObservedChangeKind::Delete
            {
                facts.push(RuleFactV2::RequiredTestDeleted {
                    path: entry.path.clone(),
                });
            }
            if entry.source_class == SourceClass::Generated && !families.contains("generation") {
                facts.push(RuleFactV2::GeneratedOutputDrift {
                    path: entry.path.clone(),
                });
            }
            if m3_dependency_path(path)
                && (!families.contains("dependency") || !families.contains("security"))
            {
                facts.push(RuleFactV2::DependencyEvidenceMissing {
                    path: entry.path.clone(),
                });
            }
            let after = if entry.binary || entry.change_kind == ObservedChangeKind::Delete {
                None
            } else {
                m3_current_text(project_root, &entry.path, entry.after_sha256.as_ref())
            };
            let before = if entry.binary || entry.change_kind == ObservedChangeKind::Add {
                None
            } else {
                m3_git_head_text(project_root, &entry.path, entry.before_sha256.as_ref())
            };
            if entry.source_class == SourceClass::Test
                && let (Some(before), Some(after)) = (before.as_deref(), after.as_deref())
            {
                if m3_marker_count(after, &["assert", "expect(", "should("])
                    < m3_marker_count(before, &["assert", "expect(", "should("])
                {
                    facts.push(RuleFactV2::AssertionCountDecreased {
                        path: entry.path.clone(),
                    });
                }
                if m3_marker_count(
                    after,
                    &["#[ignore]", ".skip(", "test.skip", "pytest.mark.skip"],
                ) > m3_marker_count(
                    before,
                    &["#[ignore]", ".skip(", "test.skip", "pytest.mark.skip"],
                ) {
                    facts.push(RuleFactV2::TestExecutionBypassAdded {
                        path: entry.path.clone(),
                    });
                }
                if m3_marker_count(after, &["test.only", "describe.only", "fdescribe(", "fit("])
                    > m3_marker_count(
                        before,
                        &["test.only", "describe.only", "fdescribe(", "fit("],
                    )
                {
                    facts.push(RuleFactV2::FocusedTestOnlyAdded {
                        path: entry.path.clone(),
                    });
                }
                if m3_marker_count(after, &["retry", "timeout"])
                    > m3_marker_count(before, &["retry", "timeout"])
                {
                    facts.push(RuleFactV2::RetryOrTimeoutIncreased {
                        path: entry.path.clone(),
                    });
                }
            }
            if !matches!(entry.source_class, SourceClass::Docs | SourceClass::Test)
                && after.as_deref().is_some_and(m3_contains_secret_candidate)
            {
                facts.push(RuleFactV2::SecretCandidate {
                    path: entry.path.clone(),
                });
            }
            if !matches!(entry.source_class, SourceClass::Docs | SourceClass::Test)
                && after.as_deref().is_some_and(m3_contains_dangerous_command)
            {
                facts.push(RuleFactV2::DangerousCommandCandidate {
                    path: entry.path.clone(),
                });
            }
        }
    }
    if has_managed_registry_change {
        const REQUIRED_REGISTRY_FAMILIES: [&str; 4] = [
            "managed_registry_contract",
            "consumer_compatibility",
            "generated_consistency",
            "docs_contract_drift",
        ];
        for family in REQUIRED_REGISTRY_FAMILIES {
            if !families.contains(family) {
                facts.push(RuleFactV2::ManagedRegistryValidationMissing {
                    family: family.to_owned(),
                });
            }
        }
        match registry_snapshot {
            None => facts.push(RuleFactV2::ManagedRegistrySnapshotMissing),
            Some(snapshot) => {
                let pinned = bundle
                    .scope_revision
                    .source_snapshot_refs
                    .iter()
                    .any(|source| {
                        source.project_id == snapshot.owner_project_id
                            && source.checkout_id == snapshot.checkout_id
                            && source.project_revision_id == snapshot.project_revision_id
                            && source.workspace_snapshot_id == snapshot.workspace_snapshot_id
                            && snapshot.code_index_snapshot_id.as_ref()
                                == Some(&source.code_index_snapshot_id)
                    });
                if !pinned
                    || snapshot.freshness
                        != star_contracts::managed_registry::RegistryFreshness::Current
                    || snapshot.completeness
                        != star_contracts::managed_registry::EvidenceCompleteness::Complete
                    || snapshot.resolution_state
                        != star_contracts::managed_registry::RegistryResolutionState::Valid
                {
                    facts.push(RuleFactV2::ManagedRegistrySnapshotStale);
                }
                for record in registry_records.iter().filter(|record| {
                    record.status
                        != star_contracts::managed_registry::RegistryConsistencyStatus::Current
                        || record.completeness
                            != star_contracts::managed_registry::EvidenceCompleteness::Complete
                }) {
                    facts.push(RuleFactV2::ManagedRegistryConsistencyDrift {
                        subject: record.subject.clone(),
                    });
                }
            }
        }
    }
    if !patch_pre_apply && (has_source_change || has_test_change) && !families.contains("test") {
        facts.push(RuleFactV2::RelatedTestCheckMissing);
    }
    if (has_source_change || has_contract_change) && !families.contains("architecture") {
        facts.push(RuleFactV2::ArchitectureCheckMissing);
    }
    if has_source_change && !families.contains("hardcoding") {
        facts.push(RuleFactV2::HardcodingCheckMissing);
    }
    if !patch_pre_apply && has_contract_change && !families.contains("contract") {
        facts.push(RuleFactV2::ContractCheckMissing);
    }
    if !patch_pre_apply && has_docs_change && !families.contains("docs") {
        facts.push(RuleFactV2::DocsCheckMissing);
    }
    if !patch_pre_apply && has_config_change && !families.contains("config") {
        facts.push(RuleFactV2::ConfigCheckMissing);
    }
    if !patch_pre_apply && has_contract_change && !has_docs_change {
        facts.push(RuleFactV2::DocumentationDrift);
    }
    if !patch_pre_apply && has_environment_change && !families.contains("project_full") {
        facts.push(RuleFactV2::EnvironmentEvidenceMissing);
    }
    if !patch_pre_apply && m3_task_requests_bug_fix(&bundle.task_spec) {
        if !has_test_change {
            facts.push(RuleFactV2::RegressionEvidenceMissing);
        }
        if !families.contains("regression") {
            facts.push(RuleFactV2::RegressionCheckMissing);
        }
    }
    if protected_changed {
        facts.push(RuleFactV2::ProtectedValidationSurfaceChanged);
        protected_before.sort();
        protected_after.sort();
        let previous_snapshot_fingerprint = canonical_sha256(&serde_json::json!({
            "domain":"star.validator-protected-snapshot",
            "version":2,
            "state":"previous",
            "entries":protected_before,
        }))
        .ok();
        let current_snapshot_fingerprint = canonical_sha256(&serde_json::json!({
            "domain":"star.validator-protected-snapshot",
            "version":2,
            "state":"current",
            "entries":protected_after,
        }))
        .ok();
        let accepted_guard = validator_guard.filter(|guard| {
            guard.artifacts_verified
                && guard.evidence.candidate_registry_fingerprint
                    == *guard.expected_candidate_registry_fingerprint
                && Some(&guard.evidence.previous_snapshot_fingerprint)
                    == previous_snapshot_fingerprint.as_ref()
                && Some(&guard.evidence.current_snapshot_fingerprint)
                    == current_snapshot_fingerprint.as_ref()
        });
        let fixtures = accepted_guard
            .map(|guard| {
                guard
                    .evidence
                    .fixture_results
                    .iter()
                    .map(|fixture| RuleFixtureResultV2 {
                        fixture_kind: match fixture.fixture_kind {
                            GuardFixtureKindV2::Positive => "positive",
                            GuardFixtureKindV2::Negative => "negative",
                            GuardFixtureKindV2::Edge => "edge",
                            GuardFixtureKindV2::Regression => "regression",
                            GuardFixtureKindV2::Adversarial => "adversarial",
                        }
                        .to_owned(),
                        previous_snapshot_passed: fixture.previous_snapshot_passed(),
                        current_snapshot_passed: fixture.current_snapshot_passed(),
                        result_fingerprint: fixture.result_fingerprint.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        facts.extend(evaluate_two_snapshot_guard(&TwoSnapshotGuardInputV2 {
            protected_surface_changed: true,
            previous_snapshot_fingerprint,
            current_snapshot_fingerprint,
            behavior_weakened: accepted_guard
                .is_some_and(|guard| guard.evidence.behavior_weakened()),
            independent_previous_executor: accepted_guard
                .is_some_and(|guard| guard.evidence.independent_previous_executor()),
            fixtures,
        }));
    }
    Ok(evaluate_rule_facts(&facts))
}

fn m3_current_text(
    root: &Path,
    path: &ProjectPathRef,
    expected_hash: Option<&Sha256Hash>,
) -> Option<String> {
    let expected_hash = expected_hash?;
    let candidate = root.join(path.as_str());
    let metadata = std::fs::symlink_metadata(&candidate).ok()?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > M3_RULE_TEXT_LIMIT
    {
        return None;
    }
    let canonical_root = std::fs::canonicalize(root).ok()?;
    let canonical_file = std::fs::canonicalize(candidate).ok()?;
    if !canonical_file.starts_with(canonical_root) {
        return None;
    }
    let mut file = std::fs::File::open(canonical_file).ok()?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.by_ref()
        .take(M3_RULE_TEXT_LIMIT + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > M3_RULE_TEXT_LIMIT || &Sha256Hash::digest(&bytes) != expected_hash {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn m3_git_head_text(
    root: &Path,
    path: &ProjectPathRef,
    expected_hash: Option<&Sha256Hash>,
) -> Option<String> {
    let expected_hash = expected_hash?;
    let object = format!("HEAD:{}", path.as_str());
    let size = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["cat-file", "-s"])
        .arg(&object)
        .output()
        .ok()?;
    if !size.status.success()
        || String::from_utf8(size.stdout)
            .ok()?
            .trim()
            .parse::<u64>()
            .ok()?
            > M3_RULE_TEXT_LIMIT
    {
        return None;
    }
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["show", "--no-textconv"])
        .arg(object)
        .output()
        .ok()?;
    if !output.status.success()
        || output.stdout.len() as u64 > M3_RULE_TEXT_LIMIT
        || &Sha256Hash::digest(&output.stdout) != expected_hash
    {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn m3_marker_count(text: &str, markers: &[&str]) -> usize {
    let normalized = text.to_ascii_lowercase();
    markers
        .iter()
        .map(|marker| normalized.matches(&marker.to_ascii_lowercase()).count())
        .sum()
}

fn m3_contains_secret_candidate(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    [
        "-----begin private key-----",
        "ghp_",
        "api_key=",
        "api_key =",
        "password=",
        "password =",
        "client_secret=",
        "client_secret =",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

fn m3_contains_dangerous_command(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    [
        "git reset --hard",
        "git clean -fd",
        "rm -rf /",
        "remove-item -recurse -force $home",
        "format-volume",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

fn m3_dependency_path(path: &str) -> bool {
    matches!(
        path,
        "Cargo.toml"
            | "Cargo.lock"
            | "package.json"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "requirements.txt"
            | "pyproject.toml"
    ) || path.ends_with("/Cargo.toml")
}

fn m3_environment_path(path: &str) -> bool {
    path.starts_with(".github/")
        || path.starts_with("packaging/")
        || path.starts_with("scripts/install")
        || path.starts_with("scripts/release/")
        || path.starts_with("rust-toolchain")
}

fn m3_validation_protected_path(path: &str) -> bool {
    path == "scripts/validate.ps1"
        || path.starts_with("scripts/validation/")
        || path.starts_with("crates/control/star-validation/")
        || path.starts_with("crates/foundation/star-contracts/src/evidence")
        || path.starts_with("specs/schemas/v1/validation-")
        || path.starts_with("specs/schemas/v1/gate-")
        || path.starts_with("specs/fixtures/management/v1/validation-")
        || path.starts_with("specs/fixtures/management/v1/gate-")
}

fn m3_task_requests_bug_fix(task: &star_contracts::planning::TaskSpec) -> bool {
    let text = format!("{} {}", task.title, task.objective).to_ascii_lowercase();
    let words = text
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .collect::<BTreeSet<_>>();
    ["bug", "fix", "bugfix", "regression", "defect"]
        .iter()
        .any(|token| words.contains(token))
        || ["버그", "오류", "회귀", "결함"]
            .iter()
            .any(|token| text.contains(token))
}

fn resolve_logical_executable_path(executable: &str) -> Option<PathBuf> {
    if executable.trim().is_empty()
        || executable == "unknown"
        || executable.contains('\0')
        || executable.contains('/')
        || executable.contains('\\')
    {
        return None;
    }
    let mut names = vec![executable.to_owned()];
    if Path::new(executable).extension().is_none() {
        #[cfg(windows)]
        names.extend(
            [".EXE", ".COM"]
                .into_iter()
                .map(|extension| format!("{executable}{extension}")),
        );
    }
    std::env::var_os("PATH")
        .and_then(|value| {
            std::env::split_paths(&value)
                .flat_map(|directory| names.iter().map(move |name| directory.join(name)))
                .find(|candidate| {
                    std::fs::symlink_metadata(candidate).is_ok_and(|metadata| {
                        metadata.is_file() && !metadata.file_type().is_symlink()
                    })
                })
        })
        .and_then(|candidate| std::fs::canonicalize(candidate).ok())
}

fn application_document_hash<T: Serialize>(value: &T) -> Result<Sha256Hash, ApplicationError> {
    let value = serde_json::to_value(value).map_err(|_| ApplicationError::Invalid)?;
    canonical_sha256(&value).map_err(|_| ApplicationError::Invalid)
}

fn merge_project_attachment(
    existing: Option<Project>,
    mut candidate: Project,
) -> Result<Project, ApplicationError> {
    let Some(existing) = existing else {
        return Ok(candidate);
    };
    if existing.project_id != candidate.project_id
        || existing.identity_scope != candidate.identity_scope
        || existing.display_name != candidate.display_name
        || existing.repository_kind != candidate.repository_kind
        || existing.source_of_truth != candidate.source_of_truth
        || existing.declaration_fingerprint != candidate.declaration_fingerprint
    {
        return Err(ApplicationError::Invalid);
    }
    candidate
        .attached_checkout_ids
        .extend(existing.attached_checkout_ids);
    candidate.attached_checkout_ids.sort();
    candidate.attached_checkout_ids.dedup();
    candidate.latest_revision_id = existing.latest_revision_id;
    candidate.latest_workspace_snapshot_id = existing.latest_workspace_snapshot_id;
    Ok(candidate)
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
        "project_id":project.project_id,
        "identity_scope":project.identity_scope,
        "display_name":project.display_name,
        "repository_kind":project.repository_kind,
        "source_of_truth":project.source_of_truth,
        "declaration_fingerprint":project.declaration_fingerprint,
        "checkout_id": checkout.checkout_id,
        "checkout_content_fingerprint": checkout.content_fingerprint,
    })
}

fn legacy_registration_fingerprint_payload(
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
        evidence_v2::{
            TASK_INVOCATION_V2_SCHEMA_ID, TaskInvocationV2, ValidationStabilityV2,
            empty_fingerprint,
        },
        ids::{
            ArtifactId, BaselineId, DispositionId, GoalId, RunId, SuppressionId, TaskInvocationId,
        },
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
    use star_state::{
        SqliteManagementRecovery, SqliteManagementRepositorySet, WindowsProjectRootBindingStore,
    };

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
    fn validation_output_sink_redacts_sensitive_streams_without_persisting_their_hash() {
        let root = std::env::temp_dir().join(format!(
            "star-validation-output-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let store = Arc::new(LocalArtifactStore::default());
        let project_id = ProjectId::new();
        let task_spec_id = TaskSpecId::new();
        let mut sink = ValidationOutputArtifactSink {
            artifacts: store.clone(),
            project_id: project_id.clone(),
            project_root: root.clone(),
            task_spec_id,
            artifact_set_id: RequestId::new(),
            redactor: PersistenceRedactor::for_current_user(),
        };
        let invocation = TaskInvocationV2 {
            schema_id: TASK_INVOCATION_V2_SCHEMA_ID.to_owned(),
            schema_version: 2,
            invocation_id: TaskInvocationId::new(),
            tool_ref: CatalogRef {
                catalog_id: "fixture.tool".to_owned(),
                format_version: 1,
                item_version: "1.0.0".to_owned(),
                sha256: Sha256Hash::digest(b"fixture.tool"),
            },
            executable: "fixture".to_owned(),
            executable_binding_fingerprint: Sha256Hash::digest(b"fixture.binding"),
            args: vec![],
            cwd: InvocationWorkingDirectoryV2::ProjectRoot,
            env_refs: BTreeMap::new(),
            stdin_ref: None,
            timeout_ms: 1_000,
            permission_action: "local_validation".to_owned(),
            idempotency_key: "fixture-output".to_owned(),
            expected_exit_codes: BTreeSet::from([0]),
            output_limits: OutputLimits {
                stdout_bytes: 1024,
                stderr_bytes: 1024,
                artifact_bytes: 4096,
            },
            input_fingerprint: empty_fingerprint(),
        }
        .seal()
        .unwrap();
        let refs = sink
            .persist(CheckOutputArtifactInput {
                invocation: &invocation,
                exit_code: Some(0),
                termination_reason: TerminationReason::Exited,
                stdout: b"token=must-not-persist",
                stderr: b"safe diagnostic code",
                stdout_truncated: false,
                stderr_truncated: false,
                output_read_failed: false,
            })
            .unwrap();
        assert_eq!(refs.len(), 2);
        let redacted = refs
            .iter()
            .find(|artifact| artifact.redaction_status == RedactionStatus::Redacted)
            .unwrap();
        let value = store.read_json(&root, redacted).unwrap();
        let encoded = serde_json::to_string(&value).unwrap();
        assert!(!encoded.contains("must-not-persist"));
        assert!(value["content"].is_null());
        assert!(value["content_sha256"].is_null());
        assert!(
            refs.iter()
                .any(|artifact| artifact.redaction_status == RedactionStatus::NotNeeded)
        );
        assert_eq!(
            store
                .discover_verified(&project_id, &root)
                .unwrap()
                .verified
                .len(),
            2
        );
    }

    #[test]
    fn explicit_roots_attach_linked_worktrees_to_one_project_with_distinct_checkouts() {
        let root = std::env::temp_dir().join(format!(
            "star-multi-root-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        let repository = root.join("repository");
        let linked = root.join("linked");
        std::fs::create_dir_all(&repository).unwrap();
        let run_git = |cwd: &Path, args: &[&str]| {
            let status = std::process::Command::new("git")
                .current_dir(cwd)
                .args(args)
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?}");
        };
        run_git(&repository, &["init"]);
        std::fs::write(
            repository.join("source.rs"),
            "pub fn value() -> u32 { 1 }\n",
        )
        .unwrap();
        run_git(&repository, &["add", "source.rs"]);
        run_git(
            &repository,
            &[
                "-c",
                "user.name=Star Test",
                "-c",
                "user.email=star@example.invalid",
                "commit",
                "-m",
                "baseline",
            ],
        );
        run_git(
            &repository,
            &[
                "worktree",
                "add",
                "-b",
                "fixture-linked",
                linked.to_str().unwrap(),
            ],
        );
        let repositories = Arc::new(
            SqliteManagementRepositorySet::open(root.join("management"), "multi-root-test")
                .unwrap(),
        );
        let bindings =
            Arc::new(WindowsProjectRootBindingStore::open(root.join("root-bindings")).unwrap());
        let service = ManagementApplicationService::new(
            repositories,
            bindings,
            Arc::new(LocalArtifactStore::default()),
        );
        let result = service
            .discover_project_roots(
                &[
                    repository.canonicalize().unwrap(),
                    linked.canonicalize().unwrap(),
                ],
                "multi-root",
            )
            .unwrap();
        assert_eq!(result.registrations.len(), 2);
        assert_eq!(result.catalog_snapshot.counts.projects, 1);
        assert_eq!(result.catalog_snapshot.counts.checkouts, 2);
        let project_id = result.registrations[0].project.project_id.clone();
        assert!(
            result
                .registrations
                .iter()
                .all(|registration| registration.project.project_id == project_id)
        );
        let checkouts = service.list_project_checkouts(&project_id).unwrap();
        assert_eq!(checkouts.len(), 2);
        assert_eq!(
            checkouts[0].repository_binding_id,
            checkouts[1].repository_binding_id
        );
        assert_ne!(
            checkouts[0].worktree_binding_id,
            checkouts[1].worktree_binding_id
        );
        assert_eq!(
            service
                .get_project_checkout(&checkouts[0].checkout_id)
                .unwrap(),
            checkouts[0]
        );
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
        std::fs::create_dir_all(source.join(".cargo")).unwrap();
        std::fs::create_dir_all(source.join("scripts")).unwrap();
        let declared_project_id = ProjectId::new();
        std::fs::write(
            source.join(".star-control/project.toml"),
            format!(
                "schema_version = 1\nproject_id = \"{}\"\ndisplay_name = \"fixture-project\"\nrepository_kind = \"none\"\nsource_of_truth = [\"source\"]\n",
                declared_project_id.as_str()
            ),
        )
        .unwrap();
        std::fs::write(
            source.join("Cargo.toml"),
            "[package]\nname = \"fixture-project\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::write(
            source.join("Cargo.lock"),
            "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n[[package]]\nname = \"fixture-project\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::write(
            source.join(".cargo/config.toml"),
            "[build]\ntarget-dir = \"../cargo-target\"\n",
        )
        .unwrap();
        std::fs::write(
            source.join("scripts/validate.ps1"),
            "param([string]$Profile,[string]$OutputFormat)\n[Console]::Out.WriteLine('{\"status\":\"pass\"}')\nexit 0\n",
        )
        .unwrap();
        std::fs::write(source.join("src/lib.rs"), b"pub fn fixture() {}  \n").unwrap();
        std::fs::write(source.join("user-change.txt"), b"preserve\n").unwrap();
        let repositories =
            Arc::new(SqliteManagementRepositorySet::open(root.join("management"), "test").unwrap());
        let bindings =
            Arc::new(WindowsProjectRootBindingStore::open(root.join("root-bindings")).unwrap());
        let mut service = ManagementApplicationService::new(
            repositories.clone(),
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
            .index_search(&project_id, "fixture", IndexTier::Text, true)
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
            profile_ids: vec![],
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
        let toolchain_planning = service
            .create_planning_bundle(
                task.clone(),
                actor.clone(),
                vec![],
                "planning-toolchain-test",
            )
            .unwrap();
        assert_eq!(
            toolchain_planning.validation_plan.readiness,
            ValidationPlanV2Readiness::Ready,
            "{:#?}",
            toolchain_planning.validation_plan
        );
        assert!(
            toolchain_planning
                .validation_plan
                .required_checks
                .iter()
                .all(|check| check.descriptor_ref.schema_id == "star.check-descriptor")
        );
        assert!(
            toolchain_planning
                .validation_plan
                .required_checks
                .iter()
                .any(|check| check.family == "test")
        );
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
        let validation_plan_ref = DocumentRef {
            schema_id: star_contracts::planning::FULL_VALIDATION_PLAN_SCHEMA_ID.to_owned(),
            document_id: planning.validation_plan.validation_plan_id.to_string(),
            revision: planning.validation_plan.revision,
            sha256: application_document_hash(&planning.validation_plan).unwrap(),
        };
        let source_ref = planning.scope_revision.source_snapshot_refs[0].clone();
        let workspace = service
            .repositories
            .project(&project_id)
            .unwrap()
            .get_workspace_snapshot(&source_ref.workspace_snapshot_id)
            .unwrap()
            .unwrap();
        let gate_policy_fingerprint = versioned_fingerprint(
            "star.gate-policy-v2",
            2,
            &planning.validation_plan.gate_policy,
        )
        .unwrap();
        let executable_bindings = planning
            .validation_plan
            .required_checks
            .iter()
            .map(|check| {
                let check_ref = star_contracts::evidence::CatalogRef {
                    catalog_id: check.check_id.clone(),
                    format_version: 1,
                    item_version: "1.0.0".to_owned(),
                    sha256: Sha256Hash::digest(check.check_id.as_bytes()),
                };
                let tool_ref = star_contracts::evidence::CatalogRef {
                    catalog_id: "fixture.validator".to_owned(),
                    format_version: 1,
                    item_version: "1.0.0".to_owned(),
                    sha256: Sha256Hash::digest(b"fixture.validator"),
                };
                let subject_binding = EvidenceSubjectBinding {
                    project_id: project_id.clone(),
                    checkout_id: source_ref.checkout_id.clone(),
                    project_revision_id: source_ref.project_revision_id.clone(),
                    workspace_snapshot_id: source_ref.workspace_snapshot_id.clone(),
                    workspace_content_fingerprint: workspace.entries_fingerprint.clone(),
                    task_spec_ref: planning.validation_plan.task_spec_ref.clone(),
                    scope_revision_ref: planning.validation_plan.scope_revision_ref.clone(),
                    impact_analysis_ref: planning.validation_plan.impact_analysis_ref.clone(),
                    change_set_refs: planning.validation_plan.change_set_refs.clone(),
                    change_plan_refs: vec![],
                    patch_set_ref: None,
                    validation_plan_ref: validation_plan_ref.clone(),
                    gate_phase: GatePhaseV2::DuringStage,
                    profile_resolution_fingerprint: planning
                        .validation_plan
                        .profile_resolution
                        .as_ref()
                        .map(|resolution| resolution.profile_resolution_fingerprint.clone())
                        .unwrap_or_else(|| planning.validation_plan.selection_fingerprint.clone()),
                    effective_config_fingerprint: planning
                        .validation_plan
                        .config_fingerprint
                        .clone(),
                    gate_policy_fingerprint: gate_policy_fingerprint.clone(),
                    catalog_snapshot_ref: planning.validation_plan.catalog_snapshot_ref.clone(),
                    validator_registry_fingerprint: Sha256Hash::digest(b"fixture-registry"),
                    check_descriptor_ref: Some(check.descriptor_ref.clone()),
                    rule_refs: vec![check_ref.clone()],
                    tool_registry_snapshot_ref: None,
                    tool_descriptor_ref: Some(tool_ref.clone()),
                    observed_tool_fingerprint: None,
                    invocation_fingerprint: None,
                    execution_environment_fingerprint: Sha256Hash::digest(b"fixture-environment"),
                    normalizer_fingerprint: Sha256Hash::digest(b"fixture-normalizer"),
                    freshness: EvidenceFreshnessV2::Current,
                    stale_reasons: vec![],
                    binding_fingerprint: Sha256Hash::digest(b""),
                    probed_at: Utc::now(),
                }
                .seal()
                .unwrap();
                ExecutableBinding {
                    check_id: check.check_id.clone(),
                    check_ref,
                    tool_ref,
                    logical_executable: check.invocation.logical_executable.clone(),
                    executable_binding_fingerprint: Sha256Hash::digest(b"fixture-binding"),
                    cwd: InvocationWorkingDirectoryV2::ProjectRoot,
                    permission_action: "local_write".to_owned(),
                    output_limits: OutputLimits {
                        stdout_bytes: 1024,
                        stderr_bytes: 1024,
                        artifact_bytes: 4096,
                    },
                    subject_binding,
                }
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
                    baselines: vec![],
                    suppressions: vec![],
                    dispositions: vec![],
                    evaluation_time: Utc::now(),
                    max_attempts_per_check: 1,
                    preflight_diagnostics: vec![],
                    completion_claims: vec![],
                    change_sets: planning.change_sets.clone(),
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
        let execution_status = service.validation_execution_status(&project_id).unwrap();
        assert_eq!(execution_status.run_count, execution.validation_runs.len());
        assert_eq!(
            execution_status.result_count,
            execution.validation_results.len()
        );
        assert_eq!(execution_status.gate_count, 1);
        assert_eq!(execution_status.evidence_bundle_count, 1);
        assert_eq!(execution_status.review_pack_count, 1);
        assert_eq!(
            service
                .get_gate_decision_v2(&project_id, &execution.gate_decision.gate_id)
                .unwrap()
                .decision_fingerprint,
            execution.gate_decision.decision_fingerprint
        );
        assert_eq!(
            service
                .get_review_pack_v1(&project_id, &execution.review_pack.review_pack_id)
                .unwrap()
                .review_pack_fingerprint,
            execution.review_pack.review_pack_fingerprint
        );
        let mut revised_task = task.clone();
        revised_task.objective = "Apply a bounded source change with explicit lineage".to_owned();
        let revised = service
            .revise_planning_bundle(
                &planning.task_spec.task_spec_id,
                revised_task,
                actor.clone(),
                check_descriptors.clone(),
                ScopeReasonCode::UserEdit,
                "fixture scope revision",
                vec![],
                "planning-revise-test",
            )
            .unwrap();
        assert_eq!(revised.task_spec.revision, 2);
        assert_eq!(revised.scope_revision.revision, 2);
        assert!(revised.scope_revision.previous_scope_revision_ref.is_some());
        assert!(
            revised
                .validation_plan
                .previous_success_comparisons
                .iter()
                .any(|comparison| comparison.starts_with("reusable:"))
        );
        let overridden = service
            .set_planning_check_override(
                &planning.task_spec.task_spec_id,
                CheckOverride {
                    family: "lint".to_owned(),
                    kind: star_contracts::planning::CheckOverrideKind::Omit,
                    reason: "fixture waiver".to_owned(),
                },
                actor.clone(),
                check_descriptors.clone(),
                "planning-override-test",
            )
            .unwrap();
        assert_eq!(planning_bundle_revision(&overridden), 3);
        assert!(
            overridden
                .validation_plan
                .candidate_checks
                .iter()
                .any(|check| {
                    check.family == "lint"
                        && check.outcome
                            == star_contracts::planning::CheckResolutionOutcome::UserWaived
                })
        );
        assert!(
            overridden
                .validation_plan
                .previous_success_comparisons
                .iter()
                .any(|comparison| comparison.starts_with("not_reusable_selection_changed:"))
        );
        let invalidated = service
            .invalidate_planning_bundle(
                &planning.task_spec.task_spec_id,
                actor.clone(),
                "fixture source invalidation",
                "planning-invalidate-test",
            )
            .unwrap();
        assert_eq!(planning_bundle_revision(&invalidated), 4);
        assert_eq!(
            invalidated.validation_plan.readiness,
            ValidationPlanV2Readiness::Invalidated
        );
        let replanned = service
            .replan_planning_bundle(
                &planning.task_spec.task_spec_id,
                actor.clone(),
                check_descriptors.clone(),
                "fixture current source replan",
                "planning-replan-test",
            )
            .unwrap();
        assert_eq!(planning_bundle_revision(&replanned), 5);
        assert_eq!(
            replanned.validation_plan.readiness,
            ValidationPlanV2Readiness::Ready
        );
        assert_eq!(
            service
                .list_planning_bundle_revisions(&planning.task_spec.task_spec_id)
                .unwrap()
                .len(),
            5
        );
        assert_eq!(
            service
                .planning_bundle_status(&planning.task_spec.task_spec_id)
                .unwrap()
                .bundle_revision,
            5
        );
        let mut conflicting_task = task;
        conflicting_task.objective = "different idempotency input".to_owned();
        assert!(matches!(
            service.create_planning_bundle(
                conflicting_task,
                actor.clone(),
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
        let prepared_v2 = service
            .prepare_change_v2(
                &project_id,
                &registration.checkout.checkout_id,
                "star.recipe.remove-trailing-whitespace@2.0.0",
                TargetSelector::Finding {
                    project_id: project_id.clone(),
                    finding_ids: vec![findings[0].finding_id.to_string()],
                    expected_finding_fingerprints: BTreeMap::from([(
                        findings[0].finding_id.to_string(),
                        findings[0].finding_fingerprint.clone(),
                    )]),
                },
                serde_json::json!({}),
                WorktreeStrategyV1::Current,
                actor.clone(),
            )
            .unwrap();
        assert_eq!(prepared_v2.patch_set.state, PatchSetStateV2::Ready);
        assert_eq!(
            std::fs::read(source.join("src/lib.rs")).unwrap(),
            b"pub fn fixture() {}  \n"
        );
        let shown = service
            .show_patch_v2(&prepared_v2.patch_set.patch_set_id)
            .unwrap();
        assert_eq!(shown.patch_set, prepared_v2.patch_set);
        assert_eq!(shown.forward_artifact_refs.len(), 1);
        assert_eq!(shown.reverse_artifact_refs.len(), 1);
        let before_v2_apply_index = service.index_status(&project_id).unwrap();
        assert!(
            before_v2_apply_index.current,
            "patch-v2 pre-apply index freshness: {:?}",
            before_v2_apply_index.snapshot.freshness
        );
        let v2_applied = service
            .apply_patch_v2(
                &prepared_v2.patch_set.patch_set_id,
                prepared_v2.patch_set.patch_fingerprint.as_str(),
                actor.clone(),
                Some("fixture-patch-v2-approval"),
                None,
            )
            .unwrap();
        let pre_apply_diagnostics = service.list_validation_diagnostics_v2(&project_id).unwrap();
        assert!(
            v2_applied.source_effect_started,
            "patch-v2 apply did not start source effects: {pre_apply_diagnostics:#?}"
        );
        assert!(
            !v2_applied.recovered,
            "patch-v2 apply recovered unexpectedly: state={:?}, reasons={:?}, diagnostics={:#?}",
            v2_applied.application.state,
            v2_applied.application.reason_codes,
            service.list_validation_diagnostics_v2(&project_id).unwrap()
        );
        assert!(matches!(
            v2_applied.application.state,
            PatchApplicationStateV1::Applied | PatchApplicationStateV1::AwaitingHumanReview
        ));
        assert_eq!(
            std::fs::read(source.join("src/lib.rs")).unwrap(),
            b"pub fn fixture() {}\n"
        );
        let reverse_ref = v2_applied
            .application
            .reverse_patch_set_ref
            .clone()
            .unwrap();
        let reverse_patch_id = PatchSetId::parse(reverse_ref.document_id).unwrap();
        let shown_reverse = service.show_patch_v2(&reverse_patch_id).unwrap();
        assert_eq!(shown_reverse.patch_set.patch_set_id, reverse_patch_id);
        let v2_status = service
            .patch_status_v2(&v2_applied.application.patch_application_id)
            .unwrap();
        assert_eq!(v2_status.observed_state, v2_applied.application.state);
        assert!(
            v2_status
                .recovery_strategies
                .contains(&PatchRecoveryStrategyV1::ReversePatch)
        );
        let v2_recovered = service
            .recover_patch_v2(
                &v2_applied.application.patch_application_id,
                PatchRecoveryStrategyV1::ReversePatch,
                actor.clone(),
            )
            .unwrap();
        assert!(v2_recovered.recovered);
        assert_eq!(
            v2_recovered.application.state,
            PatchApplicationStateV1::Reverted
        );
        assert_eq!(
            std::fs::read(source.join("src/lib.rs")).unwrap(),
            b"pub fn fixture() {}  \n"
        );
        let migration_plan = service.plan_patch_v1_to_v2_migration(&project_id).unwrap();
        assert!(
            migration_plan
                .entries
                .iter()
                .all(|entry| entry.limitations.is_empty())
        );
        let migration = service
            .apply_patch_v1_to_v2_migration(
                migration_plan.clone(),
                migration_plan.plan_fingerprint.as_str(),
            )
            .unwrap();
        assert_eq!(migration.outcome, PatchMigrationOutcomeV1::Applied);
        assert!(migration.backup_manifest_ref.is_some());
        let rollback = service
            .rollback_patch_v1_to_v2_migration(
                migration_plan.clone(),
                migration_plan.plan_fingerprint.as_str(),
            )
            .unwrap();
        assert_eq!(rollback.outcome, PatchMigrationOutcomeV1::RolledBack);
        let recovered_scan = service
            .scan_project(&project_id, "scan-after-v2-recovery")
            .unwrap();
        assert_eq!(recovered_scan.scan_run.status, ScanStatus::Succeeded);
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
            b"pub fn fixture() {}\n"
        );
        assert_eq!(
            std::fs::read(source.join("user-change.txt")).unwrap(),
            b"preserve\n"
        );
        let project_repository = repositories.project(&project_id).unwrap();
        let expected_dispositions = project_repository.list_dispositions().unwrap();
        let expected_scan = project_repository.latest_scan().unwrap().unwrap();
        let expected_artifact_refs = project_repository
            .artifact_refs_for_scan(&expected_scan.scan_run_id)
            .unwrap();
        assert!(!expected_dispositions.is_empty());
        assert!(!expected_artifact_refs.is_empty());
        for artifact in &expected_artifact_refs {
            LocalArtifactStore::default()
                .verify(&source, artifact)
                .unwrap();
        }
        let rejected_artifact = LocalArtifactStore::default()
            .put_json(
                &project_id,
                &source,
                "management/recovery/rejected.json",
                "recovery_fixture",
                "rejected_artifact",
                &serde_json::json!({"message_code":"RECOVERY_FIXTURE"}),
            )
            .unwrap();
        let rejected_artifact_path = rejected_artifact
            .relative_path
            .split('/')
            .fold(source.clone(), |path, segment| path.join(segment));
        std::fs::write(&rejected_artifact_path, b"tampered-artifact").unwrap();
        drop(project_repository);

        let backup_root = root.join("backup-set");
        let backup_plan = service.plan_backup(&backup_root).unwrap();
        let backup = service
            .apply_backup(
                &backup_root,
                &backup_plan,
                backup_plan.plan_fingerprint.as_str(),
            )
            .unwrap();
        let backup_manifest_json = serde_json::to_string(&backup.manifest).unwrap();
        assert!(!backup_manifest_json.contains(source.to_string_lossy().as_ref()));
        assert!(
            !backup_manifest_json
                .to_ascii_lowercase()
                .contains("root_binding")
        );
        for entry in &backup.manifest.entries {
            let backup_bytes = std::fs::read(backup_root.join(&entry.relative_locator)).unwrap();
            let searchable = String::from_utf8_lossy(&backup_bytes);
            assert!(!searchable.contains(source.to_string_lossy().as_ref()));
            if let Ok(username) = std::env::var("USERNAME")
                && username.len() >= 3
            {
                assert!(
                    !searchable
                        .to_ascii_lowercase()
                        .contains(&username.to_ascii_lowercase())
                );
            }
        }

        let active_set = repositories.active_set().unwrap();
        let corrupt_locator = active_set
            .entries
            .iter()
            .find(|entry| matches!(entry.scope, star_contracts::management::StoreScope::Global))
            .unwrap()
            .relative_locator
            .clone();
        let corrupt_store = root
            .join("management")
            .join(corrupt_locator)
            .join("management.v1.db");
        drop(service);
        drop(repositories);
        std::fs::write(&corrupt_store, b"simulated-corrupt-management-store").unwrap();
        let recovery = SqliteManagementRecovery::open(root.join("management"), "test").unwrap();
        assert_eq!(
            recovery.status().unwrap().mode,
            star_contracts::recovery::ControllerRecoveryMode::RecoveryOnly
        );
        let restore_plan = recovery.plan_restore(&backup_root).unwrap();
        let restored = recovery
            .apply_restore(
                &backup_root,
                &restore_plan,
                restore_plan.plan_fingerprint.as_str(),
            )
            .unwrap();
        assert_eq!(
            restored.activated_set.manifest_fingerprint,
            restore_plan.candidate_active_set.manifest_fingerprint
        );
        assert_eq!(
            std::fs::read(&corrupt_store).unwrap(),
            b"simulated-corrupt-management-store"
        );
        drop(recovery);

        let restored_repositories =
            SqliteManagementRepositorySet::open(root.join("management"), "restored-test").unwrap();
        let restored_project = restored_repositories.project(&project_id).unwrap();
        assert_eq!(
            restored_project.list_dispositions().unwrap(),
            expected_dispositions
        );
        assert_eq!(
            restored_project.latest_scan().unwrap().unwrap().scan_run_id,
            expected_scan.scan_run_id
        );
        assert_eq!(
            restored_project
                .artifact_refs_for_scan(&expected_scan.scan_run_id)
                .unwrap(),
            expected_artifact_refs
        );
        for artifact in &expected_artifact_refs {
            LocalArtifactStore::default()
                .verify(&source, artifact)
                .unwrap();
        }
        restored_repositories.verify_all().unwrap();
        let restored_active_set = restored_repositories.active_set().unwrap();
        let restored_global_locator = restored_active_set
            .entries
            .iter()
            .find(|entry| matches!(entry.scope, star_contracts::management::StoreScope::Global))
            .unwrap()
            .relative_locator
            .clone();
        let restored_global_store = root
            .join("management")
            .join(restored_global_locator)
            .join("management.v1.db");
        drop(restored_project);
        drop(restored_repositories);
        std::fs::write(&restored_global_store, b"simulated-corrupt-restored-store").unwrap();

        let recovery = SqliteManagementRecovery::open(root.join("management"), "test").unwrap();
        let rebuilt = ManagementRecoveryApplicationService::new(
            &recovery,
            bindings.clone(),
            Arc::new(LocalArtifactStore::default()),
        );
        let rebuild_plan = rebuilt.plan_source_rebuild().unwrap();
        assert_eq!(rebuild_plan.projects[0].project_id, project_id);
        assert_eq!(rebuild_plan.projects[0].rejected_artifact_count, 1);
        assert!(rebuild_plan.predicted_losses.iter().any(|loss| {
            loss.kind == RecoveryLossKind::LocalDisposition
                && loss.state == RecoveryLossState::Lost
                && loss.project_id.as_ref() == Some(&project_id)
        }));
        let rebuild_result = rebuilt
            .apply_source_rebuild(&rebuild_plan, rebuild_plan.plan_fingerprint.as_str())
            .unwrap();
        let plan_token = rebuild_plan
            .plan_fingerprint
            .as_str()
            .trim_start_matches("sha256:");
        let rebuild_receipt = root
            .join("management")
            .join("recovery-receipts")
            .join(format!("rebuild-{}.json", &plan_token[..32]));
        std::fs::rename(
            &rebuild_receipt,
            root.join("simulated-crash-after-rebuild-activation.json"),
        )
        .unwrap();
        let reconciled_rebuild = rebuilt
            .apply_source_rebuild(&rebuild_plan, rebuild_plan.plan_fingerprint.as_str())
            .unwrap();
        assert_eq!(
            reconciled_rebuild.rebuilt_projects,
            rebuild_result.rebuilt_projects
        );
        assert_eq!(reconciled_rebuild.loss_report, rebuild_result.loss_report);
        assert_eq!(
            reconciled_rebuild.activated_set,
            rebuild_result.activated_set
        );
        assert_eq!(
            rebuilt
                .apply_source_rebuild(&rebuild_plan, rebuild_plan.plan_fingerprint.as_str())
                .unwrap(),
            reconciled_rebuild
        );
        assert_eq!(rebuild_result.rebuilt_projects.len(), 1);
        assert_eq!(rebuild_result.rebuilt_projects[0].project_id, project_id);
        assert_eq!(rebuild_result.rebuilt_projects[0].finding_count, 0);
        assert!(
            rebuild_result.rebuilt_projects[0].reindexed_artifact_count
                >= u64::try_from(expected_artifact_refs.len()).unwrap()
        );
        assert_eq!(
            rebuild_result.rebuilt_projects[0].rejected_artifact_count,
            1
        );
        assert!(
            rebuild_result
                .loss_report
                .iter()
                .any(|loss| loss.kind == RecoveryLossKind::LocalDisposition
                    && loss.state == RecoveryLossState::Lost)
        );
        assert!(rebuild_result.loss_report.iter().any(|loss| {
            loss.kind == RecoveryLossKind::ArtifactReference
                && loss.state == RecoveryLossState::Lost
                && loss.count == Some(1)
        }));
        assert_eq!(
            std::fs::read(&corrupt_store).unwrap(),
            b"simulated-corrupt-management-store"
        );
        assert_eq!(
            std::fs::read(&restored_global_store).unwrap(),
            b"simulated-corrupt-restored-store"
        );
        assert_eq!(
            std::fs::read(source.join("user-change.txt")).unwrap(),
            b"preserve\n"
        );
        drop(rebuilt);
        drop(recovery);
        let reopened =
            SqliteManagementRepositorySet::open(root.join("management"), "test").unwrap();
        let rebuilt_project = reopened.project(&project_id).unwrap();
        let rebuilt_scan = rebuilt_project.latest_scan().unwrap().unwrap();
        assert_eq!(rebuilt_scan.project_id, project_id);
        let rebuilt_artifacts = rebuilt_project.list_artifact_refs().unwrap();
        for expected in &expected_artifact_refs {
            assert!(rebuilt_artifacts.contains(expected));
        }
        assert!(!rebuilt_artifacts.contains(&rejected_artifact));
        for artifact in &rebuilt_artifacts {
            LocalArtifactStore::default()
                .verify(&source, artifact)
                .unwrap();
        }
        reopened.verify_all().unwrap();
    }

    #[test]
    fn local_state_export_import_is_redacted_revision_bound_and_recovery_readable() {
        let root = std::env::temp_dir().join(format!(
            "local-state-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        let source = root.join("source");
        std::fs::create_dir_all(source.join("src")).unwrap();
        std::fs::create_dir_all(source.join(".star-control")).unwrap();
        let project_id = ProjectId::new();
        std::fs::write(
            source.join(".star-control/project.toml"),
            format!(
                "schema_version = 1\nproject_id = \"{}\"\ndisplay_name = \"local-state-fixture\"\nrepository_kind = \"none\"\nsource_of_truth = [\"source\"]\n",
                project_id.as_str()
            ),
        )
        .unwrap();
        std::fs::write(
            source.join("src/lib.rs"),
            b"pub fn answer() -> u32 { 42 }  \n",
        )
        .unwrap();
        let management_root = root.join("management");
        let repositories =
            Arc::new(SqliteManagementRepositorySet::open(&management_root, "test").unwrap());
        let bindings =
            Arc::new(WindowsProjectRootBindingStore::open(root.join("root-bindings")).unwrap());
        let service = ManagementApplicationService::new(
            repositories.clone(),
            bindings.clone(),
            Arc::new(LocalArtifactStore::default()),
        );
        service
            .register_project(&source.canonicalize().unwrap(), "local-state-register")
            .unwrap();
        service
            .scan_project(&project_id, "local-state-scan")
            .unwrap();
        let finding = service.list_findings(&project_id).unwrap().remove(0);
        let disposition = Disposition {
            schema_id: "star.disposition".to_owned(),
            schema_version: 1,
            disposition_id: DispositionId::new(),
            revision: 1,
            finding_id: finding.finding_id,
            finding_fingerprint: finding.finding_fingerprint,
            decision: DispositionDecision::NeedsAction,
            reason_code: "LOCAL_REVIEW".to_owned(),
            reason: "confirmed-local-state".to_owned(),
            scope_revision: None,
            expires_at: None,
            duplicate_of_finding_id: None,
            decided_at: Utc::now(),
            provenance: "local:event".to_owned(),
            status: DispositionStatus::Active,
        };
        service
            .put_disposition(&project_id, &disposition, 0)
            .unwrap();

        let export_path = root.join("local-state.json");
        let export_plan = service
            .plan_local_state_export(&project_id, &export_path)
            .unwrap();
        let export = service
            .apply_local_state_export(
                &export_path,
                &export_plan,
                export_plan.plan_fingerprint.as_str(),
            )
            .unwrap();
        assert_eq!(
            service
                .apply_local_state_export(
                    &export_path,
                    &export_plan,
                    export_plan.plan_fingerprint.as_str(),
                )
                .unwrap(),
            export
        );
        assert_eq!(export.bundle.local_dispositions, vec![disposition.clone()]);
        assert!(export.bundle.local_suppressions.is_empty());
        let exported_text = std::fs::read_to_string(&export_path).unwrap();
        assert!(!exported_text.contains(&source.to_string_lossy().to_string()));
        assert!(!exported_text.to_ascii_lowercase().contains("root_binding"));
        if let Ok(username) = std::env::var("USERNAME")
            && !username.is_empty()
        {
            assert!(!exported_text.contains(&username));
        }

        let active_set = repositories.active_set().unwrap();
        let global = active_set
            .entries
            .iter()
            .find(|entry| matches!(entry.scope, star_contracts::management::StoreScope::Global))
            .unwrap();
        let corrupt_global = management_root
            .join(&global.relative_locator)
            .join("management.v1.db");
        drop(service);
        drop(repositories);
        std::fs::write(&corrupt_global, b"simulated-global-corruption").unwrap();
        let recovery = SqliteManagementRecovery::open(&management_root, "test").unwrap();
        let recovery_export_path = root.join("recovery-local-state.json");
        let recovery_plan = recovery
            .plan_local_state_export(&project_id, &recovery_export_path)
            .unwrap();
        let recovery_export = recovery
            .apply_local_state_export(
                &recovery_export_path,
                &recovery_plan,
                recovery_plan.plan_fingerprint.as_str(),
            )
            .unwrap();
        assert_eq!(
            recovery
                .apply_local_state_export(
                    &recovery_export_path,
                    &recovery_plan,
                    recovery_plan.plan_fingerprint.as_str(),
                )
                .unwrap(),
            recovery_export
        );
        assert_eq!(
            recovery_export.bundle.local_dispositions,
            export.bundle.local_dispositions
        );
        assert_eq!(
            recovery_export.bundle.source_revision_id,
            export.bundle.source_revision_id
        );
        drop(recovery);

        let target_repositories = Arc::new(
            SqliteManagementRepositorySet::open(root.join("target-management"), "test").unwrap(),
        );
        let target = ManagementApplicationService::new(
            target_repositories.clone(),
            bindings,
            Arc::new(LocalArtifactStore::default()),
        );
        target
            .register_project(&source.canonicalize().unwrap(), "target-register")
            .unwrap();
        target.scan_project(&project_id, "target-scan").unwrap();
        let import_plan = target
            .plan_local_state_import(&recovery_export_path)
            .unwrap();
        assert!(import_plan.conflicts.is_empty());
        let imported = target
            .apply_local_state_import(
                &recovery_export_path,
                &import_plan,
                import_plan.plan_fingerprint.as_str(),
            )
            .unwrap();
        assert_eq!(
            target
                .apply_local_state_import(
                    &recovery_export_path,
                    &import_plan,
                    import_plan.plan_fingerprint.as_str(),
                )
                .unwrap(),
            imported
        );
        assert_eq!(imported.imported_dispositions, 1);
        assert_eq!(
            target_repositories
                .project(&project_id)
                .unwrap()
                .list_dispositions()
                .unwrap(),
            vec![disposition]
        );
        let duplicate_plan = target
            .plan_local_state_import(&recovery_export_path)
            .unwrap();
        assert!(!duplicate_plan.conflicts.is_empty());
        assert!(matches!(
            target.apply_local_state_import(
                &recovery_export_path,
                &duplicate_plan,
                duplicate_plan.plan_fingerprint.as_str(),
            ),
            Err(ApplicationError::Repository(RepositoryError {
                category: RepositoryErrorCategory::RevisionConflict,
                ..
            }))
        ));
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

        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .unwrap()
            .to_path_buf();
        let policy_path = workspace_root.join("catalog/policies/rust-style.toml");
        let service = ManagementApplicationService::new(
            Arc::new(SqliteManagementRepositorySet::open(root.join("management"), "test").unwrap()),
            Arc::new(WindowsProjectRootBindingStore::open(root.join("root-bindings")).unwrap()),
            Arc::new(LocalArtifactStore::default()),
        )
        .with_syntax_adapter(Arc::new(FixtureRustSyntaxAdapter))
        .with_profile_catalog_root(workspace_root.join("catalog/profiles"))
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
            .auto_apply_rust_style(&project_id, scope.clone(), |request| {
                rust_style::seal_rust_style_policy_approval_decision(
                    RustStylePolicyApprovalDecision {
                        schema_id: star_contracts::rust_style::RUST_STYLE_POLICY_APPROVAL_DECISION_SCHEMA_ID
                            .to_owned(),
                        schema_version: 1,
                        contract_version: 1,
                        approval_id: star_contracts::ids::ApprovalId::new(),
                        scope_hash: Sha256Hash::digest(b"persisted-policy-scope"),
                        request_fingerprint: request.request_fingerprint.clone(),
                        decision: star_contracts::fixed_mcp::ApprovalDecision::Approve,
                        resolved_at: Utc::now().to_rfc3339(),
                        decision_fingerprint: Sha256Hash::digest(b"pending-policy-decision"),
                    },
                )
                .map_err(|error| ApplicationError::RustStyle(error.into()))
            })
            .unwrap();
        assert!(result.permit_automatic);
        assert!(result.policy_approval_request.is_some());
        assert_eq!(
            result
                .policy_approval_decision
                .as_ref()
                .map(|decision| decision.decision),
            Some(star_contracts::fixed_mcp::ApprovalDecision::Approve)
        );
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
        assert!(result.applied.is_none());
        assert!(
            result
                .prepared
                .candidate_build
                .as_ref()
                .is_some_and(|run| run.success && run.exit_code == Some(0))
        );
        assert!(
            result
                .prepared
                .candidate_test_compile
                .as_ref()
                .is_some_and(|run| run.success && run.exit_code == Some(0))
        );
        let prepared_v2 = result.prepared.prepared_change_v2.as_ref().unwrap();
        assert_eq!(
            prepared_v2.planning_bundle.validation_plan.readiness,
            ValidationPlanV2Readiness::Ready
        );
        let profile_resolution = prepared_v2
            .planning_bundle
            .validation_plan
            .profile_resolution
            .as_ref()
            .unwrap();
        assert_eq!(
            profile_resolution.selected_profiles[0].profile_id,
            "rust_style_auto_fix"
        );
        assert!(
            profile_resolution
                .parent_closure
                .iter()
                .any(|profile| profile.profile_id == "refactor_codemod")
        );
        assert!(
            profile_resolution
                .required_check_families
                .iter()
                .all(|family| prepared_v2
                    .planning_bundle
                    .validation_plan
                    .required_checks
                    .iter()
                    .any(|check| check.family == *family))
        );
        assert_eq!(prepared_v2.patch_set.state, PatchSetStateV2::Ready);
        let applied = result.applied_v2.unwrap();
        assert_eq!(applied.application.state, PatchApplicationStateV1::Applied);
        assert_eq!(
            applied.compatibility_patch_set.status,
            PatchSetStatus::Applied
        );
        assert_eq!(
            applied.pre_gate_decision.decision,
            GateDecisionKind::AutoPass
        );
        assert_eq!(
            applied
                .post_gate_decision
                .as_ref()
                .map(|decision| decision.decision),
            Some(GateDecisionKind::AutoPass)
        );
        assert!(applied.source_effect_started);
        assert!(!applied.recovered);
        let execution_status = service.validation_execution_status(&project_id).unwrap();
        assert!(execution_status.gate_count >= 2);
        assert!(execution_status.evidence_bundle_count >= 2);
        assert_eq!(
            std::fs::read_to_string(source.join("src/lib.rs")).unwrap(),
            "pub fn answer() -> u32 {\n    42\n}\n"
        );
        assert_eq!(
            std::fs::read(source.join("user-change.txt")).unwrap(),
            b"preserve\n"
        );

        let second = service
            .auto_apply_rust_style(&project_id, scope, |_| {
                panic!("no-op candidate must not request a policy approval")
            })
            .unwrap();
        assert!(second.applied.is_none());
        assert!(second.applied_v2.is_none());
        assert!(second.prepared.prepared_change_v2.is_none());
        assert!(second.prepared.candidate_build.is_none());
        assert!(second.prepared.candidate_test_compile.is_none());
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
        let stale_patch = stale_candidate
            .prepared_change_v2
            .as_ref()
            .unwrap()
            .patch_set
            .clone();
        std::fs::create_dir_all(source.join(".cargo")).unwrap();
        std::fs::write(source.join(".cargo/config.toml"), "[net]\noffline = true\n").unwrap();
        assert!(matches!(
            service.apply_patch_v2(
                &stale_patch.patch_set_id,
                stale_patch.patch_fingerprint.as_str(),
                ActorRef {
                    actor_type: ActorType::User,
                    actor_id: "stale-rust-style-test".to_owned(),
                    display_name: "Stale Rust Style Test".to_owned(),
                    auth_source: "test".to_owned(),
                },
                None,
                None,
            ),
            Err(ApplicationError::IndexNotCurrent)
        ));
        assert_eq!(std::fs::read(source.join("src/lib.rs")).unwrap(), original);
    }
}
