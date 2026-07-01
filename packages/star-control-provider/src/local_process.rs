use crate::fake::{ensure_output_files_absent, provider_output_path};
use crate::{
    ExecutionRequest, ProviderAdapter, ProviderAdapterError, ProviderExecution, ProviderInstance,
    ProviderRunContext, ProviderRunResult,
};
use serde_json::{json, Value};
use star_control_state::ArtifactKind;
use std::fs::{self, File, OpenOptions};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const LOCAL_PROCESS_KIND: &str = "local_process_model";
const PROCESS_TRANSPORT: &str = "process";
const DEFAULT_TIMEOUT_SECONDS: u64 = 300;
const MAX_TIMEOUT_SECONDS: u64 = 600;
const STDOUT_FILE: &str = "stdout.txt";
const STDERR_FILE: &str = "stderr.txt";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalProcessProviderAdapter;

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

impl ProviderAdapter for LocalProcessProviderAdapter {
    fn execute(
        &self,
        request: &ExecutionRequest,
        context: &ProviderRunContext<'_>,
    ) -> Result<ProviderExecution, ProviderAdapterError> {
        let manifest = context
            .registry()
            .manifest_for_instance(request.provider_instance_id())?;
        if manifest.kind() != LOCAL_PROCESS_KIND || manifest.transport() != PROCESS_TRANSPORT {
            return Err(ProviderAdapterError::UnsupportedProvider {
                provider_instance_id: request.provider_instance_id().to_string(),
                provider_id: manifest.id().to_string(),
            });
        }

        let instance = context
            .registry()
            .instance(request.provider_instance_id())
            .ok_or_else(|| crate::ProviderRegistryError::InstanceNotFound {
                instance_id: request.provider_instance_id().to_string(),
            })?;
        let policy = LocalProcessCommandPolicy::from_instance(instance)?;

        let output_files = planned_output_files(request.provider_instance_id());
        ensure_output_files_absent(context.state_store(), request.job_id(), &output_files)?;

        let request_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            "request.json",
            request.value(),
        )?;
        let output_dir = context
            .state_store()
            .resolve_provider_output_dir(request.job_id(), request.provider_instance_id())?;
        fs::create_dir_all(&output_dir).map_err(|source| ProviderAdapterError::Io {
            path: output_dir.clone(),
            source,
        })?;

        let stdout_path = output_dir.join(STDOUT_FILE);
        let stderr_path = output_dir.join(STDERR_FILE);
        let stdout_file = create_new_output_file(&stdout_path)?;
        let stderr_file = create_new_output_file(&stderr_path)?;

        let process_result = run_process(&policy, context, stdout_file, stderr_file);
        let response_value = response_value(request, &policy, &process_result);
        let result = ProviderRunResult::from_value(
            response_value.clone(),
            provider_output_path(request.provider_instance_id(), "response.json"),
            context.schema_root(),
        )?;

        let stdout_ref = artifact_ref(
            context,
            request,
            &provider_output_path(request.provider_instance_id(), STDOUT_FILE),
        )?;
        let stderr_ref = artifact_ref(
            context,
            request,
            &provider_output_path(request.provider_instance_id(), STDERR_FILE),
        )?;
        let response_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            "response.json",
            &response_value,
        )?;

        Ok(ProviderExecution::new(
            result,
            request_ref,
            response_ref,
            stdout_ref,
            Some(stderr_ref),
        ))
    }
}

#[derive(Debug)]
enum LocalProcessRunResult {
    Exited { status: ExitStatus },
    TimedOut,
    LaunchFailed { message: String },
    WaitFailed { source: std::io::Error },
}

fn run_process(
    policy: &LocalProcessCommandPolicy,
    context: &ProviderRunContext<'_>,
    stdout_file: File,
    stderr_file: File,
) -> LocalProcessRunResult {
    let mut command = Command::new(policy.executable());
    command
        .args(policy.args())
        .current_dir(context.state_store().project_root())
        .env_clear()
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));

    for name in policy.env_allowlist() {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(source) => {
            return LocalProcessRunResult::LaunchFailed {
                message: source.to_string(),
            };
        }
    };

    let started_at = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return LocalProcessRunResult::Exited { status },
            Ok(None) => {
                if started_at.elapsed() >= Duration::from_secs(policy.timeout_seconds()) {
                    if let Err(source) = child.kill() {
                        return LocalProcessRunResult::WaitFailed { source };
                    }
                    if let Err(source) = child.wait() {
                        return LocalProcessRunResult::WaitFailed { source };
                    }
                    return LocalProcessRunResult::TimedOut;
                }
                thread::sleep(Duration::from_millis(25));
            }
            Err(source) => return LocalProcessRunResult::WaitFailed { source },
        }
    }
}

