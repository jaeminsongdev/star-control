//! Read-only Git history observation adapter for code-health maintenance.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use chrono::{DateTime, NaiveDate, Utc};
use star_contracts::{
    Sha256Hash,
    maintenance_v2::{
        GIT_HISTORY_RISK_SNAPSHOT_SCHEMA_ID, GitHistoryCompleteness, GitHistoryComponentRisk,
        GitHistoryRiskSnapshot,
    },
};
use star_domain::versioned_fingerprint;
use star_ports::{GitHistoryObservationRequest, GitHistoryPort, GitHistoryPortError};

const MAX_DEBT_SCAN_ENTRIES: usize = 100_000;
const MAX_DEBT_SCAN_DEPTH: usize = 64;
const MAX_DEBT_MARKERS: usize = 10_000;

/// Executes only read-only Git queries.  It deliberately accepts no arbitrary
/// command or native paths from callers.
#[derive(Clone, Copy, Debug, Default)]
pub struct CommandGitHistoryAdapter;

impl GitHistoryPort for CommandGitHistoryAdapter {
    fn observe(
        &self,
        project_root: &Path,
        request: &GitHistoryObservationRequest,
    ) -> Result<GitHistoryRiskSnapshot, GitHistoryPortError> {
        if !valid_revision(&request.range_end)
            || request
                .range_start
                .as_deref()
                .is_some_and(|value| !valid_revision(value))
            || !(1..=10_000).contains(&request.commit_limit)
        {
            return Err(GitHistoryPortError::Invalid);
        }
        let evaluation_date = DateTime::parse_from_rfc3339(&request.evaluation_time)
            .map_err(|_| GitHistoryPortError::Invalid)?
            .with_timezone(&Utc)
            .date_naive();
        let range_end = git(
            project_root,
            [
                "rev-parse",
                "--verify",
                &format!("{}^{{commit}}", request.range_end),
            ],
        )?;
        let range_end = range_end.trim().to_owned();
        let range_start = request
            .range_start
            .as_ref()
            .map(|start| {
                git(
                    project_root,
                    ["rev-parse", "--verify", &format!("{start}^{{commit}}",)],
                )
                .map(|value| value.trim().to_owned())
            })
            .transpose()?;
        let range = range_start
            .as_ref()
            .map(|start| format!("{start}..{range_end}"))
            .unwrap_or_else(|| range_end.clone());
        let shallow = git(project_root, ["rev-parse", "--is-shallow-repository"])?;
        let repository = git(project_root, ["rev-parse", "--git-common-dir"])?;
        let count_limit = format!("--max-count={}", request.commit_limit.saturating_add(1));
        let observed_commit_count = git(
            project_root,
            ["rev-list", "--count", count_limit.as_str(), range.as_str()],
        )?
        .trim()
        .parse::<u32>()
        .map_err(|_| GitHistoryPortError::Malformed)?;
        let commit_limit_reached = observed_commit_count > request.commit_limit;
        let commits = git(
            project_root,
            [
                "log",
                "--format=%H",
                "--numstat",
                "--no-renames",
                "-n",
                &request.commit_limit.to_string(),
                &range,
            ],
        )?;
        let mut components = BTreeMap::<String, (u32, u32, BTreeSet<String>)>::new();
        let mut current_commit = None;
        for line in commits.lines() {
            if is_commit_id(line) {
                current_commit = Some(line.to_owned());
                continue;
            }
            let Some((added, deleted, path)) = parse_numstat(line) else {
                continue;
            };
            if is_excluded(path) {
                continue;
            }
            let component = path.split('/').next().unwrap_or(".").to_owned();
            let entry = components.entry(component).or_default();
            entry.0 = entry.0.saturating_add(1);
            entry.1 = entry.1.saturating_add(added.saturating_add(deleted));
            if let Some(commit) = &current_commit {
                entry.2.insert(commit.clone());
            }
        }
        let mut limitations = Vec::new();
        let history_completeness = match (shallow.trim(), commit_limit_reached) {
            ("false", false) => GitHistoryCompleteness::Complete,
            ("false", true) => {
                limitations.push("GIT_HISTORY_COMMIT_LIMIT_REACHED".to_owned());
                GitHistoryCompleteness::Partial
            }
            ("true", truncated) => {
                limitations.push("GIT_HISTORY_SHALLOW".to_owned());
                if truncated {
                    limitations.push("GIT_HISTORY_COMMIT_LIMIT_REACHED".to_owned());
                }
                GitHistoryCompleteness::Unverified
            }
            _ => return Err(GitHistoryPortError::Malformed),
        };
        if components.is_empty() {
            limitations.push("GIT_HISTORY_EMPTY_OR_BINARY".to_owned());
        }
        let codeowners = read_codeowners(project_root);
        let (codeowners_fingerprint, codeowners_limitations) = match codeowners.as_deref() {
            Some(value) => (Some(Sha256Hash::digest(value.as_bytes())), Vec::new()),
            None => (None, vec!["CODEOWNERS_MISSING".to_owned()]),
        };
        limitations.extend(codeowners_limitations);
        let components = components
            .into_iter()
            .map(
                |(component, (changed_file_count, relative_churn, commits))| {
                    let owners = codeowners_for(codeowners.as_deref(), &component);
                    let codeowners_matched = owners.is_some();
                    GitHistoryComponentRisk {
                        component,
                        changed_file_count,
                        relative_churn,
                        change_burst: commits.len() as u32,
                        opaque_owner_buckets: owners
                            .as_ref()
                            .map(|owners| vec![format!("declared_owner_count:{}", owners.len())])
                            .unwrap_or_default(),
                        declared_owner_count: owners.map_or(0, |owners| owners.len() as u32),
                        limitations: if codeowners_matched {
                            Vec::new()
                        } else {
                            vec!["CODEOWNERS_MISSING_OR_UNMATCHED".to_owned()]
                        },
                    }
                },
            )
            .collect::<Vec<_>>();
        let (debt_markers, mut debt_limitations) = debt_markers(project_root, evaluation_date);
        limitations.append(&mut debt_limitations);
        limitations.sort();
        limitations.dedup();
        let repository_identity = repository_identity(project_root, repository.trim())?;
        let content_fingerprint = versioned_fingerprint(
            "star.git-history-risk-snapshot",
            1,
            &serde_json::json!({
                "project_id":request.project_id,
                "repository_identity":repository_identity,
                "range_start":range_start,
                "range_end":range_end,
                "history_completeness":history_completeness,
                "codeowners_fingerprint":codeowners_fingerprint,
                "components":components,
                "debt_markers":debt_markers,
                "limitations":limitations,
            }),
        )
        .map_err(|_| GitHistoryPortError::Malformed)?;
        Ok(GitHistoryRiskSnapshot {
            schema_id: GIT_HISTORY_RISK_SNAPSHOT_SCHEMA_ID.to_owned(),
            schema_version: 1,
            project_id: request.project_id.clone(),
            repository_identity,
            range_start: range_start.unwrap_or_else(|| "ROOT".to_owned()),
            range_end,
            history_completeness,
            codeowners_fingerprint,
            components,
            debt_markers,
            limitations,
            content_fingerprint,
        })
    }
}

