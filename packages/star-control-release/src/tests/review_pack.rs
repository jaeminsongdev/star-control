use crate::test_support::{create_job, open_store, schema_root, temp_project};
use crate::{
    ReleaseReadinessError, ReleaseReadinessWriter, ReleaseReviewPackWriter,
    RELEASE_REVIEW_PACK_MARKDOWN_FILE, RELEASE_REVIEW_PACK_PATH,
};
use serde_json::json;
use std::fs;

#[test]
fn release_review_pack_writer_writes_markdown_without_release_action() {
    let project = temp_project("review-pack");
    let store = open_store(&project);
    create_job(&store);
    let readiness_writer = ReleaseReadinessWriter::new(schema_root());
    let readiness = readiness_writer.not_ready(
        "release-0007",
        "star-control",
        "1.2.3",
        vec![
            readiness_writer.check(
                "required-ci-passed",
                "pass",
                vec![".github/workflows/ci.yml".to_string()],
            ),
            readiness_writer.check("version-consistent", "fail", vec!["Cargo.toml".to_string()]),
        ],
        vec!["version mismatch: expected 1.2.3, found 1.2.2".to_string()],
    );
    let review_pack_writer = ReleaseReviewPackWriter::new(schema_root());

    let artifact_ref = review_pack_writer
        .write(&store, "J-0001", &readiness)
        .expect("write release review pack");

    assert_eq!(artifact_ref["path"], RELEASE_REVIEW_PACK_PATH);
    assert_eq!(artifact_ref["kind"], "review_pack");
    assert_eq!(artifact_ref["producer"], "star-control-release");
    let path = project
        .join(".ai-runs")
        .join("J-0001")
        .join("review-packs")
        .join(RELEASE_REVIEW_PACK_MARKDOWN_FILE);
    let markdown = fs::read_to_string(&path).expect("read release review pack");
    assert!(markdown.contains("# Release Review Pack"));
    assert!(markdown.contains("release-0007"));
    assert!(markdown.contains("version-consistent"));
    assert!(markdown.contains("version mismatch: expected 1.2.3, found 1.2.2"));
    assert!(markdown.contains("release automation"));
    assert!(!project
        .join(".ai-runs")
        .join("J-0001")
        .join("release")
        .join("release-action.json")
        .exists());

    fs::remove_dir_all(project).ok();
}

#[test]
fn release_review_pack_rejects_ready_status_and_overwrite() {
    let project = temp_project("review-pack-overwrite");
    let store = open_store(&project);
    create_job(&store);
    let readiness_writer = ReleaseReadinessWriter::new(schema_root());
    let review_pack_writer = ReleaseReviewPackWriter::new(schema_root());
    let mut ready = readiness_writer.readiness(
        "release-0008",
        "star-control",
        "1.2.3",
        "ready",
        vec![readiness_writer.check("required-ci-passed", "pass", Vec::new())],
        Vec::new(),
    );
    ready["approvals"] = json!(["release approval recorded"]);

    let ready_error = review_pack_writer
        .build_markdown(&ready)
        .expect_err("ready status remains reserved");
    assert!(matches!(
        ready_error,
        ReleaseReadinessError::InvalidReleaseReadiness { .. }
    ));

    let reserved = readiness_writer.reserved("release-0009", "star-control", "0.0.0-dev");
    review_pack_writer
        .write(&store, "J-0001", &reserved)
        .expect("first review pack write");
    let overwrite_error = review_pack_writer
        .write(&store, "J-0001", &reserved)
        .expect_err("second review pack write must not overwrite");
    assert!(matches!(
        overwrite_error,
        ReleaseReadinessError::WriteFailed { .. }
    ));

    fs::remove_dir_all(project).ok();
}