fn response_value(
    request: &ExecutionRequest,
    policy: &LocalProcessCommandPolicy,
    process_result: &LocalProcessRunResult,
) -> Value {
    let stdout_path = provider_output_path(request.provider_instance_id(), STDOUT_FILE);
    let stderr_path = provider_output_path(request.provider_instance_id(), STDERR_FILE);
    let response_path = provider_output_path(request.provider_instance_id(), "response.json");
    let (status, summary, error) = match process_result {
        LocalProcessRunResult::Exited { status } if status.success() => (
            "success",
            "local process completed with exit code 0".to_string(),
            Value::Null,
        ),
        LocalProcessRunResult::Exited { status } => {
            let exit_code = status.code();
            (
                "failed",
                format!(
                    "local process exited with code {}",
                    exit_code
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                ),
                json!({
                    "kind": "local_process_exit",
                    "exit_code": exit_code
                }),
            )
        }
        LocalProcessRunResult::TimedOut => (
            "timeout",
            format!(
                "local process timed out after {} second(s)",
                policy.timeout_seconds()
            ),
            json!({
                "kind": "local_process_timeout",
                "timeout_seconds": policy.timeout_seconds()
            }),
        ),
        LocalProcessRunResult::LaunchFailed { message } => (
            "error",
            "local process failed to launch".to_string(),
            json!({
                "kind": "local_process_launch_failed",
                "message": message
            }),
        ),
        LocalProcessRunResult::WaitFailed { source } => (
            "error",
            "local process wait failed".to_string(),
            json!({
                "kind": "local_process_wait_failed",
                "message": source.to_string()
            }),
        ),
    };

    json!({
        "schema_version": "1.0.0",
        "provider_instance_id": request.provider_instance_id(),
        "job_id": request.job_id(),
        "stage": request.stage(),
        "status": status,
        "started_at": request.created_at(),
        "finished_at": request.created_at(),
        "stdout_path": stdout_path,
        "stderr_path": stderr_path,
        "summary": summary,
        "changed_files": [],
        "artifacts": [
            response_path,
            stdout_path,
            stderr_path
        ],
        "metrics": {
            "estimated_cost": 0,
            "input_tokens": 0,
            "output_tokens": 0
        },
        "error": error
    })
}

fn planned_output_files(provider_instance_id: &str) -> Vec<String> {
    vec![
        provider_output_path(provider_instance_id, "request.json"),
        provider_output_path(provider_instance_id, STDOUT_FILE),
        provider_output_path(provider_instance_id, STDERR_FILE),
        provider_output_path(provider_instance_id, "response.json"),
    ]
}

fn artifact_ref(
    context: &ProviderRunContext<'_>,
    request: &ExecutionRequest,
    relative_path: &str,
) -> Result<Value, ProviderAdapterError> {
    Ok(context.state_store().artifact_ref(
        request.job_id(),
        relative_path,
        ArtifactKind::Log,
        request.provider_instance_id(),
        None,
        Some("provider text output"),
    )?)
}

fn create_new_output_file(path: &Path) -> Result<File, ProviderAdapterError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| ProviderAdapterError::Io {
            path: path.to_path_buf(),
            source,
        })
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

fn required_string(
    value: &Value,
    path: &Path,
    display_field: &str,
    field: &str,
) -> Result<String, ProviderAdapterError> {
    value
        .get(field)
        .ok_or_else(|| ProviderAdapterError::MissingField {
            path: path.to_path_buf(),
            field: display_field.to_string(),
        })?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| ProviderAdapterError::InvalidFieldType {
            path: path.to_path_buf(),
            field: display_field.to_string(),
            expected: "string".to_string(),
        })
}

