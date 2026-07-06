use super::helpers::{provider_registry_loader, write_temp_json};
use crate::ProviderRegistryError;
use serde_json::json;
use std::fs;

#[test]
fn rejects_instance_with_unknown_provider() {
    let loader = provider_registry_loader();
    let instance_path = write_temp_json(
        "unknown-provider-instance.json",
        &json!({
            "id": "unknown-default",
            "provider": "provider.unknown",
            "enabled": true,
            "limits": {
                "timeout_seconds": 10,
                "max_parallel_jobs": 1
            },
            "routing_tags": ["test"]
        }),
    );

    let error = loader
        .load_registry(
            "examples/provider-contracts/provider-registry.example.json",
            std::slice::from_ref(&instance_path),
        )
        .expect_err("unknown provider should fail");
    fs::remove_file(instance_path).ok();

    assert!(matches!(
        error,
        ProviderRegistryError::ProviderNotFound { provider_id } if provider_id == "provider.unknown"
    ));
}

#[test]
fn rejects_registry_path_traversal() {
    let loader = provider_registry_loader();
    let error = loader
        .resolve_registry_entry_path("../outside/provider.yaml")
        .expect_err("path traversal should fail");

    assert!(matches!(
        error,
        ProviderRegistryError::PathTraversalBlocked { .. }
    ));
}

#[test]
fn rejects_schema_invalid_manifest() {
    let loader = provider_registry_loader();
    let manifest_path = write_temp_json(
        "invalid-provider-manifest.json",
        &json!({
            "id": "provider.invalid",
            "name": "Invalid Provider",
            "kind": "not_a_kind",
            "transport": "manual",
            "adapter": "code_agent",
            "capabilities": {
                "edit_files": false,
                "run_shell": false,
                "read_repo": true,
                "apply_patch": false,
                "structured_output": true,
                "offline": true,
                "requires_login_session": false
            },
            "risk": {
                "can_modify_workspace": false,
                "can_run_commands": false,
                "requires_sandbox": false
            },
            "outputs": {
                "parser": "invalid"
            }
        }),
    );

    let error = loader
        .load_manifest(&manifest_path)
        .expect_err("invalid manifest should fail schema validation");
    fs::remove_file(manifest_path).ok();

    assert!(matches!(
        error,
        ProviderRegistryError::SchemaValidationFailed { .. }
    ));
}
