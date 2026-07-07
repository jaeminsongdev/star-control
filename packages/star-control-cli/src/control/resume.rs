use super::helpers::{
    append_cli_event, ensure_approval_response_matches_request, load_job_json, state_string,
    update_state_for_control_command,
};
use super::{
    CliEvent, APPROVAL_REQUEST_PATH, APPROVAL_RESPONSE_PATH, COMMAND_RESUME, EVENT_STATE_CHANGED,
    NEXT_ACTION_REPORT, RUN_STATE_PATH, STATE_VALIDATED, STATE_WAITING_APPROVAL,
};
use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::constants::{APPROVAL_REQUEST_SCHEMA, APPROVAL_RESPONSE_SCHEMA};
use crate::error::CliError;
use crate::output::success_envelope;
use crate::{required_job, required_project};
use serde_json::{json, Value};
use star_control_state::StateStore;

pub(crate) fn resume_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    if parsed.has_recovery_source_selection() {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "resume does not accept --recovery-artifact or --recovery-source".to_string(),
        });
    }
    let project = required_project(parsed)?;
    let job_id = required_job(parsed)?;
    let store =
        StateStore::open(&project, config.schema_root()).map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    store
        .ensure_resume_allowed(&job_id)
        .map_err(|source| CliError::State {
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
    let current_stage = state
        .get("current_stage")
        .and_then(Value::as_str)
        .unwrap_or("implement")
        .to_string();

    if current_state == STATE_WAITING_APPROVAL {
        let approval_request = load_job_json(
            &store,
            &job_id,
            APPROVAL_REQUEST_PATH,
            APPROVAL_REQUEST_SCHEMA,
            &parsed.command,
            &config.schema_root(),
        )?;
        let approval_response = load_job_json(
            &store,
            &job_id,
            APPROVAL_RESPONSE_PATH,
            APPROVAL_RESPONSE_SCHEMA,
            &parsed.command,
            &config.schema_root(),
        )?;
        ensure_approval_response_matches_request(
            &approval_request,
            &approval_response,
            &parsed.command,
        )?;
        let event_id = format!("{}-cli-resumed", job_id.to_lowercase());
        let next_action = approval_response
            .get("allowed_next_stage")
            .and_then(Value::as_str)
            .unwrap_or(NEXT_ACTION_REPORT);
        update_state_for_control_command(
            &mut state,
            &store,
            STATE_VALIDATED,
            &current_stage,
            next_action,
            &event_id,
            None,
        )?;
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
                state: STATE_VALIDATED.to_string(),
                stage: current_stage.clone(),
                message: "Approval accepted; job is ready to continue",
                artifact_paths: vec![
                    RUN_STATE_PATH.to_string(),
                    APPROVAL_RESPONSE_PATH.to_string(),
                ],
                details: json!({ "previous_state": current_state, "next_action": next_action }),
            },
        )
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
        return Ok(success_envelope(
            COMMAND_RESUME,
            "success",
            json!({
                "job_id": job_id,
                "state": STATE_VALIDATED,
                "previous_state": current_state,
                "next_action": next_action,
                "resumed": true
            }),
            vec![
                format!(".ai-runs/{}/{}", job_id, RUN_STATE_PATH),
                format!(".ai-runs/{}/{}", job_id, APPROVAL_RESPONSE_PATH),
            ],
        ));
    }

    Ok(success_envelope(
        COMMAND_RESUME,
        "success",
        json!({
            "job_id": job_id,
            "state": current_state,
            "current_stage": current_stage,
            "next_action": state.get("next_action").cloned().unwrap_or_else(|| json!("")),
            "resumed": false
        }),
        vec![format!(".ai-runs/{}/{}", job_id, RUN_STATE_PATH)],
    ))
}
