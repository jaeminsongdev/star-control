use serde_json::{json, Value};
use star_control_schema::{load_schema, validate_json, ValidationError};
use star_control_state::{StateStore, StateStoreError};
use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: &str = "1.0.0";
const DAEMON_DIR: &str = "daemon";
const DAEMON_STATE_FILE: &str = "state.json";
const DAEMON_STATE_SCHEMA: &str = "daemon-state.schema.json";
const APPROVAL_RESPONSE_SCHEMA: &str = "approval-response.schema.json";
const DEFAULT_DAEMON_ID: &str = "local-daemon";
const DEFAULT_PRIORITY: &str = "normal";
const QUEUED_STATE: &str = "QUEUED";
const TERMINAL_STATES: &[&str] = &["DONE", "FAILED", "BLOCKED", "CANCELLED"];

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    daemon_id: String,
    config_root: PathBuf,
    schema_root: PathBuf,
}

impl DaemonConfig {
    pub fn new(
        daemon_id: impl Into<String>,
        config_root: impl Into<PathBuf>,
        schema_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            daemon_id: daemon_id.into(),
            config_root: config_root.into(),
            schema_root: schema_root.into(),
        }
    }

    pub fn local(config_root: impl Into<PathBuf>, schema_root: impl Into<PathBuf>) -> Self {
        Self::new(DEFAULT_DAEMON_ID, config_root, schema_root)
    }

    pub fn daemon_id(&self) -> &str {
        &self.daemon_id
    }

    pub fn config_root(&self) -> &Path {
        &self.config_root
    }

    pub fn schema_root(&self) -> &Path {
        &self.schema_root
    }
}

#[derive(Debug)]
pub enum DaemonError {
    ConfigDirectoryFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    StateReadFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    StateWriteFailed {
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
        errors: Vec<ValidationError>,
    },
    InvalidDaemonState {
        message: String,
    },
    StateStore {
        source: StateStoreError,
    },
    TerminalJobRejected {
        job_id: String,
        state: String,
    },
    ApprovalRequired {
        job_id: String,
        path: PathBuf,
    },
    ApprovalResponseNotApproved {
        job_id: String,
        response: String,
    },
    ApprovalJobMismatch {
        expected: String,
        actual: String,
    },
    DuplicateQueuedJob {
        job_id: String,
        project_root: String,
    },
}

impl fmt::Display for DaemonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfigDirectoryFailed { path, source } => {
                write!(
                    formatter,
                    "failed to create daemon config directory {}: {}",
                    path.display(),
                    source
                )
            }
            Self::StateReadFailed { path, source } => {
                write!(
                    formatter,
                    "failed to read daemon state {}: {}",
                    path.display(),
                    source
                )
            }
            Self::StateWriteFailed { path, source } => {
                write!(
                    formatter,
                    "failed to write daemon state {}: {}",
                    path.display(),
                    source
                )
            }
            Self::InvalidJson { path, source } => {
                write!(formatter, "invalid JSON at {}: {}", path.display(), source)
            }
            Self::SchemaLoadFailed { path, message } => {
                write!(
                    formatter,
                    "schema load failed at {}: {}",
                    path.display(),
                    message
                )
            }
            Self::SchemaValidationFailed { path, errors } => {
                write!(
                    formatter,
                    "schema validation failed for {} with {} error(s)",
                    path.display(),
                    errors.len()
                )
            }
            Self::InvalidDaemonState { message } => {
                write!(formatter, "invalid daemon state: {}", message)
            }
            Self::StateStore { source } => write!(formatter, "state store error: {}", source),
            Self::TerminalJobRejected { job_id, state } => {
                write!(
                    formatter,
                    "job {} is terminal and cannot be queued: {}",
                    job_id, state
                )
            }
            Self::ApprovalRequired { job_id, path } => {
                write!(
                    formatter,
                    "job {} requires approval response at {}",
                    job_id,
                    path.display()
                )
            }
            Self::ApprovalResponseNotApproved { job_id, response } => {
                write!(
                    formatter,
                    "job {} approval response is not approved: {}",
                    job_id, response
                )
            }
            Self::ApprovalJobMismatch { expected, actual } => {
                write!(
                    formatter,
                    "approval response job_id mismatch: expected {}, got {}",
                    expected, actual
                )
            }
            Self::DuplicateQueuedJob {
                job_id,
                project_root,
            } => {
                write!(
                    formatter,
                    "job {} is already queued for project {}",
                    job_id, project_root
                )
            }
        }
    }
}

