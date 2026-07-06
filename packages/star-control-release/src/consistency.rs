use crate::error::ReleaseReadinessError;
use crate::support::{
    check_status, declared_version_from_text, display_or_empty, evidence_paths,
    normalized_evidence_path, read_release_text, release_check, resolve_project_file,
};
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseConsistencyResult {
    checks: Vec<Value>,
    blockers: Vec<String>,
}

impl ReleaseConsistencyResult {
    pub fn checks(&self) -> &[Value] {
        &self.checks
    }

    pub fn blockers(&self) -> &[String] {
        &self.blockers
    }

    pub fn is_consistent(&self) -> bool {
        self.blockers.is_empty()
    }

    pub fn into_parts(self) -> (Vec<Value>, Vec<String>) {
        (self.checks, self.blockers)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReleaseConsistencyChecker;

impl ReleaseConsistencyChecker {
    pub fn check(
        expected_version: impl Into<String>,
        declared_version: impl Into<String>,
        changelog_text: impl Into<String>,
        version_evidence_path: impl Into<String>,
        changelog_evidence_path: impl Into<String>,
    ) -> ReleaseConsistencyResult {
        let expected_version = expected_version.into();
        let declared_version = declared_version.into();
        let changelog_text = changelog_text.into();
        let version_evidence_path = version_evidence_path.into();
        let changelog_evidence_path = changelog_evidence_path.into();
        let expected = expected_version.trim();
        let declared = declared_version.trim();

        let mut blockers = Vec::new();
        let version_matches = !expected.is_empty() && declared == expected;
        let changelog_mentions_version =
            !expected.is_empty() && changelog_text.lines().any(|line| line.contains(expected));

        if expected.is_empty() {
            blockers.push("expected release version is empty".to_string());
        } else {
            if !version_matches {
                blockers.push(format!(
                    "version mismatch: expected {}, found {}",
                    expected,
                    display_or_empty(declared)
                ));
            }
            if !changelog_mentions_version {
                blockers.push(format!("changelog does not mention version {}", expected));
            }
        }

        ReleaseConsistencyResult {
            checks: vec![
                release_check(
                    "version-consistent",
                    check_status(version_matches),
                    evidence_paths(version_evidence_path),
                ),
                release_check(
                    "changelog-updated",
                    check_status(changelog_mentions_version),
                    evidence_paths(changelog_evidence_path),
                ),
            ],
            blockers,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReleaseEvidenceFileChecker;

impl ReleaseEvidenceFileChecker {
    pub fn check(
        project_root: impl AsRef<Path>,
        expected_version: impl Into<String>,
        version_file: impl AsRef<str>,
        changelog_file: impl AsRef<str>,
    ) -> Result<ReleaseConsistencyResult, ReleaseReadinessError> {
        let project_root = project_root.as_ref();
        let version_file = version_file.as_ref();
        let changelog_file = changelog_file.as_ref();
        let version_path = resolve_project_file(project_root, version_file)?;
        let changelog_path = resolve_project_file(project_root, changelog_file)?;
        let version_text = read_release_text(&version_path)?;
        let changelog_text = read_release_text(&changelog_path)?;
        let declared_version = declared_version_from_text(&version_text).ok_or_else(|| {
            ReleaseReadinessError::InvalidReleaseEvidence {
                message: format!(
                    "declared version not found in release evidence {}",
                    version_file
                ),
            }
        })?;

        Ok(ReleaseConsistencyChecker::check(
            expected_version,
            declared_version,
            changelog_text,
            normalized_evidence_path(version_file)?,
            normalized_evidence_path(changelog_file)?,
        ))
    }
}
