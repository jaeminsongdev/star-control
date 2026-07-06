use crate::{ProviderInstance, ProviderManifest, ProviderRegistry, ProviderRegistryError};
use serde_json::{json, Value};
use std::path::PathBuf;

pub(crate) fn registry_with_instance(
    kind: &str,
    transport: &str,
    instance_value: Value,
) -> Result<ProviderRegistry, ProviderRegistryError> {
    let mut registry = ProviderRegistry::new();
    registry.register_manifest(ProviderManifest {
        id: "provider.cloud".to_string(),
        kind: kind.to_string(),
        transport: transport.to_string(),
        adapter: "code_agent".to_string(),
        path: PathBuf::from("provider.cloud.json"),
        value: json!({
            "id": "provider.cloud",
            "kind": kind,
            "transport": transport,
            "adapter": "code_agent"
        }),
    })?;
    registry.register_instance(ProviderInstance {
        id: "cloud-default".to_string(),
        provider_id: "provider.cloud".to_string(),
        enabled: true,
        routing_tags: vec!["cloud".to_string()],
        path: PathBuf::from("cloud-default.json"),
        value: instance_value,
    })?;
    Ok(registry)
}
