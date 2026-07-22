//! Project discovery and deterministic scan input construction.

pub mod catalog;
pub mod catalog_snapshot;
pub mod index;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use star_contracts::{
    Sha256Hash,
    evidence::ArtifactRef,
    ids::{
        CanonicalSourceId, CheckoutId, ProjectId, ProjectRevisionId, RootBindingId, ScanRunId,
        SymbolId, WorkspaceSnapshotId,
    },
    management::{
        Baseline, BaselineScope, CanonicalSource, CheckoutAttachmentState, CheckoutHeadState,
        CheckoutKind, Completeness, IdentityScope, Project, ProjectCheckout, ProjectPathRef,
        ProjectRevision, RegistrationState, RepositoryKind, Sensitivity, SourceKind, SourceRange,
        Suppression, SuppressionScope, Symbol, WorkspaceSnapshot,
    },
    planning::ObservedChangeKind,
};
use star_domain::{
    PersistenceRedactor, validate_baseline, validate_suppression, versioned_fingerprint,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("project root is unavailable or invalid")]
    InvalidRoot,
    #[error("project manifest is invalid")]
    InvalidManifest,
    #[error("project scan resource limit was reached")]
    ResourceLimit,
    #[error("project source I/O failed")]
    Io,
    #[error("project identity fingerprint failed")]
    Fingerprint,
    #[error("shared decision declaration is invalid")]
    InvalidSharedDecision,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectManifest {
    schema_version: u32,
    project_id: ProjectId,
    display_name: String,
    repository_kind: RepositoryKind,
    source_of_truth: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SharedSuppressionsFile {
    schema_version: u32,
    suppressions: Vec<Suppression>,
}

#[derive(Clone, Debug)]
pub struct SharedDecisionDeclarations {
    pub baselines: Vec<Baseline>,
    pub suppressions: Vec<Suppression>,
    pub source_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug)]
pub struct AttachedProjectSeed {
    pub project: Project,
    pub checkout: ProjectCheckout,
}

