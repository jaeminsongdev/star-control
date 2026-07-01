use serde_json::{json, Value};
use star_control_execution::{ExecutionEngine, ExecutionError};
use star_control_provider::{ProviderRegistryError, ProviderRegistryLoader};
use star_control_router::{JobSpec, RouterEngine, RouterError};
use star_control_schema::{load_schema, validate_json};
use star_control_state::{StateStore, StateStoreError};
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

const CLI_OUTPUT_SCHEMA: &str = "cli-output.schema.json";
const CLI_ERROR_SCHEMA: &str = "cli-error.schema.json";
const SCHEMA_VERSION: &str = "1.0.0";
const DEFAULT_PROVIDER: &str = "fake-default";
const DEFAULT_ENTRYPOINT: &str = "star-control";

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
            | Self::Execution { command, .. }
            | Self::Internal { command, .. } => command,
        }
    }

    fn exit_code(&self) -> i32 {
        match self {
            Self::InvalidInput { .. } => 2,
            Self::MissingArtifact { .. } | Self::State { .. } => 3,
            Self::ProviderExecution { .. } | Self::Execution { .. } => 4,
            Self::Router { .. } | Self::ProviderRegistry { .. } | Self::Internal { .. } => 5,
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
    project: Option<PathBuf>,
    job_id: Option<String>,
    request: Option<String>,
    entrypoint: Option<String>,
    provider: Option<String>,
    stage: Option<String>,
    dry_run: bool,
    json: bool,
    markdown: bool,
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
        project: None,
        job_id: None,
        request: None,
        entrypoint: None,
        provider: None,
        stage: None,
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
            "--stage" => {
                parsed.stage = Some(require_option_value(args, &mut index, "--stage", &command)?);
            }
            "--dry-run" => parsed.dry_run = true,
            "--json" => parsed.json = true,
            "--markdown" => parsed.markdown = true,
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
    if provider != DEFAULT_PROVIDER {
        return Err(CliError::InvalidInput {
            command: parsed.command.clone(),
            message: "v0 fake flow supports only --provider fake-default".to_string(),
        });
    }

    let schemas = config.schema_root();
    let store = StateStore::open(&project, &schemas).map_err(|source| CliError::State {
        command: parsed.command.clone(),
        source,
    })?;
    let registry = ProviderRegistryLoader::new(config.repo_root())
        .load_fake_default_registry()
        .map_err(|source| CliError::ProviderRegistry {
            command: parsed.command.clone(),
            source,
        })?;
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
    store
        .save_route(&job_id, route_output.route().value())
        .map_err(|source| CliError::State {
            command: parsed.command.clone(),
            source,
        })?;
    for (stage, workspec) in route_output.workspecs() {
        store
            .save_workspec(&job_id, stage, workspec.value())
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
        let report = report_from_provider_result(outcome.provider_execution().result().value());
        store
            .save_report(&job_id, &format!("{}-report", stage), &report)
            .map_err(|source| CliError::State {
                command: parsed.command.clone(),
                source,
            })?;
        artifacts.extend([
            format!(".ai-runs/{}/run-state.json", job_id),
            format!(
                ".ai-runs/{}/provider-output/{}/request.json",
                job_id, DEFAULT_PROVIDER
            ),
            format!(
                ".ai-runs/{}/provider-output/{}/response.json",
                job_id, DEFAULT_PROVIDER
            ),
            format!(".ai-runs/{}/reports/{}-report.json", job_id, stage),
        ]);
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
    use std::time::{SystemTime, UNIX_EPOCH};

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
    fn unsupported_provider_is_invalid_input() {
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
                "codex",
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
        let path =
            std::env::temp_dir().join(format!("star-control-cli-{}-{}", std::process::id(), nanos));
        fs::create_dir_all(&path).expect("create temp project");
        path
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
