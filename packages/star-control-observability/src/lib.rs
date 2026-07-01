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
const COST_METRIC_SCHEMA: &str = "cost-metric.schema.json";
pub const AUDIT_LOG_PATH: &str = "audit/audit-events.jsonl";
pub const COST_METRIC_FILE: &str = "cost-metric.json";

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
    InvalidCostMetric {
        message: String,
    },
    AppendFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    ReadFailed {
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
            Self::InvalidCostMetric { message } => {
                write!(formatter, "invalid cost metric: {}", message)
            }
            Self::AppendFailed { path, source } => {
                write!(
                    formatter,
                    "failed to append audit log {}: {}",
                    path.display(),
                    source
                )
            }
            Self::ReadFailed { path, source } => {
                write!(
                    formatter,
                    "failed to read observability artifact {}: {}",
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
            Self::ReadFailed { source, .. } => Some(source),
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

#[derive(Debug, Clone, Default)]
pub struct CostBudgetThresholds {
    max_estimated_cost: Option<f64>,
    max_wall_time_ms: Option<u64>,
    max_total_tokens: Option<u64>,
}

impl CostBudgetThresholds {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_estimated_cost(mut self, value: f64) -> Self {
        self.max_estimated_cost = Some(value);
        self
    }

    pub fn with_max_wall_time_ms(mut self, value: u64) -> Self {
        self.max_wall_time_ms = Some(value);
        self
    }

    pub fn with_max_total_tokens(mut self, value: u64) -> Self {
        self.max_total_tokens = Some(value);
        self
    }
}

#[derive(Debug, Clone)]
pub struct CostMetricWriter {
    schema_root: PathBuf,
}

impl CostMetricWriter {
    pub fn new(schema_root: impl Into<PathBuf>) -> Self {
        Self {
            schema_root: schema_root.into(),
        }
    }

    pub fn metric(
        &self,
        job_id: &str,
        stage: impl Into<String>,
        provider_instance_id: impl Into<String>,
        estimated_cost: f64,
        currency: impl Into<String>,
        wall_time_ms: u64,
    ) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": job_id,
            "stage": stage.into(),
            "provider_instance_id": provider_instance_id.into(),
            "input_tokens": 0,
            "output_tokens": 0,
            "estimated_cost": estimated_cost,
            "currency": currency.into(),
            "wall_time_ms": wall_time_ms,
            "quota_remaining": Value::Null
        })
    }

    pub fn write_provider_metric(
        &self,
        store: &StateStore,
        metric: &Value,
    ) -> Result<Value, ObservabilityError> {
        let metric = redact_value(metric.clone());
        self.validate_metric(&metric)?;
        let job_id = required_string(&metric, "job_id", "cost metric")?;
        let provider_instance_id = required_string(&metric, "provider_instance_id", "cost metric")?;
        store
            .write_provider_json(&job_id, &provider_instance_id, COST_METRIC_FILE, &metric)
            .map_err(ObservabilityError::from)
    }

    pub fn read_provider_metric(
        &self,
        store: &StateStore,
        job_id: &str,
        provider_instance_id: &str,
    ) -> Result<Option<Value>, ObservabilityError> {
        let provider_dir = store.resolve_provider_output_dir(job_id, provider_instance_id)?;
        let path = provider_dir.join(COST_METRIC_FILE);
        if !path.is_file() {
            return Ok(None);
        }
        let content =
            fs::read_to_string(&path).map_err(|source| ObservabilityError::ReadFailed {
                path: path.clone(),
                source,
            })?;
        let metric: Value =
            serde_json::from_str(&content).map_err(|source| ObservabilityError::InvalidJson {
                path: path.clone(),
                source,
            })?;
        self.validate_metric(&metric)?;
        Ok(Some(metric))
    }

    pub fn evaluate_budget(
        &self,
        metric: &Value,
        thresholds: &CostBudgetThresholds,
    ) -> Result<Value, ObservabilityError> {
        let metric = redact_value(metric.clone());
        self.validate_metric(&metric)?;
        let job_id = required_string(&metric, "job_id", "cost metric")?;
        let stage = required_string(&metric, "stage", "cost metric")?;
        let provider_instance_id = required_string(&metric, "provider_instance_id", "cost metric")?;
        let metric_path = provider_cost_metric_path(&provider_instance_id)?;
        let estimated_cost = required_f64(&metric, "estimated_cost")?;
        let wall_time_ms = required_u64(&metric, "wall_time_ms")?;
        let total_tokens =
            optional_u64(&metric, "input_tokens")? + optional_u64(&metric, "output_tokens")?;

        let mut reasons = Vec::new();
        if let Some(limit) = thresholds.max_estimated_cost {
            if estimated_cost > limit {
                reasons.push(json!({
                    "kind": "estimated_cost_exceeded",
                    "actual": estimated_cost,
                    "limit": limit
                }));
            }
        }
        if let Some(limit) = thresholds.max_wall_time_ms {
            if wall_time_ms > limit {
                reasons.push(json!({
                    "kind": "wall_time_exceeded",
                    "actual": wall_time_ms,
                    "limit": limit
                }));
            }
        }
        if let Some(limit) = thresholds.max_total_tokens {
            if total_tokens > limit {
                reasons.push(json!({
                    "kind": "total_tokens_exceeded",
                    "actual": total_tokens,
                    "limit": limit
                }));
            }
        }

        let status = if reasons.is_empty() { "ok" } else { "warning" };
        Ok(json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": job_id,
            "stage": stage,
            "provider_instance_id": provider_instance_id,
            "status": status,
            "enforcement": "warn_only",
            "metric_path": metric_path,
            "reasons": reasons,
            "thresholds": thresholds_value(thresholds)
        }))
    }

    pub fn validate_metric(&self, metric: &Value) -> Result<(), ObservabilityError> {
        let schema_path = self.schema_root.join(COST_METRIC_SCHEMA);
        let schema =
            load_schema(&schema_path).map_err(|source| ObservabilityError::SchemaLoadFailed {
                path: schema_path.clone(),
                message: source.to_string(),
            })?;
        let result = validate_json(metric, &schema);
        if result.is_ok() {
            validate_cost_metric_semantics(metric)
        } else {
            Err(ObservabilityError::SchemaValidationFailed {
                path: PathBuf::from(COST_METRIC_SCHEMA),
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

fn provider_cost_metric_path(provider_instance_id: &str) -> Result<String, ObservabilityError> {
    validate_safe_segment(provider_instance_id, "provider_instance_id")?;
    Ok(format!(
        "provider-output/{}/{}",
        provider_instance_id, COST_METRIC_FILE
    ))
}

fn validate_safe_segment(value: &str, field: &str) -> Result<(), ObservabilityError> {
    if value.is_empty()
        || value.contains('\0')
        || value.contains(':')
        || value.contains('/')
        || value.contains('\\')
        || value == "."
        || value == ".."
        || value == ".git"
    {
        return Err(ObservabilityError::InvalidCostMetric {
            message: format!("{} must be a safe path segment", field),
        });
    }
    Ok(())
}

fn validate_cost_metric_semantics(metric: &Value) -> Result<(), ObservabilityError> {
    let estimated_cost = required_f64(metric, "estimated_cost")?;
    if estimated_cost < 0.0 {
        return Err(ObservabilityError::InvalidCostMetric {
            message: "estimated_cost must be non-negative".to_string(),
        });
    }
    required_u64(metric, "wall_time_ms")?;
    optional_u64(metric, "input_tokens")?;
    optional_u64(metric, "output_tokens")?;
    Ok(())
}

fn required_string(value: &Value, field: &str, label: &str) -> Result<String, ObservabilityError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ObservabilityError::InvalidCostMetric {
            message: format!("{} requires string field {}", label, field),
        })
}

fn required_f64(value: &Value, field: &str) -> Result<f64, ObservabilityError> {
    value
        .get(field)
        .and_then(Value::as_f64)
        .ok_or_else(|| ObservabilityError::InvalidCostMetric {
            message: format!("cost metric requires numeric field {}", field),
        })
}

fn required_u64(value: &Value, field: &str) -> Result<u64, ObservabilityError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| ObservabilityError::InvalidCostMetric {
            message: format!("cost metric requires non-negative integer field {}", field),
        })
}

