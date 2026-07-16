//! Git-tracked project allowlist parsing and exact-root read-only inspection.
//!
//! This module deliberately does not discover directories recursively. The
//! manifest is canonical input; every observed value is derived and can be
//! rebuilt without changing project registration state.

use std::{
    collections::BTreeSet,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use star_contracts::Sha256Hash;
use thiserror::Error;

pub const PROJECT_CATALOG_VIEW_SCHEMA_ID: &str = "star.project-catalog-view";
pub const PROJECT_STATUS_VIEW_SCHEMA_ID: &str = "star.project-status-view";

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProjectCatalogError {
    #[error("project catalog manifest is invalid")]
    InvalidManifest,
    #[error("project catalog root is invalid")]
    InvalidRoot,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectCatalogManifest {
    pub schema_version: u32,
    pub catalog_id: String,
    pub registration_enabled: bool,
    pub root_env: String,
    pub default_root: String,
    pub projects: Vec<ProjectCatalogEntry>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectCatalogEntry {
    pub project_key: String,
    pub display_name: String,
    pub relative_path: String,
    pub role: CatalogProjectRole,
    pub repository_kind: CatalogRepositoryKind,
    pub expected_origin: Option<String>,
    pub canonical_project_key: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogProjectRole {
    ActiveCanonical,
    LinkedWorktree,
    ReadOnlyMigrationSource,
    Backup,
    Sandbox,
    BootstrapCheckout,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogRepositoryKind {
    Git,
    Directory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogAvailability {
    Available,
    UnavailableRoot,
    NotGit,
    GitProbeFailed,
    TopLevelMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogCheckoutKind {
    CanonicalWorktree,
    LinkedWorktree,
    Directory,
    Unavailable,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogMatchState {
    Match,
    Mismatch,
    Unavailable,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogIdentityStatus {
    Match,
    Mismatch,
    Unverified,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProjectCatalogSummary {
    pub total_projects: usize,
    pub active_canonical_projects: usize,
    pub available_projects: usize,
    pub unavailable_projects: usize,
    pub identity_mismatches: usize,
    pub identity_unverified: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProjectCatalogStatus {
    pub schema_id: &'static str,
    pub schema_version: u32,
    pub project_key: String,
    pub display_name: String,
    pub relative_path: String,
    pub declared_role: CatalogProjectRole,
    pub repository_kind: CatalogRepositoryKind,
    pub availability: CatalogAvailability,
    pub checkout_kind: CatalogCheckoutKind,
    pub identity_status: CatalogIdentityStatus,
    pub origin_status: CatalogMatchState,
    pub git_common_dir_status: CatalogMatchState,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProjectCatalogView {
    pub schema_id: &'static str,
    pub schema_version: u32,
    pub catalog_id: String,
    pub source_fingerprint: Sha256Hash,
    pub registration_enabled: bool,
    pub summary: ProjectCatalogSummary,
    pub items: Vec<ProjectCatalogStatus>,
}

pub fn parse_project_catalog(source: &str) -> Result<ProjectCatalogManifest, ProjectCatalogError> {
    let manifest: ProjectCatalogManifest =
        toml::from_str(source).map_err(|_| ProjectCatalogError::InvalidManifest)?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

pub fn resolve_project_catalog_root(
    manifest: &ProjectCatalogManifest,
) -> Result<PathBuf, ProjectCatalogError> {
    let value = std::env::var_os(&manifest.root_env)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(&manifest.default_root));
    if !is_absolute_catalog_path(&value) {
        return Err(ProjectCatalogError::InvalidRoot);
    }
    Ok(value)
}

pub fn inspect_project_catalog(
    manifest: &ProjectCatalogManifest,
    source: &str,
    root: &Path,
) -> ProjectCatalogView {
    let mut items = Vec::with_capacity(manifest.projects.len());
    for projects in manifest.projects.chunks(8) {
        std::thread::scope(|scope| {
            let handles: Vec<_> = projects
                .iter()
                .map(|entry| scope.spawn(move || inspect_entry(manifest, entry, root)))
                .collect();
            for handle in handles {
                items.push(handle.join().expect("catalog probe thread must not panic"));
            }
        });
    }
    items.sort_by(|left, right| left.project_key.cmp(&right.project_key));
    let summary = ProjectCatalogSummary {
        total_projects: items.len(),
        active_canonical_projects: items
            .iter()
            .filter(|item| item.declared_role == CatalogProjectRole::ActiveCanonical)
            .count(),
        available_projects: items
            .iter()
            .filter(|item| item.availability == CatalogAvailability::Available)
            .count(),
        unavailable_projects: items
            .iter()
            .filter(|item| item.availability != CatalogAvailability::Available)
            .count(),
        identity_mismatches: items
            .iter()
            .filter(|item| item.identity_status == CatalogIdentityStatus::Mismatch)
            .count(),
        identity_unverified: items
            .iter()
            .filter(|item| item.identity_status == CatalogIdentityStatus::Unverified)
            .count(),
    };
    ProjectCatalogView {
        schema_id: PROJECT_CATALOG_VIEW_SCHEMA_ID,
        schema_version: 1,
        catalog_id: manifest.catalog_id.clone(),
        source_fingerprint: Sha256Hash::digest(source.as_bytes()),
        registration_enabled: manifest.registration_enabled,
        summary,
        items,
    }
}

pub fn inspect_project_catalog_entry(
    manifest: &ProjectCatalogManifest,
    root: &Path,
    project_key: &str,
) -> Option<ProjectCatalogStatus> {
    manifest
        .projects
        .iter()
        .find(|entry| entry.project_key == project_key)
        .map(|entry| inspect_entry(manifest, entry, root))
}

pub fn project_status<'a>(
    view: &'a ProjectCatalogView,
    project_key: &str,
) -> Option<&'a ProjectCatalogStatus> {
    view.items
        .iter()
        .find(|item| item.project_key == project_key)
}

fn validate_manifest(manifest: &ProjectCatalogManifest) -> Result<(), ProjectCatalogError> {
    if manifest.schema_version != 1
        || manifest.catalog_id.trim().is_empty()
        || manifest.catalog_id.chars().count() > 128
        || !valid_environment_name(&manifest.root_env)
        || !is_absolute_catalog_path(Path::new(&manifest.default_root))
        || manifest.projects.is_empty()
        || manifest.projects.len() > 128
    {
        return Err(ProjectCatalogError::InvalidManifest);
    }
    let mut keys = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for project in &manifest.projects {
        let key = project.project_key.to_ascii_lowercase();
        let path = project.relative_path.replace('\\', "/").to_lowercase();
        if !valid_project_key(&project.project_key)
            || project.display_name.trim().is_empty()
            || project.display_name.chars().count() > 200
            || !valid_relative_path(&project.relative_path)
            || !keys.insert(key)
            || !paths.insert(path)
            || (project.role == CatalogProjectRole::ActiveCanonical
                && project.repository_kind == CatalogRepositoryKind::Git
                && project.expected_origin.as_deref().is_none_or(str::is_empty))
            || (project.role == CatalogProjectRole::LinkedWorktree
                && project
                    .canonical_project_key
                    .as_deref()
                    .is_none_or(str::is_empty))
            || project
                .expected_origin
                .as_deref()
                .is_some_and(|origin| normalize_origin(origin).is_none())
        {
            return Err(ProjectCatalogError::InvalidManifest);
        }
    }
    let known: BTreeSet<_> = manifest
        .projects
        .iter()
        .map(|project| project.project_key.as_str())
        .collect();
    if manifest.projects.iter().any(|project| {
        let canonical = project.canonical_project_key.as_deref();
        if project.role != CatalogProjectRole::LinkedWorktree {
            return canonical.is_some();
        }
        if project.repository_kind != CatalogRepositoryKind::Git {
            return true;
        }
        canonical.is_none_or(|key| {
            key == project.project_key
                || !known.contains(key)
                || manifest.projects.iter().all(|candidate| {
                    candidate.project_key != key
                        || candidate.role != CatalogProjectRole::ActiveCanonical
                        || candidate.repository_kind != CatalogRepositoryKind::Git
                })
        })
    }) {
        return Err(ProjectCatalogError::InvalidManifest);
    }
    Ok(())
}

fn inspect_entry(
    manifest: &ProjectCatalogManifest,
    entry: &ProjectCatalogEntry,
    root: &Path,
) -> ProjectCatalogStatus {
    let project_root = root.join(Path::new(&entry.relative_path));
    let mut status = ProjectCatalogStatus {
        schema_id: PROJECT_STATUS_VIEW_SCHEMA_ID,
        schema_version: 1,
        project_key: entry.project_key.clone(),
        display_name: entry.display_name.clone(),
        relative_path: entry.relative_path.replace('\\', "/"),
        declared_role: entry.role,
        repository_kind: entry.repository_kind,
        availability: CatalogAvailability::UnavailableRoot,
        checkout_kind: CatalogCheckoutKind::Unavailable,
        identity_status: CatalogIdentityStatus::Unverified,
        origin_status: CatalogMatchState::Unavailable,
        git_common_dir_status: CatalogMatchState::Unavailable,
        limitations: Vec::new(),
    };
    if !project_root.is_dir() {
        status.limitations.push("unavailable_root".to_owned());
        return status;
    }
    if entry.repository_kind == CatalogRepositoryKind::Directory {
        status.availability = CatalogAvailability::Available;
        status.checkout_kind = CatalogCheckoutKind::Directory;
        status.origin_status = CatalogMatchState::NotApplicable;
        status.git_common_dir_status = CatalogMatchState::NotApplicable;
        status.identity_status = CatalogIdentityStatus::Match;
        return status;
    }
    let Some(identity) = git_lines(
        &project_root,
        &[
            "rev-parse",
            "--show-toplevel",
            "--path-format=absolute",
            "--git-common-dir",
        ],
    ) else {
        status.availability = if project_root.join(".git").exists() {
            CatalogAvailability::GitProbeFailed
        } else {
            CatalogAvailability::NotGit
        };
        status.checkout_kind = CatalogCheckoutKind::Unknown;
        status.limitations.push(
            if status.availability == CatalogAvailability::GitProbeFailed {
                "git_identity_unavailable"
            } else {
                "not_git"
            }
            .to_owned(),
        );
        return status;
    };
    if identity.len() != 2 {
        status.availability = CatalogAvailability::GitProbeFailed;
        status.checkout_kind = CatalogCheckoutKind::Unknown;
        status
            .limitations
            .push("git_identity_unavailable".to_owned());
        return status;
    }
    if normalize_path_text(Path::new(&identity[0])) != normalize_path_text(&project_root) {
        status.availability = CatalogAvailability::TopLevelMismatch;
        status.checkout_kind = CatalogCheckoutKind::Unknown;
        status.identity_status = CatalogIdentityStatus::Mismatch;
        status.limitations.push("top_level_mismatch".to_owned());
        return status;
    }
    let common_dir = &identity[1];
    status.availability = CatalogAvailability::Available;
    let self_common_dir = project_root.join(".git");
    let common_matches_self =
        normalize_path_text(Path::new(&common_dir)) == normalize_path_text(&self_common_dir);
    status.checkout_kind = if common_matches_self {
        CatalogCheckoutKind::CanonicalWorktree
    } else {
        CatalogCheckoutKind::LinkedWorktree
    };
    let expected_common_dir =
        expected_git_common_dir(manifest, entry, root).unwrap_or(self_common_dir);
    status.git_common_dir_status = if normalize_path_text(Path::new(common_dir))
        == normalize_path_text(&expected_common_dir)
    {
        CatalogMatchState::Match
    } else {
        CatalogMatchState::Mismatch
    };
    if status.git_common_dir_status == CatalogMatchState::Mismatch {
        status
            .limitations
            .push("git_common_dir_mismatch".to_owned());
    }
    status.origin_status = match entry.expected_origin.as_deref() {
        None => CatalogMatchState::NotApplicable,
        Some(expected) => git_text(&project_root, &["remote", "get-url", "origin"])
            .and_then(|observed| {
                normalize_origin(expected)
                    .zip(normalize_origin(&observed))
                    .map(|(expected, observed)| {
                        if expected == observed {
                            CatalogMatchState::Match
                        } else {
                            CatalogMatchState::Mismatch
                        }
                    })
            })
            .unwrap_or(CatalogMatchState::Unavailable),
    };
    match status.origin_status {
        CatalogMatchState::Mismatch => status.limitations.push("origin_mismatch".to_owned()),
        CatalogMatchState::Unavailable => status.limitations.push("origin_unavailable".to_owned()),
        CatalogMatchState::Match | CatalogMatchState::NotApplicable => {}
    }
    status.identity_status = if status.git_common_dir_status == CatalogMatchState::Mismatch
        || status.origin_status == CatalogMatchState::Mismatch
    {
        CatalogIdentityStatus::Mismatch
    } else if status.git_common_dir_status == CatalogMatchState::Unavailable
        || status.origin_status == CatalogMatchState::Unavailable
    {
        CatalogIdentityStatus::Unverified
    } else {
        CatalogIdentityStatus::Match
    };
    status
}

fn expected_git_common_dir(
    manifest: &ProjectCatalogManifest,
    entry: &ProjectCatalogEntry,
    root: &Path,
) -> Option<PathBuf> {
    let canonical_key = entry.canonical_project_key.as_deref()?;
    manifest
        .projects
        .iter()
        .find(|candidate| candidate.project_key == canonical_key)
        .map(|canonical| root.join(&canonical.relative_path).join(".git"))
}

fn git_text(root: &Path, arguments: &[&str]) -> Option<String> {
    let mut lines = git_lines(root, arguments)?;
    (lines.len() == 1).then(|| lines.remove(0))
}

fn git_lines(root: &Path, arguments: &[&str]) -> Option<Vec<String>> {
    let output = crate::hidden_command("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let lines: Vec<_> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    (!lines.is_empty()).then_some(lines)
}

fn normalize_path_text(path: &Path) -> String {
    path.as_os_str()
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
}

fn normalize_origin(origin: &str) -> Option<String> {
    let mut value = origin.trim().replace('\\', "/");
    if value.contains('\0') || value.is_empty() || value.chars().count() > 2_048 {
        return None;
    }
    if let Some((user_host, path)) = value.split_once(':')
        && user_host.contains('@')
        && !user_host.contains('/')
    {
        let host = user_host.split_once('@')?.1;
        value = format!("{host}/{path}");
    } else {
        value = value
            .strip_prefix("https://")
            .or_else(|| value.strip_prefix("http://"))
            .or_else(|| value.strip_prefix("ssh://"))
            .unwrap_or(&value)
            .to_owned();
    }
    while value.ends_with('/') {
        value.pop();
    }
    if value.to_ascii_lowercase().ends_with(".git") {
        value.truncate(value.len() - 4);
    }
    (!value.is_empty()).then(|| value.to_lowercase())
}

fn valid_environment_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_project_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_lowercase()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

fn valid_relative_path(value: &str) -> bool {
    if value.is_empty()
        || value.chars().count() > 1_024
        || value.contains('\0')
        || value.contains(':')
        || value.contains('\\')
    {
        return false;
    }
    let path = Path::new(value);
    !path.is_absolute()
        && path.components().all(|component| {
            matches!(component, Component::Normal(_)) && !component.as_os_str().is_empty()
        })
}

fn is_absolute_catalog_path(path: &Path) -> bool {
    let value = path.as_os_str().to_string_lossy();
    if value.starts_with("\\\\") || value.starts_with("//") {
        return false;
    }
    let bytes = value.as_bytes();
    path.is_absolute()
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\'))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIVE_CATALOG: &str = include_str!("../../../../catalog/projects.toml");

    #[test]
    fn tracked_catalog_has_exactly_thirteen_active_projects_and_registration_is_disabled() {
        let manifest = parse_project_catalog(LIVE_CATALOG).unwrap();
        assert!(!manifest.registration_enabled);
        assert_eq!(
            manifest
                .projects
                .iter()
                .filter(|project| project.role == CatalogProjectRole::ActiveCanonical)
                .count(),
            13
        );
        assert_eq!(manifest.projects.len(), 13);
        assert!(
            manifest
                .projects
                .iter()
                .all(|project| project.role == CatalogProjectRole::ActiveCanonical)
        );
    }

    #[test]
    fn declarations_reject_duplicates_parent_paths_and_unknown_link_targets() {
        let source = r#"
schema_version = 1
catalog_id = "test"
registration_enabled = false
root_env = "STAR_DEVELOPMENT_ROOT"
default_root = "D:/dev"

[[projects]]
project_key = "one"
display_name = "One"
relative_path = "one"
role = "active_canonical"
repository_kind = "git"
expected_origin = "https://example.test/one.git"

[[projects]]
project_key = "two"
display_name = "Two"
relative_path = "../one"
role = "linked_worktree"
repository_kind = "git"
canonical_project_key = "missing"
"#;
        assert_eq!(
            parse_project_catalog(source).unwrap_err(),
            ProjectCatalogError::InvalidManifest
        );
    }

    #[test]
    fn role_and_identity_states_remain_distinct_when_roots_are_unavailable() {
        let source = r#"
schema_version = 1
catalog_id = "test"
registration_enabled = false
root_env = "STAR_DEVELOPMENT_ROOT"
default_root = "D:/dev"

[[projects]]
project_key = "backup"
display_name = "Backup"
relative_path = "backup"
role = "backup"
repository_kind = "git"

[[projects]]
project_key = "sandbox"
display_name = "Sandbox"
relative_path = "sandbox"
role = "sandbox"
repository_kind = "git"

[[projects]]
project_key = "bootstrap"
display_name = "Bootstrap"
relative_path = "bootstrap"
role = "bootstrap_checkout"
repository_kind = "git"
"#;
        let manifest = parse_project_catalog(source).unwrap();
        let root = std::env::temp_dir().join(format!(
            "star-project-catalog-missing-{}-{}",
            std::process::id(),
            Sha256Hash::digest(source.as_bytes())
        ));
        let view = inspect_project_catalog(&manifest, source, &root);
        assert_eq!(view.summary.unavailable_projects, 3);
        assert_eq!(view.summary.identity_unverified, 3);
        assert_eq!(view.items[0].declared_role, CatalogProjectRole::Backup);
        assert_eq!(
            view.items[1].declared_role,
            CatalogProjectRole::BootstrapCheckout
        );
        assert_eq!(view.items[2].declared_role, CatalogProjectRole::Sandbox);
        assert!(view.items.iter().all(|item| {
            item.availability == CatalogAvailability::UnavailableRoot
                && item.identity_status == CatalogIdentityStatus::Unverified
        }));
    }

    #[test]
    fn origin_normalization_accepts_https_and_ssh_forms() {
        assert_eq!(
            normalize_origin("https://github.com/Owner/Repo.git"),
            normalize_origin("git@github.com:owner/repo.git")
        );
    }

    #[test]
    fn common_directory_classifier_distinguishes_linked_worktrees() {
        let root = Path::new("D:/dev/project");
        assert_eq!(
            normalize_path_text(&root.join(".git")),
            normalize_path_text(Path::new("d:\\dev\\project\\.git"))
        );
        assert_ne!(
            normalize_path_text(&root.join(".git")),
            normalize_path_text(Path::new("D:/dev/project-main/.git"))
        );
    }

    #[test]
    fn linked_worktree_common_directory_is_bound_to_its_canonical_entry() {
        let source = r#"
schema_version = 1
catalog_id = "test"
registration_enabled = false
root_env = "STAR_DEVELOPMENT_ROOT"
default_root = "D:/dev"

[[projects]]
project_key = "canonical"
display_name = "Canonical"
relative_path = "canonical"
role = "active_canonical"
repository_kind = "git"
expected_origin = "https://example.test/canonical.git"

[[projects]]
project_key = "linked"
display_name = "Linked"
relative_path = "linked"
role = "linked_worktree"
repository_kind = "git"
canonical_project_key = "canonical"
"#;
        let manifest = parse_project_catalog(source).unwrap();
        let linked = manifest
            .projects
            .iter()
            .find(|project| project.project_key == "linked")
            .unwrap();
        assert_eq!(
            expected_git_common_dir(&manifest, linked, Path::new("D:/dev")),
            Some(PathBuf::from("D:/dev/canonical/.git"))
        );
    }

    #[test]
    fn catalog_paths_reject_drive_relative_unc_and_backslash_relative_forms() {
        assert!(is_absolute_catalog_path(Path::new("D:/dev")));
        assert!(is_absolute_catalog_path(Path::new("D:\\dev")));
        assert!(!is_absolute_catalog_path(Path::new("D:dev")));
        assert!(!is_absolute_catalog_path(Path::new("//server/share")));
        assert!(!is_absolute_catalog_path(Path::new("\\\\server\\share")));
        assert!(valid_relative_path("group/project"));
        assert!(!valid_relative_path("group\\project"));
    }
}
