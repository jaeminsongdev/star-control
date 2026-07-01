use crate::{ProviderRegistry, ProviderRegistryError};
use serde_json::{json, Value};
use star_control_schema::{load_schema, validate_json, ValidationError};
use star_control_state::{StateStore, StateStoreError};
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

const EXECUTION_REQUEST_SCHEMA: &str = "execution-request.schema.json";
const PROVIDER_RUN_RESULT_SCHEMA: &str = "provider-run-result.schema.json";
const FAKE_PROVIDER_ID: &str = "provider.fake";

#[derive(Debug)]
pub enum ProviderAdapterError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidJson {
        path: PathBuf,
        source: serde_json::Error,
    },
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
    Registry(ProviderRegistryError),
    State(StateStoreError),
    UnsupportedProvider {
        provider_instance_id: String,
        provider_id: String,
    },
    ProviderOutputAlreadyExists {
        path: PathBuf,
    },
    CommandPolicyDenied {
        provider_instance_id: String,
        reason: String,
    },
}

impl fmt::Display for ProviderAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "failed to read {}: {}", path.display(), source)
            }
            Self::InvalidJson { path, source } => {
                write!(
                    formatter,
                    "failed to parse JSON {}: {}",
                    path.display(),
                    source
                )
            }
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
            Self::Registry(source) => write!(formatter, "provider registry error: {}", source),
            Self::State(source) => write!(formatter, "state store error: {}", source),
            Self::UnsupportedProvider {
                provider_instance_id,
                provider_id,
            } => write!(
                formatter,
                "provider instance {} resolves to unsupported provider adapter provider {}",
                provider_instance_id, provider_id
            ),
            Self::ProviderOutputAlreadyExists { path } => {
                write!(
                    formatter,
                    "provider output already exists: {}",
                    path.display()
                )
            }
            Self::CommandPolicyDenied {
                provider_instance_id,
                reason,
            } => write!(
                formatter,
                "provider instance {} command policy denied: {}",
                provider_instance_id, reason
            ),
        }
    }
}

impl Error for ProviderAdapterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::InvalidJson { source, .. } => Some(source),
            Self::Registry(source) => Some(source),
            Self::State(source) => Some(source),
            _ => None,
        }
    }
}

impl From<ProviderRegistryError> for ProviderAdapterError {
    fn from(source: ProviderRegistryError) -> Self {
        Self::Registry(source)
    }
}

