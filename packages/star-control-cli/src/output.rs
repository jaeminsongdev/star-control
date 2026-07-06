use crate::config::CliConfig;
use crate::constants::{CLI_ERROR_SCHEMA, CLI_OUTPUT_SCHEMA, SCHEMA_VERSION};
use crate::error::CliError;
use serde_json::{json, Value};
use star_control_schema::{load_schema, validate_json};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliRunResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub(crate) fn success_envelope(
    command: &str,
    status: &str,
    data: Value,
    artifacts: Vec<String>,
) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "command": command,
        "status": status,
        "exit_code": 0,
        "data": data,
        "warnings": [],
        "artifacts": artifacts
    })
}

fn error_envelope(error: &CliError) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "command": error.command(),
        "status": if error.exit_code() == 1 { "blocked" } else { "failed" },
        "exit_code": error.exit_code(),
        "error": {
            "code": error.code(),
            "message": error.message(),
            "recoverable": matches!(error, CliError::InvalidInput { .. } | CliError::MissingArtifact { .. }),
            "category": error.category(),
            "artifact_paths": error.artifact_paths()
        },
        "warnings": []
    })
}

pub(crate) fn status_for_state(state: &str) -> &'static str {
    match state {
        "BLOCKED" => "blocked",
        "WAITING_APPROVAL" => "waiting_approval",
        "FAILED" | "CANCELLED" => "failed",
        _ => "success",
    }
}

pub(crate) fn status_for_report(status: &str) -> &'static str {
    match status {
        "BLOCKED" => "blocked",
        "FAILED" => "failed",
        "NEEDS_APPROVAL" => "waiting_approval",
        _ => "success",
    }
}

pub(crate) fn render_success(envelope: Value, json_mode: bool, config: &CliConfig) -> CliRunResult {
    let command = envelope
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    if let Err(message) = validate_cli_envelope(&envelope, &config.schema_root(), CLI_OUTPUT_SCHEMA)
    {
        return render_error(CliError::Internal { command, message }, json_mode, config);
    }
    if json_mode {
        CliRunResult {
            exit_code: 0,
            stdout: serde_json::to_string_pretty(&envelope).unwrap_or_else(|_| "{}".to_string()),
            stderr: String::new(),
        }
    } else {
        CliRunResult {
            exit_code: 0,
            stdout: human_summary(&envelope),
            stderr: String::new(),
        }
    }
}

pub(crate) fn render_error(error: CliError, json_mode: bool, config: &CliConfig) -> CliRunResult {
    let exit_code = error.exit_code();
    let envelope = error_envelope(&error);
    let stdout = if json_mode {
        let _ = validate_cli_envelope(&envelope, &config.schema_root(), CLI_ERROR_SCHEMA);
        serde_json::to_string_pretty(&envelope).unwrap_or_else(|_| "{}".to_string())
    } else {
        String::new()
    };
    CliRunResult {
        exit_code,
        stdout,
        stderr: error.to_string(),
    }
}

fn validate_cli_envelope(
    envelope: &Value,
    schema_root: &Path,
    schema_file: &str,
) -> Result<(), String> {
    let schema_path = schema_root.join(schema_file);
    let schema = load_schema(&schema_path).map_err(|source| source.to_string())?;
    let result = validate_json(envelope, &schema);
    if result.is_ok() {
        Ok(())
    } else {
        Err(format!(
            "CLI envelope failed schema validation with {} error(s)",
            result.errors.len()
        ))
    }
}

fn human_summary(envelope: &Value) -> String {
    let command = envelope
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("");
    let status = envelope.get("status").and_then(Value::as_str).unwrap_or("");
    let job_id = envelope
        .pointer("/data/job_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    if job_id.is_empty() {
        format!("{}: {}", command, status)
    } else {
        format!("{}: {} ({})", command, status, job_id)
    }
}
