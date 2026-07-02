use serde_json::{json, Value};
use star_control_daemon::{DaemonError, DaemonQueue};
use star_control_release::{ReleaseReadinessError, ReleaseReadinessWriter, RELEASE_READINESS_PATH};
use star_control_schema::{load_schema, validate_json, ValidationError};
use star_control_security::redact_value;
use star_control_state::{JobSummary, StateStore, StateStoreError};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: &str = "1.0.0";
const API_RESPONSE_SCHEMA: &str = "api-response.schema.json";
const APPROVAL_REQUEST_SCHEMA: &str = "approval-request.schema.json";
const APPROVAL_RESPONSE_SCHEMA: &str = "approval-response.schema.json";
const DEFAULT_REPORT_STAGE: &str = "implement";
const TERMINAL_STATES: &[&str] = &["DONE", "FAILED", "BLOCKED", "CANCELLED"];
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
    body: Value,
}

impl ApiRequest {
    pub fn new(method: ApiMethod, path: impl Into<String>) -> Self {
        Self::with_body(method, path, Value::Null)
    }

    pub fn with_body(method: ApiMethod, path: impl Into<String>, body: Value) -> Self {
        Self {
            method,
            path: path.into(),
            body,
        }
    }

    pub fn get(path: impl Into<String>) -> Self {
        Self::new(ApiMethod::Get, path)
    }

    pub fn post(path: impl Into<String>, body: Value) -> Self {
        Self::with_body(ApiMethod::Post, path, body)
    }

