use serde_json::{json, Value};
use star_control_provider::{
    is_cloud_cli_manifest, is_cloud_provider_manifest, CloudCliProviderAdapter,
    CloudProviderPreflightAdapter, ExecutionRequest, FakeProviderAdapter,
    LocalProcessProviderAdapter, ProviderAdapter, ProviderAdapterError, ProviderExecution,
    ProviderRegistry, ProviderRegistryError, ProviderRunContext,
};
use star_control_schema::{load_schema, validate_json, ValidationError};
use star_control_state::{StateStore, StateStoreError};
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

const EXECUTION_ATTEMPT_SCHEMA: &str = "execution-attempt.schema.json";
const SCHEMA_VERSION: &str = "1.0.0";
const FAKE_PROVIDER_ID: &str = "provider.fake";
const LOCAL_PROCESS_KIND: &str = "local_process_model";
const PROCESS_TRANSPORT: &str = "process";

#[derive(Debug)]
pub enum ExecutionError {
    SchemaLoadFailed {
        path: PathBuf,
        message: String,
    },
    SchemaValidationFailed {
        path: PathBuf,
        schema_path: PathBuf,
        errors: Vec<ValidationError>,
    },
    MissingField {
        path: PathBuf,
        field: String,
    },
    InvalidFieldType {
        path: PathBuf,
        field: String,
        expected: String,
    },
    ProviderRegistry(ProviderRegistryError),
    ProviderAdapter(ProviderAdapterError),
    State(StateStoreError),
    ProviderAssignmentMissing {
        stage: String,
    },
    ProviderAssignmentMismatch {
        provider: String,
        provider_instance: String,
    },
    ProviderOutputMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    StageAlreadyExecuted {
        job_id: String,
        stage: String,
        provider_instance_id: String,
    },
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaLoadFailed { path, message } => {
                write!(
                    formatter,
                    "failed to load schema {}: {}",
                    path.display(),
                    message
                )
            }
            Self::SchemaValidationFailed {
                path,
                schema_path,
                errors,
            } => write!(
                formatter,
                "schema validation failed for {} against {} with {} error(s)",
                path.display(),
                schema_path.display(),
                errors.len()
            ),
            Self::MissingField { path, field } => {
                write!(formatter, "missing field {} in {}", field, path.display())
            }
            Self::InvalidFieldType {
                path,
                field,
                expected,
            } => write!(
                formatter,
                "invalid field type for {} in {}, expected {}",
                field,
                path.display(),
                expected
            ),
            Self::ProviderRegistry(source) => {
                write!(formatter, "provider registry error: {}", source)
            }
            Self::ProviderAdapter(source) => {
                write!(formatter, "provider adapter error: {}", source)
            }
            Self::State(source) => write!(formatter, "state store error: {}", source),
            Self::ProviderAssignmentMissing { stage } => {
                write!(formatter, "provider assignment missing for stage {}", stage)
            }
            Self::ProviderAssignmentMismatch {
                provider,
                provider_instance,
            } => write!(
                formatter,
                "workspec provider {} does not match provider_instance {}",
                provider, provider_instance
            ),
            Self::ProviderOutputMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "provider output mismatch for {}: expected {}, got {}",
                field, expected, actual
            ),
            Self::StageAlreadyExecuted {
                job_id,
                stage,
                provider_instance_id,
            } => write!(
                formatter,
                "stage {} for job {} already has provider output for {}",
                stage, job_id, provider_instance_id
            ),
        }
    }
}

impl Error for ExecutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ProviderRegistry(source) => Some(source),
            Self::ProviderAdapter(source) => Some(source),
            Self::State(source) => Some(source),
            _ => None,
        }
    }
}

impl From<ProviderRegistryError> for ExecutionError {
    fn from(source: ProviderRegistryError) -> Self {
        Self::ProviderRegistry(source)
    }
}

impl From<ProviderAdapterError> for ExecutionError {
    fn from(source: ProviderAdapterError) -> Self {
        Self::ProviderAdapter(source)
    }
}

impl From<StateStoreError> for ExecutionError {
    fn from(source: StateStoreError) -> Self {
        Self::State(source)
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionEngine<'a> {
    state_store: &'a StateStore,
    registry: &'a ProviderRegistry,
    schema_root: PathBuf,
    fake_adapter: FakeProviderAdapter,
    local_process_adapter: LocalProcessProviderAdapter,
    cloud_cli_adapter: CloudCliProviderAdapter,
    cloud_provider_adapter: CloudProviderPreflightAdapter,
}

impl<'a> ExecutionEngine<'a> {
    pub fn new(
        state_store: &'a StateStore,
        registry: &'a ProviderRegistry,
        schema_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            state_store,
            registry,
            schema_root: schema_root.into(),
            fake_adapter: FakeProviderAdapter::success(),
            local_process_adapter: LocalProcessProviderAdapter,
            cloud_cli_adapter: CloudCliProviderAdapter,
            cloud_provider_adapter: CloudProviderPreflightAdapter,
        }
    }

    pub fn with_fake_adapter(mut self, adapter: FakeProviderAdapter) -> Self {
        self.fake_adapter = adapter;
        self
    }

