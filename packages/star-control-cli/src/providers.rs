mod options;
mod registry;
mod summary;

use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::constants::BUILTIN_PROVIDER_REGISTRY;
use crate::error::CliError;
use crate::output::success_envelope;
use options::reject_provider_command_options;
use registry::load_builtin_provider_registry;
use serde_json::{json, Value};
use summary::provider_summary_value;

pub(crate) fn providers_command(
    parsed: &ParsedArgs,
    config: &CliConfig,
) -> Result<Value, CliError> {
    reject_provider_command_options(parsed)?;
    let subcommand = parsed
        .subcommand
        .as_deref()
        .ok_or_else(|| CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "providers requires subcommand list or show".to_string(),
        })?;
    match subcommand {
        "list" => providers_list_command(parsed, config),
        "show" => providers_show_command(parsed, config),
        "healthcheck" => Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "providers healthcheck is reserved until provider smoke checks are enabled"
                .to_string(),
        }),
        other => Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: format!("unsupported providers subcommand {}", other),
        }),
    }
}

fn providers_list_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    if parsed.provider.is_some() || parsed.subject.is_some() {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "providers list does not accept provider id arguments".to_string(),
        });
    }
    let registry = load_builtin_provider_registry(parsed, config)?;
    let providers: Vec<Value> = registry
        .providers()
        .into_iter()
        .map(|manifest| {
            let profile = registry.capability_profile(manifest.id());
            provider_summary_value(manifest, profile, config)
        })
        .collect();

    Ok(success_envelope(
        "providers",
        "success",
        json!({
            "subcommand": "list",
            "registry_path": BUILTIN_PROVIDER_REGISTRY,
            "provider_count": providers.len(),
            "providers": providers,
            "healthcheck_enabled": false,
            "actions_enabled": false
        }),
        Vec::new(),
    ))
}

fn providers_show_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    let provider_id = match (parsed.subject.as_deref(), parsed.provider.as_deref()) {
        (Some(subject), Some(provider)) if subject != provider => {
            return Err(CliError::InvalidInput {
                command: parsed.command.clone(),
                message: format!(
                    "providers show provider id mismatch: argument {}, --provider {}",
                    subject, provider
                ),
            });
        }
        (Some(subject), _) => subject.to_string(),
        (_, Some(provider)) => provider.to_string(),
        (None, None) => {
            return Err(CliError::InvalidInput {
                command: parsed.command.clone(),
                message: "providers show requires a provider id".to_string(),
            });
        }
    };

    let registry = load_builtin_provider_registry(parsed, config)?;
    let manifest = registry
        .manifest(&provider_id)
        .ok_or_else(|| CliError::InvalidInput {
            command: parsed.command.clone(),
            message: format!("provider {} is not registered", provider_id),
        })?;
    let profile =
        registry
            .capability_profile(&provider_id)
            .ok_or_else(|| CliError::InvalidInput {
                command: parsed.command.clone(),
                message: format!("provider {} has no capability profile", provider_id),
            })?;

    Ok(success_envelope(
        "providers",
        "success",
        json!({
            "subcommand": "show",
            "registry_path": BUILTIN_PROVIDER_REGISTRY,
            "provider": provider_summary_value(manifest, Some(profile), config),
            "manifest": manifest.value(),
            "capability_profile": profile.value(),
            "healthcheck_enabled": false,
            "actions_enabled": false
        }),
        Vec::new(),
    ))
}
