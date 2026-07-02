use serde_json::{json, Value};
use star_control_execution::{ExecutionEngine, ExecutionError};
use star_control_provider::{
    CapabilityProfile, ProviderManifest, ProviderRegistry, ProviderRegistryError,
    ProviderRegistryLoader,
};
use star_control_release::{ReleaseReadinessError, ReleaseReadinessWriter, RELEASE_READINESS_PATH};
use star_control_router::{JobSpec, RouterEngine, RouterError};
use star_control_schema::{load_schema, validate_json};
use star_control_state::{StateStore, StateStoreError};
use star_sentinel::{
    build_diagnostics_artifact, build_review_pack_artifact, read_changed_lines,
    read_p0_rule_registry, read_task, run_selfcheck, validate_diagnostics_artifact,
    write_gate_artifacts, write_review_pack_artifacts, ChangedLines, Decision, EvaluationResult,
    P0Evaluator, ReviewValidation, SentinelError, SentinelTask, CHANGED_LINES_SCHEMA,
    DIAGNOSTICS_FILE, SENTINEL_TASK_SCHEMA, STAR_SENTINEL_TOOL_OUTPUT_DIR,
};
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const CLI_OUTPUT_SCHEMA: &str = "cli-output.schema.json";
const CLI_ERROR_SCHEMA: &str = "cli-error.schema.json";
const APPROVAL_REQUEST_SCHEMA: &str = "approval-request.schema.json";
const APPROVAL_RESPONSE_SCHEMA: &str = "approval-response.schema.json";
const SCHEMA_VERSION: &str = "1.0.0";
const DEFAULT_PROVIDER: &str = "fake-default";
const DEFAULT_ENTRYPOINT: &str = "star-control";
const BUILTIN_PROVIDER_REGISTRY: &str = "configs/registries/builtin-provider-registry.yaml";
const TERMINAL_STATES: &[&str] = &["DONE", "FAILED", "BLOCKED", "CANCELLED"];

#[derive(Debug, Clone)]
pub struct CliConfig {
    repo_root: PathBuf,
}

impl CliConfig {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }

    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    pub fn schema_root(&self) -> PathBuf {
        self.repo_root.join("specs").join("schemas")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliRunResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug)]
pub enum CliError {
    InvalidInput {
        command: String,
        message: String,
    },
    MissingArtifact {
        command: String,
        message: String,
        artifact_paths: Vec<String>,
    },
    ProviderExecution {
        command: String,
        message: String,
    },
    State {
        command: String,
        source: StateStoreError,
    },
    Router {
        command: String,
        source: RouterError,
    },
    ProviderRegistry {
        command: String,
        source: ProviderRegistryError,
    },
    Sentinel {
        command: String,
        source: SentinelError,
    },
    ReleaseReadiness {
        command: String,
        source: ReleaseReadinessError,
    },
    Execution {
        command: String,
        source: ExecutionError,
    },
    Internal {
        command: String,
        message: String,
    },
}

impl CliError {
    fn command(&self) -> &str {
        match self {
            Self::InvalidInput { command, .. }
            | Self::MissingArtifact { command, .. }
            | Self::ProviderExecution { command, .. }
            | Self::State { command, .. }
            | Self::Router { command, .. }
            | Self::ProviderRegistry { command, .. }
            | Self::Sentinel { command, .. }
            | Self::ReleaseReadiness { command, .. }
            | Self::Execution { command, .. }
            | Self::Internal { command, .. } => command,
        }
    }

    fn exit_code(&self) -> i32 {
        match self {
            Self::InvalidInput { .. } => 2,
            Self::MissingArtifact { .. } | Self::State { .. } => 3,
            Self::ProviderExecution { .. } | Self::Execution { .. } => 4,
            Self::Router { .. }
            | Self::ProviderRegistry { .. }
            | Self::Sentinel { .. }
            | Self::Internal { .. } => 5,
            Self::ReleaseReadiness { .. } => 5,
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "InvalidInput",
            Self::MissingArtifact { .. } => "MissingArtifact",
            Self::ProviderExecution { .. } => "ProviderExecutionFailed",
            Self::State { .. } => "StateReadFailed",
            Self::Router { .. } => "RouteFailed",
            Self::ProviderRegistry { .. } => "ProviderRegistryFailed",
            Self::Sentinel { .. } => "StarSentinelFailed",
            Self::ReleaseReadiness { .. } => "ReleaseReadinessReadFailed",
            Self::Execution { .. } => "ExecutionFailed",
            Self::Internal { .. } => "InternalError",
        }
    }

    fn category(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "input",
            Self::MissingArtifact { .. } | Self::State { .. } => "state-store",
            Self::ProviderExecution { .. } | Self::Execution { .. } => "provider-execution",
            Self::Router { .. } => "router",
            Self::ProviderRegistry { .. } => "provider-registry",
            Self::Sentinel { .. } => "star-sentinel",
            Self::ReleaseReadiness { .. } => "release-readiness",
            Self::Internal { .. } => "internal",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::InvalidInput { message, .. }
            | Self::MissingArtifact { message, .. }
            | Self::ProviderExecution { message, .. }
            | Self::Internal { message, .. } => message.clone(),
            Self::State { source, .. } => source.to_string(),
            Self::Router { source, .. } => source.to_string(),
            Self::ProviderRegistry { source, .. } => source.to_string(),
            Self::Sentinel { source, .. } => source.to_string(),
            Self::ReleaseReadiness { source, .. } => source.to_string(),
            Self::Execution { source, .. } => source.to_string(),
        }
    }

    fn artifact_paths(&self) -> Vec<String> {
        match self {
            Self::MissingArtifact { artifact_paths, .. } => artifact_paths.clone(),
            _ => Vec::new(),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code(), self.message())
    }
}

impl Error for CliError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedArgs {
    command: String,
    subcommand: Option<String>,
    subject: Option<String>,
    project: Option<PathBuf>,
    job_id: Option<String>,
    request: Option<String>,
    entrypoint: Option<String>,
    provider: Option<String>,
    provider_instances: Vec<PathBuf>,
    stage: Option<String>,
    response: Option<String>,
    reason: Option<String>,
    constraints: Vec<String>,
    release_readiness: bool,
    recovery_list: bool,
    dry_run: bool,
    json: bool,
    markdown: bool,
}

#[derive(Debug, Clone)]
struct CliEvent {
    event_id: String,
    event_type: &'static str,
    state: String,
    stage: String,
    message: &'static str,
    artifact_paths: Vec<String>,
    details: Value,
}

pub fn run_cli<I, S>(args: I, config: &CliConfig) -> CliRunResult
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let raw_args: Vec<String> = args.into_iter().map(Into::into).collect();
    let parsed = match parse_args(&raw_args) {
        Ok(parsed) => parsed,
        Err(error) => return render_error(error, true, config),
    };
    let json_mode = parsed.json;
    let command = parsed.command.clone();
    let result = match command.as_str() {
        "run" => run_command(&parsed, config),
        "status" => status_command(&parsed, config),
        "report" => report_command(&parsed, config),
        "approve" => approve_command(&parsed, config),
        "cancel" => cancel_command(&parsed, config),
        "resume" => resume_command(&parsed, config),
        "recover" => recover_command(&parsed, config),
        "providers" => providers_command(&parsed, config),
        "sentinel" => sentinel_command(&parsed, config),
        _ => Err(CliError::InvalidInput {
            command,
            message: "unsupported command".to_string(),
        }),
    };

    match result {
        Ok(envelope) => render_success(envelope, json_mode, config),
        Err(error) => render_error(error, json_mode, config),
    }
}

fn parse_args(args: &[String]) -> Result<ParsedArgs, CliError> {
    let Some(command) = args.first().cloned() else {
        return Err(CliError::InvalidInput {
            command: "unknown".to_string(),
            message: "missing command".to_string(),
        });
    };

    let mut parsed = ParsedArgs {
        command: command.clone(),
        subcommand: None,
        subject: None,
        project: None,
        job_id: None,
        request: None,
        entrypoint: None,
        provider: None,
        provider_instances: Vec::new(),
        stage: None,
        response: None,
        reason: None,
        constraints: Vec::new(),
        release_readiness: false,
        recovery_list: false,
        dry_run: false,
        json: false,
        markdown: false,
    };

    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--project" => {
                parsed.project = Some(PathBuf::from(require_option_value(
                    args,
                    &mut index,
                    "--project",
                    &command,
                )?));
            }
            "--job" => {
                parsed.job_id = Some(require_option_value(args, &mut index, "--job", &command)?);
            }
            "--request" => {
                parsed.request = Some(require_option_value(
                    args,
                    &mut index,
                    "--request",
                    &command,
                )?);
            }
            "--entrypoint" => {
                parsed.entrypoint = Some(require_option_value(
                    args,
                    &mut index,
                    "--entrypoint",
                    &command,
                )?);
            }
            "--provider" => {
                parsed.provider = Some(require_option_value(
                    args,
                    &mut index,
                    "--provider",
                    &command,
                )?);
            }
            "--provider-instance" => {
                parsed
                    .provider_instances
                    .push(PathBuf::from(require_option_value(
                        args,
                        &mut index,
                        "--provider-instance",
                        &command,
                    )?));
            }
            "--stage" => {
                parsed.stage = Some(require_option_value(args, &mut index, "--stage", &command)?);
            }
            "--response" => {
                parsed.response = Some(require_option_value(
                    args,
                    &mut index,
                    "--response",
                    &command,
                )?);
            }
            "--reason" => {
                parsed.reason = Some(require_option_value(
                    args, &mut index, "--reason", &command,
                )?);
            }
            "--constraint" => {
                parsed.constraints.push(require_option_value(
                    args,
                    &mut index,
                    "--constraint",
                    &command,
                )?);
            }
            "--dry-run" => parsed.dry_run = true,
            "--release-readiness" => parsed.release_readiness = true,
            "--list" => parsed.recovery_list = true,
            "--json" => parsed.json = true,
            "--markdown" => parsed.markdown = true,
            positional if is_command_group_position(&command, positional) => {
                if parsed.subcommand.is_none() {
                    parsed.subcommand = Some(positional.to_string());
                } else if parsed.subject.is_none() {
                    parsed.subject = Some(positional.to_string());
                } else {
                    return Err(CliError::InvalidInput {
                        command,
                        message: format!("unsupported argument {}", positional),
                    });
                }
            }
            unknown => {
                return Err(CliError::InvalidInput {
                    command,
                    message: format!("unsupported option {}", unknown),
                });
            }
        }
        index += 1;
    }

    Ok(parsed)
}

fn is_command_group_position(command: &str, argument: &str) -> bool {
    matches!(command, "providers" | "sentinel") && !argument.starts_with("--")
}

fn require_option_value(
    args: &[String],
    index: &mut usize,
    option: &str,
    command: &str,
) -> Result<String, CliError> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| CliError::InvalidInput {
            command: command.to_string(),
            message: format!("missing value for {}", option),
        })
}

