use serde_json::{json, Map, Value};
use star_control_daemon::{DaemonError, DaemonQueue};
use star_control_schema::{load_schema, validate_json, ValidationError};
use star_control_state::{JobSummary, StateStore, StateStoreError};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

const SCHEMA_VERSION: &str = "1.0.0";
const API_RESPONSE_SCHEMA: &str = "api-response.schema.json";
const DEFAULT_REPORT_STAGE: &str = "implement";
const CANONICAL_STAGES: &[&str] = &[
    "route",
    "plan",
    "design",
    "implement",
    "validate",
    "review",
    "polish",
    "report",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl ApiMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApiRequest {
    method: ApiMethod,
    path: String,
}

impl ApiRequest {
    pub fn new(method: ApiMethod, path: impl Into<String>) -> Self {
        Self {
            method,
            path: path.into(),
        }
    }

    pub fn get(path: impl Into<String>) -> Self {
        Self::new(ApiMethod::Get, path)
    }

    pub fn method(&self) -> ApiMethod {
        self.method
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

#[derive(Debug)]
pub enum ApiError {
    DuplicateProject {
        project_id: String,
    },
    InvalidProjectId {
        project_id: String,
    },
    SchemaLoadFailed {
        path: PathBuf,
        message: String,
    },
    SchemaValidationFailed {
        path: PathBuf,
        errors: Vec<ValidationError>,
    },
}

impl fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateProject { project_id } => {
                write!(formatter, "duplicate API project id: {}", project_id)
            }
            Self::InvalidProjectId { project_id } => {
                write!(formatter, "invalid API project id: {}", project_id)
            }
            Self::SchemaLoadFailed { path, message } => {
                write!(
                    formatter,
                    "schema load failed at {}: {}",
                    path.display(),
                    message
                )
            }
            Self::SchemaValidationFailed { path, errors } => {
                write!(
                    formatter,
                    "schema validation failed for {} with {} error(s)",
                    path.display(),
                    errors.len()
                )
            }
        }
    }
}

impl Error for ApiError {}

#[derive(Debug, Clone)]
pub struct ApiReadOnlyService {
    schema_root: PathBuf,
    daemon_queue: Option<DaemonQueue>,
    projects: BTreeMap<String, StateStore>,
}

impl ApiReadOnlyService {
    pub fn new(schema_root: impl Into<PathBuf>) -> Self {
        Self {
            schema_root: schema_root.into(),
            daemon_queue: None,
            projects: BTreeMap::new(),
        }
    }

    pub fn register_daemon_queue(&mut self, daemon_queue: DaemonQueue) {
        self.daemon_queue = Some(daemon_queue);
    }

    pub fn register_project_store(
        &mut self,
        project_id: impl Into<String>,
        store: StateStore,
    ) -> Result<(), ApiError> {
        let project_id = project_id.into();
        validate_project_id(&project_id)?;
        if self.projects.contains_key(&project_id) {
            return Err(ApiError::DuplicateProject { project_id });
        }
        self.projects.insert(project_id, store);
        Ok(())
    }

    pub fn handle_get(&self, path: &str) -> Result<Value, ApiError> {
        self.handle(ApiRequest::get(path))
    }

    pub fn handle(&self, request: ApiRequest) -> Result<Value, ApiError> {
        if request.method() != ApiMethod::Get {
            return self.error_envelope(
                "method_not_allowed",
                &format!(
                    "read-only API only supports GET, got {}",
                    request.method().as_str()
                ),
                json!({ "method": request.method().as_str(), "path": request.path() }),
            );
        }

        let parsed = ParsedPath::parse(request.path());
        let segments = parsed.segments();
        match segments.as_slice() {
            ["daemon", "state"] => self.daemon_state_response(),
            ["projects"] => self.projects_response(),
            ["projects", project_id, "jobs"] => self.jobs_response(project_id),
            ["projects", project_id, "jobs", job_id] => self.job_response(project_id, job_id),
            ["projects", project_id, "jobs", job_id, "events"] => {
                self.events_response(project_id, job_id)
            }
            ["projects", project_id, "jobs", job_id, "report"] => {
                let stage = parsed.query_value("stage").unwrap_or(DEFAULT_REPORT_STAGE);
                self.report_response(project_id, job_id, stage)
            }
            _ => self.error_envelope(
                "endpoint_not_found",
                "read-only API endpoint not found",
                json!({ "path": request.path() }),
            ),
        }
    }

