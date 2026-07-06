use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn write_local_process_instance(project: &Path, args: Vec<String>) -> PathBuf {
    let path = project.join("local-process-instance.json");
    fs::write(
        &path,
        serde_json::to_string_pretty(&json!({
            "id": "local-default",
            "provider": "provider.local-process",
            "enabled": true,
            "limits": {
                "timeout_seconds": 10,
                "max_parallel_jobs": 1
            },
            "routing_tags": ["local", "process"],
            "command_policy": {
                "shell": false,
                "allowed_executables": [current_test_executable()],
                "env_allowlist": [],
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
    path
}

fn current_test_executable() -> String {
    std::env::current_exe()
        .expect("current test executable")
        .display()
        .to_string()
}