fn run_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    let project = required_project(parsed)?;
    let request = parsed
        .request
        .clone()
        .ok_or_else(|| CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "--request is required for run".to_string(),
        })?;
    let provider = parsed.provider.as_deref().unwrap_or(DEFAULT_PROVIDER);
    let provider_instance_id = provider.to_string();

    let schemas = config.schema_root();
    let store = StateStore::open(&project, &schemas).map_err(|source| CliError::State {
        command: parsed.command.clone(),
        source,
    })?;
    let registry = load_run_registry(parsed, config, provider)?;
    let job = store
        .create_job(
            request,
            parsed
                .entrypoint
                .clone()
                .unwrap_or_else(|| DEFAULT_ENTRYPOINT.to_string()),
            Vec::new(),
        )
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let job_id = string_field(&job, "job_id", &parsed.command)?;
    let job_spec = JobSpec::from_value(job.clone(), "job.json", &schemas).map_err(|source| {
        CliError::Router {
            command: parsed.command.clone(),
            source,
        }
    })?;
    let router = RouterEngine::new(&registry, &schemas);
    let route_output = router.route(&job_spec).map_err(|source| CliError::Router {
        command: parsed.command.clone(),
        source,
    })?;
    let route_value = route_value_for_provider(route_output.route().value(), &provider_instance_id);
    store
        .save_route(&job_id, &route_value)
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    for (stage, workspec) in route_output.workspecs() {
        let workspec_value = workspec_value_for_provider(workspec.value(), &provider_instance_id);
        store
            .save_workspec(&job_id, stage, &workspec_value)
            .map_err(|source| CliError::State {
                command: parsed.command.clone(),
                source,
            })?;
    }

    let mut artifacts = vec![
        format!(".ai-runs/{}/job.json", job_id),
        format!(".ai-runs/{}/route.json", job_id),
    ];

    let (state, executed_stage) = if parsed.dry_run {
        let state = routed_state(&job_id);
        store
            .save_state(&job_id, &state)
            .map_err(|source| CliError::State {
                command: parsed.command.clone(),
                source,
            })?;
        artifacts.push(format!(".ai-runs/{}/run-state.json", job_id));
        (state, None)
    } else {
        let stage = route_output
            .workspec("implement")
            .map(|workspec| workspec.stage().to_string())
            .or_else(|| route_output.workspecs().keys().next().cloned())
            .ok_or_else(|| CliError::Internal {
                command: parsed.command.clone(),
                message: "route produced no executable WorkSpec".to_string(),
            })?;
        let engine = ExecutionEngine::new(&store, &registry, &schemas);
        let outcome =
            engine
                .execute_stage(&job_id, &stage)
                .map_err(|source| CliError::Execution {
                    command: parsed.command.clone(),
                    source,
                })?;
        let provider_result = outcome.provider_execution().result().value();
        let report = report_from_provider_result(provider_result);
        store
            .save_report(&job_id, &format!("{}-report", stage), &report)
            .map_err(|source| CliError::State {
                command: parsed.command.clone(),
                source,
            })?;
        artifacts.push(format!(".ai-runs/{}/run-state.json", job_id));
        artifacts.push(format!(
            ".ai-runs/{}/provider-output/{}/request.json",
            job_id, provider_instance_id
        ));
        artifacts.extend(provider_result_artifacts(provider_result, &job_id));
        artifacts.push(format!(".ai-runs/{}/reports/{}-report.json", job_id, stage));
        (outcome.state().clone(), Some(stage))
    };

    Ok(success_envelope(
        "run",
        status_for_state(
            state
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("FAILED"),
        ),
        json!({
            "job_id": job_id,
            "state": state.get("state").cloned().unwrap_or_else(|| json!("")),
            "current_stage": state.get("current_stage").cloned().unwrap_or_else(|| json!("")),
            "run_dir": format!(".ai-runs/{}", job_id),
            "next_action": state.get("next_action").cloned().unwrap_or_else(|| json!("")),
            "dry_run": parsed.dry_run,
            "executed_stage": executed_stage
        }),
        artifacts,
    ))
}

fn provider_result_artifacts(result: &Value, job_id: &str) -> Vec<String> {
    result
        .get("artifacts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|path| format!(".ai-runs/{}/{}", job_id, path))
        .collect()
}

fn load_run_registry(
    parsed: &ParsedArgs,
    config: &CliConfig,
    provider: &str,
) -> Result<ProviderRegistry, CliError> {
    if parsed.provider.is_none() && !parsed.provider_instances.is_empty() {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "--provider is required when --provider-instance is set".to_string(),
        });
    }
    if provider != DEFAULT_PROVIDER && parsed.provider_instances.is_empty() {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "--provider-instance is required when --provider is not fake-default"
                .to_string(),
        });
    }

    let loader = ProviderRegistryLoader::new(config.repo_root());
    let registry = if provider == DEFAULT_PROVIDER && parsed.provider_instances.is_empty() {
        loader
            .load_fake_default_registry()
            .map_err(|source| CliError::ProviderRegistry {
                command: parsed.command.clone(),
                source,
            })?
    } else {
        let mut instance_paths = vec![PathBuf::from(
            "configs/provider-instances/fake-provider.example.yaml",
        )];
        instance_paths.extend(parsed.provider_instances.iter().cloned());
        loader
            .load_registry(
                "configs/registries/builtin-provider-registry.yaml",
                &instance_paths,
            )
            .map_err(|source| CliError::ProviderRegistry {
                command: parsed.command.clone(),
                source,
            })?
    };

    registry
        .instance(provider)
        .ok_or_else(|| CliError::InvalidInput {
            command: parsed.command.clone(),
            message: format!("provider instance {} is not loaded", provider),
        })?;
    Ok(registry)
}

fn providers_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    reject_provider_command_options(parsed)?;
    let subcommand = parsed
        .subcommand
        .as_deref()
        .ok_or_else(|| CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "providers requires subcommand list or show".to_string(),
        })?;
    match subcommand {
        "list" => providers_list_command(parsed, config),
        "show" => providers_show_command(parsed, config),
        "healthcheck" => Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "providers healthcheck is reserved until provider smoke checks are enabled"
                .to_string(),
        }),
        other => Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: format!("unsupported providers subcommand {}", other),
        }),
    }
}

fn providers_list_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    if parsed.provider.is_some() || parsed.subject.is_some() {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "providers list does not accept provider id arguments".to_string(),
        });
    }
    let registry = load_builtin_provider_registry(parsed, config)?;
    let providers: Vec<Value> = registry
        .providers()
        .into_iter()
        .map(|manifest| {
            let profile = registry.capability_profile(manifest.id());
            provider_summary_value(manifest, profile, config)
        })
        .collect();

    Ok(success_envelope(
        "providers",
        "success",
        json!({
            "subcommand": "list",
            "registry_path": BUILTIN_PROVIDER_REGISTRY,
            "provider_count": providers.len(),
            "providers": providers,
            "healthcheck_enabled": false,
            "actions_enabled": false
        }),
        Vec::new(),
    ))
}

fn providers_show_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    let provider_id = match (parsed.subject.as_deref(), parsed.provider.as_deref()) {
        (Some(subject), Some(provider)) if subject != provider => {
            return Err(CliError::InvalidInput {
                command: parsed.command.clone(),
                message: format!(
                    "providers show provider id mismatch: argument {}, --provider {}",
                    subject, provider
                ),
            });
        }
        (Some(subject), _) => subject.to_string(),
        (_, Some(provider)) => provider.to_string(),
        (None, None) => {
            return Err(CliError::InvalidInput {
                command: parsed.command.clone(),
                message: "providers show requires a provider id".to_string(),
            });
        }
    };

    let registry = load_builtin_provider_registry(parsed, config)?;
    let manifest = registry
        .manifest(&provider_id)
        .ok_or_else(|| CliError::InvalidInput {
            command: parsed.command.clone(),
            message: format!("provider {} is not registered", provider_id),
        })?;
    let profile =
        registry
            .capability_profile(&provider_id)
            .ok_or_else(|| CliError::InvalidInput {
                command: parsed.command.clone(),
                message: format!("provider {} has no capability profile", provider_id),
            })?;

    Ok(success_envelope(
        "providers",
        "success",
        json!({
            "subcommand": "show",
            "registry_path": BUILTIN_PROVIDER_REGISTRY,
            "provider": provider_summary_value(manifest, Some(profile), config),
            "manifest": manifest.value(),
            "capability_profile": profile.value(),
            "healthcheck_enabled": false,
            "actions_enabled": false
        }),
        Vec::new(),
    ))
}

fn load_builtin_provider_registry(
    parsed: &ParsedArgs,
    config: &CliConfig,
) -> Result<ProviderRegistry, CliError> {
    let loader = ProviderRegistryLoader::new(config.repo_root());
    loader
        .load_registry(BUILTIN_PROVIDER_REGISTRY, &[])
        .map_err(|source| CliError::ProviderRegistry {
            command: parsed.command.clone(),
            source,
        })
}

fn provider_summary_value(
    manifest: &ProviderManifest,
    profile: Option<&CapabilityProfile>,
    config: &CliConfig,
) -> Value {
    json!({
        "id": manifest.id(),
        "kind": manifest.kind(),
        "transport": manifest.transport(),
        "adapter": manifest.adapter(),
        "manifest_path": repo_relative_path(config.repo_root(), manifest.path()),
        "capabilities_path": profile
            .map(|profile| repo_relative_path(config.repo_root(), profile.path()))
            .unwrap_or_default(),
        "routing_tags": profile
            .map(|profile| profile.routing_tags().to_vec())
            .unwrap_or_default()
    })
}

fn reject_provider_command_options(parsed: &ParsedArgs) -> Result<(), CliError> {
    let unsupported = [
        (parsed.project.is_some(), "--project"),
        (parsed.job_id.is_some(), "--job"),
        (parsed.request.is_some(), "--request"),
        (parsed.entrypoint.is_some(), "--entrypoint"),
        (!parsed.provider_instances.is_empty(), "--provider-instance"),
        (parsed.stage.is_some(), "--stage"),
        (parsed.response.is_some(), "--response"),
        (parsed.reason.is_some(), "--reason"),
        (!parsed.constraints.is_empty(), "--constraint"),
        (parsed.release_readiness, "--release-readiness"),
        (parsed.recovery_list, "--list"),
        (parsed.dry_run, "--dry-run"),
        (parsed.markdown, "--markdown"),
    ];
    for (is_set, option) in unsupported {
        if is_set {
            return Err(CliError::InvalidInput {
                command: parsed.command.clone(),
                message: format!("providers does not accept {}", option),
            });
        }
    }
    Ok(())
}

fn repo_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn sentinel_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    let subcommand = parsed
        .subcommand
        .as_deref()
        .ok_or_else(|| CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "sentinel requires subcommand check, gate, review-pack, or selfcheck"
                .to_string(),
        })?;
    match subcommand {
        "check" => sentinel_check_command(parsed, config),
        "gate" => sentinel_gate_command(parsed, config),
        "review-pack" => sentinel_review_pack_command(parsed, config),
        "selfcheck" => sentinel_selfcheck_command(parsed, config),
        other => Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: format!("unsupported sentinel subcommand {}", other),
        }),
    }
}

