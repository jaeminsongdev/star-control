//! Exact-precondition PatchSet preparation, application, and safe rollback.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
pub use star_contracts::development_effect::{DevelopmentEffectKind, DevelopmentEffectState};
use star_contracts::{
    Sha256Hash,
    evidence::{ArtifactRef, CatalogRef},
    evidence_v2::{InvocationWorkingDirectoryV2, TASK_INVOCATION_V2_SCHEMA_ID, TaskInvocationV2},
    ids::{ChangePlanId, PatchSetId, TaskInvocationId},
    management::{
        ChangePlan, ChangePlanStatus, ChangeRecipe, ChangeRecipeRef, FileOperationKind, Finding,
        Occurrence, PatchFileOperation, PatchSet, PatchSetStatus, ProjectPathRef,
        WorkspaceSnapshot,
    },
    patch_v2::{
        CHANGE_RECIPE_V2_SCHEMA_ID, ChangeRecipeV2, PatchOperationKindV2, PatchSetV2,
        RewriteAssuranceV2, TargetSelectorKindV2,
    },
};
use star_domain::versioned_fingerprint;
use star_ports::{
    MaterializedRewrite, PatchPortError, RewriteTransformRequest, RewriteTransformResult,
    RewriteTransformerPort, SourceMutationObservation, SourceMutationPort, SourceMutationRequest,
    SourceMutationResult, SourceMutationState, ToolExecutionRequest, ToolExecutionResult,
    ToolExecutorPort, WorktreeMaterialization, WorktreePort,
};
use star_validation::{
    permit::PatchPermitUseV2,
    process_executor::{RegisteredProcessCheckExecutor, ResolvedExecutableV2},
    runner::CheckExecutor,
};
use thiserror::Error;
use windows::{
    Win32::Storage::FileSystem::{REPLACEFILE_WRITE_THROUGH, ReplaceFileW},
    core::{HSTRING, PCWSTR},
};

pub mod rust_style;

pub const RECIPE_ID: &str = "star.recipe.remove-trailing-whitespace";
pub const RECIPE_VERSION: &str = "1.0.0";
pub const EXACT_REVERSE_RECIPE_ID: &str = "star.recipe.exact-reverse-recovery";
pub const MANAGED_DECLARATION_RECIPE_ID: &str = "star.recipe.managed-declaration-change";

pub fn trailing_whitespace_recipe() -> Result<ChangeRecipe, ExecutionError> {
    let definition_fingerprint = versioned_fingerprint(
        "star.change-recipe-definition",
        1,
        &serde_json::json!({
            "recipe_id":RECIPE_ID,
            "recipe_version":RECIPE_VERSION,
            "finding_selectors":["rule:star.rule.trailing-whitespace"],
            "preconditions":["exact_before_sha256","no_target_overlap","explicit_apply"],
            "transformer_ref":"builtin.remove-trailing-whitespace.v1",
            "validation_requirements":["complete_rescan","affected_finding_absent"],
            "rollback_contract":"exact_after_sha256_then_restore_original_bytes",
        }),
    )
    .map_err(|_| ExecutionError::Fingerprint)?;
    Ok(ChangeRecipe {
        schema_id: "star.change-recipe".to_owned(),
        schema_version: 1,
        recipe_id: RECIPE_ID.to_owned(),
        recipe_version: RECIPE_VERSION.to_owned(),
        definition_fingerprint,
        finding_selectors: vec!["rule:star.rule.trailing-whitespace".to_owned()],
        preconditions: vec![
            "exact_before_sha256".to_owned(),
            "no_target_overlap".to_owned(),
            "explicit_apply".to_owned(),
        ],
        parameter_schema_ref: "star.recipe.remove-trailing-whitespace.parameters.v1".to_owned(),
        transformer_ref: "builtin.remove-trailing-whitespace.v1".to_owned(),
        allowed_path_scope: vec!["**/*".to_owned()],
        idempotency_contract: "already_trimmed_is_no_change".to_owned(),
        validation_requirements: vec![
            "complete_rescan".to_owned(),
            "affected_finding_absent".to_owned(),
        ],
        risk_class: "local_write".to_owned(),
        permission_actions: vec!["local_write".to_owned()],
        rollback_contract: "exact_after_sha256_then_restore_original_bytes".to_owned(),
    })
}

pub fn trailing_whitespace_recipe_v2() -> Result<ChangeRecipeV2, ExecutionError> {
    ChangeRecipeV2 {
        schema_id: CHANGE_RECIPE_V2_SCHEMA_ID.to_owned(),
        schema_version: 2,
        recipe_id: RECIPE_ID.to_owned(),
        recipe_version: "2.0.0".to_owned(),
        display_name: "Remove trailing whitespace".to_owned(),
        language: None,
        selector_kinds: vec![TargetSelectorKindV2::Finding, TargetSelectorKindV2::Path],
        rewrite_assurance: RewriteAssuranceV2::TextExact,
        parameter_schema: serde_json::json!({
            "type":"object",
            "properties":{},
            "additionalProperties":false
        }),
        transformer_ref: "builtin.remove-trailing-whitespace.v2".to_owned(),
        allowed_path_patterns: vec!["**/*".to_owned()],
        intended_postconditions: vec!["selected_lines_have_no_trailing_whitespace".to_owned()],
        validation_families: vec!["build".to_owned(), "test".to_owned()],
        permission_actions: vec!["local_write".to_owned()],
        idempotence_contract: "replay_on_preview_after_bytes_produces_zero_operations".to_owned(),
        rollback_contract: "exact_after_sha256_then_restore_reverse_artifact".to_owned(),
        definition_fingerprint: Sha256Hash::digest(b""),
    }
    .seal()
    .map_err(|_| ExecutionError::InvalidArtifact)
}

pub fn rust_style_recipe_v2() -> Result<ChangeRecipeV2, ExecutionError> {
    ChangeRecipeV2 {
        schema_id: CHANGE_RECIPE_V2_SCHEMA_ID.to_owned(),
        schema_version: 2,
        recipe_id: "rust_style_v1".to_owned(),
        recipe_version: "1.0.0".to_owned(),
        display_name: "Apply the fixed Rust style pipeline".to_owned(),
        language: Some("rust".to_owned()),
        selector_kinds: vec![TargetSelectorKindV2::Path],
        rewrite_assurance: RewriteAssuranceV2::TextExact,
        parameter_schema: serde_json::json!({
            "type":"object",
            "properties":{},
            "additionalProperties":false
        }),
        transformer_ref: "builtin.rust-style-v1.prevalidated-exact-bytes".to_owned(),
        allowed_path_patterns: vec!["**/*.rs".to_owned()],
        intended_postconditions: vec![
            "rustfmt_and_allowlisted_clippy_replay_produces_zero_operations".to_owned(),
        ],
        validation_families: vec![
            "format".to_owned(),
            "lint".to_owned(),
            "build".to_owned(),
            "test".to_owned(),
        ],
        permission_actions: vec!["local_write".to_owned()],
        idempotence_contract:
            "rust_style_v1_replay_on_exact_candidate_bytes_produces_zero_operations".to_owned(),
        rollback_contract: "exact_after_sha256_then_restore_reverse_artifact".to_owned(),
        definition_fingerprint: Sha256Hash::digest(b""),
    }
    .seal()
    .map_err(|_| ExecutionError::InvalidArtifact)
}

/// Internal recovery recipe used only for an already-materialized reverse
/// PatchSet. It never performs discovery or an unbounded text replacement.
pub fn exact_reverse_recipe_v2() -> Result<ChangeRecipeV2, ExecutionError> {
    ChangeRecipeV2 {
        schema_id: CHANGE_RECIPE_V2_SCHEMA_ID.to_owned(),
        schema_version: 2,
        recipe_id: EXACT_REVERSE_RECIPE_ID.to_owned(),
        recipe_version: "1.0.0".to_owned(),
        display_name: "Restore exact pre-apply bytes".to_owned(),
        language: None,
        selector_kinds: vec![TargetSelectorKindV2::Path],
        rewrite_assurance: RewriteAssuranceV2::TextExact,
        parameter_schema: serde_json::json!({
            "type":"object",
            "properties":{},
            "additionalProperties":false
        }),
        transformer_ref: "builtin.exact-reverse-recovery.v1".to_owned(),
        allowed_path_patterns: vec!["**/*".to_owned()],
        intended_postconditions: vec!["each_path_matches_exact_pre_apply_sha256".to_owned()],
        validation_families: vec!["build".to_owned(), "test".to_owned()],
        permission_actions: vec!["local_write".to_owned()],
        idempotence_contract: "restored_before_hash_is_reported_as_already_satisfied_without_write"
            .to_owned(),
        rollback_contract: "exact_restored_hash_then_reapply_forward_artifact".to_owned(),
        definition_fingerprint: Sha256Hash::digest(b""),
    }
    .seal()
    .map_err(|_| ExecutionError::InvalidArtifact)
}

