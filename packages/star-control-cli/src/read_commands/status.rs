use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::error::CliError;
use crate::output::{status_for_state, success_envelope};
use crate::{required_job, required_project};
use serde_json::{json, Value};
use star_control_state::StateStore;

pub(crate) fn status_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    let project = required_project(parsed)?;
    let job_id = required_job(parsed)?;
    let store =
        StateStore::open(&project, config.schema_root()).map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let state = store
        .load_state(&job_id)
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let events = store
        .read_events(&job_id)
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let latest_event = events
        .last()
        .and_then(|event| event.get("event_id"))
        .cloned()
        .unwrap_or_else(|| json!(""));

    Ok(success_envelope(
        "status",
        status_for_state(
            state
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("FAILED"),
        ),
        json!({
            "job_id": job_id,
            "state": state.get("state").cloned().unwrap_or_else(|| json!("")),
            "current_stage": state.get("current_stage").cloned().unwrap_or_else(|| json!("")),
            "next_action": state.get("next_action").cloned().unwrap_or_else(|| json!("")),
            "latest_event": latest_event,
            "artifacts": state.get("artifacts").cloned().unwrap_or_else(|| json!({}))
        }),
        vec![
            format!(".ai-runs/{}/run-state.json", job_id),
            format!(".ai-runs/{}/events.jsonl", job_id),
        ],
    ))
}