fn sentinel_check_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    reject_sentinel_command_options(parsed, true)?;
    let job_id = required_job(parsed)?;
    let (store, task, _changed_lines, result) = evaluate_sentinel_job(parsed, config, &job_id)?;
    let diagnostics = build_diagnostics_artifact(&result);
    let sentinel_schema_root = sentinel_schema_root(config);
    validate_diagnostics_artifact(&diagnostics, &sentinel_schema_root).map_err(|source| {
        CliError::Sentinel {
            command: parsed.command.clone(),
            source,
        }
    })?;
    store
        .write_tool_json(
            &job_id,
            STAR_SENTINEL_TOOL_OUTPUT_DIR,
            DIAGNOSTICS_FILE,
            &diagnostics,
        )
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let diagnostics_path = sentinel_artifact_path(&job_id, DIAGNOSTICS_FILE);

    Ok(success_envelope(
        "sentinel",
        "success",
        json!({
            "subcommand": "check",
            "job_id": job_id,
            "task_id": task.task_id,
            "decision": result.decision.as_str(),
            "diagnostic_count": result.diagnostics.len(),
            "diagnostics": diagnostics,
            "diagnostics_path": diagnostics_path,
            "actions_enabled": false
        }),
        vec![diagnostics_path],
    ))
}

fn sentinel_gate_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    reject_sentinel_command_options(parsed, true)?;
    let job_id = required_job(parsed)?;
    let (store, task, _changed_lines, result) = evaluate_sentinel_job(parsed, config, &job_id)?;
    write_gate_artifacts(
        &store,
        &job_id,
        &task,
        &result,
        sentinel_schema_root(config),
    )
    .map_err(|source| CliError::Sentinel {
        command: parsed.command.clone(),
        source,
    })?;
    let diagnostics_path = sentinel_artifact_path(&job_id, DIAGNOSTICS_FILE);
    let approval_path = sentinel_artifact_path(&job_id, star_sentinel::APPROVAL_FILE);

    Ok(success_envelope(
        "sentinel",
        status_for_sentinel_decision(result.decision),
        json!({
            "subcommand": "gate",
            "job_id": job_id,
            "task_id": task.task_id,
            "decision": result.decision.as_str(),
            "diagnostic_count": result.diagnostics.len(),
            "diagnostics_path": diagnostics_path,
            "approval_path": approval_path,
            "actions_enabled": false
        }),
        vec![diagnostics_path, approval_path],
    ))
}

fn sentinel_review_pack_command(
    parsed: &ParsedArgs,
    config: &CliConfig,
) -> Result<Value, CliError> {
    reject_sentinel_command_options(parsed, true)?;
    let job_id = required_job(parsed)?;
    let (store, task, changed_lines, result) = evaluate_sentinel_job(parsed, config, &job_id)?;
    let review_pack = build_review_pack_artifact(
        &task,
        &changed_lines,
        &result,
        &[ReviewValidation::new(
            "star-control sentinel check",
            validation_result_for_sentinel_decision(result.decision),
        )],
    );
    write_review_pack_artifacts(&store, &job_id, &review_pack, sentinel_schema_root(config))
        .map_err(|source| CliError::Sentinel {
            command: parsed.command.clone(),
            source,
        })?;
    let tool_json_path = sentinel_artifact_path(&job_id, star_sentinel::REVIEW_PACK_JSON_FILE);
    let tool_markdown_path =
        sentinel_artifact_path(&job_id, star_sentinel::REVIEW_PACK_MARKDOWN_FILE);
    let review_json_path = format!(
        ".ai-runs/{}/review-packs/{}",
        job_id,
        star_sentinel::REVIEW_PACK_JSON_FILE
    );
    let review_markdown_path = format!(
        ".ai-runs/{}/review-packs/{}",
        job_id,
        star_sentinel::REVIEW_PACK_MARKDOWN_FILE
    );

    Ok(success_envelope(
        "sentinel",
        status_for_sentinel_decision(result.decision),
        json!({
            "subcommand": "review-pack",
            "job_id": job_id,
            "task_id": task.task_id,
            "decision": result.decision.as_str(),
            "review_pack_path": review_markdown_path,
            "tool_review_pack_path": tool_markdown_path,
            "actions_enabled": false
        }),
        vec![
            tool_json_path,
            tool_markdown_path,
            review_json_path,
            review_markdown_path,
        ],
    ))
}

fn sentinel_selfcheck_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    reject_sentinel_command_options(parsed, false)?;
    let report = run_selfcheck(config.repo_root());
    Ok(success_envelope(
        "sentinel",
        if report.ok { "success" } else { "failed" },
        json!({
            "subcommand": "selfcheck",
            "ok": report.ok,
            "diagnostic_count": report.diagnostics.len(),
            "diagnostics": report.diagnostics,
            "actions_enabled": false
        }),
        Vec::new(),
    ))
}

fn evaluate_sentinel_job(
    parsed: &ParsedArgs,
    config: &CliConfig,
    job_id: &str,
) -> Result<(StateStore, SentinelTask, ChangedLines, EvaluationResult), CliError> {
    let project = required_project(parsed)?;
    let store =
        StateStore::open(&project, config.schema_root()).map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let task_path = require_sentinel_input(&store, job_id, "task.json", SENTINEL_TASK_SCHEMA)?;
    let changed_lines_path =
        require_sentinel_input(&store, job_id, "changed_lines.json", CHANGED_LINES_SCHEMA)?;
    let sentinel_schema_root = sentinel_schema_root(config);
    let task =
        read_task(&task_path, &sentinel_schema_root).map_err(|source| CliError::Sentinel {
            command: parsed.command.clone(),
            source,
        })?;
    let changed_lines =
        read_changed_lines(&changed_lines_path, &sentinel_schema_root).map_err(|source| {
            CliError::Sentinel {
                command: parsed.command.clone(),
                source,
            }
        })?;
    let registry = read_p0_rule_registry(sentinel_registry_path(config), &sentinel_schema_root)
        .map_err(|source| CliError::Sentinel {
            command: parsed.command.clone(),
            source,
        })?;
    let result = P0Evaluator::new(registry)
        .evaluate(&task, &changed_lines)
        .map_err(|source| CliError::Sentinel {
            command: parsed.command.clone(),
            source,
        })?;
    Ok((store, task, changed_lines, result))
}

fn require_sentinel_input(
    store: &StateStore,
    job_id: &str,
    file_name: &str,
    schema_name: &str,
) -> Result<PathBuf, CliError> {
    let relative_path = format!(
        "tool-output/{}/{}",
        STAR_SENTINEL_TOOL_OUTPUT_DIR, file_name
    );
    let path = store
        .resolve_job_path(job_id, &relative_path)
        .map_err(|source| CliError::State {
            command: "sentinel".to_string(),
            source,
        })?;
    if path.is_file() {
        Ok(path)
    } else {
        Err(CliError::MissingArtifact {
            command: "sentinel".to_string(),
            message: format!(
                "required Star Sentinel input not found: {} ({})",
                relative_path, schema_name
            ),
            artifact_paths: vec![format!(".ai-runs/{}/{}", job_id, relative_path)],
        })
    }
}

fn reject_sentinel_command_options(
    parsed: &ParsedArgs,
    requires_project_job: bool,
) -> Result<(), CliError> {
    let unsupported = [
        (parsed.subject.is_some(), "extra positional argument"),
        (parsed.request.is_some(), "--request"),
        (parsed.entrypoint.is_some(), "--entrypoint"),
        (parsed.provider.is_some(), "--provider"),
        (!parsed.provider_instances.is_empty(), "--provider-instance"),
        (parsed.stage.is_some(), "--stage"),
        (parsed.response.is_some(), "--response"),
        (parsed.reason.is_some(), "--reason"),
        (!parsed.constraints.is_empty(), "--constraint"),
        (parsed.release_readiness, "--release-readiness"),
        (parsed.recovery_list, "--list"),
        (parsed.dry_run, "--dry-run"),
        (parsed.markdown, "--markdown"),
    ];
    for (is_set, option) in unsupported {
        if is_set {
            return Err(CliError::InvalidInput {
                command: parsed.command.clone(),
                message: format!("sentinel does not accept {}", option),
            });
        }
    }
    if requires_project_job {
        let _ = required_project(parsed)?;
        let _ = required_job(parsed)?;
    } else if parsed.project.is_some() || parsed.job_id.is_some() {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "sentinel selfcheck does not accept --project or --job".to_string(),
        });
    }
    Ok(())
}

fn sentinel_schema_root(config: &CliConfig) -> PathBuf {
    config
        .repo_root()
        .join("builtin-tools")
        .join("star-sentinel")
        .join("schemas")
}

fn sentinel_registry_path(config: &CliConfig) -> PathBuf {
    config
        .repo_root()
        .join("builtin-tools")
        .join("star-sentinel")
        .join("policies")
        .join("p0-rule-registry.json")
}

fn sentinel_artifact_path(job_id: &str, file_name: &str) -> String {
    format!(
        ".ai-runs/{}/tool-output/{}/{}",
        job_id, STAR_SENTINEL_TOOL_OUTPUT_DIR, file_name
    )
}

fn status_for_sentinel_decision(decision: Decision) -> &'static str {
    match decision {
        Decision::AutoPass => "success",
        Decision::HumanReview => "waiting_approval",
        Decision::Block => "blocked",
    }
}

fn validation_result_for_sentinel_decision(decision: Decision) -> &'static str {
    match decision {
        Decision::AutoPass => "PASS",
        Decision::HumanReview => "HUMAN_REVIEW",
        Decision::Block => "BLOCK",
    }
}

fn route_value_for_provider(route: &Value, provider_instance_id: &str) -> Value {
    let mut route = route.clone();
    if provider_instance_id == DEFAULT_PROVIDER {
        return route;
    }
    if let Some(assignments) = route.get_mut("assignments").and_then(Value::as_object_mut) {
        for assignment in assignments.values_mut() {
            if let Some(assignment) = assignment.as_object_mut() {
                assignment.insert(
                    "provider".to_string(),
                    Value::String(provider_instance_id.to_string()),
                );
            }
        }
    }
    if let Some(reasons) = route
        .get_mut("routing_reasons")
        .and_then(Value::as_array_mut)
    {
        reasons.push(Value::String(format!(
            "cli provider override: {}",
            provider_instance_id
        )));
    }
    route
}

fn workspec_value_for_provider(workspec: &Value, provider_instance_id: &str) -> Value {
    let mut workspec = workspec.clone();
    if provider_instance_id == DEFAULT_PROVIDER {
        return workspec;
    }
    if let Some(workspec) = workspec.as_object_mut() {
        workspec.insert(
            "provider".to_string(),
            Value::String(provider_instance_id.to_string()),
        );
        workspec.insert(
            "provider_instance".to_string(),
            Value::String(provider_instance_id.to_string()),
        );
        workspec.insert(
            "required_outputs".to_string(),
            json!([format!(
                "provider-output/{}/response.json",
                provider_instance_id
            )]),
        );
    }
    workspec
}