impl Error for DaemonError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ConfigDirectoryFailed { source, .. }
            | Self::StateReadFailed { source, .. }
            | Self::StateWriteFailed { source, .. } => Some(source),
            Self::InvalidJson { source, .. } => Some(source),
            Self::StateStore { source } => Some(source),
            Self::SchemaLoadFailed { .. }
            | Self::SchemaValidationFailed { .. }
            | Self::InvalidDaemonState { .. }
            | Self::TerminalJobRejected { .. }
            | Self::ApprovalRequired { .. }
            | Self::ApprovalResponseNotApproved { .. }
            | Self::ApprovalJobMismatch { .. }
            | Self::DuplicateQueuedJob { .. } => None,
        }
    }
}

impl From<StateStoreError> for DaemonError {
    fn from(source: StateStoreError) -> Self {
        Self::StateStore { source }
    }
}

#[derive(Debug, Clone)]
pub struct DaemonQueue {
    config: DaemonConfig,
    daemon_dir: PathBuf,
    state_path: PathBuf,
}

impl DaemonQueue {
    pub fn open(config: DaemonConfig) -> Result<Self, DaemonError> {
        let daemon_dir = config.config_root().join(DAEMON_DIR);
        fs::create_dir_all(&daemon_dir).map_err(|source| DaemonError::ConfigDirectoryFailed {
            path: daemon_dir.clone(),
            source,
        })?;
        let state_path = daemon_dir.join(DAEMON_STATE_FILE);
        let queue = Self {
            config,
            daemon_dir,
            state_path,
        };
        if !queue.state_path.is_file() {
            queue.save_state(&queue.default_state())?;
        } else {
            queue.load_state()?;
        }
        Ok(queue)
    }

    pub fn daemon_dir(&self) -> &Path {
        &self.daemon_dir
    }

    pub fn state_path(&self) -> &Path {
        &self.state_path
    }

    pub fn load_state(&self) -> Result<Value, DaemonError> {
        let content = fs::read_to_string(&self.state_path).map_err(|source| {
            DaemonError::StateReadFailed {
                path: self.state_path.clone(),
                source,
            }
        })?;
        let state: Value =
            serde_json::from_str(&content).map_err(|source| DaemonError::InvalidJson {
                path: self.state_path.clone(),
                source,
            })?;
        self.validate_schema(DAEMON_STATE_SCHEMA, &self.state_path, &state)?;
        Ok(state)
    }

