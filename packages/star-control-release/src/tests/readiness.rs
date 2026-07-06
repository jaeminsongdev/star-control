use crate::test_support::{create_job, open_store, schema_root, temp_project};
use crate::{ReleaseReadinessError, ReleaseReadinessWriter, RELEASE_READINESS_PATH};
use serde_json::json;
use std::fs;

#[test]
fn writes_reserved_release_readiness_inside_job_dir() {
    let project = temp_project("reserved");
    let store = open_store(&project);
    create_job(&store);
    let writer = ReleaseReadinessWriter::new(schema_root());
    let readiness = writer.reserved("release-0001", "star-control", "0.0.0-dev");

    let artifact_ref = writer
        .write(&store, "J-0001", &readiness)
        .expect("write release readiness");

    assert_eq!(artifact_ref["path"], RELEASE_READINESS_PATH);
    assert_eq!(artifact_ref["kind"], "other");
    assert_eq!(artifact_ref["producer"], "star-control-release");
    assert_eq!(
        artifact_ref["schema_path"],
        "specs/schemas/release-readiness.schema.json"
    );
    let path = project.join(".ai-runs/J-0001/release/release-readiness.json");
    assert!(path.is_file());
    let read = writer
        .read(&store, "J-0001")
        .expect("read release readiness")
        .expect("release readiness exists");
    assert_eq!(read["status"], "reserved");
    assert!(read["blockers"]
        .as_array()
        .expect("blockers")
        .contains(&json!("release automation is not implemented yet")));

    fs::remove_dir_all(project).ok();
}

#[test]
fn rejects_ready_status_until_release_approval_flow_exists() {
    let writer = ReleaseReadinessWriter::new(schema_root());
    let mut readiness = writer.readiness(
        "release-0002",
        "star-control",
        "0.1.0",
        "ready",
        vec![writer.check("required-ci-passed", "pass", Vec::new())],
        Vec::new(),
    );
    readiness["approvals"] = json!(["release approval recorded"]);

    let error = writer
        .validate_readiness(&readiness)
        .expect_err("ready status is reserved");
    assert!(matches!(
        error,
        ReleaseReadinessError::InvalidReleaseReadiness { .. }
    ));
}

#[test]
fn rejects_reserved_status_without_blocker_explanation() {
    let writer = ReleaseReadinessWriter::new(schema_root());
    let readiness = writer.readiness(
        "release-0003",
        "star-control",
        "0.0.0-dev",
        "reserved",
        vec![writer.check("required-ci-passed", "reserved", Vec::new())],
        Vec::new(),
    );

    let error = writer
        .validate_readiness(&readiness)
        .expect_err("reserved status needs blocker explanation");
    assert!(matches!(
        error,
        ReleaseReadinessError::InvalidReleaseReadiness { .. }
    ));
}

#[test]
fn refuses_to_overwrite_existing_release_readiness() {
    let project = temp_project("overwrite");
    let store = open_store(&project);
    create_job(&store);
    let writer = ReleaseReadinessWriter::new(schema_root());
    let readiness = writer.reserved("release-0001", "star-control", "0.0.0-dev");

    writer
        .write(&store, "J-0001", &readiness)
        .expect("first write");
    let error = writer
        .write(&store, "J-0001", &readiness)
        .expect_err("second write must not overwrite");

    assert!(matches!(error, ReleaseReadinessError::WriteFailed { .. }));
    fs::remove_dir_all(project).ok();
}

#[test]
fn rejects_path_traversal_job_id_without_writing() {
    let project = temp_project("traversal");
    let store = open_store(&project);
    create_job(&store);
    let writer = ReleaseReadinessWriter::new(schema_root());
    let readiness = writer.reserved("release-0001", "star-control", "0.0.0-dev");

    let error = writer
        .write(&store, "../J-0001", &readiness)
        .expect_err("unsafe job id");
    assert!(matches!(error, ReleaseReadinessError::State { .. }));
    assert!(!project
        .join(".ai-runs/release/release-readiness.json")
        .exists());
    fs::remove_dir_all(project).ok();
}
