mod daemon;
mod envelope;
mod jobs;
mod projects;
mod release;
mod reports;

use crate::constants::DEFAULT_REPORT_STAGE;
use crate::error::ApiError;
use crate::paths::{validate_project_id, ParsedPath};
use crate::request::{ApiMethod, ApiRequest};
use serde_json::{json, Value};
use star_control_daemon::DaemonQueue;
use star_control_state::StateStore;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ApiReadOnlyService {
    pub(crate) schema_root: PathBuf,
    pub(crate) daemon_queue: Option<DaemonQueue>,
    pub(crate) projects: BTreeMap<String, StateStore>,
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
}