fn required_string_array(
    value: &Value,
    provider_instance_id: &str,
    display_field: &str,
    field: &str,
) -> Result<Vec<String>, ProviderAdapterError> {
    let array = value.get(field).and_then(Value::as_array).ok_or_else(|| {
        policy_denied(
            provider_instance_id,
            &format!("{} must be an array of strings", display_field),
        )
    })?;
    if array.is_empty() {
        return Err(policy_denied(
            provider_instance_id,
            &format!("{} must not be empty", display_field),
        ));
    }
    strings_from_array(provider_instance_id, display_field, array)
}

fn optional_string_array(
    value: &Value,
    provider_instance_id: &str,
    display_field: &str,
    field: &str,
) -> Result<Vec<String>, ProviderAdapterError> {
    let Some(array) = value.get(field) else {
        return Ok(Vec::new());
    };
    let Some(array) = array.as_array() else {
        return Err(policy_denied(
            provider_instance_id,
            &format!("{} must be an array of strings", display_field),
        ));
    };
    strings_from_array(provider_instance_id, display_field, array)
}

fn strings_from_array(
    provider_instance_id: &str,
    display_field: &str,
    array: &[Value],
) -> Result<Vec<String>, ProviderAdapterError> {
    array
        .iter()
        .map(|value| {
            value.as_str().map(str::to_string).ok_or_else(|| {
                policy_denied(
                    provider_instance_id,
                    &format!("{} must be an array of strings", display_field),
                )
            })
        })
        .collect()
}

fn timeout_seconds(value: &Value, provider_instance_id: &str) -> Result<u64, ProviderAdapterError> {
    let timeout_seconds = value
        .pointer("/limits/timeout_seconds")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_TIMEOUT_SECONDS);
    if timeout_seconds > MAX_TIMEOUT_SECONDS {
        return Err(policy_denied(
            provider_instance_id,
            &format!("limits.timeout_seconds must be <= {}", MAX_TIMEOUT_SECONDS),
        ));
    }
    Ok(timeout_seconds)
}

