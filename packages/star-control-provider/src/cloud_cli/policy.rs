use super::fields::{optional_string_array, required_string};
use super::render::render_arg;
use crate::cloud_constants::{DEFAULT_TIMEOUT_SECONDS, MAX_TIMEOUT_SECONDS};
use crate::cloud_policy::{cloud_policy_denied, string_field};
use crate::{ExecutionRequest, ProviderAdapterError, ProviderInstance};
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CloudCliCommandPolicy {
    executable: String,
    args: Vec<String>,
    env_allowlist: Vec<String>,
    timeout_seconds: u64,
}

impl CloudCliCommandPolicy {
    pub(crate) fn from_instance(instance: &ProviderInstance) -> Result<Self, ProviderAdapterError> {
        let source = instance.path();
        let value = instance.value();
        let command = value
            .get("command")
            .ok_or_else(|| cloud_policy_denied(instance.id(), "command object is required"))?;

        if value
            .pointer("/command_policy/shell")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(cloud_policy_denied(instance.id(), "shell must be false"));
        }

        let executable = required_string(command, source, "command.executable", "executable")?;
        validate_executable(instance.id(), &executable)?;
        let args = optional_string_array(command, instance.id(), "command.args", "args")?;
        let env_allowlist = optional_string_array(
            value.pointer("/command_policy").unwrap_or(&Value::Null),
            instance.id(),
            "command_policy.env_allowlist",
            "env_allowlist",
        )?;
        validate_no_credential_ref_env_passthrough(instance, &env_allowlist)?;
        let timeout_seconds = timeout_seconds(value, instance.id())?;

        Ok(Self {
            executable,
            args,
            env_allowlist,
            timeout_seconds,
        })
    }

    pub(super) fn executable(&self) -> &str {
        &self.executable
    }

    pub(super) fn timeout_seconds(&self) -> u64 {
        self.timeout_seconds
    }

    pub(super) fn env_allowlist(&self) -> &[String] {
        &self.env_allowlist
    }

    pub(super) fn rendered_args(
        &self,
        request: &ExecutionRequest,
        request_ref: &Value,
    ) -> Vec<String> {
        self.args
            .iter()
            .map(|arg| render_arg(arg, request, request_ref))
            .collect()
    }
}

pub(crate) fn timeout_seconds(
    value: &Value,
    provider_instance_id: &str,
) -> Result<u64, ProviderAdapterError> {
    let timeout_seconds = value
        .pointer("/limits/timeout_seconds")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_TIMEOUT_SECONDS);
    if timeout_seconds > MAX_TIMEOUT_SECONDS {
        return Err(cloud_policy_denied(
            provider_instance_id,
            &format!("limits.timeout_seconds must be <= {}", MAX_TIMEOUT_SECONDS),
        ));
    }
    Ok(timeout_seconds)
}

fn validate_executable(
    provider_instance_id: &str,
    executable: &str,
) -> Result<(), ProviderAdapterError> {
    if executable.trim().is_empty() {
        return Err(cloud_policy_denied(
            provider_instance_id,
            "command.executable must not be empty",
        ));
    }
    let lower_name = Path::new(executable)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(executable)
        .to_ascii_lowercase();
    let shell_wrappers = [
        "cmd",
        "cmd.exe",
        "powershell",
        "powershell.exe",
        "pwsh",
        "pwsh.exe",
        "sh",
        "bash",
        "zsh",
    ];
    if shell_wrappers.contains(&lower_name.as_str()) {
        return Err(cloud_policy_denied(
            provider_instance_id,
            "shell wrapper executable is not allowed for cloud CLI transport",
        ));
    }
    Ok(())
}

fn validate_no_credential_ref_env_passthrough(
    instance: &ProviderInstance,
    env_allowlist: &[String],
) -> Result<(), ProviderAdapterError> {
    let Some(credential_ref) = string_field(instance.value(), "credential_ref") else {
        return Ok(());
    };
    let Some(env_name) = credential_ref.strip_prefix("env:") else {
        return Ok(());
    };
    if env_allowlist.iter().any(|name| name == env_name) {
        return Err(cloud_policy_denied(
            instance.id(),
            "credential_ref env var must not be passed through command_policy.env_allowlist",
        ));
    }
    Ok(())
}
