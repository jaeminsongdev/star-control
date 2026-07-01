use crate::fake::{ensure_output_files_absent, provider_output_path};
use crate::{
    ExecutionRequest, ProviderAdapter, ProviderAdapterError, ProviderExecution, ProviderInstance,
    ProviderManifest, ProviderRunContext, ProviderRunResult,
};
use serde_json::{json, Value};
use star_control_schema::{load_schema, validate_json};
use star_control_state::ArtifactKind;
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const CLOUD_CLI_KIND: &str = "cloud_cli_agent";
const CLOUD_API_KIND: &str = "cloud_api_model";
const CLI_TRANSPORT: &str = "cli";
const HTTP_TRANSPORT: &str = "http";
const STDOUT_FILE: &str = "stdout.txt";
const STDERR_FILE: &str = "stderr.txt";
const PRIVACY_HANDOFF_FILE: &str = "privacy-handoff.json";
const COST_METRIC_FILE: &str = "cost-metric.json";
const PRIVACY_HANDOFF_SCHEMA: &str = "privacy-handoff.schema.json";
const COST_METRIC_SCHEMA: &str = "cost-metric.schema.json";
const DEFAULT_TIMEOUT_SECONDS: u64 = 300;
const MAX_TIMEOUT_SECONDS: u64 = 1800;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CloudCliProviderAdapter;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CloudProviderPreflightAdapter;

pub fn is_cloud_provider_manifest(manifest: &ProviderManifest) -> bool {
    (manifest.kind() == CLOUD_CLI_KIND && manifest.transport() == CLI_TRANSPORT)
        || (manifest.kind() == CLOUD_API_KIND && manifest.transport() == HTTP_TRANSPORT)
}

pub fn is_cloud_cli_manifest(manifest: &ProviderManifest) -> bool {
    manifest.kind() == CLOUD_CLI_KIND && manifest.transport() == CLI_TRANSPORT
}

impl ProviderAdapter for CloudCliProviderAdapter {
    fn execute(
        &self,
        request: &ExecutionRequest,
        context: &ProviderRunContext<'_>,
    ) -> Result<ProviderExecution, ProviderAdapterError> {
        let manifest = context
            .registry()
            .manifest_for_instance(request.provider_instance_id())?;
        if !is_cloud_cli_manifest(manifest) {
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
        let decision = CloudProviderPolicyDecision::evaluate(manifest, instance);
        if !decision.allows_transport_execution() {
            return CloudProviderPreflightAdapter.execute(request, context);
        }

        let policy = CloudCliCommandPolicy::from_instance(instance)?;

        ensure_output_files_absent(
            context.state_store(),
            request.job_id(),
            &planned_output_files(request.provider_instance_id()),
        )?;

        let request_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            "request.json",
            request.value(),
        )?;
        let privacy_handoff = privacy_handoff_value(request, manifest, true);
        validate_contract(
            &privacy_handoff,
            Path::new(PRIVACY_HANDOFF_FILE),
            context.schema_root(),
            PRIVACY_HANDOFF_SCHEMA,
        )?;
        let privacy_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            PRIVACY_HANDOFF_FILE,
            &privacy_handoff,
        )?;

        let output_dir = context
            .state_store()
            .resolve_provider_output_dir(request.job_id(), request.provider_instance_id())?;
        std::fs::create_dir_all(&output_dir).map_err(|source| ProviderAdapterError::Io {
            path: output_dir.clone(),
            source,
        })?;
        let stdout_path = output_dir.join(STDOUT_FILE);
        let stderr_path = output_dir.join(STDERR_FILE);
        let stdout_file = create_new_output_file(&stdout_path)?;
        let stderr_file = create_new_output_file(&stderr_path)?;

        let started_at = Instant::now();
        let process_result = run_cloud_cli_process(
            &policy,
            request,
            context,
            &request_ref,
            stdout_file,
            stderr_file,
        )?;
        let wall_time_ms = started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;

        let stdout_ref = artifact_ref(context, request, STDOUT_FILE)?;
        let stderr_ref = artifact_ref(context, request, STDERR_FILE)?;
        let cost_metric = cost_metric_value_with_wall_time(request, instance, wall_time_ms);
        validate_contract(
            &cost_metric,
            Path::new(COST_METRIC_FILE),
            context.schema_root(),
            COST_METRIC_SCHEMA,
        )?;
        let cost_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            COST_METRIC_FILE,
            &cost_metric,
        )?;

        let response_value =
            cli_response_value(request, manifest, instance, &process_result, wall_time_ms);
        let result = ProviderRunResult::from_value(
            response_value.clone(),
            provider_output_path(request.provider_instance_id(), "response.json"),
            context.schema_root(),
        )?;
        let response_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            "response.json",
            &response_value,
        )?;

        let execution = ProviderExecution::new(
            result,
            request_ref,
            response_ref,
            stdout_ref,
            Some(stderr_ref),
        );
        assert_provider_sidecar_refs(&execution, &privacy_ref, &cost_ref);
        Ok(execution)
    }
}