    pub fn enqueue_project_job(
        &self,
        project_store: &StateStore,
        job_id: &str,
    ) -> Result<Value, DaemonError> {
        project_store.load_job(job_id)?;
        let run_state = project_store.load_state(job_id)?;
        let state = string_field(&run_state, "state").unwrap_or_default();
        if TERMINAL_STATES.contains(&state.as_str()) {
            return Err(DaemonError::TerminalJobRejected {
                job_id: job_id.to_string(),
                state,
            });
        }
        if state == "WAITING_APPROVAL" {
            self.ensure_approved_response(project_store, job_id)?;
        }

        let project_root = project_store.project_root().display().to_string();
        let current_stage =
            string_field(&run_state, "current_stage").unwrap_or_else(|| "implement".to_string());
        let entry = json!({
            "job_id": job_id,
            "priority": DEFAULT_PRIORITY,
            "state": QUEUED_STATE,
            "project_root": project_root,
            "current_stage": current_stage,
            "run_state": state,
            "run_dir": format!(".ai-runs/{}", job_id)
        });

        let mut daemon_state = self.load_state()?;
        let queue = daemon_state
            .get_mut("queue")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| DaemonError::InvalidDaemonState {
                message: "queue must be an array".to_string(),
            })?;
        if queue.iter().any(|item| {
            item.get("job_id").and_then(Value::as_str) == Some(job_id)
                && item.get("project_root").and_then(Value::as_str) == Some(project_root.as_str())
        }) {
            return Err(DaemonError::DuplicateQueuedJob {
                job_id: job_id.to_string(),
                project_root,
            });
        }
        queue.push(entry.clone());
        self.save_state(&daemon_state)?;
        Ok(entry)
    }

    pub fn queue_len(&self) -> Result<usize, DaemonError> {
        let state = self.load_state()?;
        state
            .get("queue")
            .and_then(Value::as_array)
            .map(Vec::len)
            .ok_or_else(|| DaemonError::InvalidDaemonState {
                message: "queue must be an array".to_string(),
            })
    }

    fn default_state(&self) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "daemon_id": self.config.daemon_id(),
            "status": "reserved",
            "queue": [],
            "active_jobs": [],
            "last_error": null
        })
    }

    fn save_state(&self, state: &Value) -> Result<(), DaemonError> {
        self.validate_schema(DAEMON_STATE_SCHEMA, &self.state_path, state)?;
        let mut bytes =
            serde_json::to_vec_pretty(state).map_err(|source| DaemonError::InvalidJson {
                path: self.state_path.clone(),
                source,
            })?;
        bytes.push(b'\n');
        write_bytes_atomic(&self.daemon_dir, &self.state_path, &bytes)
    }

    fn ensure_approved_response(
        &self,
        project_store: &StateStore,
        job_id: &str,
    ) -> Result<(), DaemonError> {
        let response_path =
            project_store.resolve_job_path(job_id, "approvals/approval-response.json")?;
        if !response_path.is_file() {
            return Err(DaemonError::ApprovalRequired {
                job_id: job_id.to_string(),
                path: response_path,
            });
        }
        let content =
            fs::read_to_string(&response_path).map_err(|source| DaemonError::StateReadFailed {
                path: response_path.clone(),
                source,
            })?;
        let response: Value =
            serde_json::from_str(&content).map_err(|source| DaemonError::InvalidJson {
                path: response_path.clone(),
                source,
            })?;
        self.validate_schema(APPROVAL_RESPONSE_SCHEMA, &response_path, &response)?;
        let actual_job_id = response
            .get("job_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if actual_job_id != job_id {
            return Err(DaemonError::ApprovalJobMismatch {
                expected: job_id.to_string(),
                actual: actual_job_id.to_string(),
            });
        }
        let response_value = response
            .get("response")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if response_value != "approved" {
            return Err(DaemonError::ApprovalResponseNotApproved {
                job_id: job_id.to_string(),
                response: response_value.to_string(),
            });
        }
        Ok(())
    }

    fn validate_schema(
        &self,
        schema_file: &str,
        document_path: &Path,
        value: &Value,
    ) -> Result<(), DaemonError> {
        let schema_path = self.config.schema_root().join(schema_file);
        let schema = load_schema(&schema_path).map_err(|source| DaemonError::SchemaLoadFailed {
            path: schema_path,
            message: source.to_string(),
        })?;
        let result = validate_json(value, &schema);
        if result.is_ok() {
            Ok(())
        } else {
            Err(DaemonError::SchemaValidationFailed {
                path: document_path.to_path_buf(),
                errors: result.errors,
            })
        }
    }
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_string)
}