    fn daemon_state_response(&self) -> Result<Value, ApiError> {
        let Some(daemon_queue) = &self.daemon_queue else {
            return self.error_envelope(
                "daemon_not_registered",
                "daemon queue is not registered in read-only API",
                json!({}),
            );
        };
        match daemon_queue.load_state() {
            Ok(state) => self.success_envelope(json!({
                "daemon_state": state
            })),
            Err(source) => self.daemon_error_envelope("daemon_state_read_failed", source),
        }
    }

    fn projects_response(&self) -> Result<Value, ApiError> {
        let projects = self
            .projects
            .iter()
            .map(|(project_id, store)| {
                json!({
                    "project_id": project_id,
                    "project_root": store.project_root().display().to_string(),
                    "ai_runs_dir": ".ai-runs"
                })
            })
            .collect::<Vec<_>>();
        self.success_envelope(json!({ "projects": projects }))
    }

    fn jobs_response(&self, project_id: &str) -> Result<Value, ApiError> {
        let Some(store) = self.projects.get(project_id) else {
            return self.project_not_found(project_id);
        };
        match store.list_jobs() {
            Ok(jobs) => {
                let jobs = jobs.iter().map(job_summary_value).collect::<Vec<Value>>();
                self.success_envelope(json!({
                    "project_id": project_id,
                    "project_root": store.project_root().display().to_string(),
                    "jobs": jobs
                }))
            }
            Err(source) => self.state_error_envelope("jobs_read_failed", source),
        }
    }

    fn job_response(&self, project_id: &str, job_id: &str) -> Result<Value, ApiError> {
        let Some(store) = self.projects.get(project_id) else {
            return self.project_not_found(project_id);
        };
        let job = match store.load_job(job_id) {
            Ok(value) => value,
            Err(source) => return self.state_error_envelope("job_read_failed", source),
        };
        let state = match store.load_state(job_id) {
            Ok(value) => value,
            Err(source) => return self.state_error_envelope("state_read_failed", source),
        };
        let latest_event = store
            .read_events(job_id)
            .ok()
            .and_then(|events| events.last().cloned())
            .unwrap_or_else(|| json!({}));
        let api_status = status_for_run_state(state.get("state").and_then(Value::as_str));
        self.envelope(
            api_status,
            json!({
                "project_id": project_id,
                "project_root": store.project_root().display().to_string(),
                "job_id": job_id,
                "run_dir": format!(".ai-runs/{}", job_id),
                "job": job,
                "state": state,
                "latest_event": latest_event
            }),
            Value::Null,
            Vec::new(),
        )
    }

    fn events_response(&self, project_id: &str, job_id: &str) -> Result<Value, ApiError> {
        let Some(store) = self.projects.get(project_id) else {
            return self.project_not_found(project_id);
        };
        match store.read_events(job_id) {
            Ok(events) => self.success_envelope(json!({
                "project_id": project_id,
                "job_id": job_id,
                "run_dir": format!(".ai-runs/{}", job_id),
                "event_count": events.len(),
                "events": events
            })),
            Err(source) => self.state_error_envelope("events_read_failed", source),
        }
    }

    fn report_response(
        &self,
        project_id: &str,
        job_id: &str,
        stage: &str,
    ) -> Result<Value, ApiError> {
        let Some(store) = self.projects.get(project_id) else {
            return self.project_not_found(project_id);
        };
        if !CANONICAL_STAGES.contains(&stage) {
            return self.error_envelope(
                "invalid_report_stage",
                "report stage is not canonical",
                json!({ "stage": stage }),
            );
        }
        let report_name = format!("{}-report", stage);
        match store.load_report(job_id, &report_name) {
            Ok(report) => {
                let status = status_for_report(report.get("status").and_then(Value::as_str));
                self.envelope(
                    status,
                    json!({
                        "project_id": project_id,
                        "job_id": job_id,
                        "stage": stage,
                        "report_path": format!(".ai-runs/{}/reports/{}.json", job_id, report_name),
                        "report": report
                    }),
                    Value::Null,
                    Vec::new(),
                )
            }
            Err(source) => self.state_error_envelope("report_read_failed", source),
        }
    }

    fn project_not_found(&self, project_id: &str) -> Result<Value, ApiError> {
        self.error_envelope(
            "project_not_found",
            "project is not registered in read-only API",
            json!({ "project_id": project_id }),
        )
    }

