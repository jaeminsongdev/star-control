//! Fail-closed Codex App Server JSONL adapter.
//!
//! The adapter never infers an operation from a package version. A caller must
//! first generate and retain the version-specific JSON Schema bundle; only
//! exact method and field names observed in that bundle can become capability
//! facts.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    env, fs,
    io::{BufRead, BufReader, Read, Write},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, ExitStatus, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use star_contracts::{
    CapabilitySnapshotId, Sha256Hash, canonical_sha256,
    evidence::ArtifactRef,
    parse_no_duplicate_keys,
    routing::{
        CAPABILITY_SNAPSHOT_SCHEMA_ID, CapabilitySnapshotV1, CapabilitySourceV1,
        CodexPermissionCapabilitiesV1, ExecutionModeV1, ModelCapabilityV1,
        ROUTING_CONTRACT_VERSION, ReasoningEffortV1,
    },
};
use thiserror::Error;
use windows::Win32::System::Threading::CREATE_NO_WINDOW;

const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
const MAX_SCHEMA_FILES: usize = 2_048;
const MAX_SCHEMA_BYTES: usize = 128 * 1024 * 1024;
const MAX_MODEL_PAGES: usize = 64;
const MAX_MODELS: usize = 1_024;
const PROBE_PROCESS_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_VERSION_BYTES: usize = 4_096;
const DEFAULT_CODEX_ENVIRONMENT: &[&str] = &[
    "SYSTEMROOT",
    "WINDIR",
    "COMSPEC",
    "PATH",
    "PATHEXT",
    "TEMP",
    "TMP",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
];

fn apply_allowed_environment(
    command: &mut Command,
    allowed_environment_names: &[String],
) -> Result<(), CodexAppServerError> {
    if allowed_environment_names.len() > 128
        || allowed_environment_names.iter().any(|name| {
            name.is_empty()
                || name.len() > 128
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        })
    {
        return Err(CodexAppServerError::Protocol);
    }
    let mut normalized_names = allowed_environment_names
        .iter()
        .map(|name| name.to_ascii_uppercase())
        .collect::<Vec<_>>();
    normalized_names.sort();
    normalized_names.dedup();
    command.env_clear();
    for name in normalized_names {
        if let Some(value) = env::var_os(&name) {
            command.env(name, value);
        }
    }
    Ok(())
}

pub fn default_codex_environment_names() -> Vec<String> {
    DEFAULT_CODEX_ENVIRONMENT
        .iter()
        .map(|name| (*name).to_owned())
        .collect()
}

const OBSERVED_METHODS: &[(&str, &str)] = &[
    ("initialize", "initialize"),
    ("model_list", "model/list"),
    (
        "model_provider_capabilities_read",
        "modelProvider/capabilities/read",
    ),
    ("thread_start", "thread/start"),
    ("thread_resume", "thread/resume"),
    ("thread_fork", "thread/fork"),
    ("turn_start", "turn/start"),
    ("turn_steer", "turn/steer"),
    ("turn_interrupt", "turn/interrupt"),
];

const OBSERVED_FIELDS: &[&str] = &[
    "approvalPolicy",
    "sandbox",
    "sandboxPolicy",
    "networkAccess",
    "serviceTier",
];

