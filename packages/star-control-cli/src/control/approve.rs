use super::helpers::{
    allowed_next_stage_for, append_cli_event, load_job_json, next_action_after_approval_response,
    required_response, state_after_approval_response, state_string, timestamp_string,
    update_state_for_control_command, validate_approval_response_value, validate_schema_value,
};
use super::{
    CliEvent, APPROVAL_REQUEST_PATH, APPROVAL_RESPONSE_FILE, APPROVAL_RESPONSE_PATH, CLI_REVIEWER,
    COMMAND_APPROVE, EVENT_APPROVAL_RECORDED, RESPONSE_APPROVED, STATE_WAITING_APPROVAL,
};
use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::constants::{APPROVAL_REQUEST_SCHEMA, APPROVAL_RESPONSE_SCHEMA, SCHEMA_VERSION};
use crate::error::CliError;
use crate::output::success_envelope;
use crate::{required_job, required_project, string_field};
use serde_json::{json, Value};
use star_control_state::StateStore;

pub(crate) fn approve_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    if parsed.has_recovery_source_selection() {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "approve does not accept --recovery-artifact or --recovery-source".to_string(),
        });
    }
    let project = required_project(parsed)?;
    let job_id = required_job(parsed)?;
    let response = required_response(parsed)?;
    let reason = parsed
        .reason
        .clone()
        .ok_or_else(|| CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "--reason is required for approve".to_string(),
        })?;
    validate_approval_response_value(&response, &parsed.command)?;

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
    if current_state != STATE_WAITING_APPROVAL {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: format!(
                "approve requires WAITING_APPROVAL state, got {}",
                current_state
            ),
        });
    }

    let approval_request = load_job_json(
        &store,
        &job_id,
        APPROVAL_REQUEST_PATH,
        APPROVAL_REQUEST_SCHEMA,
        &parsed.command,
        &config.schema_root(),
    )?;
    let stage = string_field(&approval_request, "stage", &parsed.command)?;
    let task_id = string_field(&approval_request, "task_id", &parsed.command)?;
    let allowed_next_stage = (response == RESPONSE_APPROVED)
        .then(|| allowed_next_stage_for(&stage))
        .flatten();
    let approval_response = json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id.clone(),
        "stage": stage.clone(),
        "task_id": task_id.clone(),
        "response": response.clone(),
        "reviewer": CLI_REVIEWER,
        "responded_at": timestamp_string(),
        "reason": reason,
        "allowed_next_stage": allowed_next_stage,
        "constraints": parsed.constraints.clone()
    });
    validate_schema_value(
        &approval_response,
        &config.schema_root(),
        APPROVAL_RESPONSE_SCHEMA,
        APPROVAL_RESPONSE_PATH,
    )
    .map_err(|message| CliError::Internal {
        command: parsed.command.clone(),
        message,
    })?;

    let approval_ref = store
        .write_approval_json(&job_id, APPROVAL_RESPONSE_FILE, &approval_response)
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let next_state = state_after_approval_response(&response);
    let next_action = next_action_after_approval_response(&response);
    let event_id = format!("{}-cli-approval-recorded", job_id.to_lowercase());
    update_state_for_control_command(
        &mut state,
        &store,
        next_state,
        &stage,
        next_action,
        &event_id,
        Some(("approval_response", &approval_ref)),
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
            event_type: EVENT_APPROVAL_RECORDED,
            state: next_state.to_string(),
            stage: stage.clone(),
            message: "Approval response recorded",
            artifact_paths: vec![APPROVAL_RESPONSE_PATH.to_string()],
            details: json!({
                "response": approval_response["response"],
                "allowed_next_stage": approval_response["allowed_next_stage"]
            }),
        },
    )
    .map_err(|source| CliError::State {
        command: parsed.command.clone(),
        source,
    })?;

    Ok(success_envelope(
        COMMAND_APPROVE,
        "success",
        json!({
            "job_id": job_id,
            "state": state["state"],
            "approval_response": approval_response["response"],
            "allowed_next_stage": approval_response["allowed_next_stage"]
        }),
        vec![format!(".ai-runs/{}/{}", job_id, APPROVAL_RESPONSE_PATH)],
    ))
}