fn codeowners_for(source: Option<&str>, component: &str) -> Option<Vec<String>> {
    let source = source?;
    let mut selected = None;
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split_whitespace();
        let pattern = fields.next()?;
        let owners = fields.map(|_| "declared".to_owned()).collect::<Vec<_>>();
        if !owners.is_empty() && codeowners_matches(pattern, component) {
            selected = Some(owners);
        }
    }
    selected
}

fn read_codeowners(project_root: &Path) -> Option<String> {
    ["CODEOWNERS", ".github/CODEOWNERS"]
        .into_iter()
        .find_map(|relative| {
            let path = project_root.join(relative);
            let metadata = fs::symlink_metadata(&path).ok()?;
            if !metadata.is_file()
                || metadata_is_link_or_reparse(&metadata)
                || metadata.len() > 1_000_000
            {
                return None;
            }
            fs::read_to_string(path).ok()
        })
}

fn codeowners_matches(pattern: &str, component: &str) -> bool {
    let pattern = pattern.trim_start_matches('/').trim_end_matches('/');
    pattern == "*"
        || pattern == component
        || pattern.strip_suffix("/**") == Some(component)
        || pattern.strip_suffix("/*") == Some(component)
}

fn debt_markers(
    project_root: &Path,
    evaluation_date: NaiveDate,
) -> (
    Vec<star_contracts::maintenance_v2::DebtMarkerObservation>,
    Vec<String>,
) {
    let mut markers = Vec::new();
    let mut limitations = Vec::new();
    let mut visited_entries = 0_usize;
    visit_debt_markers(
        project_root,
        project_root,
        evaluation_date,
        &mut markers,
        &mut limitations,
        &mut visited_entries,
        0,
    );
    markers.sort_by(|left, right| left.marker_id.cmp(&right.marker_id));
    (markers, limitations)
}