fn write_bytes_atomic(tmp_dir: &Path, target_path: &Path, bytes: &[u8]) -> Result<(), DaemonError> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|source| DaemonError::StateWriteFailed {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::create_dir_all(tmp_dir).map_err(|source| DaemonError::StateWriteFailed {
        path: tmp_dir.to_path_buf(),
        source,
    })?;

    let tmp_name = format!(
        "{}.tmp-{}-{}",
        target_path
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("state.json"),
        std::process::id(),
        timestamp_nanos()
    );
    let tmp_path = tmp_dir.join(tmp_name);
    {
        let mut file = File::create(&tmp_path).map_err(|source| DaemonError::StateWriteFailed {
            path: tmp_path.clone(),
            source,
        })?;
        file.write_all(bytes)
            .and_then(|_| file.flush())
            .and_then(|_| file.sync_all())
            .map_err(|source| DaemonError::StateWriteFailed {
                path: tmp_path.clone(),
                source,
            })?;
    }
    replace_file(&tmp_path, target_path).map_err(|source| DaemonError::StateWriteFailed {
        path: target_path.to_path_buf(),
        source,
    })
}

fn timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(windows)]
fn replace_file(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "Kernel32")]
    extern "system" {
        fn MoveFileExW(existing: *const u16, new_name: *const u16, flags: u32) -> i32;
    }

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    let source = wide(source);
    let target = wide(target);
    let ok = unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(source, target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn schema_root() -> PathBuf {
        repo_root().join("specs/schemas")
    }

    fn temp_dir(name: &str) -> PathBuf {
        let count = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "star-control-daemon-{}-{}-{}-{}",
            name,
            std::process::id(),
            timestamp_nanos(),
            count
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn open_project_store(project: &Path) -> StateStore {
        StateStore::open(project, schema_root()).expect("open project store")
    }

    fn open_daemon_queue(config_root: &Path) -> DaemonQueue {
        DaemonQueue::open(DaemonConfig::local(config_root, schema_root()))
            .expect("open daemon queue")
    }

    fn create_job(store: &StateStore, state_name: &str, stage: &str) {
        let job = store
            .create_job("test request", "README.md", Vec::new())
            .expect("create job");
        let job_id = job["job_id"].as_str().expect("job id");
        assert_eq!(job_id, "J-0001");
        store
            .save_state(job_id, &run_state(job_id, state_name, stage))
            .expect("save run state");
    }

    fn run_state(job_id: &str, state_name: &str, stage: &str) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": job_id,
            "state": state_name,
            "current_stage": stage,
            "workers": {},
            "artifacts": {},
            "next_action": "run"
        })
    }

    fn approval_response(job_id: &str, response: &str) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": job_id,
            "stage": "validate",
            "task_id": "approval-1",
            "response": response,
            "reviewer": "test",
            "responded_at": "unix:1",
            "reason": "test",
            "allowed_next_stage": "report",
            "constraints": []
        })
    }

    #[test]
    fn opens_default_state_under_config_root_not_project_root() {
        let project = temp_dir("project");
        let config = temp_dir("config");
        let queue = open_daemon_queue(&config);

        assert_eq!(
            queue.state_path(),
            config.join("daemon/state.json").as_path()
        );
        assert!(queue.state_path().is_file());
        assert!(!project.join("daemon/state.json").exists());
        assert!(!project.join(".star-control/daemon/state.json").exists());

        let state = queue.load_state().expect("load daemon state");
        assert_eq!(state["schema_version"], SCHEMA_VERSION);
        assert_eq!(state["daemon_id"], DEFAULT_DAEMON_ID);
        assert_eq!(state["status"], "reserved");
        assert_eq!(state["queue"].as_array().expect("queue").len(), 0);

        fs::remove_dir_all(project).ok();
        fs::remove_dir_all(config).ok();
    }

    #[test]
    fn enqueue_nonterminal_job_records_project_reference_without_copying_artifacts() {
        let project = temp_dir("project");
        let config = temp_dir("config");
        let store = open_project_store(&project);
        create_job(&store, "ROUTED", "implement");
        let queue = open_daemon_queue(&config);

        let entry = queue
            .enqueue_project_job(&store, "J-0001")
            .expect("enqueue job");
        assert_eq!(entry["job_id"], "J-0001");
        assert_eq!(entry["priority"], DEFAULT_PRIORITY);
        assert_eq!(entry["state"], QUEUED_STATE);
        assert_eq!(
            entry["project_root"],
            store.project_root().display().to_string()
        );
        assert_eq!(entry["current_stage"], "implement");
        assert_eq!(entry["run_state"], "ROUTED");
        assert_eq!(entry["run_dir"], ".ai-runs/J-0001");

        let daemon_state = queue.load_state().expect("load daemon state");
        assert_eq!(daemon_state["queue"].as_array().expect("queue").len(), 1);
        assert!(project.join(".ai-runs/J-0001/job.json").is_file());
        assert!(project.join(".ai-runs/J-0001/run-state.json").is_file());
        assert!(!queue.daemon_dir().join(".ai-runs").exists());

        fs::remove_dir_all(project).ok();
        fs::remove_dir_all(config).ok();
    }

    #[test]
    fn terminal_job_is_not_queued() {
        let project = temp_dir("project");
        let config = temp_dir("config");
        let store = open_project_store(&project);
        create_job(&store, "DONE", "report");
        let queue = open_daemon_queue(&config);

        let error = queue
            .enqueue_project_job(&store, "J-0001")
            .expect_err("terminal job rejected");
        assert!(matches!(error, DaemonError::TerminalJobRejected { .. }));
        assert_eq!(queue.queue_len().expect("queue len"), 0);

        fs::remove_dir_all(project).ok();
        fs::remove_dir_all(config).ok();
    }

    #[test]
    fn waiting_approval_requires_approved_response() {
        let project = temp_dir("project");
        let config = temp_dir("config");
        let store = open_project_store(&project);
        create_job(&store, "WAITING_APPROVAL", "validate");
        let queue = open_daemon_queue(&config);

        let missing = queue
            .enqueue_project_job(&store, "J-0001")
            .expect_err("missing approval response rejected");
        assert!(matches!(missing, DaemonError::ApprovalRequired { .. }));

        store
            .write_approval_json(
                "J-0001",
                "approval-response.json",
                &approval_response("J-0001", "approved"),
            )
            .expect("write approval response");
        let entry = queue
            .enqueue_project_job(&store, "J-0001")
            .expect("enqueue approved job");
        assert_eq!(entry["run_state"], "WAITING_APPROVAL");
        assert_eq!(queue.queue_len().expect("queue len"), 1);

        fs::remove_dir_all(project).ok();
        fs::remove_dir_all(config).ok();
    }

    #[test]
    fn non_approved_response_is_not_queued() {
        let project = temp_dir("project");
        let config = temp_dir("config");
        let store = open_project_store(&project);
        create_job(&store, "WAITING_APPROVAL", "validate");
        store
            .write_approval_json(
                "J-0001",
                "approval-response.json",
                &approval_response("J-0001", "needs_changes"),
            )
            .expect("write approval response");
        let queue = open_daemon_queue(&config);

        let error = queue
            .enqueue_project_job(&store, "J-0001")
            .expect_err("needs_changes rejected");
        assert!(matches!(
            error,
            DaemonError::ApprovalResponseNotApproved { .. }
        ));
        assert_eq!(queue.queue_len().expect("queue len"), 0);

        fs::remove_dir_all(project).ok();
        fs::remove_dir_all(config).ok();
    }

    #[test]
    fn duplicate_queue_entry_is_rejected() {
        let project = temp_dir("project");
        let config = temp_dir("config");
        let store = open_project_store(&project);
        create_job(&store, "VALIDATED", "report");
        let queue = open_daemon_queue(&config);

        queue
            .enqueue_project_job(&store, "J-0001")
            .expect("first enqueue");
        let error = queue
            .enqueue_project_job(&store, "J-0001")
            .expect_err("duplicate rejected");
        assert!(matches!(error, DaemonError::DuplicateQueuedJob { .. }));
        assert_eq!(queue.queue_len().expect("queue len"), 1);

        fs::remove_dir_all(project).ok();
        fs::remove_dir_all(config).ok();
    }
}
