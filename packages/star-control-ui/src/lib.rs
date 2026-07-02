use serde_json::{json, Value};
use star_control_api::{ApiControlService, ApiError, ApiReadOnlyService};
use star_control_schema::{load_schema, validate_json, ValidationError};
use star_control_security::redact_value;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

const SCHEMA_VERSION: &str = "1.0.0";
const UI_JOB_VIEW_SCHEMA: &str = "ui-job-view.schema.json";
const DEFAULT_REPORT_STAGE: &str = "implement";
const CONTROL_TRANSPORT: &str = "in_process_api_control_service";
const TERMINAL_STATES: &[&str] = &["DONE", "FAILED", "BLOCKED", "CANCELLED"];

#[derive(Debug)]
pub enum UiError {
    Api {
        source: ApiError,
    },
    ApiEnvelopeFailed {
        endpoint: String,
        code: String,
        message: String,
    },
    InvalidApiData {
        endpoint: String,
        message: String,
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

impl fmt::Display for UiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Api { source } => write!(formatter, "UI API error: {}", source),
            Self::ApiEnvelopeFailed {
                endpoint,
                code,
                message,
            } => write!(
                formatter,
                "UI API endpoint {} failed with {}: {}",
                endpoint, code, message
            ),
            Self::InvalidApiData { endpoint, message } => {
                write!(
                    formatter,
                    "invalid UI API data for {}: {}",
                    endpoint, message
                )
            }
            Self::SchemaLoadFailed { path, message } => {
                write!(
                    formatter,
                    "UI schema load failed at {}: {}",
                    path.display(),
                    message
                )
            }
            Self::SchemaValidationFailed { path, errors } => {
                write!(
                    formatter,
                    "UI schema validation failed for {} with {} error(s)",
                    path.display(),
                    errors.len()
                )
            }
        }
    }
}

impl Error for UiError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Api { source } => Some(source),
            _ => None,
        }
    }
}

impl From<ApiError> for UiError {
    fn from(source: ApiError) -> Self {
        Self::Api { source }
    }
}

#[derive(Debug, Clone)]
pub struct UiReadOnlyShell {
    schema_root: PathBuf,
    api: ApiReadOnlyService,
}

impl UiReadOnlyShell {
    pub fn new(schema_root: impl Into<PathBuf>, api: ApiReadOnlyService) -> Self {
        Self {
            schema_root: schema_root.into(),
            api,
        }
    }

    pub fn job_list(&self, project_id: &str) -> Result<Value, UiError> {
        let endpoint = format!("/projects/{}/jobs", project_id);
        let response = self.api_get(&endpoint)?;
        let data = self.data_or_error(response, &endpoint)?;
        let jobs = data
            .get("jobs")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_data(&endpoint, "jobs array is missing"))?;
        let mut views = Vec::with_capacity(jobs.len());
        for summary in jobs {
            let view = job_summary_view(summary)?;
            self.validate_job_view(&view)?;
            views.push(view);
        }