    pub fn method(&self) -> ApiMethod {
        self.method
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn body(&self) -> &Value {
        &self.body
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
            ["projects", project_id, "jobs", job_id, "release-readiness"] => {
                self.release_readiness_response(project_id, job_id)
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

    fn release_readiness_response(
        &self,
        project_id: &str,
        job_id: &str,
    ) -> Result<Value, ApiError> {
        let Some(store) = self.projects.get(project_id) else {
            return self.project_not_found(project_id);
        };
        if let Err(source) = store.load_job(job_id) {
            return self.state_error_envelope("job_read_failed", source);
        }
        let writer = ReleaseReadinessWriter::new(&self.schema_root);
        match writer.read(store, job_id) {
            Ok(Some(readiness)) => self.success_envelope(json!({
                "project_id": project_id,
                "job_id": job_id,
                "readiness_path": format!(".ai-runs/{}/{}", job_id, RELEASE_READINESS_PATH),
                "readiness": readiness
            })),
            Ok(None) => self.error_envelope(
                "release_readiness_not_found",
                "release readiness artifact not found",
                json!({
                    "project_id": project_id,
                    "job_id": job_id,
                    "artifact_path": format!(".ai-runs/{}/{}", job_id, RELEASE_READINESS_PATH)
                }),
            ),
            Err(source) => self.release_error_envelope("release_readiness_read_failed", source),
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

    fn release_error_envelope(
        &self,
        code: &str,
        source: ReleaseReadinessError,
    ) -> Result<Value, ApiError> {
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
pub struct ApiControlService {
    read_only: ApiReadOnlyService,
}

impl ApiControlService {
    pub fn new(schema_root: impl Into<PathBuf>) -> Self {
        Self {
            read_only: ApiReadOnlyService::new(schema_root),
        }
    }

    pub fn from_read_only(read_only: ApiReadOnlyService) -> Self {
        Self { read_only }
    }

    pub fn register_daemon_queue(&mut self, daemon_queue: DaemonQueue) {
        self.read_only.register_daemon_queue(daemon_queue);
    }

    pub fn register_project_store(
        &mut self,
        project_id: impl Into<String>,
        store: StateStore,
    ) -> Result<(), ApiError> {
        self.read_only.register_project_store(project_id, store)
    }

    pub fn handle_get(&self, path: &str) -> Result<Value, ApiError> {
        self.read_only.handle_get(path)
    }

    pub fn handle_post(&self, path: &str, body: Value) -> Result<Value, ApiError> {
        self.handle(ApiRequest::post(path, body))
    }

    pub fn handle(&self, request: ApiRequest) -> Result<Value, ApiError> {
        if request.method() == ApiMethod::Get {
            return self.read_only.handle(request);
        }
        if request.method() != ApiMethod::Post {
            return self.read_only.error_envelope(
                "method_not_allowed",
                &format!(
                    "control API supports GET and POST, got {}",
                    request.method().as_str()
                ),
                json!({ "method": request.method().as_str(), "path": request.path() }),
            );
        }

        let parsed = ParsedPath::parse(request.path());
        let segments = parsed.segments();
        match segments.as_slice() {
            ["projects", project_id, "jobs", job_id, "approve"] => {
                self.approve_response(project_id, job_id, request.body())
            }
            ["projects", project_id, "jobs", job_id, "cancel"] => {
                self.cancel_response(project_id, job_id)
            }
            ["projects", project_id, "jobs", job_id, "resume"] => {
                self.resume_response(project_id, job_id)
            }
            _ => self.read_only.error_envelope(
                "endpoint_not_found",
                "control API endpoint not found",
                json!({ "method": request.method().as_str(), "path": request.path() }),
            ),
        }
    }

    fn approve_response(
        &self,
        project_id: &str,
        job_id: &str,
        body: &Value,
    ) -> Result<Value, ApiError> {
        let Some(store) = self.read_only.projects.get(project_id) else {
            return self.read_only.project_not_found(project_id);
        };
        let mut state = match store.load_state(job_id) {
            Ok(value) => value,
            Err(source) => {
                return self
                    .read_only
                    .state_error_envelope("state_read_failed", source)
            }
        };
        let current_state = state_string(&state);
        if current_state != "WAITING_APPROVAL" {
            return self.read_only.error_envelope(
                "invalid_control_state",
                "approve requires WAITING_APPROVAL state",
                json!({ "job_id": job_id, "state": current_state }),
            );
        }

        let response = match body_string(body, "response") {
            Ok(value) => value,
            Err(message) => return self.invalid_control_request(&message),
        };
        if !matches!(
            response.as_str(),
            "approved" | "rejected" | "needs_changes" | "cancelled"
        ) {
            return self
                .invalid_control_request(&format!("unsupported approval response {}", response));
        }
        let reason = match body_string(body, "reason") {
            Ok(value) => value,
            Err(message) => return self.invalid_control_request(&message),
        };
        let reviewer =
            body_string(body, "reviewer").unwrap_or_else(|_| "star-control-api".to_string());
        let constraints = match body_string_array(body, "constraints") {
            Ok(value) => value,
            Err(message) => return self.invalid_control_request(&message),
        };

        let approval_request = match load_job_json(
            store,
            job_id,
            "approvals/approval-request.json",
            APPROVAL_REQUEST_SCHEMA,
            &self.read_only.schema_root,
        ) {
            Ok(value) => value,
            Err(ControlArtifactError::Missing { path }) => {
                return self.read_only.error_envelope(
                    "approval_request_missing",
                    "approval request artifact is required before approve",
                    json!({ "path": path }),
                )
            }
            Err(error) => {
                return self.read_only.error_envelope(
                    "approval_request_invalid",
                    &error.to_string(),
                    json!({ "job_id": job_id }),
                )
            }
        };
        let stage = string_field(&approval_request, "stage").unwrap_or("validate");
        let task_id = string_field(&approval_request, "task_id").unwrap_or("approval");
        let allowed_next_stage = (response == "approved")
            .then(|| allowed_next_stage_for(stage))
            .flatten();
        let approval_response = json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": job_id,
            "stage": stage,
            "task_id": task_id,
            "response": response,
            "reviewer": reviewer,
            "responded_at": timestamp_string(),
            "reason": reason,
            "allowed_next_stage": allowed_next_stage,
            "constraints": constraints
        });
        if let Err(errors) = validate_schema_value(
            &approval_response,
            &self.read_only.schema_root,
            APPROVAL_RESPONSE_SCHEMA,
        ) {
            let message = format!(
                "approval response failed schema validation with {} error(s)",
                errors
            );
            return self.read_only.error_envelope(
                "approval_response_invalid",
                &message,
                json!({ "job_id": job_id }),
            );
        }

        let approval_ref =
            match store.write_approval_json(job_id, "approval-response.json", &approval_response) {
                Ok(value) => value,
                Err(source) => {
                    return self
                        .read_only
                        .state_error_envelope("approval_response_write_failed", source)
                }
            };
        let next_state = state_after_approval_response(&response);
        let next_action = next_action_after_approval_response(&response);
        let event_id = format!("{}-api-approval-recorded", job_id.to_ascii_lowercase());
        if let Err(source) = update_state_for_control_command(
            &mut state,
            store,
            next_state,
            stage,
            next_action,
            &event_id,
            Some(("approval_response", &approval_ref)),
        ) {
            return self
                .read_only
                .state_error_envelope("state_update_failed", source);
        }
        if let Err(source) = store.save_state(job_id, &state) {
            return self
                .read_only
                .state_error_envelope("state_write_failed", source);
        }
        if let Err(source) = append_api_event(
            store,
            job_id,
            ApiControlEvent {
                event_id,
                event_type: "APPROVAL_RECORDED",
                state: next_state,
                stage,
                message: "Approval response recorded by API",
                artifact_paths: vec!["approvals/approval-response.json".to_string()],
                details: json!({
                    "response": approval_response["response"],
                    "allowed_next_stage": approval_response["allowed_next_stage"]
                }),
            },
        ) {
            return self
                .read_only
                .state_error_envelope("event_write_failed", source);
        }

        self.read_only.success_envelope(json!({
            "command": "approve",
            "job_id": job_id,
            "state": state["state"],
            "approval_response": approval_response["response"],
            "allowed_next_stage": approval_response["allowed_next_stage"],
            "artifacts": [format!(".ai-runs/{}/approvals/approval-response.json", job_id)]
        }))
    }

    fn cancel_response(&self, project_id: &str, job_id: &str) -> Result<Value, ApiError> {
        let Some(store) = self.read_only.projects.get(project_id) else {
            return self.read_only.project_not_found(project_id);
        };
        let mut state = match store.load_state(job_id) {
            Ok(value) => value,
            Err(source) => {
                return self
                    .read_only
                    .state_error_envelope("state_read_failed", source)
            }
        };
        let current_state = state_string(&state);
        if TERMINAL_STATES.contains(&current_state.as_str()) {
            return self.read_only.error_envelope(
                "invalid_control_state",
                "cannot cancel terminal job state",
                json!({ "job_id": job_id, "state": current_state }),
            );
        }
        let current_stage = string_field(&state, "current_stage")
            .unwrap_or("implement")
            .to_string();
        let event_id = format!("{}-api-cancelled", job_id.to_ascii_lowercase());
        if let Err(source) = update_state_for_control_command(
            &mut state,
            store,
            "CANCELLED",
            &current_stage,
            "stop",
            &event_id,
            None,
        ) {
            return self
                .read_only
                .state_error_envelope("state_update_failed", source);
        }
        if let Some(state_object) = state.as_object_mut() {
            state_object.insert("active_provider".to_string(), Value::Null);
        }
        if let Err(source) = store.save_state(job_id, &state) {
            return self
                .read_only
                .state_error_envelope("state_write_failed", source);
        }
        if let Err(source) = append_api_event(
            store,
            job_id,
            ApiControlEvent {
                event_id,
                event_type: "STATE_CHANGED",
                state: "CANCELLED",
                stage: &current_stage,
                message: "Job cancelled by API",
                artifact_paths: vec!["run-state.json".to_string()],
                details: json!({ "previous_state": current_state }),
            },
        ) {
            return self
                .read_only
                .state_error_envelope("event_write_failed", source);
        }

        self.read_only.success_envelope(json!({
            "command": "cancel",
            "job_id": job_id,
            "state": "CANCELLED",
            "previous_state": current_state,
            "next_action": "stop",
            "artifacts": [format!(".ai-runs/{}/run-state.json", job_id)]
        }))
    }

    fn resume_response(&self, project_id: &str, job_id: &str) -> Result<Value, ApiError> {
        let Some(store) = self.read_only.projects.get(project_id) else {
            return self.read_only.project_not_found(project_id);
        };
        if let Err(source) = store.ensure_resume_allowed(job_id) {
            return self
                .read_only
                .state_error_envelope("resume_precondition_failed", source);
        }
        let mut state = match store.load_state(job_id) {
            Ok(value) => value,
            Err(source) => {
                return self
                    .read_only
                    .state_error_envelope("state_read_failed", source)
            }
        };
        let current_state = state_string(&state);
        let current_stage = string_field(&state, "current_stage")
            .unwrap_or("implement")
            .to_string();

        if current_state != "WAITING_APPROVAL" {
            return self.read_only.success_envelope(json!({
                "command": "resume",
                "job_id": job_id,
                "state": current_state,
                "current_stage": current_stage,
                "next_action": state.get("next_action").cloned().unwrap_or_else(|| json!("")),
                "resumed": false,
                "artifacts": [format!(".ai-runs/{}/run-state.json", job_id)]
            }));
        }

        let approval_request = match load_job_json(
            store,
            job_id,
            "approvals/approval-request.json",
            APPROVAL_REQUEST_SCHEMA,
            &self.read_only.schema_root,
        ) {
            Ok(value) => value,
            Err(ControlArtifactError::Missing { path }) => {
                return self.read_only.error_envelope(
                    "approval_request_missing",
                    "approval request artifact is required before resume",
                    json!({ "path": path }),
                )
            }
            Err(error) => {
                return self.read_only.error_envelope(
                    "approval_request_invalid",
                    &error.to_string(),
                    json!({ "job_id": job_id }),
                )
            }
        };
        let approval_response = match load_job_json(
            store,
            job_id,
            "approvals/approval-response.json",
            APPROVAL_RESPONSE_SCHEMA,
            &self.read_only.schema_root,
        ) {
            Ok(value) => value,
            Err(ControlArtifactError::Missing { path }) => {
                return self.read_only.error_envelope(
                    "approval_response_missing",
                    "approval response artifact is required before resume",
                    json!({ "path": path }),
                )
            }
            Err(error) => {
                return self.read_only.error_envelope(
                    "approval_response_invalid",
                    &error.to_string(),
                    json!({ "job_id": job_id }),
                )
            }
        };
        if let Err(message) =
            ensure_approval_response_matches_request(&approval_request, &approval_response)
        {
            return self.read_only.error_envelope(
                "invalid_control_state",
                &message,
                json!({ "job_id": job_id }),
            );
        }

        let event_id = format!("{}-api-resumed", job_id.to_ascii_lowercase());
        let next_action = approval_response
            .get("allowed_next_stage")
            .and_then(Value::as_str)
            .unwrap_or("report");
        if let Err(source) = update_state_for_control_command(
            &mut state,
            store,
            "VALIDATED",
            &current_stage,
            next_action,
            &event_id,
            None,
        ) {
            return self
                .read_only
                .state_error_envelope("state_update_failed", source);
        }
        if let Err(source) = store.save_state(job_id, &state) {
            return self
                .read_only
                .state_error_envelope("state_write_failed", source);
        }
        if let Err(source) = append_api_event(
            store,
            job_id,
            ApiControlEvent {
                event_id,
                event_type: "STATE_CHANGED",
                state: "VALIDATED",
                stage: &current_stage,
                message: "Approval accepted; job is ready to continue",
                artifact_paths: vec![
                    "run-state.json".to_string(),
                    "approvals/approval-response.json".to_string(),
                ],
                details: json!({ "previous_state": current_state, "next_action": next_action }),
            },
        ) {
            return self
                .read_only
                .state_error_envelope("event_write_failed", source);
        }

        self.read_only.success_envelope(json!({
            "command": "resume",
            "job_id": job_id,
            "state": "VALIDATED",
            "previous_state": current_state,
            "next_action": next_action,
            "resumed": true,
            "artifacts": [
                format!(".ai-runs/{}/run-state.json", job_id),
                format!(".ai-runs/{}/approvals/approval-response.json", job_id)
            ]
        }))
    }

    fn invalid_control_request(&self, message: &str) -> Result<Value, ApiError> {
        self.read_only
            .error_envelope("invalid_control_request", message, json!({}))
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

#[derive(Debug)]
enum ControlArtifactError {
    Missing { path: String },
    ReadFailed { path: PathBuf, message: String },
    InvalidJson { path: PathBuf, message: String },
    SchemaInvalid { schema: String, errors: usize },
    State { source: StateStoreError },
}

impl fmt::Display for ControlArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { path } => write!(formatter, "required artifact not found: {}", path),
            Self::ReadFailed { path, message } => {
                write!(formatter, "failed to read {}: {}", path.display(), message)
            }
            Self::InvalidJson { path, message } => {
                write!(formatter, "invalid JSON at {}: {}", path.display(), message)
            }
            Self::SchemaInvalid { schema, errors } => {
                write!(
                    formatter,
                    "artifact failed schema validation against {} with {} error(s)",
                    schema, errors
                )
            }
            Self::State { source } => write!(formatter, "state store error: {}", source),
        }
    }
}

fn load_job_json(
    store: &StateStore,
    job_id: &str,
    relative_path: &str,
    schema_file: &str,
    schema_root: &Path,
) -> Result<Value, ControlArtifactError> {
    let path = store
        .resolve_job_path(job_id, relative_path)
        .map_err(|source| ControlArtifactError::State { source })?;
    if !path.is_file() {
        return Err(ControlArtifactError::Missing {
            path: format!(".ai-runs/{}/{}", job_id, relative_path),
        });
    }
    let text = fs::read_to_string(&path).map_err(|source| ControlArtifactError::ReadFailed {
        path: path.clone(),
        message: source.to_string(),
    })?;
    let value: Value =
        serde_json::from_str(&text).map_err(|source| ControlArtifactError::InvalidJson {
            path,
            message: source.to_string(),
        })?;
    validate_schema_value(&value, schema_root, schema_file).map_err(|errors| {
        ControlArtifactError::SchemaInvalid {
            schema: schema_file.to_string(),
            errors,
        }
    })?;
    Ok(value)
}

fn validate_schema_value(
    value: &Value,
    schema_root: &Path,
    schema_file: &str,
) -> Result<(), usize> {
    let schema_path = schema_root.join(schema_file);
    let schema = match load_schema(&schema_path) {
        Ok(value) => value,
        Err(_) => return Err(1),
    };
    let result = validate_json(value, &schema);
    if result.is_ok() {
        Ok(())
    } else {
        Err(result.errors.len())
    }
}

fn body_string(body: &Value, field: &str) -> Result<String, String> {
    body.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("{} string field is required", field))
}

fn body_string_array(body: &Value, field: &str) -> Result<Vec<String>, String> {
    let Some(value) = body.get(field) else {
        return Ok(Vec::new());
    };
    let Some(items) = value.as_array() else {
        return Err(format!("{} must be an array of strings", field));
    };
    let mut output = Vec::with_capacity(items.len());
    for item in items {
        let Some(text) = item.as_str() else {
            return Err(format!("{} must be an array of strings", field));
        };
        output.push(text.to_string());
    }
    Ok(output)
}

fn string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn state_string(state: &Value) -> String {
    string_field(state, "state").unwrap_or("FAILED").to_string()
}

fn state_after_approval_response(response: &str) -> &'static str {
    match response {
        "approved" => "WAITING_APPROVAL",
        "cancelled" => "CANCELLED",
        _ => "BLOCKED",
    }
}

fn next_action_after_approval_response(response: &str) -> &'static str {
    match response {
        "approved" => "resume",
        "cancelled" => "stop",
        "needs_changes" => "revise",
        _ => "stop",
    }
}

fn allowed_next_stage_for(stage: &str) -> Option<&'static str> {
    match stage {
        "route" => Some("plan"),
        "plan" => Some("design"),
        "design" => Some("implement"),
        "implement" => Some("validate"),
        "validate" => Some("report"),
        "review" => Some("polish"),
        "polish" => Some("report"),
        _ => None,
    }
}