impl ProviderAdapter for CloudProviderPreflightAdapter {
    fn execute(
        &self,
        request: &ExecutionRequest,
        context: &ProviderRunContext<'_>,
    ) -> Result<ProviderExecution, ProviderAdapterError> {
        let manifest = context
            .registry()
            .manifest_for_instance(request.provider_instance_id())?;
        if !is_cloud_provider_manifest(manifest) {
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
        let decision = CloudProviderPolicyDecision::evaluate(manifest, instance);

        ensure_output_files_absent(
            context.state_store(),
            request.job_id(),
            &planned_output_files(request.provider_instance_id()),
        )?;

        let request_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            "request.json",
            request.value(),
        )?;
        let privacy_handoff = privacy_handoff_value(request, manifest, decision.privacy_approved);
        validate_contract(
            &privacy_handoff,
            Path::new(PRIVACY_HANDOFF_FILE),
            context.schema_root(),
            PRIVACY_HANDOFF_SCHEMA,
        )?;
        let privacy_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            PRIVACY_HANDOFF_FILE,
            &privacy_handoff,
        )?;

        let cost_metric = cost_metric_value(request, instance);
        validate_contract(
            &cost_metric,
            Path::new(COST_METRIC_FILE),
            context.schema_root(),
            COST_METRIC_SCHEMA,
        )?;
        let cost_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            COST_METRIC_FILE,
            &cost_metric,
        )?;

        let stdout_ref = context.state_store().write_provider_text(
            request.job_id(),
            request.provider_instance_id(),
            STDOUT_FILE,
            &stdout_value(manifest, &decision),
        )?;
        let stderr_ref = context.state_store().write_provider_text(
            request.job_id(),
            request.provider_instance_id(),
            STDERR_FILE,
            &stderr_value(&decision),
        )?;

        let response_value = response_value(request, manifest, instance, &decision);
        let result = ProviderRunResult::from_value(
            response_value.clone(),
            provider_output_path(request.provider_instance_id(), "response.json"),
            context.schema_root(),
        )?;
        let response_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            "response.json",
            &response_value,
        )?;

        let execution = ProviderExecution::new(
            result,
            request_ref,
            response_ref,
            stdout_ref,
            Some(stderr_ref),
        );
        assert_provider_sidecar_refs(&execution, &privacy_ref, &cost_ref);
        Ok(execution)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CloudProviderPolicyDecision {
    privacy_approved: bool,
    credential_ref_present: bool,
    auth_mode_login_session: bool,
    block: CloudProviderBlock,
}

impl CloudProviderPolicyDecision {
    fn evaluate(manifest: &ProviderManifest, instance: &ProviderInstance) -> Self {
        let privacy_approved = bool_pointer(
            instance.value(),
            "/transport_config/privacy_handoff_approved",
        )
        .unwrap_or(false);
        let credential_ref = string_field(instance.value(), "credential_ref");
        let credential_ref_present = credential_ref.is_some();
        let auth_mode_login_session =
            string_pointer(instance.value(), "/transport_config/auth_mode")
                == Some("login_session");

        let block = if let Some(field) = raw_credential_field(instance.value()) {
            CloudProviderBlock::new(
                "cloud_provider_raw_credential",
                "cloud provider instance contains a raw credential-like field",
                Some(field),
            )
        } else if let Some(value) = credential_ref {
            if !is_allowed_credential_ref(value) {
                CloudProviderBlock::new(
                    "cloud_provider_credential_ref_invalid",
                    "credential_ref must use an allowed reference prefix",
                    Some("credential_ref".to_string()),
                )
            } else if !privacy_approved {
                CloudProviderBlock::new(
                    "cloud_privacy_handoff_unapproved",
                    "cloud provider handoff requires explicit privacy approval",
                    Some("transport_config.privacy_handoff_approved".to_string()),
                )
            } else {
                CloudProviderBlock::transport_not_implemented()
            }
        } else if manifest.kind() == CLOUD_API_KIND {
            CloudProviderBlock::new(
                "cloud_api_credential_ref_required",
                "cloud API provider requires credential_ref and never accepts raw credential values",
                Some("credential_ref".to_string()),
            )
        } else if !auth_mode_login_session {
            CloudProviderBlock::new(
                "cloud_cli_auth_reference_required",
                "cloud CLI provider requires credential_ref or transport_config.auth_mode=login_session",
                Some("credential_ref".to_string()),
            )
        } else if !privacy_approved {
            CloudProviderBlock::new(
                "cloud_privacy_handoff_unapproved",
                "cloud provider handoff requires explicit privacy approval",
                Some("transport_config.privacy_handoff_approved".to_string()),
            )
        } else {
            CloudProviderBlock::transport_not_implemented()
        };

        Self {
            privacy_approved,
            credential_ref_present,
            auth_mode_login_session,
            block,
        }
    }

