use crate::{ProviderAdapterError, ProviderInstance};
use executable::validate_executable_policy;
use fields::{optional_string_array, required_string, required_string_array, timeout_seconds};
use serde_json::Value;

mod executable;
mod fields;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalProcessCommandPolicy {
    executable: String,
    args: Vec<String>,
    env_allowlist: Vec<String>,
    timeout_seconds: u64,
}

impl LocalProcessCommandPolicy {
    pub fn from_instance(instance: &ProviderInstance) -> Result<Self, ProviderAdapterError> {
        let source = instance.path();
        let value = instance.value();
        let command = value
            .get("command")
            .ok_or_else(|| policy_denied(instance.id(), "command object is required"))?;
        let command_policy = value
            .get("command_policy")
            .ok_or_else(|| policy_denied(instance.id(), "command_policy object is required"))?;

        if command_policy
            .get("shell")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(policy_denied(instance.id(), "shell must be false"));
        }

        require_policy_string(command_policy, instance.id(), "network", "deny")?;
        require_policy_string(command_policy, instance.id(), "workspace_write", "deny")?;
        require_cwd_policy(command_policy, instance.id())?;

        let executable = required_string(command, source, "command.executable", "executable")?;
        let args = optional_string_array(command, instance.id(), "command.args", "args")?;
        let allowed_executables = required_string_array(
            command_policy,
            instance.id(),
            "command_policy.allowed_executables",
            "allowed_executables",
        )?;
        let env_allowlist = optional_string_array(
            command_policy,
            instance.id(),
            "command_policy.env_allowlist",
            "env_allowlist",
        )?;
        let timeout_seconds = timeout_seconds(value, instance.id())?;

        validate_executable_policy(instance.id(), &executable, &allowed_executables)?;

        Ok(Self {
            executable,
            args,
            env_allowlist,
            timeout_seconds,
        })
    }

    pub fn executable(&self) -> &str {
        &self.executable
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }

    pub fn env_allowlist(&self) -> &[String] {
        &self.env_allowlist
    }

    pub fn timeout_seconds(&self) -> u64 {
        self.timeout_seconds
    }
}

fn require_policy_string(
    command_policy: &Value,
    provider_instance_id: &str,
    field: &str,
    expected: &str,
) -> Result<(), ProviderAdapterError> {
    let Some(value) = command_policy.get(field) else {
        return Err(policy_denied(
            provider_instance_id,
            &format!("command_policy.{} must be {}", field, expected),
        ));
    };

    if value.as_str() == Some(expected) {
        return Ok(());
    }
    if expected == "deny" && value.as_bool() == Some(false) {
        return Ok(());
    }

    Err(policy_denied(
        provider_instance_id,
        &format!("command_policy.{} must be {}", field, expected),
    ))
}

fn require_cwd_policy(
    command_policy: &Value,
    provider_instance_id: &str,
) -> Result<(), ProviderAdapterError> {
    match command_policy.get("cwd_policy").and_then(Value::as_str) {
        None | Some("project_root") => Ok(()),
        Some(value) => Err(policy_denied(
            provider_instance_id,
            &format!("unsupported command_policy.cwd_policy {}", value),
        )),
    }
}

fn policy_denied(provider_instance_id: &str, reason: &str) -> ProviderAdapterError {
    ProviderAdapterError::CommandPolicyDenied {
        provider_instance_id: provider_instance_id.to_string(),
        reason: reason.to_string(),
    }
}