        Ok(redact_value(json!({
            "schema_version": SCHEMA_VERSION,
            "view": "job_list",
            "read_only": true,
            "mutations_enabled": false,
            "project_id": project_id,
            "project_root": data.get("project_root").cloned().unwrap_or(Value::Null),
            "jobs": views
        })))
    }

    pub fn job_detail(&self, project_id: &str, job_id: &str) -> Result<Value, UiError> {
        let endpoint = format!("/projects/{}/jobs/{}", project_id, job_id);
        let response = self.api_get(&endpoint)?;
        let detail_data = self.data_or_error(response, &endpoint)?;
        let state = detail_data
            .get("state")
            .ok_or_else(|| invalid_data(&endpoint, "state object is missing"))?;
        let job = detail_data
            .get("job")
            .ok_or_else(|| invalid_data(&endpoint, "job object is missing"))?;
        let latest_event = detail_data.get("latest_event").unwrap_or(&Value::Null);
        let job_view = job_detail_view(job, state, latest_event)?;
        self.validate_job_view(&job_view)?;

        let events = self.events(project_id, job_id)?;
        let report_stage = state
            .get("current_stage")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_REPORT_STAGE);
        let report = self.report(project_id, job_id, report_stage)?;
        let release_readiness = self.release_readiness(project_id, job_id)?;
        let artifact_sections = artifact_sections(state.get("artifacts").unwrap_or(&Value::Null));
        let approval = approval_summary(&job_view, &artifact_sections);

        Ok(redact_value(json!({
            "schema_version": SCHEMA_VERSION,
            "view": "job_detail",
            "read_only": true,
            "mutations_enabled": false,
            "mutation_surface": "api_or_cli",
            "project_id": project_id,
            "job": job_view,
            "long_running": {
                "job_id": job_id,
                "state": string_field(state, "state").unwrap_or("UNKNOWN"),
                "current_stage": string_field(state, "current_stage").unwrap_or("unknown"),
                "active_provider": state.get("active_provider").cloned().unwrap_or(Value::Null),
                "latest_event": latest_event_id(latest_event, state).map(Value::String).unwrap_or(Value::Null),
                "elapsed_time": Value::Null,
                "approval_required": state_is_waiting_approval(state),
                "blocked_reason": report
                    .get("report")
                    .and_then(|value| value.get("blocked_reason"))
                    .cloned()
                    .unwrap_or(Value::Null),
                "next_action": string_field(state, "next_action").unwrap_or("inspect")
            },
            "timeline": {
                "events": events
            },
            "provider_output_viewer": {
                "paths": paths_for_section(&artifact_sections, "provider_output")
            },
            "validation_result_viewer": {
                "paths": paths_for_section(&artifact_sections, "validation")
            },
            "approval_request_viewer": approval,
            "review_pack_viewer": {
                "paths": paths_for_section(&artifact_sections, "review_pack")
            },
            "release_readiness_viewer": release_readiness,
            "artifact_sections": artifact_sections,
            "report_summary": report
        })))
    }

    pub fn release_readiness(&self, project_id: &str, job_id: &str) -> Result<Value, UiError> {
        let endpoint = format!("/projects/{}/jobs/{}/release-readiness", project_id, job_id);
        let response = self.api_get(&endpoint)?;
        if response.get("status").and_then(Value::as_str) == Some("failed") {
            return Ok(redact_value(json!({
                "available": false,
                "read_only": true,
                "mutations_enabled": false,
                "release_actions_enabled": false,
                "error": response.get("error").cloned().unwrap_or(Value::Null)
            })));
        }
        let data = self.data_or_error(response, &endpoint)?;
        let readiness = data
            .get("readiness")
            .ok_or_else(|| invalid_data(&endpoint, "readiness object is missing"))?;

        Ok(redact_value(json!({
            "available": true,
            "read_only": true,
            "mutations_enabled": false,
            "release_actions_enabled": false,
            "readiness_path": data.get("readiness_path").cloned().unwrap_or(Value::Null),
            "release_id": readiness.get("release_id").cloned().unwrap_or(Value::Null),
            "target": readiness.get("target").cloned().unwrap_or(Value::Null),
            "version": readiness.get("version").cloned().unwrap_or(Value::Null),
            "status": readiness.get("status").cloned().unwrap_or(Value::Null),
            "checks": readiness.get("checks").cloned().unwrap_or_else(|| json!([])),
            "blockers": readiness.get("blockers").cloned().unwrap_or_else(|| json!([])),
            "approvals": readiness.get("approvals").cloned().unwrap_or_else(|| json!([])),
            "generated_at": readiness.get("generated_at").cloned().unwrap_or(Value::Null)
        })))
    }

    pub fn validate_job_view(&self, view: &Value) -> Result<(), UiError> {
        let schema_path = self.schema_root.join(UI_JOB_VIEW_SCHEMA);
        let schema = load_schema(&schema_path).map_err(|source| UiError::SchemaLoadFailed {
            path: schema_path.clone(),
            message: source.to_string(),
        })?;
        let result = validate_json(view, &schema);
        if result.is_ok() {
            Ok(())
        } else {
            Err(UiError::SchemaValidationFailed {
                path: PathBuf::from(UI_JOB_VIEW_SCHEMA),
                errors: result.errors,
            })
        }
    }

    fn api_get(&self, endpoint: &str) -> Result<Value, UiError> {
        Ok(self.api.handle_get(endpoint)?)
    }

    fn data_or_error(&self, response: Value, endpoint: &str) -> Result<Value, UiError> {
        if response.get("status").and_then(Value::as_str) == Some("failed") {
            return Err(UiError::ApiEnvelopeFailed {
                endpoint: endpoint.to_string(),
                code: response
                    .get("error")
                    .and_then(|value| value.get("code"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                message: response
                    .get("error")
                    .and_then(|value| value.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("read-only API request failed")
                    .to_string(),
            });
        }
        let data = response
            .get("data")
            .cloned()
            .ok_or_else(|| invalid_data(endpoint, "data object is missing"))?;
        if data.is_object() {
            Ok(data)
        } else {
            Err(invalid_data(endpoint, "data is not an object"))
        }
    }

    fn events(&self, project_id: &str, job_id: &str) -> Result<Vec<Value>, UiError> {
        let endpoint = format!("/projects/{}/jobs/{}/events", project_id, job_id);
        let response = self.api_get(&endpoint)?;
        let data = self.data_or_error(response, &endpoint)?;
        let events = data
            .get("events")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_data(&endpoint, "events array is missing"))?;
        Ok(events.iter().cloned().map(redact_value).collect())
    }

    fn report(&self, project_id: &str, job_id: &str, stage: &str) -> Result<Value, UiError> {
        let endpoint = format!(
            "/projects/{}/jobs/{}/report?stage={}",
            project_id, job_id, stage
        );
        let response = self.api_get(&endpoint)?;
        if response.get("status").and_then(Value::as_str) == Some("failed") {
            return Ok(json!({
                "available": false,
                "stage": stage,
                "error": response.get("error").cloned().unwrap_or(Value::Null)
            }));
        }
        let data = self.data_or_error(response, &endpoint)?;
        Ok(json!({
            "available": true,
            "stage": stage,
            "report_path": data.get("report_path").cloned().unwrap_or(Value::Null),
            "report": data.get("report").cloned().unwrap_or(Value::Null)
        }))
    }
}

#[derive(Debug, Clone)]
pub struct UiBrowserShell {
    schema_root: PathBuf,
    api: ApiControlService,
}

impl UiBrowserShell {
    pub fn new(schema_root: impl Into<PathBuf>, api: ApiControlService) -> Self {
        Self {
            schema_root: schema_root.into(),
            api,
        }
    }

    pub fn action_panel(&self, project_id: &str, job_id: &str) -> Result<Value, UiError> {
        let endpoint = format!("/projects/{}/jobs/{}", project_id, job_id);
        let response = self.api.handle_get(&endpoint)?;
        let detail_data = data_or_error(response, &endpoint)?;
        let state = detail_data
            .get("state")
            .ok_or_else(|| invalid_data(&endpoint, "state object is missing"))?;
        let job = detail_data
            .get("job")
            .ok_or_else(|| invalid_data(&endpoint, "job object is missing"))?;
        let latest_event = detail_data.get("latest_event").unwrap_or(&Value::Null);
        let job_view = job_detail_view(job, state, latest_event)?;
        self.validate_job_view(&job_view)?;

        Ok(redact_value(json!({
            "schema_version": SCHEMA_VERSION,
            "view": "browser_control_shell",
            "render_target": "browser",
            "runtime": "library_model",
            "project_id": project_id,
            "job": job_view,
            "mutation_surface": "api_control_service",
            "transport": CONTROL_TRANSPORT,
            "mutations_enabled": true,
            "network_server_enabled": false,
            "package_manager_required": false,
            "actions": control_actions(project_id, job_id, state),
            "reserved": {
                "browser_app": true,
                "http_server": true,
                "remote_exposure": true,
                "auth_session": true
            }
        })))
    }

    pub fn approve(
        &self,
        project_id: &str,
        job_id: &str,
        response: &str,
        reason: &str,
        constraints: Vec<String>,
    ) -> Result<Value, UiError> {
        self.control_action(
            project_id,
            job_id,
            "approve",
            json!({
                "response": response,
                "reason": reason,
                "constraints": constraints
            }),
        )
    }

    pub fn cancel(&self, project_id: &str, job_id: &str) -> Result<Value, UiError> {
        self.control_action(project_id, job_id, "cancel", json!({}))
    }

    pub fn resume(&self, project_id: &str, job_id: &str) -> Result<Value, UiError> {
        self.control_action(project_id, job_id, "resume", json!({}))
    }

    pub fn validate_job_view(&self, view: &Value) -> Result<(), UiError> {
        validate_job_view_at(&self.schema_root, view)
    }

    fn control_action(
        &self,
        project_id: &str,
        job_id: &str,
        command: &str,
        body: Value,
    ) -> Result<Value, UiError> {
        let endpoint = format!("/projects/{}/jobs/{}/{}", project_id, job_id, command);
        let response = self.api.handle_post(&endpoint, body)?;
        let status = response
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("failed");
        Ok(redact_value(json!({
            "schema_version": SCHEMA_VERSION,
            "view": "browser_control_result",
            "render_target": "browser",
            "runtime": "library_model",
            "project_id": project_id,
            "job_id": job_id,
            "command": command,
            "endpoint": endpoint,
            "mutation_surface": "api_control_service",
            "transport": CONTROL_TRANSPORT,
            "succeeded": status != "failed",
            "status": status,
            "api_response": response
        })))
    }
}

fn invalid_data(endpoint: &str, message: &str) -> UiError {
    UiError::InvalidApiData {
        endpoint: endpoint.to_string(),
        message: message.to_string(),
    }
}

fn data_or_error(response: Value, endpoint: &str) -> Result<Value, UiError> {
    if response.get("status").and_then(Value::as_str) == Some("failed") {
        return Err(UiError::ApiEnvelopeFailed {
            endpoint: endpoint.to_string(),
            code: response
                .get("error")
                .and_then(|value| value.get("code"))
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
            message: response
                .get("error")
                .and_then(|value| value.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("API request failed")
                .to_string(),
        });
    }
    let data = response
        .get("data")
        .cloned()
        .ok_or_else(|| invalid_data(endpoint, "data object is missing"))?;
    if data.is_object() {
        Ok(data)
    } else {
        Err(invalid_data(endpoint, "data is not an object"))
    }
}

fn validate_job_view_at(schema_root: &std::path::Path, view: &Value) -> Result<(), UiError> {
    let schema_path = schema_root.join(UI_JOB_VIEW_SCHEMA);
    let schema = load_schema(&schema_path).map_err(|source| UiError::SchemaLoadFailed {
        path: schema_path.clone(),
        message: source.to_string(),
    })?;
    let result = validate_json(view, &schema);
    if result.is_ok() {
        Ok(())
    } else {
        Err(UiError::SchemaValidationFailed {
            path: PathBuf::from(UI_JOB_VIEW_SCHEMA),
            errors: result.errors,
        })
    }
}

fn job_summary_view(summary: &Value) -> Result<Value, UiError> {
    let endpoint = "job summary";
    let job_id = string_field(summary, "job_id")
        .ok_or_else(|| invalid_data(endpoint, "job_id is missing"))?;
    let state = string_field(summary, "state").unwrap_or("UNKNOWN");
    let current_stage = string_field(summary, "current_stage").unwrap_or("unknown");
    let title = string_field(summary, "summary")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(job_id);
    Ok(json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id,
        "title": title,
        "state": state,
        "current_stage": current_stage,
        "approval_required": state == "WAITING_APPROVAL",
        "next_action": next_action_for_state(state),
        "latest_event": Value::Null,
        "artifacts": []
    }))
}

