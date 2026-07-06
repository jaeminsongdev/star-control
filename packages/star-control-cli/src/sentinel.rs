use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::error::CliError;
use serde_json::Value;

mod commands;
mod evaluation;
mod options;
mod paths;
mod status;

pub(crate) fn sentinel_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    let subcommand = parsed
        .subcommand
        .as_deref()
        .ok_or_else(|| CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "sentinel requires subcommand check, gate, review-pack, or selfcheck"
                .to_string(),
        })?;
    match subcommand {
        "check" => commands::sentinel_check_command(parsed, config),
        "gate" => commands::sentinel_gate_command(parsed, config),
        "review-pack" => commands::sentinel_review_pack_command(parsed, config),
        "selfcheck" => commands::sentinel_selfcheck_command(parsed, config),
        other => Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: format!("unsupported sentinel subcommand {}", other),
        }),
    }
}
