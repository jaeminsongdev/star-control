use serde_json::{json, Value};
use star_control_schema::{load_schema, validate_json, ValidationError};
use star_control_security::redact_value;
use star_control_state::{ArtifactKind, StateStore, StateStoreError};
use std::error::Error;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: &str = "1.0.0";
const AUDIT_EVENT_SCHEMA: &str = "audit-event.schema.json";
pub const AUDIT_LOG_PATH: &str = "audit/audit-events.jsonl";

#[derive(Debug)]
pub enum ObservabilityError {
    State {
        source: StateStoreError,
    },
    SchemaLoadFailed {
        path: PathBuf,
        message: String,
    },
    SchemaValidationFailed {
        path: PathBuf,
        errors: Vec<ValidationError>,
    },
    InvalidAuditEvent {
        message: String,
    },
    AppendFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidJson {
        path: PathBuf,
        source: serde_json::Error,
    },
    CorruptAuditLog {
        path: PathBuf,
        line: usize,
        message: String,
    },
}

impl fmt::Display for ObservabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::State { source } => write!(formatter, "state store error: {}", source),
            Self::SchemaLoadFailed { path, message } => {
                write!(
                    formatter,
                    "audit schema load failed at {}: {}",
                    path.display(),
                    message
                )
            }
            Self::SchemaValidationFailed { path, errors } => {
                write!(
                    formatter,
                    "audit event schema validation failed for {} with {} error(s)",
                    path.display(),
                    errors.len()
                )
            }
            Self::InvalidAuditEvent { message } => {
                write!(formatter, "invalid audit event: {}", message)
            }
            Self::AppendFailed { path, source } => {
                write!(
                    formatter,
                    "failed to append audit log {}: {}",
                    path.display(),
                    source
                )
            }
            Self::InvalidJson { path, source } => {
                write!(
                    formatter,
                    "invalid audit JSON at {}: {}",
                    path.display(),
                    source
                )
            }
            Self::CorruptAuditLog {
                path,
                line,
                message,
            } => write!(
                formatter,
                "corrupt audit log {} at line {}: {}",
                path.display(),
                line,
                message
            ),
        }
    }
}

impl Error for ObservabilityError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::State { source } => Some(source),
            Self::AppendFailed { source, .. } => Some(source),
            Self::InvalidJson { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<StateStoreError> for ObservabilityError {
    fn from(source: StateStoreError) -> Self {
        Self::State { source }
    }
}

#[derive(Debug, Clone)]
pub struct AuditEventWriter {
    schema_root: PathBuf,
}

impl AuditEventWriter {
    pub fn new(schema_root: impl Into<PathBuf>) -> Self {
        Self {
            schema_root: schema_root.into(),
        }
    }

    pub fn event(
        &self,
        job_id: &str,
        event_id: impl Into<String>,
        event_type: impl Into<String>,
        actor: impl Into<String>,
        summary: impl Into<String>,
    ) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "event_id": event_id.into(),
            "job_id": job_id,
            "type": event_type.into(),
            "created_at": timestamp_string(),
            "actor": actor.into(),
            "summary": summary.into()
        })
    }

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

    pub fn validate_event(&self, event: &Value) -> Result<(), ObservabilityError> {
        let schema_path = self.schema_root.join(AUDIT_EVENT_SCHEMA);
        let schema =
            load_schema(&schema_path).map_err(|source| ObservabilityError::SchemaLoadFailed {
                path: schema_path.clone(),
                message: source.to_string(),
            })?;
        let result = validate_json(event, &schema);
        if result.is_ok() {
            Ok(())
        } else {
            Err(ObservabilityError::SchemaValidationFailed {
                path: PathBuf::from(AUDIT_EVENT_SCHEMA),
                errors: result.errors,
            })
        }
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

fn timestamp_string() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("unix:{}", seconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_control_state::StateStore;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn schema_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../specs/schemas")
    }

    fn temp_project(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "star-control-observability-{}-{}-{}",
            label,
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(&path).expect("create temp project");
        path
    }

    fn open_store(project: &Path) -> StateStore {
        StateStore::open(project, schema_root()).expect("open state store")
    }

    fn create_job(store: &StateStore) {
        store
            .create_job("Audit event writer", ".", Vec::new())
            .expect("create job");
    }

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
}
