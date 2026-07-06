use super::ApiReadOnlyService;
use crate::error::ApiError;
use serde_json::{json, Value};
use star_control_release::{ReleaseReadinessError, ReleaseReadinessWriter, RELEASE_READINESS_PATH};

impl ApiReadOnlyService {
    pub(super) fn release_readiness_response(
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
}