fn ensure_approval_response_matches_request(
    approval_request: &Value,
    approval_response: &Value,
) -> Result<(), String> {
    for field in ["job_id", "stage", "task_id"] {
        let expected = string_field(approval_request, field)
            .ok_or_else(|| format!("approval request missing {}", field))?;
        let actual = string_field(approval_response, field)
            .ok_or_else(|| format!("approval response missing {}", field))?;
        if expected != actual {
            return Err(format!(
                "approval response {} mismatch: expected {}, got {}",
                field, expected, actual
            ));
        }
    }
    let response = string_field(approval_response, "response")
        .ok_or_else(|| "approval response missing response".to_string())?;
    if response != "approved" {
        return Err(format!(
            "resume requires approved response, got {}",
            response
        ));
    }
    Ok(())
}

fn update_state_for_control_command(
    state: &mut Value,
    store: &StateStore,
    next_state: &str,
    current_stage: &str,
    next_action: &str,
    latest_event_id: &str,
    artifact_ref: Option<(&str, &Value)>,
) -> Result<(), StateStoreError> {
    if let Some(state_object) = state.as_object_mut() {
        state_object.insert("state".to_string(), Value::String(next_state.to_string()));
        state_object.insert(
            "current_stage".to_string(),
            Value::String(current_stage.to_string()),
        );
        state_object.insert("updated_at".to_string(), Value::String(timestamp_string()));
        state_object.insert(
            "latest_event_id".to_string(),
            Value::String(latest_event_id.to_string()),
        );
        state_object.insert(
            "next_action".to_string(),
            Value::String(next_action.to_string()),
        );
        let history = state_object
            .entry("history")
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(history) = history.as_array_mut() {
            history.push(json!({
                "stage": current_stage,
                "state": next_state,
                "next_action": next_action,
                "event_id": latest_event_id
            }));
        } else {
            state_object.insert(
                "history".to_string(),
                json!([{
                    "stage": current_stage,
                    "state": next_state,
                    "next_action": next_action,
                    "event_id": latest_event_id
                }]),
            );
        }
    } else {
        return Err(StateStoreError::InvalidArtifactShape {
            message: "RunState must be a JSON object".to_string(),
        });
    }
    if let Some((key, artifact_ref)) = artifact_ref {
        store.register_artifact_ref(state, key, artifact_ref)?;
    }
    Ok(())
}

