use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::constants::{
    BUILTIN_PROVIDER_REGISTRY, DEFAULT_PROVIDER, FAKE_PROVIDER_INSTANCE_EXAMPLE,
};
use crate::error::CliError;
use star_control_provider::{ProviderRegistry, ProviderRegistryLoader};
use std::path::PathBuf;

pub(super) fn load_run_registry(
    parsed: &ParsedArgs,
    config: &CliConfig,
    provider: &str,
) -> Result<ProviderRegistry, CliError> {
    if parsed.provider.is_none() && !parsed.provider_instances.is_empty() {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "--provider is required when --provider-instance is set".to_string(),
        });
    }
    if provider != DEFAULT_PROVIDER && parsed.provider_instances.is_empty() {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "--provider-instance is required when --provider is not fake-default"
                .to_string(),
        });
    }

    let loader = ProviderRegistryLoader::new(config.repo_root());
    let registry = if provider == DEFAULT_PROVIDER && parsed.provider_instances.is_empty() {
        loader
            .load_fake_default_registry()
            .map_err(|source| CliError::ProviderRegistry {
                command: parsed.command.clone(),
                source,
            })?
    } else {
        let mut instance_paths = vec![PathBuf::from(FAKE_PROVIDER_INSTANCE_EXAMPLE)];
        instance_paths.extend(parsed.provider_instances.iter().cloned());
        loader
            .load_registry(BUILTIN_PROVIDER_REGISTRY, &instance_paths)
            .map_err(|source| CliError::ProviderRegistry {
                command: parsed.command.clone(),
                source,
            })?
    };

    registry
        .instance(provider)
        .ok_or_else(|| CliError::InvalidInput {
            command: parsed.command.clone(),
            message: format!("provider instance {} is not loaded", provider),
        })?;
    Ok(registry)
}
