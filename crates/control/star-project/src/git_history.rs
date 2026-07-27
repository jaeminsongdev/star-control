//! Read-only Git history observation adapter for code-health maintenance.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::Command,
};

use star_contracts::{
    Sha256Hash,
    maintenance_v2::{
        GIT_HISTORY_RISK_SNAPSHOT_SCHEMA_ID, GitHistoryCompleteness, GitHistoryComponentRisk,
        GitHistoryRiskSnapshot,
    },
};
use star_domain::versioned_fingerprint;
use star_ports::{GitHistoryObservationRequest, GitHistoryPort, GitHistoryPortError};

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
            || request.commit_limit == 0
        {
            return Err(GitHistoryPortError::Invalid);
        }
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
        let history_completeness = match shallow.trim() {
            "false" => GitHistoryCompleteness::Complete,
            "true" => {
                limitations.push("GIT_HISTORY_SHALLOW".to_owned());
                GitHistoryCompleteness::Unverified
            }
            _ => return Err(GitHistoryPortError::Malformed),
        };
        if components.is_empty() {
            limitations.push("GIT_HISTORY_EMPTY_OR_BINARY".to_owned());
        }
        let components = components
            .into_iter()
            .map(
                |(component, (changed_file_count, relative_churn, commits))| {
                    let owners = codeowners_for(project_root, &component);
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
        let (codeowners_fingerprint, mut codeowners_limitations) =
            codeowners_fingerprint(project_root);
        limitations.append(&mut codeowners_limitations);
        let (debt_markers, mut debt_limitations) = debt_markers(project_root);
        limitations.append(&mut debt_limitations);
        limitations.sort();
        limitations.dedup();
        let repository_identity = Sha256Hash::digest(repository.trim().as_bytes()).to_string();
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

fn codeowners_for(project_root: &Path, component: &str) -> Option<Vec<String>> {
    let source = read_codeowners(project_root)?;
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

fn codeowners_fingerprint(project_root: &Path) -> (Option<Sha256Hash>, Vec<String>) {
    match read_codeowners(project_root) {
        Some(value) => (Some(Sha256Hash::digest(value.as_bytes())), Vec::new()),
        None => (None, vec!["CODEOWNERS_MISSING".to_owned()]),
    }
}

fn read_codeowners(project_root: &Path) -> Option<String> {
    ["CODEOWNERS", ".github/CODEOWNERS"]
        .into_iter()
        .find_map(|relative| fs::read_to_string(project_root.join(relative)).ok())
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
) -> (
    Vec<star_contracts::maintenance_v2::DebtMarkerObservation>,
    Vec<String>,
) {
    let mut markers = Vec::new();
    let mut limitations = Vec::new();
    visit_debt_markers(project_root, project_root, &mut markers, &mut limitations);
    markers.sort_by(|left, right| left.marker_id.cmp(&right.marker_id));
    (markers, limitations)
}

fn visit_debt_markers(
    root: &Path,
    current: &Path,
    markers: &mut Vec<star_contracts::maintenance_v2::DebtMarkerObservation>,
    limitations: &mut Vec<String>,
) {
    let Ok(entries) = fs::read_dir(current) else {
        limitations.push("DEBT_MARKER_READ_FAILED".to_owned());
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let relative = path.strip_prefix(root).ok().and_then(|path| path.to_str());
        let Some(relative) = relative.map(|path| path.replace('\\', "/")) else {
            limitations.push("DEBT_MARKER_NON_UTF8_PATH".to_owned());
            continue;
        };
        if path.is_dir() {
            if !is_excluded(&relative) && !relative.starts_with(".git/") {
                visit_debt_markers(root, &path, markers, limitations);
            }
            continue;
        }
        if is_excluded(&relative)
            || entry
                .metadata()
                .map(|metadata| metadata.len() > 1_000_000)
                .unwrap_or(true)
        {
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
                let structured =
                    line.contains("owner=") || line.contains("issue=") || line.contains("expires=");
                let expiry = metadata_value(line, "expires");
                let replacement_declared = line.contains("replacement=");
                let stale = expiry.as_deref().is_some_and(|value| value < "2026-07-27")
                    || (marker_kind == "DEPRECATED" && !replacement_declared);
                let marker_id = Sha256Hash::digest(
                    format!("{relative}:{marker_kind}:{}", index + 1).as_bytes(),
                )
                .to_string();
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
                    limitations: if structured {
                        Vec::new()
                    } else {
                        vec!["DEBT_MARKER_UNSTRUCTURED".to_owned()]
                    },
                });
            }
        }
    }
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

    fn observe(root: &Path) -> star_contracts::maintenance_v2::GitHistoryRiskSnapshot {
        CommandGitHistoryAdapter
            .observe(
                root,
                &GitHistoryObservationRequest {
                    project_id: ProjectId::from_stable_bytes(b"git-history-fixture"),
                    range_start: None,
                    range_end: "HEAD".to_owned(),
                    commit_limit: 100,
                },
            )
            .unwrap()
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
        repo.write("crates/a.rs", "// DEPRECATED\nfn a() {}\n");
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
                .any(|marker| marker.marker_kind == "DEPRECATED" && marker.stale)
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
                },
            )
            .unwrap_err();
        assert_eq!(error, GitHistoryPortError::Unverified);
        let _ = fs::remove_dir_all(clone);
    }
}