fn optional_u64(value: &Value, field: &str) -> Result<u64, ObservabilityError> {
    match value.get(field) {
        Some(Value::Null) | None => Ok(0),
        Some(item) => item
            .as_u64()
            .ok_or_else(|| ObservabilityError::InvalidCostMetric {
                message: format!("cost metric field {} must be a non-negative integer", field),
            }),
    }
}

fn thresholds_value(thresholds: &CostBudgetThresholds) -> Value {
    json!({
        "max_estimated_cost": thresholds.max_estimated_cost,
        "max_wall_time_ms": thresholds.max_wall_time_ms,
        "max_total_tokens": thresholds.max_total_tokens
    })
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

    #[test]
    fn writes_schema_valid_cost_metric_inside_provider_output() {
        let project = temp_project("cost-write");
        let store = open_store(&project);
        create_job(&store);
        let writer = CostMetricWriter::new(schema_root());
        let metric = writer.metric("J-0001", "implement", "fake-default", 0.0, "USD", 1);

        let artifact_ref = writer
            .write_provider_metric(&store, &metric)
            .expect("write cost metric");
        assert_eq!(
            artifact_ref["path"],
            "provider-output/fake-default/cost-metric.json"
        );
        assert_eq!(artifact_ref["kind"], "provider_output");

        let read = writer
            .read_provider_metric(&store, "J-0001", "fake-default")
            .expect("read cost metric")
            .expect("cost metric exists");
        assert_eq!(read["estimated_cost"], 0.0);
        assert_eq!(read["wall_time_ms"], 1);

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn cost_metric_writer_redacts_unexpected_secret_fields() {
        let project = temp_project("cost-redact");
        let store = open_store(&project);
        create_job(&store);
        let writer = CostMetricWriter::new(schema_root());
        let api_key = format!("{}{}", "sk-test", "-secret");
        let mut metric = writer.metric("J-0001", "implement", "cloud-default", 1.0, "USD", 20);
        metric["debug"] = json!(format!("Authorization: Bearer {}", api_key));

        writer
            .write_provider_metric(&store, &metric)
            .expect("write redacted cost metric");
        let text = fs::read_to_string(
            project.join(".ai-runs/J-0001/provider-output/cloud-default/cost-metric.json"),
        )
        .expect("read cost metric");
        assert!(!text.contains(&api_key));
        assert!(!text.contains("Bearer"));
        assert!(text.contains("[REDACTED]"));

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn budget_guard_warns_without_requiring_metric_to_exist() {
        let project = temp_project("budget");
        let store = open_store(&project);
        create_job(&store);
        let writer = CostMetricWriter::new(schema_root());
        assert!(writer
            .read_provider_metric(&store, "J-0001", "fake-default")
            .expect("missing cost metric is not fatal")
            .is_none());

        let mut metric = writer.metric("J-0001", "implement", "cloud-default", 1.25, "USD", 50);
        metric["input_tokens"] = json!(25);
        metric["output_tokens"] = json!(30);
        let evaluation = writer
            .evaluate_budget(
                &metric,
                &CostBudgetThresholds::new()
                    .with_max_estimated_cost(1.0)
                    .with_max_wall_time_ms(25)
                    .with_max_total_tokens(50),
            )
            .expect("evaluate budget");

        assert_eq!(evaluation["status"], "warning");
        assert_eq!(evaluation["enforcement"], "warn_only");
        assert_eq!(evaluation["reasons"].as_array().expect("reasons").len(), 3);
        assert_eq!(
            evaluation["metric_path"],
            "provider-output/cloud-default/cost-metric.json"
        );

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn cost_metric_writer_rejects_unsafe_provider_path() {
        let project = temp_project("cost-traversal");
        let store = open_store(&project);
        create_job(&store);
        let writer = CostMetricWriter::new(schema_root());
        let metric = writer.metric("J-0001", "implement", "../cloud-default", 0.0, "USD", 1);

        let result = writer.write_provider_metric(&store, &metric);
        assert!(result.is_err());
        assert!(!project
            .join(".ai-runs/J-0001/provider-output/cost-metric.json")
            .exists());

        fs::remove_dir_all(project).ok();
    }
}
