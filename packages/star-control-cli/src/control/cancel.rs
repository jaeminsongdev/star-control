use super::helpers::{append_cli_event, state_string, update_state_for_control_command};
use super::{
    CliEvent, COMMAND_CANCEL, EVENT_STATE_CHANGED, NEXT_ACTION_STOP, RUN_STATE_PATH,
    STATE_CANCELLED,
};
use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::constants::TERMINAL_STATES;
use crate::error::CliError;
use crate::output::success_envelope;
use crate::{required_job, required_project};
use serde_json::{json, Value};
use star_control_state::StateStore;

pub(crate) fn cancel_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    let project = required_project(parsed)?;
    let job_id = required_job(parsed)?;
    let store =
        StateStore::open(&project, config.schema_root()).map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let mut state = store
        .load_state(&job_id)
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let current_state = state_string(&state);
    if TERMINAL_STATES.contains(&current_state.as_str()) {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: format!("cannot cancel terminal job state {}", current_state),
        });
    }
    let current_stage = state
        .get("current_stage")
        .and_then(Value::as_str)
        .unwrap_or("implement")
        .to_string();
    let event_id = format!("{}-cli-cancelled", job_id.to_lowercase());
    update_state_for_control_command(
        &mut state,
        &store,
        STATE_CANCELLED,
        &current_stage,
        NEXT_ACTION_STOP,
        &event_id,
        None,
    )?;
    if let Some(state_object) = state.as_object_mut() {
        state_object.insert("active_provider".to_string(), Value::Null);
    }
    store
        .save_state(&job_id, &state)
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    append_cli_event(
        &store,
        &job_id,
        CliEvent {
            event_id,
            event_type: EVENT_STATE_CHANGED,
            state: STATE_CANCELLED.to_string(),
            stage: current_stage.clone(),
            message: "Job cancelled by CLI",
            artifact_paths: vec![RUN_STATE_PATH.to_string()],
            details: json!({ "previous_state": current_state }),
        },
    )
    .map_err(|source| CliError::State {
        command: parsed.command.clone(),
        source,
    })?;

    Ok(success_envelope(
        COMMAND_CANCEL,
        "success",
        json!({
            "job_id": job_id,
            "state": STATE_CANCELLED,
            "previous_state": current_state,
            "next_action": NEXT_ACTION_STOP
        }),
        vec![format!(".ai-runs/{}/{}", job_id, RUN_STATE_PATH)],
    ))
}