fn job_detail_view(job: &Value, state: &Value, latest_event: &Value) -> Result<Value, UiError> {
    let endpoint = "job detail";
    let job_id = string_field(state, "job_id")
        .or_else(|| string_field(job, "job_id"))
        .ok_or_else(|| invalid_data(endpoint, "job_id is missing"))?;
    let state_value = string_field(state, "state").unwrap_or("UNKNOWN");
    let current_stage = string_field(state, "current_stage").unwrap_or("unknown");
    let next_action = string_field(state, "next_action").unwrap_or_else(|| {
        if state_value == "WAITING_APPROVAL" {
            "approve"
        } else {
            next_action_for_state(state_value)
        }
    });
    let title = string_field(job, "request_text")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(job_id);
    let paths = artifact_paths(state.get("artifacts").unwrap_or(&Value::Null));
    Ok(json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id,
        "title": title,
        "state": state_value,
        "current_stage": current_stage,
        "approval_required": state_is_waiting_approval(state) || next_action == "approve",
        "next_action": next_action,
        "latest_event": latest_event_id(latest_event, state).map(Value::String).unwrap_or(Value::Null),
        "artifacts": paths
    }))
}

fn control_actions(project_id: &str, job_id: &str, state: &Value) -> Vec<Value> {
    let state_value = string_field(state, "state").unwrap_or("UNKNOWN");
    let next_action = string_field(state, "next_action").unwrap_or_else(|| {
        if state_value == "WAITING_APPROVAL" {
            "approve"
        } else {
            next_action_for_state(state_value)
        }
    });
    let terminal = TERMINAL_STATES.contains(&state_value);
    let waiting_approval = state_value == "WAITING_APPROVAL";

    vec![
        json!({
            "id": "approve",
            "label": "Approve",
            "method": "POST",
            "endpoint": format!("/projects/{}/jobs/{}/approve", project_id, job_id),
            "transport": CONTROL_TRANSPORT,
            "enabled": waiting_approval && next_action == "approve",
            "disabled_reason": disabled_reason(waiting_approval && next_action == "approve", "approval response already recorded or job is not waiting for approval"),
            "body_contract": "approval-response.schema.json",
            "response_options": ["approved", "rejected", "needs_changes", "cancelled"],
            "required_fields": ["response", "reason"]
        }),
        json!({
            "id": "cancel",
            "label": "Cancel",
            "method": "POST",
            "endpoint": format!("/projects/{}/jobs/{}/cancel", project_id, job_id),
            "transport": CONTROL_TRANSPORT,
            "enabled": !terminal,
            "disabled_reason": disabled_reason(!terminal, "terminal job cannot be cancelled"),
            "body_contract": Value::Null,
            "response_options": [],
            "required_fields": []
        }),
        json!({
            "id": "resume",
            "label": "Resume",
            "method": "POST",
            "endpoint": format!("/projects/{}/jobs/{}/resume", project_id, job_id),
            "transport": CONTROL_TRANSPORT,
            "enabled": waiting_approval && next_action == "resume",
            "disabled_reason": disabled_reason(waiting_approval && next_action == "resume", "resume requires an approved approval response"),
            "body_contract": Value::Null,
            "response_options": [],
            "required_fields": []
        }),
    ]
}

