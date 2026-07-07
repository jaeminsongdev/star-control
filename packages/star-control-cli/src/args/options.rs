use super::model::ParsedArgs;
use crate::CliError;
use std::path::PathBuf;

pub(super) fn parse_next_argument(
    args: &[String],
    index: &mut usize,
    parsed: &mut ParsedArgs,
) -> Result<(), CliError> {
    let command = parsed.command.clone();
    match args[*index].as_str() {
        "--project" => {
            parsed.project = Some(PathBuf::from(require_option_value(
                args,
                index,
                "--project",
                &command,
            )?));
        }
        "--job" => {
            parsed.job_id = Some(require_option_value(args, index, "--job", &command)?);
        }
        "--request" => {
            parsed.request = Some(require_option_value(args, index, "--request", &command)?);
        }
        "--entrypoint" => {
            parsed.entrypoint = Some(require_option_value(args, index, "--entrypoint", &command)?);
        }
        "--provider" => {
            parsed.provider = Some(require_option_value(args, index, "--provider", &command)?);
        }
        "--provider-instance" => {
            parsed
                .provider_instances
                .push(PathBuf::from(require_option_value(
                    args,
                    index,
                    "--provider-instance",
                    &command,
                )?));
        }
        "--stage" => {
            parsed.stage = Some(require_option_value(args, index, "--stage", &command)?);
        }
        "--response" => {
            parsed.response = Some(require_option_value(args, index, "--response", &command)?);
        }
        "--reason" => {
            parsed.reason = Some(require_option_value(args, index, "--reason", &command)?);
        }
        "--constraint" => {
            parsed
                .constraints
                .push(require_option_value(args, index, "--constraint", &command)?);
        }
        "--dry-run" => parsed.dry_run = true,
        "--release-readiness" => parsed.release_readiness = true,
        "--list" => parsed.recovery_list = true,
        "--action" => {
            parsed.action = Some(require_option_value(args, index, "--action", &command)?);
        }
        "--approve-recovery-action" => {
            parsed.recovery_approval = Some(require_option_value(
                args,
                index,
                "--approve-recovery-action",
                &command,
            )?);
        }
        "--recovery-artifact" => {
            parsed.recovery_artifact = Some(require_option_value(
                args,
                index,
                "--recovery-artifact",
                &command,
            )?);
        }
        "--recovery-source" => {
            parsed.recovery_source = Some(require_option_value(
                args,
                index,
                "--recovery-source",
                &command,
            )?);
        }
        "--approve-release-action" => {
            parsed.release_approval = Some(require_option_value(
                args,
                index,
                "--approve-release-action",
                &command,
            )?);
        }
        "--json" => parsed.json = true,
        "--markdown" => parsed.markdown = true,
        positional if is_command_group_position(&command, positional) => {
            if parsed.subcommand.is_none() {
                parsed.subcommand = Some(positional.to_string());
            } else if parsed.subject.is_none() {
                parsed.subject = Some(positional.to_string());
            } else {
                return Err(CliError::InvalidInput {
                    command,
                    message: format!("unsupported argument {}", positional),
                });
            }
        }
        unknown => {
            return Err(CliError::InvalidInput {
                command,
                message: format!("unsupported option {}", unknown),
            });
        }
    }
    Ok(())
}

fn is_command_group_position(command: &str, argument: &str) -> bool {
    matches!(command, "providers" | "sentinel") && !argument.starts_with("--")
}

fn require_option_value(
    args: &[String],
    index: &mut usize,
    option: &str,
    command: &str,
) -> Result<String, CliError> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| CliError::InvalidInput {
            command: command.to_string(),
            message: format!("missing value for {}", option),
        })
}
