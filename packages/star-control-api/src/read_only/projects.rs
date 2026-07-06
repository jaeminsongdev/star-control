use super::ApiReadOnlyService;
use crate::error::ApiError;
use serde_json::{json, Value};

impl ApiReadOnlyService {
    pub(super) fn projects_response(&self) -> Result<Value, ApiError> {
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

    pub(crate) fn project_not_found(&self, project_id: &str) -> Result<Value, ApiError> {
        self.error_envelope(
            "project_not_found",
            "project is not registered in read-only API",
            json!({ "project_id": project_id }),
        )
    }
}