    fn state_error_envelope(&self, code: &str, source: StateStoreError) -> Result<Value, ApiError> {
        let status = match source {
            StateStoreError::ArtifactNotFound { .. } | StateStoreError::JobNotFound { .. } => {
                "failed"
            }
            StateStoreError::TerminalStateBlocked { .. } => "blocked",
            _ => "failed",
        };
        self.envelope(
            status,
            json!({}),
            json!({
                "code": code,
                "message": source.to_string()
            }),
            Vec::new(),
        )
    }

    fn daemon_error_envelope(&self, code: &str, source: DaemonError) -> Result<Value, ApiError> {
        self.envelope(
            "failed",
            json!({}),
            json!({
                "code": code,
                "message": source.to_string()
            }),
            Vec::new(),
        )
    }

    fn success_envelope(&self, data: Value) -> Result<Value, ApiError> {
        self.envelope("success", data, Value::Null, Vec::new())
    }

    fn error_envelope(&self, code: &str, message: &str, details: Value) -> Result<Value, ApiError> {
        self.envelope(
            "failed",
            json!({}),
            json!({
                "code": code,
                "message": message,
                "details": details
            }),
            Vec::new(),
        )
    }

    fn envelope(
        &self,
        status: &str,
        data: Value,
        error: Value,
        warnings: Vec<String>,
    ) -> Result<Value, ApiError> {
        let envelope = json!({
            "schema_version": SCHEMA_VERSION,
            "status": status,
            "data": redact_value(data),
            "error": redact_value(error),
            "warnings": warnings
        });
        self.validate_response(&envelope)?;
        Ok(envelope)
    }

    fn validate_response(&self, envelope: &Value) -> Result<(), ApiError> {
        let schema_path = self.schema_root.join(API_RESPONSE_SCHEMA);
        let schema = load_schema(&schema_path).map_err(|source| ApiError::SchemaLoadFailed {
            path: schema_path,
            message: source.to_string(),
        })?;
        let result = validate_json(envelope, &schema);
        if result.is_ok() {
            Ok(())
        } else {
            Err(ApiError::SchemaValidationFailed {
                path: PathBuf::from(API_RESPONSE_SCHEMA),
                errors: result.errors,
            })
        }
    }
}

#[derive(Debug, Clone)]
struct ParsedPath {
    path: String,
    query: BTreeMap<String, String>,
}

impl ParsedPath {
    fn parse(raw_path: &str) -> Self {
        let (path, query) = raw_path.split_once('?').unwrap_or((raw_path, ""));
        let mut query_map = BTreeMap::new();
        for pair in query.split('&').filter(|pair| !pair.is_empty()) {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            query_map.insert(key.to_string(), value.to_string());
        }
        Self {
            path: path.to_string(),
            query: query_map,
        }
    }

    fn segments(&self) -> Vec<&str> {
        self.path
            .trim_matches('/')
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect()
    }

    fn query_value(&self, key: &str) -> Option<&str> {
        self.query.get(key).map(String::as_str)
    }
}

fn validate_project_id(project_id: &str) -> Result<(), ApiError> {
    let valid = !project_id.is_empty()
        && project_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if valid {
        Ok(())
    } else {
        Err(ApiError::InvalidProjectId {
            project_id: project_id.to_string(),
        })
    }
}

fn job_summary_value(summary: &JobSummary) -> Value {
    json!({
        "job_id": summary.job_id,
        "state": summary.state,
        "current_stage": summary.current_stage,
        "created_at": summary.created_at,
        "updated_at": summary.updated_at,
        "summary": summary.summary,
        "corrupt": summary.corrupt,
        "corrupt_reason": summary.corrupt_reason,
        "run_dir": format!(".ai-runs/{}", summary.job_id)
    })
}

fn status_for_run_state(state: Option<&str>) -> &'static str {
    match state.unwrap_or_default() {
        "WAITING_APPROVAL" => "waiting_approval",
        "BLOCKED" => "blocked",
        "FAILED" | "CANCELLED" => "failed",
        _ => "success",
    }
}

fn status_for_report(status: Option<&str>) -> &'static str {
    match status.unwrap_or_default() {
        "NEEDS_APPROVAL" | "NEEDS_REVIEW" => "waiting_approval",
        "BLOCKED" => "blocked",
        "FAILED" => "failed",
        _ => "success",
    }
}