#[derive(Debug, Error)]
pub enum CodexAppServerError {
    #[error("Codex executable or protocol bundle path is invalid")]
    Path,
    #[error("Codex App Server I/O failed")]
    Io,
    #[error("Codex App Server protocol frame is invalid")]
    Protocol,
    #[error("Codex App Server request timed out")]
    Timeout,
    #[error("Codex App Server rejected a request with code {0}")]
    Remote(i64),
    #[error("Codex App Server initiated an unsupported request")]
    UnsupportedServerRequest,
    #[error("Codex App Server advertised an unsupported reasoning effort")]
    UnsupportedReasoningEffort,
    #[error("Codex execution mode is not implemented by the current Controller")]
    UnsupportedExecutionMode,
    #[error("Codex App Server capability evidence does not match its artifact")]
    Evidence,
    #[error("Codex App Server process exited")]
    Exited,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolSchemaObservationV1 {
    pub schema_fingerprint: Sha256Hash,
    pub file_count: u32,
    pub total_bytes: u64,
    pub methods: BTreeSet<String>,
    pub fields: BTreeSet<String>,
}

impl ProtocolSchemaObservationV1 {
    pub fn operation_map(&self) -> BTreeMap<String, bool> {
        OBSERVED_METHODS
            .iter()
            .map(|(operation, method)| ((*operation).to_owned(), self.methods.contains(*method)))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitializeObservationV1 {
    pub user_agent: String,
    pub platform_family: String,
    pub platform_os: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderCapabilityObservationV1 {
    pub namespace_tools: bool,
    pub image_generation: bool,
    pub web_search: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityProbeEvidenceV1 {
    pub codex_version: String,
    pub captured_at: DateTime<Utc>,
    pub protocol_schema: ProtocolSchemaObservationV1,
    pub initialize: InitializeObservationV1,
    pub models: Vec<ModelCapabilityV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderCapabilityObservationV1>,
    pub limitations: Vec<String>,
}

impl CapabilityProbeEvidenceV1 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CodexAppServerError> {
        // LocalArtifactStore persists JSON using this exact deterministic
        // representation; bind the snapshot to the bytes that are actually
        // retained rather than to a different compact serialization.
        serde_json::to_vec_pretty(self).map_err(|_| CodexAppServerError::Protocol)
    }

    pub fn into_snapshot(
        self,
        expires_at: DateTime<Utc>,
        raw_artifact_ref: ArtifactRef,
        allow_managed_ultra: bool,
    ) -> Result<CapabilitySnapshotV1, CodexAppServerError> {
        if Sha256Hash::digest(&self.canonical_bytes()?) != raw_artifact_ref.sha256 {
            return Err(CodexAppServerError::Evidence);
        }
        let operations = self.protocol_schema.operation_map();
        let permission_capabilities = CodexPermissionCapabilitiesV1 {
            approval_policy_configurable: self.protocol_schema.fields.contains("approvalPolicy"),
            sandbox_mode_configurable: self.protocol_schema.fields.contains("sandbox")
                || self.protocol_schema.fields.contains("sandboxPolicy"),
            network_policy_observable: self.protocol_schema.fields.contains("networkAccess"),
            // A service tier is not a verified price or paid-effect receipt.
            paid_action_observable: false,
        };
        let permission_overrides_supported = permission_capabilities.approval_policy_configurable
            && permission_capabilities.sandbox_mode_configurable;
        let (native_execution_modes, managed_execution_modes) = verified_execution_modes(
            &operations,
            permission_overrides_supported,
            allow_managed_ultra,
        )?;
        let mut limitations = self.limitations;
        if !permission_overrides_supported {
            limitations.push("controller_permission_overrides_unavailable".to_owned());
        }
        CapabilitySnapshotV1 {
            schema_id: CAPABILITY_SNAPSHOT_SCHEMA_ID.to_owned(),
            schema_version: ROUTING_CONTRACT_VERSION,
            capability_snapshot_id: CapabilitySnapshotId::new(),
            source: CapabilitySourceV1::CodexAppServer,
            captured_at: self.captured_at,
            expires_at,
            codex_version: Some(self.codex_version),
            protocol_version: "app-server-jsonrpc-v2".to_owned(),
            protocol_schema_fingerprint: self.protocol_schema.schema_fingerprint,
            models: self.models,
            operations,
            native_execution_modes,
            managed_execution_modes,
            permission_capabilities,
            limits: BTreeMap::from([
                ("max_model_pages".to_owned(), MAX_MODEL_PAGES as u32),
                ("max_models".to_owned(), MAX_MODELS as u32),
            ]),
            limitations,
            raw_artifact_ref,
            snapshot_fingerprint: Sha256Hash::digest(b"unsealed-capability-snapshot"),
        }
        .seal()
        .map_err(|_| CodexAppServerError::Evidence)
    }
}

fn verified_execution_modes(
    operations: &BTreeMap<String, bool>,
    permission_overrides_supported: bool,
    allow_managed_ultra: bool,
) -> Result<(Vec<ExecutionModeV1>, Vec<ExecutionModeV1>), CodexAppServerError> {
    // Reasoning effort is model metadata, not an execution-mode capability.
    // The current A06 executor owns one App Server thread/turn at a time. Until
    // a durable fan-out/integration handler exists, advertising managed Ultra
    // would create a route that the owning executor must reject later.
    if allow_managed_ultra {
        return Err(CodexAppServerError::UnsupportedExecutionMode);
    }
    let native = if permission_overrides_supported
        && operations.get("thread_start") == Some(&true)
        && operations.get("turn_start") == Some(&true)
    {
        vec![ExecutionModeV1::Single]
    } else {
        Vec::new()
    };
    Ok((native, Vec::new()))
}

/// Generates a version-specific protocol bundle into a caller-owned, empty
/// evidence directory. Existing files are never overwritten or removed.
pub fn generate_protocol_schema_bundle(
    codex_executable: &Path,
    output_directory: &Path,
) -> Result<(), CodexAppServerError> {
    if !codex_executable.is_absolute()
        || !codex_executable.is_file()
        || !output_directory.is_absolute()
        || output_directory.exists()
    {
        return Err(CodexAppServerError::Path);
    }
    let mut command = Command::new(codex_executable);
    command
        .args(["app-server", "generate-json-schema", "--out"])
        .arg(output_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command.creation_flags(CREATE_NO_WINDOW.0);
    apply_allowed_environment(&mut command, &default_codex_environment_names())?;
    let mut child = command.spawn().map_err(|_| CodexAppServerError::Io)?;
    let status = wait_child_bounded(&mut child, PROBE_PROCESS_TIMEOUT)?;
    if !status.success() {
        return Err(CodexAppServerError::Exited);
    }
    Ok(())
}

pub fn inspect_protocol_schema_bundle(
    root: &Path,
) -> Result<ProtocolSchemaObservationV1, CodexAppServerError> {
    if !root.is_absolute() || !root.is_dir() {
        return Err(CodexAppServerError::Path);
    }
    let canonical_root = fs::canonicalize(root).map_err(|_| CodexAppServerError::Path)?;
    let mut pending = vec![canonical_root.clone()];
    let mut documents = Vec::<(String, Vec<u8>)>::new();
    let mut total_bytes = 0_usize;
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory).map_err(|_| CodexAppServerError::Io)?;
        for entry in entries {
            let entry = entry.map_err(|_| CodexAppServerError::Io)?;
            let file_type = entry.file_type().map_err(|_| CodexAppServerError::Io)?;
            if file_type.is_symlink() {
                return Err(CodexAppServerError::Path);
            }
            let path = entry.path();
            if file_type.is_dir() {
                pending.push(path);
                continue;
            }
            if !file_type.is_file()
                || path.extension().and_then(|value| value.to_str()) != Some("json")
            {
                continue;
            }
            if documents.len() >= MAX_SCHEMA_FILES {
                return Err(CodexAppServerError::Protocol);
            }
            let bytes = fs::read(&path).map_err(|_| CodexAppServerError::Io)?;
            total_bytes = total_bytes
                .checked_add(bytes.len())
                .ok_or(CodexAppServerError::Protocol)?;
            if total_bytes > MAX_SCHEMA_BYTES {
                return Err(CodexAppServerError::Protocol);
            }
            let canonical_path = fs::canonicalize(&path).map_err(|_| CodexAppServerError::Io)?;
            if !canonical_path.starts_with(&canonical_root) {
                return Err(CodexAppServerError::Path);
            }
            let relative = canonical_path
                .strip_prefix(&canonical_root)
                .map_err(|_| CodexAppServerError::Path)?
                .to_string_lossy()
                .replace('\\', "/");
            documents.push((relative, bytes));
        }
    }
    observe_schema_documents(documents)
}

fn observe_schema_documents(
    mut documents: Vec<(String, Vec<u8>)>,
) -> Result<ProtocolSchemaObservationV1, CodexAppServerError> {
    if documents.is_empty() {
        return Err(CodexAppServerError::Protocol);
    }
    documents.sort_by(|left, right| left.0.cmp(&right.0));
    let mut methods = BTreeSet::new();
    let mut fields = BTreeSet::new();
    let mut fingerprint_records = Vec::with_capacity(documents.len());
    let mut total_bytes = 0_u64;
    for (relative, bytes) in &documents {
        let text = std::str::from_utf8(bytes).map_err(|_| CodexAppServerError::Protocol)?;
        let value = parse_no_duplicate_keys(text).map_err(|_| CodexAppServerError::Protocol)?;
        collect_observed_schema_tokens(&value, &mut methods, &mut fields);
        total_bytes = total_bytes
            .checked_add(u64::try_from(bytes.len()).map_err(|_| CodexAppServerError::Protocol)?)
            .ok_or(CodexAppServerError::Protocol)?;
        fingerprint_records.push(json!({
            "relative_path": relative,
            "sha256": Sha256Hash::digest(bytes),
            "size": bytes.len(),
        }));
    }
    let schema_fingerprint = canonical_sha256(&json!({
        "domain":"codex-app-server-json-schema-bundle",
        "version":1,
        "files":fingerprint_records,
    }))
    .map_err(|_| CodexAppServerError::Protocol)?;
    Ok(ProtocolSchemaObservationV1 {
        schema_fingerprint,
        file_count: u32::try_from(documents.len()).map_err(|_| CodexAppServerError::Protocol)?,
        total_bytes,
        methods,
        fields,
    })
}

fn collect_observed_schema_tokens(
    value: &Value,
    methods: &mut BTreeSet<String>,
    fields: &mut BTreeSet<String>,
) {
    match value {
        Value::String(_) => {}
        Value::Array(values) => {
            for value in values {
                collect_observed_schema_tokens(value, methods, fields);
            }
        }
        Value::Object(values) => {
            if let Some(Value::Object(properties)) = values.get("properties") {
                for field in OBSERVED_FIELDS {
                    if properties.contains_key(*field) {
                        fields.insert((*field).to_owned());
                    }
                }
                if let Some(method_schema) = properties.get("method") {
                    collect_schema_method_literals(method_schema, methods);
                }
            }
            for value in values.values() {
                collect_observed_schema_tokens(value, methods, fields);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn collect_schema_method_literals(value: &Value, methods: &mut BTreeSet<String>) {
    match value {
        Value::Object(values) => {
            if let Some(Value::String(constant)) = values.get("const") {
                record_observed_method(constant, methods);
            }
            if let Some(Value::Array(variants)) = values.get("enum") {
                for variant in variants.iter().filter_map(Value::as_str) {
                    record_observed_method(variant, methods);
                }
            }
            for (key, nested) in values {
                if !matches!(
                    key.as_str(),
                    "description" | "title" | "examples" | "default"
                ) {
                    collect_schema_method_literals(nested, methods);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_schema_method_literals(value, methods);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn record_observed_method(value: &str, methods: &mut BTreeSet<String>) {
    if OBSERVED_METHODS.iter().any(|(_, method)| value == *method) {
        methods.insert(value.to_owned());
    }
}

pub fn probe_codex_version(codex_executable: &Path) -> Result<String, CodexAppServerError> {
    probe_codex_version_with_environment(codex_executable, &default_codex_environment_names())
}

pub fn probe_codex_version_with_environment(
    codex_executable: &Path,
    allowed_environment_names: &[String],
) -> Result<String, CodexAppServerError> {
    if !codex_executable.is_absolute() || !codex_executable.is_file() {
        return Err(CodexAppServerError::Path);
    }
    let mut command = Command::new(codex_executable);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped());
    apply_allowed_environment(&mut command, allowed_environment_names)?;
    command.creation_flags(CREATE_NO_WINDOW.0);
    let mut child = command.spawn().map_err(|_| CodexAppServerError::Io)?;
    let stdout = child.stdout.take().ok_or(CodexAppServerError::Io)?;
    let reader = thread::spawn(move || read_version_output(stdout));
    let status = wait_child_bounded(&mut child, PROBE_PROCESS_TIMEOUT);
    let (stdout, overflow) = reader.join().map_err(|_| CodexAppServerError::Io)??;
    let status = status?;
    if !status.success() || overflow {
        return Err(CodexAppServerError::Exited);
    }
    let version = std::str::from_utf8(&stdout)
        .map_err(|_| CodexAppServerError::Protocol)?
        .trim()
        .to_owned();
    if version.is_empty() || version.chars().any(char::is_control) {
        return Err(CodexAppServerError::Protocol);
    }
    Ok(version)
}

fn wait_child_bounded(
    child: &mut Child,
    timeout: Duration,
) -> Result<ExitStatus, CodexAppServerError> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(CodexAppServerError::Io);
            }
        }
        if Instant::now() >= deadline {
            if let Ok(Some(status)) = child.try_wait() {
                return Ok(status);
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(CodexAppServerError::Timeout);
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn read_version_output(mut stdout: impl Read) -> Result<(Vec<u8>, bool), CodexAppServerError> {
    let mut retained = Vec::with_capacity(MAX_VERSION_BYTES);
    let mut overflow = false;
    let mut buffer = [0_u8; 4_096];
    loop {
        let read = stdout
            .read(&mut buffer)
            .map_err(|_| CodexAppServerError::Io)?;
        if read == 0 {
            break;
        }
        let remaining = MAX_VERSION_BYTES.saturating_sub(retained.len());
        retained.extend_from_slice(&buffer[..read.min(remaining)]);
        overflow |= read > remaining;
    }
    Ok((retained, overflow))
}

pub struct CodexAppServerProcess {
    child: Child,
    stdin: ChildStdin,
    receiver: Receiver<Result<Value, CodexAppServerError>>,
    next_request_id: u64,
    notifications: VecDeque<Value>,
}

fn insert_permission_overrides(
    params: &mut serde_json::Map<String, Value>,
    approval_policy: Option<&str>,
    sandbox_mode: Option<&str>,
    runtime_workspace_roots: &[PathBuf],
    writable_roots: Option<&[PathBuf]>,
    turn_shape: bool,
) -> Result<(), CodexAppServerError> {
    if runtime_workspace_roots.len() > 64
        || runtime_workspace_roots
            .iter()
            .any(|root| !root.is_absolute())
    {
        return Err(CodexAppServerError::Path);
    }
    if writable_roots
        .is_some_and(|roots| roots.len() > 64 || roots.iter().any(|root| !root.is_absolute()))
    {
        return Err(CodexAppServerError::Path);
    }
    if let Some(policy) = approval_policy {
        if policy != "never" {
            return Err(CodexAppServerError::Protocol);
        }
        params.insert(
            "approvalPolicy".to_owned(),
            Value::String(policy.to_owned()),
        );
    }
    if let Some(mode) = sandbox_mode {
        let wire_mode = match mode {
            "read-only" => "readOnly",
            "workspace-write" => "workspaceWrite",
            _ => return Err(CodexAppServerError::Protocol),
        };
        if turn_shape {
            let mut sandbox = serde_json::Map::from_iter([(
                "type".to_owned(),
                Value::String(wire_mode.to_owned()),
            )]);
            if mode == "workspace-write" {
                sandbox.insert("networkAccess".to_owned(), Value::Bool(false));
                sandbox.insert(
                    "writableRoots".to_owned(),
                    Value::Array(
                        writable_roots
                            .unwrap_or(runtime_workspace_roots)
                            .iter()
                            .map(|root| Value::String(root.to_string_lossy().into_owned()))
                            .collect(),
                    ),
                );
            }
            params.insert("sandboxPolicy".to_owned(), Value::Object(sandbox));
        } else {
            params.insert("sandbox".to_owned(), Value::String(wire_mode.to_owned()));
        }
    }
    if !runtime_workspace_roots.is_empty() {
        params.insert(
            "runtimeWorkspaceRoots".to_owned(),
            Value::Array(
                runtime_workspace_roots
                    .iter()
                    .map(|root| Value::String(root.to_string_lossy().into_owned()))
                    .collect(),
            ),
        );
    }
    Ok(())
}

impl CodexAppServerProcess {
    pub fn spawn(codex_executable: &Path) -> Result<Self, CodexAppServerError> {
        let environment = default_codex_environment_names();
        Self::spawn_with_environment(codex_executable, &environment)
    }

    pub fn spawn_with_environment(
        codex_executable: &Path,
        allowed_environment_names: &[String],
    ) -> Result<Self, CodexAppServerError> {
        if !codex_executable.is_absolute() || !codex_executable.is_file() {
            return Err(CodexAppServerError::Path);
        }
        let mut command = Command::new(codex_executable);
        command
            .args(["app-server", "--listen", "stdio://"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        apply_allowed_environment(&mut command, allowed_environment_names)?;
        command.creation_flags(CREATE_NO_WINDOW.0);
        let mut child = command.spawn().map_err(|_| CodexAppServerError::Io)?;
        let stdin = child.stdin.take().ok_or(CodexAppServerError::Io)?;
        let stdout = child.stdout.take().ok_or(CodexAppServerError::Io)?;
        let stderr = child.stderr.take().ok_or(CodexAppServerError::Io)?;
        let (sender, receiver) = mpsc::sync_channel(256);
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let result = read_bounded_line(&mut reader).and_then(|line| {
                    let line =
                        std::str::from_utf8(&line).map_err(|_| CodexAppServerError::Protocol)?;
                    parse_no_duplicate_keys(line).map_err(|_| CodexAppServerError::Protocol)
                });
                let terminal = result.is_err();
                if sender.send(result).is_err() || terminal {
                    break;
                }
            }
        });
        // Drain stderr so the child cannot block. Raw stderr may contain paths or
        // provider data and is intentionally not returned through this adapter.
        thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut buffer = [0_u8; 8_192];
            while let Ok(read) = reader.read(&mut buffer) {
                if read == 0 {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            stdin,
            receiver,
            next_request_id: 1,
            notifications: VecDeque::new(),
        })
    }

    pub fn initialize(
        &mut self,
        client_version: &str,
        timeout: Duration,
    ) -> Result<InitializeObservationV1, CodexAppServerError> {
        if client_version.trim().is_empty() || client_version.len() > 128 {
            return Err(CodexAppServerError::Protocol);
        }
        let result = self.request(
            "initialize",
            json!({
                "clientInfo":{
                    "name":"star_control",
                    "title":"Star-Control",
                    "version":client_version,
                },
                "capabilities":{"experimentalApi":true}
            }),
            timeout,
        )?;
        self.notify("initialized", json!({}))?;
        Ok(InitializeObservationV1 {
            user_agent: required_string(&result, "userAgent", 512)?,
            platform_family: required_string(&result, "platformFamily", 128)?,
            platform_os: required_string(&result, "platformOs", 128)?,
        })
    }

    pub fn probe_capabilities(
        &mut self,
        codex_version: String,
        protocol_schema: ProtocolSchemaObservationV1,
        captured_at: DateTime<Utc>,
        timeout: Duration,
    ) -> Result<CapabilityProbeEvidenceV1, CodexAppServerError> {
        if !protocol_schema.methods.contains("initialize")
            || !protocol_schema.methods.contains("model/list")
        {
            return Err(CodexAppServerError::Protocol);
        }
        let initialize = self.initialize(env!("CARGO_PKG_VERSION"), timeout)?;
        let models = self.model_list(timeout)?;
        let mut limitations = Vec::new();
        let provider = if protocol_schema
            .methods
            .contains("modelProvider/capabilities/read")
        {
            Some(self.provider_capabilities(timeout)?)
        } else {
            limitations.push("provider_capabilities_unavailable".to_owned());
            None
        };
        Ok(CapabilityProbeEvidenceV1 {
            codex_version,
            captured_at,
            protocol_schema,
            initialize,
            models,
            provider,
            limitations,
        })
    }

    pub fn thread_start(
        &mut self,
        model: &str,
        cwd: Option<&Path>,
        timeout: Duration,
    ) -> Result<String, CodexAppServerError> {
        self.thread_start_with_policy(model, cwd, None, None, &[], timeout)
    }

    pub fn thread_start_with_policy(
        &mut self,
        model: &str,
        cwd: Option<&Path>,
        approval_policy: Option<&str>,
        sandbox_mode: Option<&str>,
        runtime_workspace_roots: &[PathBuf],
        timeout: Duration,
    ) -> Result<String, CodexAppServerError> {
        let mut params =
            serde_json::Map::from_iter([("model".to_owned(), Value::String(model.to_owned()))]);
        if let Some(cwd) = cwd {
            if !cwd.is_absolute() {
                return Err(CodexAppServerError::Path);
            }
            params.insert(
                "cwd".to_owned(),
                Value::String(cwd.to_string_lossy().into_owned()),
            );
        }
        insert_permission_overrides(
            &mut params,
            approval_policy,
            sandbox_mode,
            runtime_workspace_roots,
            None,
            false,
        )?;
        let result = self.request("thread/start", Value::Object(params), timeout)?;
        nested_required_string(&result, &["thread", "id"], 512)
    }

    pub fn thread_resume(
        &mut self,
        thread_id: &str,
        timeout: Duration,
    ) -> Result<String, CodexAppServerError> {
        self.thread_resume_with_policy(thread_id, None, None, &[], timeout)
    }

    pub fn thread_resume_with_policy(
        &mut self,
        thread_id: &str,
        approval_policy: Option<&str>,
        sandbox_mode: Option<&str>,
        runtime_workspace_roots: &[PathBuf],
        timeout: Duration,
    ) -> Result<String, CodexAppServerError> {
        let mut params = serde_json::Map::from_iter([(
            "threadId".to_owned(),
            Value::String(thread_id.to_owned()),
        )]);
        insert_permission_overrides(
            &mut params,
            approval_policy,
            sandbox_mode,
            runtime_workspace_roots,
            None,
            false,
        )?;
        let result = self.request("thread/resume", Value::Object(params), timeout)?;
        nested_required_string(&result, &["thread", "id"], 512)
    }

    pub fn thread_fork(
        &mut self,
        thread_id: &str,
        timeout: Duration,
    ) -> Result<String, CodexAppServerError> {
        self.thread_fork_with_policy(thread_id, None, None, &[], timeout)
    }

    pub fn thread_fork_with_policy(
        &mut self,
        thread_id: &str,
        approval_policy: Option<&str>,
        sandbox_mode: Option<&str>,
        runtime_workspace_roots: &[PathBuf],
        timeout: Duration,
    ) -> Result<String, CodexAppServerError> {
        let mut params = serde_json::Map::from_iter([(
            "threadId".to_owned(),
            Value::String(thread_id.to_owned()),
        )]);
        insert_permission_overrides(
            &mut params,
            approval_policy,
            sandbox_mode,
            runtime_workspace_roots,
            None,
            false,
        )?;
        let result = self.request("thread/fork", Value::Object(params), timeout)?;
        nested_required_string(&result, &["thread", "id"], 512)
    }

    pub fn turn_start(
        &mut self,
        thread_id: &str,
        instruction: &str,
        model: Option<&str>,
        reasoning_effort: Option<ReasoningEffortV1>,
        timeout: Duration,
    ) -> Result<String, CodexAppServerError> {
        self.turn_start_with_policy(
            thread_id,
            instruction,
            model,
            reasoning_effort,
            None,
            None,
            &[],
            &[],
            timeout,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "wire-level turn overrides stay explicit"
    )]
    pub fn turn_start_with_policy(
        &mut self,
        thread_id: &str,
        instruction: &str,
        model: Option<&str>,
        reasoning_effort: Option<ReasoningEffortV1>,
        approval_policy: Option<&str>,
        sandbox_mode: Option<&str>,
        runtime_workspace_roots: &[PathBuf],
        writable_roots: &[PathBuf],
        timeout: Duration,
    ) -> Result<String, CodexAppServerError> {
        if instruction.trim().is_empty() || instruction.len() > 256 * 1024 {
            return Err(CodexAppServerError::Protocol);
        }
        let mut params = serde_json::Map::from_iter([
            ("threadId".to_owned(), Value::String(thread_id.to_owned())),
            (
                "input".to_owned(),
                json!([{"type":"text","text":instruction}]),
            ),
        ]);
        if let Some(model) = model {
            params.insert("model".to_owned(), Value::String(model.to_owned()));
        }
        if let Some(effort) = reasoning_effort {
            params.insert(
                "effort".to_owned(),
                serde_json::to_value(effort).map_err(|_| CodexAppServerError::Protocol)?,
            );
        }
        insert_permission_overrides(
            &mut params,
            approval_policy,
            sandbox_mode,
            runtime_workspace_roots,
            Some(writable_roots),
            true,
        )?;
        let result = self.request("turn/start", Value::Object(params), timeout)?;
        nested_required_string(&result, &["turn", "id"], 512)
    }

    pub fn turn_interrupt(
        &mut self,
        thread_id: &str,
        turn_id: &str,
        timeout: Duration,
    ) -> Result<(), CodexAppServerError> {
        self.request(
            "turn/interrupt",
            json!({"threadId":thread_id,"turnId":turn_id}),
            timeout,
        )?;
        Ok(())
    }

    pub fn next_notification(&mut self, timeout: Duration) -> Result<Value, CodexAppServerError> {
        if let Some(notification) = self.notifications.pop_front() {
            return Ok(notification);
        }
        let value = self
            .receiver
            .recv_timeout(timeout)
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => CodexAppServerError::Timeout,
                mpsc::RecvTimeoutError::Disconnected => CodexAppServerError::Exited,
            })??;
        if value.get("method").and_then(Value::as_str).is_some() && value.get("id").is_none() {
            Ok(value)
        } else if value.get("method").is_some() && value.get("id").is_some() {
            Err(CodexAppServerError::UnsupportedServerRequest)
        } else {
            Err(CodexAppServerError::Protocol)
        }
    }

    fn model_list(
        &mut self,
        timeout: Duration,
    ) -> Result<Vec<ModelCapabilityV1>, CodexAppServerError> {
        let mut cursor: Option<String> = None;
        let mut models = Vec::new();
        let mut seen_cursors = BTreeSet::new();
        for _ in 0..MAX_MODEL_PAGES {
            let result = self.request(
                "model/list",
                json!({"cursor":cursor,"limit":100,"includeHidden":false}),
                timeout,
            )?;
            let data = result
                .get("data")
                .and_then(Value::as_array)
                .ok_or(CodexAppServerError::Protocol)?;
            for model in data {
                models.push(parse_model(model)?);
                if models.len() > MAX_MODELS {
                    return Err(CodexAppServerError::Protocol);
                }
            }
            cursor = match result.get("nextCursor") {
                None | Some(Value::Null) => None,
                Some(Value::String(value)) if !value.is_empty() => Some(value.clone()),
                Some(_) => return Err(CodexAppServerError::Protocol),
            };
            let Some(next) = cursor.as_ref() else {
                break;
            };
            if next.is_empty() || !seen_cursors.insert(next.clone()) {
                return Err(CodexAppServerError::Protocol);
            }
        }
        if cursor.is_some() || models.is_empty() {
            return Err(CodexAppServerError::Protocol);
        }
        Ok(models)
    }

    fn provider_capabilities(
        &mut self,
        timeout: Duration,
    ) -> Result<ProviderCapabilityObservationV1, CodexAppServerError> {
        let result = self.request("modelProvider/capabilities/read", json!({}), timeout)?;
        Ok(ProviderCapabilityObservationV1 {
            namespace_tools: required_bool(&result, "namespaceTools")?,
            image_generation: required_bool(&result, "imageGeneration")?,
            web_search: required_bool(&result, "webSearch")?,
        })
    }

    fn request(
        &mut self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, CodexAppServerError> {
        let request_id = self.next_request_id;
        self.next_request_id = self
            .next_request_id
            .checked_add(1)
            .ok_or(CodexAppServerError::Protocol)?;
        self.send(&json!({"method":method,"id":request_id,"params":params}))?;
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or(CodexAppServerError::Timeout)?;
            let value = self
                .receiver
                .recv_timeout(remaining)
                .map_err(|error| match error {
                    mpsc::RecvTimeoutError::Timeout => CodexAppServerError::Timeout,
                    mpsc::RecvTimeoutError::Disconnected => CodexAppServerError::Exited,
                })??;
            if value.get("method").is_some() {
                if value.get("id").is_some() {
                    return Err(CodexAppServerError::UnsupportedServerRequest);
                }
                if self.notifications.len() >= 2_048 {
                    return Err(CodexAppServerError::Protocol);
                }
                self.notifications.push_back(value);
                continue;
            }
            if value.get("id").and_then(Value::as_u64) != Some(request_id) {
                return Err(CodexAppServerError::Protocol);
            }
            if let Some(error) = value.get("error") {
                let code = error
                    .get("code")
                    .and_then(Value::as_i64)
                    .ok_or(CodexAppServerError::Protocol)?;
                return Err(CodexAppServerError::Remote(code));
            }
            return value
                .get("result")
                .cloned()
                .ok_or(CodexAppServerError::Protocol);
        }
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<(), CodexAppServerError> {
        self.send(&json!({"method":method,"params":params}))
    }

    fn send(&mut self, value: &Value) -> Result<(), CodexAppServerError> {
        let mut bytes = serde_json::to_vec(value).map_err(|_| CodexAppServerError::Protocol)?;
        if bytes.len() > MAX_FRAME_BYTES {
            return Err(CodexAppServerError::Protocol);
        }
        bytes.push(b'\n');
        self.stdin
            .write_all(&bytes)
            .and_then(|_| self.stdin.flush())
            .map_err(|_| CodexAppServerError::Io)
    }
}

impl Drop for CodexAppServerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_bounded_line<R: BufRead>(reader: &mut R) -> Result<Vec<u8>, CodexAppServerError> {
    let mut line = Vec::new();
    loop {
        let buffer = reader.fill_buf().map_err(|_| CodexAppServerError::Io)?;
        if buffer.is_empty() {
            return Err(CodexAppServerError::Exited);
        }
        let take = buffer
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(buffer.len(), |index| index + 1);
        if line.len() + take > MAX_FRAME_BYTES + 1 {
            return Err(CodexAppServerError::Protocol);
        }
        line.extend_from_slice(&buffer[..take]);
        reader.consume(take);
        if line.last() == Some(&b'\n') {
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if line.is_empty() {
                return Err(CodexAppServerError::Protocol);
            }
            return Ok(line);
        }
    }
}

fn parse_model(value: &Value) -> Result<ModelCapabilityV1, CodexAppServerError> {
    let efforts = value
        .get("supportedReasoningEfforts")
        .and_then(Value::as_array)
        .ok_or(CodexAppServerError::Protocol)?
        .iter()
        .map(|entry| {
            entry
                .get("reasoningEffort")
                .and_then(Value::as_str)
                .ok_or(CodexAppServerError::Protocol)
                .and_then(parse_reasoning_effort)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let default_reasoning_effort = value
        .get("defaultReasoningEffort")
        .and_then(Value::as_str)
        .ok_or(CodexAppServerError::Protocol)
        .and_then(parse_reasoning_effort)?;
    Ok(ModelCapabilityV1 {
        catalog_id: required_string(value, "id", 128)?,
        model_id: required_string(value, "model", 128)?,
        display_name: required_string(value, "displayName", 256)?,
        hidden: required_bool(value, "hidden")?,
        is_default: required_bool(value, "isDefault")?,
        supported_reasoning_efforts: efforts,
        default_reasoning_effort,
    })
}

fn parse_reasoning_effort(value: &str) -> Result<ReasoningEffortV1, CodexAppServerError> {
    match value {
        "none" => Ok(ReasoningEffortV1::None),
        "minimal" => Ok(ReasoningEffortV1::Minimal),
        "low" => Ok(ReasoningEffortV1::Low),
        "medium" => Ok(ReasoningEffortV1::Medium),
        "high" => Ok(ReasoningEffortV1::High),
        "xhigh" => Ok(ReasoningEffortV1::Xhigh),
        "max" => Ok(ReasoningEffortV1::Max),
        "ultra" => Ok(ReasoningEffortV1::Ultra),
        _ => Err(CodexAppServerError::UnsupportedReasoningEffort),
    }
}

fn required_string(value: &Value, key: &str, max: usize) -> Result<String, CodexAppServerError> {
    let value = value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(CodexAppServerError::Protocol)?;
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err(CodexAppServerError::Protocol);
    }
    Ok(value.to_owned())
}

fn nested_required_string(
    value: &Value,
    keys: &[&str],
    max: usize,
) -> Result<String, CodexAppServerError> {
    let mut current = value;
    for key in keys {
        current = current.get(*key).ok_or(CodexAppServerError::Protocol)?;
    }
    let value = current.as_str().ok_or(CodexAppServerError::Protocol)?;
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err(CodexAppServerError::Protocol);
    }
    Ok(value.to_owned())
}

fn required_bool(value: &Value, key: &str) -> Result<bool, CodexAppServerError> {
    value
        .get(key)
        .and_then(Value::as_bool)
        .ok_or(CodexAppServerError::Protocol)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn model_value(effort: &str) -> Value {
        json!({
            "id":"catalog-terra",
            "model":"gpt-5.6-terra",
            "displayName":"Terra",
            "hidden":false,
            "isDefault":true,
            "supportedReasoningEfforts":[
                {"reasoningEffort":"medium","description":"standard"},
                {"reasoningEffort":effort,"description":"extended"}
            ],
            "defaultReasoningEffort":"medium"
        })
    }

    #[test]
    fn app_server_positive_preserves_advertised_effort_order() {
        let model = parse_model(&model_value("max")).unwrap();
        assert_eq!(
            model.supported_reasoning_efforts,
            vec![ReasoningEffortV1::Medium, ReasoningEffortV1::Max]
        );
        assert_eq!(model.model_id, "gpt-5.6-terra");
    }

    #[test]
    fn app_server_negative_does_not_conflate_reasoning_effort_with_execution_mode() {
        let model = parse_model(&model_value("ultra")).unwrap();
        assert!(
            model
                .supported_reasoning_efforts
                .contains(&ReasoningEffortV1::Ultra)
        );
        let operations = BTreeMap::from([
            ("thread_start".to_owned(), true),
            ("turn_start".to_owned(), true),
        ]);
        let (native, managed) = verified_execution_modes(&operations, true, false).unwrap();
        assert_eq!(native, vec![ExecutionModeV1::Single]);
        assert!(managed.is_empty());
    }

    #[test]
    fn app_server_failure_does_not_advertise_single_without_permission_overrides() {
        let operations = BTreeMap::from([
            ("thread_start".to_owned(), true),
            ("turn_start".to_owned(), true),
        ]);
        let (native, managed) = verified_execution_modes(&operations, false, false).unwrap();
        assert!(native.is_empty());
        assert!(managed.is_empty());
    }

    #[test]
    fn app_server_failure_rejects_unimplemented_managed_ultra() {
        let operations = BTreeMap::from([
            ("thread_start".to_owned(), true),
            ("turn_start".to_owned(), true),
        ]);
        assert!(matches!(
            verified_execution_modes(&operations, true, true),
            Err(CodexAppServerError::UnsupportedExecutionMode)
        ));
    }

    #[test]
    fn app_server_negative_rejects_duplicate_json_keys() {
        let mut reader = Cursor::new(b"{\"id\":1,\"id\":2}\n".to_vec());
        let line = read_bounded_line(&mut reader).unwrap();
        assert!(parse_no_duplicate_keys(std::str::from_utf8(&line).unwrap()).is_err());
    }

    #[test]
    fn app_server_failure_rejects_unknown_effort_without_guessing() {
        assert!(matches!(
            parse_model(&model_value("future-effort")),
            Err(CodexAppServerError::UnsupportedReasoningEffort)
        ));
    }

    #[test]
    fn app_server_recovery_schema_change_gets_a_new_fingerprint() {
        let first = observe_schema_documents(vec![(
            "request.json".to_owned(),
            br#"{"properties":{"method":{"const":"model/list"},"approvalPolicy":{"type":"string"}}}"#.to_vec(),
        )])
        .unwrap();
        let second = observe_schema_documents(vec![(
            "request.json".to_owned(),
            br#"{"properties":{"method":{"enum":["model/list"]},"sandbox":{"type":"string"}}}"#
                .to_vec(),
        )])
        .unwrap();
        assert_ne!(first.schema_fingerprint, second.schema_fingerprint);
        assert!(first.methods.contains("model/list"));
        assert!(second.fields.contains("sandbox"));
    }

    #[test]
    fn app_server_negative_ignores_method_and_field_words_outside_schema_properties() {
        let observed = observe_schema_documents(vec![(
            "request.json".to_owned(),
            br#"{"description":"thread/start approvalPolicy","examples":["turn/start"],"title":"sandbox"}"#.to_vec(),
        )])
        .unwrap();
        assert!(observed.methods.is_empty());
        assert!(observed.fields.is_empty());
    }

    #[test]
    fn app_server_positive_serializes_exact_turn_permission_policy() {
        let workspace = PathBuf::from(r"C:\workspace");
        let writable = PathBuf::from(r"C:\workspace\src");
        let mut params = serde_json::Map::new();
        insert_permission_overrides(
            &mut params,
            Some("never"),
            Some("workspace-write"),
            std::slice::from_ref(&workspace),
            Some(std::slice::from_ref(&writable)),
            true,
        )
        .unwrap();
        assert_eq!(
            Value::Object(params),
            json!({
                "approvalPolicy":"never",
                "sandboxPolicy":{
                    "type":"workspaceWrite",
                    "networkAccess":false,
                    "writableRoots":[writable.to_string_lossy()]
                },
                "runtimeWorkspaceRoots":[workspace.to_string_lossy()]
            })
        );
    }

    #[test]
    fn app_server_negative_rejects_permission_escalation() {
        let mut params = serde_json::Map::new();
        assert!(matches!(
            insert_permission_overrides(
                &mut params,
                Some("on-request"),
                Some("workspace-write"),
                &[PathBuf::from(r"C:\workspace")],
                None,
                true,
            ),
            Err(CodexAppServerError::Protocol)
        ));
    }

    #[test]
    fn app_server_failure_rejects_relative_permission_roots() {
        let mut params = serde_json::Map::new();
        assert!(matches!(
            insert_permission_overrides(
                &mut params,
                Some("never"),
                Some("read-only"),
                &[PathBuf::from("relative")],
                None,
                false,
            ),
            Err(CodexAppServerError::Path)
        ));
    }

    #[test]
    fn app_server_recovery_read_only_policy_omits_write_capabilities() {
        let root = PathBuf::from(r"C:\workspace");
        let mut first = serde_json::Map::new();
        let mut second = serde_json::Map::new();
        for params in [&mut first, &mut second] {
            insert_permission_overrides(
                params,
                Some("never"),
                Some("read-only"),
                std::slice::from_ref(&root),
                Some(&[]),
                true,
            )
            .unwrap();
        }
        assert_eq!(first, second);
        let sandbox = first
            .get("sandboxPolicy")
            .and_then(Value::as_object)
            .unwrap();
        assert_eq!(sandbox.get("type"), Some(&json!("readOnly")));
        assert!(!sandbox.contains_key("writableRoots"));
        assert!(!sandbox.contains_key("networkAccess"));
    }

    #[test]
    fn app_server_failure_kills_a_hung_probe_at_the_deadline() {
        let mut child = Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 30",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let started = Instant::now();
        assert!(matches!(
            wait_child_bounded(&mut child, Duration::from_millis(25)),
            Err(CodexAppServerError::Timeout)
        ));
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(child.try_wait().unwrap().is_some());
    }
}