    pub fn execute_stage(
        &self,
        job_id: &str,
        stage: &str,
    ) -> Result<ExecutionOutcome, ExecutionError> {
        let job = self.state_store.load_job(job_id)?;
        let workspec = self.state_store.load_workspec(job_id, stage)?;
        let assignment = ProviderAssignment::from_workspec(&workspec, stage)?;
        self.registry
            .instance(&assignment.provider_instance)
            .ok_or_else(|| ProviderRegistryError::InstanceNotFound {
                instance_id: assignment.provider_instance.clone(),
            })?;
        self.ensure_stage_not_executed(job_id, stage, &assignment.provider_instance)?;

        let request = self.execution_request(&job, &workspec, &assignment)?;
        let attempt = execution_attempt(&request, "running");
        validate_contract(
            &attempt,
            Path::new("execution-attempt.json"),
            &self.schema_root,
            EXECUTION_ATTEMPT_SCHEMA,
        )?;

        self.append_event(
            job_id,
            stage,
            "PROVIDER_STARTED",
            "Provider execution started",
            &[format!(
                "provider-output/{}/request.json",
                request.provider_instance_id()
            )],
            json!({
                "provider_instance_id": request.provider_instance_id(),
                "attempt_id": attempt["attempt_id"]
            }),
        )?;

        let context = ProviderRunContext::new(self.registry, self.state_store, &self.schema_root);
        let provider_execution = self.execute_provider(&request, &context)?;
        verify_provider_result(&request, &provider_execution)?;

        let completed_attempt = execution_attempt(&request, provider_execution.result().status());
        validate_contract(
            &completed_attempt,
            Path::new("execution-attempt.json"),
            &self.schema_root,
            EXECUTION_ATTEMPT_SCHEMA,
        )?;
        let state = self.update_run_state(&job, stage, &provider_execution, &completed_attempt)?;

        self.append_event(
            job_id,
            stage,
            "PROVIDER_FINISHED",
            "Provider execution finished",
            &[
                format!(
                    "provider-output/{}/request.json",
                    request.provider_instance_id()
                ),
                format!(
                    "provider-output/{}/response.json",
                    request.provider_instance_id()
                ),
            ],
            json!({
                "provider_instance_id": request.provider_instance_id(),
                "attempt_id": completed_attempt["attempt_id"],
                "status": provider_execution.result().status()
            }),
        )?;

        Ok(ExecutionOutcome {
            request,
            provider_execution,
            attempt: completed_attempt,
            state,
        })
    }

    fn execute_provider(
        &self,
        request: &ExecutionRequest,
        context: &ProviderRunContext<'_>,
    ) -> Result<ProviderExecution, ExecutionError> {
        let manifest = self
            .registry
            .manifest_for_instance(request.provider_instance_id())?;
        if manifest.id() == FAKE_PROVIDER_ID {
            return Ok(self.fake_adapter.execute(request, context)?);
        }
        if manifest.kind() == LOCAL_PROCESS_KIND && manifest.transport() == PROCESS_TRANSPORT {
            return Ok(self.local_process_adapter.execute(request, context)?);
        }
        if is_cloud_cli_manifest(manifest) {
            return Ok(self.cloud_cli_adapter.execute(request, context)?);
        }
        if is_cloud_provider_manifest(manifest) {
            return Ok(self.cloud_provider_adapter.execute(request, context)?);
        }

        Err(ProviderAdapterError::UnsupportedProvider {
            provider_instance_id: request.provider_instance_id().to_string(),
            provider_id: manifest.id().to_string(),
        }
        .into())
    }

    fn execution_request(
        &self,
        job: &Value,
        workspec: &Value,
        assignment: &ProviderAssignment,
    ) -> Result<ExecutionRequest, ExecutionError> {
        let job_path = Path::new("job.json");
        let workspec_path = Path::new("workspec.json");
        let job_id = required_string(job, job_path, "job_id")?;
        let stage = required_string(workspec, workspec_path, "stage")?;
        let created_at = required_string(job, job_path, "created_at")?;
        let goal = required_string(workspec, workspec_path, "goal")?;

        let request_value = json!({
            "schema_version": SCHEMA_VERSION,
            "request_id": format!("{}-{}-request-0001", job_id.to_lowercase(), stage),
            "job_id": job_id,
            "stage": stage,
            "provider_instance_id": assignment.provider_instance,
            "attempt_id": "attempt-0001",
            "workspec_path": format!("workspecs/{}.json", stage),
            "created_at": created_at,
            "goal": goal,
            "allowed_scope": workspec.get("allowed_scope").cloned().unwrap_or_else(|| json!([])),
            "forbidden_actions": workspec
                .get("forbidden_actions")
                .cloned()
                .unwrap_or_else(|| json!([])),
            "required_outputs": workspec
                .get("required_outputs")
                .cloned()
                .unwrap_or_else(|| json!([])),
            "validation_requirements": workspec
                .get("validation_requirements")
                .cloned()
                .unwrap_or_else(|| json!([])),
            "context_pack": workspec
                .get("context_pack")
                .cloned()
                .unwrap_or_else(|| json!({}))
        });

        ExecutionRequest::from_value(request_value, "execution-request.json", &self.schema_root)
            .map_err(ExecutionError::from)
    }

