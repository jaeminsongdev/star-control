use serde_json::{json, Value};
use star_control_schema::{load_schema, validate_json, ValidationError};
use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: &str = "1.0.0";
const AI_RUNS_DIR: &str = ".ai-runs";
const TERMINAL_STATES: &[&str] = &["DONE", "FAILED", "BLOCKED", "CANCELLED"];
const CANONICAL_STAGES: &[&str] = &[
    "route",
    "plan",
    "design",
    "implement",
    "validate",
    "review",
    "polish",
    "report",
];

#[derive(Debug)]
pub enum StateStoreError {
    ProjectRootNotFound {
        path: PathBuf,
    },
    ProjectRootNotDirectory {
        path: PathBuf,
    },
    AiRunsNotWritable {
        path: PathBuf,
        source: std::io::Error,
    },
    JobNotFound {
        job_id: String,
    },
    JobAlreadyExists {
        job_id: String,
    },
    ArtifactNotFound {
        path: PathBuf,
    },
    ArtifactAlreadyExists {
        path: PathBuf,
    },
    InvalidArtifactShape {
        message: String,
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
    CorruptEventLog {
        path: PathBuf,
        line: usize,
        message: String,
    },
    AtomicWriteFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    PathTraversalBlocked {
        path: String,
    },
    PathOutsideJobDirectory {
        path: PathBuf,
    },
    TerminalStateBlocked {
        job_id: String,
        state: String,
    },
    InvalidJobId {
        job_id: String,
    },
    InvalidStage {
        stage: String,
    },
    JobIdMismatch {
        expected: String,
        actual: String,
    },
}

impl fmt::Display for StateStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectRootNotFound { path } => {
                write!(formatter, "project root does not exist: {}", path.display())
            }
            Self::ProjectRootNotDirectory { path } => {
                write!(
                    formatter,
                    "project root is not a directory: {}",
                    path.display()
                )
            }
            Self::AiRunsNotWritable { path, source } => {
                write!(
                    formatter,
                    ".ai-runs directory is not writable at {}: {}",
                    path.display(),
                    source
                )
            }
            Self::JobNotFound { job_id } => write!(formatter, "job not found: {}", job_id),
            Self::JobAlreadyExists { job_id } => {
                write!(formatter, "job already exists: {}", job_id)
            }
            Self::ArtifactNotFound { path } => {
                write!(formatter, "artifact not found: {}", path.display())
            }
            Self::ArtifactAlreadyExists { path } => {
                write!(formatter, "artifact already exists: {}", path.display())
            }
            Self::InvalidArtifactShape { message } => {
                write!(formatter, "invalid artifact shape: {}", message)
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
            Self::CorruptEventLog {
                path,
                line,
                message,
            } => write!(
                formatter,
                "corrupt event log {} at line {}: {}",
                path.display(),
                line,
                message
            ),
            Self::AtomicWriteFailed { path, source } => {
                write!(
                    formatter,
                    "atomic write failed for {}: {}",
                    path.display(),
                    source
                )
            }
            Self::PathTraversalBlocked { path } => {
                write!(formatter, "path traversal blocked: {}", path)
            }
            Self::PathOutsideJobDirectory { path } => {
                write!(
                    formatter,
                    "path is outside job directory: {}",
                    path.display()
                )
            }
            Self::TerminalStateBlocked { job_id, state } => {
                write!(formatter, "job {} is in terminal state {}", job_id, state)
            }
            Self::InvalidJobId { job_id } => write!(formatter, "invalid job id: {}", job_id),
            Self::InvalidStage { stage } => write!(formatter, "invalid stage: {}", stage),
            Self::JobIdMismatch { expected, actual } => {
                write!(
                    formatter,
                    "artifact job_id mismatch: expected {}, got {}",
                    expected, actual
                )
            }
        }
    }
}