fn redact_value(value: Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(redact_object(object)),
        Value::Array(items) => Value::Array(items.into_iter().map(redact_value).collect()),
        Value::String(text) if looks_sensitive_string(&text) => json!("[REDACTED]"),
        other => other,
    }
}

fn redact_object(object: Map<String, Value>) -> Map<String, Value> {
    object
        .into_iter()
        .map(|(key, value)| {
            if is_sensitive_key(&key) {
                (key, json!("[REDACTED]"))
            } else {
                (key, redact_value(value))
            }
        })
        .collect()
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("credential")
        || key.contains("secret")
        || key.contains("password")
        || key.contains("api_key")
        || key.contains("apikey")
        || key.contains("authorization")
        || key == "token"
        || key.ends_with("_token")
}

fn looks_sensitive_string(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("bearer ")
        || lower.contains("api_key=")
        || lower.contains("apikey=")
        || lower.contains("password=")
        || lower.contains("token=")
        || value.contains("sk-")
        || value.contains("-----BEGIN PRIVATE KEY-----")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use star_control_daemon::{DaemonConfig, DaemonQueue};
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn schema_root() -> PathBuf {
        repo_root().join("specs/schemas")
    }

    fn temp_project() -> PathBuf {
        let count = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "star-control-api-{}-{}-{}",
            std::process::id(),
            timestamp_nanos(),
            count
        ));
        fs::create_dir_all(&path).expect("create temp project");
        path
    }

    fn timestamp_nanos() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    }

    fn open_store(project: &Path) -> StateStore {
        StateStore::open(project, schema_root()).expect("open state store")
    }

    fn api_with_store(store: StateStore) -> ApiReadOnlyService {
        let mut service = ApiReadOnlyService::new(schema_root());
        service
            .register_project_store("local", store)
            .expect("register project");
        service
    }

    fn open_daemon_queue(config_root: &Path) -> DaemonQueue {
        DaemonQueue::open(DaemonConfig::local(config_root, schema_root()))
            .expect("open daemon queue")
    }

    fn create_job(store: &StateStore, state_name: &str, stage: &str) {
        let job = store
            .create_job("implement API", "README.md", Vec::new())
            .expect("create job");
        let job_id = job["job_id"].as_str().expect("job id");
        store
            .save_state(job_id, &run_state(job_id, state_name, stage))
            .expect("save state");
    }

    fn run_state(job_id: &str, state_name: &str, stage: &str) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": job_id,
            "state": state_name,
            "current_stage": stage,
            "updated_at": "unix:1",
            "workers": {},
            "artifacts": {},
            "next_action": "report"
        })
    }

    fn event(job_id: &str) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "event_id": "J-0001-api-test",
            "job_id": job_id,
            "type": "STATE_CHANGED",
            "created_at": "unix:2",
            "stage": "implement",
            "state": "IMPLEMENTED",
            "message": "implemented",
            "artifact_paths": ["run-state.json"],
            "details": {}
        })
    }

    fn report(job_id: &str, risks: Vec<&str>) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": job_id,
            "stage": "implement",
            "status": "DONE",
            "changed_files": [],
            "commands_run": [],
            "validation": [],
            "risks": risks,
            "blocked_reason": null,
            "next_step": "done",
            "artifacts": []
        })
    }

    #[test]
    fn projects_jobs_and_job_detail_are_schema_valid_and_read_only() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store, "IMPLEMENTED", "implement");
        let state_path = project.join(".ai-runs/J-0001/run-state.json");
        let before_state = fs::read_to_string(&state_path).expect("read state before");
        let service = api_with_store(store);

        let projects = service.handle_get("/projects").expect("projects");
        assert_eq!(projects["status"], "success");
        assert_eq!(projects["data"]["projects"][0]["project_id"], "local");

        let jobs = service.handle_get("/projects/local/jobs").expect("jobs");
        assert_eq!(jobs["status"], "success");
        assert_eq!(jobs["data"]["jobs"][0]["job_id"], "J-0001");
        assert_eq!(jobs["data"]["jobs"][0]["run_dir"], ".ai-runs/J-0001");

        let detail = service
            .handle_get("/projects/local/jobs/J-0001")
            .expect("job detail");
        assert_eq!(detail["status"], "success");
        assert_eq!(detail["data"]["state"]["state"], "IMPLEMENTED");
        assert_eq!(detail["data"]["run_dir"], ".ai-runs/J-0001");

        let after_state = fs::read_to_string(&state_path).expect("read state after");
        assert_eq!(after_state, before_state);
        assert!(!project.join(".ai-runs/J-0001/api-response.json").exists());

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn daemon_state_endpoint_reads_registered_queue_state() {
        let config = temp_project();
        let queue = open_daemon_queue(&config);
        let state_path = queue.state_path().to_path_buf();
        let before_state = fs::read_to_string(&state_path).expect("read daemon state before");
        let mut service = ApiReadOnlyService::new(schema_root());
        service.register_daemon_queue(queue);

        let response = service.handle_get("/daemon/state").expect("daemon state");
        assert_eq!(response["status"], "success");
        assert_eq!(response["data"]["daemon_state"]["status"], "reserved");
        assert_eq!(
            response["data"]["daemon_state"]["queue"]
                .as_array()
                .expect("queue")
                .len(),
            0
        );

        let after_state = fs::read_to_string(&state_path).expect("read daemon state after");
        assert_eq!(after_state, before_state);

        fs::remove_dir_all(config).ok();
    }

    #[test]
    fn events_and_report_endpoints_return_read_artifacts() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store, "DONE", "report");
        store
            .append_event("J-0001", &event("J-0001"))
            .expect("event");
        store
            .save_report("J-0001", "implement-report", &report("J-0001", Vec::new()))
            .expect("save report");
        let service = api_with_store(store);

        let events = service
            .handle_get("/projects/local/jobs/J-0001/events")
            .expect("events");
        assert_eq!(events["status"], "success");
        assert_eq!(events["data"]["event_count"], 2);

        let report = service
            .handle_get("/projects/local/jobs/J-0001/report?stage=implement")
            .expect("report");
        assert_eq!(report["status"], "success");
        assert_eq!(
            report["data"]["report_path"],
            ".ai-runs/J-0001/reports/implement-report.json"
        );
        assert_eq!(report["data"]["report"]["status"], "DONE");

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn missing_project_job_and_report_are_structured_errors() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store, "IMPLEMENTED", "implement");
        let service = api_with_store(store);

        let missing_project = service
            .handle_get("/projects/missing/jobs")
            .expect("missing project response");
        assert_eq!(missing_project["status"], "failed");
        assert_eq!(missing_project["error"]["code"], "project_not_found");

        let missing_job = service
            .handle_get("/projects/local/jobs/J-9999")
            .expect("missing job response");
        assert_eq!(missing_job["status"], "failed");
        assert_eq!(missing_job["error"]["code"], "job_read_failed");

        let missing_report = service
            .handle_get("/projects/local/jobs/J-0001/report?stage=implement")
            .expect("missing report response");
        assert_eq!(missing_report["status"], "failed");
        assert_eq!(missing_report["error"]["code"], "report_read_failed");

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn mutation_methods_and_unknown_paths_are_not_implemented() {
        let service = ApiReadOnlyService::new(schema_root());

        let missing_daemon = service
            .handle_get("/daemon/state")
            .expect("missing daemon response");
        assert_eq!(missing_daemon["status"], "failed");
        assert_eq!(missing_daemon["error"]["code"], "daemon_not_registered");

        let mutation = service
            .handle(ApiRequest::new(ApiMethod::Post, "/projects/local/jobs"))
            .expect("mutation response");
        assert_eq!(mutation["status"], "failed");
        assert_eq!(mutation["error"]["code"], "method_not_allowed");

        let unknown = service.handle_get("/projects/local/jobs/J-0001/approve");
        let unknown = unknown.expect("unknown response");
        assert_eq!(unknown["status"], "failed");
        assert_eq!(unknown["error"]["code"], "endpoint_not_found");
    }

    #[test]
    fn report_response_redacts_sensitive_values() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store, "DONE", "report");
        store
            .save_report(
                "J-0001",
                "implement-report",
                &report("J-0001", vec!["Authorization: Bearer sk-test-secret"]),
            )
            .expect("save report");
        let service = api_with_store(store);

        let response = service
            .handle_get("/projects/local/jobs/J-0001/report")
            .expect("report");
        let text = serde_json::to_string(&response).expect("response text");
        assert!(!text.contains("sk-test-secret"));
        assert!(text.contains("[REDACTED]"));

        fs::remove_dir_all(project).ok();
    }
}