pub fn load_shared_decisions(
    project: &Project,
    root: &Path,
) -> Result<SharedDecisionDeclarations, ProjectError> {
    let control = root.join(".star-control");
    let suppressions_path = control.join("suppressions.toml");
    let baselines_dir = control.join("baselines");
    let has_shared_declarations = suppressions_path.exists() || baselines_dir.exists();
    if has_shared_declarations && project.identity_scope != IdentityScope::Shared {
        return Err(ProjectError::InvalidSharedDecision);
    }
    let mut suppressions = if suppressions_path.exists() {
        let source = fs::read_to_string(&suppressions_path)
            .map_err(|_| ProjectError::InvalidSharedDecision)?;
        let document: SharedSuppressionsFile =
            toml::from_str(&source).map_err(|_| ProjectError::InvalidSharedDecision)?;
        if document.schema_version != 1 {
            return Err(ProjectError::InvalidSharedDecision);
        }
        document.suppressions
    } else {
        Vec::new()
    };
    let mut baselines = Vec::new();
    if baselines_dir.exists() {
        if !baselines_dir.is_dir() {
            return Err(ProjectError::InvalidSharedDecision);
        }
        let mut paths = Vec::new();
        for entry in
            fs::read_dir(&baselines_dir).map_err(|_| ProjectError::InvalidSharedDecision)?
        {
            let path = entry
                .map_err(|_| ProjectError::InvalidSharedDecision)?
                .path();
            if path
                .extension()
                .is_some_and(|extension| extension == "toml")
            {
                paths.push(path);
            }
        }
        paths.sort();
        for path in paths {
            let source =
                fs::read_to_string(path).map_err(|_| ProjectError::InvalidSharedDecision)?;
            baselines.push(
                toml::from_str::<Baseline>(&source)
                    .map_err(|_| ProjectError::InvalidSharedDecision)?,
            );
        }
    }
    let redactor = PersistenceRedactor::for_current_user();
    for suppression in &suppressions {
        if suppression.schema_id != "star.suppression"
            || suppression.schema_version != 1
            || suppression.scope_kind != SuppressionScope::Shared
            || suppression.project_id != project.project_id
            || suppression.reason.trim().is_empty()
            || validate_suppression(suppression).is_err()
            || validate_serialized_strings(&redactor, suppression).is_err()
        {
            return Err(ProjectError::InvalidSharedDecision);
        }
    }
    for baseline in &baselines {
        if baseline.schema_id != "star.baseline"
            || baseline.schema_version != 1
            || baseline.scope_kind != BaselineScope::Shared
            || baseline.project_id != project.project_id
            || baseline.reason.trim().is_empty()
            || validate_baseline(baseline).is_err()
            || validate_serialized_strings(&redactor, baseline).is_err()
        {
            return Err(ProjectError::InvalidSharedDecision);
        }
    }
    suppressions.sort_by(|left, right| {
        left.suppression_id
            .cmp(&right.suppression_id)
            .then_with(|| left.revision.cmp(&right.revision))
    });
    baselines.sort_by(|left, right| {
        left.baseline_id
            .cmp(&right.baseline_id)
            .then_with(|| left.revision.cmp(&right.revision))
    });
    if suppressions
        .windows(2)
        .any(|pair| pair[0].suppression_id == pair[1].suppression_id)
        || baselines
            .windows(2)
            .any(|pair| pair[0].baseline_id == pair[1].baseline_id)
    {
        return Err(ProjectError::InvalidSharedDecision);
    }
    let source_fingerprint = versioned_fingerprint(
        "star.shared-decision-declarations",
        1,
        &serde_json::json!({
            "baselines":baselines,
            "suppressions":suppressions,
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    Ok(SharedDecisionDeclarations {
        baselines,
        suppressions,
        source_fingerprint,
    })
}

fn validate_serialized_strings(
    redactor: &PersistenceRedactor,
    value: &impl Serialize,
) -> Result<(), ProjectError> {
    fn visit(
        redactor: &PersistenceRedactor,
        value: &serde_json::Value,
    ) -> Result<(), ProjectError> {
        match value {
            serde_json::Value::String(value) => redactor
                .validate(value)
                .map_err(|_| ProjectError::InvalidSharedDecision),
            serde_json::Value::Array(values) => {
                for value in values {
                    visit(redactor, value)?;
                }
                Ok(())
            }
            serde_json::Value::Object(values) => {
                for (key, value) in values {
                    redactor
                        .validate(key)
                        .map_err(|_| ProjectError::InvalidSharedDecision)?;
                    visit(redactor, value)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    let value = serde_json::to_value(value).map_err(|_| ProjectError::InvalidSharedDecision)?;
    visit(redactor, &value)
}

#[derive(Clone, Debug)]
pub struct ProjectSeed {
    pub project_id: ProjectId,
    pub identity_scope: IdentityScope,
    pub display_name: String,
    pub repository_kind: RepositoryKind,
    pub source_of_truth: Vec<String>,
    pub declaration_fingerprint: Sha256Hash,
}

impl ProjectSeed {
    pub fn discover(root: &Path) -> Result<Self, ProjectError> {
        Self::discover_with_local_project_id(root, None)
    }

    pub fn discover_with_local_project_id(
        root: &Path,
        existing_local_project_id: Option<ProjectId>,
    ) -> Result<Self, ProjectError> {
        if !root.is_absolute() || !root.is_dir() {
            return Err(ProjectError::InvalidRoot);
        }
        let manifest_path = root.join(".star-control").join("project.toml");
        if manifest_path.exists() {
            let source =
                fs::read_to_string(manifest_path).map_err(|_| ProjectError::InvalidManifest)?;
            let manifest: ProjectManifest =
                toml::from_str(&source).map_err(|_| ProjectError::InvalidManifest)?;
            if manifest.schema_version != 1
                || manifest.display_name.trim().is_empty()
                || manifest.source_of_truth.is_empty()
                || manifest
                    .source_of_truth
                    .iter()
                    .any(|value| ProjectPathRef::parse(value).is_err())
            {
                return Err(ProjectError::InvalidManifest);
            }
            PersistenceRedactor::for_current_user()
                .validate(&manifest.display_name)
                .map_err(|_| ProjectError::InvalidManifest)?;
            for source in &manifest.source_of_truth {
                PersistenceRedactor::for_current_user()
                    .validate(source)
                    .map_err(|_| ProjectError::InvalidManifest)?;
            }
            let declaration_fingerprint = versioned_fingerprint(
                "star.identity.project-declaration",
                1,
                &serde_json::json!({
                    "project_id":manifest.project_id,
                    "display_name":manifest.display_name,
                    "repository_kind":manifest.repository_kind,
                    "source_of_truth":manifest.source_of_truth,
                }),
            )
            .map_err(|_| ProjectError::Fingerprint)?;
            return Ok(Self {
                project_id: manifest.project_id,
                identity_scope: IdentityScope::Shared,
                display_name: manifest.display_name,
                repository_kind: manifest.repository_kind,
                source_of_truth: manifest.source_of_truth,
                declaration_fingerprint,
            });
        }
        let project_id = existing_local_project_id.unwrap_or_default();
        let declaration_fingerprint = versioned_fingerprint(
            "star.identity.project-declaration",
            1,
            &serde_json::json!({
                "project_id":project_id,
                "identity_scope":"local",
            }),
        )
        .map_err(|_| ProjectError::Fingerprint)?;
        Ok(Self {
            project_id,
            identity_scope: IdentityScope::Local,
            display_name: "Local project".to_owned(),
            repository_kind: if root.join(".git").exists() {
                RepositoryKind::Git
            } else {
                RepositoryKind::None
            },
            source_of_truth: vec!["source".to_owned()],
            declaration_fingerprint,
        })
    }

    pub fn attach(
        self,
        checkout_id: CheckoutId,
        root_binding_id: RootBindingId,
        root: &Path,
    ) -> Result<AttachedProjectSeed, ProjectError> {
        let checkout_kind = match self.repository_kind {
            RepositoryKind::Git if root.join(".git").is_file() => CheckoutKind::LinkedWorktree,
            RepositoryKind::Git => CheckoutKind::MainWorktree,
            RepositoryKind::None => CheckoutKind::FilesystemRoot,
        };
        let (head_state, head_ref, head_commit_id, head_tree_id, object_format, mut limitations) =
            if self.repository_kind == RepositoryKind::Git {
                let head_ref = git_text(root, &["symbolic-ref", "--quiet", "--short", "HEAD"]);
                let head_commit = git_text(root, &["rev-parse", "--verify", "HEAD"]);
                let head_tree = git_text(root, &["rev-parse", "--verify", "HEAD^{tree}"]);
                let object_format = git_text(root, &["rev-parse", "--show-object-format"]);
                let state = if head_ref.is_ok() {
                    CheckoutHeadState::Branch
                } else if head_commit.is_ok() {
                    CheckoutHeadState::Detached
                } else {
                    CheckoutHeadState::Unborn
                };
                (
                    state,
                    head_ref.ok(),
                    head_commit.ok(),
                    head_tree.ok(),
                    object_format.ok(),
                    vec!["repository_binding_deferred_to_m1_probe".to_owned()],
                )
            } else {
                (
                    CheckoutHeadState::Unavailable,
                    None,
                    None,
                    None,
                    None,
                    Vec::new(),
                )
            };
        limitations.sort();
        let fingerprint_payload = serde_json::json!({
            "identity_contract_version":1,
            "checkout_id":checkout_id,
            "project_id":self.project_id,
            "root_binding_id":root_binding_id,
            "repository_kind":self.repository_kind,
            "checkout_kind":checkout_kind,
            "repository_binding_id":null,
            "worktree_binding_id":null,
            "object_format":object_format,
            "head_state":head_state,
            "head_ref":head_ref,
            "head_commit_id":head_commit_id,
            "head_tree_id":head_tree_id,
            "upstream_ref":null,
            "default_branch_hint":null,
            "remote_identity":null,
            "attachment_state":"attached",
            "limitations":limitations,
        });
        let content_fingerprint =
            versioned_fingerprint("star.identity.project-checkout", 1, &fingerprint_payload)
                .map_err(|_| ProjectError::Fingerprint)?;
        let project = Project {
            schema_id: "star.project".to_owned(),
            schema_version: 2,
            project_id: self.project_id.clone(),
            identity_scope: self.identity_scope,
            display_name: self.display_name,
            repository_kind: self.repository_kind,
            source_of_truth: self.source_of_truth,
            declaration_fingerprint: self.declaration_fingerprint,
            registration_state: RegistrationState::Attached,
            attached_checkout_ids: vec![checkout_id.clone()],
            latest_revision_id: None,
            latest_workspace_snapshot_id: None,
        };
        Ok(AttachedProjectSeed {
            project,
            checkout: ProjectCheckout {
                schema_id: "star.project-checkout".to_owned(),
                schema_version: 1,
                checkout_id,
                project_id: self.project_id,
                root_binding_id: Some(root_binding_id),
                repository_kind: self.repository_kind,
                checkout_kind,
                repository_binding_id: None,
                worktree_binding_id: None,
                object_format,
                head_state,
                head_ref,
                head_commit_id,
                head_tree_id,
                upstream_ref: None,
                default_branch_hint: None,
                remote_identity: None,
                attachment_state: CheckoutAttachmentState::Attached,
                last_observed_at: Utc::now(),
                limitations,
                content_fingerprint,
            },
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ScanPolicy {
    pub include_untracked: bool,
    pub include_ignored: bool,
    pub follow_symlinks: bool,
    pub binary_mode: String,
    pub max_file_bytes: u64,
    pub max_files: usize,
    pub max_total_bytes: u64,
    pub max_parallel_files: usize,
    /// Registered nested checkout roots, relative to this checkout, whose bytes
    /// belong to another Project/Checkout partition.
    pub excluded_relative_roots: Vec<ProjectPathRef>,
}

impl Default for ScanPolicy {
    fn default() -> Self {
        Self {
            include_untracked: true,
            include_ignored: false,
            follow_symlinks: false,
            binary_mode: "metadata_only".to_owned(),
            max_file_bytes: 16 * 1024 * 1024,
            max_files: 200_000,
            max_total_bytes: 8 * 1024 * 1024 * 1024,
            max_parallel_files: 4,
            excluded_relative_roots: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FileObservation {
    pub path: ProjectPathRef,
    pub content_sha256: Sha256Hash,
    pub size_bytes: u64,
    pub text: Option<String>,
    pub language_id: Option<String>,
    pub line_count: u32,
}

#[derive(Clone, Debug)]
pub struct ProjectObservation {
    pub revision: ProjectRevision,
    pub entries_manifest: serde_json::Value,
    pub entries_fingerprint: Sha256Hash,
    pub dirty_summary: BTreeMap<String, u64>,
    pub completeness: Completeness,
    pub limitations: Vec<String>,
    pub files: Vec<FileObservation>,
    pub scan_config_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug)]
pub struct WorkspaceChangeObservation {
    pub path: ProjectPathRef,
    pub rename_from: Option<ProjectPathRef>,
    pub change_kind: ObservedChangeKind,
    pub before_sha256: Option<Sha256Hash>,
    pub after_sha256: Option<Sha256Hash>,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub binary: bool,
}

#[derive(Clone, Debug)]
pub struct WorkspaceChangeObservationSet {
    pub entries: Vec<WorkspaceChangeObservation>,
    pub completeness: Completeness,
    pub limitations: Vec<String>,
}

pub fn observe_workspace_changes(
    project: &Project,
    root: &Path,
    current_sources: &[star_contracts::index::SourceEntry],
) -> Result<WorkspaceChangeObservationSet, ProjectError> {
    if !root.is_absolute() || !root.is_dir() {
        return Err(ProjectError::InvalidRoot);
    }
    if project.repository_kind != RepositoryKind::Git {
        return Ok(WorkspaceChangeObservationSet {
            entries: vec![],
            completeness: Completeness::Complete,
            limitations: vec![],
        });
    }
    let output = hidden_command("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "core.quotepath=false",
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
        ])
        .output()
        .map_err(|_| ProjectError::Io)?;
    if !output.status.success() || output.stdout.len() > 8 * 1024 * 1024 {
        return Err(ProjectError::ResourceLimit);
    }
    let after_hashes = current_sources
        .iter()
        .map(|source| (source.path.as_str(), source.content_sha256.clone()))
        .collect::<BTreeMap<_, _>>();
    let records = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .collect::<Vec<_>>();
    let mut cursor = 0_usize;
    let mut entries = Vec::new();
    let mut limitations = Vec::new();
    while cursor < records.len() {
        let record = records[cursor];
        cursor += 1;
        if record.len() < 4 || record[2] != b' ' {
            return Err(ProjectError::Io);
        }
        let x = record[0];
        let y = record[1];
        let path_text = String::from_utf8(record[3..].to_vec()).map_err(|_| ProjectError::Io)?;
        let path = ProjectPathRef::parse(path_text).map_err(|_| ProjectError::InvalidRoot)?;
        let renamed = matches!(x, b'R' | b'C') || matches!(y, b'R' | b'C');
        let rename_from = if renamed {
            let original = records.get(cursor).ok_or(ProjectError::Io)?;
            cursor += 1;
            Some(
                ProjectPathRef::parse(
                    String::from_utf8((*original).to_vec()).map_err(|_| ProjectError::Io)?,
                )
                .map_err(|_| ProjectError::InvalidRoot)?,
            )
        } else {
            None
        };
        let untracked = x == b'?' && y == b'?';
        let staged = !untracked && x != b' ';
        let unstaged = !untracked && y != b' ';
        let change_kind = if renamed {
            ObservedChangeKind::Rename
        } else if x == b'D' || y == b'D' {
            ObservedChangeKind::Delete
        } else if untracked || x == b'A' || y == b'A' {
            ObservedChangeKind::Add
        } else {
            ObservedChangeKind::Modify
        };
        let before_path = rename_from.as_ref().unwrap_or(&path);
        let before_sha256 = if untracked || change_kind == ObservedChangeKind::Add {
            None
        } else {
            match git_head_content_sha256(root, before_path) {
                Ok(value) => value,
                Err(_) => {
                    limitations.push("git_head_content_unavailable".to_owned());
                    None
                }
            }
        };
        let after_sha256 = if change_kind == ObservedChangeKind::Delete {
            None
        } else {
            after_hashes.get(path.as_str()).cloned()
        };
        if change_kind != ObservedChangeKind::Delete && after_sha256.is_none() {
            limitations.push("current_index_entry_unavailable".to_owned());
        }
        entries.push(WorkspaceChangeObservation {
            path,
            rename_from,
            change_kind,
            before_sha256,
            after_sha256,
            staged,
            unstaged,
            untracked,
            binary: false,
        });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    limitations.sort();
    limitations.dedup();
    Ok(WorkspaceChangeObservationSet {
        entries,
        completeness: if limitations.is_empty() {
            Completeness::Complete
        } else {
            Completeness::Partial
        },
        limitations,
    })
}

fn git_head_content_sha256(
    root: &Path,
    path: &ProjectPathRef,
) -> Result<Option<Sha256Hash>, ProjectError> {
    let object = format!("HEAD:{}", path.as_str());
    let size = git_text(root, &["cat-file", "-s", &object])?
        .parse::<u64>()
        .map_err(|_| ProjectError::Io)?;
    if size > 16 * 1024 * 1024 {
        return Err(ProjectError::ResourceLimit);
    }
    let output = hidden_command("git")
        .arg("-C")
        .arg(root)
        .args(["show", &object])
        .output()
        .map_err(|_| ProjectError::Io)?;
    if !output.status.success() || output.stdout.len() as u64 != size {
        return Ok(None);
    }
    Ok(Some(Sha256Hash::digest(&output.stdout)))
}

impl ProjectObservation {
    pub fn workspace_snapshot_id(
        &self,
        project_id: &ProjectId,
    ) -> Result<WorkspaceSnapshotId, ProjectError> {
        let identity = versioned_fingerprint(
            "star.identity.workspace-snapshot",
            1,
            &serde_json::json!({
                "project_id":project_id,
                "project_revision_id":self.revision.project_revision_id,
                "scope":["**/*"],
                "entries_fingerprint":self.entries_fingerprint,
                "ignored_policy":"exclude",
                "symlink_policy":"do_not_follow",
                "completeness":self.completeness,
            }),
        )
        .map_err(|_| ProjectError::Fingerprint)?;
        Ok(WorkspaceSnapshotId::from_fingerprint(&identity))
    }

    pub fn workspace_snapshot(
        &self,
        project_id: &ProjectId,
        entries_manifest_ref: ArtifactRef,
    ) -> Result<WorkspaceSnapshot, ProjectError> {
        Ok(WorkspaceSnapshot {
            schema_id: "star.workspace-snapshot".to_owned(),
            schema_version: 1,
            workspace_snapshot_id: self.workspace_snapshot_id(project_id)?,
            project_id: project_id.clone(),
            project_revision_id: self.revision.project_revision_id.clone(),
            scope: vec!["**/*".to_owned()],
            entries_manifest_ref,
            entries_fingerprint: self.entries_fingerprint.clone(),
            dirty_summary: self.dirty_summary.clone(),
            ignored_policy: "exclude".to_owned(),
            symlink_policy: "do_not_follow".to_owned(),
            captured_at: Utc::now(),
            completeness: self.completeness,
            limitations: self.limitations.clone(),
        })
    }

    pub fn source_graph(
        &self,
        project_id: &ProjectId,
        workspace_snapshot_id: &WorkspaceSnapshotId,
        scan_run_id: &ScanRunId,
    ) -> Result<(Vec<CanonicalSource>, Vec<Symbol>), ProjectError> {
        let mut sources = Vec::new();
        let mut symbols = Vec::new();
        for file in &self.files {
            let source_fingerprint = versioned_fingerprint(
                "star.identity.canonical-source",
                1,
                &serde_json::json!({
                    "project_id":project_id,
                    "source_kind":"file",
                    "path":file.path,
                }),
            )
            .map_err(|_| ProjectError::Fingerprint)?;
            let source_id = CanonicalSourceId::from_fingerprint(&source_fingerprint);
            sources.push(CanonicalSource {
                schema_id: "star.canonical-source".to_owned(),
                schema_version: 1,
                canonical_source_id: source_id.clone(),
                project_id: project_id.clone(),
                path: Some(file.path.clone()),
                source_kind: SourceKind::File,
                language_id: file.language_id.clone(),
                content_sha256: Some(file.content_sha256.clone()),
                project_revision_id: Some(self.revision.project_revision_id.clone()),
                workspace_snapshot_id: Some(workspace_snapshot_id.clone()),
                generated_from_refs: vec![],
                sensitivity: Sensitivity::Internal,
            });
            let symbol_fingerprint = versioned_fingerprint(
                "star.identity.symbol",
                1,
                &serde_json::json!({
                    "project_id":project_id,
                    "language_id":file.language_id.as_deref().unwrap_or("text"),
                    "symbol_kind":"file",
                    "qualified_name":file.path,
                    "canonical_source_id":source_id,
                }),
            )
            .map_err(|_| ProjectError::Fingerprint)?;
            symbols.push(Symbol {
                schema_id: "star.symbol".to_owned(),
                schema_version: 1,
                symbol_id: SymbolId::from_fingerprint(&symbol_fingerprint),
                project_id: project_id.clone(),
                canonical_source_id: source_id,
                language_id: file
                    .language_id
                    .clone()
                    .unwrap_or_else(|| "text".to_owned()),
                symbol_kind: "file".to_owned(),
                qualified_name: file.path.as_str().to_owned(),
                signature_fingerprint: None,
                declaration_range: SourceRange {
                    start_line: 1,
                    start_column: 1,
                    end_line: file.line_count.max(1) + 1,
                    end_column: 1,
                },
                visibility: None,
                workspace_snapshot_id: workspace_snapshot_id.clone(),
                scan_run_id: scan_run_id.clone(),
                content_fingerprint: symbol_fingerprint,
            });
        }
        Ok((sources, symbols))
    }
}

pub fn observe_project(
    project: &Project,
    root: &Path,
    policy: &ScanPolicy,
) -> Result<ProjectObservation, ProjectError> {
    if !root.is_absolute() || !root.is_dir() {
        return Err(ProjectError::InvalidRoot);
    }
    let mut limitations = Vec::new();
    let mut completeness = Completeness::Complete;
    let redactor = PersistenceRedactor::for_current_user();
    let paths = if matches!(project.repository_kind, RepositoryKind::Git) {
        match git_paths(root, policy.include_untracked, policy.include_ignored) {
            Ok(paths) => paths,
            Err(_) => {
                completeness = Completeness::Partial;
                limitations.push("git_path_enumeration_failed".to_owned());
                filesystem_paths(root, policy)?
            }
        }
    } else {
        filesystem_paths(root, policy)?
    };
    let paths: Vec<_> = paths
        .into_iter()
        .filter(|path| {
            path.strip_prefix(root)
                .ok()
                .is_none_or(|relative| !is_excluded_relative_path(relative, policy))
        })
        .collect();
    if paths.len() > policy.max_files {
        return Err(ProjectError::ResourceLimit);
    }
    let mut total_bytes = 0_u64;
    let mut files = Vec::new();
    for path in paths {
        let relative = relative_path(root, &path)?;
        if redactor.validate(relative.as_str()).is_err() {
            completeness = Completeness::Partial;
            limitations.push("prohibited_path_discarded".to_owned());
            continue;
        }
        let metadata = fs::symlink_metadata(&path).map_err(|_| ProjectError::Io)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            continue;
        }
        if metadata.len() > policy.max_file_bytes {
            completeness = Completeness::Partial;
            limitations.push("max_file_bytes".to_owned());
            continue;
        }
        total_bytes = total_bytes
            .checked_add(metadata.len())
            .ok_or(ProjectError::ResourceLimit)?;
        if total_bytes > policy.max_total_bytes {
            return Err(ProjectError::ResourceLimit);
        }
        let bytes = fs::read(&path).map_err(|_| ProjectError::Io)?;
        if redactor.validate(&String::from_utf8_lossy(&bytes)).is_err() {
            completeness = Completeness::Partial;
            limitations.push("sensitive_literal_discarded".to_owned());
            continue;
        }
        let binary = bytes.iter().take(8192).any(|byte| *byte == 0);
        let text = if binary {
            None
        } else {
            String::from_utf8(bytes.clone()).ok()
        };
        if !binary && text.is_none() {
            completeness = Completeness::Partial;
            limitations.push("non_utf8_text_metadata_only".to_owned());
        }
        let line_count = text
            .as_deref()
            .map(|text| text.lines().count().try_into().unwrap_or(u32::MAX))
            .unwrap_or(0);
        files.push(FileObservation {
            path: relative,
            content_sha256: Sha256Hash::digest(&bytes),
            size_bytes: metadata.len(),
            text,
            language_id: language_for(&path),
            line_count,
        });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let verification_limitations = verify_observed_files(root, &files);
    if !verification_limitations.is_empty() {
        completeness = Completeness::Partial;
        limitations.extend(verification_limitations);
    }
    limitations.sort();
    limitations.dedup();
    let entries: Vec<_> = files
        .iter()
        .map(|file| {
            serde_json::json!({
                "path":file.path,
                "kind":"file",
                "size_bytes":file.size_bytes,
                "content_sha256":file.content_sha256,
            })
        })
        .collect();
    let entries_manifest = serde_json::json!({
        "schema_version":1,
        "project_id":project.project_id,
        "entries":entries,
        "completeness":completeness,
        "limitations":limitations,
    });
    let entries_fingerprint =
        versioned_fingerprint("star.identity.workspace-entries", 1, &entries_manifest)
            .map_err(|_| ProjectError::Fingerprint)?;
    let mut revision = project_revision(project, root, &entries_fingerprint)?;
    if revision.revision_kind == star_contracts::management::RevisionKind::FilesystemManifest {
        revision.completeness = completeness;
        revision.limitations = limitations.clone();
    }
    let dirty_summary = git_dirty_summary(root).unwrap_or_default();
    let scan_config_fingerprint = versioned_fingerprint("star.scan-config", 1, policy)
        .map_err(|_| ProjectError::Fingerprint)?;
    Ok(ProjectObservation {
        revision,
        entries_manifest,
        entries_fingerprint,
        dirty_summary,
        completeness,
        limitations,
        files,
        scan_config_fingerprint,
    })
}

fn verify_observed_files(root: &Path, files: &[FileObservation]) -> Vec<String> {
    let mut limitations = Vec::new();
    for file in files {
        let verification_path = root.join(file.path.as_str());
        match fs::read(verification_path) {
            Ok(bytes) if Sha256Hash::digest(&bytes) == file.content_sha256 => {}
            Ok(_) => limitations.push("SCAN_SOURCE_CHANGED_DURING_RUN".to_owned()),
            Err(_) => limitations.push("SCAN_SOURCE_UNREADABLE".to_owned()),
        }
    }
    limitations.sort();
    limitations.dedup();
    limitations
}

fn project_revision(
    project: &Project,
    root: &Path,
    manifest_fingerprint: &Sha256Hash,
) -> Result<ProjectRevision, ProjectError> {
    let git = if matches!(project.repository_kind, RepositoryKind::Git) {
        let format = git_text(root, &["rev-parse", "--show-object-format"]);
        let commit = git_text(root, &["rev-parse", "HEAD"]);
        let tree = git_text(root, &["rev-parse", "HEAD^{tree}"]);
        format
            .ok()
            .zip(commit.ok())
            .zip(tree.ok())
            .map(|((format, commit), tree)| (format, commit, tree))
    } else {
        None
    };
    let (revision_kind, format, commit, tree, manifest) = if let Some((format, commit, tree)) = git
    {
        (
            star_contracts::management::RevisionKind::GitCommit,
            Some(format),
            Some(commit),
            Some(tree),
            None,
        )
    } else {
        (
            star_contracts::management::RevisionKind::FilesystemManifest,
            None,
            None,
            None,
            Some(manifest_fingerprint.clone()),
        )
    };
    let identity_payload = match revision_kind {
        star_contracts::management::RevisionKind::GitCommit => serde_json::json!({
            "project_id":project.project_id,
            "revision_kind":revision_kind,
            "vcs_object_format":format.as_deref().ok_or(ProjectError::Fingerprint)?,
            "commit_id":commit.as_deref().ok_or(ProjectError::Fingerprint)?,
            "tree_id":tree.as_deref().ok_or(ProjectError::Fingerprint)?,
        }),
        star_contracts::management::RevisionKind::FilesystemManifest => serde_json::json!({
            "project_id":project.project_id,
            "revision_kind":revision_kind,
            "manifest_fingerprint":manifest.as_ref().ok_or(ProjectError::Fingerprint)?,
        }),
    };
    let identity = versioned_fingerprint("star.identity.project-revision", 1, &identity_payload)
        .map_err(|_| ProjectError::Fingerprint)?;
    Ok(ProjectRevision {
        schema_id: "star.project-revision".to_owned(),
        schema_version: 1,
        project_revision_id: ProjectRevisionId::from_fingerprint(&identity),
        project_id: project.project_id.clone(),
        revision_kind,
        vcs_object_format: format,
        commit_id: commit,
        tree_id: tree,
        manifest_fingerprint: manifest,
        captured_at: Utc::now(),
        completeness: Completeness::Complete,
        limitations: vec![],
    })
}

pub(crate) fn hidden_command(executable: &str) -> Command {
    let mut command = Command::new(executable);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    command
}

fn git_text(root: &Path, arguments: &[&str]) -> Result<String, ProjectError> {
    let output = hidden_command("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .map_err(|_| ProjectError::Io)?;
    if !output.status.success() {
        return Err(ProjectError::Io);
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|_| ProjectError::Io)
}

fn git_paths(
    root: &Path,
    include_untracked: bool,
    include_ignored: bool,
) -> Result<Vec<PathBuf>, ProjectError> {
    let mut command = hidden_command("git");
    command
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z", "--cached"]);
    if include_untracked {
        command.arg("--others");
        if !include_ignored {
            command.arg("--exclude-standard");
        }
    }
    if include_ignored {
        command.arg("--ignored").arg("--exclude-standard");
    }
    let output = command.output().map_err(|_| ProjectError::Io)?;
    if !output.status.success() {
        return Err(ProjectError::Io);
    }
    let mut unique = BTreeSet::new();
    for raw in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|raw| !raw.is_empty())
    {
        let relative = String::from_utf8(raw.to_vec()).map_err(|_| ProjectError::Io)?;
        unique.insert(root.join(relative));
    }
    Ok(unique.into_iter().collect())
}

fn filesystem_paths(root: &Path, policy: &ScanPolicy) -> Result<Vec<PathBuf>, ProjectError> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let mut entries: Vec<_> = fs::read_dir(&directory)
            .map_err(|_| ProjectError::Io)?
            .collect::<Result<_, _>>()
            .map_err(|_| ProjectError::Io)?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|_| ProjectError::InvalidRoot)?;
            if is_excluded_relative_path(relative, policy) {
                continue;
            }
            if relative.components().next().is_some_and(|component| {
                matches!(component.as_os_str().to_str(), Some(".git" | ".ai-runs"))
            }) {
                continue;
            }
            let metadata = fs::symlink_metadata(&path).map_err(|_| ProjectError::Io)?;
            if metadata.file_type().is_symlink() && !policy.follow_symlinks {
                continue;
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() {
                files.push(path);
                if files.len() > policy.max_files {
                    return Err(ProjectError::ResourceLimit);
                }
            }
        }
    }
    Ok(files)
}

fn is_excluded_relative_path(relative: &Path, policy: &ScanPolicy) -> bool {
    let relative_text = relative
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/");
    policy.excluded_relative_roots.iter().any(|excluded| {
        relative_text == excluded.as_str()
            || relative_text.starts_with(&format!("{}/", excluded.as_str()))
    })
}

fn relative_path(root: &Path, path: &Path) -> Result<ProjectPathRef, ProjectError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| ProjectError::InvalidRoot)?;
    let parts: Option<Vec<_>> = relative
        .components()
        .map(|component| component.as_os_str().to_str().map(str::to_owned))
        .collect();
    ProjectPathRef::parse(parts.ok_or(ProjectError::InvalidRoot)?.join("/"))
        .map_err(|_| ProjectError::InvalidRoot)
}

fn git_dirty_summary(root: &Path) -> Result<BTreeMap<String, u64>, ProjectError> {
    let output = hidden_command("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain=v1", "-z"])
        .output()
        .map_err(|_| ProjectError::Io)?;
    if !output.status.success() {
        return Err(ProjectError::Io);
    }
    let mut counts = BTreeMap::from([
        ("modified".to_owned(), 0),
        ("added".to_owned(), 0),
        ("deleted".to_owned(), 0),
        ("untracked".to_owned(), 0),
    ]);
    for record in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|record| record.len() >= 2)
    {
        let status = &record[..2];
        let key = if status == b"??" {
            "untracked"
        } else if status.contains(&b'D') {
            "deleted"
        } else if status.contains(&b'A') {
            "added"
        } else {
            "modified"
        };
        *counts.get_mut(key).expect("known dirty status") += 1;
    }
    Ok(counts)
}

fn language_for(path: &Path) -> Option<String> {
    let language = match path.extension().and_then(|value| value.to_str())? {
        "rs" => "rust",
        "toml" => "toml",
        "json" => "json",
        "md" => "markdown",
        "yml" | "yaml" => "yaml",
        "ps1" => "powershell",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        _ => "text",
    };
    Some(language.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(root: &Path, arguments: &[&str]) {
        let status = hidden_command("git")
            .arg("-C")
            .arg(root)
            .args(arguments)
            .status()
            .unwrap();
        assert!(status.success(), "git {arguments:?}");
    }

    #[test]
    fn local_first_project_and_scan_manifest_use_only_relative_paths() {
        let root = std::env::temp_dir().join(format!(
            "star-project-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), b"fn main() {}  \n").unwrap();
        fs::write(root.join("src/private.txt"), b"token=do-not-persist\n").unwrap();
        fs::create_dir_all(root.join(".ai-runs")).unwrap();
        fs::write(root.join(".ai-runs/ignored.log"), b"ignored").unwrap();
        let seed = ProjectSeed::discover(&root.canonicalize().unwrap()).unwrap();
        assert_eq!(seed.identity_scope, IdentityScope::Local);
        let attached = seed
            .attach(
                CheckoutId::new(),
                RootBindingId::new(),
                &root.canonicalize().unwrap(),
            )
            .unwrap();
        let observation = observe_project(
            &attached.project,
            &root.canonicalize().unwrap(),
            &ScanPolicy::default(),
        )
        .unwrap();
        let serialized = serde_json::to_string(&observation.entries_manifest).unwrap();
        assert!(serialized.contains("src/lib.rs"));
        assert!(!serialized.contains(&root.to_string_lossy().to_string()));
        assert!(!serialized.contains("ignored.log"));
        assert!(!serialized.contains("private.txt"));
        assert_eq!(observation.completeness, Completeness::Partial);
        assert!(
            observation
                .limitations
                .contains(&"sensitive_literal_discarded".to_owned())
        );
    }

    #[test]
    fn verification_detects_changed_bytes_and_nested_ownership_excludes_child_sources() {
        let root = std::env::temp_dir().join(format!(
            "star-project-change-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join("root.txt"), b"root\n").unwrap();
        fs::write(root.join("nested/child.txt"), b"child\n").unwrap();
        let seed = ProjectSeed::discover(&root.canonicalize().unwrap()).unwrap();
        let attached = seed
            .attach(
                CheckoutId::new(),
                RootBindingId::new(),
                &root.canonicalize().unwrap(),
            )
            .unwrap();
        let policy = ScanPolicy {
            excluded_relative_roots: vec![ProjectPathRef::parse("nested").unwrap()],
            ..ScanPolicy::default()
        };
        let observation =
            observe_project(&attached.project, &root.canonicalize().unwrap(), &policy).unwrap();
        assert_eq!(observation.files.len(), 1);
        assert_eq!(observation.files[0].path.as_str(), "root.txt");
        fs::write(root.join("root.txt"), b"changed\n").unwrap();
        assert_eq!(
            verify_observed_files(&root.canonicalize().unwrap(), &observation.files),
            vec!["SCAN_SOURCE_CHANGED_DURING_RUN".to_owned()]
        );
    }

    #[test]
    fn workspace_change_observation_separates_staged_unstaged_untracked_and_rename() {
        let root = std::env::temp_dir().join(format!(
            "star-project-dirty-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), b"pub fn value() -> u32 { 1 }\n").unwrap();
        fs::write(root.join("src/old.rs"), b"pub fn old() {}\n").unwrap();
        git(&root, &["init", "--initial-branch=main"]);
        git(&root, &["config", "user.email", "fixture@example.invalid"]);
        git(&root, &["config", "user.name", "Fixture"]);
        git(&root, &["add", "."]);
        git(&root, &["commit", "-m", "baseline"]);
        fs::write(root.join("src/lib.rs"), b"pub fn value() -> u32 { 2 }\n").unwrap();
        git(&root, &["add", "src/lib.rs"]);
        fs::write(root.join("src/lib.rs"), b"pub fn value() -> u32 { 3 }\n").unwrap();
        git(&root, &["mv", "src/old.rs", "src/new.rs"]);
        fs::write(root.join("src/untracked.rs"), b"pub fn untracked() {}\n").unwrap();

        let canonical = root.canonicalize().unwrap();
        let seed = ProjectSeed::discover(&canonical).unwrap();
        let attached = seed
            .attach(CheckoutId::new(), RootBindingId::new(), &canonical)
            .unwrap();
        let observation =
            observe_project(&attached.project, &canonical, &ScanPolicy::default()).unwrap();
        let current_sources = observation
            .files
            .iter()
            .map(|file| star_contracts::index::SourceEntry {
                canonical_source_id: CanonicalSourceId::from_stable_bytes(
                    file.path.as_str().as_bytes(),
                ),
                path: file.path.clone(),
                content_sha256: file.content_sha256.clone(),
                size_bytes: file.size_bytes,
                source_class: star_contracts::index::SourceClass::Source,
                facets: vec![],
                language_id: file
                    .language_id
                    .clone()
                    .unwrap_or_else(|| "text".to_owned()),
                encoding: "utf-8".to_owned(),
                owner_project_id: attached.project.project_id.clone(),
                owner_checkout_id: attached.checkout.checkout_id.clone(),
                analysis_eligible: true,
                content_fingerprint: file.content_sha256.clone(),
            })
            .collect::<Vec<_>>();
        let changes =
            observe_workspace_changes(&attached.project, &canonical, &current_sources).unwrap();
        assert_eq!(changes.completeness, Completeness::Complete);
        let modified = changes
            .entries
            .iter()
            .find(|entry| entry.path.as_str() == "src/lib.rs")
            .unwrap();
        assert!(modified.staged && modified.unstaged && !modified.untracked);
        assert!(modified.before_sha256.is_some() && modified.after_sha256.is_some());
        let renamed = changes
            .entries
            .iter()
            .find(|entry| entry.path.as_str() == "src/new.rs")
            .unwrap();
        assert_eq!(renamed.change_kind, ObservedChangeKind::Rename);
        assert_eq!(renamed.rename_from.as_ref().unwrap().as_str(), "src/old.rs");
        let untracked = changes
            .entries
            .iter()
            .find(|entry| entry.path.as_str() == "src/untracked.rs")
            .unwrap();
        assert!(untracked.untracked && untracked.before_sha256.is_none());
    }
}
