use super::time::timestamp_string;
use crate::constants::SCHEMA_VERSION;
use serde_json::{json, Value};
use star_control_state::{StateStore, StateStoreError};

pub(in crate::control) struct ApiControlEvent<'a> {
    pub(in crate::control) event_id: String,
    pub(in crate::control) event_type: &'a str,
    pub(in crate::control) state: &'a str,
    pub(in crate::control) stage: &'a str,
    pub(in crate::control) message: &'a str,
    pub(in crate::control) artifact_paths: Vec<String>,
    pub(in crate::control) details: Value,
}

pub(in crate::control) fn append_api_event(
    store: &StateStore,
    job_id: &str,
    event: ApiControlEvent<'_>,
) -> Result<(), StateStoreError> {
    store.append_event(
        job_id,
        &json!({
            "schema_version": SCHEMA_VERSION,
            "event_id": event.event_id,
            "job_id": job_id,
            "type": event.event_type,
            "created_at": timestamp_string(),
            "stage": event.stage,
            "state": event.state,
            "message": event.message,
            "artifact_paths": event.artifact_paths,
            "details": event.details
        }),
    )
}