    fn ensure_stage_not_executed(
        &self,
        job_id: &str,
        stage: &str,
        provider_instance_id: &str,
    ) -> Result<(), ExecutionError> {
        let response_path = self.state_store.resolve_job_path(
            job_id,
            &format!("provider-output/{}/response.json", provider_instance_id),
        )?;
        if response_path.exists() {
            return Err(ExecutionError::StageAlreadyExecuted {
                job_id: job_id.to_string(),
                stage: stage.to_string(),
                provider_instance_id: provider_instance_id.to_string(),
            });
        }
        Ok(())
    }

    fn update_run_state(
        &self,
        job: &Value,
        stage: &str,
        provider_execution: &ProviderExecution,
        attempt: &Value,
    ) -> Result<Value, ExecutionError> {
        let job_path = Path::new("job.json");
        let job_id = required_string(job, job_path, "job_id")?;
        let created_at = required_string(job, job_path, "created_at")?;
        let mut state = match self.state_store.load_state(&job_id) {
            Ok(state) => state,
            Err(StateStoreError::ArtifactNotFound { .. }) => {
                initial_state(&job_id, stage, &created_at)
            }
            Err(source) => return Err(ExecutionError::State(source)),
        };

        let result = provider_execution.result();
        let next_state = state_for_provider_status(stage, result.status());
        set_object_field(&mut state, "state", Value::String(next_state.to_string()))?;
        set_object_field(
            &mut state,
            "current_stage",
            Value::String(stage.to_string()),
        )?;
        set_object_field(&mut state, "updated_at", Value::String(created_at))?;
        set_object_field(&mut state, "active_provider", Value::Null)?;
        set_object_field(
            &mut state,
            "latest_event_id",
            Value::String(format!(
                "{}-{}-provider-finished",
                job_id.to_lowercase(),
                stage
            )),
        )?;

        self.state_store.register_artifact_ref(
            &mut state,
            &format!("{}_provider_request", stage),
            provider_execution.request_ref(),
        )?;
        self.state_store.register_artifact_ref(
            &mut state,
            &format!("{}_provider_response", stage),
            provider_execution.response_ref(),
        )?;
        self.state_store.register_artifact_ref(
            &mut state,
            &format!("{}_provider_stdout", stage),
            provider_execution.stdout_ref(),
        )?;
        if let Some(stderr_ref) = provider_execution.stderr_ref() {
            self.state_store.register_artifact_ref(
                &mut state,
                &format!("{}_provider_stderr", stage),
                stderr_ref,
            )?;
        }

        push_history(
            &mut state,
            json!({
                "stage": stage,
                "provider_instance_id": result.provider_instance_id(),
                "status": result.status(),
                "attempt": attempt
            }),
        )?;
        self.state_store.save_state(&job_id, &state)?;
        Ok(state)
    }