pub fn managed_declaration_recipe_v2() -> Result<ChangeRecipeV2, ExecutionError> {
    ChangeRecipeV2 {
        schema_id: CHANGE_RECIPE_V2_SCHEMA_ID.to_owned(),
        schema_version: 2,
        recipe_id: MANAGED_DECLARATION_RECIPE_ID.to_owned(),
        recipe_version: "1.0.0".to_owned(),
        display_name: "Apply a typed managed declaration intent".to_owned(),
        language: None,
        selector_kinds: vec![
            TargetSelectorKindV2::ManagedDeclaration,
            TargetSelectorKindV2::Path,
        ],
        rewrite_assurance: RewriteAssuranceV2::TextExact,
        parameter_schema: serde_json::json!({
            "type":"object",
            "required":["intent"],
            "properties":{
                "intent":{"type":"object"}
            },
            "additionalProperties":false
        }),
        transformer_ref: "managed-registry.typed-intent.v1".to_owned(),
        allowed_path_patterns: vec![".star-control/registry/**".to_owned()],
        intended_postconditions: vec![
            "registry_manifest_matches_typed_intent".to_owned(),
            "registry_rebuild_is_current_and_consistent".to_owned(),
        ],
        validation_families: vec![
            "architecture".to_owned(),
            "consumer_compatibility".to_owned(),
            "contract".to_owned(),
            "docs_contract_drift".to_owned(),
            "generated_consistency".to_owned(),
            "hardcoding".to_owned(),
            "managed_registry_contract".to_owned(),
            "test".to_owned(),
            "validator_guard".to_owned(),
        ],
        permission_actions: vec!["local_write".to_owned()],
        idempotence_contract:
            "replay_of_the_same_typed_intent_on_after_bytes_produces_zero_operations".to_owned(),
        rollback_contract: "exact_after_sha256_then_restore_reverse_artifact".to_owned(),
        definition_fingerprint: Sha256Hash::digest(b""),
    }
    .seal()
    .map_err(|_| ExecutionError::InvalidArtifact)
}

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("PatchSet input is stale or inconsistent")]
    Stale,
    #[error("PatchSet path is unsafe")]
    UnsafePath,
    #[error("PatchSet source I/O failed")]
    Io,
    #[error("PatchSet fingerprint failed")]
    Fingerprint,
    #[error("PatchSet artifact is invalid")]
    InvalidArtifact,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WhitespaceKind {
    Space,
    Tab,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LineEdit {
    line: u32,
    trailing: Vec<WhitespaceKind>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRecipe {
    path: ProjectPathRef,
    before_sha256: Sha256Hash,
    after_sha256: Sha256Hash,
    edits: Vec<LineEdit>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PatchRecipeArtifact {
    schema_version: u32,
    recipe_id: String,
    recipe_version: String,
    recipe_definition_fingerprint: Sha256Hash,
    files: Vec<FileRecipe>,
}

pub struct PreparedPatch {
    pub change_plan: ChangePlan,
    pub patch_set: PatchSet,
    pub recipe_artifact: serde_json::Value,
}

pub fn prepare_exact_materialized_patch(
    project_id: &star_contracts::ids::ProjectId,
    snapshot: &WorkspaceSnapshot,
    recipe: &ChangeRecipeV2,
    files: &[MaterializedPatchFile],
    parameters: BTreeMap<String, String>,
) -> Result<PreparedPatch, ExecutionError> {
    if files.is_empty()
        || files.windows(2).any(|pair| pair[0].path >= pair[1].path)
        || files.iter().any(|file| {
            file.before_sha256 != Sha256Hash::digest(&file.before_bytes)
                || file.after_sha256 != Sha256Hash::digest(&file.after_bytes)
                || file.before_sha256 == file.after_sha256
        })
    {
        return Err(ExecutionError::InvalidArtifact);
    }
    let operations = files
        .iter()
        .map(|file| {
            let operation_fingerprint = versioned_fingerprint(
                "star.patch-file-operation",
                1,
                &serde_json::json!({
                    "kind":"modify",
                    "path":file.path,
                    "before_sha256":file.before_sha256,
                    "after_sha256":file.after_sha256,
                }),
            )
            .map_err(|_| ExecutionError::Fingerprint)?;
            Ok(PatchFileOperation {
                kind: FileOperationKind::Modify,
                path: file.path.clone(),
                rename_from: None,
                before_sha256: Some(file.before_sha256.clone()),
                after_sha256: Some(file.after_sha256.clone()),
                before_mode: None,
                after_mode: None,
                operation_fingerprint,
            })
        })
        .collect::<Result<Vec<_>, ExecutionError>>()?;
    let recipe_artifact = serde_json::json!({
        "schema_id":"star.v2-only-exact-patch-compatibility-artifact",
        "schema_version":1,
        "recipe_id":recipe.recipe_id,
        "recipe_version":recipe.recipe_version,
        "recipe_definition_fingerprint":recipe.definition_fingerprint,
        "files":files.iter().map(|file| serde_json::json!({
            "path":file.path,
            "before_sha256":file.before_sha256,
            "after_sha256":file.after_sha256,
        })).collect::<Vec<_>>(),
    });
    let plan_id = ChangePlanId::new();
    let now = Utc::now();
    let change_plan = ChangePlan {
        schema_id: "star.change-plan".to_owned(),
        schema_version: 1,
        change_plan_id: plan_id.clone(),
        revision: 1,
        project_id: project_id.clone(),
        target_workspace_snapshot_id: snapshot.workspace_snapshot_id.clone(),
        finding_refs: Vec::new(),
        recipe_refs: vec![ChangeRecipeRef {
            recipe_id: recipe.recipe_id.clone(),
            recipe_version: recipe.recipe_version.clone(),
            definition_fingerprint: recipe.definition_fingerprint.clone(),
        }],
        parameters,
        expected_paths: files.iter().map(|file| file.path.clone()).collect(),
        preconditions: files
            .iter()
            .map(|file| file.before_sha256.clone())
            .collect(),
        risk: "managed_contract_local_write".to_owned(),
        permission_plan_ref: "local_write.explicit_apply".to_owned(),
        validation_plan_ref: "star.validation.managed-registry.v1".to_owned(),
        status: ChangePlanStatus::Ready,
        created_at: now,
        updated_at: now,
    };
    let provisional = versioned_fingerprint("star.patch-set.provisional", 1, &recipe_artifact)
        .map_err(|_| ExecutionError::Fingerprint)?;
    Ok(PreparedPatch {
        change_plan,
        patch_set: PatchSet {
            schema_id: "star.patch-set".to_owned(),
            schema_version: 1,
            patch_set_id: PatchSetId::new(),
            change_plan_id: plan_id,
            change_plan_revision: 1,
            project_id: project_id.clone(),
            base_workspace_snapshot_id: snapshot.workspace_snapshot_id.clone(),
            patch_fingerprint: provisional,
            operations,
            patch_artifact_refs: Vec::new(),
            affected_finding_ids: Vec::new(),
            expected_result_fingerprint: None,
            status: PatchSetStatus::Proposed,
            applied_workspace_snapshot_id: None,
            rollback_artifact_refs: Vec::new(),
        },
        recipe_artifact,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterializedPatchFile {
    pub path: ProjectPathRef,
    pub before_bytes: Vec<u8>,
    pub after_bytes: Vec<u8>,
    pub before_sha256: Sha256Hash,
    pub after_sha256: Sha256Hash,
}

pub struct PreparedPatchTransformerAdapter<'a> {
    prepared: &'a PreparedPatch,
}

impl<'a> PreparedPatchTransformerAdapter<'a> {
    pub fn new(prepared: &'a PreparedPatch) -> Self {
        Self { prepared }
    }
}

impl RewriteTransformerPort for PreparedPatchTransformerAdapter<'_> {
    fn materialize(
        &self,
        project_root: &Path,
        request: &RewriteTransformRequest,
    ) -> Result<RewriteTransformResult, PatchPortError> {
        let expected_recipe =
            trailing_whitespace_recipe_v2().map_err(|_| PatchPortError::Invalid)?;
        if request.recipe != expected_recipe
            || request.target_selector.project_id() != &self.prepared.patch_set.project_id
            || request.target_selector.validate().is_err()
            || request
                .parameters
                .as_object()
                .is_none_or(|parameters| !parameters.is_empty())
        {
            return Err(PatchPortError::Invalid);
        }
        let files = self
            .prepared
            .materialize_preview(project_root)
            .map_err(|error| match error {
                ExecutionError::UnsafePath => PatchPortError::Unsafe,
                ExecutionError::Stale | ExecutionError::InvalidArtifact => PatchPortError::Invalid,
                ExecutionError::Io | ExecutionError::Fingerprint => PatchPortError::Unavailable,
            })?
            .into_iter()
            .map(|file| MaterializedRewrite {
                path: file.path,
                before_sha256: file.before_sha256,
                after_sha256: file.after_sha256,
                before_bytes: file.before_bytes,
                after_bytes: file.after_bytes,
            })
            .collect();
        Ok(RewriteTransformResult {
            files,
            replay_operation_count: 0,
            idempotence_proved: true,
        })
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ExactFileSourceMutationAdapter;

impl SourceMutationPort for ExactFileSourceMutationAdapter {
    type Permit = PatchPermitUseV2;

    fn apply(
        &self,
        project_root: &Path,
        request: &SourceMutationRequest,
        permit: Self::Permit,
    ) -> Result<SourceMutationResult, PatchPortError> {
        if permit.patch_fingerprint != request.patch_set.patch_fingerprint
            || request.patch_set.state != star_contracts::patch_v2::PatchSetStateV2::Ready
            || request.files.len() != request.patch_set.operations.len()
        {
            return Err(PatchPortError::Invalid);
        }
        let mut prepared = Vec::with_capacity(request.files.len());
        for operation in &request.patch_set.operations {
            if operation.kind != PatchOperationKindV2::Modify {
                return Err(PatchPortError::Invalid);
            }
            let file = request
                .files
                .iter()
                .find(|file| file.path == operation.path)
                .ok_or(PatchPortError::Invalid)?;
            if operation.before_sha256.as_ref() != Some(&file.before_sha256)
                || operation.after_sha256.as_ref() != Some(&file.after_sha256)
                || Sha256Hash::digest(&file.before_bytes) != file.before_sha256
                || Sha256Hash::digest(&file.after_bytes) != file.after_sha256
            {
                return Err(PatchPortError::Invalid);
            }
            let path =
                resolve_safe_file(project_root, &file.path).map_err(|_| PatchPortError::Unsafe)?;
            let current = fs::read(&path).map_err(|_| PatchPortError::Unavailable)?;
            if Sha256Hash::digest(&current) != file.before_sha256 {
                return Err(PatchPortError::Invalid);
            }
            prepared.push((path, file.clone()));
        }
        let mut applied: Vec<(PathBuf, Vec<u8>, Sha256Hash)> = Vec::new();
        for (path, file) in &prepared {
            let still_current = fs::read(path)
                .ok()
                .is_some_and(|bytes| Sha256Hash::digest(&bytes) == file.before_sha256);
            if !still_current || replace_file_atomic(path, &file.after_bytes).is_err() {
                return if rollback_originals(&applied) {
                    Err(PatchPortError::Unavailable)
                } else {
                    Err(PatchPortError::Partial)
                };
            }
            applied.push((
                path.clone(),
                file.before_bytes.clone(),
                file.after_sha256.clone(),
            ));
        }
        Ok(SourceMutationResult {
            state: SourceMutationState::AppliedExact,
            observations: request
                .files
                .iter()
                .map(|file| SourceMutationObservation {
                    path: file.path.clone(),
                    observed_sha256: Some(file.after_sha256.clone()),
                })
                .collect(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct GitWorktreeAdapter {
    base_directory: PathBuf,
}

impl GitWorktreeAdapter {
    pub fn new(base_directory: PathBuf) -> Result<Self, PatchPortError> {
        if base_directory.as_os_str().is_empty() || base_directory.parent().is_none() {
            return Err(PatchPortError::Unsafe);
        }
        Ok(Self { base_directory })
    }

    fn target_path(&self, decision: &star_contracts::patch_v2::WorktreeDecision) -> PathBuf {
        self.base_directory
            .join(decision.worktree_decision_id.as_str())
    }

    pub fn synchronize_preview_inputs(
        &self,
        materialization: &WorktreeMaterialization,
        files: &[MaterializedRewrite],
    ) -> Result<(), PatchPortError> {
        self.verify_materialization_path(materialization)?;
        for file in files {
            let path = resolve_safe_file(&materialization.root, &file.path)
                .map_err(|_| PatchPortError::Unsafe)?;
            if replace_file_atomic(&path, &file.before_bytes).is_err() {
                return Err(PatchPortError::Unavailable);
            }
            let observed = fs::read(&path).map_err(|_| PatchPortError::Unavailable)?;
            if Sha256Hash::digest(&observed) != file.before_sha256 {
                return Err(PatchPortError::OutcomeUnknown);
            }
        }
        Ok(())
    }

    fn verify_materialization_path(
        &self,
        materialization: &WorktreeMaterialization,
    ) -> Result<(), PatchPortError> {
        let base = fs::canonicalize(&self.base_directory).map_err(|_| PatchPortError::Unsafe)?;
        let root = fs::canonicalize(&materialization.root).map_err(|_| PatchPortError::Unsafe)?;
        if root.parent() != Some(base.as_path()) || root == base {
            return Err(PatchPortError::Unsafe);
        }
        Ok(())
    }
}

impl WorktreePort for GitWorktreeAdapter {
    fn materialize(
        &self,
        repository_root: &Path,
        decision: &star_contracts::patch_v2::WorktreeDecision,
    ) -> Result<WorktreeMaterialization, PatchPortError> {
        if decision.strategy != star_contracts::patch_v2::WorktreeStrategyV1::Isolated
            || decision.state != star_contracts::patch_v2::WorktreeDecisionStateV1::Selected
        {
            return Err(PatchPortError::Invalid);
        }
        let repository_root =
            fs::canonicalize(repository_root).map_err(|_| PatchPortError::Unavailable)?;
        fs::create_dir_all(&self.base_directory).map_err(|_| PatchPortError::Unavailable)?;
        let target = self.target_path(decision);
        if fs::symlink_metadata(&target).is_ok() {
            return Err(PatchPortError::Unsafe);
        }
        let head = Command::new("git")
            .current_dir(&repository_root)
            .args(["rev-parse", "--verify", "HEAD^{commit}"])
            .output()
            .map_err(|_| PatchPortError::Unavailable)?;
        if !head.status.success() {
            return Err(PatchPortError::Unavailable);
        }
        let commit = String::from_utf8(head.stdout).map_err(|_| PatchPortError::Invalid)?;
        let commit = commit.trim();
        if commit.len() != 40 && commit.len() != 64 {
            return Err(PatchPortError::Invalid);
        }
        let status = Command::new("git")
            .current_dir(&repository_root)
            .arg("worktree")
            .arg("add")
            .arg("--detach")
            .arg(&target)
            .arg(commit)
            .status()
            .map_err(|_| PatchPortError::Unavailable)?;
        if !status.success() {
            return Err(PatchPortError::Unavailable);
        }
        let root = fs::canonicalize(&target).map_err(|_| PatchPortError::OutcomeUnknown)?;
        let repository_fingerprint =
            Sha256Hash::digest(repository_root.to_string_lossy().as_bytes());
        let locator_fingerprint = versioned_fingerprint(
            "star.git-worktree-locator",
            1,
            &serde_json::json!({
                "repository_root_fingerprint":repository_fingerprint,
                "worktree_decision_id":decision.worktree_decision_id,
                "commit":commit,
                "target_name":decision.worktree_decision_id.as_str(),
            }),
        )
        .map_err(|_| PatchPortError::Invalid)?;
        Ok(WorktreeMaterialization {
            root,
            locator_fingerprint,
            evidence_refs: vec![],
        })
    }

    fn discard(
        &self,
        repository_root: &Path,
        materialization: &WorktreeMaterialization,
    ) -> Result<(), PatchPortError> {
        self.verify_materialization_path(materialization)?;
        let status = Command::new("git")
            .current_dir(repository_root)
            .arg("worktree")
            .arg("remove")
            .arg("--force")
            .arg(&materialization.root)
            .status()
            .map_err(|_| PatchPortError::Unavailable)?;
        if status.success() {
            Ok(())
        } else {
            Err(PatchPortError::OutcomeUnknown)
        }
    }
}

#[derive(Clone, Debug)]
pub struct RegisteredToolBinding {
    pub tool_ref: CatalogRef,
    pub executable: ResolvedExecutableV2,
    pub allowed_permission_actions: BTreeSet<String>,
}

pub struct RegisteredToolExecutorAdapter {
    executor: Mutex<RegisteredProcessCheckExecutor>,
    bindings: BTreeMap<Sha256Hash, RegisteredToolBinding>,
}

impl RegisteredToolExecutorAdapter {
    pub fn new(bindings: Vec<RegisteredToolBinding>) -> Result<Self, PatchPortError> {
        let by_fingerprint = bindings
            .iter()
            .cloned()
            .map(|binding| {
                (
                    binding.executable.executable_binding_fingerprint.clone(),
                    binding,
                )
            })
            .collect::<BTreeMap<_, _>>();
        if by_fingerprint.is_empty()
            || by_fingerprint.len() != bindings.len()
            || by_fingerprint
                .values()
                .any(|binding| binding.allowed_permission_actions.is_empty())
        {
            return Err(PatchPortError::Invalid);
        }
        let executor = RegisteredProcessCheckExecutor::new(
            bindings
                .into_iter()
                .map(|binding| binding.executable)
                .collect(),
        )
        .map_err(|_| PatchPortError::Invalid)?;
        Ok(Self {
            executor: Mutex::new(executor),
            bindings: by_fingerprint,
        })
    }
}

impl ToolExecutorPort for RegisteredToolExecutorAdapter {
    fn execute(
        &self,
        request: &ToolExecutionRequest,
    ) -> Result<ToolExecutionResult, PatchPortError> {
        let binding = self
            .bindings
            .get(&request.executable_binding_fingerprint)
            .ok_or(PatchPortError::Invalid)?;
        if binding.tool_ref != request.tool_ref
            || binding.executable.logical_executable != request.logical_executable
            || !binding
                .allowed_permission_actions
                .contains(&request.permission_action)
        {
            return Err(PatchPortError::Invalid);
        }
        let requested_cwd =
            fs::canonicalize(&request.working_directory).map_err(|_| PatchPortError::Unsafe)?;
        let cwd = if requested_cwd == binding.executable.project_root {
            InvocationWorkingDirectoryV2::ProjectRoot
        } else {
            let relative = requested_cwd
                .strip_prefix(&binding.executable.project_root)
                .map_err(|_| PatchPortError::Unsafe)?;
            let relative = relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            InvocationWorkingDirectoryV2::ProjectPath {
                path: ProjectPathRef::parse(relative).map_err(|_| PatchPortError::Unsafe)?,
            }
        };
        let invocation = TaskInvocationV2 {
            schema_id: TASK_INVOCATION_V2_SCHEMA_ID.to_owned(),
            schema_version: 2,
            invocation_id: TaskInvocationId::new(),
            tool_ref: request.tool_ref.clone(),
            executable: request.logical_executable.clone(),
            executable_binding_fingerprint: request.executable_binding_fingerprint.clone(),
            args: request.args.clone(),
            cwd,
            env_refs: BTreeMap::new(),
            stdin_ref: None,
            timeout_ms: request.timeout_ms,
            permission_action: request.permission_action.clone(),
            idempotency_key: format!(
                "codemod-{}",
                request
                    .input_fingerprint
                    .as_str()
                    .trim_start_matches("sha256:")
            ),
            expected_exit_codes: request.expected_exit_codes.clone(),
            output_limits: request.output_limits.clone(),
            input_fingerprint: request.input_fingerprint.clone(),
        }
        .seal()
        .map_err(|_| PatchPortError::Invalid)?;
        if invocation.input_fingerprint != request.input_fingerprint {
            return Err(PatchPortError::Invalid);
        }
        let observation = self
            .executor
            .lock()
            .map_err(|_| PatchPortError::OutcomeUnknown)?
            .execute(&invocation)
            .map_err(|error| match error.termination_reason {
                star_contracts::evidence::TerminationReason::OutcomeUnknown => {
                    PatchPortError::OutcomeUnknown
                }
                _ => PatchPortError::Unavailable,
            })?;
        let observed_tool = observation
            .observed_tool
            .ok_or(PatchPortError::OutcomeUnknown)?;
        let success = observation.termination_reason
            == star_contracts::evidence::TerminationReason::Exited
            && observation
                .exit_code
                .is_some_and(|code| request.expected_exit_codes.contains(&code))
            && observation.completeness == star_contracts::evidence::Completeness::Complete
            && observation
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.blocking);
        Ok(ToolExecutionResult {
            exit_code: observation.exit_code,
            termination_reason: observation.termination_reason,
            completeness: observation.completeness,
            success,
            output_artifact_refs: observation.artifact_refs,
            observed_executable_fingerprint: observed_tool.sha256,
        })
    }
}

#[derive(Clone, Debug)]
pub struct DevelopmentEffectRequest {
    pub kind: DevelopmentEffectKind,
    pub tool: ToolExecutionRequest,
    pub expected_executable_sha256: Sha256Hash,
    pub exact_subject_fingerprint: Sha256Hash,
    pub approval_ref: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DevelopmentEffectObservation {
    pub kind: DevelopmentEffectKind,
    pub exact_subject_fingerprint: Sha256Hash,
    pub state: DevelopmentEffectState,
    pub source_effect_started: bool,
    pub approval_ref: Option<String>,
    pub execution: ToolExecutionResult,
}

/// Shared M7-M9 effect adapter. It does not decide policy or synthesize a
/// typed domain result; it consumes an already selected, hash-bound executable
/// and returns only bounded execution evidence for the owning workflow.
pub struct RegisteredDevelopmentEffectAdapter<'a, E: ToolExecutorPort> {
    executor: &'a E,
}

pub trait DevelopmentEffectPort {
    fn execute_effect(
        &self,
        request: DevelopmentEffectRequest,
    ) -> Result<DevelopmentEffectObservation, PatchPortError>;
}

impl<'a, E: ToolExecutorPort> RegisteredDevelopmentEffectAdapter<'a, E> {
    pub fn new(executor: &'a E) -> Self {
        Self { executor }
    }

    pub fn run(
        &self,
        request: DevelopmentEffectRequest,
    ) -> Result<DevelopmentEffectObservation, PatchPortError> {
        if request.tool.permission_action != request.kind.permission_action()
            || request.exact_subject_fingerprint != request.tool.input_fingerprint
            || request.kind.requires_approval()
                && request
                    .approval_ref
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
            || !request.kind.requires_approval() && request.approval_ref.is_some()
        {
            return Err(PatchPortError::Invalid);
        }
        let execution = self.executor.execute(&request.tool)?;
        if execution.observed_executable_fingerprint != request.expected_executable_sha256 {
            return Err(PatchPortError::OutcomeUnknown);
        }
        let state = match (
            execution.termination_reason,
            execution.completeness,
            execution.success,
        ) {
            (
                star_contracts::evidence::TerminationReason::Exited,
                star_contracts::evidence::Completeness::Complete,
                true,
            ) => DevelopmentEffectState::Succeeded,
            (star_contracts::evidence::TerminationReason::OutcomeUnknown, _, _)
            | (star_contracts::evidence::TerminationReason::Cancelled, _, _) => {
                DevelopmentEffectState::OutcomeUnknown
            }
            (_, star_contracts::evidence::Completeness::Partial, _) => {
                DevelopmentEffectState::Partial
            }
            (_, star_contracts::evidence::Completeness::Unverified, _) => {
                DevelopmentEffectState::OutcomeUnknown
            }
            _ => DevelopmentEffectState::Failed,
        };
        Ok(DevelopmentEffectObservation {
            kind: request.kind,
            exact_subject_fingerprint: request.exact_subject_fingerprint,
            state,
            source_effect_started: !matches!(
                execution.termination_reason,
                star_contracts::evidence::TerminationReason::LaunchError
            ),
            approval_ref: request.approval_ref,
            execution,
        })
    }
}

impl<E: ToolExecutorPort> DevelopmentEffectPort for RegisteredDevelopmentEffectAdapter<'_, E> {
    fn execute_effect(
        &self,
        request: DevelopmentEffectRequest,
    ) -> Result<DevelopmentEffectObservation, PatchPortError> {
        self.run(request)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatchFilesystemStateV2 {
    Before,
    After,
    Mixed,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReversePatchMaterialV2 {
    pub path: ProjectPathRef,
    pub expected_after_sha256: Sha256Hash,
    pub after_bytes: Vec<u8>,
    pub restore_before_sha256: Sha256Hash,
    pub restore_before_bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReversePatchResultV2 {
    pub restored_paths: Vec<ProjectPathRef>,
}

pub fn observe_patch_set_v2(project_root: &Path, patch_set: &PatchSetV2) -> PatchFilesystemStateV2 {
    let mut before = 0_usize;
    let mut after = 0_usize;
    for operation in &patch_set.operations {
        if !matches!(
            operation.kind,
            PatchOperationKindV2::Modify | PatchOperationKindV2::GeneratorInput
        ) {
            return PatchFilesystemStateV2::Unknown;
        }
        let Ok(path) = resolve_safe_file(project_root, &operation.path) else {
            return PatchFilesystemStateV2::Unknown;
        };
        let Ok(bytes) = fs::read(path) else {
            return PatchFilesystemStateV2::Unknown;
        };
        let hash = Sha256Hash::digest(&bytes);
        if operation.before_sha256.as_ref() == Some(&hash) {
            before += 1;
        } else if operation.after_sha256.as_ref() == Some(&hash) {
            after += 1;
        } else {
            return PatchFilesystemStateV2::Unknown;
        }
    }
    match (before, after) {
        (0, value) if value == patch_set.operations.len() => PatchFilesystemStateV2::After,
        (value, 0) if value == patch_set.operations.len() => PatchFilesystemStateV2::Before,
        _ => PatchFilesystemStateV2::Mixed,
    }
}

pub fn recover_patch_set_v2(
    project_root: &Path,
    patch_set: &PatchSetV2,
    materials: &[ReversePatchMaterialV2],
) -> Result<ReversePatchResultV2, Box<ApplyFailure>> {
    if patch_set.operations.len() != materials.len() {
        return Err(failure(
            legacy_failure_projection(patch_set),
            false,
            "PATCH_RECOVERY_ARTIFACT_MISMATCH",
        ));
    }
    let mut prepared = Vec::new();
    for operation in &patch_set.operations {
        let Some(material) = materials
            .iter()
            .find(|material| material.path == operation.path)
        else {
            return Err(failure(
                legacy_failure_projection(patch_set),
                false,
                "PATCH_RECOVERY_ARTIFACT_MISMATCH",
            ));
        };
        if operation.before_sha256.as_ref() != Some(&material.restore_before_sha256)
            || operation.after_sha256.as_ref() != Some(&material.expected_after_sha256)
            || Sha256Hash::digest(&material.after_bytes) != material.expected_after_sha256
            || Sha256Hash::digest(&material.restore_before_bytes) != material.restore_before_sha256
        {
            return Err(failure(
                legacy_failure_projection(patch_set),
                false,
                "PATCH_RECOVERY_ARTIFACT_MISMATCH",
            ));
        }
        let path = resolve_safe_file(project_root, &operation.path).map_err(|_| {
            failure(
                legacy_failure_projection(patch_set),
                false,
                "PATCH_PATH_UNSAFE",
            )
        })?;
        let current = fs::read(&path).map_err(|_| {
            failure(
                legacy_failure_projection(patch_set),
                false,
                "PATCH_READ_FAILED",
            )
        })?;
        if Sha256Hash::digest(&current) != material.expected_after_sha256 {
            return Err(failure(
                legacy_failure_projection(patch_set),
                false,
                "PATCH_RECOVERY_PRECONDITION_FAILED",
            ));
        }
        prepared.push((path, material.clone()));
    }
    let mut restored = Vec::new();
    for (path, material) in prepared {
        if replace_file_atomic(&path, &material.restore_before_bytes).is_err() {
            let rollback_complete = restored.iter().rev().all(
                |(restored_path, restored_material): &(PathBuf, ReversePatchMaterialV2)| {
                    fs::read(restored_path).ok().is_some_and(|bytes| {
                        Sha256Hash::digest(&bytes) == restored_material.restore_before_sha256
                    }) && replace_file_atomic(restored_path, &restored_material.after_bytes).is_ok()
                },
            );
            return Err(failure(
                legacy_failure_projection(patch_set),
                !rollback_complete,
                "PATCH_RECOVERY_APPLY_FAILED",
            ));
        }
        restored.push((path, material));
    }
    Ok(ReversePatchResultV2 {
        restored_paths: restored
            .into_iter()
            .map(|(_, material)| material.path)
            .collect(),
    })
}

fn legacy_failure_projection(patch_set: &PatchSetV2) -> PatchSet {
    PatchSet {
        schema_id: "star.patch-set".to_owned(),
        schema_version: 1,
        patch_set_id: patch_set.patch_set_id.clone(),
        change_plan_id: patch_set.change_plan_id.clone(),
        change_plan_revision: patch_set.change_plan_revision,
        project_id: patch_set.project_id.clone(),
        base_workspace_snapshot_id: patch_set.base_workspace_snapshot_id.clone(),
        patch_fingerprint: patch_set.patch_fingerprint.clone(),
        operations: vec![],
        patch_artifact_refs: vec![],
        affected_finding_ids: vec![],
        expected_result_fingerprint: None,
        status: PatchSetStatus::Failed,
        applied_workspace_snapshot_id: None,
        rollback_artifact_refs: vec![],
    }
}

impl PreparedPatch {
    pub fn attach_artifact(mut self, artifact: ArtifactRef) -> Result<Self, ExecutionError> {
        let artifact_bytes = serde_json::to_vec_pretty(&self.recipe_artifact)
            .map_err(|_| ExecutionError::InvalidArtifact)?;
        if artifact.sha256 != Sha256Hash::digest(&artifact_bytes) {
            return Err(ExecutionError::InvalidArtifact);
        }
        let patch_fingerprint = versioned_fingerprint(
            "star.patch-set",
            1,
            &serde_json::json!({
                "project_id":self.patch_set.project_id,
                "base_workspace_snapshot_id":self.patch_set.base_workspace_snapshot_id,
                "change_plan_id":self.patch_set.change_plan_id,
                "change_plan_revision":self.patch_set.change_plan_revision,
                "operations":self.patch_set.operations,
                "artifact_sha256":artifact.sha256,
            }),
        )
        .map_err(|_| ExecutionError::Fingerprint)?;
        self.patch_set.patch_fingerprint = patch_fingerprint;
        self.patch_set.patch_artifact_refs = vec![artifact];
        Ok(self)
    }

    pub fn materialize_preview(
        &self,
        project_root: &Path,
    ) -> Result<Vec<MaterializedPatchFile>, ExecutionError> {
        let recipe: PatchRecipeArtifact = serde_json::from_value(self.recipe_artifact.clone())
            .map_err(|_| ExecutionError::InvalidArtifact)?;
        if recipe.files.len() != self.patch_set.operations.len() {
            return Err(ExecutionError::InvalidArtifact);
        }
        let mut files = Vec::with_capacity(recipe.files.len());
        for file in &recipe.files {
            let operation = self
                .patch_set
                .operations
                .iter()
                .find(|operation| operation.path == file.path)
                .ok_or(ExecutionError::InvalidArtifact)?;
            let path = resolve_safe_file(project_root, &file.path)?;
            let before_bytes = fs::read(path).map_err(|_| ExecutionError::Io)?;
            let before_sha256 = Sha256Hash::digest(&before_bytes);
            if before_sha256 != file.before_sha256
                || operation.before_sha256.as_ref() != Some(&before_sha256)
            {
                return Err(ExecutionError::Stale);
            }
            let after_bytes = apply_recipe(&before_bytes, &file.edits)?;
            let after_sha256 = Sha256Hash::digest(&after_bytes);
            if after_sha256 != file.after_sha256
                || operation.after_sha256.as_ref() != Some(&after_sha256)
                || !replay_is_no_change(&after_bytes, &file.edits)?
            {
                return Err(ExecutionError::InvalidArtifact);
            }
            files.push(MaterializedPatchFile {
                path: file.path.clone(),
                before_bytes,
                after_bytes,
                before_sha256,
                after_sha256,
            });
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(files)
    }
}

pub fn prepare_trailing_whitespace_patch(
    project_root: &Path,
    finding: &Finding,
    occurrences: &[Occurrence],
    snapshot: &WorkspaceSnapshot,
) -> Result<PreparedPatch, ExecutionError> {
    let recipe = trailing_whitespace_recipe()?;
    if occurrences.is_empty()
        || occurrences
            .iter()
            .any(|occurrence| occurrence.finding_id != finding.finding_id)
    {
        return Err(ExecutionError::Stale);
    }
    let mut by_path: BTreeMap<ProjectPathRef, Vec<&Occurrence>> = BTreeMap::new();
    for occurrence in occurrences {
        by_path
            .entry(occurrence.location_path.clone())
            .or_default()
            .push(occurrence);
    }
    let mut recipes = Vec::new();
    let mut operations = Vec::new();
    for (path, occurrences) in by_path {
        let source_path = resolve_safe_file(project_root, &path)?;
        let before = fs::read(&source_path).map_err(|_| ExecutionError::Io)?;
        let before_sha256 = Sha256Hash::digest(&before);
        if occurrences
            .iter()
            .any(|occurrence| occurrence.source_content_sha256 != before_sha256)
        {
            return Err(ExecutionError::Stale);
        }
        let target_lines: BTreeSet<_> = occurrences
            .iter()
            .map(|occurrence| occurrence.location_range.start_line)
            .collect();
        let (after, edits) = remove_trailing_whitespace(&before, &target_lines)?;
        if edits.len() != target_lines.len() {
            return Err(ExecutionError::Stale);
        }
        let after_sha256 = Sha256Hash::digest(&after);
        let operation_fingerprint = versioned_fingerprint(
            "star.patch-file-operation",
            1,
            &serde_json::json!({
                "kind":"modify",
                "path":path,
                "before_sha256":before_sha256,
                "after_sha256":after_sha256,
            }),
        )
        .map_err(|_| ExecutionError::Fingerprint)?;
        operations.push(PatchFileOperation {
            kind: FileOperationKind::Modify,
            path: path.clone(),
            rename_from: None,
            before_sha256: Some(before_sha256.clone()),
            after_sha256: Some(after_sha256.clone()),
            before_mode: None,
            after_mode: None,
            operation_fingerprint,
        });
        recipes.push(FileRecipe {
            path,
            before_sha256,
            after_sha256,
            edits,
        });
    }
    recipes.sort_by(|left, right| left.path.cmp(&right.path));
    operations.sort_by(|left, right| left.path.cmp(&right.path));
    let recipe_artifact = PatchRecipeArtifact {
        schema_version: 1,
        recipe_id: recipe.recipe_id.clone(),
        recipe_version: recipe.recipe_version.clone(),
        recipe_definition_fingerprint: recipe.definition_fingerprint.clone(),
        files: recipes,
    };
    let recipe_value =
        serde_json::to_value(&recipe_artifact).map_err(|_| ExecutionError::InvalidArtifact)?;
    let plan_id = ChangePlanId::new();
    let now = Utc::now();
    let change_plan = ChangePlan {
        schema_id: "star.change-plan".to_owned(),
        schema_version: 1,
        change_plan_id: plan_id.clone(),
        revision: 1,
        project_id: finding.project_id.clone(),
        target_workspace_snapshot_id: snapshot.workspace_snapshot_id.clone(),
        finding_refs: vec![finding.finding_id.clone()],
        recipe_refs: vec![ChangeRecipeRef {
            recipe_id: recipe.recipe_id,
            recipe_version: recipe.recipe_version,
            definition_fingerprint: recipe.definition_fingerprint,
        }],
        parameters: BTreeMap::new(),
        expected_paths: operations
            .iter()
            .map(|operation| operation.path.clone())
            .collect(),
        preconditions: operations
            .iter()
            .filter_map(|operation| operation.before_sha256.clone())
            .collect(),
        risk: "local_write".to_owned(),
        permission_plan_ref: "local_write.explicit_apply".to_owned(),
        validation_plan_ref: "star.validation.trailing-whitespace.v1".to_owned(),
        status: ChangePlanStatus::Ready,
        created_at: now,
        updated_at: now,
    };
    let provisional = versioned_fingerprint("star.patch-set.provisional", 1, &recipe_value)
        .map_err(|_| ExecutionError::Fingerprint)?;
    let patch_set = PatchSet {
        schema_id: "star.patch-set".to_owned(),
        schema_version: 1,
        patch_set_id: PatchSetId::new(),
        change_plan_id: plan_id,
        change_plan_revision: 1,
        project_id: finding.project_id.clone(),
        base_workspace_snapshot_id: snapshot.workspace_snapshot_id.clone(),
        patch_fingerprint: provisional,
        operations,
        patch_artifact_refs: vec![],
        affected_finding_ids: vec![finding.finding_id.clone()],
        expected_result_fingerprint: None,
        status: PatchSetStatus::Proposed,
        applied_workspace_snapshot_id: None,
        rollback_artifact_refs: vec![],
    };
    Ok(PreparedPatch {
        change_plan,
        patch_set,
        recipe_artifact: recipe_value,
    })
}

pub fn prepare_trailing_whitespace_paths(
    project_root: &Path,
    project_id: &star_contracts::ids::ProjectId,
    expected_paths: &BTreeMap<ProjectPathRef, Sha256Hash>,
    snapshot: &WorkspaceSnapshot,
) -> Result<PreparedPatch, ExecutionError> {
    let recipe = trailing_whitespace_recipe()?;
    if expected_paths.is_empty() {
        return Err(ExecutionError::Stale);
    }
    let mut recipes = Vec::new();
    let mut operations = Vec::new();
    for (path, expected_before) in expected_paths {
        let source_path = resolve_safe_file(project_root, path)?;
        let before = fs::read(source_path).map_err(|_| ExecutionError::Io)?;
        let before_sha256 = Sha256Hash::digest(&before);
        if &before_sha256 != expected_before {
            return Err(ExecutionError::Stale);
        }
        let target_lines = trailing_whitespace_lines(&before)?;
        if target_lines.is_empty() {
            continue;
        }
        let (after, edits) = remove_trailing_whitespace(&before, &target_lines)?;
        let after_sha256 = Sha256Hash::digest(&after);
        let operation_fingerprint = versioned_fingerprint(
            "star.patch-file-operation",
            1,
            &serde_json::json!({
                "kind":"modify",
                "path":path,
                "before_sha256":before_sha256,
                "after_sha256":after_sha256,
            }),
        )
        .map_err(|_| ExecutionError::Fingerprint)?;
        operations.push(PatchFileOperation {
            kind: FileOperationKind::Modify,
            path: path.clone(),
            rename_from: None,
            before_sha256: Some(before_sha256.clone()),
            after_sha256: Some(after_sha256.clone()),
            before_mode: None,
            after_mode: None,
            operation_fingerprint,
        });
        recipes.push(FileRecipe {
            path: path.clone(),
            before_sha256,
            after_sha256,
            edits,
        });
    }
    if operations.is_empty() {
        return Err(ExecutionError::Stale);
    }
    recipes.sort_by(|left, right| left.path.cmp(&right.path));
    operations.sort_by(|left, right| left.path.cmp(&right.path));
    let recipe_artifact = PatchRecipeArtifact {
        schema_version: 1,
        recipe_id: recipe.recipe_id.clone(),
        recipe_version: recipe.recipe_version.clone(),
        recipe_definition_fingerprint: recipe.definition_fingerprint.clone(),
        files: recipes,
    };
    let recipe_value =
        serde_json::to_value(&recipe_artifact).map_err(|_| ExecutionError::InvalidArtifact)?;
    let plan_id = ChangePlanId::new();
    let now = Utc::now();
    let change_plan = ChangePlan {
        schema_id: "star.change-plan".to_owned(),
        schema_version: 1,
        change_plan_id: plan_id.clone(),
        revision: 1,
        project_id: project_id.clone(),
        target_workspace_snapshot_id: snapshot.workspace_snapshot_id.clone(),
        finding_refs: vec![],
        recipe_refs: vec![ChangeRecipeRef {
            recipe_id: recipe.recipe_id,
            recipe_version: recipe.recipe_version,
            definition_fingerprint: recipe.definition_fingerprint,
        }],
        parameters: BTreeMap::new(),
        expected_paths: operations
            .iter()
            .map(|operation| operation.path.clone())
            .collect(),
        preconditions: operations
            .iter()
            .filter_map(|operation| operation.before_sha256.clone())
            .collect(),
        risk: "local_write".to_owned(),
        permission_plan_ref: "local_write.explicit_apply".to_owned(),
        validation_plan_ref: "star.validation.trailing-whitespace.v2".to_owned(),
        status: ChangePlanStatus::Ready,
        created_at: now,
        updated_at: now,
    };
    let provisional = versioned_fingerprint("star.patch-set.provisional", 1, &recipe_value)
        .map_err(|_| ExecutionError::Fingerprint)?;
    let patch_set = PatchSet {
        schema_id: "star.patch-set".to_owned(),
        schema_version: 1,
        patch_set_id: PatchSetId::new(),
        change_plan_id: plan_id,
        change_plan_revision: 1,
        project_id: project_id.clone(),
        base_workspace_snapshot_id: snapshot.workspace_snapshot_id.clone(),
        patch_fingerprint: provisional,
        operations,
        patch_artifact_refs: vec![],
        affected_finding_ids: vec![],
        expected_result_fingerprint: None,
        status: PatchSetStatus::Proposed,
        applied_workspace_snapshot_id: None,
        rollback_artifact_refs: vec![],
    };
    Ok(PreparedPatch {
        change_plan,
        patch_set,
        recipe_artifact: recipe_value,
    })
}

#[derive(Debug)]
pub struct AppliedPatch {
    pub patch_set: PatchSet,
    originals: Vec<(PathBuf, Vec<u8>, Sha256Hash)>,
}

#[derive(Debug)]
pub struct ApplyFailure {
    pub patch_set: PatchSet,
    pub partial: bool,
    pub code: &'static str,
}

impl std::fmt::Display for ApplyFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code)
    }
}

impl std::error::Error for ApplyFailure {}

pub fn apply_patch(
    mut patch_set: PatchSet,
    project_root: &Path,
    recipe_artifact: &serde_json::Value,
    approved_patch_fingerprint: &str,
) -> Result<AppliedPatch, Box<ApplyFailure>> {
    if patch_set.status != PatchSetStatus::Proposed
        || patch_set.patch_fingerprint.as_str() != approved_patch_fingerprint
    {
        return Err(failure(
            patch_set,
            false,
            "PATCH_APPROVAL_OR_STATE_MISMATCH",
        ));
    }
    let recipe_contract = match trailing_whitespace_recipe() {
        Ok(recipe) => recipe,
        Err(_) => return Err(failure(patch_set, false, "PATCH_RECIPE_CONTRACT_INVALID")),
    };
    let recipe: PatchRecipeArtifact =
        match serde_json::from_value::<PatchRecipeArtifact>(recipe_artifact.clone()) {
            Ok(recipe)
                if recipe.schema_version == 1
                    && recipe.recipe_id == recipe_contract.recipe_id
                    && recipe.recipe_version == recipe_contract.recipe_version
                    && recipe.recipe_definition_fingerprint
                        == recipe_contract.definition_fingerprint =>
            {
                recipe
            }
            _ => return Err(failure(patch_set, false, "PATCH_ARTIFACT_INVALID")),
        };
    let recipe_bytes = match serde_json::to_vec_pretty(recipe_artifact) {
        Ok(bytes) => bytes,
        Err(_) => return Err(failure(patch_set, false, "PATCH_ARTIFACT_INVALID")),
    };
    if patch_set.patch_artifact_refs.len() != 1
        || patch_set.patch_artifact_refs[0].sha256 != Sha256Hash::digest(&recipe_bytes)
    {
        return Err(failure(patch_set, false, "PATCH_ARTIFACT_INVALID"));
    }
    let expected_patch_fingerprint = match versioned_fingerprint(
        "star.patch-set",
        1,
        &serde_json::json!({
            "project_id":patch_set.project_id,
            "base_workspace_snapshot_id":patch_set.base_workspace_snapshot_id,
            "change_plan_id":patch_set.change_plan_id,
            "change_plan_revision":patch_set.change_plan_revision,
            "operations":patch_set.operations,
            "artifact_sha256":patch_set.patch_artifact_refs[0].sha256,
        }),
    ) {
        Ok(fingerprint) => fingerprint,
        Err(_) => return Err(failure(patch_set, false, "PATCH_ARTIFACT_INVALID")),
    };
    if expected_patch_fingerprint != patch_set.patch_fingerprint {
        return Err(failure(patch_set, false, "PATCH_ARTIFACT_MISMATCH"));
    }
    if recipe.files.len() != patch_set.operations.len() {
        return Err(failure(patch_set, false, "PATCH_ARTIFACT_MISMATCH"));
    }
    let mut prepared: Vec<(PathBuf, Vec<u8>, Vec<u8>, Sha256Hash)> = Vec::new();
    for file in &recipe.files {
        let Some(operation) = patch_set
            .operations
            .iter()
            .find(|operation| operation.path == file.path)
        else {
            return Err(failure(patch_set, false, "PATCH_ARTIFACT_MISMATCH"));
        };
        let path = match resolve_safe_file(project_root, &file.path) {
            Ok(path) => path,
            Err(_) => return Err(failure(patch_set, false, "PATCH_PATH_UNSAFE")),
        };
        let before = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => return Err(failure(patch_set, false, "PATCH_READ_FAILED")),
        };
        let before_hash = Sha256Hash::digest(&before);
        if before_hash != file.before_sha256
            || operation.before_sha256.as_ref() != Some(&before_hash)
        {
            return Err(failure(patch_set, false, "PATCH_TARGET_DIRTY_OR_STALE"));
        }
        let after = match apply_recipe(&before, &file.edits) {
            Ok(after) => after,
            Err(_) => return Err(failure(patch_set, false, "PATCH_ARTIFACT_MISMATCH")),
        };
        if Sha256Hash::digest(&after) != file.after_sha256
            || operation.after_sha256.as_ref() != Some(&file.after_sha256)
        {
            return Err(failure(patch_set, false, "PATCH_RESULT_HASH_MISMATCH"));
        }
        prepared.push((path, before, after, file.after_sha256.clone()));
    }
    let mut originals: Vec<(PathBuf, Vec<u8>, Sha256Hash)> = Vec::new();
    for (path, before, after, after_hash) in prepared {
        let still_current = fs::read(&path)
            .ok()
            .is_some_and(|bytes| Sha256Hash::digest(&bytes) == Sha256Hash::digest(&before));
        if !still_current {
            let partial = !rollback_originals(&originals);
            patch_set.status = if partial {
                PatchSetStatus::PartiallyApplied
            } else {
                PatchSetStatus::Failed
            };
            return Err(failure(patch_set, partial, "PATCH_TARGET_DIRTY_OR_STALE"));
        }
        if replace_file_atomic(&path, &after).is_err() {
            let partial = !rollback_originals(&originals);
            patch_set.status = if partial {
                PatchSetStatus::PartiallyApplied
            } else {
                PatchSetStatus::Failed
            };
            return Err(failure(patch_set, partial, "PATCH_APPLY_FAILED"));
        }
        originals.push((path, before, after_hash));
    }
    patch_set.status = PatchSetStatus::Applied;
    Ok(AppliedPatch {
        patch_set,
        originals,
    })
}

fn rollback_originals(originals: &[(PathBuf, Vec<u8>, Sha256Hash)]) -> bool {
    let mut complete = true;
    for (applied_path, original, expected_after) in originals.iter().rev() {
        let safe = fs::read(applied_path)
            .ok()
            .is_some_and(|bytes| Sha256Hash::digest(&bytes) == *expected_after);
        if !safe || replace_file_atomic(applied_path, original).is_err() {
            complete = false;
        }
    }
    complete
}

pub fn rollback_applied(mut applied: AppliedPatch) -> Result<PatchSet, Box<ApplyFailure>> {
    let mut failed = false;
    for (path, original, expected_after) in applied.originals.iter().rev() {
        let safe = fs::read(path)
            .ok()
            .is_some_and(|bytes| Sha256Hash::digest(&bytes) == *expected_after);
        if !safe || replace_file_atomic(path, original).is_err() {
            failed = true;
        }
    }
    if failed {
        applied.patch_set.status = PatchSetStatus::PartiallyApplied;
        return Err(failure(applied.patch_set, true, "PATCH_ROLLBACK_BLOCKED"));
    }
    applied.patch_set.status = PatchSetStatus::Reverted;
    Ok(applied.patch_set)
}

fn failure(patch_set: PatchSet, partial: bool, code: &'static str) -> Box<ApplyFailure> {
    Box::new(ApplyFailure {
        patch_set,
        partial,
        code,
    })
}

fn remove_trailing_whitespace(
    bytes: &[u8],
    target_lines: &BTreeSet<u32>,
) -> Result<(Vec<u8>, Vec<LineEdit>), ExecutionError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ExecutionError::Stale)?;
    let mut output = String::with_capacity(text.len());
    let mut edits = Vec::new();
    for (index, segment) in text.split_inclusive('\n').enumerate() {
        let line = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let (body, ending) = if let Some(body) = segment.strip_suffix("\r\n") {
            (body, "\r\n")
        } else if let Some(body) = segment.strip_suffix('\n') {
            (body, "\n")
        } else {
            (segment, "")
        };
        if target_lines.contains(&line) {
            let trimmed = body.trim_end_matches([' ', '\t']);
            let trailing = body[trimmed.len()..]
                .chars()
                .map(|character| match character {
                    ' ' => Ok(WhitespaceKind::Space),
                    '\t' => Ok(WhitespaceKind::Tab),
                    _ => Err(ExecutionError::Stale),
                })
                .collect::<Result<Vec<_>, _>>()?;
            if trailing.is_empty() {
                return Err(ExecutionError::Stale);
            }
            output.push_str(trimmed);
            edits.push(LineEdit { line, trailing });
        } else {
            output.push_str(body);
        }
        output.push_str(ending);
    }
    Ok((output.into_bytes(), edits))
}

fn trailing_whitespace_lines(bytes: &[u8]) -> Result<BTreeSet<u32>, ExecutionError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ExecutionError::Stale)?;
    Ok(text
        .split_inclusive('\n')
        .enumerate()
        .filter_map(|(index, segment)| {
            let body = segment
                .strip_suffix("\r\n")
                .or_else(|| segment.strip_suffix('\n'))
                .unwrap_or(segment);
            body.ends_with([' ', '\t'])
                .then(|| u32::try_from(index + 1).unwrap_or(u32::MAX))
        })
        .collect())
}

fn apply_recipe(bytes: &[u8], edits: &[LineEdit]) -> Result<Vec<u8>, ExecutionError> {
    let target_lines: BTreeSet<_> = edits.iter().map(|edit| edit.line).collect();
    if target_lines.len() != edits.len() {
        return Err(ExecutionError::InvalidArtifact);
    }
    let (after, observed) = remove_trailing_whitespace(bytes, &target_lines)?;
    for (expected, actual) in edits.iter().zip(observed.iter()) {
        if expected.line != actual.line
            || expected.trailing.len() != actual.trailing.len()
            || expected
                .trailing
                .iter()
                .zip(&actual.trailing)
                .any(|(left, right)| std::mem::discriminant(left) != std::mem::discriminant(right))
        {
            return Err(ExecutionError::InvalidArtifact);
        }
    }
    Ok(after)
}

fn replay_is_no_change(bytes: &[u8], edits: &[LineEdit]) -> Result<bool, ExecutionError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ExecutionError::InvalidArtifact)?;
    let target_lines = edits.iter().map(|edit| edit.line).collect::<BTreeSet<_>>();
    if target_lines.len() != edits.len() {
        return Err(ExecutionError::InvalidArtifact);
    }
    for (index, segment) in text.split_inclusive('\n').enumerate() {
        let line = u32::try_from(index + 1).unwrap_or(u32::MAX);
        if !target_lines.contains(&line) {
            continue;
        }
        let body = segment
            .strip_suffix("\r\n")
            .or_else(|| segment.strip_suffix('\n'))
            .unwrap_or(segment);
        if body.ends_with([' ', '\t']) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn resolve_safe_file(root: &Path, relative: &ProjectPathRef) -> Result<PathBuf, ExecutionError> {
    let canonical_root = root
        .canonicalize()
        .map_err(|_| ExecutionError::UnsafePath)?;
    let candidate = relative
        .as_str()
        .split('/')
        .fold(canonical_root.clone(), |path, segment| path.join(segment));
    let metadata = fs::symlink_metadata(&candidate).map_err(|_| ExecutionError::UnsafePath)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(ExecutionError::UnsafePath);
    }
    let canonical = candidate
        .canonicalize()
        .map_err(|_| ExecutionError::UnsafePath)?;
    if !canonical.starts_with(&canonical_root) {
        return Err(ExecutionError::UnsafePath);
    }
    Ok(canonical)
}

fn replace_file_atomic(path: &Path, bytes: &[u8]) -> Result<(), ExecutionError> {
    let parent = path.parent().ok_or(ExecutionError::UnsafePath)?;
    let temporary = parent.join(format!(
        ".star-patch-{}.tmp",
        PatchSetId::new().as_str().trim_start_matches("pat_")
    ));
    fs::write(&temporary, bytes).map_err(|_| ExecutionError::Io)?;
    let file = fs::OpenOptions::new()
        .write(true)
        .open(&temporary)
        .map_err(|_| ExecutionError::Io)?;
    file.sync_all().map_err(|_| ExecutionError::Io)?;
    drop(file);
    let target = HSTRING::from(path.as_os_str().to_string_lossy().as_ref());
    let replacement = HSTRING::from(temporary.as_os_str().to_string_lossy().as_ref());
    let replace_result = unsafe {
        ReplaceFileW(
            &target,
            &replacement,
            PCWSTR::null(),
            REPLACEFILE_WRITE_THROUGH,
            None,
            None,
        )
    };
    if replace_result.is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(ExecutionError::Io);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::{
        evidence::{ArtifactKind, ProducerRef, RedactionStatus, RetentionClass},
        ids::{
            ArtifactId, CanonicalSourceId, FindingId, OccurrenceId, ProjectId, ProjectRevisionId,
            ScanRunId, SymbolId, WorkspaceSnapshotId,
        },
        management::{
            Completeness, Confidence, FindingLifecycle, RedactionState, Severity, SourceRange,
        },
    };

    fn test_producer() -> ProducerRef {
        ProducerRef {
            component: "star-execution-test".to_owned(),
            product_version: env!("CARGO_PKG_VERSION").to_owned(),
            build_id: "test".to_owned(),
            platform: std::env::consts::OS.to_owned(),
        }
    }

    #[test]
    fn exact_hash_apply_preserves_unrelated_dirty_file_and_safe_rollback_restores_target() {
        let root = std::env::temp_dir().join(format!(
            "star-execution-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), b"fn main() {}  \n").unwrap();
        fs::write(root.join("unrelated.txt"), b"user change\n").unwrap();
        let project_id = ProjectId::new();
        let scan_id = ScanRunId::new();
        let finding_id = FindingId::new();
        let occurrence_id = OccurrenceId::new();
        let revision_id = ProjectRevisionId::new();
        let snapshot_id = WorkspaceSnapshotId::new();
        let path = ProjectPathRef::parse("src/lib.rs").unwrap();
        let before_hash = Sha256Hash::digest(b"fn main() {}  \n");
        let artifact = ArtifactRef {
            artifact_id: ArtifactId::new(),
            kind: ArtifactKind::Manifest,
            project_id: Some(project_id.clone()),
            relative_path: ".ai-runs/manifest.json".to_owned(),
            media_type: "application/json".to_owned(),
            size_bytes: 1,
            sha256: before_hash.clone(),
            created_at: Utc::now(),
            producer: test_producer(),
            redaction_status: RedactionStatus::NotNeeded,
            retention_class: RetentionClass::Run,
            source_artifact_ref: None,
        };
        let snapshot = WorkspaceSnapshot {
            schema_id: "star.workspace-snapshot".to_owned(),
            schema_version: 1,
            workspace_snapshot_id: snapshot_id.clone(),
            project_id: project_id.clone(),
            project_revision_id: revision_id.clone(),
            scope: vec!["**/*".to_owned()],
            entries_manifest_ref: artifact,
            entries_fingerprint: before_hash.clone(),
            dirty_summary: BTreeMap::new(),
            ignored_policy: "exclude".to_owned(),
            symlink_policy: "do_not_follow".to_owned(),
            captured_at: Utc::now(),
            completeness: Completeness::Complete,
            limitations: vec![],
        };
        let managed_recipe = managed_declaration_recipe_v2().unwrap();
        assert!(
            managed_recipe
                .selector_kinds
                .contains(&star_contracts::patch_v2::TargetSelectorKindV2::ManagedDeclaration)
        );
        for family in [
            "managed_registry_contract",
            "consumer_compatibility",
            "generated_consistency",
            "docs_contract_drift",
        ] {
            assert!(
                managed_recipe
                    .validation_families
                    .contains(&family.to_owned())
            );
        }
        let exact = prepare_exact_materialized_patch(
            &project_id,
            &snapshot,
            &managed_recipe,
            &[MaterializedPatchFile {
                path: ProjectPathRef::parse("src/lib.rs").unwrap(),
                before_sha256: Sha256Hash::digest(b"fn main() {}  \n"),
                after_sha256: Sha256Hash::digest(b"fn main() {}\n"),
                before_bytes: b"fn main() {}  \n".to_vec(),
                after_bytes: b"fn main() {}\n".to_vec(),
            }],
            BTreeMap::from([(
                "managed_registry_intent_fingerprint".to_owned(),
                Sha256Hash::digest(b"intent").to_string(),
            )]),
        )
        .unwrap();
        assert_eq!(exact.patch_set.operations.len(), 1);
        assert_eq!(exact.recipe_artifact["recipe_id"], managed_recipe.recipe_id);
        let finding = Finding {
            schema_id: "star.finding".to_owned(),
            schema_version: 1,
            finding_id: finding_id.clone(),
            finding_fingerprint: Sha256Hash::digest(b"finding"),
            project_id: project_id.clone(),
            rule_id: "star.rule.trailing-whitespace".to_owned(),
            rule_version: "1.0.0".to_owned(),
            identity_anchor: "file".to_owned(),
            identity_tokens: vec![],
            title_code: "TITLE".to_owned(),
            message_code: "TRAILING_WHITESPACE".to_owned(),
            severity: Severity::Warning,
            confidence: Confidence::High,
            lifecycle: FindingLifecycle::Open,
            first_observed_scan_id: scan_id.clone(),
            last_observed_scan_id: scan_id.clone(),
            current_occurrence_ids: vec![occurrence_id.clone()],
            active_disposition_id: None,
            active_suppression_ids: vec![],
            content_fingerprint: Sha256Hash::digest(b"content"),
        };
        let occurrence = Occurrence {
            schema_id: "star.occurrence".to_owned(),
            schema_version: 1,
            occurrence_id,
            occurrence_fingerprint: Sha256Hash::digest(b"occurrence"),
            finding_id,
            scan_run_id: scan_id,
            project_revision_id: revision_id,
            workspace_snapshot_id: snapshot_id,
            canonical_source_id: CanonicalSourceId::new(),
            source_content_sha256: before_hash,
            location_path: path,
            location_range: SourceRange {
                start_line: 1,
                start_column: 13,
                end_line: 1,
                end_column: 15,
            },
            symbol_id: Some(SymbolId::new()),
            message_parameters: BTreeMap::new(),
            evidence_refs: vec![],
            observed_at: Utc::now(),
            redaction_state: RedactionState::NotNeeded,
        };
        let prepared = prepare_trailing_whitespace_patch(
            &root,
            &finding,
            std::slice::from_ref(&occurrence),
            &snapshot,
        )
        .unwrap();
        let artifact_ref = ArtifactRef {
            artifact_id: ArtifactId::new(),
            kind: ArtifactKind::ChangeSet,
            project_id: Some(project_id),
            relative_path: ".ai-runs/patch.json".to_owned(),
            media_type: "application/json".to_owned(),
            size_bytes: 1,
            sha256: Sha256Hash::digest(
                &serde_json::to_vec_pretty(&prepared.recipe_artifact).unwrap(),
            ),
            created_at: Utc::now(),
            producer: test_producer(),
            redaction_status: RedactionStatus::NotNeeded,
            retention_class: RetentionClass::Evidence,
            source_artifact_ref: None,
        };
        let prepared = prepared.attach_artifact(artifact_ref).unwrap();
        let approval = prepared.patch_set.patch_fingerprint.as_str().to_owned();
        let applied = apply_patch(
            prepared.patch_set,
            &root,
            &prepared.recipe_artifact,
            &approval,
        )
        .unwrap();
        assert_eq!(
            fs::read(root.join("src/lib.rs")).unwrap(),
            b"fn main() {}\n"
        );
        assert_eq!(
            fs::read(root.join("unrelated.txt")).unwrap(),
            b"user change\n"
        );
        let reverted = rollback_applied(applied).unwrap();
        assert_eq!(reverted.status, PatchSetStatus::Reverted);
        assert_eq!(
            fs::read(root.join("src/lib.rs")).unwrap(),
            b"fn main() {}  \n"
        );

        let unsealed = prepare_trailing_whitespace_patch(
            &root,
            &finding,
            std::slice::from_ref(&occurrence),
            &snapshot,
        )
        .unwrap();
        let unsealed_approval = unsealed.patch_set.patch_fingerprint.to_string();
        let error = apply_patch(
            unsealed.patch_set,
            &root,
            &unsealed.recipe_artifact,
            &unsealed_approval,
        )
        .unwrap_err();
        assert_eq!(error.code, "PATCH_ARTIFACT_INVALID");

        let prepared =
            prepare_trailing_whitespace_patch(&root, &finding, &[occurrence], &snapshot).unwrap();
        let artifact_ref = ArtifactRef {
            artifact_id: ArtifactId::new(),
            kind: ArtifactKind::ChangeSet,
            project_id: Some(finding.project_id.clone()),
            relative_path: ".ai-runs/stale-patch.json".to_owned(),
            media_type: "application/json".to_owned(),
            size_bytes: 1,
            sha256: Sha256Hash::digest(
                &serde_json::to_vec_pretty(&prepared.recipe_artifact).unwrap(),
            ),
            created_at: Utc::now(),
            producer: test_producer(),
            redaction_status: RedactionStatus::NotNeeded,
            retention_class: RetentionClass::Evidence,
            source_artifact_ref: None,
        };
        let prepared = prepared.attach_artifact(artifact_ref).unwrap();
        let approval = prepared.patch_set.patch_fingerprint.to_string();
        fs::write(root.join("src/lib.rs"), b"fn main() { changed(); }  \n").unwrap();
        let error = apply_patch(
            prepared.patch_set,
            &root,
            &prepared.recipe_artifact,
            &approval,
        )
        .unwrap_err();
        assert_eq!(error.code, "PATCH_TARGET_DIRTY_OR_STALE");
        assert!(!error.partial);
        assert_eq!(
            fs::read(root.join("src/lib.rs")).unwrap(),
            b"fn main() { changed(); }  \n"
        );
    }

    struct FakeToolExecutor {
        result: ToolExecutionResult,
    }

    impl ToolExecutorPort for FakeToolExecutor {
        fn execute(
            &self,
            _request: &ToolExecutionRequest,
        ) -> Result<ToolExecutionResult, PatchPortError> {
            Ok(self.result.clone())
        }
    }

    fn effect_request(kind: DevelopmentEffectKind) -> DevelopmentEffectRequest {
        let subject = Sha256Hash::digest(b"subject");
        DevelopmentEffectRequest {
            kind,
            tool: ToolExecutionRequest {
                tool_ref: CatalogRef {
                    catalog_id: "star.test.external".to_owned(),
                    format_version: 1,
                    item_version: "1.0.0".to_owned(),
                    sha256: Sha256Hash::digest(b"descriptor"),
                },
                logical_executable: "fixture.exe".to_owned(),
                executable_binding_fingerprint: Sha256Hash::digest(b"binding"),
                args: vec!["--json".to_owned()],
                working_directory: std::env::temp_dir(),
                timeout_ms: 5_000,
                permission_action: kind.permission_action().to_owned(),
                expected_exit_codes: BTreeSet::from([0]),
                output_limits: star_contracts::evidence::OutputLimits {
                    stdout_bytes: 1024,
                    stderr_bytes: 1024,
                    artifact_bytes: 2048,
                },
                input_fingerprint: subject.clone(),
            },
            expected_executable_sha256: Sha256Hash::digest(b"executable"),
            exact_subject_fingerprint: subject,
            approval_ref: kind.requires_approval().then(|| "apr_fixture".to_owned()),
        }
    }

    #[test]
    fn registered_effect_adapter_covers_m7_m8_m9_permissions_and_preserves_unknown() {
        let kinds = [
            DevelopmentEffectKind::SecurityRefresh,
            DevelopmentEffectKind::DebuggerCapture,
            DevelopmentEffectKind::LicenseScan,
            DevelopmentEffectKind::DependencyPrepare,
            DevelopmentEffectKind::DependencyApply,
            DevelopmentEffectKind::UpdaterApply,
            DevelopmentEffectKind::MigrationExecute,
            DevelopmentEffectKind::PerformanceRun,
            DevelopmentEffectKind::LanguageCutover,
            DevelopmentEffectKind::RemoteRecovery,
        ];
        for kind in kinds {
            let executor = FakeToolExecutor {
                result: ToolExecutionResult {
                    exit_code: Some(0),
                    termination_reason: star_contracts::evidence::TerminationReason::Exited,
                    completeness: star_contracts::evidence::Completeness::Complete,
                    success: true,
                    output_artifact_refs: vec![],
                    observed_executable_fingerprint: Sha256Hash::digest(b"executable"),
                },
            };
            let observation = RegisteredDevelopmentEffectAdapter::new(&executor)
                .run(effect_request(kind))
                .unwrap();
            assert_eq!(observation.kind, kind);
            assert_eq!(observation.state, DevelopmentEffectState::Succeeded);
            assert!(observation.source_effect_started);
        }

        let executor = FakeToolExecutor {
            result: ToolExecutionResult {
                exit_code: None,
                termination_reason: star_contracts::evidence::TerminationReason::OutcomeUnknown,
                completeness: star_contracts::evidence::Completeness::Unverified,
                success: false,
                output_artifact_refs: vec![],
                observed_executable_fingerprint: Sha256Hash::digest(b"executable"),
            },
        };
        let observation = RegisteredDevelopmentEffectAdapter::new(&executor)
            .run(effect_request(DevelopmentEffectKind::RemoteRecovery))
            .unwrap();
        assert_eq!(observation.state, DevelopmentEffectState::OutcomeUnknown);

        let mut missing_approval = effect_request(DevelopmentEffectKind::LanguageCutover);
        missing_approval.approval_ref = None;
        assert!(matches!(
            RegisteredDevelopmentEffectAdapter::new(&executor).run(missing_approval),
            Err(PatchPortError::Invalid)
        ));
    }
}
