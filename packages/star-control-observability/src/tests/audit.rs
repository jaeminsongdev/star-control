use super::helpers::{create_job, open_store, schema_root, temp_project};
use crate::{AuditEventWriter, AUDIT_LOG_PATH, SCHEMA_VERSION};
use serde_json::json;
use std::fs;

#[test]
fn appends_schema_valid_audit_events_inside_job_dir() {
    let project = temp_project("append");
    let store = open_store(&project);
    create_job(&store);
    let writer = AuditEventWriter::new(schema_root());
    let event = json!({
        "schema_version": SCHEMA_VERSION,
        "event_id": "audit-0001",
        "job_id": "J-0001",
        "type": "approval_recorded",
        "created_at": "unix:1",
        "actor": "api-control-service",
        "summary": "Approval response was recorded.",
        "artifact_paths": ["approvals/approval-response.json"],
        "risk_level": "LOW"
    });

    let artifact_ref = writer.append(&store, &event).expect("append audit event");
    assert_eq!(artifact_ref["path"], AUDIT_LOG_PATH);
    assert_eq!(artifact_ref["kind"], "log");

    let audit_path = project.join(".ai-runs/J-0001/audit/audit-events.jsonl");
    assert!(audit_path.is_file());
    let events = writer.read(&store, "J-0001").expect("read audit events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["type"], "approval_recorded");

    fs::remove_dir_all(project).ok();
}

#[test]
fn audit_writer_redacts_secret_like_summary_before_persisting() {
    let project = temp_project("redact");
    let store = open_store(&project);
    create_job(&store);
    let writer = AuditEventWriter::new(schema_root());
    let api_key = format!("{}{}", "sk-test", "-secret");
    let event = writer.event(
        "J-0001",
        "audit-0001",
        "provider_executed",
        "test",
        format!("Authorization: Bearer {}", api_key),
    );

    writer
        .append(&store, &event)
        .expect("append redacted event");
    let text = fs::read_to_string(project.join(".ai-runs/J-0001/audit/audit-events.jsonl"))
        .expect("read audit log");
    assert!(!text.contains(&api_key));
    assert!(!text.contains("Bearer"));
    assert!(text.contains("[REDACTED]"));

    fs::remove_dir_all(project).ok();
}

#[test]
fn audit_writer_rejects_path_traversal_job_path() {
    let project = temp_project("traversal");
    let store = open_store(&project);
    create_job(&store);
    let writer = AuditEventWriter::new(schema_root());
    let event = json!({
        "schema_version": SCHEMA_VERSION,
        "event_id": "audit-0001",
        "job_id": "../J-0001",
        "type": "job_failed",
        "created_at": "unix:1",
        "actor": "test",
        "summary": "invalid job id"
    });

    let result = writer.append(&store, &event);
    assert!(result.is_err());
    assert!(!project.join(".ai-runs/audit/audit-events.jsonl").exists());

    fs::remove_dir_all(project).ok();
}
