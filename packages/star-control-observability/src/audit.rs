mod io;
mod time;
mod validation;

use crate::constants::SCHEMA_VERSION;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AuditEventWriter {
    schema_root: PathBuf,
}

impl AuditEventWriter {
    pub fn new(schema_root: impl Into<PathBuf>) -> Self {
        Self {
            schema_root: schema_root.into(),
        }
    }

    pub fn event(
        &self,
        job_id: &str,
        event_id: impl Into<String>,
        event_type: impl Into<String>,
        actor: impl Into<String>,
        summary: impl Into<String>,
    ) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "event_id": event_id.into(),
            "job_id": job_id,
            "type": event_type.into(),
            "created_at": time::timestamp_string(),
            "actor": actor.into(),
            "summary": summary.into()
        })
    }
}