    fn append_event(
        &self,
        job_id: &str,
        stage: &str,
        event_type: &str,
        message: &str,
        artifact_paths: &[String],
        details: Value,
    ) -> Result<(), ExecutionError> {
        let event = json!({
            "schema_version": SCHEMA_VERSION,
            "event_id": format!("{}-{}-{}", job_id.to_lowercase(), stage, event_type.to_lowercase().replace('_', "-")),
            "job_id": job_id,
            "type": event_type,
            "created_at": "execution:deterministic",
            "stage": stage,
            "state": "",
            "message": message,
            "artifact_paths": artifact_paths,
            "details": details
        });
        self.state_store.append_event(job_id, &event)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionOutcome {
    request: ExecutionRequest,
    provider_execution: ProviderExecution,
    attempt: Value,
    state: Value,
}

impl ExecutionOutcome {
    pub fn request(&self) -> &ExecutionRequest {
        &self.request
    }

    pub fn provider_execution(&self) -> &ProviderExecution {
        &self.provider_execution
    }

    pub fn attempt(&self) -> &Value {
        &self.attempt
    }

    pub fn state(&self) -> &Value {
        &self.state
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderAssignment {
    provider: String,
    provider_instance: String,
}

impl ProviderAssignment {
    fn from_workspec(workspec: &Value, stage: &str) -> Result<Self, ExecutionError> {
        let path = Path::new("workspec.json");
        let provider = required_string(workspec, path, "provider").map_err(|_| {
            ExecutionError::ProviderAssignmentMissing {
                stage: stage.to_string(),
            }
        })?;
        let provider_instance =
            required_string(workspec, path, "provider_instance").map_err(|_| {
                ExecutionError::ProviderAssignmentMissing {
                    stage: stage.to_string(),
                }
            })?;
        if provider != provider_instance {
            return Err(ExecutionError::ProviderAssignmentMismatch {
                provider,
                provider_instance,
            });
        }
        Ok(Self {
            provider,
            provider_instance,
        })
    }
}

fn execution_attempt(request: &ExecutionRequest, status: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "attempt_id": "attempt-0001",
        "job_id": request.job_id(),
        "stage": request.stage(),
        "status": status
    })
}

fn verify_provider_result(
    request: &ExecutionRequest,
    provider_execution: &ProviderExecution,
) -> Result<(), ExecutionError> {
    let result = provider_execution.result();
    compare_output("job_id", request.job_id(), result.job_id())?;
    compare_output("stage", request.stage(), result.stage())?;
    compare_output(
        "provider_instance_id",
        request.provider_instance_id(),
        result.provider_instance_id(),
    )
}

fn compare_output(field: &str, expected: &str, actual: &str) -> Result<(), ExecutionError> {
    if expected == actual {
        Ok(())
    } else {
        Err(ExecutionError::ProviderOutputMismatch {
            field: field.to_string(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    }
}

fn state_for_provider_status(stage: &str, status: &str) -> &'static str {
    match status {
        "success" => completed_state_for_stage(stage),
        "blocked" => "BLOCKED",
        "cancelled" => "CANCELLED",
        "failed" | "timeout" | "error" => "FAILED",
        _ => "FAILED",
    }
}

fn completed_state_for_stage(stage: &str) -> &'static str {
    match stage {
        "route" => "ROUTED",
        "plan" => "PLANNED",
        "design" => "PLANNED",
        "implement" => "IMPLEMENTED",
        "validate" => "VALIDATED",
        "review" => "REVIEWED",
        "polish" => "POLISHED",
        "report" => "DONE",
        _ => "DONE",
    }
}

fn initial_state(job_id: &str, stage: &str, created_at: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "job_id": job_id,
        "state": "REQUESTED",
        "current_stage": stage,
        "updated_at": created_at,
        "threads": {},
        "workers": {},
        "artifacts": {},
        "latest_event_id": "",
        "active_provider": null,
        "next_action": "continue",
        "budget": {},
        "history": []
    })
}

fn set_object_field(
    value: &mut Value,
    key: &str,
    field_value: Value,
) -> Result<(), ExecutionError> {
    let Some(object) = value.as_object_mut() else {
        return Err(ExecutionError::InvalidFieldType {
            path: PathBuf::from("run-state.json"),
            field: "$".to_string(),
            expected: "object".to_string(),
        });
    };
    object.insert(key.to_string(), field_value);
    Ok(())
}

fn push_history(value: &mut Value, entry: Value) -> Result<(), ExecutionError> {
    let Some(object) = value.as_object_mut() else {
        return Err(ExecutionError::InvalidFieldType {
            path: PathBuf::from("run-state.json"),
            field: "$".to_string(),
            expected: "object".to_string(),
        });
    };
    let history = object
        .entry("history")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(history) = history.as_array_mut() else {
        return Err(ExecutionError::InvalidFieldType {
            path: PathBuf::from("run-state.json"),
            field: "history".to_string(),
            expected: "array".to_string(),
        });
    };
    history.push(entry);
    Ok(())
}

fn validate_contract(
    value: &Value,
    path: &Path,
    schema_root: &Path,
    schema_file: &str,
) -> Result<(), ExecutionError> {
    let schema_path = schema_root.join(schema_file);
    let schema = load_schema(&schema_path).map_err(|source| ExecutionError::SchemaLoadFailed {
        path: schema_path.clone(),
        message: source.to_string(),
    })?;
    let result = validate_json(value, &schema);
    if result.is_ok() {
        Ok(())
    } else {
        Err(ExecutionError::SchemaValidationFailed {
            path: path.to_path_buf(),
            schema_path,
            errors: result.errors,
        })
    }
}

fn required_string(value: &Value, path: &Path, field: &str) -> Result<String, ExecutionError> {
    value
        .get(field)
        .ok_or_else(|| ExecutionError::MissingField {
            path: path.to_path_buf(),
            field: field.to_string(),
        })?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| ExecutionError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field.to_string(),
            expected: "string".to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_control_provider::{FakeProviderAdapter, ProviderRegistryLoader};
    use star_control_router::{JobSpec, RouterEngine};
    use std::fs;
    use std::sync::{Mutex, MutexGuard};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn fake_provider_workspec_execution_writes_artifacts_and_state() {
        let fixture = Fixture::new();
        let outcome = fixture
            .engine(FakeProviderAdapter::success())
            .execute_stage("J-0001", "implement")
            .expect("execute stage");

        assert_eq!(outcome.request().provider_instance_id(), "fake-default");
        assert_eq!(outcome.provider_execution().result().status(), "success");
        assert_eq!(outcome.attempt()["status"], "success");
        assert_eq!(outcome.state()["state"], "IMPLEMENTED");
        assert!(fixture
            .project
            .join(".ai-runs/J-0001/provider-output/fake-default/request.json")
            .is_file());
        assert!(fixture
            .project
            .join(".ai-runs/J-0001/provider-output/fake-default/response.json")
            .is_file());

        let events = fixture.store.read_events("J-0001").expect("events");
        assert!(events
            .iter()
            .any(|event| event["type"] == "PROVIDER_STARTED"));
        assert!(events
            .iter()
            .any(|event| event["type"] == "PROVIDER_FINISHED"));
    }

    #[test]
    fn execution_refuses_to_overwrite_existing_provider_output() {
        let fixture = Fixture::new();
        let engine = fixture.engine(FakeProviderAdapter::success());
        engine
            .execute_stage("J-0001", "implement")
            .expect("first execute");
        let error = engine
            .execute_stage("J-0001", "implement")
            .expect_err("second execute should fail");

        assert!(matches!(error, ExecutionError::StageAlreadyExecuted { .. }));
    }

    #[test]
    fn failed_and_blocked_provider_results_update_state() {
        let failed = Fixture::new();
        let failed_outcome = failed
            .engine(FakeProviderAdapter::failed("unit failure"))
            .execute_stage("J-0001", "implement")
            .expect("failed execution");
        assert_eq!(
            failed_outcome.provider_execution().result().status(),
            "failed"
        );
        assert_eq!(failed_outcome.state()["state"], "FAILED");

        let blocked = Fixture::new();
        let blocked_outcome = blocked
            .engine(FakeProviderAdapter::blocked("approval required"))
            .execute_stage("J-0001", "implement")
            .expect("blocked execution");
        assert_eq!(
            blocked_outcome.provider_execution().result().status(),
            "blocked"
        );
        assert_eq!(blocked_outcome.state()["state"], "BLOCKED");
    }

    #[test]
    fn local_process_provider_executes_by_manifest_kind() {
        let mut fixture = Fixture::new();
        fixture.use_local_process_registry(vec!["--help".to_string()], Vec::new(), 10);
        fixture.assign_implement_stage_to_local_process();

        let outcome = ExecutionEngine::new(&fixture.store, &fixture.registry, &fixture.schemas)
            .execute_stage("J-0001", "implement")
            .expect("execute local process stage");

        assert_eq!(outcome.request().provider_instance_id(), "local-default");
        assert_eq!(outcome.provider_execution().result().status(), "success");
        assert_eq!(outcome.state()["state"], "IMPLEMENTED");
        assert_eq!(
            outcome.state()["artifacts"]["implement_provider_stdout"]["path"],
            "provider-output/local-default/stdout.txt"
        );
        assert!(fixture
            .project
            .join(".ai-runs/J-0001/provider-output/local-default/request.json")
            .is_file());
        assert!(fixture
            .project
            .join(".ai-runs/J-0001/provider-output/local-default/stdout.txt")
            .is_file());
        assert!(fixture
            .project
            .join(".ai-runs/J-0001/provider-output/local-default/stderr.txt")
            .is_file());
        assert!(fixture
            .project
            .join(".ai-runs/J-0001/provider-output/local-default/response.json")
            .is_file());
    }

    #[test]
    fn local_process_timeout_updates_run_state_to_failed() {
        let mut fixture = Fixture::new();
        let _env = EnvVarGuard::set("STAR_CONTROL_EXECUTION_SLEEP_HELPER", "1");
        fixture.use_local_process_registry(
            vec![
                "--exact".to_string(),
                "tests::execution_sleep_helper".to_string(),
                "--nocapture".to_string(),
            ],
            vec!["STAR_CONTROL_EXECUTION_SLEEP_HELPER".to_string()],
            1,
        );
        fixture.assign_implement_stage_to_local_process();

        let outcome = ExecutionEngine::new(&fixture.store, &fixture.registry, &fixture.schemas)
            .execute_stage("J-0001", "implement")
            .expect("execute timeout stage");

        assert_eq!(outcome.provider_execution().result().status(), "timeout");
        assert_eq!(outcome.state()["state"], "FAILED");
    }

    #[test]
    fn local_process_cancelled_updates_run_state_to_cancelled() {
        let mut fixture = Fixture::new();
        let _env = EnvVarGuard::set("STAR_CONTROL_EXECUTION_SLEEP_HELPER", "1");
        fixture.use_local_process_registry(
            vec![
                "--exact".to_string(),
                "tests::execution_sleep_helper".to_string(),
                "--nocapture".to_string(),
            ],
            vec!["STAR_CONTROL_EXECUTION_SLEEP_HELPER".to_string()],
            10,
        );
        fixture.assign_implement_stage_to_local_process();

        let cancel_project = fixture.project.clone();
        let cancel_schemas = fixture.schemas.clone();
        let cancel_thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            let store =
                StateStore::open(cancel_project, cancel_schemas).expect("open cancel store");
            let mut state = store.load_state("J-0001").expect("load state");
            state["state"] = json!("CANCELLED");
            state["next_action"] = json!("stop");
            store
                .save_state("J-0001", &state)
                .expect("save cancelled state");
        });