fn visit_debt_markers(
    root: &Path,
    current: &Path,
    evaluation_date: NaiveDate,
    markers: &mut Vec<star_contracts::maintenance_v2::DebtMarkerObservation>,
    limitations: &mut Vec<String>,
    visited_entries: &mut usize,
    depth: usize,
) {
    if depth > MAX_DEBT_SCAN_DEPTH {
        limitations.push("DEBT_MARKER_DEPTH_LIMIT".to_owned());
        return;
    }
    let Ok(entries) = fs::read_dir(current) else {
        limitations.push("DEBT_MARKER_READ_FAILED".to_owned());
        return;
    };
    for entry in entries.flatten() {
        *visited_entries = visited_entries.saturating_add(1);
        if *visited_entries > MAX_DEBT_SCAN_ENTRIES {
            limitations.push("DEBT_MARKER_ENTRY_LIMIT".to_owned());
            return;
        }
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            limitations.push("DEBT_MARKER_READ_FAILED".to_owned());
            continue;
        };
        let relative = path.strip_prefix(root).ok().and_then(|path| path.to_str());
        let Some(relative) = relative.map(|path| path.replace('\\', "/")) else {
            limitations.push("DEBT_MARKER_NON_UTF8_PATH".to_owned());
            continue;
        };
        if relative == ".git" || relative.starts_with(".git/") {
            continue;
        }
        if metadata_is_link_or_reparse(&metadata) {
            limitations.push("DEBT_MARKER_LINK_SKIPPED".to_owned());
            continue;
        }
        if metadata.is_dir() {
            if !is_excluded(&relative) {
                visit_debt_markers(
                    root,
                    &path,
                    evaluation_date,
                    markers,
                    limitations,
                    visited_entries,
                    depth + 1,
                );
            }
            continue;
        }
        if is_excluded(&relative) || metadata.len() > 1_000_000 {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            limitations.push("DEBT_MARKER_READ_FAILED".to_owned());
            continue;
        };
        let Ok(source) = String::from_utf8(bytes) else {
            limitations.push("DEBT_MARKER_NON_UTF8".to_owned());
            continue;
        };
        for (index, line) in source.lines().enumerate() {
            for marker_kind in [
                "TODO",
                "FIXME",
                "HACK",
                "TEMP",
                "DEPRECATED",
                "REMOVE_AFTER",
            ] {
                if !line.contains(marker_kind) {
                    continue;
                }
                if markers.len() >= MAX_DEBT_MARKERS {
                    limitations.push("DEBT_MARKER_COUNT_LIMIT".to_owned());
                    return;
                }
                let structured =
                    line.contains("owner=") || line.contains("issue=") || line.contains("expires=");
                let raw_expiry = metadata_value(line, "expires");
                let expiry_date = raw_expiry
                    .as_deref()
                    .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok());
                let expiry = expiry_date.map(|value| value.format("%Y-%m-%d").to_string());
                let replacement_declared = line.contains("replacement=");
                let stale = expiry_date.is_some_and(|value| value < evaluation_date)
                    || (marker_kind == "DEPRECATED" && !replacement_declared);
                let marker_id = Sha256Hash::digest(
                    format!("{relative}:{marker_kind}:{}", index + 1).as_bytes(),
                )
                .to_string();
                let mut marker_limitations = Vec::new();
                if raw_expiry.is_some() && expiry_date.is_none() {
                    marker_limitations.push("DEBT_MARKER_EXPIRY_INVALID".to_owned());
                }
                if !structured {
                    marker_limitations.push("DEBT_MARKER_UNSTRUCTURED".to_owned());
                }
                markers.push(star_contracts::maintenance_v2::DebtMarkerObservation {
                    marker_id,
                    project_relative_path: relative.clone(),
                    marker_kind: marker_kind.to_owned(),
                    line: (index + 1) as u32,
                    structured,
                    owner_declared: line.contains("owner="),
                    issue_declared: line.contains("issue="),
                    replacement_declared,
                    expiry,
                    stale,
                    limitations: marker_limitations,
                });
            }
        }
    }
}

