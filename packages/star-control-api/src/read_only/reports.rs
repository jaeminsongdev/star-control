use super::ApiReadOnlyService;
use crate::constants::CANONICAL_STAGES;
use crate::error::ApiError;
use serde_json::{json, Value};

impl ApiReadOnlyService {
    pub(super) fn report_response(
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
}

fn status_for_report(status: Option<&str>) -> &'static str {
    match status.unwrap_or_default() {
        "NEEDS_APPROVAL" | "NEEDS_REVIEW" => "waiting_approval",
        "BLOCKED" => "blocked",
        "FAILED" => "failed",
        _ => "success",
    }
}