fn status_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    let project = required_project(parsed)?;
    let job_id = required_job(parsed)?;
    let store =
        StateStore::open(&project, config.schema_root()).map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let state = store
        .load_state(&job_id)
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let events = store
        .read_events(&job_id)
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let latest_event = events
        .last()
        .and_then(|event| event.get("event_id"))
        .cloned()
        .unwrap_or_else(|| json!(""));

    Ok(success_envelope(
        "status",
        status_for_state(
            state
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("FAILED"),
        ),
        json!({
            "job_id": job_id,
            "state": state.get("state").cloned().unwrap_or_else(|| json!("")),
            "current_stage": state.get("current_stage").cloned().unwrap_or_else(|| json!("")),
            "next_action": state.get("next_action").cloned().unwrap_or_else(|| json!("")),
            "latest_event": latest_event,
            "artifacts": state.get("artifacts").cloned().unwrap_or_else(|| json!({}))
        }),
        vec![
            format!(".ai-runs/{}/run-state.json", job_id),
            format!(".ai-runs/{}/events.jsonl", job_id),
        ],
    ))
}

fn report_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    let project = required_project(parsed)?;
    let job_id = required_job(parsed)?;
    if parsed.release_readiness {
        return release_readiness_report_command(parsed, config, project, job_id);
    }
    let stage = parsed.stage.as_deref().unwrap_or("implement");
    let store =
        StateStore::open(&project, config.schema_root()).map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let report_name = format!("{}-report", stage);
    let report = store
        .load_report(&job_id, &report_name)
        .map_err(|source| match source {
            StateStoreError::ArtifactNotFound { .. } => CliError::MissingArtifact {
                command: parsed.command.clone(),
                message: format!("report artifact not found for stage {}", stage),
                artifact_paths: vec![format!(".ai-runs/{}/reports/{}.json", job_id, report_name)],
            },
            source => CliError::State {
                command: parsed.command.clone(),
                source,
            },
        })?;

    Ok(success_envelope(
        "report",
        status_for_report(
            report
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("FAILED"),
        ),
        json!({
            "job_id": job_id,
            "stage": stage,
            "report": report
        }),
        vec![format!(".ai-runs/{}/reports/{}.json", job_id, report_name)],
    ))
}

fn release_readiness_report_command(
    parsed: &ParsedArgs,
    config: &CliConfig,
    project: PathBuf,
    job_id: String,
) -> Result<Value, CliError> {
    if parsed.stage.is_some() {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "--stage cannot be combined with --release-readiness".to_string(),
        });
    }
    let store =
        StateStore::open(&project, config.schema_root()).map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    store.load_job(&job_id).map_err(|source| CliError::State {
        command: parsed.command.clone(),
        source,
    })?;
    let writer = ReleaseReadinessWriter::new(config.schema_root());
    let readiness = writer
        .read(&store, &job_id)
        .map_err(|source| CliError::ReleaseReadiness {
            command: parsed.command.clone(),
            source,
        })?
        .ok_or_else(|| CliError::MissingArtifact {
            command: parsed.command.clone(),
            message: "release readiness artifact not found".to_string(),
            artifact_paths: vec![format!(".ai-runs/{}/{}", job_id, RELEASE_READINESS_PATH)],
        })?;

    Ok(success_envelope(
        "report",
        "success",
        json!({
            "job_id": job_id,
            "report_kind": "release_readiness",
            "release_readiness_path": format!(".ai-runs/{}/{}", job_id, RELEASE_READINESS_PATH),
            "release_actions_enabled": false,
            "readiness": readiness
        }),
        vec![format!(".ai-runs/{}/{}", job_id, RELEASE_READINESS_PATH)],
    ))
}

fn approve_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    let project = required_project(parsed)?;
    let job_id = required_job(parsed)?;
    let response = required_response(parsed)?;
    let reason = parsed
        .reason
        .clone()
        .ok_or_else(|| CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "--reason is required for approve".to_string(),
        })?;
    validate_approval_response_value(&response, &parsed.command)?;

    let store =
        StateStore::open(&project, config.schema_root()).map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let mut state = store
        .load_state(&job_id)
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let current_state = state_string(&state);
    if current_state != "WAITING_APPROVAL" {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: format!(
                "approve requires WAITING_APPROVAL state, got {}",
                current_state
            ),
        });
    }

    let approval_request = load_job_json(
        &store,
        &job_id,
        "approvals/approval-request.json",
        APPROVAL_REQUEST_SCHEMA,
        &parsed.command,
        &config.schema_root(),
    )?;
    let stage = string_field(&approval_request, "stage", &parsed.command)?;
    let task_id = string_field(&approval_request, "task_id", &parsed.command)?;
    let allowed_next_stage = (response == "approved")
        .then(|| allowed_next_stage_for(&stage))
        .flatten();
    let approval_response = json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id.clone(),
        "stage": stage.clone(),
        "task_id": task_id.clone(),
        "response": response.clone(),
        "reviewer": "star-control-cli",
        "responded_at": timestamp_string(),
        "reason": reason,
        "allowed_next_stage": allowed_next_stage,
        "constraints": parsed.constraints.clone()
    });
    validate_schema_value(
        &approval_response,
        &config.schema_root(),
        APPROVAL_RESPONSE_SCHEMA,
        "approvals/approval-response.json",
    )
    .map_err(|message| CliError::Internal {
        command: parsed.command.clone(),
        message,
    })?;

    let approval_ref = store
        .write_approval_json(&job_id, "approval-response.json", &approval_response)
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let next_state = state_after_approval_response(&response);
    let next_action = next_action_after_approval_response(&response);
    let event_id = format!("{}-cli-approval-recorded", job_id.to_lowercase());
    update_state_for_control_command(
        &mut state,
        &store,
        next_state,
        &stage,
        next_action,
        &event_id,
        Some(("approval_response", &approval_ref)),
    )?;
    store
        .save_state(&job_id, &state)
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    append_cli_event(
        &store,
        &job_id,
        CliEvent {
            event_id,
            event_type: "APPROVAL_RECORDED",
            state: next_state.to_string(),
            stage: stage.clone(),
            message: "Approval response recorded",
            artifact_paths: vec!["approvals/approval-response.json".to_string()],
            details: json!({
                "response": approval_response["response"],
                "allowed_next_stage": approval_response["allowed_next_stage"]
            }),
        },
    )
    .map_err(|source| CliError::State {
        command: parsed.command.clone(),
        source,
    })?;

    Ok(success_envelope(
        "approve",
        "success",
        json!({
            "job_id": job_id,
            "state": state["state"],
            "approval_response": approval_response["response"],
            "allowed_next_stage": approval_response["allowed_next_stage"]
        }),
        vec![format!(
            ".ai-runs/{}/approvals/approval-response.json",
            job_id
        )],
    ))
}

fn cancel_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    let project = required_project(parsed)?;
    let job_id = required_job(parsed)?;
    let store =
        StateStore::open(&project, config.schema_root()).map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let mut state = store
        .load_state(&job_id)
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let current_state = state_string(&state);
    if TERMINAL_STATES.contains(&current_state.as_str()) {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: format!("cannot cancel terminal job state {}", current_state),
        });
    }
    let current_stage = state
        .get("current_stage")
        .and_then(Value::as_str)
        .unwrap_or("implement")
        .to_string();
    let event_id = format!("{}-cli-cancelled", job_id.to_lowercase());
    update_state_for_control_command(
        &mut state,
        &store,
        "CANCELLED",
        &current_stage,
        "stop",
        &event_id,
        None,
    )?;
    if let Some(state_object) = state.as_object_mut() {
        state_object.insert("active_provider".to_string(), Value::Null);
    }
    store
        .save_state(&job_id, &state)
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    append_cli_event(
        &store,
        &job_id,
        CliEvent {
            event_id,
            event_type: "STATE_CHANGED",
            state: "CANCELLED".to_string(),
            stage: current_stage.clone(),
            message: "Job cancelled by CLI",
            artifact_paths: vec!["run-state.json".to_string()],
            details: json!({ "previous_state": current_state }),
        },
    )
    .map_err(|source| CliError::State {
        command: parsed.command.clone(),
        source,
    })?;

    Ok(success_envelope(
        "cancel",
        "success",
        json!({
            "job_id": job_id,
            "state": "CANCELLED",
            "previous_state": current_state,
            "next_action": "stop"
        }),
        vec![format!(".ai-runs/{}/run-state.json", job_id)],
    ))
}

fn resume_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    let project = required_project(parsed)?;
    let job_id = required_job(parsed)?;
    let store =
        StateStore::open(&project, config.schema_root()).map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    store
        .ensure_resume_allowed(&job_id)
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let mut state = store
        .load_state(&job_id)
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let current_state = state_string(&state);
    let current_stage = state
        .get("current_stage")
        .and_then(Value::as_str)
        .unwrap_or("implement")
        .to_string();

    if current_state == "WAITING_APPROVAL" {
        let approval_request = load_job_json(
            &store,
            &job_id,
            "approvals/approval-request.json",
            APPROVAL_REQUEST_SCHEMA,
            &parsed.command,
            &config.schema_root(),
        )?;
        let approval_response = load_job_json(
            &store,
            &job_id,
            "approvals/approval-response.json",
            APPROVAL_RESPONSE_SCHEMA,
            &parsed.command,
            &config.schema_root(),
        )?;
        ensure_approval_response_matches_request(
            &approval_request,
            &approval_response,
            &parsed.command,
        )?;
        let event_id = format!("{}-cli-resumed", job_id.to_lowercase());
        let next_action = approval_response
            .get("allowed_next_stage")
            .and_then(Value::as_str)
            .unwrap_or("report");
        update_state_for_control_command(
            &mut state,
            &store,
            "VALIDATED",
            &current_stage,
            next_action,
            &event_id,
            None,
        )?;
        store
            .save_state(&job_id, &state)
            .map_err(|source| CliError::State {
                command: parsed.command.clone(),
                source,
            })?;
        append_cli_event(
            &store,
            &job_id,
            CliEvent {
                event_id,
                event_type: "STATE_CHANGED",
                state: "VALIDATED".to_string(),
                stage: current_stage.clone(),
                message: "Approval accepted; job is ready to continue",
                artifact_paths: vec![
                    "run-state.json".to_string(),
                    "approvals/approval-response.json".to_string(),
                ],
                details: json!({ "previous_state": current_state, "next_action": next_action }),
            },
        )
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
        return Ok(success_envelope(
            "resume",
            "success",
            json!({
                "job_id": job_id,
                "state": "VALIDATED",
                "previous_state": current_state,
                "next_action": next_action,
                "resumed": true
            }),
            vec![
                format!(".ai-runs/{}/run-state.json", job_id),
                format!(".ai-runs/{}/approvals/approval-response.json", job_id),
            ],
        ));
    }

    Ok(success_envelope(
        "resume",
        "success",
        json!({
            "job_id": job_id,
            "state": current_state,
            "current_stage": current_stage,
            "next_action": state.get("next_action").cloned().unwrap_or_else(|| json!("")),
            "resumed": false
        }),
        vec![format!(".ai-runs/{}/run-state.json", job_id)],
    ))
}

