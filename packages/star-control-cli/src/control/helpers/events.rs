use super::super::CliEvent;
use super::time::timestamp_string;
use crate::constants::SCHEMA_VERSION;
use serde_json::json;
use star_control_state::{StateStore, StateStoreError};

pub(in crate::control) fn append_cli_event(
    store: &StateStore,
    job_id: &str,
    event: CliEvent,
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
