use super::policy_denied;
use crate::ProviderAdapterError;
use std::path::Path;

pub(super) fn validate_executable_policy(
    provider_instance_id: &str,
    executable: &str,
    allowed_executables: &[String],
) -> Result<(), ProviderAdapterError> {
    let executable_lower = executable.to_ascii_lowercase();
    let executable_file_name = Path::new(executable)
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .unwrap_or(executable)
        .to_ascii_lowercase();

    if looks_like_shell_command_string(&executable_lower) {
        return Err(policy_denied(
            provider_instance_id,
            "executable must not be a shell command string",
        ));
    }

    if is_shell_wrapper(&executable_file_name) {
        return Err(policy_denied(
            provider_instance_id,
            "shell wrapper executable is forbidden",
        ));
    }

    if is_forbidden_action_executable(&executable_file_name) {
        return Err(policy_denied(
            provider_instance_id,
            "approval-required executable category is forbidden",
        ));
    }

    let allowed = allowed_executables.iter().any(|allowed| {
        if has_path_separator(allowed) {
            allowed == executable
        } else {
            allowed.eq_ignore_ascii_case(&executable_file_name)
        }
    });
    if !allowed {
        return Err(policy_denied(
            provider_instance_id,
            "executable is not in command_policy.allowed_executables",
        ));
    }

    Ok(())
}

fn looks_like_shell_command_string(executable_lower: &str) -> bool {
    [
        "cmd ",
        "cmd.exe ",
        "powershell ",
        "powershell.exe ",
        "pwsh ",
        "pwsh.exe ",
        "sh ",
        "bash ",
    ]
    .iter()
    .any(|prefix| executable_lower.starts_with(prefix))
}

fn is_shell_wrapper(file_name_lower: &str) -> bool {
    matches!(
        file_name_lower,
        "cmd"
            | "cmd.exe"
            | "powershell"
            | "powershell.exe"
            | "pwsh"
            | "pwsh.exe"
            | "sh"
            | "bash"
            | "zsh"
            | "fish"
    )
}

fn is_forbidden_action_executable(file_name_lower: &str) -> bool {
    matches!(
        file_name_lower,
        "npm"
            | "npm.cmd"
            | "pnpm"
            | "pnpm.cmd"
            | "yarn"
            | "yarn.cmd"
            | "bun"
            | "bun.exe"
            | "cargo"
            | "cargo.exe"
            | "pip"
            | "pip.exe"
            | "pip3"
            | "pip3.exe"
            | "poetry"
            | "poetry.exe"
            | "uv"
            | "uv.exe"
            | "git"
            | "git.exe"
            | "gh"
            | "gh.exe"
            | "kubectl"
            | "kubectl.exe"
            | "terraform"
            | "terraform.exe"
            | "rm"
            | "rmdir"
    )
}

fn has_path_separator(value: &str) -> bool {
    value.contains('/') || value.contains('\\')
}
