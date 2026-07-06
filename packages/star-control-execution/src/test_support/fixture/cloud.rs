use super::super::helpers::{current_test_executable, repo_root};
use super::Fixture;
use serde_json::{json, Value};
use star_control_provider::ProviderRegistryLoader;
use std::fs;

impl Fixture {
    pub(crate) fn use_cloud_cli_registry(
        &mut self,
        args: Vec<String>,
        env_allowlist: Vec<String>,
        timeout_seconds: u64,
    ) {
        let instance_path = self.project.join("cloud-cli-instance.json");
        fs::write(
            &instance_path,
            serde_json::to_string_pretty(&json!({
                "id": "cloud-default",
                "provider": "provider.codex-cli",
                "enabled": true,
                "limits": {
                    "timeout_seconds": timeout_seconds,
                    "max_parallel_jobs": 1
                },
                "routing_tags": ["cloud", "cli"],
                "transport_config": {
                    "auth_mode": "login_session",
                    "privacy_handoff_approved": true
                },
                "budget": {
                    "estimated_cost": 0,
                    "currency": "USD"
                },
                "command_policy": {
                    "shell": false,
                    "env_allowlist": env_allowlist
                },
                "command": {
                    "executable": current_test_executable(),
                    "args": args
                }
            }))
            .expect("serialize cloud CLI instance"),
        )
        .expect("write cloud CLI instance");
        self.registry = ProviderRegistryLoader::new(repo_root())
            .load_registry(
                "configs/registries/builtin-provider-registry.yaml",
                &[instance_path],
            )
            .expect("load cloud CLI registry");
    }

    pub(crate) fn write_openai_response_fixture(&self, relative_path: &str, value: &Value) {
        let path = self.project.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create response fixture parent");
        }
        fs::write(
            path,
            serde_json::to_string_pretty(value).expect("serialize response fixture"),
        )
        .expect("write response fixture");
    }

    pub(crate) fn use_cloud_api_offline_registry(&mut self, fixture_relative_path: &str) {
        let instance_path = self.project.join("cloud-api-instance.json");
        fs::write(
            &instance_path,
            serde_json::to_string_pretty(&json!({
                "id": "cloud-default",
                "provider": "provider.openai",
                "enabled": true,
                "credential_ref": "env:OPENAI_API_KEY",
                "limits": {
                    "timeout_seconds": 300,
                    "max_parallel_jobs": 1
                },
                "routing_tags": ["cloud", "api"],
                "transport_config": {
                    "privacy_handoff_approved": true,
                    "offline_response_fixture": fixture_relative_path
                },
                "budget": {
                    "estimated_cost": 0,
                    "currency": "USD"
                },
                "endpoint": {
                    "base_url": "https://api.openai.com/v1",
                    "model": "gpt-example"
                }
            }))
            .expect("serialize cloud API instance"),
        )
        .expect("write cloud API instance");
        self.registry = ProviderRegistryLoader::new(repo_root())
            .load_registry(
                "configs/registries/builtin-provider-registry.yaml",
                &[instance_path],
            )
            .expect("load cloud API registry");
    }

    pub(crate) fn use_cloud_api_live_approval_registry(&mut self) {
        let instance_path = self.project.join("cloud-api-instance.json");
        fs::write(
            &instance_path,
            serde_json::to_string_pretty(&json!({
                "id": "cloud-default",
                "provider": "provider.openai",
                "enabled": true,
                "credential_ref": "env:OPENAI_API_KEY",
                "limits": {
                    "timeout_seconds": 300,
                    "max_parallel_jobs": 1
                },
                "routing_tags": ["cloud", "api"],
                "transport_config": {
                    "privacy_handoff_approved": true,
                    "live_api_call_requested": true
                },
                "budget": {
                    "estimated_cost": 0,
                    "currency": "USD"
                },
                "endpoint": {
                    "base_url": "https://api.openai.com/v1",
                    "model": "gpt-example"
                }
            }))
            .expect("serialize cloud API instance"),
        )
        .expect("write cloud API instance");
        self.registry = ProviderRegistryLoader::new(repo_root())
            .load_registry(
                "configs/registries/builtin-provider-registry.yaml",
                &[instance_path],
            )
            .expect("load cloud API registry");
    }
}
