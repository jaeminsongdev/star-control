use crate::constants::{CONTROL_TRANSPORT, SCHEMA_VERSION};
use crate::control_actions::control_actions;
use crate::error::UiError;
use crate::helpers::{data_or_error, invalid_data, validate_job_view_at};
use crate::view::job_detail_view;
use serde_json::{json, Value};
use star_control_api::ApiControlService;
use star_control_security::redact_value;
use std::path::PathBuf;

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