fn recover_command(parsed: &ParsedArgs, config: &CliConfig) -> Result<Value, CliError> {
    let project = required_project(parsed)?;
    let job_id = required_job(parsed)?;
    if !parsed.recovery_list {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "recover currently supports --list only".to_string(),
        });
    }
    if parsed.release_readiness
        || parsed.stage.is_some()
        || parsed.markdown
        || parsed.dry_run
        || parsed.request.is_some()
        || parsed.entrypoint.is_some()
        || parsed.provider.is_some()
        || !parsed.provider_instances.is_empty()
        || parsed.response.is_some()
        || parsed.reason.is_some()
        || !parsed.constraints.is_empty()
    {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "recover --list only accepts --project, --job, --list, and --json".to_string(),
        });
    }

    let store =
        StateStore::open(&project, config.schema_root()).map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let inspection = store
        .inspect_recovery(&job_id)
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    let inspection_value = inspection.to_value();
    let mut artifacts = vec![
        format!(".ai-runs/{}/job.json", job_id),
        format!(".ai-runs/{}/run-state.json", job_id),
        format!(".ai-runs/{}/events.jsonl", job_id),
    ];
    artifacts.extend(
        inspection
            .issues
            .iter()
            .map(|issue| format!(".ai-runs/{}/{}", job_id, issue.artifact_path)),
    );
    artifacts.sort();
    artifacts.dedup();

    Ok(success_envelope(
        "recover",
        "success",
        json!({
            "job_id": job_id,
            "mode": "inspect_only",
            "recovery_actions_enabled": false,
            "recovery": inspection_value
        }),
        artifacts,
    ))
}

fn required_project(parsed: &ParsedArgs) -> Result<PathBuf, CliError> {
    parsed
        .project
        .clone()
        .ok_or_else(|| CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "--project is required".to_string(),
        })
}

fn required_job(parsed: &ParsedArgs) -> Result<String, CliError> {
    parsed.job_id.clone().ok_or_else(|| CliError::InvalidInput {
        command: parsed.command.clone(),
        message: "--job is required".to_string(),
    })
}

fn required_response(parsed: &ParsedArgs) -> Result<String, CliError> {
    parsed
        .response
        .clone()
        .ok_or_else(|| CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "--response is required for approve".to_string(),
        })
}

fn string_field(value: &Value, field: &str, command: &str) -> Result<String, CliError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| CliError::Internal {
            command: command.to_string(),
            message: format!("missing string field {}", field),
        })
}

fn load_job_json(
    store: &StateStore,
    job_id: &str,
    relative_path: &str,
    schema_file: &str,
    command: &str,
    schema_root: &Path,
) -> Result<Value, CliError> {
    let path = store
        .resolve_job_path(job_id, relative_path)
        .map_err(|source| CliError::State {
            command: command.to_string(),
            source,
        })?;
    if !path.is_file() {
        return Err(CliError::MissingArtifact {
            command: command.to_string(),
            message: format!("required artifact not found: {}", relative_path),
            artifact_paths: vec![format!(".ai-runs/{}/{}", job_id, relative_path)],
        });
    }
    let value: Value =
        serde_json::from_str(
            &fs::read_to_string(&path).map_err(|source| CliError::Internal {
                command: command.to_string(),
                message: format!("failed to read {}: {}", path.display(), source),
            })?,
        )
        .map_err(|source| CliError::Internal {
            command: command.to_string(),
            message: format!("invalid JSON at {}: {}", path.display(), source),
        })?;
    validate_schema_value(&value, schema_root, schema_file, relative_path).map_err(|message| {
        CliError::Internal {
            command: command.to_string(),
            message,
        }
    })?;
    Ok(value)
}

fn validate_schema_value(
    value: &Value,
    schema_root: &Path,
    schema_file: &str,
    logical_path: &str,
) -> Result<(), String> {
    let schema_path = schema_root.join(schema_file);
    let schema = load_schema(&schema_path).map_err(|source| source.to_string())?;
    let result = validate_json(value, &schema);
    if result.is_ok() {
        Ok(())
    } else {
        Err(format!(
            "{} failed schema validation against {} with {} error(s)",
            logical_path,
            schema_file,
            result.errors.len()
        ))
    }
}

fn validate_approval_response_value(response: &str, command: &str) -> Result<(), CliError> {
    match response {
        "approved" | "rejected" | "needs_changes" | "cancelled" => Ok(()),
        _ => Err(CliError::InvalidInput {
            command: command.to_string(),
            message: format!("unsupported approval response {}", response),
        }),
    }
}

fn ensure_approval_response_matches_request(
    approval_request: &Value,
    approval_response: &Value,
    command: &str,
) -> Result<(), CliError> {
    for field in ["job_id", "stage", "task_id"] {
        let expected = string_field(approval_request, field, command)?;
        let actual = string_field(approval_response, field, command)?;
        if expected != actual {
            return Err(CliError::InvalidInput {
                command: command.to_string(),
                message: format!(
                    "approval response {} mismatch: expected {}, got {}",
                    field, expected, actual
                ),
            });
        }
    }
    let response = string_field(approval_response, "response", command)?;
    if response != "approved" {
        return Err(CliError::InvalidInput {
            command: command.to_string(),
            message: format!("resume requires approved response, got {}", response),
        });
    }
    Ok(())
}

fn state_string(state: &Value) -> String {
    state
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("FAILED")
        .to_string()
}

fn state_after_approval_response(response: &str) -> &'static str {
    match response {
        "approved" => "WAITING_APPROVAL",
        "cancelled" => "CANCELLED",
        _ => "BLOCKED",
    }
}

fn next_action_after_approval_response(response: &str) -> &'static str {
    match response {
        "approved" => "resume",
        "cancelled" => "stop",
        "needs_changes" => "revise",
        _ => "stop",
    }
}

fn allowed_next_stage_for(stage: &str) -> Option<&'static str> {
    match stage {
        "route" => Some("plan"),
        "plan" => Some("design"),
        "design" => Some("implement"),
        "implement" => Some("validate"),
        "validate" => Some("report"),
        "review" => Some("polish"),
        "polish" => Some("report"),
        _ => None,
    }
}

fn update_state_for_control_command(
    state: &mut Value,
    store: &StateStore,
    next_state: &str,
    current_stage: &str,
    next_action: &str,
    latest_event_id: &str,
    artifact_ref: Option<(&str, &Value)>,
) -> Result<(), CliError> {
    {
        let Some(state_object) = state.as_object_mut() else {
            return Err(CliError::Internal {
                command: "control".to_string(),
                message: "RunState must be a JSON object".to_string(),
            });
        };
        state_object.insert("state".to_string(), Value::String(next_state.to_string()));
        state_object.insert(
            "current_stage".to_string(),
            Value::String(current_stage.to_string()),
        );
        state_object.insert("updated_at".to_string(), Value::String(timestamp_string()));
        state_object.insert(
            "latest_event_id".to_string(),
            Value::String(latest_event_id.to_string()),
        );
        state_object.insert(
            "next_action".to_string(),
            Value::String(next_action.to_string()),
        );
        let history = state_object
            .entry("history")
            .or_insert_with(|| Value::Array(Vec::new()));
        let Some(history) = history.as_array_mut() else {
            return Err(CliError::Internal {
                command: "control".to_string(),
                message: "RunState history must be an array".to_string(),
            });
        };
        history.push(json!({
            "stage": current_stage,
            "state": next_state,
            "next_action": next_action,
            "event_id": latest_event_id
        }));
    }
    if let Some((key, artifact_ref)) = artifact_ref {
        store
            .register_artifact_ref(state, key, artifact_ref)
            .map_err(|source| CliError::State {
                command: "control".to_string(),
                source,
            })?;
    }
    Ok(())
}

fn append_cli_event(
    store: &StateStore,
    job_id: &str,
    event: CliEvent,
) -> Result<(), StateStoreError> {
    store.append_event(
        job_id,
        &json!({
            "schema_version": SCHEMA_VERSION,
            "event_id": event.event_id,
            "job_id": job_id,
            "type": event.event_type,
            "created_at": timestamp_string(),
            "stage": event.stage,
            "state": event.state,
            "message": event.message,
            "artifact_paths": event.artifact_paths,
            "details": event.details
        }),
    )
}

fn timestamp_string() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("unix:{}", nanos)
}

fn routed_state(job_id: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id,
        "state": "ROUTED",
        "current_stage": "implement",
        "updated_at": "cli:dry-run",
        "threads": {},
        "workers": {},
        "artifacts": {},
        "latest_event_id": "",
        "active_provider": null,
        "next_action": "run",
        "budget": {},
        "history": []
    })
}

fn report_from_provider_result(result: &Value) -> Value {
    let provider_status = result
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("error");
    let report_status = match provider_status {
        "success" => "DONE",
        "blocked" => "BLOCKED",
        _ => "FAILED",
    };
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": result.get("job_id").cloned().unwrap_or_else(|| json!("J-0000")),
        "stage": result.get("stage").cloned().unwrap_or_else(|| json!("implement")),
        "status": report_status,
        "changed_files": result.get("changed_files").cloned().unwrap_or_else(|| json!([])),
        "commands_run": [],
        "validation": [],
        "risks": [],
        "blocked_reason": if provider_status == "blocked" {
            result.pointer("/error/message").cloned().unwrap_or_else(|| json!("blocked"))
        } else {
            Value::Null
        },
        "next_step": "status",
        "artifacts": result.get("artifacts").cloned().unwrap_or_else(|| json!([]))
    })
}

fn success_envelope(command: &str, status: &str, data: Value, artifacts: Vec<String>) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "command": command,
        "status": status,
        "exit_code": 0,
        "data": data,
        "warnings": [],
        "artifacts": artifacts
    })
}

fn error_envelope(error: &CliError) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "command": error.command(),
        "status": if error.exit_code() == 1 { "blocked" } else { "failed" },
        "exit_code": error.exit_code(),
        "error": {
            "code": error.code(),
            "message": error.message(),
            "recoverable": matches!(error, CliError::InvalidInput { .. } | CliError::MissingArtifact { .. }),
            "category": error.category(),
            "artifact_paths": error.artifact_paths()
        },
        "warnings": []
    })
}

fn status_for_state(state: &str) -> &'static str {
    match state {
        "BLOCKED" => "blocked",
        "WAITING_APPROVAL" => "waiting_approval",
        "FAILED" | "CANCELLED" => "failed",
        _ => "success",
    }
}

fn status_for_report(status: &str) -> &'static str {
    match status {
        "BLOCKED" => "blocked",
        "FAILED" => "failed",
        "NEEDS_APPROVAL" => "waiting_approval",
        _ => "success",
    }
}

fn render_success(envelope: Value, json_mode: bool, config: &CliConfig) -> CliRunResult {
    let command = envelope
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    if let Err(message) = validate_cli_envelope(&envelope, &config.schema_root(), CLI_OUTPUT_SCHEMA)
    {
        return render_error(CliError::Internal { command, message }, json_mode, config);
    }
    if json_mode {
        CliRunResult {
            exit_code: 0,
            stdout: serde_json::to_string_pretty(&envelope).unwrap_or_else(|_| "{}".to_string()),
            stderr: String::new(),
        }
    } else {
        CliRunResult {
            exit_code: 0,
            stdout: human_summary(&envelope),
            stderr: String::new(),
        }
    }
}

