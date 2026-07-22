//! Exact-precondition PatchSet preparation, application, and safe rollback.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use star_contracts::{
    Sha256Hash,
    evidence::ArtifactRef,
    ids::{ChangePlanId, PatchSetId},
    management::{
        ChangePlan, ChangePlanStatus, ChangeRecipe, ChangeRecipeRef, FileOperationKind, Finding,
        Occurrence, PatchFileOperation, PatchSet, PatchSetStatus, ProjectPathRef,
        WorkspaceSnapshot,
    },
};
use star_domain::versioned_fingerprint;
use thiserror::Error;
use windows::{
    Win32::Storage::FileSystem::{REPLACEFILE_WRITE_THROUGH, ReplaceFileW},
    core::{HSTRING, PCWSTR},
};

pub mod rust_style;

pub const RECIPE_ID: &str = "star.recipe.remove-trailing-whitespace";
pub const RECIPE_VERSION: &str = "1.0.0";

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
}