        let outcome = ExecutionEngine::new(&fixture.store, &fixture.registry, &fixture.schemas)
            .execute_stage("J-0001", "implement")
            .expect("execute cancelled stage");
        cancel_thread.join().expect("cancel thread");

        assert_eq!(outcome.provider_execution().result().status(), "cancelled");
        assert_eq!(outcome.state()["state"], "CANCELLED");
        assert_eq!(outcome.state()["next_action"], "stop");
    }

    #[test]
    fn local_process_forbidden_action_evidence_updates_run_state_to_blocked() {
        let mut fixture = Fixture::new();
        let _env = EnvVarGuard::set("STAR_CONTROL_EXECUTION_FORBIDDEN_EVIDENCE_HELPER", "1");
        fixture.use_local_process_registry(
            vec![
                "--exact".to_string(),
                "tests::execution_forbidden_evidence_helper".to_string(),
                "--nocapture".to_string(),
            ],
            vec!["STAR_CONTROL_EXECUTION_FORBIDDEN_EVIDENCE_HELPER".to_string()],
            10,
        );
        fixture.assign_implement_stage_to_local_process();

        let outcome = ExecutionEngine::new(&fixture.store, &fixture.registry, &fixture.schemas)
            .execute_stage("J-0001", "implement")
            .expect("execute forbidden evidence stage");

        assert_eq!(outcome.provider_execution().result().status(), "blocked");
        assert_eq!(outcome.state()["state"], "BLOCKED");
        assert_eq!(
            outcome.provider_execution().result().value()["error"]["kind"],
            "local_process_forbidden_action"
        );
        assert_eq!(
            outcome.provider_execution().result().value()["error"]["action"],
            "dependency_install"
        );
    }

    #[test]
    fn cloud_cli_transport_records_handoff_and_updates_state() {
        let mut fixture = Fixture::new();
        let _env = EnvVarGuard::set("STAR_CONTROL_EXECUTION_CLOUD_CLI_HELPER", "1");
        fixture.use_cloud_cli_registry(
            vec![
                "--exact".to_string(),
                "tests::execution_cloud_cli_success_helper".to_string(),
                "--nocapture".to_string(),
            ],
            vec!["STAR_CONTROL_EXECUTION_CLOUD_CLI_HELPER".to_string()],
            10,
        );
        fixture.assign_implement_stage_to_cloud_provider();

        let outcome = ExecutionEngine::new(&fixture.store, &fixture.registry, &fixture.schemas)
            .execute_stage("J-0001", "implement")
            .expect("execute cloud CLI stage");

        assert_eq!(outcome.request().provider_instance_id(), "cloud-default");
        assert_eq!(outcome.provider_execution().result().status(), "success");
        assert_eq!(outcome.state()["state"], "IMPLEMENTED");
        assert_eq!(
            outcome.provider_execution().result().value()["error"],
            Value::Null
        );
        assert_eq!(
            outcome.state()["artifacts"]["implement_provider_request"]["path"],
            "provider-output/cloud-default/request.json"
        );
        assert_eq!(
            outcome.state()["artifacts"]["implement_provider_response"]["path"],
            "provider-output/cloud-default/response.json"
        );
        assert_eq!(
            outcome.state()["artifacts"]["implement_provider_stdout"]["path"],
            "provider-output/cloud-default/stdout.txt"
        );
        assert_eq!(
            outcome.state()["artifacts"]["implement_provider_stderr"]["path"],
            "provider-output/cloud-default/stderr.txt"
        );
        assert!(fixture
            .project
            .join(".ai-runs/J-0001/provider-output/cloud-default/privacy-handoff.json")
            .is_file());
        assert!(fixture
            .project
            .join(".ai-runs/J-0001/provider-output/cloud-default/cost-metric.json")
            .is_file());

        let result = outcome.provider_execution().result().value();
        assert!(result["artifacts"]
            .as_array()
            .expect("artifacts")
            .iter()
            .any(|path| path == "provider-output/cloud-default/privacy-handoff.json"));
        let events = fixture.store.read_events("J-0001").expect("events");
        assert!(events.iter().any(|event| {
            event["type"] == "PROVIDER_FINISHED" && event["details"]["status"] == "success"
        }));
    }

    #[test]
    fn local_process_provider_conformance_fixture_covers_m5_runtime_contract() {
        let cases = vec![
            LocalProcessConformanceCase {
                id: "allowed_command_success",
                args: vec!["--help".to_string()],
                env_name: None,
                timeout_seconds: 10,
                cancel_after: None,
                expected_status: "success",
                expected_state: "IMPLEMENTED",
                expected_error_kind: None,
                expected_error_action: None,
            },
            LocalProcessConformanceCase {
                id: "timeout_failed_state",
                args: local_process_sleep_args(),
                env_name: Some("STAR_CONTROL_EXECUTION_SLEEP_HELPER"),
                timeout_seconds: 1,
                cancel_after: None,
                expected_status: "timeout",
                expected_state: "FAILED",
                expected_error_kind: Some("local_process_timeout"),
                expected_error_action: None,
            },
            LocalProcessConformanceCase {
                id: "cancelled_state",
                args: local_process_sleep_args(),
                env_name: Some("STAR_CONTROL_EXECUTION_SLEEP_HELPER"),
                timeout_seconds: 10,
                cancel_after: Some(Duration::from_millis(150)),
                expected_status: "cancelled",
                expected_state: "CANCELLED",
                expected_error_kind: Some("local_process_cancelled"),
                expected_error_action: None,
            },
            LocalProcessConformanceCase {
                id: "forbidden_action_blocked_state",
                args: local_process_forbidden_evidence_args(),
                env_name: Some("STAR_CONTROL_EXECUTION_FORBIDDEN_EVIDENCE_HELPER"),
                timeout_seconds: 10,
                cancel_after: None,
                expected_status: "blocked",
                expected_state: "BLOCKED",
                expected_error_kind: Some("local_process_forbidden_action"),
                expected_error_action: Some("dependency_install"),
            },
        ];

        for case in cases {
            run_local_process_conformance_case(case);
        }
    }

    #[test]
    fn execution_sleep_helper() {
        let is_child_helper = std::env::args()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|args| args[0] == "--exact" && args[1] == "tests::execution_sleep_helper");
        if is_child_helper && std::env::var("STAR_CONTROL_EXECUTION_SLEEP_HELPER").is_ok() {
            std::thread::sleep(std::time::Duration::from_secs(5));
        }
    }

    #[test]
    fn execution_forbidden_evidence_helper() {
        let is_child_helper = std::env::args().collect::<Vec<_>>().windows(2).any(|args| {
            args[0] == "--exact" && args[1] == "tests::execution_forbidden_evidence_helper"
        });
        if is_child_helper
            && std::env::var("STAR_CONTROL_EXECUTION_FORBIDDEN_EVIDENCE_HELPER").is_ok()
        {
            println!(
                "STAR_CONTROL_FORBIDDEN_ACTION_EVIDENCE:dependency_install from execution helper"
            );
        }
    }

    #[test]
    fn execution_cloud_cli_success_helper() {
        let is_child_helper = std::env::args().collect::<Vec<_>>().windows(2).any(|args| {
            args[0] == "--exact" && args[1] == "tests::execution_cloud_cli_success_helper"
        });
        if is_child_helper && std::env::var("STAR_CONTROL_EXECUTION_CLOUD_CLI_HELPER").is_ok() {
            println!("cloud cli execution helper completed");
        }
    }

    #[test]
    fn unknown_provider_instance_fails_before_writing_output() {
        let fixture = Fixture::new();
        let mut workspec = fixture
            .store
            .load_workspec("J-0001", "implement")
            .expect("workspec");
        workspec["provider"] = json!("missing-provider");
        workspec["provider_instance"] = json!("missing-provider");
        fixture
            .store
            .save_workspec("J-0001", "implement", &workspec)
            .expect("save unknown provider workspec");

        let error = fixture
            .engine(FakeProviderAdapter::success())
            .execute_stage("J-0001", "implement")
            .expect_err("unknown provider should fail");

        assert!(matches!(
            error,
            ExecutionError::ProviderRegistry(ProviderRegistryError::InstanceNotFound { .. })
        ));
        assert!(!fixture
            .project
            .join(".ai-runs/J-0001/provider-output/missing-provider/response.json")
            .exists());
    }

    struct LocalProcessConformanceCase {
        id: &'static str,
        args: Vec<String>,
        env_name: Option<&'static str>,
        timeout_seconds: u64,
        cancel_after: Option<Duration>,
        expected_status: &'static str,
        expected_state: &'static str,
        expected_error_kind: Option<&'static str>,
        expected_error_action: Option<&'static str>,
    }

    fn run_local_process_conformance_case(case: LocalProcessConformanceCase) {
        let mut fixture = Fixture::new();
        let _env = case.env_name.map(|name| EnvVarGuard::set(name, "1"));
        fixture.use_local_process_registry(
            case.args.clone(),
            case.env_name
                .map(|name| vec![name.to_string()])
                .unwrap_or_default(),
            case.timeout_seconds,
        );
        fixture.assign_implement_stage_to_local_process();

        let cancel_thread = case.cancel_after.map(|delay| {
            let cancel_project = fixture.project.clone();
            let cancel_schemas = fixture.schemas.clone();
            std::thread::spawn(move || {
                std::thread::sleep(delay);
                let store =
                    StateStore::open(cancel_project, cancel_schemas).expect("open cancel store");
                let mut state = store.load_state("J-0001").expect("load state");
                state["state"] = json!("CANCELLED");
                state["next_action"] = json!("stop");
                store
                    .save_state("J-0001", &state)
                    .expect("save cancelled state");
            })
        });

        let outcome = ExecutionEngine::new(&fixture.store, &fixture.registry, &fixture.schemas)
            .execute_stage("J-0001", "implement")
            .unwrap_or_else(|error| panic!("{} execute local process: {}", case.id, error));
        if let Some(cancel_thread) = cancel_thread {
            cancel_thread.join().expect("cancel thread");
        }

        assert_eq!(
            outcome.request().provider_instance_id(),
            "local-default",
            "{} provider instance",
            case.id
        );
        assert_eq!(
            outcome.provider_execution().result().status(),
            case.expected_status,
            "{} provider status",
            case.id
        );
        assert_eq!(
            outcome.attempt()["status"],
            case.expected_status,
            "{} execution attempt status",
            case.id
        );
        assert_eq!(
            outcome.state()["state"],
            case.expected_state,
            "{} run state",
            case.id
        );
        assert_local_process_output_contract(&fixture, &outcome, &case);
    }

    fn assert_local_process_output_contract(
        fixture: &Fixture,
        outcome: &ExecutionOutcome,
        case: &LocalProcessConformanceCase,
    ) {
        let expected_paths = [
            "provider-output/local-default/response.json",
            "provider-output/local-default/stdout.txt",
            "provider-output/local-default/stderr.txt",
        ];
        assert!(
            fixture
                .project
                .join(".ai-runs/J-0001/provider-output/local-default/request.json")
                .is_file(),
            "{} missing request artifact",
            case.id
        );
        for relative_path in expected_paths {
            assert!(
                fixture
                    .project
                    .join(".ai-runs/J-0001")
                    .join(relative_path)
                    .is_file(),
                "{} missing artifact {}",
                case.id,
                relative_path
            );
        }

        let result = outcome.provider_execution().result().value();
        let artifacts = result["artifacts"]
            .as_array()
            .expect("provider result artifacts");
        assert!(
            artifacts.iter().all(|path| path
                .as_str()
                .map(|path| path.starts_with("provider-output/local-default/"))
                .unwrap_or(false)),
            "{} artifacts stay inside provider output directory",
            case.id
        );
        assert_eq!(
            result["artifacts"],
            json!(expected_paths),
            "{} provider result artifacts",
            case.id
        );
        assert_eq!(
            outcome.state()["artifacts"]["implement_provider_request"]["path"],
            "provider-output/local-default/request.json",
            "{} request artifact ref",
            case.id
        );
        assert_eq!(
            outcome.state()["artifacts"]["implement_provider_response"]["path"],
            "provider-output/local-default/response.json",
            "{} response artifact ref",
            case.id
        );
        assert_eq!(
            outcome.state()["artifacts"]["implement_provider_stdout"]["path"],
            "provider-output/local-default/stdout.txt",
            "{} stdout artifact ref",
            case.id
        );
        assert_eq!(
            outcome.state()["artifacts"]["implement_provider_stderr"]["path"],
            "provider-output/local-default/stderr.txt",
            "{} stderr artifact ref",
            case.id
        );

        match case.expected_error_kind {
            Some(kind) => assert_eq!(
                result["error"]["kind"], kind,
                "{} provider error kind",
                case.id
            ),
            None => assert_eq!(result["error"], Value::Null, "{} provider error", case.id),
        }
        if let Some(action) = case.expected_error_action {
            assert_eq!(
                result["error"]["action"], action,
                "{} provider error action",
                case.id
            );
        }

        let events = fixture.store.read_events("J-0001").expect("events");
        assert!(
            events.iter().any(|event| {
                event["type"] == "PROVIDER_FINISHED"
                    && event["details"]["status"] == case.expected_status
            }),
            "{} provider finished event",
            case.id
        );
    }

    fn local_process_sleep_args() -> Vec<String> {
        vec![
            "--exact".to_string(),
            "tests::execution_sleep_helper".to_string(),
            "--nocapture".to_string(),
        ]
    }

    fn local_process_forbidden_evidence_args() -> Vec<String> {
        vec![
            "--exact".to_string(),
            "tests::execution_forbidden_evidence_helper".to_string(),
            "--nocapture".to_string(),
        ]
    }

    struct Fixture {
        project: PathBuf,
        store: StateStore,
        registry: ProviderRegistry,
        schemas: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let project = temp_project();
            let schemas = schema_root();
            let store = StateStore::open(&project, &schemas).expect("open store");
            let job = store
                .create_job("runtime code 구현", "codex", vec![])
                .expect("create job");
            let registry = ProviderRegistryLoader::new(repo_root())
                .load_fake_default_registry()
                .expect("load registry");
            let router = RouterEngine::new(&registry, &schemas);
            let job_spec =
                JobSpec::from_value(job.clone(), "job.json", &schemas).expect("job spec");
            let output = router.route(&job_spec).expect("route");
            store
                .save_route("J-0001", output.route().value())
                .expect("save route");
            for (stage, workspec) in output.workspecs() {
                store
                    .save_workspec("J-0001", stage, workspec.value())
                    .expect("save workspec");
            }
            store
                .save_state(
                    "J-0001",
                    &initial_state(
                        "J-0001",
                        "implement",
                        job["created_at"].as_str().unwrap_or("created"),
                    ),
                )
                .expect("save state");
            Self {
                project,
                store,
                registry,
                schemas,
            }
        }

        fn engine(&self, adapter: FakeProviderAdapter) -> ExecutionEngine<'_> {
            ExecutionEngine::new(&self.store, &self.registry, &self.schemas)
                .with_fake_adapter(adapter)
        }

        fn use_local_process_registry(
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

        fn assign_implement_stage_to_local_process(&self) {
            let mut workspec = self
                .store
                .load_workspec("J-0001", "implement")
                .expect("load workspec");
            workspec["provider"] = json!("local-default");
            workspec["provider_instance"] = json!("local-default");
            workspec["required_outputs"] = json!(["provider-output/local-default/response.json"]);
            self.store
                .save_workspec("J-0001", "implement", &workspec)
                .expect("save local process workspec");
        }

        fn use_cloud_cli_registry(
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

        fn assign_implement_stage_to_cloud_provider(&self) {
            let mut workspec = self
                .store
                .load_workspec("J-0001", "implement")
                .expect("load workspec");
            workspec["provider"] = json!("cloud-default");
            workspec["provider_instance"] = json!("cloud-default");
            workspec["required_outputs"] = json!(["provider-output/cloud-default/response.json"]);
            self.store
                .save_workspec("J-0001", "implement", &workspec)
                .expect("save cloud workspec");
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.project).ok();
        }
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

    fn temp_project() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "star-control-execution-{}-{}",
            std::process::id(),
            nanos
        ));
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

    fn schema_root() -> PathBuf {
        repo_root().join("specs").join("schemas")
    }

    fn current_test_executable() -> String {
        std::env::current_exe()
            .expect("current test executable")
            .display()
            .to_string()
    }
}