fn disabled_reason(enabled: bool, reason: &str) -> Value {
    if enabled {
        Value::Null
    } else {
        Value::String(reason.to_string())
    }
}

fn string_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn state_is_waiting_approval(state: &Value) -> bool {
    string_field(state, "state") == Some("WAITING_APPROVAL")
}

fn latest_event_id(latest_event: &Value, state: &Value) -> Option<String> {
    latest_event
        .get("event_id")
        .and_then(Value::as_str)
        .or_else(|| state.get("latest_event_id").and_then(Value::as_str))
        .map(str::to_string)
}

fn next_action_for_state(state: &str) -> &'static str {
    match state {
        "WAITING_APPROVAL" => "approve",
        "DONE" | "FAILED" | "BLOCKED" | "CANCELLED" => "none",
        "UNKNOWN" => "inspect",
        _ => "inspect",
    }
}

fn artifact_sections(artifacts: &Value) -> Vec<Value> {
    let mut sections = Vec::new();
    if let Some(object) = artifacts.as_object() {
        for (section, value) in object {
            let paths = artifact_paths(value);
            if !paths.is_empty() {
                sections.push(json!({
                    "section": normalize_section(section, &paths),
                    "source_key": section,
                    "paths": paths
                }));
            }
        }
    }
    sections
}

