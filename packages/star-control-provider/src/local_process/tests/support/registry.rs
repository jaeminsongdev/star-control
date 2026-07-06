use crate::local_process::constants::{LOCAL_PROCESS_KIND, PROCESS_TRANSPORT};
use crate::{ProviderInstance, ProviderManifest, ProviderRegistry, ProviderRegistryError};
use serde_json::json;
use std::path::PathBuf;

pub(crate) fn registry_with_instance(
    executable: &str,
    args: Vec<String>,
    allowed_executables: Vec<String>,
    env_allowlist: Vec<String>,
    timeout_seconds: u64,
) -> Result<ProviderRegistry, ProviderRegistryError> {
    let mut registry = ProviderRegistry::new();
    registry.register_manifest(ProviderManifest {
        id: "provider.local-process".to_string(),
        kind: LOCAL_PROCESS_KIND.to_string(),
        transport: PROCESS_TRANSPORT.to_string(),
        adapter: "chat_model".to_string(),
        path: PathBuf::from("provider.local-process.json"),
        value: json!({
            "id": "provider.local-process",
            "kind": LOCAL_PROCESS_KIND,
            "transport": PROCESS_TRANSPORT,
            "adapter": "chat_model"
        }),
    })?;
    registry.register_instance(ProviderInstance {
        id: "local-default".to_string(),
        provider_id: "provider.local-process".to_string(),
        enabled: true,
        routing_tags: vec!["local".to_string(), "process".to_string()],
        path: PathBuf::from("local-default.json"),
        value: json!({
            "id": "local-default",
            "provider": "provider.local-process",
            "enabled": true,
            "limits": {
                "timeout_seconds": timeout_seconds,
                "max_parallel_jobs": 1
            },
            "routing_tags": ["local", "process"],
            "command_policy": {
                "shell": false,
                "allowed_executables": allowed_executables,
                "env_allowlist": env_allowlist,
                "cwd_policy": "project_root",
                "network": "deny",
                "workspace_write": "deny"
            },
            "command": {
                "executable": executable,
                "args": args
            }
        }),
    })?;
    Ok(registry)
}
