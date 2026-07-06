use crate::constants::{DEFAULT_PRIORITY, QUEUED_STATE, TERMINAL_STATES};
use crate::error::DaemonError;
use crate::queue::fields::string_field;
use crate::queue::DaemonQueue;
use serde_json::{json, Value};
use star_control_state::StateStore;

impl DaemonQueue {
    pub fn enqueue_project_job(
        &self,
        project_store: &StateStore,
        job_id: &str,
    ) -> Result<Value, DaemonError> {
        project_store.load_job(job_id)?;
        let run_state = project_store.load_state(job_id)?;
        let state = string_field(&run_state, "state").unwrap_or_default();
        if TERMINAL_STATES.contains(&state.as_str()) {
            return Err(DaemonError::TerminalJobRejected {
                job_id: job_id.to_string(),
                state,
            });
        }
        if state == "WAITING_APPROVAL" {
            self.ensure_approved_response(project_store, job_id)?;
        }

        let project_root = project_store.project_root().display().to_string();
        let current_stage =
            string_field(&run_state, "current_stage").unwrap_or_else(|| "implement".to_string());
        let entry = json!({
            "job_id": job_id,
            "priority": DEFAULT_PRIORITY,
            "state": QUEUED_STATE,
            "project_root": project_root,
            "current_stage": current_stage,
            "run_state": state,
            "run_dir": format!(".ai-runs/{}", job_id)
        });

        let mut daemon_state = self.load_state()?;
        let queue = daemon_state
            .get_mut("queue")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| DaemonError::InvalidDaemonState {
                message: "queue must be an array".to_string(),
            })?;
        if queue.iter().any(|item| {
            item.get("job_id").and_then(Value::as_str) == Some(job_id)
                && item.get("project_root").and_then(Value::as_str) == Some(project_root.as_str())
        }) {
            return Err(DaemonError::DuplicateQueuedJob {
                job_id: job_id.to_string(),
                project_root,
            });
        }
        queue.push(entry.clone());
        self.save_state(&daemon_state)?;
        Ok(entry)
    }
}
