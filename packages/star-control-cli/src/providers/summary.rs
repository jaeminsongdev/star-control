use crate::config::CliConfig;
use serde_json::{json, Value};
use star_control_provider::{CapabilityProfile, ProviderManifest};
use std::path::Path;

pub(in crate::providers) fn provider_summary_value(
    manifest: &ProviderManifest,
    profile: Option<&CapabilityProfile>,
    config: &CliConfig,
) -> Value {
    json!({
        "id": manifest.id(),
        "kind": manifest.kind(),
        "transport": manifest.transport(),
        "adapter": manifest.adapter(),
        "manifest_path": repo_relative_path(config.repo_root(), manifest.path()),
        "capabilities_path": profile
            .map(|profile| repo_relative_path(config.repo_root(), profile.path()))
            .unwrap_or_default(),
        "routing_tags": profile
            .map(|profile| profile.routing_tags().to_vec())
            .unwrap_or_default()
    })
}

fn repo_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