fn normalize_section(section: &str, paths: &[String]) -> String {
    if section.contains("provider") || paths.iter().any(|path| path.contains("provider-output/")) {
        "provider_output".to_string()
    } else if section.contains("validation")
        || paths.iter().any(|path| path.contains("validation/"))
    {
        "validation".to_string()
    } else if section.contains("approval") || paths.iter().any(|path| path.contains("approvals/")) {
        "approval_request".to_string()
    } else if section.contains("review") || paths.iter().any(|path| path.contains("review-packs/"))
    {
        "review_pack".to_string()
    } else {
        section.to_string()
    }
}

fn artifact_paths(artifacts: &Value) -> Vec<String> {
    let mut paths = Vec::new();
    collect_artifact_paths(artifacts, &mut paths);
    paths.sort();
    paths.dedup();
    paths
}

fn collect_artifact_paths(value: &Value, paths: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            if let Some(path) = object.get("path").and_then(Value::as_str) {
                paths.push(path.to_string());
            }
            for value in object.values() {
                collect_artifact_paths(value, paths);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_artifact_paths(item, paths);
            }
        }
        _ => {}
    }
}

fn paths_for_section(sections: &[Value], section_name: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for section in sections {
        if section.get("section").and_then(Value::as_str) == Some(section_name) {
            if let Some(section_paths) = section.get("paths").and_then(Value::as_array) {
                paths.extend(
                    section_paths
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string),
                );
            }
        }
    }
    paths
}