impl From<StateStoreError> for ProviderAdapterError {
    fn from(source: StateStoreError) -> Self {
        Self::State(source)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionRequest {
    request_id: String,
    job_id: String,
    stage: String,
    provider_instance_id: String,
    workspec_path: String,
    created_at: String,
    goal: String,
    value: Value,
}

impl ExecutionRequest {
    pub fn from_value(
        value: Value,
        source_path: impl Into<PathBuf>,
        schema_root: impl AsRef<Path>,
    ) -> Result<Self, ProviderAdapterError> {
        let source_path = source_path.into();
        validate_contract(
            &value,
            &source_path,
            schema_root.as_ref(),
            EXECUTION_REQUEST_SCHEMA,
        )?;

        Ok(Self {
            request_id: required_string(&value, &source_path, "request_id")?,
            job_id: required_string(&value, &source_path, "job_id")?,
            stage: required_string(&value, &source_path, "stage")?,
            provider_instance_id: required_string(&value, &source_path, "provider_instance_id")?,
            workspec_path: required_string(&value, &source_path, "workspec_path")?,
            created_at: required_string(&value, &source_path, "created_at")?,
            goal: required_string(&value, &source_path, "goal")?,
            value,
        })
    }

    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    pub fn stage(&self) -> &str {
        &self.stage
    }

    pub fn provider_instance_id(&self) -> &str {
        &self.provider_instance_id
    }

    pub fn workspec_path(&self) -> &str {
        &self.workspec_path
    }

    pub fn created_at(&self) -> &str {
        &self.created_at
    }

    pub fn goal(&self) -> &str {
        &self.goal
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}

pub fn load_execution_request(
    path: impl AsRef<Path>,
    schema_root: impl AsRef<Path>,
) -> Result<ExecutionRequest, ProviderAdapterError> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|source| ProviderAdapterError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let value: Value =
        serde_json::from_str(&content).map_err(|source| ProviderAdapterError::InvalidJson {
            path: path.to_path_buf(),
            source,
        })?;
    ExecutionRequest::from_value(value, path.to_path_buf(), schema_root)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderRunResult {
    provider_instance_id: String,
    job_id: String,
    stage: String,
    status: String,
    value: Value,
}

impl ProviderRunResult {
    pub fn from_value(
        value: Value,
        source_path: impl Into<PathBuf>,
        schema_root: impl AsRef<Path>,
    ) -> Result<Self, ProviderAdapterError> {
        let source_path = source_path.into();
        validate_contract(
            &value,
            &source_path,
            schema_root.as_ref(),
            PROVIDER_RUN_RESULT_SCHEMA,
        )?;

        Ok(Self {
            provider_instance_id: required_string(&value, &source_path, "provider_instance_id")?,
            job_id: required_string(&value, &source_path, "job_id")?,
            stage: required_string(&value, &source_path, "stage")?,
            status: required_string(&value, &source_path, "status")?,
            value,
        })
    }

    pub fn provider_instance_id(&self) -> &str {
        &self.provider_instance_id
    }

    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    pub fn stage(&self) -> &str {
        &self.stage
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FakeProviderSimulation {
    Success,
    Failed(String),
    Blocked(String),
}

impl FakeProviderSimulation {
    fn status(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failed(_) => "failed",
            Self::Blocked(_) => "blocked",
        }
    }

    fn summary(&self) -> String {
        match self {
            Self::Success => "fake provider completed".to_string(),
            Self::Failed(message) => format!("fake provider failed: {}", message),
            Self::Blocked(reason) => format!("fake provider blocked: {}", reason),
        }
    }

    fn stdout(&self) -> String {
        match self {
            Self::Success => "fake provider completed\n".to_string(),
            Self::Failed(message) => format!("fake provider failed: {}\n", message),
            Self::Blocked(reason) => format!("fake provider blocked: {}\n", reason),
        }
    }

    fn stderr(&self) -> Option<String> {
        match self {
            Self::Success => None,
            Self::Failed(message) => Some(format!("fake failure: {}\n", message)),
            Self::Blocked(reason) => Some(format!("fake blocked: {}\n", reason)),
        }
    }

    fn error(&self) -> Value {
        match self {
            Self::Success => Value::Null,
            Self::Failed(message) => json!({
                "kind": "fake_failed",
                "message": message,
            }),
            Self::Blocked(reason) => json!({
                "kind": "fake_blocked",
                "message": reason,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeProviderAdapter {
    simulation: FakeProviderSimulation,
}

impl FakeProviderAdapter {
    pub fn success() -> Self {
        Self {
            simulation: FakeProviderSimulation::Success,
        }
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            simulation: FakeProviderSimulation::Failed(message.into()),
        }
    }

    pub fn blocked(reason: impl Into<String>) -> Self {
        Self {
            simulation: FakeProviderSimulation::Blocked(reason.into()),
        }
    }

    pub fn simulation(&self) -> &FakeProviderSimulation {
        &self.simulation
    }
}

impl Default for FakeProviderAdapter {
    fn default() -> Self {
        Self::success()
    }
}

pub trait ProviderAdapter {
    fn execute(
        &self,
        request: &ExecutionRequest,
        context: &ProviderRunContext<'_>,
    ) -> Result<ProviderExecution, ProviderAdapterError>;
}

#[derive(Debug, Clone, Copy)]
pub struct ProviderRunContext<'a> {
    registry: &'a ProviderRegistry,
    state_store: &'a StateStore,
    schema_root: &'a Path,
}

impl<'a> ProviderRunContext<'a> {
    pub fn new(
        registry: &'a ProviderRegistry,
        state_store: &'a StateStore,
        schema_root: &'a Path,
    ) -> Self {
        Self {
            registry,
            state_store,
            schema_root,
        }
    }

    pub fn registry(&self) -> &ProviderRegistry {
        self.registry
    }

    pub fn state_store(&self) -> &StateStore {
        self.state_store
    }

    pub fn schema_root(&self) -> &Path {
        self.schema_root
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderExecution {
    result: ProviderRunResult,
    request_ref: Value,
    response_ref: Value,
    stdout_ref: Value,
    stderr_ref: Option<Value>,
}

impl ProviderExecution {
    pub(crate) fn new(
        result: ProviderRunResult,
        request_ref: Value,
        response_ref: Value,
        stdout_ref: Value,
        stderr_ref: Option<Value>,
    ) -> Self {
        Self {
            result,
            request_ref,
            response_ref,
            stdout_ref,
            stderr_ref,
        }
    }

    pub fn result(&self) -> &ProviderRunResult {
        &self.result
    }

    pub fn request_ref(&self) -> &Value {
        &self.request_ref
    }

    pub fn response_ref(&self) -> &Value {
        &self.response_ref
    }

    pub fn stdout_ref(&self) -> &Value {
        &self.stdout_ref
    }

    pub fn stderr_ref(&self) -> Option<&Value> {
        self.stderr_ref.as_ref()
    }
}

impl ProviderAdapter for FakeProviderAdapter {
    fn execute(
        &self,
        request: &ExecutionRequest,
        context: &ProviderRunContext<'_>,
    ) -> Result<ProviderExecution, ProviderAdapterError> {
        let manifest = context
            .registry()
            .manifest_for_instance(request.provider_instance_id())?;
        if manifest.id() != FAKE_PROVIDER_ID {
            return Err(ProviderAdapterError::UnsupportedProvider {
                provider_instance_id: request.provider_instance_id().to_string(),
                provider_id: manifest.id().to_string(),
            });
        }

        let output_files = planned_output_files(
            request.provider_instance_id(),
            self.simulation.stderr().is_some(),
        );
        ensure_output_files_absent(context.state_store(), request.job_id(), &output_files)?;

        let response_value = self.response_value(request);
        let result = ProviderRunResult::from_value(
            response_value.clone(),
            format!(
                "provider-output/{}/response.json",
                request.provider_instance_id()
            ),
            context.schema_root(),
        )?;

        let request_ref = context.state_store().write_provider_json(
            request.job_id(),
            request.provider_instance_id(),
            "request.json",
            request.value(),
        )?;
        let stdout_ref = context.state_store().write_provider_text(
            request.job_id(),
            request.provider_instance_id(),
            "stdout.txt",
            &self.simulation.stdout(),
        )?;
        let stderr_content = self.simulation.stderr();
        let stderr_ref = if let Some(stderr) = stderr_content {
            Some(context.state_store().write_provider_text(
                request.job_id(),
                request.provider_instance_id(),
                "stderr.txt",
                &stderr,
            )?)
        } else {
            None
        };
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
            stderr_ref,
        ))
    }
}

impl FakeProviderAdapter {
    fn response_value(&self, request: &ExecutionRequest) -> Value {
        let stderr_path = if self.simulation.stderr().is_some() {
            Value::String(provider_output_path(
                request.provider_instance_id(),
                "stderr.txt",
            ))
        } else {
            Value::Null
        };

        json!({
            "schema_version": "1.0.0",
            "provider_instance_id": request.provider_instance_id(),
            "job_id": request.job_id(),
            "stage": request.stage(),
            "status": self.simulation.status(),
            "started_at": request.created_at(),
            "finished_at": request.created_at(),
            "stdout_path": provider_output_path(request.provider_instance_id(), "stdout.txt"),
            "stderr_path": stderr_path,
            "summary": self.simulation.summary(),
            "changed_files": [],
            "artifacts": [
                provider_output_path(request.provider_instance_id(), "response.json")
            ],
            "metrics": {
                "estimated_cost": 0,
                "input_tokens": 0,
                "output_tokens": 0
            },
            "error": self.simulation.error()
        })
    }
}

fn planned_output_files(provider_instance_id: &str, include_stderr: bool) -> Vec<String> {
    let mut files = vec![
        provider_output_path(provider_instance_id, "request.json"),
        provider_output_path(provider_instance_id, "stdout.txt"),
        provider_output_path(provider_instance_id, "response.json"),
    ];
    if include_stderr {
        files.push(provider_output_path(provider_instance_id, "stderr.txt"));
    }
    files
}

pub(crate) fn provider_output_path(provider_instance_id: &str, file_name: &str) -> String {
    format!("provider-output/{}/{}", provider_instance_id, file_name)
}

pub(crate) fn ensure_output_files_absent(
    state_store: &StateStore,
    job_id: &str,
    relative_paths: &[String],
) -> Result<(), ProviderAdapterError> {
    for relative_path in relative_paths {
        let path = state_store.resolve_job_path(job_id, relative_path)?;
        if path.exists() {
            return Err(ProviderAdapterError::ProviderOutputAlreadyExists { path });
        }
    }
    Ok(())
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

fn required_string(
    value: &Value,
    path: &Path,
    field: &str,
) -> Result<String, ProviderAdapterError> {
    value
        .get(field)
        .ok_or_else(|| ProviderAdapterError::MissingField {
            path: path.to_path_buf(),
            field: field.to_string(),
        })?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| ProviderAdapterError::InvalidFieldType {
            path: path.to_path_buf(),
            field: field.to_string(),
            expected: "string".to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProviderRegistryLoader;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn loads_execution_request_example() {
        let request = load_execution_request(
            repo_root().join("examples/execution-contracts/execution-request.fake.example.json"),
            schema_root(),
        )
        .expect("load request example");

        assert_eq!(request.request_id(), "request-0001");
        assert_eq!(request.job_id(), "J-0001");
        assert_eq!(request.provider_instance_id(), "fake-default");
    }

    #[test]
    fn fake_provider_writes_deterministic_success_output() {
        let project = temp_project();
        let store = open_store(&project);
        store
            .create_job("implement feature", "codex", vec![])
            .expect("create job");
        let registry = ProviderRegistryLoader::new(repo_root())
            .load_fake_default_registry()
            .expect("load fake registry");
        let request = request_value("success goal");
        let request =
            ExecutionRequest::from_value(request, "request.json", schema_root()).expect("request");
        let schemas = schema_root();
        let context = ProviderRunContext::new(&registry, &store, &schemas);

        let execution = FakeProviderAdapter::success()
            .execute(&request, &context)
            .expect("execute fake provider");

        assert_eq!(execution.result().status(), "success");
        assert_eq!(
            execution.result().value()["metrics"]["estimated_cost"],
            json!(0)
        );
        assert_eq!(
            execution.request_ref()["path"],
            "provider-output/fake-default/request.json"
        );
        assert_eq!(
            execution.response_ref()["path"],
            "provider-output/fake-default/response.json"
        );
        assert_eq!(
            execution.stdout_ref()["path"],
            "provider-output/fake-default/stdout.txt"
        );
        assert!(execution.stderr_ref().is_none());
        assert!(project
            .join(".ai-runs/J-0001/provider-output/fake-default/request.json")
            .is_file());
        assert!(project
            .join(".ai-runs/J-0001/provider-output/fake-default/response.json")
            .is_file());

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn fake_provider_simulates_failed_and_blocked_results() {
        let failed = execute_with_adapter(FakeProviderAdapter::failed("unit failure"));
        assert_eq!(failed.result().status(), "failed");
        assert_eq!(failed.result().value()["error"]["kind"], "fake_failed");
        assert!(failed.stderr_ref().is_some());

        let blocked = execute_with_adapter(FakeProviderAdapter::blocked("approval required"));
        assert_eq!(blocked.result().status(), "blocked");
        assert_eq!(blocked.result().value()["error"]["kind"], "fake_blocked");
        assert!(blocked.stderr_ref().is_some());
    }

    #[test]
    fn fake_provider_refuses_to_overwrite_existing_output() {
        let project = temp_project();
        let store = open_store(&project);
        store
            .create_job("implement feature", "codex", vec![])
            .expect("create job");
        let registry = ProviderRegistryLoader::new(repo_root())
            .load_fake_default_registry()
            .expect("load fake registry");
        let request =
            ExecutionRequest::from_value(request_value("overwrite"), "request.json", schema_root())
                .expect("request");
        let schemas = schema_root();
        let context = ProviderRunContext::new(&registry, &store, &schemas);

        FakeProviderAdapter::success()
            .execute(&request, &context)
            .expect("first execute");
        let error = FakeProviderAdapter::success()
            .execute(&request, &context)
            .expect_err("second execute should fail");

        assert!(matches!(
            error,
            ProviderAdapterError::ProviderOutputAlreadyExists { .. }
        ));
        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn fake_provider_rejects_non_fake_instance() {
        let project = temp_project();
        let store = open_store(&project);
        store
            .create_job("implement feature", "codex", vec![])
            .expect("create job");
        let registry = ProviderRegistryLoader::new(repo_root())
            .load_registry(
                "configs/registries/builtin-provider-registry.yaml",
                &[PathBuf::from(
                    "configs/provider-instances/codex-cli.example.yaml",
                )],
            )
            .expect("load builtin registry");
        let mut request = request_value("wrong provider");
        request["provider_instance_id"] = json!("my-codex-cli");
        let request =
            ExecutionRequest::from_value(request, "request.json", schema_root()).expect("request");
        let schemas = schema_root();
        let context = ProviderRunContext::new(&registry, &store, &schemas);

        let error = FakeProviderAdapter::success()
            .execute(&request, &context)
            .expect_err("non-fake instance should fail");
        assert!(matches!(
            error,
            ProviderAdapterError::UnsupportedProvider { .. }
        ));

        fs::remove_dir_all(project).ok();
    }

    fn execute_with_adapter(adapter: FakeProviderAdapter) -> ProviderExecution {
        let project = temp_project();
        let store = open_store(&project);
        store
            .create_job("implement feature", "codex", vec![])
            .expect("create job");
        let registry = ProviderRegistryLoader::new(repo_root())
            .load_fake_default_registry()
            .expect("load fake registry");
        let request =
            ExecutionRequest::from_value(request_value("simulate"), "request.json", schema_root())
                .expect("request");
        let schemas = schema_root();
        let context = ProviderRunContext::new(&registry, &store, &schemas);
        let execution = adapter.execute(&request, &context).expect("execute");
        fs::remove_dir_all(project).ok();
        execution
    }

    fn request_value(goal: &str) -> Value {
        json!({
            "schema_version": "1.0.0",
            "request_id": "request-0001",
            "job_id": "J-0001",
            "stage": "implement",
            "provider_instance_id": "fake-default",
            "attempt_id": "attempt-0001",
            "workspec_path": "workspecs/implement.json",
            "created_at": "2026-06-28T00:00:00Z",
            "goal": goal,
            "allowed_scope": ["src/**", "tests/**"],
            "forbidden_actions": ["dependency_install", "file_delete"],
            "required_outputs": ["provider-output/fake-default/response.json"],
            "validation_requirements": ["policy:p0"],
            "context_pack": { "files": [] }
        })
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
            "star-control-provider-fake-{}-{}",
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