    fn allows_transport_execution(&self) -> bool {
        self.block.kind == "cloud_provider_transport_not_implemented"
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CloudProviderBlock {
    kind: String,
    message: String,
    field: Option<String>,
}

impl CloudProviderBlock {
    fn new(kind: &str, message: &str, field: Option<String>) -> Self {
        Self {
            kind: kind.to_string(),
            message: message.to_string(),
            field,
        }
    }

    fn transport_not_implemented() -> Self {
        Self::new(
            "cloud_provider_transport_not_implemented",
            "cloud provider preflight passed, but transport execution is reserved for the next M6 slice",
            None,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CloudCliCommandPolicy {
    executable: String,
    args: Vec<String>,
    env_allowlist: Vec<String>,
    timeout_seconds: u64,
}

impl CloudCliCommandPolicy {
    fn from_instance(instance: &ProviderInstance) -> Result<Self, ProviderAdapterError> {
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

    fn executable(&self) -> &str {
        &self.executable
    }

    fn timeout_seconds(&self) -> u64 {
        self.timeout_seconds
    }

    fn rendered_args(&self, request: &ExecutionRequest, request_ref: &Value) -> Vec<String> {
        self.args
            .iter()
            .map(|arg| render_arg(arg, request, request_ref))
            .collect()
    }
}

#[derive(Debug)]
enum CloudCliRunResult {
    Exited { status: ExitStatus },
    TimedOut,
    LaunchFailed { message: String },
    WaitFailed { source: std::io::Error },
}

fn response_value(
    request: &ExecutionRequest,
    manifest: &ProviderManifest,
    instance: &ProviderInstance,
    decision: &CloudProviderPolicyDecision,
) -> Value {
    json!({
        "schema_version": "1.0.0",
        "provider_instance_id": request.provider_instance_id(),
        "job_id": request.job_id(),
        "stage": request.stage(),
        "status": "blocked",
        "started_at": request.created_at(),
        "finished_at": request.created_at(),
        "stdout_path": provider_output_path(request.provider_instance_id(), STDOUT_FILE),
        "stderr_path": provider_output_path(request.provider_instance_id(), STDERR_FILE),
        "summary": format!("cloud provider preflight blocked: {}", decision.block.message),
        "changed_files": [],
        "artifacts": [
            provider_output_path(request.provider_instance_id(), "response.json"),
            provider_output_path(request.provider_instance_id(), STDOUT_FILE),
            provider_output_path(request.provider_instance_id(), STDERR_FILE),
            provider_output_path(request.provider_instance_id(), PRIVACY_HANDOFF_FILE),
            provider_output_path(request.provider_instance_id(), COST_METRIC_FILE)
        ],
        "metrics": {
            "estimated_cost": estimated_cost(instance),
            "currency": currency(instance),
            "input_tokens": 0,
            "output_tokens": 0,
            "wall_time_ms": 0,
            "credential_ref_present": decision.credential_ref_present,
            "auth_mode_login_session": decision.auth_mode_login_session,
            "privacy_handoff_approved": decision.privacy_approved
        },
        "error": {
            "kind": decision.block.kind,
            "message": decision.block.message,
            "field": decision.block.field,
            "provider_id": manifest.id(),
            "provider_kind": manifest.kind(),
            "transport": manifest.transport()
        }
    })
}

fn cli_response_value(
    request: &ExecutionRequest,
    manifest: &ProviderManifest,
    instance: &ProviderInstance,
    process_result: &CloudCliRunResult,
    wall_time_ms: u64,
) -> Value {
    let stdout_path = provider_output_path(request.provider_instance_id(), STDOUT_FILE);
    let stderr_path = provider_output_path(request.provider_instance_id(), STDERR_FILE);
    let response_path = provider_output_path(request.provider_instance_id(), "response.json");
    let privacy_path = provider_output_path(request.provider_instance_id(), PRIVACY_HANDOFF_FILE);
    let cost_path = provider_output_path(request.provider_instance_id(), COST_METRIC_FILE);
    let (status, summary, error, exit_code) = match process_result {
        CloudCliRunResult::Exited { status } if status.success() => (
            "success",
            "cloud CLI provider completed with exit code 0".to_string(),
            Value::Null,
            status.code(),
        ),
        CloudCliRunResult::Exited { status } => {
            let exit_code = status.code();
            (
                "failed",
                format!(
                    "cloud CLI provider exited with code {}",
                    exit_code
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                ),
                json!({
                    "kind": "cloud_cli_exit",
                    "exit_code": exit_code,
                    "provider_id": manifest.id()
                }),
                exit_code,
            )
        }
        CloudCliRunResult::TimedOut => (
            "timeout",
            "cloud CLI provider timed out".to_string(),
            json!({
                "kind": "cloud_cli_timeout",
                "provider_id": manifest.id()
            }),
            None,
        ),
        CloudCliRunResult::LaunchFailed { message } => (
            "error",
            "cloud CLI provider failed to launch".to_string(),
            json!({
                "kind": "cloud_cli_launch_failed",
                "message": message,
                "provider_id": manifest.id()
            }),
            None,
        ),
        CloudCliRunResult::WaitFailed { source } => (
            "error",
            "cloud CLI provider wait failed".to_string(),
            json!({
                "kind": "cloud_cli_wait_failed",
                "message": source.to_string(),
                "provider_id": manifest.id()
            }),
            None,
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
            stderr_path,
            privacy_path,
            cost_path
        ],
        "metrics": {
            "estimated_cost": estimated_cost(instance),
            "currency": currency(instance),
            "input_tokens": 0,
            "output_tokens": 0,
            "wall_time_ms": wall_time_ms,
            "exit_code": exit_code,
            "transport": CLI_TRANSPORT
        },
        "error": error
    })
}

fn privacy_handoff_value(
    request: &ExecutionRequest,
    manifest: &ProviderManifest,
    approved: bool,
) -> Value {
    json!({
        "schema_version": "1.0.0",
        "job_id": request.job_id(),
        "destination": manifest.id(),
        "context_paths": [
            request.workspec_path(),
            provider_output_path(request.provider_instance_id(), "request.json")
        ],
        "redaction_required": true,
        "approved": approved,
        "notes": "Cloud provider preflight records handoff scope before any external transport execution."
    })
}

fn cost_metric_value(request: &ExecutionRequest, instance: &ProviderInstance) -> Value {
    cost_metric_value_with_wall_time(request, instance, 0)
}

fn cost_metric_value_with_wall_time(
    request: &ExecutionRequest,
    instance: &ProviderInstance,
    wall_time_ms: u64,
) -> Value {
    json!({
        "schema_version": "1.0.0",
        "job_id": request.job_id(),
        "stage": request.stage(),
        "provider_instance_id": request.provider_instance_id(),
        "input_tokens": 0,
        "output_tokens": 0,
        "estimated_cost": estimated_cost(instance),
        "currency": currency(instance),
        "wall_time_ms": wall_time_ms,
        "quota_remaining": null
    })
}

fn stdout_value(manifest: &ProviderManifest, decision: &CloudProviderPolicyDecision) -> String {
    format!(
        "cloud provider preflight\nprovider_id={}\nkind={}\ntransport={}\ncredential_ref_present={}\nauth_mode_login_session={}\nprivacy_handoff_approved={}\ntransport_execution=false\n",
        manifest.id(),
        manifest.kind(),
        manifest.transport(),
        decision.credential_ref_present,
        decision.auth_mode_login_session,
        decision.privacy_approved,
    )
}

fn stderr_value(decision: &CloudProviderPolicyDecision) -> String {
    format!(
        "blocked kind={} field={} message={}\n",
        decision.block.kind,
        decision.block.field.as_deref().unwrap_or(""),
        decision.block.message
    )
}

fn planned_output_files(provider_instance_id: &str) -> Vec<String> {
    vec![
        provider_output_path(provider_instance_id, "request.json"),
        provider_output_path(provider_instance_id, "response.json"),
        provider_output_path(provider_instance_id, STDOUT_FILE),
        provider_output_path(provider_instance_id, STDERR_FILE),
        provider_output_path(provider_instance_id, PRIVACY_HANDOFF_FILE),
        provider_output_path(provider_instance_id, COST_METRIC_FILE),
    ]
}

fn run_cloud_cli_process(
    policy: &CloudCliCommandPolicy,
    request: &ExecutionRequest,
    context: &ProviderRunContext<'_>,
    request_ref: &Value,
    stdout_file: File,
    stderr_file: File,
) -> Result<CloudCliRunResult, ProviderAdapterError> {
    let mut command = Command::new(policy.executable());
    command
        .args(policy.rendered_args(request, request_ref))
        .current_dir(context.state_store().project_root())
        .env_clear()
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));

    for name in &policy.env_allowlist {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(source) => {
            return Ok(CloudCliRunResult::LaunchFailed {
                message: source.to_string(),
            });
        }
    };

    wait_for_cloud_cli_child(&mut child, policy.timeout_seconds())
}

fn wait_for_cloud_cli_child(
    child: &mut Child,
    timeout_seconds: u64,
) -> Result<CloudCliRunResult, ProviderAdapterError> {
    let started_at = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(CloudCliRunResult::Exited { status }),
            Ok(None) => {
                if started_at.elapsed() >= Duration::from_secs(timeout_seconds) {
                    if let Err(source) = child.kill() {
                        return Ok(CloudCliRunResult::WaitFailed { source });
                    }
                    if let Err(source) = child.wait() {
                        return Ok(CloudCliRunResult::WaitFailed { source });
                    }
                    return Ok(CloudCliRunResult::TimedOut);
                }
                thread::sleep(Duration::from_millis(25));
            }
            Err(source) => return Ok(CloudCliRunResult::WaitFailed { source }),
        }
    }
}

fn artifact_ref(
    context: &ProviderRunContext<'_>,
    request: &ExecutionRequest,
    file_name: &str,
) -> Result<Value, ProviderAdapterError> {
    Ok(context.state_store().artifact_ref(
        request.job_id(),
        &provider_output_path(request.provider_instance_id(), file_name),
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

fn render_arg(arg: &str, request: &ExecutionRequest, request_ref: &Value) -> String {
    let request_path = request_ref
        .get("path")
        .and_then(Value::as_str)
        .map(|path| format!(".ai-runs/{}/{}", request.job_id(), path))
        .unwrap_or_else(|| request.workspec_path().to_string());
    arg.replace("{{request_path}}", &request_path)
        .replace("{{job_id}}", request.job_id())
        .replace("{{stage}}", request.stage())
        .replace("{{goal}}", request.goal())
}

fn raw_credential_field(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if is_raw_credential_key(key) && child.as_str().is_some_and(|text| !text.is_empty())
                {
                    return Some(key.to_string());
                }
                if let Some(field) = raw_credential_field(child) {
                    return Some(format!("{}.{}", key, field));
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(raw_credential_field),
        _ => None,
    }
}

fn is_raw_credential_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| *character != '-' && *character != '_')
        .collect::<String>()
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "apikey"
            | "token"
            | "accesstoken"
            | "refreshtoken"
            | "secret"
            | "password"
            | "credential"
            | "credentials"
            | "bearertoken"
    )
}

fn is_allowed_credential_ref(value: &str) -> bool {
    const ALLOWED_PREFIXES: &[&str] = &["env:", "keychain:", "secret-manager:", "login-session:"];
    ALLOWED_PREFIXES.iter().any(|prefix| {
        value
            .strip_prefix(prefix)
            .is_some_and(|suffix| !suffix.is_empty())
    })
}

fn estimated_cost(instance: &ProviderInstance) -> f64 {
    number_pointer(instance.value(), "/budget/estimated_cost").unwrap_or(0.0)
}

fn currency(instance: &ProviderInstance) -> String {
    string_pointer(instance.value(), "/budget/currency")
        .unwrap_or("USD")
        .to_string()
}

fn string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn string_pointer<'a>(value: &'a Value, pointer: &str) -> Option<&'a str> {
    value.pointer(pointer).and_then(Value::as_str)
}