fn approval_summary(job_view: &Value, sections: &[Value]) -> Value {
    let required = job_view
        .get("approval_required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let request_paths = paths_for_section(sections, "approval_request");
    json!({
        "required": required,
        "paths": request_paths,
        "response_contract": "approval-response.schema.json",
        "mutation_surface": "api_or_cli",
        "mutations_enabled": false,
        "actions": ["approved", "rejected", "needs_changes", "cancelled"]
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
            "star-control-ui-{}-{}-{}",
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

    fn ui_with_store(store: StateStore) -> UiReadOnlyShell {
        let mut api = ApiReadOnlyService::new(schema_root());
        api.register_project_store("local", store)
            .expect("register project");
        UiReadOnlyShell::new(schema_root(), api)
    }

    fn browser_with_store(store: StateStore) -> UiBrowserShell {
        let mut api = ApiControlService::new(schema_root());
        api.register_project_store("local", store)
            .expect("register project");
        UiBrowserShell::new(schema_root(), api)
    }

    fn create_job(store: &StateStore, state: &str, stage: &str, next_action: &str) {
        let mut job = store
            .create_job("Core schema contract update", ".", Vec::new())
            .expect("create job");
        job["state"] = json!(state);
        store.save_job("J-0001", &job).expect("save job");

        store
            .save_state(
                "J-0001",
                &json!({
                    "schema_version": SCHEMA_VERSION,
                    "job_id": "J-0001",
                    "state": state,
                    "current_stage": stage,
                    "updated_at": "unix:2",
                    "workers": {},
                    "artifacts": {
                        "provider_output": {
                            "path": "provider-output/fake-default/output.json",
                            "kind": "provider_output"
                        },
                        "validation": {
                            "path": "validation/validation-decision.json",
                            "kind": "other"
                        },
                        "approval_request": {
                            "path": "approvals/approval-request.json",
                            "kind": "approval"
                        },
                        "review_pack": {
                            "path": "review-packs/review_pack.md",
                            "kind": "review_pack"
                        }
                    },
                    "latest_event_id": "J-0001-0002",
                    "active_provider": "fake-default",
                    "next_action": next_action
                }),
            )
            .expect("save state");
        store
            .append_event(
                "J-0001",
                &json!({
                    "schema_version": SCHEMA_VERSION,
                    "event_id": "J-0001-0002",
                    "job_id": "J-0001",
                    "type": "APPROVAL_REQUESTED",
                    "created_at": "unix:2",
                    "stage": stage,
                    "state": state,
                    "message": "Approval requested",
                    "artifact_paths": ["approvals/approval-request.json"],
                    "details": {}
                }),
            )
            .expect("append event");
    }

    fn save_report(store: &StateStore, stage: &str, risks: Vec<&str>) {
        store
            .save_report(
                "J-0001",
                &format!("{}-report", stage),
                &json!({
                    "schema_version": SCHEMA_VERSION,
                    "job_id": "J-0001",
                    "stage": stage,
                    "status": "NEEDS_APPROVAL",
                    "changed_files": ["src/lib.rs"],
                    "commands_run": [],
                    "validation": [],
                    "risks": risks,
                    "blocked_reason": Value::Null,
                    "next_step": "approve",
                    "artifacts": ["approvals/approval-request.json"]
                }),
            )
            .expect("save report");
    }

    fn write_approval_request(store: &StateStore, stage: &str) {
        store
            .write_approval_json(
                "J-0001",
                "approval-request.json",
                &json!({
                    "schema_version": SCHEMA_VERSION,
                    "job_id": "J-0001",
                    "stage": stage,
                    "task_id": format!("{}-approval", stage),
                    "decision": "HUMAN_REVIEW",
                    "reasons": ["API control mutation requires human approval"],
                    "changed_files": ["src/lib.rs"],
                    "risks": [],
                    "diagnostics": [],
                    "review_pack_path": "review-packs/review_pack.md",
                    "requested_at": "unix:2",
                    "requested_by": "star-control-ui-test"
                }),
            )
            .expect("write approval request");
    }

    fn write_release_readiness(project: &Path) -> PathBuf {
        let path = project.join(".ai-runs/J-0001/release/release-readiness.json");
        fs::create_dir_all(path.parent().expect("release dir")).expect("create release dir");
        fs::write(
            &path,
            serde_json::to_vec_pretty(&json!({
                "schema_version": SCHEMA_VERSION,
                "release_id": "release-0007",
                "target": "star-control",
                "version": "1.2.3",
                "status": "reserved",
                "checks": [
                    {
                        "name": "release-profile-passed",
                        "status": "pass",
                        "evidence_paths": ["review-packs/release-profile.json"]
                    },
                    {
                        "name": "version-consistent",
                        "status": "pass",
                        "evidence_paths": ["VERSION"]
                    }
                ],
                "blockers": [
                    "release approval/signing/publish/deploy automation remains reserved"
                ],
                "approvals": [],
                "generated_at": "unix:7"
            }))
            .expect("release readiness JSON"),
        )
        .expect("write release readiness");
        path
    }

    #[test]
    fn job_list_builds_schema_valid_views_from_api() {
        let project = temp_project("list");
        let store = open_store(&project);
        create_job(&store, "WAITING_APPROVAL", "validate", "approve");
        let ui = ui_with_store(store);

        let view = ui.job_list("local").expect("job list");
        assert_eq!(view["view"], "job_list");
        assert_eq!(view["read_only"], true);
        assert_eq!(view["mutations_enabled"], false);
        let job = &view["jobs"][0];
        assert_eq!(job["job_id"], "J-0001");
        assert_eq!(job["approval_required"], true);
        assert_eq!(job["next_action"], "approve");
        ui.validate_job_view(job).expect("schema-valid job view");

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn job_detail_includes_timeline_report_and_artifacts_without_writes() {
        let project = temp_project("detail");
        let store = open_store(&project);
        create_job(&store, "WAITING_APPROVAL", "validate", "approve");
        save_report(&store, "validate", Vec::new());
        let state_path = project.join(".ai-runs/J-0001/run-state.json");
        let before_state = fs::read_to_string(&state_path).expect("read state before");
        let ui = ui_with_store(store);

        let view = ui.job_detail("local", "J-0001").expect("job detail");
        assert_eq!(view["view"], "job_detail");
        assert_eq!(view["read_only"], true);
        assert_eq!(view["job"]["latest_event"], "J-0001-0002");
        assert!(view["timeline"]["events"].as_array().expect("events").len() >= 2);
        assert_eq!(view["report_summary"]["available"], true);
        assert_eq!(view["release_readiness_viewer"]["available"], false);
        assert_eq!(
            view["release_readiness_viewer"]["error"]["code"],
            "release_readiness_not_found"
        );
        assert_eq!(
            view["provider_output_viewer"]["paths"][0],
            "provider-output/fake-default/output.json"
        );
        assert_eq!(
            view["validation_result_viewer"]["paths"][0],
            "validation/validation-decision.json"
        );
        assert_eq!(
            view["review_pack_viewer"]["paths"][0],
            "review-packs/review_pack.md"
        );

        let after_state = fs::read_to_string(&state_path).expect("read state after");
        assert_eq!(after_state, before_state);
        assert!(!project.join(".ai-runs/J-0001/ui-view.json").exists());

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn release_readiness_viewer_reads_api_artifact_without_mutation() {
        let project = temp_project("release-readiness");
        let store = open_store(&project);
        create_job(&store, "DONE", "report", "none");
        save_report(&store, "report", Vec::new());
        let readiness_path = write_release_readiness(&project);
        let before_readiness = fs::read_to_string(&readiness_path).expect("read readiness before");
        let ui = ui_with_store(store);

        let readiness = ui
            .release_readiness("local", "J-0001")
            .expect("release readiness view");
        assert_eq!(readiness["available"], true);
        assert_eq!(readiness["read_only"], true);
        assert_eq!(readiness["mutations_enabled"], false);
        assert_eq!(readiness["release_actions_enabled"], false);
        assert_eq!(
            readiness["readiness_path"],
            ".ai-runs/J-0001/release/release-readiness.json"
        );
        assert_eq!(readiness["status"], "reserved");
        assert_eq!(readiness["checks"][0]["name"], "release-profile-passed");
        assert_eq!(
            readiness["blockers"][0],
            "release approval/signing/publish/deploy automation remains reserved"
        );

        let detail = ui.job_detail("local", "J-0001").expect("job detail");
        assert_eq!(detail["release_readiness_viewer"], readiness);
        let after_readiness = fs::read_to_string(&readiness_path).expect("read readiness after");
        assert_eq!(after_readiness, before_readiness);
        assert!(!project
            .join(".ai-runs/J-0001/release/release-action.json")
            .exists());

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn waiting_approval_view_exposes_approval_path_without_mutation() {
        let project = temp_project("approval");
        let store = open_store(&project);
        create_job(&store, "WAITING_APPROVAL", "validate", "approve");
        save_report(&store, "validate", Vec::new());
        let ui = ui_with_store(store);

        let view = ui.job_detail("local", "J-0001").expect("job detail");
        let approval = &view["approval_request_viewer"];
        assert_eq!(approval["required"], true);
        assert_eq!(approval["mutations_enabled"], false);
        assert_eq!(approval["mutation_surface"], "api_or_cli");
        assert_eq!(
            approval["response_contract"],
            "approval-response.schema.json"
        );
        assert_eq!(approval["paths"][0], "approvals/approval-request.json");
        assert!(!project
            .join(".ai-runs/J-0001/approvals/approval-response.json")
            .exists());

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn ui_view_model_redacts_secret_like_values() {
        let project = temp_project("redact");
        let store = open_store(&project);
        create_job(&store, "WAITING_APPROVAL", "validate", "approve");
        save_report(
            &store,
            "validate",
            vec!["Authorization: Bearer sk-test-secret"],
        );
        let ui = ui_with_store(store);

        let view = ui.job_detail("local", "J-0001").expect("job detail");
        let text = serde_json::to_string(&view).expect("view text");
        assert!(!text.contains("sk-test-secret"));
        assert!(text.contains("[REDACTED]"));

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn missing_api_artifact_surfaces_read_only_report_error() {
        let project = temp_project("missing-report");
        let store = open_store(&project);
        create_job(&store, "IMPLEMENTED", "implement", "report");
        let ui = ui_with_store(store);

        let view = ui.job_detail("local", "J-0001").expect("job detail");
        assert_eq!(view["report_summary"]["available"], false);
        assert_eq!(
            view["report_summary"]["error"]["code"],
            "report_read_failed"
        );
        assert_eq!(view["read_only"], true);

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn browser_shell_action_panel_exposes_control_actions_without_network_runtime() {
        let project = temp_project("browser-actions");
        let store = open_store(&project);
        create_job(&store, "WAITING_APPROVAL", "validate", "approve");
        let browser = browser_with_store(store);

        let panel = browser
            .action_panel("local", "J-0001")
            .expect("action panel");
        assert_eq!(panel["view"], "browser_control_shell");
        assert_eq!(panel["render_target"], "browser");
        assert_eq!(panel["runtime"], "library_model");
        assert_eq!(panel["transport"], CONTROL_TRANSPORT);
        assert_eq!(panel["mutations_enabled"], true);
        assert_eq!(panel["network_server_enabled"], false);
        assert_eq!(panel["package_manager_required"], false);
        browser
            .validate_job_view(&panel["job"])
            .expect("schema-valid job view");

        let actions = panel["actions"].as_array().expect("actions");
        let approve = actions
            .iter()
            .find(|action| action["id"] == "approve")
            .expect("approve action");
        let cancel = actions
            .iter()
            .find(|action| action["id"] == "cancel")
            .expect("cancel action");
        let resume = actions
            .iter()
            .find(|action| action["id"] == "resume")
            .expect("resume action");
        assert_eq!(approve["enabled"], true);
        assert_eq!(approve["endpoint"], "/projects/local/jobs/J-0001/approve");
        assert_eq!(cancel["enabled"], true);
        assert_eq!(resume["enabled"], false);
        assert_eq!(
            resume["disabled_reason"],
            "resume requires an approved approval response"
        );

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn browser_shell_approve_then_resume_uses_api_control_service() {
        let project = temp_project("browser-approve-resume");
        let store = open_store(&project);
        create_job(&store, "WAITING_APPROVAL", "validate", "approve");
        write_approval_request(&store, "validate");
        let browser = browser_with_store(store.clone());

        let approve = browser
            .approve(
                "local",
                "J-0001",
                "approved",
                "reviewed in browser shell",
                vec!["keep schema stable".to_string()],
            )
            .expect("approve result");
        assert_eq!(approve["view"], "browser_control_result");
        assert_eq!(approve["command"], "approve");
        assert_eq!(approve["succeeded"], true);
        assert_eq!(approve["api_response"]["data"]["state"], "WAITING_APPROVAL");
        assert_eq!(
            store.load_state("J-0001").expect("state after approve")["next_action"],
            "resume"
        );
        assert!(project
            .join(".ai-runs/J-0001/approvals/approval-response.json")
            .is_file());

        let panel = browser
            .action_panel("local", "J-0001")
            .expect("resume action panel");
        let resume = panel["actions"]
            .as_array()
            .expect("actions")
            .iter()
            .find(|action| action["id"] == "resume")
            .expect("resume action")
            .clone();
        assert_eq!(resume["enabled"], true);

        let resume_result = browser.resume("local", "J-0001").expect("resume result");
        assert_eq!(resume_result["command"], "resume");
        assert_eq!(resume_result["succeeded"], true);
        assert_eq!(resume_result["api_response"]["data"]["state"], "VALIDATED");
        assert_eq!(
            store.load_state("J-0001").expect("state after resume")["state"],
            "VALIDATED"
        );

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn browser_shell_surfaces_terminal_cancel_failure_as_result_view() {
        let project = temp_project("browser-cancel-terminal");
        let store = open_store(&project);
        create_job(&store, "DONE", "report", "none");
        let browser = browser_with_store(store);

        let panel = browser
            .action_panel("local", "J-0001")
            .expect("action panel");
        let cancel = panel["actions"]
            .as_array()
            .expect("actions")
            .iter()
            .find(|action| action["id"] == "cancel")
            .expect("cancel action")
            .clone();
        assert_eq!(cancel["enabled"], false);
        assert_eq!(
            cancel["disabled_reason"],
            "terminal job cannot be cancelled"
        );

        let result = browser.cancel("local", "J-0001").expect("cancel result");
        assert_eq!(result["view"], "browser_control_result");
        assert_eq!(result["command"], "cancel");
        assert_eq!(result["succeeded"], false);
        assert_eq!(result["status"], "failed");
        assert_eq!(
            result["api_response"]["error"]["code"],
            "invalid_control_state"
        );

        fs::remove_dir_all(project).ok();
    }
}