fn metadata_is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

fn repository_identity(
    project_root: &Path,
    git_common_dir: &str,
) -> Result<String, GitHistoryPortError> {
    if git_common_dir.is_empty() || git_common_dir.contains('\0') {
        return Err(GitHistoryPortError::Malformed);
    }
    let common_dir = PathBuf::from(git_common_dir);
    let common_dir = if common_dir.is_absolute() {
        common_dir
    } else {
        project_root.join(common_dir)
    };
    let canonical = fs::canonicalize(common_dir).map_err(|_| GitHistoryPortError::Unverified)?;
    let normalized = canonical.to_string_lossy().replace('\\', "/");
    #[cfg(windows)]
    let normalized = normalized.to_ascii_lowercase();
    Ok(Sha256Hash::digest(normalized.as_bytes()).to_string())
}

fn metadata_value(line: &str, key: &str) -> Option<String> {
    let value = line.split_once(&format!("{key}="))?.1;
    let value = value.split([')', ']', ';', ',', ' ']).next()?;
    (!value.is_empty()).then_some(value.to_owned())
}

fn git<'a>(
    project_root: &Path,
    args: impl IntoIterator<Item = &'a str>,
) -> Result<String, GitHistoryPortError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(project_root)
        .output()
        .map_err(|_| GitHistoryPortError::Unavailable)?;
    if !output.status.success() {
        return Err(GitHistoryPortError::Unverified);
    }
    String::from_utf8(output.stdout).map_err(|_| GitHistoryPortError::Malformed)
}

fn is_commit_id(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_revision(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'/' | b'-' | b'~' | b'^')
        })
}

fn parse_numstat(value: &str) -> Option<(u32, u32, &str)> {
    let mut fields = value.splitn(3, '\t');
    let added = fields.next()?.parse().ok()?;
    let deleted = fields.next()?.parse().ok()?;
    let path = fields.next()?;
    (!path.is_empty() && !path.starts_with('/') && !path.contains(".."))
        .then_some((added, deleted, path))
}

