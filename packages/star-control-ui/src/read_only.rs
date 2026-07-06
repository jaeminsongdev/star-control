mod api;

use crate::constants::{DEFAULT_REPORT_STAGE, SCHEMA_VERSION};
use crate::error::UiError;
use crate::helpers::{invalid_data, string_field, validate_job_view_at};
use crate::view::{
    approval_summary, artifact_sections, job_detail_view, job_summary_view, latest_event_id,
    paths_for_section, state_is_waiting_approval,
};
use serde_json::{json, Value};
use star_control_api::ApiReadOnlyService;
use star_control_security::redact_value;
use std::path::PathBuf;

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

    pub fn validate_job_view(&self, view: &Value) -> Result<(), UiError> {
        validate_job_view_at(&self.schema_root, view)
    }
}
