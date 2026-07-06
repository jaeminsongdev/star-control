use super::UiReadOnlyShell;
use crate::error::UiError;
use crate::helpers::{data_or_error_with_message, invalid_data};
use serde_json::{json, Value};
use star_control_security::redact_value;

impl UiReadOnlyShell {
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

    pub(super) fn api_get(&self, endpoint: &str) -> Result<Value, UiError> {
        Ok(self.api.handle_get(endpoint)?)
    }

    pub(super) fn data_or_error(&self, response: Value, endpoint: &str) -> Result<Value, UiError> {
        data_or_error_with_message(response, endpoint, "read-only API request failed")
    }

    pub(super) fn events(&self, project_id: &str, job_id: &str) -> Result<Vec<Value>, UiError> {
        let endpoint = format!("/projects/{}/jobs/{}/events", project_id, job_id);
        let response = self.api_get(&endpoint)?;
        let data = self.data_or_error(response, &endpoint)?;
        let events = data
            .get("events")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_data(&endpoint, "events array is missing"))?;
        Ok(events.iter().cloned().map(redact_value).collect())
    }

    pub(super) fn report(
        &self,
        project_id: &str,
        job_id: &str,
        stage: &str,
    ) -> Result<Value, UiError> {
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