struct ApiControlEvent<'a> {
    event_id: String,
    event_type: &'a str,
    state: &'a str,
    stage: &'a str,
    message: &'a str,
    artifact_paths: Vec<String>,
    details: Value,
}

fn append_api_event(
    store: &StateStore,
    job_id: &str,
    event: ApiControlEvent<'_>,
) -> Result<(), StateStoreError> {
    store.append_event(
        job_id,
        &json!({
            "schema_version": SCHEMA_VERSION,
            "event_id": event.event_id,
            "job_id": job_id,
            "type": event.event_type,
            "created_at": timestamp_string(),
            "stage": event.stage,
            "state": event.state,
            "message": event.message,
            "artifact_paths": event.artifact_paths,
            "details": event.details
        }),
    )
}

fn timestamp_string() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("unix:{}", nanos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use star_control_daemon::{DaemonConfig, DaemonQueue};
    use star_control_release::ReleaseReadinessWriter;
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

    fn control_with_store(store: StateStore) -> ApiControlService {
        let mut service = ApiControlService::new(schema_root());
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

    fn approval_request(job_id: &str, stage: &str) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": job_id,
            "stage": stage,
            "task_id": "approval-1",
            "decision": "HUMAN_REVIEW",
            "reasons": ["test approval"],
            "changed_files": ["src/lib.rs"],
            "risks": ["requires human review"],
            "diagnostics": [],
            "review_pack_path": "review-packs/review_pack.md",
            "requested_at": "unix:1",
            "requested_by": "star-control-test"
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

        let missing_readiness = service
            .handle_get("/projects/local/jobs/J-0001/release-readiness")
            .expect("missing release readiness response");
        assert_eq!(missing_readiness["status"], "failed");
        assert_eq!(
            missing_readiness["error"]["code"],
            "release_readiness_not_found"
        );
        assert_eq!(
            missing_readiness["error"]["details"]["artifact_path"],
            ".ai-runs/J-0001/release/release-readiness.json"
        );

        let missing_job_readiness = service
            .handle_get("/projects/local/jobs/J-9999/release-readiness")
            .expect("missing job release readiness response");
        assert_eq!(missing_job_readiness["status"], "failed");
        assert_eq!(missing_job_readiness["error"]["code"], "job_read_failed");

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn release_readiness_endpoint_reads_schema_valid_artifact_without_mutation() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store, "DONE", "report");
        let writer = ReleaseReadinessWriter::new(schema_root());
        let readiness = writer.reserved("release-0001", "star-control", "0.0.0-dev");
        writer
            .write(&store, "J-0001", &readiness)
            .expect("write release readiness");
        let state_path = project.join(".ai-runs/J-0001/run-state.json");
        let before_state = fs::read_to_string(&state_path).expect("read state before");
        let service = api_with_store(store);

        let response = service
            .handle_get("/projects/local/jobs/J-0001/release-readiness")
            .expect("release readiness response");

        assert_eq!(response["status"], "success");
        assert_eq!(response["data"]["project_id"], "local");
        assert_eq!(response["data"]["job_id"], "J-0001");
        assert_eq!(
            response["data"]["readiness_path"],
            ".ai-runs/J-0001/release/release-readiness.json"
        );
        assert_eq!(response["data"]["readiness"]["status"], "reserved");
        assert_eq!(
            response["data"]["readiness"]["blockers"][0],
            "release automation is not implemented yet"
        );
        let after_state = fs::read_to_string(&state_path).expect("read state after");
        assert_eq!(after_state, before_state);
        assert!(!project.join(".ai-runs/J-0001/api-response.json").exists());

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
    fn control_approve_and_resume_match_cli_gate() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store, "WAITING_APPROVAL", "validate");
        store
            .write_approval_json(
                "J-0001",
                "approval-request.json",
                &approval_request("J-0001", "validate"),
            )
            .expect("write approval request");
        let service = control_with_store(store.clone());

        let approve = service
            .handle_post(
                "/projects/local/jobs/J-0001/approve",
                json!({
                    "response": "approved",
                    "reason": "approved by API test",
                    "constraints": ["keep schema stable"]
                }),
            )
            .expect("approve response");
        assert_eq!(approve["status"], "success");
        assert_eq!(approve["data"]["command"], "approve");
        assert_eq!(approve["data"]["state"], "WAITING_APPROVAL");
        assert_eq!(approve["data"]["approval_response"], "approved");
        assert_eq!(approve["data"]["allowed_next_stage"], "report");
        assert!(project
            .join(".ai-runs/J-0001/approvals/approval-response.json")
            .is_file());

        let approved_state = store.load_state("J-0001").expect("state after approve");
        assert_eq!(approved_state["state"], "WAITING_APPROVAL");
        assert_eq!(approved_state["next_action"], "resume");
        assert_eq!(
            approved_state["artifacts"]["approval_response"]["path"],
            "approvals/approval-response.json"
        );

        let resume = service
            .handle_post("/projects/local/jobs/J-0001/resume", json!({}))
            .expect("resume response");
        assert_eq!(resume["status"], "success");
        assert_eq!(resume["data"]["command"], "resume");
        assert_eq!(resume["data"]["previous_state"], "WAITING_APPROVAL");
        assert_eq!(resume["data"]["state"], "VALIDATED");
        assert_eq!(resume["data"]["next_action"], "report");

        let resumed_state = store.load_state("J-0001").expect("state after resume");
        assert_eq!(resumed_state["state"], "VALIDATED");
        assert_eq!(resumed_state["next_action"], "report");
        let events = store.read_events("J-0001").expect("events");
        assert!(events.iter().any(|event| {
            event["type"] == "APPROVAL_RECORDED" && event["state"] == "WAITING_APPROVAL"
        }));
        assert!(events
            .iter()
            .any(|event| { event["type"] == "STATE_CHANGED" && event["state"] == "VALIDATED" }));

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn control_cancel_updates_nonterminal_and_rejects_terminal() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store, "ROUTED", "implement");
        let service = control_with_store(store.clone());

        let cancel = service
            .handle_post("/projects/local/jobs/J-0001/cancel", json!({}))
            .expect("cancel response");
        assert_eq!(cancel["status"], "success");
        assert_eq!(cancel["data"]["command"], "cancel");
        assert_eq!(cancel["data"]["previous_state"], "ROUTED");
        assert_eq!(cancel["data"]["state"], "CANCELLED");
        assert_eq!(
            store.load_state("J-0001").expect("cancelled state")["state"],
            "CANCELLED"
        );

        let second_cancel = service
            .handle_post("/projects/local/jobs/J-0001/cancel", json!({}))
            .expect("second cancel response");
        assert_eq!(second_cancel["status"], "failed");
        assert_eq!(second_cancel["error"]["code"], "invalid_control_state");

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn control_requires_approval_request_and_approved_response() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store, "WAITING_APPROVAL", "validate");
        let service = control_with_store(store.clone());

        let missing_request = service
            .handle_post(
                "/projects/local/jobs/J-0001/approve",
                json!({
                    "response": "approved",
                    "reason": "missing request"
                }),
            )
            .expect("missing request response");
        assert_eq!(missing_request["status"], "failed");
        assert_eq!(missing_request["error"]["code"], "approval_request_missing");

        store
            .write_approval_json(
                "J-0001",
                "approval-request.json",
                &approval_request("J-0001", "validate"),
            )
            .expect("write approval request");
        let missing_response = service
            .handle_post("/projects/local/jobs/J-0001/resume", json!({}))
            .expect("missing response");
        assert_eq!(missing_response["status"], "failed");
        assert_eq!(
            missing_response["error"]["code"],
            "approval_response_missing"
        );

        fs::remove_dir_all(project).ok();
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