fn is_excluded(path: &str) -> bool {
    path.split('/').any(|part| {
        matches!(
            part,
            "target" | "dist" | "generated" | "vendor" | "node_modules"
        )
    })
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path, process::Command};

    use star_contracts::ids::ProjectId;
    use star_ports::{GitHistoryObservationRequest, GitHistoryPort, GitHistoryPortError};

    use super::{
        CommandGitHistoryAdapter, codeowners_matches, is_commit_id, metadata_value, parse_numstat,
        valid_revision,
    };

    struct FixtureRepo(std::path::PathBuf);

    impl FixtureRepo {
        fn new(name: &str) -> Self {
            let digest = star_contracts::Sha256Hash::digest(name.as_bytes());
            let suffix = digest.as_str().trim_start_matches("sha256:");
            let path = std::env::temp_dir().join(format!(
                "star-project-git-history-{name}-{}-{suffix}",
                std::process::id(),
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            git(&path, ["init"]);
            git(&path, ["config", "user.name", "Alice Example"]);
            git(&path, ["config", "user.email", "alice@example.test"]);
            Self(path)
        }

        fn write(&self, relative: &str, value: impl AsRef<[u8]>) {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, value).unwrap();
        }

        fn commit(&self, message: &str) {
            git(&self.0, ["add", "."]);
            git(&self.0, ["commit", "-m", message]);
        }
    }

    impl Drop for FixtureRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn git<'a>(root: &Path, args: impl IntoIterator<Item = &'a str>) {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn observe_with_limit(
        root: &Path,
        commit_limit: u32,
    ) -> star_contracts::maintenance_v2::GitHistoryRiskSnapshot {
        CommandGitHistoryAdapter
            .observe(
                root,
                &GitHistoryObservationRequest {
                    project_id: ProjectId::from_stable_bytes(b"git-history-fixture"),
                    range_start: None,
                    range_end: "HEAD".to_owned(),
                    commit_limit,
                    evaluation_time: "2026-07-28T00:00:00Z".to_owned(),
                },
            )
            .unwrap()
    }

    fn observe(root: &Path) -> star_contracts::maintenance_v2::GitHistoryRiskSnapshot {
        observe_with_limit(root, 100)
    }

    #[test]
    fn numstat_rejects_binary_and_unsafe_paths() {
        assert_eq!(
            parse_numstat("1\t2\tsrc/lib.rs"),
            Some((1, 2, "src/lib.rs"))
        );
        assert_eq!(parse_numstat("-\t-\tasset.bin"), None);
        assert_eq!(parse_numstat("1\t2\t../secret"), None);
        assert!(is_commit_id("a".repeat(40).as_str()));
        assert!(!is_commit_id("not-a-commit"));
        assert!(valid_revision("HEAD~2"));
        assert!(!valid_revision("--format=%H"));
    }

    #[test]
    fn codeowners_uses_last_matching_rule_without_retaining_identity() {
        assert!(codeowners_matches("*", "crates"));
        assert!(codeowners_matches("/crates/**", "crates"));
        assert!(codeowners_matches("docs/*", "docs"));
        assert!(!codeowners_matches("/apps/**", "crates"));
    }

    #[test]
    fn structured_marker_metadata_is_bounded() {
        let line = "// TODO(owner=team;issue=ABC-1;expires=2020-01-01;replacement=next)";
        assert_eq!(
            metadata_value(line, "expires"),
            Some("2020-01-01".to_owned())
        );
        assert_eq!(metadata_value("TODO(owner=team)", "issue"), None);
    }

    #[test]
    fn full_history_is_redacted_and_covers_codeowners_debt_and_exclusions() {
        let repo = FixtureRepo::new("full");
        repo.write("CODEOWNERS", "* @all\n/crates/** @one @two\n");
        repo.write(
            "crates/a.rs",
            "// TODO(owner=core;issue=SC-1;expires=2020-01-01)\nfn a() {}\n",
        );
        repo.write("generated/out.rs", "// TODO(owner=generated)\n");
        repo.write("asset.bin", [0_u8, 0xff, 4]);
        repo.commit("initial");
        repo.write(
            "crates/a.rs",
            "// DEPRECATED(expires=not-a-date)\nfn a() {}\n",
        );
        repo.commit("rewrite");
        git(&repo.0, ["mv", "crates/a.rs", "crates/b.rs"]);
        repo.commit("rename");
        let snapshot = observe(&repo.0);
        assert_eq!(
            snapshot.history_completeness,
            star_contracts::maintenance_v2::GitHistoryCompleteness::Complete
        );
        let crates = snapshot
            .components
            .iter()
            .find(|item| item.component == "crates")
            .unwrap();
        assert_eq!(crates.change_burst, 3);
        assert_eq!(crates.declared_owner_count, 2);
        assert!(
            snapshot
                .debt_markers
                .iter()
                .all(|marker| !marker.project_relative_path.starts_with("generated/"))
        );
        assert!(
            snapshot
                .debt_markers
                .iter()
                .all(|marker| !marker.project_relative_path.starts_with(".git"))
        );
        assert!(
            snapshot
                .debt_markers
                .iter()
                .any(|marker| marker.marker_kind == "DEPRECATED" && marker.stale)
        );
        let deprecated = snapshot
            .debt_markers
            .iter()
            .find(|marker| marker.marker_kind == "DEPRECATED")
            .unwrap();
        assert!(deprecated.expiry.is_none());
        assert!(
            deprecated
                .limitations
                .iter()
                .any(|value| value == "DEBT_MARKER_EXPIRY_INVALID")
        );
        let encoded = serde_json::to_string(&snapshot).unwrap();
        assert!(!encoded.contains("alice@example.test"));
        assert!(!encoded.contains("Alice Example"));
    }

    #[test]
    fn shallow_and_rewritten_history_are_never_promoted_to_complete() {
        let repo = FixtureRepo::new("shallow");
        repo.write("crates/a.rs", "fn a() {}\n");
        repo.commit("first");
        repo.write("crates/a.rs", "fn a() { let _ = 1; }\n");
        repo.commit("second");
        let clone = repo.0.with_file_name(format!(
            "{}-clone",
            repo.0.file_name().unwrap().to_string_lossy()
        ));
        let source_url = format!("file:///{}", repo.0.to_string_lossy().replace('\\', "/"));
        git(
            repo.0.parent().unwrap(),
            [
                "clone",
                "--depth",
                "1",
                &source_url,
                clone.file_name().unwrap().to_str().unwrap(),
            ],
        );
        let snapshot = observe(&clone);
        assert_eq!(
            snapshot.history_completeness,
            star_contracts::maintenance_v2::GitHistoryCompleteness::Unverified
        );
        assert!(
            snapshot
                .limitations
                .iter()
                .any(|value| value == "GIT_HISTORY_SHALLOW")
        );
        let error = CommandGitHistoryAdapter
            .observe(
                &repo.0,
                &GitHistoryObservationRequest {
                    project_id: ProjectId::from_stable_bytes(b"git-history-fixture"),
                    range_start: Some("0000000000000000000000000000000000000000".to_owned()),
                    range_end: "HEAD".to_owned(),
                    commit_limit: 100,
                    evaluation_time: "2026-07-28T00:00:00Z".to_owned(),
                },
            )
            .unwrap_err();
        assert_eq!(error, GitHistoryPortError::Unverified);
        let _ = fs::remove_dir_all(clone);
    }

    #[test]
    fn repository_identity_and_commit_limit_are_fail_closed() {
        let first = FixtureRepo::new("identity-first");
        first.write("src/lib.rs", "fn first() {}\n");
        first.commit("first");
        first.write("src/lib.rs", "fn first() { let _ = 1; }\n");
        first.commit("second");

        let second = FixtureRepo::new("identity-second");
        second.write("src/lib.rs", "fn second() {}\n");
        second.commit("first");

        assert_ne!(
            observe(&first.0).repository_identity,
            observe(&second.0).repository_identity
        );
        let limited = observe_with_limit(&first.0, 1);
        assert_eq!(
            limited.history_completeness,
            star_contracts::maintenance_v2::GitHistoryCompleteness::Partial
        );
        assert!(
            limited
                .limitations
                .iter()
                .any(|value| value == "GIT_HISTORY_COMMIT_LIMIT_REACHED")
        );

        for request in [
            GitHistoryObservationRequest {
                project_id: ProjectId::from_stable_bytes(b"git-history-fixture"),
                range_start: None,
                range_end: "HEAD".to_owned(),
                commit_limit: 10_001,
                evaluation_time: "2026-07-28T00:00:00Z".to_owned(),
            },
            GitHistoryObservationRequest {
                project_id: ProjectId::from_stable_bytes(b"git-history-fixture"),
                range_start: None,
                range_end: "HEAD".to_owned(),
                commit_limit: 100,
                evaluation_time: "not-a-time".to_owned(),
            },
        ] {
            assert_eq!(
                CommandGitHistoryAdapter
                    .observe(&first.0, &request)
                    .unwrap_err(),
                GitHistoryPortError::Invalid
            );
        }
    }

    #[test]
    fn debt_marker_scan_depth_is_bounded() {
        let repo = FixtureRepo::new("debt-depth");
        repo.write("src/lib.rs", "fn fixture() {}\n");
        repo.commit("initial");
        let deep = (0..=super::MAX_DEBT_SCAN_DEPTH)
            .map(|_| "d")
            .collect::<Vec<_>>()
            .join("/");
        fs::create_dir_all(repo.0.join(deep)).unwrap();
        let snapshot = observe(&repo.0);
        assert!(
            snapshot
                .limitations
                .iter()
                .any(|value| value == "DEBT_MARKER_DEPTH_LIMIT")
        );
    }
}
