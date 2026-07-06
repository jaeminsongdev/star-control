use super::super::{
    NEXT_ACTION_RESUME, NEXT_ACTION_REVISE, NEXT_ACTION_STOP, RESPONSE_APPROVED,
    RESPONSE_CANCELLED, RESPONSE_NEEDS_CHANGES, RESPONSE_REJECTED, STATE_BLOCKED, STATE_CANCELLED,
    STATE_WAITING_APPROVAL,
};
use crate::args::ParsedArgs;
use crate::error::CliError;
use crate::string_field;
use serde_json::Value;

pub(in crate::control) fn required_response(parsed: &ParsedArgs) -> Result<String, CliError> {
    parsed
        .response
        .clone()
        .ok_or_else(|| CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "--response is required for approve".to_string(),
        })
}

pub(in crate::control) fn validate_approval_response_value(
    response: &str,
    command: &str,
) -> Result<(), CliError> {
    match response {
        RESPONSE_APPROVED | RESPONSE_REJECTED | RESPONSE_NEEDS_CHANGES | RESPONSE_CANCELLED => {
            Ok(())
        }
        _ => Err(CliError::InvalidInput {
            command: command.to_string(),
            message: format!("unsupported approval response {}", response),
        }),
    }
}

pub(in crate::control) fn ensure_approval_response_matches_request(
    approval_request: &Value,
    approval_response: &Value,
    command: &str,
) -> Result<(), CliError> {
    for field in ["job_id", "stage", "task_id"] {
        let expected = string_field(approval_request, field, command)?;
        let actual = string_field(approval_response, field, command)?;
        if expected != actual {
            return Err(CliError::InvalidInput {
                command: command.to_string(),
                message: format!(
                    "approval response {} mismatch: expected {}, got {}",
                    field, expected, actual
                ),
            });
        }
    }
    let response = string_field(approval_response, "response", command)?;
    if response != RESPONSE_APPROVED {
        return Err(CliError::InvalidInput {
            command: command.to_string(),
            message: format!("resume requires approved response, got {}", response),
        });
    }
    Ok(())
}

pub(in crate::control) fn state_after_approval_response(response: &str) -> &'static str {
    match response {
        RESPONSE_APPROVED => STATE_WAITING_APPROVAL,
        RESPONSE_CANCELLED => STATE_CANCELLED,
        _ => STATE_BLOCKED,
    }
}

pub(in crate::control) fn next_action_after_approval_response(response: &str) -> &'static str {
    match response {
        RESPONSE_APPROVED => NEXT_ACTION_RESUME,
        RESPONSE_CANCELLED => NEXT_ACTION_STOP,
        RESPONSE_NEEDS_CHANGES => NEXT_ACTION_REVISE,
        _ => NEXT_ACTION_STOP,
    }
}
