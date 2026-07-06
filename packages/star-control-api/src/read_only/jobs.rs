use super::ApiReadOnlyService;
use crate::error::ApiError;
use serde_json::{json, Value};
use star_control_state::JobSummary;

impl ApiReadOnlyService {
    pub(super) fn jobs_response(&self, project_id: &str) -> Result<Value, ApiError> {
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

    pub(super) fn job_response(&self, project_id: &str, job_id: &str) -> Result<Value, ApiError> {
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

    pub(super) fn events_response(
        &self,
        project_id: &str,
        job_id: &str,
    ) -> Result<Value, ApiError> {
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
