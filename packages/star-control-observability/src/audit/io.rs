use super::AuditEventWriter;
use crate::constants::AUDIT_LOG_PATH;
use crate::error::ObservabilityError;
use serde_json::Value;
use star_control_security::redact_value;
use star_control_state::{ArtifactKind, StateStore};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

impl AuditEventWriter {
    pub fn append(&self, store: &StateStore, event: &Value) -> Result<Value, ObservabilityError> {
        let event = redact_value(event.clone());
        let job_id = event.get("job_id").and_then(Value::as_str).ok_or_else(|| {
            ObservabilityError::InvalidAuditEvent {
                message: "job_id is required".to_string(),
            }
        })?;
        self.validate_event(&event)?;
        let path = store.resolve_job_path(job_id, AUDIT_LOG_PATH)?;
        append_jsonl(&path, &event)?;
        store
            .artifact_ref(
                job_id,
                AUDIT_LOG_PATH,
                ArtifactKind::Log,
                "star-control-observability",
                Some("specs/schemas/audit-event.schema.json"),
                Some("audit event log"),
            )
            .map_err(ObservabilityError::from)
    }

    pub fn read(&self, store: &StateStore, job_id: &str) -> Result<Vec<Value>, ObservabilityError> {
        let path = store.resolve_job_path(job_id, AUDIT_LOG_PATH)?;
        if !path.is_file() {
            return Ok(Vec::new());
        }
        let file = File::open(&path).map_err(|source| ObservabilityError::AppendFailed {
            path: path.clone(),
            source,
        })?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();
        for (index, line) in reader.lines().enumerate() {
            let line_number = index + 1;
            let line = line.map_err(|source| ObservabilityError::CorruptAuditLog {
                path: path.clone(),
                line: line_number,
                message: source.to_string(),
            })?;
            if line.trim().is_empty() {
                continue;
            }
            let event: Value =
                serde_json::from_str(&line).map_err(|source| ObservabilityError::InvalidJson {
                    path: path.clone(),
                    source,
                })?;
            self.validate_event(&event)?;
            events.push(event);
        }
        Ok(events)
    }
}

fn append_jsonl(path: &Path, value: &Value) -> Result<(), ObservabilityError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ObservabilityError::AppendFailed {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| ObservabilityError::AppendFailed {
            path: path.to_path_buf(),
            source,
        })?;
    serde_json::to_writer(&mut file, value).map_err(|source| ObservabilityError::InvalidJson {
        path: path.to_path_buf(),
        source,
    })?;
    file.write_all(b"\n")
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
        .map_err(|source| ObservabilityError::AppendFailed {
            path: path.to_path_buf(),
            source,
        })
}
