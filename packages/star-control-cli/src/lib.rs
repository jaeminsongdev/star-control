use args::{parse_args, ParsedArgs};
use output::{render_error, render_success};
use serde_json::Value;
use std::path::PathBuf;

mod args;
mod config;
mod constants;
mod control;
mod error;
mod output;
mod providers;
mod read_commands;
mod release;
mod run;
mod sentinel;
#[cfg(test)]
mod test_support;

pub use config::CliConfig;
pub use error::CliError;
pub use output::CliRunResult;

pub fn run_cli<I, S>(args: I, config: &CliConfig) -> CliRunResult
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let raw_args: Vec<String> = args.into_iter().map(Into::into).collect();
    let parsed = match parse_args(&raw_args) {
        Ok(parsed) => parsed,
        Err(error) => return render_error(error, true, config),
    };
    let json_mode = parsed.json;
    let command = parsed.command.clone();
    let result = match command.as_str() {
        "run" => run::run_command(&parsed, config),
        "status" => read_commands::status_command(&parsed, config),
        "report" => read_commands::report_command(&parsed, config),
        "approve" => control::approve_command(&parsed, config),
        "cancel" => control::cancel_command(&parsed, config),
        "resume" => control::resume_command(&parsed, config),
        "recover" => read_commands::recover_command(&parsed, config),
        "release" => release::release_command(&parsed, config),
        "providers" => providers::providers_command(&parsed, config),
        "sentinel" => sentinel::sentinel_command(&parsed, config),
        _ => Err(CliError::InvalidInput {
            command,
            message: "unsupported command".to_string(),
        }),
    };

    match result {
        Ok(envelope) => render_success(envelope, json_mode, config),
        Err(error) => render_error(error, json_mode, config),
    }
}

pub(crate) fn required_project(parsed: &ParsedArgs) -> Result<PathBuf, CliError> {
    parsed
        .project
        .clone()
        .ok_or_else(|| CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "--project is required".to_string(),
        })
}

pub(crate) fn required_job(parsed: &ParsedArgs) -> Result<String, CliError> {
    parsed.job_id.clone().ok_or_else(|| CliError::InvalidInput {
        command: parsed.command.clone(),
        message: "--job is required".to_string(),
    })
}

pub(crate) fn string_field(value: &Value, field: &str, command: &str) -> Result<String, CliError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| CliError::Internal {
            command: command.to_string(),
            message: format!("missing string field {}", field),
        })
}

#[cfg(test)]
mod tests;