fn render_error(error: CliError, json_mode: bool, config: &CliConfig) -> CliRunResult {
    let exit_code = error.exit_code();
    let envelope = error_envelope(&error);
    let stdout = if json_mode {
        let _ = validate_cli_envelope(&envelope, &config.schema_root(), CLI_ERROR_SCHEMA);
        serde_json::to_string_pretty(&envelope).unwrap_or_else(|_| "{}".to_string())
    } else {
        String::new()
    };
    CliRunResult {
        exit_code,
        stdout,
        stderr: error.to_string(),
    }
}

fn validate_cli_envelope(
    envelope: &Value,
    schema_root: &Path,
    schema_file: &str,
) -> Result<(), String> {
    let schema_path = schema_root.join(schema_file);
    let schema = load_schema(&schema_path).map_err(|source| source.to_string())?;
    let result = validate_json(envelope, &schema);
    if result.is_ok() {
        Ok(())
    } else {
        Err(format!(
            "CLI envelope failed schema validation with {} error(s)",
            result.errors.len()
        ))
    }
}

fn human_summary(envelope: &Value) -> String {
    let command = envelope
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("");
    let status = envelope.get("status").and_then(Value::as_str).unwrap_or("");
    let job_id = envelope
        .pointer("/data/job_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    if job_id.is_empty() {
        format!("{}: {}", command, status)
    } else {
        format!("{}: {} ({})", command, status, job_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_PROJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn run_status_and_report_json_work_for_fake_project() {
        let project = temp_project();
        let config = CliConfig::new(repo_root());

        let run = run_cli(
            [
                "run",
                "--project",
                project.to_str().expect("project path"),
                "--request",
                "runtime code 구현",
                "--provider",
                "fake-default",
                "--json",
            ],
            &config,
        );
        assert_eq!(run.exit_code, 0, "{}", run.stderr);
        let run_json: Value = serde_json::from_str(&run.stdout).expect("run json");
        assert_eq!(run_json["command"], "run");
        assert_eq!(run_json["status"], "success");
        assert_eq!(run_json["data"]["job_id"], "J-0001");
        assert_eq!(run_json["data"]["executed_stage"], "implement");
        assert!(project
            .join(".ai-runs/J-0001/provider-output/fake-default/response.json")
            .is_file());

        let status = run_cli(
            [
                "status",
                "--project",
                project.to_str().expect("project path"),
                "--job",
                "J-0001",
                "--json",
            ],
            &config,
        );
        assert_eq!(status.exit_code, 0, "{}", status.stderr);
        let status_json: Value = serde_json::from_str(&status.stdout).expect("status json");
        assert_eq!(status_json["command"], "status");
        assert_eq!(status_json["data"]["state"], "IMPLEMENTED");

        let report = run_cli(
            [
                "report",
                "--project",
                project.to_str().expect("project path"),
                "--job",
                "J-0001",
                "--stage",
                "implement",
                "--json",
            ],
            &config,
        );
        assert_eq!(report.exit_code, 0, "{}", report.stderr);
        let report_json: Value = serde_json::from_str(&report.stdout).expect("report json");
        assert_eq!(report_json["command"], "report");
        assert_eq!(report_json["data"]["report"]["status"], "DONE");

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn run_dry_run_writes_route_without_provider_output() {
        let project = temp_project();
        let config = CliConfig::new(repo_root());
        let run = run_cli(
            [
                "run",
                "--project",
                project.to_str().expect("project path"),
                "--request",
                "README 문서 수정",
                "--dry-run",
                "--json",
            ],
            &config,
        );

        assert_eq!(run.exit_code, 0, "{}", run.stderr);
        let run_json: Value = serde_json::from_str(&run.stdout).expect("run json");
        assert_eq!(run_json["data"]["dry_run"], true);
        assert!(project.join(".ai-runs/J-0001/route.json").is_file());
        assert!(!project
            .join(".ai-runs/J-0001/provider-output/fake-default/response.json")
            .exists());
        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn providers_list_and_show_are_schema_valid_and_read_only() {
        let config = CliConfig::new(repo_root());

        let list = run_cli(["providers", "list", "--json"], &config);
        assert_eq!(list.exit_code, 0, "{}", list.stderr);
        let list_json: Value = serde_json::from_str(&list.stdout).expect("providers list json");
        assert_eq!(list_json["command"], "providers");
        assert_eq!(list_json["data"]["subcommand"], "list");
        assert_eq!(list_json["data"]["actions_enabled"], false);
        assert_eq!(list_json["data"]["healthcheck_enabled"], false);
        assert_eq!(
            list_json["artifacts"].as_array().expect("artifacts").len(),
            0
        );
        let providers = list_json["data"]["providers"]
            .as_array()
            .expect("providers array");
        assert!(providers.len() >= 20);
        let fake = providers
            .iter()
            .find(|provider| provider["id"] == "provider.fake")
            .expect("provider.fake listed");
        assert_eq!(fake["kind"], "fake_provider");
        assert_eq!(
            fake["manifest_path"],
            "builtin-providers/test/fake-provider/provider.yaml"
        );

        let show = run_cli(["providers", "show", "provider.fake", "--json"], &config);
        assert_eq!(show.exit_code, 0, "{}", show.stderr);
        let show_json: Value = serde_json::from_str(&show.stdout).expect("providers show json");
        assert_eq!(show_json["command"], "providers");
        assert_eq!(show_json["data"]["subcommand"], "show");
        assert_eq!(show_json["data"]["provider"]["id"], "provider.fake");
        assert_eq!(
            show_json["data"]["capability_profile"]["provider"],
            "provider.fake"
        );
        assert_eq!(show_json["data"]["actions_enabled"], false);
        assert_eq!(show_json["data"]["healthcheck_enabled"], false);

        let show_with_option = run_cli(
            ["providers", "show", "--provider", "provider.fake", "--json"],
            &config,
        );
        assert_eq!(show_with_option.exit_code, 0, "{}", show_with_option.stderr);
        let show_with_option_json: Value =
            serde_json::from_str(&show_with_option.stdout).expect("providers show option json");
        assert_eq!(
            show_with_option_json["data"]["provider"]["id"],
            "provider.fake"
        );
    }

    #[test]
    fn providers_rejects_mutating_or_reserved_options() {
        let config = CliConfig::new(repo_root());

        let missing = run_cli(["providers", "show", "--json"], &config);
        assert_eq!(missing.exit_code, 2);
        let missing_json: Value =
            serde_json::from_str(&missing.stdout).expect("missing provider error");
        assert_eq!(missing_json["error"]["code"], "InvalidInput");

        let reserved = run_cli(["providers", "healthcheck", "--json"], &config);
        assert_eq!(reserved.exit_code, 2);
        let reserved_json: Value =
            serde_json::from_str(&reserved.stdout).expect("reserved provider error");
        assert_eq!(reserved_json["error"]["code"], "InvalidInput");
        assert!(reserved_json["error"]["message"]
            .as_str()
            .expect("message")
            .contains("reserved"));

        let invalid_option = run_cli(
            [
                "providers",
                "list",
                "--project",
                "target/not-used",
                "--json",
            ],
            &config,
        );
        assert_eq!(invalid_option.exit_code, 2);
    }

    #[test]
    fn sentinel_commands_wrap_star_sentinel_artifacts() {
        let config = CliConfig::new(repo_root());

        let selfcheck = run_cli(["sentinel", "selfcheck", "--json"], &config);
        assert_eq!(selfcheck.exit_code, 0, "{}", selfcheck.stderr);
        let selfcheck_json: Value =
            serde_json::from_str(&selfcheck.stdout).expect("selfcheck json");
        assert_eq!(selfcheck_json["command"], "sentinel");
        assert_eq!(selfcheck_json["data"]["subcommand"], "selfcheck");
        assert_eq!(selfcheck_json["data"]["ok"], true);
        assert_eq!(selfcheck_json["data"]["actions_enabled"], false);

        let check_project = temp_project();
        write_sentinel_input_job(&check_project, "p0-auto-pass", vec!["src/**"], "src/lib.rs");
        let check = run_cli(
            [
                "sentinel",
                "check",
                "--project",
                check_project.to_str().expect("project path"),
                "--job",
                "J-0001",
                "--json",
            ],
            &config,
        );
        assert_eq!(check.exit_code, 0, "{}", check.stderr);
        let check_json: Value = serde_json::from_str(&check.stdout).expect("check json");
        assert_eq!(check_json["data"]["subcommand"], "check");
        assert_eq!(check_json["data"]["decision"], "AUTO_PASS");
        assert_eq!(check_json["data"]["actions_enabled"], false);
        assert!(check_project
            .join(".ai-runs/J-0001/tool-output/star-sentinel/diagnostics.json")
            .is_file());
        assert!(!check_project
            .join(".ai-runs/J-0001/tool-output/star-sentinel/approval.json")
            .exists());

        let gate_project = temp_project();
        write_sentinel_input_job(&gate_project, "p0-human-review", vec!["**"], "Cargo.toml");
        let gate = run_cli(
            [
                "sentinel",
                "gate",
                "--project",
                gate_project.to_str().expect("project path"),
                "--job",
                "J-0001",
                "--json",
            ],
            &config,
        );
        assert_eq!(gate.exit_code, 0, "{}", gate.stderr);
        let gate_json: Value = serde_json::from_str(&gate.stdout).expect("gate json");
        assert_eq!(gate_json["status"], "waiting_approval");
        assert_eq!(gate_json["data"]["decision"], "HUMAN_REVIEW");
        assert!(gate_project
            .join(".ai-runs/J-0001/tool-output/star-sentinel/approval.json")
            .is_file());

        let review_project = temp_project();
        write_sentinel_input_job(
            &review_project,
            "p0-block",
            vec!["src/allowed/**"],
            "src/other.rs",
        );
        let review = run_cli(
            [
                "sentinel",
                "review-pack",
                "--project",
                review_project.to_str().expect("project path"),
                "--job",
                "J-0001",
                "--json",
            ],
            &config,
        );
        assert_eq!(review.exit_code, 0, "{}", review.stderr);
        let review_json: Value = serde_json::from_str(&review.stdout).expect("review json");
        assert_eq!(review_json["status"], "blocked");
        assert_eq!(review_json["data"]["decision"], "BLOCK");
        assert!(review_project
            .join(".ai-runs/J-0001/review-packs/review_pack.md")
            .is_file());

        fs::remove_dir_all(check_project).ok();
        fs::remove_dir_all(gate_project).ok();
        fs::remove_dir_all(review_project).ok();
    }

    #[test]
    fn sentinel_rejects_missing_inputs_and_reserved_options() {
        let config = CliConfig::new(repo_root());
        let project = temp_project();
        let store =
            StateStore::open(&project, repo_root().join("specs/schemas")).expect("open store");
        store
            .create_job("missing sentinel inputs", "codex", vec![])
            .expect("create job");

        let missing = run_cli(
            [
                "sentinel",
                "check",
                "--project",
                project.to_str().expect("project path"),
                "--job",
                "J-0001",
                "--json",
            ],
            &config,
        );
        assert_eq!(missing.exit_code, 3);
        let missing_json: Value = serde_json::from_str(&missing.stdout).expect("missing json");
        assert_eq!(missing_json["error"]["code"], "MissingArtifact");
        assert_eq!(
            missing_json["error"]["artifact_paths"][0],
            ".ai-runs/J-0001/tool-output/star-sentinel/task.json"
        );

        let invalid_selfcheck = run_cli(
            [
                "sentinel",
                "selfcheck",
                "--project",
                project.to_str().expect("project path"),
                "--json",
            ],
            &config,
        );
        assert_eq!(invalid_selfcheck.exit_code, 2);

        let invalid_option = run_cli(
            [
                "sentinel",
                "gate",
                "--project",
                project.to_str().expect("project path"),
                "--job",
                "J-0001",
                "--provider",
                "fake-default",
                "--json",
            ],
            &config,
        );
        assert_eq!(invalid_option.exit_code, 2);

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn report_release_readiness_reads_existing_artifact_without_mutation() {
        let project = temp_project();
        let config = CliConfig::new(repo_root());
        write_release_readiness_job(&project, true);
        let readiness_path = project.join(".ai-runs/J-0001/release/release-readiness.json");
        let before_readiness = fs::read_to_string(&readiness_path).expect("read readiness before");

        let report = run_cli(
            [
                "report",
                "--project",
                project.to_str().expect("project path"),
                "--job",
                "J-0001",
                "--release-readiness",
                "--json",
            ],
            &config,
        );

        assert_eq!(report.exit_code, 0, "{}", report.stderr);
        let report_json: Value = serde_json::from_str(&report.stdout).expect("report json");
        assert_eq!(report_json["command"], "report");
        assert_eq!(report_json["data"]["report_kind"], "release_readiness");
        assert_eq!(report_json["data"]["release_actions_enabled"], false);
        assert_eq!(
            report_json["data"]["release_readiness_path"],
            ".ai-runs/J-0001/release/release-readiness.json"
        );
        assert_eq!(report_json["data"]["readiness"]["status"], "reserved");
        assert_eq!(
            report_json["artifacts"][0],
            ".ai-runs/J-0001/release/release-readiness.json"
        );
        let after_readiness = fs::read_to_string(&readiness_path).expect("read readiness after");
        assert_eq!(after_readiness, before_readiness);
        assert!(!project
            .join(".ai-runs/J-0001/release/release-action.json")
            .exists());

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn report_release_readiness_requires_existing_artifact_and_rejects_stage() {
        let project = temp_project();
        let config = CliConfig::new(repo_root());
        write_release_readiness_job(&project, false);

        let missing = run_cli(
            [
                "report",
                "--project",
                project.to_str().expect("project path"),
                "--job",
                "J-0001",
                "--release-readiness",
                "--json",
            ],
            &config,
        );
        assert_eq!(missing.exit_code, 3);
        let missing_json: Value = serde_json::from_str(&missing.stdout).expect("missing json");
        assert_eq!(missing_json["error"]["code"], "MissingArtifact");
        assert_eq!(
            missing_json["error"]["artifact_paths"][0],
            ".ai-runs/J-0001/release/release-readiness.json"
        );

        let invalid = run_cli(
            [
                "report",
                "--project",
                project.to_str().expect("project path"),
                "--job",
                "J-0001",
                "--release-readiness",
                "--stage",
                "implement",
                "--json",
            ],
            &config,
        );
        assert_eq!(invalid.exit_code, 2);
        let invalid_json: Value = serde_json::from_str(&invalid.stdout).expect("invalid json");
        assert_eq!(invalid_json["error"]["code"], "InvalidInput");

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn recover_list_reports_inspection_without_mutation() {
        let project = temp_project();
        let config = CliConfig::new(repo_root());
        write_recovery_inspection_job(&project);
        let tmp_path = project.join(".ai-runs/J-0001/tmp/run-state.json.tmp-test");
        let state_path = project.join(".ai-runs/J-0001/run-state.json");
        let events_path = project.join(".ai-runs/J-0001/events.jsonl");
        let before_state = fs::read_to_string(&state_path).expect("state before");
        let before_events = fs::read_to_string(&events_path).expect("events before");

        let recover = run_cli(
            [
                "recover",
                "--project",
                project.to_str().expect("project path"),
                "--job",
                "J-0001",
                "--list",
                "--json",
            ],
            &config,
        );

        assert_eq!(recover.exit_code, 0, "{}", recover.stderr);
        let recover_json: Value = serde_json::from_str(&recover.stdout).expect("recover json");
        assert_eq!(recover_json["command"], "recover");
        assert_eq!(recover_json["status"], "success");
        assert_eq!(recover_json["data"]["mode"], "inspect_only");
        assert_eq!(recover_json["data"]["recovery_actions_enabled"], false);
        assert_eq!(recover_json["data"]["recovery"]["status"], "needs_recovery");
        assert_eq!(
            recover_json["data"]["recovery"]["destructive_actions_performed"],
            false
        );
        assert_eq!(
            recover_json["data"]["recovery"]["issues"][0]["kind"],
            "partial_tmp_file"
        );
        assert_eq!(
            recover_json["data"]["recovery"]["issues"][0]["artifact_path"],
            "tmp/run-state.json.tmp-test"
        );
        assert!(recover_json["artifacts"]
            .as_array()
            .expect("artifacts")
            .contains(&json!(".ai-runs/J-0001/tmp/run-state.json.tmp-test")));
        assert_eq!(
            fs::read_to_string(&state_path).expect("state after"),
            before_state
        );
        assert_eq!(
            fs::read_to_string(&events_path).expect("events after"),
            before_events
        );
        assert!(tmp_path.is_file());
        assert!(!project.join(".ai-runs/J-0001/recovery").exists());

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn recover_requires_list_and_rejects_non_recovery_options() {
        let project = temp_project();
        let config = CliConfig::new(repo_root());
        write_recovery_inspection_job(&project);

        let missing_mode = run_cli(
            [
                "recover",
                "--project",
                project.to_str().expect("project path"),
                "--job",
                "J-0001",
                "--json",
            ],
            &config,
        );
        assert_eq!(missing_mode.exit_code, 2);
        let missing_mode_json: Value =
            serde_json::from_str(&missing_mode.stdout).expect("missing mode json");
        assert_eq!(missing_mode_json["error"]["code"], "InvalidInput");

        let invalid_combo = run_cli(
            [
                "recover",
                "--project",
                project.to_str().expect("project path"),
                "--job",
                "J-0001",
                "--list",
                "--stage",
                "implement",
                "--json",
            ],
            &config,
        );
        assert_eq!(invalid_combo.exit_code, 2);
        let invalid_combo_json: Value =
            serde_json::from_str(&invalid_combo.stdout).expect("invalid combo json");
        assert_eq!(invalid_combo_json["error"]["code"], "InvalidInput");

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn run_with_local_process_provider_instance_executes_process() {
        let project = temp_project();
        let provider_instance = write_local_process_instance(&project, vec!["--help".to_string()]);
        let config = CliConfig::new(repo_root());

        let run = run_cli(
            [
                "run",
                "--project",
                project.to_str().expect("project path"),
                "--request",
                "runtime code 구현",
                "--provider",
                "local-default",
                "--provider-instance",
                provider_instance.to_str().expect("provider instance path"),
                "--json",
            ],
            &config,
        );

        assert_eq!(run.exit_code, 0, "{}", run.stderr);
        let run_json: Value = serde_json::from_str(&run.stdout).expect("run json");
        assert_eq!(run_json["command"], "run");
        assert_eq!(run_json["status"], "success");
        assert_eq!(run_json["data"]["state"], "IMPLEMENTED");
        assert!(project
            .join(".ai-runs/J-0001/provider-output/local-default/response.json")
            .is_file());
        assert!(project
            .join(".ai-runs/J-0001/provider-output/local-default/stdout.txt")
            .is_file());
        assert!(!project
            .join(".ai-runs/J-0001/provider-output/fake-default/response.json")
            .exists());

        let route: Value = serde_json::from_str(
            &fs::read_to_string(project.join(".ai-runs/J-0001/route.json")).expect("route"),
        )
        .expect("route json");
        assert_eq!(
            route["assignments"]["implement"]["provider"],
            "local-default"
        );
        let workspec: Value = serde_json::from_str(
            &fs::read_to_string(project.join(".ai-runs/J-0001/workspecs/implement.json"))
                .expect("workspec"),
        )
        .expect("workspec json");
        assert_eq!(workspec["provider_instance"], "local-default");

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn approve_writes_response_and_resume_advances_waiting_approval_gate() {
        let project = temp_project();
        let config = CliConfig::new(repo_root());
        write_waiting_approval_job(&project, true);

        let approve = run_cli(
            [
                "approve",
                "--project",
                project.to_str().expect("project path"),
                "--job",
                "J-0001",
                "--response",
                "approved",
                "--reason",
                "approved by CLI test",
                "--constraint",
                "keep validation strict",
                "--json",
            ],
            &config,
        );
        assert_eq!(approve.exit_code, 0, "{}", approve.stderr);
        let approve_json: Value = serde_json::from_str(&approve.stdout).expect("approve json");
        assert_eq!(approve_json["command"], "approve");
        assert_eq!(approve_json["status"], "success");
        assert_eq!(approve_json["data"]["state"], "WAITING_APPROVAL");
        assert_eq!(approve_json["data"]["approval_response"], "approved");
        assert_eq!(approve_json["data"]["allowed_next_stage"], "report");
        assert!(project
            .join(".ai-runs/J-0001/approvals/approval-response.json")
            .is_file());

        let store = StateStore::open(&project, repo_root().join("specs/schemas")).expect("store");
        let approved_state = store.load_state("J-0001").expect("state after approve");
        assert_eq!(approved_state["state"], "WAITING_APPROVAL");
        assert_eq!(approved_state["next_action"], "resume");
        assert_eq!(
            approved_state["artifacts"]["approval_response"]["path"],
            "approvals/approval-response.json"
        );

        let resume = run_cli(
            [
                "resume",
                "--project",
                project.to_str().expect("project path"),
                "--job",
                "J-0001",
                "--json",
            ],
            &config,
        );
        assert_eq!(resume.exit_code, 0, "{}", resume.stderr);
        let resume_json: Value = serde_json::from_str(&resume.stdout).expect("resume json");
        assert_eq!(resume_json["command"], "resume");
        assert_eq!(resume_json["data"]["previous_state"], "WAITING_APPROVAL");
        assert_eq!(resume_json["data"]["state"], "VALIDATED");
        assert_eq!(resume_json["data"]["next_action"], "report");
        let resumed_state = store.load_state("J-0001").expect("state after resume");
        assert_eq!(resumed_state["state"], "VALIDATED");
        assert_eq!(resumed_state["next_action"], "report");
        let events = store.read_events("J-0001").expect("events");
        assert!(events
            .iter()
            .any(|event| event["type"] == "APPROVAL_RECORDED"));
        assert!(events
            .iter()
            .any(|event| { event["type"] == "STATE_CHANGED" && event["state"] == "VALIDATED" }));

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn approve_requires_approval_request_artifact() {
        let project = temp_project();
        let config = CliConfig::new(repo_root());
        write_waiting_approval_job(&project, false);

        let approve = run_cli(
            [
                "approve",
                "--project",
                project.to_str().expect("project path"),
                "--job",
                "J-0001",
                "--response",
                "approved",
                "--reason",
                "approved by CLI test",
                "--json",
            ],
            &config,
        );
        assert_eq!(approve.exit_code, 3);
        let error_json: Value = serde_json::from_str(&approve.stdout).expect("approve error json");
        assert_eq!(error_json["error"]["code"], "MissingArtifact");
        assert_eq!(
            error_json["error"]["artifact_paths"][0],
            ".ai-runs/J-0001/approvals/approval-request.json"
        );

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn resume_waiting_approval_requires_approved_response() {
        let project = temp_project();
        let config = CliConfig::new(repo_root());
        write_waiting_approval_job(&project, true);

        let resume = run_cli(
            [
                "resume",
                "--project",
                project.to_str().expect("project path"),
                "--job",
                "J-0001",
                "--json",
            ],
            &config,
        );
        assert_eq!(resume.exit_code, 3);
        let error_json: Value = serde_json::from_str(&resume.stdout).expect("resume error json");
        assert_eq!(error_json["error"]["code"], "MissingArtifact");
        assert_eq!(
            error_json["error"]["artifact_paths"][0],
            ".ai-runs/J-0001/approvals/approval-response.json"
        );

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn cancel_updates_nonterminal_state_and_rejects_terminal_cancel() {
        let project = temp_project();
        let config = CliConfig::new(repo_root());
        let run = run_cli(
            [
                "run",
                "--project",
                project.to_str().expect("project path"),
                "--request",
                "README 문서 수정",
                "--dry-run",
                "--json",
            ],
            &config,
        );
        assert_eq!(run.exit_code, 0, "{}", run.stderr);

        let cancel = run_cli(
            [
                "cancel",
                "--project",
                project.to_str().expect("project path"),
                "--job",
                "J-0001",
                "--json",
            ],
            &config,
        );
        assert_eq!(cancel.exit_code, 0, "{}", cancel.stderr);
        let cancel_json: Value = serde_json::from_str(&cancel.stdout).expect("cancel json");
        assert_eq!(cancel_json["command"], "cancel");
        assert_eq!(cancel_json["data"]["previous_state"], "ROUTED");
        assert_eq!(cancel_json["data"]["state"], "CANCELLED");
        let store = StateStore::open(&project, repo_root().join("specs/schemas")).expect("store");
        assert_eq!(
            store.load_state("J-0001").expect("state")["state"],
            "CANCELLED"
        );

        let second_cancel = run_cli(
            [
                "cancel",
                "--project",
                project.to_str().expect("project path"),
                "--job",
                "J-0001",
                "--json",
            ],
            &config,
        );
        assert_eq!(second_cancel.exit_code, 2);
        let error_json: Value =
            serde_json::from_str(&second_cancel.stdout).expect("cancel error json");
        assert_eq!(error_json["error"]["code"], "InvalidInput");

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn missing_job_returns_schema_valid_error() {
        let project = temp_project();
        let config = CliConfig::new(repo_root());
        let result = run_cli(
            [
                "status",
                "--project",
                project.to_str().expect("project path"),
                "--job",
                "J-9999",
                "--json",
            ],
            &config,
        );

        assert_eq!(result.exit_code, 3);
        let error_json: Value = serde_json::from_str(&result.stdout).expect("error json");
        assert_eq!(error_json["command"], "status");
        assert_eq!(error_json["error"]["code"], "StateReadFailed");
        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn non_default_provider_requires_provider_instance_path() {
        let project = temp_project();
        let config = CliConfig::new(repo_root());
        let result = run_cli(
            [
                "run",
                "--project",
                project.to_str().expect("project path"),
                "--request",
                "runtime code 구현",
                "--provider",
                "local-default",
                "--json",
            ],
            &config,
        );

        assert_eq!(result.exit_code, 2);
        let error_json: Value = serde_json::from_str(&result.stdout).expect("error json");
        assert_eq!(error_json["error"]["code"], "InvalidInput");
        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn provider_instance_path_requires_explicit_provider() {
        let project = temp_project();
        let provider_instance = write_local_process_instance(&project, vec!["--help".to_string()]);
        let config = CliConfig::new(repo_root());
        let result = run_cli(
            [
                "run",
                "--project",
                project.to_str().expect("project path"),
                "--request",
                "runtime code 구현",
                "--provider-instance",
                provider_instance.to_str().expect("provider instance path"),
                "--json",
            ],
            &config,
        );

        assert_eq!(result.exit_code, 2);
        let error_json: Value = serde_json::from_str(&result.stdout).expect("error json");
        assert_eq!(error_json["error"]["code"], "InvalidInput");
        fs::remove_dir_all(project).ok();
    }

    fn temp_project() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let counter = TEMP_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "star-control-cli-{}-{}-{}",
            std::process::id(),
            nanos,
            counter
        ));
        fs::create_dir_all(&path).expect("create temp project");
        path
    }

    fn write_local_process_instance(project: &Path, args: Vec<String>) -> PathBuf {
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

    fn write_sentinel_input_job(
        project: &Path,
        task_id: &str,
        allowed_paths: Vec<&str>,
        changed_path: &str,
    ) {
        let store =
            StateStore::open(project, repo_root().join("specs/schemas")).expect("open store");
        store
            .create_job("sentinel input", "codex", vec![])
            .expect("create job");
        store
            .write_tool_json(
                "J-0001",
                "star-sentinel",
                "task.json",
                &sentinel_task_value(task_id, allowed_paths),
            )
            .expect("write sentinel task");
        store
            .write_tool_json(
                "J-0001",
                "star-sentinel",
                "changed_lines.json",
                &changed_lines_value(task_id, changed_path),
            )
            .expect("write changed lines");
    }

    fn sentinel_task_value(task_id: &str, allowed_paths: Vec<&str>) -> Value {
        json!({
            "schema_version": "1.0.0",
            "task_id": task_id,
            "goal": "Validate a scoped CLI sentinel fixture.",
            "allowed_paths": allowed_paths,
            "forbidden_paths": [
                ".github/workflows/**",
                "package.json",
                "package-lock.json"
            ],
            "forbidden_change_types": [
                "test_deletion",
                "assertion_weakening",
                "validator_bypass",
                "secret_exposure"
            ],
            "required_validation": [
                "policy:p0"
            ],
            "approval_required_changes": [
                "public_api_change",
                "schema_change",
                "dependency_addition"
            ],
            "notes": "CLI sentinel command fixture."
        })
    }

    fn changed_lines_value(task_id: &str, path: &str) -> Value {
        json!({
            "schema_version": "1.0.0",
            "task_id": task_id,
            "files": [
                {
                    "path": path,
                    "change_type": "modified",
                    "old_path": null,
                    "hunks": [
                        {
                            "old_start": 1,
                            "old_lines": 2,
                            "new_start": 1,
                            "new_lines": 3,
                            "lines": [
                                {
                                    "kind": "context",
                                    "old_line": 1,
                                    "new_line": 1,
                                    "content": "fn existing() {}"
                                },
                                {
                                    "kind": "added",
                                    "old_line": null,
                                    "new_line": 2,
                                    "content": "fn added() {}"
                                }
                            ]
                        }
                    ]
                }
            ]
        })
    }

    fn write_waiting_approval_job(project: &Path, include_request: bool) {
        let store =
            StateStore::open(project, repo_root().join("specs/schemas")).expect("open store");
        store
            .create_job("needs approval", "codex", vec![])
            .expect("create job");
        store
            .save_state(
                "J-0001",
                &json!({
                    "schema_version": "1.0.0",
                    "job_id": "J-0001",
                    "state": "WAITING_APPROVAL",
                    "current_stage": "validate",
                    "updated_at": "test:waiting-approval",
                    "threads": {},
                    "workers": {},
                    "artifacts": {},
                    "latest_event_id": "",
                    "active_provider": null,
                    "next_action": "await_approval",
                    "budget": {},
                    "history": []
                }),
            )
            .expect("save waiting approval state");
        if include_request {
            store
                .write_approval_json(
                    "J-0001",
                    "approval-request.json",
                    &json!({
                        "schema_version": "1.0.0",
                        "job_id": "J-0001",
                        "stage": "validate",
                        "task_id": "p0-human-review",
                        "decision": "HUMAN_REVIEW",
                        "reasons": ["dependency_change_requires_approval"],
                        "changed_files": ["Cargo.toml"],
                        "risks": ["dependency update"],
                        "diagnostics": [
                            {
                                "rule_id": "dependency.requires_approval",
                                "severity": "human_review",
                                "message": "Review dependency change before continuing."
                            }
                        ],
                        "review_pack_path": "review-packs/review_pack.md",
                        "requested_at": "2026-07-01T00:00:00Z",
                        "requested_by": "star-sentinel"
                    }),
                )
                .expect("write approval request");
        }
    }

    fn write_release_readiness_job(project: &Path, include_readiness: bool) {
        let store =
            StateStore::open(project, repo_root().join("specs/schemas")).expect("open store");
        store
            .create_job("release readiness", "codex", vec![])
            .expect("create job");
        if include_readiness {
            let path = project.join(".ai-runs/J-0001/release/release-readiness.json");
            fs::create_dir_all(path.parent().expect("release dir")).expect("create release dir");
            fs::write(
                &path,
                serde_json::to_vec_pretty(&json!({
                    "schema_version": "1.0.0",
                    "release_id": "release-0008",
                    "target": "star-control",
                    "version": "1.2.3",
                    "status": "reserved",
                    "checks": [
                        {
                            "name": "release-profile-passed",
                            "status": "pass",
                            "evidence_paths": ["review-packs/release-profile.json"]
                        }
                    ],
                    "blockers": [
                        "release approval/signing/publish/deploy automation remains reserved"
                    ],
                    "approvals": [],
                    "generated_at": "unix:8"
                }))
                .expect("release readiness JSON"),
            )
            .expect("write release readiness");
        }
    }

    fn write_recovery_inspection_job(project: &Path) {
        let store =
            StateStore::open(project, repo_root().join("specs/schemas")).expect("open store");
        store
            .create_job("recovery inspection", "codex", vec![])
            .expect("create job");
        store
            .save_state(
                "J-0001",
                &json!({
                    "schema_version": "1.0.0",
                    "job_id": "J-0001",
                    "state": "DONE",
                    "current_stage": "report",
                    "updated_at": "test:recovery",
                    "threads": {},
                    "workers": {},
                    "artifacts": {},
                    "latest_event_id": "J-0001-0001",
                    "active_provider": null,
                    "next_action": "none",
                    "budget": {},
                    "history": []
                }),
            )
            .expect("save recovery state");
        let tmp_path = project.join(".ai-runs/J-0001/tmp/run-state.json.tmp-test");
        fs::write(&tmp_path, b"{\"partial\":true").expect("write tmp file");
    }

    fn current_test_executable() -> String {
        std::env::current_exe()
            .expect("current test executable")
            .display()
            .to_string()
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("packages dir")
            .parent()
            .expect("repo root")
            .to_path_buf()
    }
}
