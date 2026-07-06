use crate::constants::{DAEMON_STATE_SCHEMA, SCHEMA_VERSION};
use crate::error::DaemonError;
use crate::io::write_bytes_atomic;
use crate::queue::DaemonQueue;
use serde_json::{json, Value};
use std::fs;

impl DaemonQueue {
    pub fn load_state(&self) -> Result<Value, DaemonError> {
        let content = fs::read_to_string(&self.state_path).map_err(|source| {
            DaemonError::StateReadFailed {
                path: self.state_path.clone(),
                source,
            }
        })?;
        let state: Value =
            serde_json::from_str(&content).map_err(|source| DaemonError::InvalidJson {
                path: self.state_path.clone(),
                source,
            })?;
        self.validate_schema(DAEMON_STATE_SCHEMA, &self.state_path, &state)?;
        Ok(state)
    }

    pub fn queue_len(&self) -> Result<usize, DaemonError> {
        let state = self.load_state()?;
        state
            .get("queue")
            .and_then(Value::as_array)
            .map(Vec::len)
            .ok_or_else(|| DaemonError::InvalidDaemonState {
                message: "queue must be an array".to_string(),
            })
    }

    pub(crate) fn default_state(&self) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "daemon_id": self.config.daemon_id(),
            "status": "reserved",
            "queue": [],
            "active_jobs": [],
            "last_error": null
        })
    }

    pub(crate) fn save_state(&self, state: &Value) -> Result<(), DaemonError> {
        self.validate_schema(DAEMON_STATE_SCHEMA, &self.state_path, state)?;
        let mut bytes =
            serde_json::to_vec_pretty(state).map_err(|source| DaemonError::InvalidJson {
                path: self.state_path.clone(),
                source,
            })?;
        bytes.push(b'\n');
        write_bytes_atomic(&self.daemon_dir, &self.state_path, &bytes)
    }
}
