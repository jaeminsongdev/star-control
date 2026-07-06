use super::super::options::reject_sentinel_command_options;
use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::error::CliError;
use crate::output::success_envelope;
use serde_json::{json, Value};
use star_sentinel::run_selfcheck;

pub(in crate::sentinel) fn sentinel_selfcheck_command(
    parsed: &ParsedArgs,
    config: &CliConfig,
) -> Result<Value, CliError> {
    reject_sentinel_command_options(parsed, false)?;
    let report = run_selfcheck(config.repo_root());
    Ok(success_envelope(
        "sentinel",
        if report.ok { "success" } else { "failed" },
        json!({
            "subcommand": "selfcheck",
            "ok": report.ok,
            "diagnostic_count": report.diagnostics.len(),
            "diagnostics": report.diagnostics,
            "actions_enabled": false
        }),
        Vec::new(),
    ))
}
