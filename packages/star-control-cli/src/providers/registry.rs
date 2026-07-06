use crate::args::ParsedArgs;
use crate::config::CliConfig;
use crate::constants::BUILTIN_PROVIDER_REGISTRY;
use crate::error::CliError;
use star_control_provider::{ProviderRegistry, ProviderRegistryLoader};

pub(super) fn load_builtin_provider_registry(
    parsed: &ParsedArgs,
    config: &CliConfig,
) -> Result<ProviderRegistry, CliError> {
    let loader = ProviderRegistryLoader::new(config.repo_root());
    loader
        .load_registry(BUILTIN_PROVIDER_REGISTRY, &[])
        .map_err(|source| CliError::ProviderRegistry {
            command: parsed.command.clone(),
            source,
        })
}
