use crate::test_support::{schema_root, temp_project};
use crate::{
    ReleaseConsistencyChecker, ReleaseEvidenceFileChecker, ReleaseReadinessError,
    ReleaseReadinessWriter,
};
use std::fs;

#[test]
fn release_consistency_checker_passes_matching_version_and_changelog() {
    let result = ReleaseConsistencyChecker::check(
        "1.2.3",
        "1.2.3\n",
        "# Changelog\n\n## 1.2.3\n- release notes\n",
        "Cargo.toml",
        "CHANGELOG.md",
    );

    assert!(result.is_consistent());
    assert!(result.blockers().is_empty());
    assert_eq!(result.checks()[0]["name"], "version-consistent");
    assert_eq!(result.checks()[0]["status"], "pass");
    assert_eq!(result.checks()[0]["evidence_paths"][0], "Cargo.toml");
    assert_eq!(result.checks()[1]["name"], "changelog-updated");
    assert_eq!(result.checks()[1]["status"], "pass");
    assert_eq!(result.checks()[1]["evidence_paths"][0], "CHANGELOG.md");
}

#[test]
fn release_consistency_checker_blocks_version_and_changelog_mismatch() {
    let result = ReleaseConsistencyChecker::check(
        "1.2.3",
        "1.2.2",
        "# Changelog\n\n## 1.2.2\n- previous release\n",
        "Cargo.toml",
        "CHANGELOG.md",
    );

    assert!(!result.is_consistent());
    assert_eq!(result.checks()[0]["status"], "fail");
    assert_eq!(result.checks()[1]["status"], "fail");
    assert!(result
        .blockers()
        .contains(&"version mismatch: expected 1.2.3, found 1.2.2".to_string()));
    assert!(result
        .blockers()
        .contains(&"changelog does not mention version 1.2.3".to_string()));
}

#[test]
fn release_consistency_result_feeds_schema_valid_not_ready_readiness() {
    let writer = ReleaseReadinessWriter::new(schema_root());
    let result =
        ReleaseConsistencyChecker::check("1.2.3", "", "no version yet", "", "CHANGELOG.md");
    let (checks, blockers) = result.into_parts();
    let readiness = writer.not_ready("release-0004", "star-control", "1.2.3", checks, blockers);

    writer
        .validate_readiness(&readiness)
        .expect("schema-valid not_ready release readiness");
    assert_eq!(readiness["status"], "not_ready");
    assert_eq!(readiness["checks"][0]["name"], "version-consistent");
    assert_eq!(readiness["checks"][1]["name"], "changelog-updated");
    assert!(!readiness["blockers"]
        .as_array()
        .expect("blockers")
        .is_empty());
}

#[test]
fn release_evidence_file_checker_reads_version_and_changelog_inside_project() {
    let project = temp_project("evidence-pass");
    fs::write(
        project.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"1.2.3\"\n",
    )
    .expect("write Cargo.toml");
    fs::write(
        project.join("CHANGELOG.md"),
        "# Changelog\n\n## 1.2.3\n- release notes\n",
    )
    .expect("write changelog");

    let result = ReleaseEvidenceFileChecker::check(&project, "1.2.3", "Cargo.toml", "CHANGELOG.md")
        .expect("file evidence result");

    assert!(result.is_consistent());
    assert_eq!(result.checks()[0]["status"], "pass");
    assert_eq!(result.checks()[0]["evidence_paths"][0], "Cargo.toml");
    assert_eq!(result.checks()[1]["status"], "pass");
    assert_eq!(result.checks()[1]["evidence_paths"][0], "CHANGELOG.md");
    fs::remove_dir_all(project).ok();
}

#[test]
fn release_evidence_file_checker_blocks_mismatch_from_files() {
    let project = temp_project("evidence-mismatch");
    fs::write(project.join("VERSION"), "1.2.2\n").expect("write VERSION");
    fs::write(project.join("CHANGELOG.md"), "## 1.2.2\n").expect("write changelog");

    let result = ReleaseEvidenceFileChecker::check(&project, "1.2.3", "VERSION", "CHANGELOG.md")
        .expect("file evidence result");

    assert!(!result.is_consistent());
    assert_eq!(result.checks()[0]["status"], "fail");
    assert_eq!(result.checks()[1]["status"], "fail");
    assert!(result
        .blockers()
        .contains(&"version mismatch: expected 1.2.3, found 1.2.2".to_string()));
    assert!(result
        .blockers()
        .contains(&"changelog does not mention version 1.2.3".to_string()));
    fs::remove_dir_all(project).ok();
}

#[test]
fn release_evidence_file_checker_rejects_unsafe_paths_and_missing_version() {
    let project = temp_project("evidence-invalid");
    fs::write(project.join("Cargo.toml"), "[package]\nname = \"demo\"\n")
        .expect("write Cargo.toml");
    fs::write(project.join("VERSION"), "1.2.3\n").expect("write VERSION");
    fs::write(project.join("CHANGELOG.md"), "## 1.2.3\n").expect("write changelog");

    for unsafe_path in [
        "../Cargo.toml",
        "/Cargo.toml",
        "C:/Cargo.toml",
        "nested/../Cargo.toml",
    ] {
        let unsafe_error =
            ReleaseEvidenceFileChecker::check(&project, "1.2.3", unsafe_path, "CHANGELOG.md")
                .expect_err("unsafe version evidence path");
        assert!(matches!(
            unsafe_error,
            ReleaseReadinessError::InvalidReleaseEvidence { .. }
        ));
    }

    let unsafe_changelog_error =
        ReleaseEvidenceFileChecker::check(&project, "1.2.3", "VERSION", "../CHANGELOG.md")
            .expect_err("unsafe changelog evidence path");
    assert!(matches!(
        unsafe_changelog_error,
        ReleaseReadinessError::InvalidReleaseEvidence { .. }
    ));

    let missing_version_error =
        ReleaseEvidenceFileChecker::check(&project, "1.2.3", "Cargo.toml", "CHANGELOG.md")
            .expect_err("missing version declaration");
    assert!(matches!(
        missing_version_error,
        ReleaseReadinessError::InvalidReleaseEvidence { .. }
    ));
    fs::remove_dir_all(project).ok();
}