fn validate_executable_policy(
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

fn policy_denied(provider_instance_id: &str, reason: &str) -> ProviderAdapterError {
    ProviderAdapterError::CommandPolicyDenied {
        provider_instance_id: provider_instance_id.to_string(),
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProviderManifest, ProviderRegistry, ProviderRegistryError, ProviderRunContext};
    use star_control_state::StateStore;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn local_process_executes_allowlisted_command_and_captures_output() {
        let executable = current_test_executable();
        let (execution, project) = execute_with_command(
            &executable,
            vec!["--help".to_string()],
            vec![executable.clone()],
            Vec::new(),
            10,
        )
        .expect("execute local process");

        assert_eq!(execution.result().status(), "success");
        assert_eq!(
            execution.result().value()["stdout_path"],
            "provider-output/local-default/stdout.txt"
        );
        assert_eq!(
            execution.result().value()["stderr_path"],
            "provider-output/local-default/stderr.txt"
        );
        assert!(execution.stderr_ref().is_some());

        assert!(project
            .join(".ai-runs/J-0001/provider-output/local-default/request.json")
            .is_file());
        assert!(project
            .join(".ai-runs/J-0001/provider-output/local-default/stdout.txt")
            .is_file());
        assert!(project
            .join(".ai-runs/J-0001/provider-output/local-default/stderr.txt")
            .is_file());
        assert!(project
            .join(".ai-runs/J-0001/provider-output/local-default/response.json")
            .is_file());
        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn local_process_rejects_shell_wrapper() {
        let error = execute_with_command(
            shell_wrapper_name(),
            Vec::new(),
            vec![shell_wrapper_name().to_string()],
            Vec::new(),
            10,
        )
        .expect_err("shell wrapper should be rejected");

        assert!(matches!(
            error,
            ProviderAdapterError::CommandPolicyDenied { .. }
        ));
    }

    #[test]
    fn local_process_rejects_executable_outside_allowlist() {
        let executable = current_test_executable();
        let error = execute_with_command(
            &executable,
            Vec::new(),
            vec!["other-runner".to_string()],
            Vec::new(),
            10,
        )
        .expect_err("executable outside allowlist should be rejected");

        assert!(matches!(
            error,
            ProviderAdapterError::CommandPolicyDenied { .. }
        ));
    }

    #[test]
    fn local_process_timeout_writes_timeout_result() {
        let executable = current_test_executable();
        std::env::set_var("STAR_CONTROL_LOCAL_PROCESS_SLEEP_HELPER", "1");
        let (execution, project) = execute_with_command(
            &executable,
            vec![
                "--exact".to_string(),
                "local_process::tests::local_process_sleep_helper".to_string(),
                "--nocapture".to_string(),
            ],
            vec![executable.clone()],
            vec!["STAR_CONTROL_LOCAL_PROCESS_SLEEP_HELPER".to_string()],
            1,
        )
        .expect("execute timeout helper");
        std::env::remove_var("STAR_CONTROL_LOCAL_PROCESS_SLEEP_HELPER");

        assert_eq!(execution.result().status(), "timeout");
        assert_eq!(
            execution.result().value()["error"]["kind"],
            "local_process_timeout"
        );

        assert!(project
            .join(".ai-runs/J-0001/provider-output/local-default/stdout.txt")
            .is_file());
        assert!(project
            .join(".ai-runs/J-0001/provider-output/local-default/stderr.txt")
            .is_file());
        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn local_process_sleep_helper() {
        let is_child_helper = std::env::args().collect::<Vec<_>>().windows(2).any(|args| {
            args[0] == "--exact" && args[1] == "local_process::tests::local_process_sleep_helper"
        });
        if is_child_helper && std::env::var("STAR_CONTROL_LOCAL_PROCESS_SLEEP_HELPER").is_ok() {
            thread::sleep(Duration::from_secs(5));
        }
    }

    fn execute_with_command(
        executable: &str,
        args: Vec<String>,
        allowed_executables: Vec<String>,
        env_allowlist: Vec<String>,
        timeout_seconds: u64,
    ) -> Result<(ProviderExecution, PathBuf), ProviderAdapterError> {
        let project = temp_project();
        let store = open_store(&project);
        store
            .create_job("implement local process feature", "codex", vec![])
            .expect("create job");
        let registry = registry_with_instance(
            executable,
            args,
            allowed_executables,
            env_allowlist,
            timeout_seconds,
        )
        .expect("registry");
        let request = ExecutionRequest::from_value(request_value(), "request.json", schema_root())
            .expect("request");
        let schemas = schema_root();
        let context = ProviderRunContext::new(&registry, &store, &schemas);
        match LocalProcessProviderAdapter.execute(&request, &context) {
            Ok(execution) => Ok((execution, project)),
            Err(error) => {
                fs::remove_dir_all(project).ok();
                Err(error)
            }
        }
    }

    fn registry_with_instance(
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

    fn request_value() -> Value {
        json!({
            "schema_version": "1.0.0",
            "request_id": "request-0001",
            "job_id": "J-0001",
            "stage": "implement",
            "provider_instance_id": "local-default",
            "attempt_id": "attempt-0001",
            "workspec_path": "workspecs/implement.json",
            "created_at": "2026-06-28T00:00:00Z",
            "goal": "run local process provider",
            "allowed_scope": ["src/**", "tests/**"],
            "forbidden_actions": ["dependency_install", "file_delete"],
            "required_outputs": ["provider-output/local-default/response.json"],
            "validation_requirements": ["policy:p0"],
            "context_pack": { "files": [] }
        })
    }

    fn current_test_executable() -> String {
        std::env::current_exe()
            .expect("current test executable")
            .display()
            .to_string()
    }

    fn shell_wrapper_name() -> &'static str {
        if cfg!(windows) {
            "cmd.exe"
        } else {
            "sh"
        }
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("packages dir")
            .parent()
            .expect("repo root")
            .to_path_buf()
    }

    fn schema_root() -> PathBuf {
        repo_root().join("specs").join("schemas")
    }

    fn temp_project() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "star-control-provider-local-process-{}-{}",
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(&path).expect("create temp project");
        path
    }

    fn open_store(project_root: &Path) -> StateStore {
        StateStore::open(project_root, schema_root()).expect("open state store")
    }
}
