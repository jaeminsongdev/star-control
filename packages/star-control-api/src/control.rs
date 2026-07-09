use crate::error::ApiError;
use crate::paths::ParsedPath;
use crate::read_only::ApiReadOnlyService;
use crate::request::{ApiMethod, ApiRequest};
use serde_json::{json, Value};
use star_control_daemon::DaemonQueue;
use star_control_state::StateStore;
use std::path::PathBuf;

mod helpers;
mod mutations;

#[derive(Debug, Clone)]
pub struct ApiControlService {
    pub(crate) read_only: ApiReadOnlyService,
}

impl ApiControlService {
    pub fn new(schema_root: impl Into<PathBuf>) -> Self {
        Self {
            read_only: ApiReadOnlyService::new(schema_root),
        }
    }

    pub fn from_read_only(read_only: ApiReadOnlyService) -> Self {
        Self { read_only }
    }

    pub fn register_daemon_queue(&mut self, daemon_queue: DaemonQueue) {
        self.read_only.register_daemon_queue(daemon_queue);
    }

    pub fn register_config_root(&mut self, config_root: impl Into<PathBuf>) {
        self.read_only.register_config_root(config_root);
    }

    pub fn register_project_store(
        &mut self,
        project_id: impl Into<String>,
        store: StateStore,
    ) -> Result<(), ApiError> {
        self.read_only.register_project_store(project_id, store)
    }

    pub fn handle_get(&self, path: &str) -> Result<Value, ApiError> {
        self.read_only.handle_get(path)
    }

    pub fn handle_post(&self, path: &str, body: Value) -> Result<Value, ApiError> {
        self.handle(ApiRequest::post(path, body))
    }

    pub fn handle(&self, request: ApiRequest) -> Result<Value, ApiError> {
        if request.method() == ApiMethod::Get {
            return self.read_only.handle(request);
        }
        if request.method() != ApiMethod::Post {
            return self.read_only.error_envelope(
                "method_not_allowed",
                &format!(
                    "control API supports GET and POST, got {}",
                    request.method().as_str()
                ),
                json!({ "method": request.method().as_str(), "path": request.path() }),
            );
        }

        let parsed = ParsedPath::parse(request.path());
        let segments = parsed.segments();
        match segments.as_slice() {
            ["provider-connections", "instances"] => {
                self.provider_connection_save_response(request.body())
            }
            ["provider-connections", "validate"] => self
                .read_only
                .provider_connection_validate_response(request.body()),
            ["provider-connections", "select"] => {
                self.provider_connection_select_response(request.body())
            }
            ["provider-connections", "healthcheck"] => {
                self.provider_connection_healthcheck_response(request.body())
            }
            ["provider-connections", "run-request"] => {
                self.provider_connection_run_request_response(request.body())
            }
            ["projects", project_id, "jobs", job_id, "approve"] => {
                self.approve_response(project_id, job_id, request.body())
            }
            ["projects", project_id, "jobs", job_id, "cancel"] => {
                self.cancel_response(project_id, job_id)
            }
            ["projects", project_id, "jobs", job_id, "resume"] => {
                self.resume_response(project_id, job_id)
            }
            _ => self.read_only.error_envelope(
                "endpoint_not_found",
                "control API endpoint not found",
                json!({ "method": request.method().as_str(), "path": request.path() }),
            ),
        }
    }
}
