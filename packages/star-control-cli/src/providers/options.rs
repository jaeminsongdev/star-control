use crate::args::ParsedArgs;
use crate::error::CliError;

pub(super) fn reject_provider_command_options(parsed: &ParsedArgs) -> Result<(), CliError> {
    let unsupported = [
        (parsed.project.is_some(), "--project"),
        (parsed.job_id.is_some(), "--job"),
        (parsed.request.is_some(), "--request"),
        (parsed.entrypoint.is_some(), "--entrypoint"),
        (!parsed.provider_instances.is_empty(), "--provider-instance"),
        (parsed.stage.is_some(), "--stage"),
        (parsed.response.is_some(), "--response"),
        (parsed.reason.is_some(), "--reason"),
        (!parsed.constraints.is_empty(), "--constraint"),
        (parsed.release_readiness, "--release-readiness"),
        (parsed.recovery_list, "--list"),
        (parsed.has_recovery_source_selection(), "--recovery-source"),
        (parsed.dry_run, "--dry-run"),
        (parsed.markdown, "--markdown"),
    ];
    for (is_set, option) in unsupported {
        if is_set {
            return Err(CliError::InvalidInput {
                command: parsed.command.clone(),
                message: format!("providers does not accept {}", option),
            });
        }
    }
    Ok(())
}
