use super::super::helpers::{current_test_executable, repo_root};
use super::Fixture;
use serde_json::json;
use star_control_provider::ProviderRegistryLoader;
use std::fs;

impl Fixture {
    pub(crate) fn use_local_process_registry(
        &mut self,
        args: Vec<String>,
        env_allowlist: Vec<String>,
        timeout_seconds: u64,
    ) {
        let instance_path = self.project.join("local-process-instance.json");
        fs::write(
            &instance_path,
            serde_json::to_string_pretty(&json!({
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
                    "allowed_executables": [current_test_executable()],
                    "env_allowlist": env_allowlist,
                    "cwd_policy": "project_root",
                    "network": "deny",
                    "workspace_write": "deny"
                },
                "command": {
                    "executable": current_test_executable(),
                    "args": args
                }
            }))
            .expect("serialize local process instance"),
        )
        .expect("write local process instance");
        self.registry = ProviderRegistryLoader::new(repo_root())
            .load_registry(
                "configs/registries/builtin-provider-registry.yaml",
                &[instance_path],
            )
            .expect("load local process registry");
    }
}