fn bool_pointer(value: &Value, pointer: &str) -> Option<bool> {
    value.pointer(pointer).and_then(Value::as_bool)
}

fn number_pointer(value: &Value, pointer: &str) -> Option<f64> {
    value.pointer(pointer).and_then(Value::as_f64)
}

fn required_string(
    value: &Value,
    path: &Path,
    field_path: &str,
    field: &str,
) -> Result<String, ProviderAdapterError> {
    value
        .get(field)
        .ok_or_else(|| ProviderAdapterError::MissingField {
            path: path.to_path_buf(),
            field: field_path.to_string(),
        })?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| ProviderAdapterError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field_path.to_string(),
            expected: "string".to_string(),
        })
}

fn optional_string_array(
    value: &Value,
    provider_instance_id: &str,
    field_path: &str,
    field: &str,
) -> Result<Vec<String>, ProviderAdapterError> {
    let Some(array) = value.get(field) else {
        return Ok(Vec::new());
    };
    let Some(items) = array.as_array() else {
        return Err(cloud_policy_denied(
            provider_instance_id,
            &format!("{} must be an array", field_path),
        ));
    };

    items
        .iter()
        .map(|item| {
            item.as_str().map(str::to_string).ok_or_else(|| {
                cloud_policy_denied(
                    provider_instance_id,
                    &format!("{} must contain strings", field_path),
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

fn cloud_policy_denied(provider_instance_id: &str, reason: &str) -> ProviderAdapterError {
    ProviderAdapterError::CommandPolicyDenied {
        provider_instance_id: provider_instance_id.to_string(),
        reason: reason.to_string(),
    }
}

fn validate_contract(
    value: &Value,
    path: &Path,
    schema_root: &Path,
    schema_file: &str,
) -> Result<(), ProviderAdapterError> {
    let schema_path = schema_root.join(schema_file);
    let schema =
        load_schema(&schema_path).map_err(|source| ProviderAdapterError::SchemaLoadFailed {
            path: schema_path.clone(),
            message: source.to_string(),
        })?;
    let result = validate_json(value, &schema);
    if result.is_ok() {
        Ok(())
    } else {
        Err(ProviderAdapterError::SchemaValidationFailed {
            path: path.to_path_buf(),
            schema_path,
            errors: result.errors,
        })
    }
}

fn assert_provider_sidecar_refs(
    _execution: &ProviderExecution,
    privacy_ref: &Value,
    cost_ref: &Value,
) {
    debug_assert_eq!(privacy_ref["kind"], "provider_output");
    debug_assert_eq!(cost_ref["kind"], "provider_output");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ProviderConformanceChecker, ProviderConformanceProfile, ProviderRegistry,
        ProviderRegistryError,
    };
    use serde_json::json;
    use star_control_state::StateStore;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Mutex, MutexGuard};
    use std::time::{SystemTime, UNIX_EPOCH};

    static ENV_LOCK: Mutex<()> = Mutex::new(());
    static TEMP_PROJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn cloud_cli_preflight_writes_privacy_and_cost_artifacts() {
        let (execution, project) = execute_cloud_provider(
            CLOUD_CLI_KIND,
            CLI_TRANSPORT,
            json!({
                "id": "cloud-default",
                "provider": "provider.cloud",
                "enabled": true,
                "limits": {
                    "timeout_seconds": 300,
                    "max_parallel_jobs": 1
                },
                "routing_tags": ["cloud", "cli"],
                "transport_config": {
                    "auth_mode": "login_session",
                    "privacy_handoff_approved": true
                },
                "budget": {
                    "estimated_cost": 0.25,
                    "currency": "USD"
                },
                "command": {
                    "executable": "cloud-agent"
                }
            }),
        )
        .expect("execute cloud preflight");

        assert_eq!(execution.result().status(), "blocked");
        assert_eq!(
            execution.result().value()["error"]["kind"],
            "cloud_provider_transport_not_implemented"
        );
        assert_eq!(
            execution.result().value()["metrics"]["privacy_handoff_approved"],
            true
        );
        assert!(project
            .join(".ai-runs/J-0001/provider-output/cloud-default/privacy-handoff.json")
            .is_file());
        let cost_metric = read_json(
            &project.join(".ai-runs/J-0001/provider-output/cloud-default/cost-metric.json"),
        );
        assert_eq!(cost_metric["estimated_cost"], 0.25);
        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn cloud_api_preflight_requires_credential_ref() {
        let (execution, project) = execute_cloud_provider(
            CLOUD_API_KIND,
            HTTP_TRANSPORT,
            json!({
                "id": "cloud-default",
                "provider": "provider.cloud",
                "enabled": true,
                "limits": {
                    "timeout_seconds": 300,
                    "max_parallel_jobs": 1
                },
                "routing_tags": ["cloud", "api"],
                "transport_config": {
                    "privacy_handoff_approved": true
                },
                "endpoint": {
                    "base_url": "https://api.example.invalid/v1"
                }
            }),
        )
        .expect("execute cloud preflight");

        assert_eq!(execution.result().status(), "blocked");
        assert_eq!(
            execution.result().value()["error"]["kind"],
            "cloud_api_credential_ref_required"
        );
        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn cloud_preflight_blocks_raw_credential_without_echoing_value() {
        let raw_secret = "sk-raw-secret-value";
        let (execution, project) = execute_cloud_provider(
            CLOUD_API_KIND,
            HTTP_TRANSPORT,
            json!({
                "id": "cloud-default",
                "provider": "provider.cloud",
                "enabled": true,
                "credential_ref": "env:STAR_CONTROL_TEST_TOKEN",
                "api_key": raw_secret,
                "limits": {
                    "timeout_seconds": 300,
                    "max_parallel_jobs": 1
                },
                "routing_tags": ["cloud", "api"],
                "transport_config": {
                    "privacy_handoff_approved": true
                }
            }),
        )
        .expect("execute cloud preflight");

        assert_eq!(
            execution.result().value()["error"]["kind"],
            "cloud_provider_raw_credential"
        );
        let response_text =
            serde_json::to_string(execution.result().value()).expect("serialize response");
        assert!(!response_text.contains(raw_secret));
        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn cloud_cli_transport_executes_command_and_writes_contract() {
        let _env = EnvVarGuard::set("STAR_CONTROL_CLOUD_CLI_SUCCESS_HELPER", "1");
        let instance_value = json!({
            "id": "cloud-default",
            "provider": "provider.cloud",
            "enabled": true,
            "limits": {
                "timeout_seconds": 10,
                "max_parallel_jobs": 1
            },
            "routing_tags": ["cloud", "cli"],
            "transport_config": {
                "auth_mode": "login_session",
                "privacy_handoff_approved": true
            },
            "command_policy": {
                "shell": false,
                "env_allowlist": ["STAR_CONTROL_CLOUD_CLI_SUCCESS_HELPER"]
            },
            "command": {
                "executable": current_test_executable(),
                "args": [
                    "--exact",
                    "cloud::tests::cloud_cli_success_helper",
                    "--nocapture"
                ]
            }
        });
        let (execution, project) = execute_cloud_cli_transport(instance_value.clone())
            .expect("execute cloud CLI transport");

        assert_eq!(execution.result().status(), "success");
        assert_eq!(execution.result().value()["error"], Value::Null);
        assert_eq!(
            execution.result().value()["artifacts"],
            json!([
                "provider-output/cloud-default/response.json",
                "provider-output/cloud-default/stdout.txt",
                "provider-output/cloud-default/stderr.txt",
                "provider-output/cloud-default/privacy-handoff.json",
                "provider-output/cloud-default/cost-metric.json"
            ])
        );
        let schemas = schema_root();
        let store = StateStore::open(&project, &schemas).expect("open executed project");
        let registry = registry_with_instance(CLOUD_CLI_KIND, CLI_TRANSPORT, instance_value)
            .expect("reload cloud registry");
        let context = ProviderRunContext::new(&registry, &store, &schemas);
        let conformance = ProviderConformanceChecker
            .check_execution(&execution, &context, ProviderConformanceProfile::Cloud)
            .expect("cloud CLI provider conformance");
        assert_eq!(conformance.provider_instance_id(), "cloud-default");
        assert!(conformance
            .checked_artifacts()
            .contains(&"provider-output/cloud-default/privacy-handoff.json".to_string()));
        assert!(conformance
            .checked_artifacts()
            .contains(&"provider-output/cloud-default/cost-metric.json".to_string()));
        let stdout = fs::read_to_string(
            project.join(".ai-runs/J-0001/provider-output/cloud-default/stdout.txt"),
        )
        .expect("read stdout");
        assert!(stdout.contains("cloud cli success"));
        let cost_metric = read_json(
            &project.join(".ai-runs/J-0001/provider-output/cloud-default/cost-metric.json"),
        );
        assert_eq!(cost_metric["provider_instance_id"], "cloud-default");
        assert!(cost_metric["wall_time_ms"].as_u64().is_some());
        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn cloud_cli_transport_timeout_writes_timeout_result() {
        let _env = EnvVarGuard::set("STAR_CONTROL_CLOUD_CLI_SLEEP_HELPER", "1");
        let (execution, project) = execute_cloud_cli_transport(json!({
            "id": "cloud-default",
            "provider": "provider.cloud",
            "enabled": true,
            "limits": {
                "timeout_seconds": 1,
                "max_parallel_jobs": 1
            },
            "routing_tags": ["cloud", "cli"],
            "transport_config": {
                "auth_mode": "login_session",
                "privacy_handoff_approved": true
            },
            "command_policy": {
                "shell": false,
                "env_allowlist": ["STAR_CONTROL_CLOUD_CLI_SLEEP_HELPER"]
            },
            "command": {
                "executable": current_test_executable(),
                "args": [
                    "--exact",
                    "cloud::tests::cloud_cli_sleep_helper",
                    "--nocapture"
                ]
            }
        }))
        .expect("execute cloud CLI timeout");

        assert_eq!(execution.result().status(), "timeout");
        assert_eq!(
            execution.result().value()["error"]["kind"],
            "cloud_cli_timeout"
        );
        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn cloud_cli_success_helper() {
        if is_child_helper("cloud::tests::cloud_cli_success_helper")
            && std::env::var("STAR_CONTROL_CLOUD_CLI_SUCCESS_HELPER").is_ok()
        {
            println!("cloud cli success");
        }
    }

    #[test]
    fn cloud_cli_sleep_helper() {
        if is_child_helper("cloud::tests::cloud_cli_sleep_helper")
            && std::env::var("STAR_CONTROL_CLOUD_CLI_SLEEP_HELPER").is_ok()
        {
            std::thread::sleep(Duration::from_secs(5));
        }
    }

    fn execute_cloud_provider(
        kind: &str,
        transport: &str,
        instance_value: Value,
    ) -> Result<(ProviderExecution, PathBuf), ProviderAdapterError> {
        let project = temp_project();
        let schemas = schema_root();
        let store = StateStore::open(&project, &schemas).expect("open store");
        store
            .create_job("use cloud provider", "codex", vec![])
            .expect("create job");
        let registry = registry_with_instance(kind, transport, instance_value)
            .expect("register cloud provider");
        let request = ExecutionRequest::from_value(request_value(), "request.json", &schemas)
            .expect("request");
        let context = ProviderRunContext::new(&registry, &store, &schemas);
        match CloudProviderPreflightAdapter.execute(&request, &context) {
            Ok(execution) => Ok((execution, project)),
            Err(error) => {
                fs::remove_dir_all(project).ok();
                Err(error)
            }
        }
    }

    fn execute_cloud_cli_transport(
        instance_value: Value,
    ) -> Result<(ProviderExecution, PathBuf), ProviderAdapterError> {
        let project = temp_project();
        let schemas = schema_root();
        let store = StateStore::open(&project, &schemas).expect("open store");
        store
            .create_job("use cloud CLI provider", "codex", vec![])
            .expect("create job");
        let registry = registry_with_instance(CLOUD_CLI_KIND, CLI_TRANSPORT, instance_value)
            .expect("register cloud provider");
        let request = ExecutionRequest::from_value(request_value(), "request.json", &schemas)
            .expect("request");
        let context = ProviderRunContext::new(&registry, &store, &schemas);
        match CloudCliProviderAdapter.execute(&request, &context) {
            Ok(execution) => Ok((execution, project)),
            Err(error) => {
                fs::remove_dir_all(project).ok();
                Err(error)
            }
        }
    }

    fn registry_with_instance(
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

    fn request_value() -> Value {
        json!({
            "schema_version": "1.0.0",
            "request_id": "request-0001",
            "job_id": "J-0001",
            "stage": "implement",
            "provider_instance_id": "cloud-default",
            "attempt_id": "attempt-0001",
            "workspec_path": "workspecs/implement.json",
            "created_at": "2026-06-28T00:00:00Z",
            "goal": "run cloud provider",
            "allowed_scope": ["src/**", "tests/**"],
            "forbidden_actions": ["dependency_install", "file_delete"],
            "required_outputs": ["provider-output/cloud-default/response.json"],
            "validation_requirements": ["policy:p0"],
            "context_pack": { "files": [] }
        })
    }

    fn read_json(path: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(path).expect("read json")).expect("parse json")
    }

    fn is_child_helper(test_name: &str) -> bool {
        std::env::args()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|args| args[0] == "--exact" && args[1] == test_name)
    }

    fn current_test_executable() -> String {
        std::env::current_exe()
            .expect("current test executable")
            .display()
            .to_string()
    }

    struct EnvVarGuard<'a> {
        key: &'static str,
        _lock: MutexGuard<'a, ()>,
    }

    impl EnvVarGuard<'_> {
        fn set(key: &'static str, value: &'static str) -> Self {
            let lock = ENV_LOCK.lock().expect("env lock");
            std::env::set_var(key, value);
            Self { key, _lock: lock }
        }
    }

    impl Drop for EnvVarGuard<'_> {
        fn drop(&mut self) {
            std::env::remove_var(self.key);
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
        let counter = TEMP_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "star-control-provider-cloud-{}-{}-{}",
            std::process::id(),
            nanos,
            counter
        ));
        fs::create_dir_all(&path).expect("create temp project");
        path
    }
}
