use serde_json::{json, Map, Value};
use star_control_api::{ApiError, ApiReadOnlyService};
use star_control_schema::{load_schema, validate_json, ValidationError};
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

const SCHEMA_VERSION: &str = "1.0.0";
const UI_JOB_VIEW_SCHEMA: &str = "ui-job-view.schema.json";
const DEFAULT_REPORT_STAGE: &str = "implement";

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
            Self::Api { source } => write!(formatter, "read-only API error: {}", source),
            Self::ApiEnvelopeFailed {
                endpoint,
                code,
                message,
            } => write!(
                formatter,
                "read-only API endpoint {} failed with {}: {}",
                endpoint, code, message
            ),
            Self::InvalidApiData { endpoint, message } => {
                write!(formatter, "invalid API data for {}: {}", endpoint, message)
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
            let view = self.job_summary_view(summary)?;
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
        let job_view = self.job_detail_view(job, state, latest_event)?;
        self.validate_job_view(&job_view)?;

        let events = self.events(project_id, job_id)?;
        let report_stage = state
            .get("current_stage")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_REPORT_STAGE);
        let report = self.report(project_id, job_id, report_stage)?;
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
            "artifact_sections": artifact_sections,
            "report_summary": report
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

    fn job_summary_view(&self, summary: &Value) -> Result<Value, UiError> {
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

    fn job_detail_view(
        &self,
        job: &Value,
        state: &Value,
        latest_event: &Value,
    ) -> Result<Value, UiError> {
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

fn invalid_data(endpoint: &str, message: &str) -> UiError {
    UiError::InvalidApiData {
        endpoint: endpoint.to_string(),
        message: message.to_string(),
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
}