impl Error for StateStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AiRunsNotWritable { source, .. } => Some(source),
            Self::InvalidJson { source, .. } => Some(source),
            Self::AtomicWriteFailed { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobSummary {
    pub job_id: String,
    pub state: Option<String>,
    pub current_stage: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub summary: Option<String>,
    pub corrupt: bool,
    pub corrupt_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryIssue {
    pub artifact_path: String,
    pub kind: String,
    pub severity: String,
    pub message: String,
    pub recommended_action: String,
}

impl RecoveryIssue {
    pub fn new(
        artifact_path: impl Into<String>,
        kind: impl Into<String>,
        severity: impl Into<String>,
        message: impl Into<String>,
        recommended_action: impl Into<String>,
    ) -> Self {
        Self {
            artifact_path: artifact_path.into(),
            kind: kind.into(),
            severity: severity.into(),
            message: message.into(),
            recommended_action: recommended_action.into(),
        }
    }

    pub fn to_value(&self) -> Value {
        json!({
            "artifact_path": self.artifact_path,
            "kind": self.kind,
            "severity": self.severity,
            "message": self.message,
            "recommended_action": self.recommended_action
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryInspection {
    pub job_id: String,
    pub mode: String,
    pub status: String,
    pub manual_followup_required: bool,
    pub destructive_actions_performed: bool,
    pub issues: Vec<RecoveryIssue>,
}

impl RecoveryInspection {
    fn inspect_only(job_id: impl Into<String>, issues: Vec<RecoveryIssue>) -> Self {
        let manual_followup_required = !issues.is_empty();
        Self {
            job_id: job_id.into(),
            mode: "inspect_only".to_string(),
            status: if manual_followup_required {
                "needs_recovery".to_string()
            } else {
                "ok".to_string()
            },
            manual_followup_required,
            destructive_actions_performed: false,
            issues,
        }
    }

    pub fn to_value(&self) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": self.job_id,
            "mode": self.mode,
            "status": self.status,
            "manual_followup_required": self.manual_followup_required,
            "destructive_actions_performed": self.destructive_actions_performed,
            "issues": self.issues.iter().map(RecoveryIssue::to_value).collect::<Vec<_>>()
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    Job,
    State,
    EventLog,
    Route,
    WorkSpec,
    Report,
    ProviderOutput,
    ToolOutput,
    Approval,
    ReviewPack,
    Log,
    Other,
}

impl ArtifactKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Job => "job",
            Self::State => "state",
            Self::EventLog => "event_log",
            Self::Route => "route",
            Self::WorkSpec => "workspec",
            Self::Report => "report",
            Self::ProviderOutput => "provider_output",
            Self::ToolOutput => "tool_output",
            Self::Approval => "approval",
            Self::ReviewPack => "review_pack",
            Self::Log => "log",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StateStore {
    project_root: PathBuf,
    ai_runs_dir: PathBuf,
    schema_root: PathBuf,
}

impl StateStore {
    pub fn open(
        project_root: impl AsRef<Path>,
        schema_root: impl AsRef<Path>,
    ) -> Result<Self, StateStoreError> {
        let project_root = project_root.as_ref();
        if !project_root.exists() {
            return Err(StateStoreError::ProjectRootNotFound {
                path: project_root.to_path_buf(),
            });
        }
        if !project_root.is_dir() {
            return Err(StateStoreError::ProjectRootNotDirectory {
                path: project_root.to_path_buf(),
            });
        }

        let project_root = fs::canonicalize(project_root).map_err(|source| {
            StateStoreError::AiRunsNotWritable {
                path: project_root.to_path_buf(),
                source,
            }
        })?;
        let ai_runs_dir = project_root.join(AI_RUNS_DIR);
        fs::create_dir_all(&ai_runs_dir).map_err(|source| StateStoreError::AiRunsNotWritable {
            path: ai_runs_dir.clone(),
            source,
        })?;

        Ok(Self {
            project_root,
            ai_runs_dir,
            schema_root: schema_root.as_ref().to_path_buf(),
        })
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn ai_runs_dir(&self) -> &Path {
        &self.ai_runs_dir
    }

    pub fn schema_root(&self) -> &Path {
        &self.schema_root
    }

    pub fn allocate_job_id(&self) -> Result<String, StateStoreError> {
        let mut highest = 0_u64;
        for entry in fs::read_dir(&self.ai_runs_dir).map_err(|source| {
            StateStoreError::AiRunsNotWritable {
                path: self.ai_runs_dir.clone(),
                source,
            }
        })? {
            let entry = entry.map_err(|source| StateStoreError::AiRunsNotWritable {
                path: self.ai_runs_dir.clone(),
                source,
            })?;
            if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if let Some(number) = parse_job_number(&name) {
                highest = highest.max(number);
            }
        }

        Ok(format!("J-{:04}", highest + 1))
    }

    pub fn create_job(
        &self,
        request_text: impl Into<String>,
        entrypoint: impl Into<String>,
        user_constraints: Vec<String>,
    ) -> Result<Value, StateStoreError> {
        let job_id = self.allocate_job_id()?;
        let job_dir = self.create_job_dir(&job_id)?;
        ensure_standard_dirs(&job_dir)?;

        let timestamp = timestamp_string();
        let job = json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": job_id,
            "project_root": self.project_root.display().to_string(),
            "request_text": request_text.into(),
            "created_at": timestamp,
            "updated_at": timestamp,
            "entrypoint": entrypoint.into(),
            "state": "REQUESTED",
            "user_constraints": user_constraints,
        });

        self.save_job(&job_id, &job)?;
        self.append_event(
            &job_id,
            &json!({
                "schema_version": SCHEMA_VERSION,
                "event_id": format!("{}-0001", job_id),
                "job_id": job_id,
                "type": "JOB_CREATED",
                "created_at": timestamp,
                "state": "REQUESTED",
                "message": "Job created",
                "artifact_paths": ["job.json"],
                "details": {}
            }),
        )?;

        Ok(job)
    }

    pub fn create_job_dir(&self, job_id: &str) -> Result<PathBuf, StateStoreError> {
        validate_job_id(job_id)?;
        let job_dir = self.ai_runs_dir.join(job_id);
        if job_dir.exists() {
            return Err(StateStoreError::JobAlreadyExists {
                job_id: job_id.to_string(),
            });
        }
        fs::create_dir_all(&job_dir).map_err(|source| StateStoreError::AiRunsNotWritable {
            path: job_dir.clone(),
            source,
        })?;
        Ok(job_dir)
    }

    pub fn job_dir(&self, job_id: &str) -> Result<PathBuf, StateStoreError> {
        validate_job_id(job_id)?;
        let job_dir = self.ai_runs_dir.join(job_id);
        if !job_dir.is_dir() {
            return Err(StateStoreError::JobNotFound {
                job_id: job_id.to_string(),
            });
        }
        Ok(job_dir)
    }

    pub fn save_job(&self, job_id: &str, job: &Value) -> Result<(), StateStoreError> {
        ensure_artifact_job_id(job, job_id)?;
        self.write_json_artifact(job_id, "job.json", CoreSchema::Job, job)
    }

    pub fn load_job(&self, job_id: &str) -> Result<Value, StateStoreError> {
        self.read_json_artifact(job_id, "job.json", CoreSchema::Job)
    }

    pub fn save_state(&self, job_id: &str, state: &Value) -> Result<(), StateStoreError> {
        ensure_artifact_job_id(state, job_id)?;
        self.write_json_artifact(job_id, "run-state.json", CoreSchema::RunState, state)
    }

    pub fn load_state(&self, job_id: &str) -> Result<Value, StateStoreError> {
        self.read_json_artifact(job_id, "run-state.json", CoreSchema::RunState)
    }

    pub fn append_event(&self, job_id: &str, event: &Value) -> Result<(), StateStoreError> {
        ensure_artifact_job_id(event, job_id)?;
        self.validate_artifact(
            CoreSchema::Event,
            self.job_dir(job_id)?.join("events.jsonl"),
            event,
        )?;
        let events_path = self.resolve_job_path(job_id, "events.jsonl")?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&events_path)
            .map_err(|source| StateStoreError::AtomicWriteFailed {
                path: events_path.clone(),
                source,
            })?;
        serde_json::to_writer(&mut file, event).map_err(|source| StateStoreError::InvalidJson {
            path: events_path.clone(),
            source,
        })?;
        file.write_all(b"\n")
            .and_then(|_| file.flush())
            .and_then(|_| file.sync_all())
            .map_err(|source| StateStoreError::AtomicWriteFailed {
                path: events_path,
                source,
            })
    }

    pub fn read_events(&self, job_id: &str) -> Result<Vec<Value>, StateStoreError> {
        let events_path = self.resolve_job_path(job_id, "events.jsonl")?;
        if !events_path.is_file() {
            return Ok(Vec::new());
        }
        let file =
            File::open(&events_path).map_err(|source| StateStoreError::AtomicWriteFailed {
                path: events_path.clone(),
                source,
            })?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();
        for (index, line) in reader.lines().enumerate() {
            let line_number = index + 1;
            let line = line.map_err(|source| StateStoreError::CorruptEventLog {
                path: events_path.clone(),
                line: line_number,
                message: source.to_string(),
            })?;
            if line.trim().is_empty() {
                continue;
            }
            let event: Value =
                serde_json::from_str(&line).map_err(|source| StateStoreError::CorruptEventLog {
                    path: events_path.clone(),
                    line: line_number,
                    message: source.to_string(),
                })?;
            self.validate_artifact(CoreSchema::Event, events_path.clone(), &event)?;
            events.push(event);
        }
        Ok(events)
    }

    pub fn save_route(&self, job_id: &str, route: &Value) -> Result<(), StateStoreError> {
        ensure_artifact_job_id(route, job_id)?;
        self.write_json_artifact(job_id, "route.json", CoreSchema::Route, route)
    }

    pub fn load_route(&self, job_id: &str) -> Result<Value, StateStoreError> {
        self.read_json_artifact(job_id, "route.json", CoreSchema::Route)
    }

    pub fn save_workspec(
        &self,
        job_id: &str,
        stage: &str,
        workspec: &Value,
    ) -> Result<(), StateStoreError> {
        validate_stage(stage)?;
        ensure_artifact_job_id(workspec, job_id)?;
        self.write_json_artifact(
            job_id,
            &format!("workspecs/{}.json", stage),
            CoreSchema::WorkSpec,
            workspec,
        )
    }

    pub fn load_workspec(&self, job_id: &str, stage: &str) -> Result<Value, StateStoreError> {
        validate_stage(stage)?;
        self.read_json_artifact(
            job_id,
            &format!("workspecs/{}.json", stage),
            CoreSchema::WorkSpec,
        )
    }

    pub fn save_report(
        &self,
        job_id: &str,
        name: &str,
        report: &Value,
    ) -> Result<(), StateStoreError> {
        validate_safe_name(name)?;
        ensure_artifact_job_id(report, job_id)?;
        self.write_json_artifact(
            job_id,
            &format!("reports/{}.json", name),
            CoreSchema::Report,
            report,
        )
    }

    pub fn load_report(&self, job_id: &str, name: &str) -> Result<Value, StateStoreError> {
        validate_safe_name(name)?;
        self.read_json_artifact(
            job_id,
            &format!("reports/{}.json", name),
            CoreSchema::Report,
        )
    }

    pub fn list_jobs(&self) -> Result<Vec<JobSummary>, StateStoreError> {
        let mut jobs = Vec::new();
        for entry in fs::read_dir(&self.ai_runs_dir).map_err(|source| {
            StateStoreError::AiRunsNotWritable {
                path: self.ai_runs_dir.clone(),
                source,
            }
        })? {
            let entry = entry.map_err(|source| StateStoreError::AiRunsNotWritable {
                path: self.ai_runs_dir.clone(),
                source,
            })?;
            if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                continue;
            }
            let Some(job_id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if !job_id.starts_with("J-") {
                continue;
            }
            jobs.push(self.job_summary(&job_id));
        }
        jobs.sort_by(|left, right| left.job_id.cmp(&right.job_id));
        Ok(jobs)
    }

    pub fn ensure_resume_allowed(&self, job_id: &str) -> Result<(), StateStoreError> {
        let state = self.load_state(job_id)?;
        let state_value = state
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if TERMINAL_STATES.contains(&state_value.as_str()) {
            return Err(StateStoreError::TerminalStateBlocked {
                job_id: job_id.to_string(),
                state: state_value,
            });
        }
        self.read_events(job_id)?;
        Ok(())
    }

    pub fn inspect_recovery(&self, job_id: &str) -> Result<RecoveryInspection, StateStoreError> {
        self.job_dir(job_id)?;
        let mut issues = Vec::new();

        if let Err(error) = self.load_job(job_id) {
            issues.push(recovery_issue_from_error("job.json", &error));
        }
        if let Err(error) = self.load_state(job_id) {
            issues.push(recovery_issue_from_error("run-state.json", &error));
        }
        if let Err(error) = self.read_events(job_id) {
            issues.push(recovery_issue_from_error("events.jsonl", &error));
        } else {
            let events_path = self.resolve_job_path(job_id, "events.jsonl")?;
            if !events_path.is_file() {
                issues.push(RecoveryIssue::new(
                    "events.jsonl",
                    "missing_required_file",
                    "block",
                    "required event log is missing",
                    "inspect the job and recreate only through an explicit recovery command",
                ));
            }
        }

        issues.extend(self.tmp_file_issues(job_id)?);
        Ok(RecoveryInspection::inspect_only(job_id, issues))
    }

    pub fn resolve_job_path(
        &self,
        job_id: &str,
        relative_path: &str,
    ) -> Result<PathBuf, StateStoreError> {
        let job_dir = self.job_dir(job_id)?;
        resolve_inside_job(&job_dir, relative_path)
    }

    pub fn resolve_provider_output_dir(
        &self,
        job_id: &str,
        provider_instance_id: &str,
    ) -> Result<PathBuf, StateStoreError> {
        validate_safe_name(provider_instance_id)?;
        self.resolve_job_path(job_id, &format!("provider-output/{}", provider_instance_id))
    }

    pub fn resolve_tool_output_dir(
        &self,
        job_id: &str,
        tool_output_dir: &str,
    ) -> Result<PathBuf, StateStoreError> {
        validate_safe_name(tool_output_dir)?;
        self.resolve_job_path(job_id, &format!("tool-output/{}", tool_output_dir))
    }

    pub fn artifact_ref(
        &self,
        job_id: &str,
        relative_path: &str,
        kind: ArtifactKind,
        producer: &str,
        schema_path: Option<&str>,
        description: Option<&str>,
    ) -> Result<Value, StateStoreError> {
        let normalized_path = normalized_relative_path(relative_path)?;
        self.resolve_job_path(job_id, &normalized_path)?;
        let artifact_ref = json!({
            "schema_version": SCHEMA_VERSION,
            "path": normalized_path,
            "kind": kind.as_str(),
            "producer": producer,
            "schema_path": schema_path,
            "description": description.unwrap_or("")
        });
        self.validate_artifact(
            CoreSchema::ArtifactRef,
            self.job_dir(job_id)?.join("artifact-ref.json"),
            &artifact_ref,
        )?;
        Ok(artifact_ref)
    }

    pub fn register_artifact_ref(
        &self,
        state: &mut Value,
        key: &str,
        artifact_ref: &Value,
    ) -> Result<(), StateStoreError> {
        validate_safe_name(key)?;
        self.validate_artifact(
            CoreSchema::ArtifactRef,
            PathBuf::from("artifact-ref.json"),
            artifact_ref,
        )?;
        let Some(state_object) = state.as_object_mut() else {
            return Err(StateStoreError::InvalidArtifactShape {
                message: "RunState must be a JSON object".to_string(),
            });
        };
        let artifacts = state_object
            .entry("artifacts")
            .or_insert_with(|| Value::Object(Default::default()));
        let Some(artifacts_object) = artifacts.as_object_mut() else {
            return Err(StateStoreError::InvalidArtifactShape {
                message: "RunState artifacts must be a JSON object".to_string(),
            });
        };
        artifacts_object.insert(key.to_string(), artifact_ref.clone());
        Ok(())
    }

    pub fn write_provider_json(
        &self,
        job_id: &str,
        provider_instance_id: &str,
        file_name: &str,
        value: &Value,
    ) -> Result<Value, StateStoreError> {
        validate_safe_name(provider_instance_id)?;
        validate_safe_name(file_name)?;
        let relative_path = format!("provider-output/{}/{}", provider_instance_id, file_name);
        self.write_new_json_artifact(job_id, &relative_path, value)?;
        self.artifact_ref(
            job_id,
            &relative_path,
            ArtifactKind::ProviderOutput,
            provider_instance_id,
            None,
            Some("provider JSON output"),
        )
    }

    pub fn write_provider_text(
        &self,
        job_id: &str,
        provider_instance_id: &str,
        file_name: &str,
        content: &str,
    ) -> Result<Value, StateStoreError> {
        validate_safe_name(provider_instance_id)?;
        validate_safe_name(file_name)?;
        let relative_path = format!("provider-output/{}/{}", provider_instance_id, file_name);
        self.write_new_text_artifact(job_id, &relative_path, content)?;
        self.artifact_ref(
            job_id,
            &relative_path,
            ArtifactKind::Log,
            provider_instance_id,
            None,
            Some("provider text output"),
        )
    }

    pub fn write_tool_json(
        &self,
        job_id: &str,
        tool_output_dir: &str,
        file_name: &str,
        value: &Value,
    ) -> Result<Value, StateStoreError> {
        validate_safe_name(tool_output_dir)?;
        validate_safe_name(file_name)?;
        let relative_path = format!("tool-output/{}/{}", tool_output_dir, file_name);
        self.write_new_json_artifact(job_id, &relative_path, value)?;
        self.artifact_ref(
            job_id,
            &relative_path,
            ArtifactKind::ToolOutput,
            tool_output_dir,
            None,
            Some("tool JSON output"),
        )
    }

    pub fn write_tool_text(
        &self,
        job_id: &str,
        tool_output_dir: &str,
        file_name: &str,
        content: &str,
    ) -> Result<Value, StateStoreError> {
        validate_safe_name(tool_output_dir)?;
        validate_safe_name(file_name)?;
        let relative_path = format!("tool-output/{}/{}", tool_output_dir, file_name);
        self.write_new_text_artifact(job_id, &relative_path, content)?;
        self.artifact_ref(
            job_id,
            &relative_path,
            ArtifactKind::ToolOutput,
            tool_output_dir,
            None,
            Some("tool text output"),
        )
    }

    pub fn write_approval_json(
        &self,
        job_id: &str,
        file_name: &str,
        value: &Value,
    ) -> Result<Value, StateStoreError> {
        validate_safe_name(file_name)?;
        let relative_path = format!("approvals/{}", file_name);
        self.write_new_json_artifact(job_id, &relative_path, value)?;
        self.artifact_ref(
            job_id,
            &relative_path,
            ArtifactKind::Approval,
            "state-store",
            None,
            Some("approval artifact"),
        )
    }

    pub fn write_review_pack_json(
        &self,
        job_id: &str,
        file_name: &str,
        value: &Value,
    ) -> Result<Value, StateStoreError> {
        validate_safe_name(file_name)?;
        let relative_path = format!("review-packs/{}", file_name);
        self.write_new_json_artifact(job_id, &relative_path, value)?;
        self.artifact_ref(
            job_id,
            &relative_path,
            ArtifactKind::ReviewPack,
            "state-store",
            None,
            Some("review pack JSON artifact"),
        )
    }

    pub fn write_review_pack_markdown(
        &self,
        job_id: &str,
        file_name: &str,
        content: &str,
    ) -> Result<Value, StateStoreError> {
        validate_safe_name(file_name)?;
        let relative_path = format!("review-packs/{}", file_name);
        self.write_new_text_artifact(job_id, &relative_path, content)?;
        self.artifact_ref(
            job_id,
            &relative_path,
            ArtifactKind::ReviewPack,
            "state-store",
            None,
            Some("review pack Markdown artifact"),
        )
    }

    pub fn write_validation_json(
        &self,
        job_id: &str,
        file_name: &str,
        value: &Value,
    ) -> Result<Value, StateStoreError> {
        validate_safe_name(file_name)?;
        let relative_path = format!("validation/{}", file_name);
        self.write_new_json_artifact(job_id, &relative_path, value)?;
        self.artifact_ref(
            job_id,
            &relative_path,
            ArtifactKind::Other,
            "validation-engine",
            None,
            Some("validation JSON artifact"),
        )
    }

    pub fn write_tmp_json(
        &self,
        job_id: &str,
        target_name: &str,
        value: &Value,
    ) -> Result<String, StateStoreError> {
        validate_safe_name(target_name)?;
        let relative_path = format!(
            "tmp/{}.tmp-{}-{}",
            target_name,
            std::process::id(),
            timestamp_nanos()
        );
        self.write_new_json_artifact(job_id, &relative_path, value)?;
        Ok(relative_path)
    }

    fn job_summary(&self, job_id: &str) -> JobSummary {
        match self.load_job(job_id) {
            Ok(job) => {
                let state = self.load_state(job_id).ok();
                JobSummary {
                    job_id: job_id.to_string(),
                    state: state
                        .as_ref()
                        .and_then(|state| state.get("state"))
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    current_stage: state
                        .as_ref()
                        .and_then(|state| state.get("current_stage"))
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    created_at: job
                        .get("created_at")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    updated_at: state
                        .as_ref()
                        .and_then(|state| state.get("updated_at"))
                        .and_then(Value::as_str)
                        .or_else(|| job.get("updated_at").and_then(Value::as_str))
                        .map(str::to_owned),
                    summary: job
                        .get("request_text")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    corrupt: false,
                    corrupt_reason: None,
                }
            }
            Err(error) => JobSummary {
                job_id: job_id.to_string(),
                state: None,
                current_stage: None,
                created_at: None,
                updated_at: None,
                summary: None,
                corrupt: true,
                corrupt_reason: Some(error.to_string()),
            },
        }
    }

    fn tmp_file_issues(&self, job_id: &str) -> Result<Vec<RecoveryIssue>, StateStoreError> {
        let tmp_dir = self.resolve_job_path(job_id, "tmp")?;
        if !tmp_dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut issues = Vec::new();
        collect_tmp_file_issues(&tmp_dir, "tmp", &mut issues)?;
        issues.sort_by(|left, right| left.artifact_path.cmp(&right.artifact_path));
        Ok(issues)
    }

    fn read_json_artifact(
        &self,
        job_id: &str,
        relative_path: &str,
        schema: CoreSchema,
    ) -> Result<Value, StateStoreError> {
        let path = self.resolve_job_path(job_id, relative_path)?;
        if !path.is_file() {
            return Err(StateStoreError::ArtifactNotFound { path });
        }
        let content =
            fs::read_to_string(&path).map_err(|source| StateStoreError::AtomicWriteFailed {
                path: path.clone(),
                source,
            })?;
        let value: Value =
            serde_json::from_str(&content).map_err(|source| StateStoreError::InvalidJson {
                path: path.clone(),
                source,
            })?;
        self.validate_artifact(schema, path, &value)?;
        Ok(value)
    }

    fn write_json_artifact(
        &self,
        job_id: &str,
        relative_path: &str,
        schema: CoreSchema,
        value: &Value,
    ) -> Result<(), StateStoreError> {
        let target_path = self.resolve_job_path(job_id, relative_path)?;
        self.validate_artifact(schema, target_path.clone(), value)?;
        self.write_json_value_atomic(job_id, relative_path, value)
    }

    fn write_new_json_artifact(
        &self,
        job_id: &str,
        relative_path: &str,
        value: &Value,
    ) -> Result<(), StateStoreError> {
        let target_path = self.resolve_job_path(job_id, relative_path)?;
        if target_path.exists() {
            return Err(StateStoreError::ArtifactAlreadyExists { path: target_path });
        }
        self.write_json_value_atomic(job_id, relative_path, value)
    }

    fn write_json_value_atomic(
        &self,
        job_id: &str,
        relative_path: &str,
        value: &Value,
    ) -> Result<(), StateStoreError> {
        let target_path = self.resolve_job_path(job_id, relative_path)?;
        let mut bytes =
            serde_json::to_vec_pretty(value).map_err(|source| StateStoreError::InvalidJson {
                path: target_path.clone(),
                source,
            })?;
        bytes.push(b'\n');
        self.write_bytes_atomic(job_id, &target_path, &bytes)
    }

    fn write_new_text_artifact(
        &self,
        job_id: &str,
        relative_path: &str,
        content: &str,
    ) -> Result<(), StateStoreError> {
        let target_path = self.resolve_job_path(job_id, relative_path)?;
        if target_path.exists() {
            return Err(StateStoreError::ArtifactAlreadyExists { path: target_path });
        }
        self.write_bytes_atomic(job_id, &target_path, content.as_bytes())
    }

    fn write_bytes_atomic(
        &self,
        job_id: &str,
        target_path: &Path,
        bytes: &[u8],
    ) -> Result<(), StateStoreError> {
        let job_dir = self.job_dir(job_id)?;
        let tmp_dir = job_dir.join("tmp");
        fs::create_dir_all(&tmp_dir).map_err(|source| StateStoreError::AtomicWriteFailed {
            path: tmp_dir.clone(),
            source,
        })?;
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|source| StateStoreError::AtomicWriteFailed {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let tmp_name = format!(
            "{}.tmp-{}-{}",
            target_path
                .file_name()
                .and_then(OsStr::to_str)
                .unwrap_or("artifact.json"),
            std::process::id(),
            timestamp_nanos()
        );
        let tmp_path = tmp_dir.join(tmp_name);
        {
            let mut file =
                File::create(&tmp_path).map_err(|source| StateStoreError::AtomicWriteFailed {
                    path: tmp_path.clone(),
                    source,
                })?;
            file.write_all(bytes)
                .and_then(|_| file.flush())
                .and_then(|_| file.sync_all())
                .map_err(|source| StateStoreError::AtomicWriteFailed {
                    path: tmp_path.clone(),
                    source,
                })?;
        }

        replace_file(&tmp_path, target_path).map_err(|source| StateStoreError::AtomicWriteFailed {
            path: target_path.to_path_buf(),
            source,
        })
    }

    fn validate_artifact(
        &self,
        schema: CoreSchema,
        artifact_path: PathBuf,
        value: &Value,
    ) -> Result<(), StateStoreError> {
        let schema_path = self.schema_root.join(schema.file_name());
        let schema =
            load_schema(&schema_path).map_err(|source| StateStoreError::SchemaLoadFailed {
                path: schema_path,
                message: source.to_string(),
            })?;
        let result = validate_json(value, &schema);
        if result.is_ok() {
            Ok(())
        } else {
            Err(StateStoreError::SchemaValidationFailed {
                path: artifact_path,
                errors: result.errors,
            })
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum CoreSchema {
    Job,
    RunState,
    Route,
    WorkSpec,
    Report,
    Event,
    ArtifactRef,
}

impl CoreSchema {
    fn file_name(self) -> &'static str {
        match self {
            Self::Job => "job.schema.json",
            Self::RunState => "run-state.schema.json",
            Self::Route => "route.schema.json",
            Self::WorkSpec => "workspec.schema.json",
            Self::Report => "report.schema.json",
            Self::Event => "event.schema.json",
            Self::ArtifactRef => "artifact-ref.schema.json",
        }
    }
}

fn ensure_standard_dirs(job_dir: &Path) -> Result<(), StateStoreError> {
    for name in [
        "workspecs",
        "reports",
        "provider-output",
        "tool-output",
        "approvals",
        "review-packs",
        "validation",
        "tmp",
    ] {
        let path = job_dir.join(name);
        fs::create_dir_all(&path)
            .map_err(|source| StateStoreError::AiRunsNotWritable { path, source })?;
    }
    Ok(())
}

fn recovery_issue_from_error(relative_path: &str, error: &StateStoreError) -> RecoveryIssue {
    match error {
        StateStoreError::ArtifactNotFound { .. } => RecoveryIssue::new(
            relative_path,
            "missing_required_file",
            "block",
            "required artifact is missing",
            "inspect the job and recreate only through an explicit recovery command",
        ),
        StateStoreError::InvalidJson { .. } => RecoveryIssue::new(
            relative_path,
            "invalid_json",
            "block",
            "artifact is not valid JSON",
            "preserve the original artifact and prepare a replacement through an explicit recovery command",
        ),
        StateStoreError::SchemaValidationFailed { errors, .. } => RecoveryIssue::new(
            relative_path,
            "schema_mismatch",
            "block",
            format!("artifact failed schema validation with {} error(s)", errors.len()),
            "inspect schema errors and write a corrected artifact only through an explicit recovery command",
        ),
        StateStoreError::CorruptEventLog { line, .. } => RecoveryIssue::new(
            relative_path,
            "corrupt_event_log",
            "block",
            format!("event log contains an invalid line at {}", line),
            "preserve the original log and create a recovered copy before replacing anything",
        ),
        StateStoreError::PathTraversalBlocked { .. }
        | StateStoreError::PathOutsideJobDirectory { .. } => RecoveryIssue::new(
            relative_path,
            "path_violation",
            "block",
            "artifact path violates job directory containment",
            "reject the recovery input and inspect the caller-provided path",
        ),
        _ => RecoveryIssue::new(
            relative_path,
            "inspection_failed",
            "block",
            "artifact inspection failed",
            "inspect the job manually before attempting recovery",
        ),
    }
}

fn collect_tmp_file_issues(
    directory: &Path,
    relative_dir: &str,
    issues: &mut Vec<RecoveryIssue>,
) -> Result<(), StateStoreError> {
    for entry in fs::read_dir(directory).map_err(|source| StateStoreError::AiRunsNotWritable {
        path: directory.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| StateStoreError::AiRunsNotWritable {
            path: directory.to_path_buf(),
            source,
        })?;
        let name = entry.file_name().to_string_lossy().to_string();
        let relative_path = format!("{}/{}", relative_dir, name);
        let file_type = entry
            .file_type()
            .map_err(|source| StateStoreError::AiRunsNotWritable {
                path: entry.path(),
                source,
            })?;
        if file_type.is_dir() {
            collect_tmp_file_issues(&entry.path(), &relative_path, issues)?;
        } else if file_type.is_file() {
            issues.push(RecoveryIssue::new(
                relative_path,
                "partial_tmp_file",
                "warn",
                "tmp file is not a canonical artifact",
                "leave the tmp file untouched until an explicit discard-tmp recovery command is approved",
            ));
        }
    }
    Ok(())
}

fn resolve_inside_job(job_dir: &Path, relative_path: &str) -> Result<PathBuf, StateStoreError> {
    let (normalized, _) = normalize_relative_path(relative_path)?;
    let resolved = job_dir.join(normalized);
    if !resolved.starts_with(job_dir) {
        return Err(StateStoreError::PathOutsideJobDirectory { path: resolved });
    }
    Ok(resolved)
}

fn normalized_relative_path(relative_path: &str) -> Result<String, StateStoreError> {
    let (_, normalized) = normalize_relative_path(relative_path)?;
    Ok(normalized)
}

fn normalize_relative_path(relative_path: &str) -> Result<(PathBuf, String), StateStoreError> {
    if relative_path.is_empty()
        || relative_path.contains('\0')
        || relative_path.contains(':')
        || Path::new(relative_path).is_absolute()
    {
        return Err(StateStoreError::PathTraversalBlocked {
            path: relative_path.to_string(),
        });
    }

    let mut normalized = PathBuf::new();
    let mut normalized_segments = Vec::new();
    for component in Path::new(relative_path).components() {
        match component {
            Component::Normal(segment) if segment == ".git" => {
                return Err(StateStoreError::PathTraversalBlocked {
                    path: relative_path.to_string(),
                });
            }
            Component::Normal(segment) => {
                normalized.push(segment);
                normalized_segments.push(segment.to_string_lossy().to_string());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(StateStoreError::PathTraversalBlocked {
                    path: relative_path.to_string(),
                });
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(StateStoreError::PathTraversalBlocked {
            path: relative_path.to_string(),
        });
    }

    Ok((normalized, normalized_segments.join("/")))
}

fn ensure_artifact_job_id(value: &Value, expected: &str) -> Result<(), StateStoreError> {
    validate_job_id(expected)?;
    let actual = value.get("job_id").and_then(Value::as_str).ok_or_else(|| {
        StateStoreError::JobIdMismatch {
            expected: expected.to_string(),
            actual: "<missing>".to_string(),
        }
    })?;
    if actual == expected {
        Ok(())
    } else {
        Err(StateStoreError::JobIdMismatch {
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    }
}

fn validate_job_id(job_id: &str) -> Result<(), StateStoreError> {
    if parse_job_number(job_id).is_some() {
        Ok(())
    } else {
        Err(StateStoreError::InvalidJobId {
            job_id: job_id.to_string(),
        })
    }
}

fn parse_job_number(job_id: &str) -> Option<u64> {
    let suffix = job_id.strip_prefix("J-")?;
    if suffix.len() < 4 || !suffix.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    suffix.parse().ok()
}

fn validate_stage(stage: &str) -> Result<(), StateStoreError> {
    if CANONICAL_STAGES.contains(&stage) {
        Ok(())
    } else {
        Err(StateStoreError::InvalidStage {
            stage: stage.to_string(),
        })
    }
}

fn validate_safe_name(name: &str) -> Result<(), StateStoreError> {
    if name.is_empty()
        || name.contains('\0')
        || name.contains(':')
        || name.contains('/')
        || name.contains('\\')
        || name == "."
        || name == ".."
        || name == ".git"
    {
        return Err(StateStoreError::PathTraversalBlocked {
            path: name.to_string(),
        });
    }
    Ok(())
}

fn timestamp_string() -> String {
    format!("unix:{}", timestamp_nanos())
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
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_PROJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn schema_root() -> PathBuf {
        repo_root().join("specs/schemas")
    }

    fn temp_project() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let counter = TEMP_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "star-control-state-{}-{}-{}",
            std::process::id(),
            nanos,
            counter
        ));
        fs::create_dir_all(&path).expect("create temp project");
        path
    }

    fn open_store(project_root: &Path) -> StateStore {
        StateStore::open(project_root, schema_root()).expect("open state store")
    }

    fn create_job(store: &StateStore) -> Value {
        store
            .create_job("implement feature", "codex", vec!["no deploy".to_string()])
            .expect("create job")
    }

    fn state(job_id: &str, state: &str) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": job_id,
            "state": state,
            "current_stage": "route",
            "updated_at": "2026-07-01T00:00:00Z",
            "threads": {},
            "workers": {},
            "artifacts": {},
            "latest_event_id": "EV-0001",
            "active_provider": null,
            "next_action": "continue",
            "budget": {},
            "history": []
        })
    }

    fn event(job_id: &str, event_id: &str, message: &str) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "event_id": event_id,
            "job_id": job_id,
            "type": "STATE_CHANGED",
            "created_at": "2026-07-01T00:00:00Z",
            "stage": "route",
            "state": "ROUTING",
            "message": message,
            "artifact_paths": ["run-state.json"],
            "details": {}
        })
    }

    fn read_example(relative_path: &str) -> Value {
        let path = repo_root().join(relative_path);
        serde_json::from_str(&fs::read_to_string(path).expect("read example"))
            .expect("parse example")
    }

    #[test]
    fn creates_ai_runs_and_first_job_directory() {
        let project = temp_project();
        let store = open_store(&project);
        let job = create_job(&store);

        assert_eq!(job["job_id"], "J-0001");
        assert!(project.join(".ai-runs/J-0001/job.json").is_file());
        assert!(project.join(".ai-runs/J-0001/workspecs").is_dir());
        assert!(project.join(".ai-runs/J-0001/tmp").is_dir());

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn allocates_next_job_id_from_existing_jobs() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store);
        let second = create_job(&store);

        assert_eq!(second["job_id"], "J-0002");

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn job_and_state_roundtrip_with_schema_validation() {
        let project = temp_project();
        let store = open_store(&project);
        let job = create_job(&store);
        let job_id = job["job_id"].as_str().unwrap();

        assert_eq!(store.load_job(job_id).expect("load job"), job);

        let state = state(job_id, "REQUESTED");
        store.save_state(job_id, &state).expect("save state");
        assert_eq!(store.load_state(job_id).expect("load state"), state);

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn appends_events_in_order() {
        let project = temp_project();
        let store = open_store(&project);
        let job = create_job(&store);
        let job_id = job["job_id"].as_str().unwrap();

        store
            .append_event(job_id, &event(job_id, "EV-0002", "second"))
            .expect("append second event");
        store
            .append_event(job_id, &event(job_id, "EV-0003", "third"))
            .expect("append third event");
        let events = store.read_events(job_id).expect("read events");

        assert_eq!(events.len(), 3);
        assert_eq!(events[0]["type"], "JOB_CREATED");
        assert_eq!(events[1]["event_id"], "EV-0002");
        assert_eq!(events[2]["event_id"], "EV-0003");

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn route_workspec_and_report_roundtrip() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store);

        let route = read_example("examples/runs/J-0001/route.json");
        let workspec = read_example("examples/runs/J-0001/workspecs/implement.json");
        let report = read_example("examples/fake/impl-report-done.json");

        store.save_route("J-0001", &route).expect("save route");
        store
            .save_workspec("J-0001", "implement", &workspec)
            .expect("save workspec");
        store
            .save_report("J-0001", "implement-report", &report)
            .expect("save report");

        assert_eq!(store.load_route("J-0001").expect("load route"), route);
        assert_eq!(
            store
                .load_workspec("J-0001", "implement")
                .expect("load workspec"),
            workspec
        );
        assert_eq!(
            store
                .load_report("J-0001", "implement-report")
                .expect("load report"),
            report
        );

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn reports_missing_job_and_invalid_json() {
        let project = temp_project();
        let store = open_store(&project);
        assert!(matches!(
            store.load_job("J-9999"),
            Err(StateStoreError::JobNotFound { .. })
        ));

        create_job(&store);
        fs::write(
            project.join(".ai-runs/J-0001/run-state.json"),
            "{ invalid json",
        )
        .expect("write invalid state");

        assert!(matches!(
            store.load_state("J-0001"),
            Err(StateStoreError::InvalidJson { .. })
        ));

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn blocks_path_traversal_absolute_paths_and_git_paths() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store);

        assert!(matches!(
            store.resolve_job_path("J-0001", "../outside.json"),
            Err(StateStoreError::PathTraversalBlocked { .. })
        ));
        assert!(matches!(
            store.resolve_job_path("J-0001", "C:\\temp\\file.json"),
            Err(StateStoreError::PathTraversalBlocked { .. })
        ));
        assert!(matches!(
            store.resolve_job_path("J-0001", ".git/config"),
            Err(StateStoreError::PathTraversalBlocked { .. })
        ));

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn terminal_state_blocks_resume_but_preserves_state() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store);

        let done = state("J-0001", "DONE");
        store
            .save_state("J-0001", &done)
            .expect("save terminal state");

        assert!(matches!(
            store.ensure_resume_allowed("J-0001"),
            Err(StateStoreError::TerminalStateBlocked { .. })
        ));
        assert_eq!(store.load_state("J-0001").expect("load terminal"), done);

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn list_jobs_includes_corrupt_jobs() {
        let project = temp_project();
        let store = open_store(&project);
        let corrupt_dir = project.join(".ai-runs/J-0001");
        fs::create_dir_all(&corrupt_dir).expect("create corrupt job");
        fs::write(corrupt_dir.join("job.json"), "{ invalid json").expect("write corrupt job");

        let jobs = store.list_jobs().expect("list jobs");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_id, "J-0001");
        assert!(jobs[0].corrupt);

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn tmp_files_are_not_read_as_artifacts() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store);
        fs::write(
            project.join(".ai-runs/J-0001/tmp/run-state.json.tmp-test"),
            "{ invalid json",
        )
        .expect("write tmp file");

        assert!(matches!(
            store.load_state("J-0001"),
            Err(StateStoreError::ArtifactNotFound { .. })
        ));

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn recovery_inspection_reports_ok_for_complete_job() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store);
        store
            .save_state("J-0001", &state("J-0001", "REQUESTED"))
            .expect("save state");

        let inspection = store.inspect_recovery("J-0001").expect("inspect recovery");
        assert_eq!(inspection.status, "ok");
        assert_eq!(inspection.mode, "inspect_only");
        assert!(!inspection.manual_followup_required);
        assert!(!inspection.destructive_actions_performed);
        assert!(inspection.issues.is_empty());
        assert_eq!(inspection.to_value()["status"], "ok");

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn recovery_inspection_reports_missing_required_files_and_tmp_without_mutation() {
        let project = temp_project();
        let store = open_store(&project);
        let corrupt_dir = project.join(".ai-runs/J-0001");
        fs::create_dir_all(corrupt_dir.join("tmp/nested")).expect("create corrupt job dirs");
        let tmp_file = corrupt_dir.join("tmp/nested/run-state.json.tmp-test");
        fs::write(&tmp_file, "{ partial json").expect("write tmp file");

        let inspection = store
            .inspect_recovery("J-0001")
            .expect("inspect corrupt job");
        let kinds: Vec<_> = inspection
            .issues
            .iter()
            .map(|issue| issue.kind.as_str())
            .collect();

        assert_eq!(inspection.status, "needs_recovery");
        assert!(inspection.manual_followup_required);
        assert!(!inspection.destructive_actions_performed);
        assert!(kinds.contains(&"missing_required_file"));
        assert!(kinds.contains(&"partial_tmp_file"));
        assert!(inspection
            .issues
            .iter()
            .any(|issue| issue.artifact_path == "tmp/nested/run-state.json.tmp-test"));
        assert!(tmp_file.is_file(), "inspection must not delete tmp files");

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn recovery_inspection_reports_invalid_state_and_corrupt_event_log() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store);
        fs::write(
            project.join(".ai-runs/J-0001/run-state.json"),
            "{ invalid json",
        )
        .expect("write invalid state");
        let mut events = OpenOptions::new()
            .append(true)
            .open(project.join(".ai-runs/J-0001/events.jsonl"))
            .expect("open events");
        writeln!(events, "{{ invalid event").expect("append corrupt event");

        let inspection = store
            .inspect_recovery("J-0001")
            .expect("inspect corrupt artifacts");
        assert!(inspection
            .issues
            .iter()
            .any(|issue| issue.artifact_path == "run-state.json" && issue.kind == "invalid_json"));
        assert!(inspection.issues.iter().any(
            |issue| issue.artifact_path == "events.jsonl" && issue.kind == "corrupt_event_log"
        ));
        assert_eq!(
            inspection.to_value()["destructive_actions_performed"],
            false
        );

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn recovery_inspection_rejects_unsafe_job_id() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store);

        assert!(matches!(
            store.inspect_recovery("../J-0001"),
            Err(StateStoreError::InvalidJobId { .. })
        ));

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn resolves_provider_and_tool_output_dirs_inside_job() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store);

        assert!(store
            .resolve_provider_output_dir("J-0001", "fake-default")
            .expect("provider output dir")
            .ends_with("provider-output/fake-default"));
        assert!(store
            .resolve_tool_output_dir("J-0001", "star-sentinel")
            .expect("tool output dir")
            .ends_with("tool-output/star-sentinel"));

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn writes_output_artifacts_and_artifact_refs_inside_job() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store);

        let provider_ref = store
            .write_provider_json(
                "J-0001",
                "fake-default",
                "request.json",
                &json!({ "goal": "test" }),
            )
            .expect("write provider json");
        let stdout_ref = store
            .write_provider_text("J-0001", "fake-default", "stdout.txt", "ok\n")
            .expect("write provider stdout");
        let tool_ref = store
            .write_tool_json("J-0001", "star-sentinel", "diagnostics.json", &json!([]))
            .expect("write tool json");
        let tool_markdown_ref = store
            .write_tool_text("J-0001", "star-sentinel", "review_pack.md", "# Review\n")
            .expect("write tool markdown");
        let approval_ref = store
            .write_approval_json("J-0001", "approval-request.json", &json!({ "ok": true }))
            .expect("write approval");
        let review_json_ref = store
            .write_review_pack_json("J-0001", "review_pack.json", &json!({ "items": [] }))
            .expect("write review json");
        let review_md_ref = store
            .write_review_pack_markdown("J-0001", "review_pack.md", "# Review\n")
            .expect("write review markdown");
        let validation_ref = store
            .write_validation_json(
                "J-0001",
                "validation-decision.json",
                &json!({ "decision": "AUTO_PASS" }),
            )
            .expect("write validation json");
        let tmp_path = store
            .write_tmp_json("J-0001", "run-state.json", &json!({ "tmp": true }))
            .expect("write tmp json");

        assert_eq!(
            provider_ref["path"],
            "provider-output/fake-default/request.json"
        );
        assert_eq!(provider_ref["kind"], "provider_output");
        assert_eq!(stdout_ref["kind"], "log");
        assert_eq!(
            tool_ref["path"],
            "tool-output/star-sentinel/diagnostics.json"
        );
        assert_eq!(
            tool_markdown_ref["path"],
            "tool-output/star-sentinel/review_pack.md"
        );
        assert_eq!(approval_ref["kind"], "approval");
        assert_eq!(review_json_ref["kind"], "review_pack");
        assert_eq!(review_md_ref["path"], "review-packs/review_pack.md");
        assert_eq!(
            validation_ref["path"],
            "validation/validation-decision.json"
        );
        assert_eq!(validation_ref["kind"], "other");
        assert!(tmp_path.starts_with("tmp/run-state.json.tmp-"));
        assert!(project
            .join(".ai-runs/J-0001/provider-output/fake-default/request.json")
            .is_file());
        assert!(project
            .join(".ai-runs/J-0001/tool-output/star-sentinel/diagnostics.json")
            .is_file());
        assert!(project
            .join(".ai-runs/J-0001/tool-output/star-sentinel/review_pack.md")
            .is_file());
        assert!(project
            .join(".ai-runs/J-0001/approvals/approval-request.json")
            .is_file());
        assert!(project
            .join(".ai-runs/J-0001/review-packs/review_pack.md")
            .is_file());
        assert!(project
            .join(".ai-runs/J-0001/validation/validation-decision.json")
            .is_file());

        assert!(matches!(
            store.write_provider_json(
                "J-0001",
                "fake-default",
                "request.json",
                &json!({ "goal": "overwrite" }),
            ),
            Err(StateStoreError::ArtifactAlreadyExists { .. })
        ));

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn registers_artifact_ref_in_run_state() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store);

        let mut state = state("J-0001", "REQUESTED");
        let route_ref = store
            .artifact_ref(
                "J-0001",
                "route.json",
                ArtifactKind::Route,
                "router",
                Some("specs/schemas/route.schema.json"),
                Some("RouteSpec artifact"),
            )
            .expect("artifact ref");
        store
            .register_artifact_ref(&mut state, "route", &route_ref)
            .expect("register artifact ref");
        store.save_state("J-0001", &state).expect("save state");

        let loaded = store.load_state("J-0001").expect("load state");
        assert_eq!(loaded["artifacts"]["route"]["path"], "route.json");
        assert_eq!(loaded["artifacts"]["route"]["kind"], "route");

        fs::remove_dir_all(project).ok();
    }

    #[test]
    fn artifact_writers_reject_unsafe_names() {
        let project = temp_project();
        let store = open_store(&project);
        create_job(&store);

        assert!(matches!(
            store.write_provider_json("J-0001", "../fake", "request.json", &json!({})),
            Err(StateStoreError::PathTraversalBlocked { .. })
        ));
        assert!(matches!(
            store.write_tool_json("J-0001", "star-sentinel", "../diagnostics.json", &json!({})),
            Err(StateStoreError::PathTraversalBlocked { .. })
        ));
        assert!(matches!(
            store.artifact_ref(
                "J-0001",
                "/absolute/path.json",
                ArtifactKind::Other,
                "test",
                None,
                None,
            ),
            Err(StateStoreError::PathTraversalBlocked { .. })
        ));

        fs::remove_dir_all(project).ok();
    }
}
