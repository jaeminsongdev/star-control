use crate::args::ParsedArgs;
use crate::error::CliError;
use crate::{required_job, required_project};

pub(super) fn reject_sentinel_command_options(
    parsed: &ParsedArgs,
    requires_project_job: bool,
) -> Result<(), CliError> {
    let unsupported = [
        (parsed.subject.is_some(), "extra positional argument"),
        (parsed.request.is_some(), "--request"),
        (parsed.entrypoint.is_some(), "--entrypoint"),
        (parsed.provider.is_some(), "--provider"),
        (!parsed.provider_instances.is_empty(), "--provider-instance"),
        (parsed.stage.is_some(), "--stage"),
        (parsed.response.is_some(), "--response"),
        (parsed.reason.is_some(), "--reason"),
        (!parsed.constraints.is_empty(), "--constraint"),
        (parsed.release_readiness, "--release-readiness"),
        (parsed.recovery_list, "--list"),
        (parsed.dry_run, "--dry-run"),
        (parsed.markdown, "--markdown"),
    ];
    for (is_set, option) in unsupported {
        if is_set {
            return Err(CliError::InvalidInput {
                command: parsed.command.clone(),
                message: format!("sentinel does not accept {}", option),
            });
        }
    }
    if requires_project_job {
        let _ = required_project(parsed)?;
        let _ = required_job(parsed)?;
    } else if parsed.project.is_some() || parsed.job_id.is_some() {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "sentinel selfcheck does not accept --project or --job".to_string(),
        });
    }
    Ok(())
}
