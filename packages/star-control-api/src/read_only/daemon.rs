use super::ApiReadOnlyService;
use crate::error::ApiError;
use serde_json::json;
use star_control_daemon::DaemonError;

impl ApiReadOnlyService {
    pub(super) fn daemon_state_response(&self) -> Result<serde_json::Value, ApiError> {
        let Some(daemon_queue) = &self.daemon_queue else {
            return self.error_envelope(
                "daemon_not_registered",
                "daemon queue is not registered in read-only API",
                json!({}),
            );
        };
        match daemon_queue.load_state() {
            Ok(state) => self.success_envelope(json!({
                "daemon_state": state
            })),
            Err(source) => self.daemon_error_envelope("daemon_state_read_failed", source),
        }
    }

    fn daemon_error_envelope(
        &self,
        code: &str,
        source: DaemonError,
    ) -> Result<serde_json::Value, ApiError> {
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
