//! Bounded direct-EXE process adapter.
//!
//! This layer accepts only a pre-resolved absolute `.exe` path and typed argv
//! values. It never invokes a shell, PATH lookup, command string or script
//! host. Descriptor/trust/lease validation is intentionally performed by the
//! caller before this adapter is reached.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use star_contracts::{
    Sha256Hash,
    canonical::jcs_bytes,
    manifest::ArgvBinding,
    parse_no_duplicate_keys,
    runtime::{
        ExternalToolProgress, ExternalToolRequest, ExternalToolResponse, ExternalToolResultStatus,
    },
};
use thiserror::Error;
#[cfg(not(windows))]
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
};

#[cfg(windows)]
use windows::{
    Win32::{
        Foundation::{CloseHandle, GENERIC_READ, HANDLE},
        Storage::FileSystem::{CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, OPEN_EXISTING},
        System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW,
            JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        },
    },
    core::HSTRING,
};

#[cfg(windows)]
pub mod win32_launcher;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputEncoding {
    Utf8,
    Oem,
    Utf16Le,
    Binary,
}

#[derive(Clone, Debug)]
pub struct DirectExeSpec {
    pub executable: PathBuf,
    pub argv: Vec<OsString>,
    pub working_directory: PathBuf,
    pub environment: Vec<(OsString, OsString)>,
    pub stdin: Option<Vec<u8>>,
    pub timeout: Duration,
    pub max_stdout_bytes: u64,
    pub max_stderr_bytes: u64,
    /// Only protocol-aware adapters may opt into an AppContainer token.  A
    /// missing value is the explicit `trusted_desktop` compatibility path.
    pub appcontainer_profile: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CapturedStream {
    /// Prefix retained for parsing/quarantine. The stream is nevertheless
    /// drained after this cap to prevent a child-pipe deadlock.
    pub captured: Vec<u8>,
    pub total_bytes: u64,
    pub exceeded_limit: bool,
}

#[derive(Clone, Debug)]
pub struct DirectExeOutcome {
    pub stdout: CapturedStream,
    pub stderr: CapturedStream,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("TOOL_EXECUTABLE_INVALID")]
    ExecutableInvalid,
    #[error("TOOL_PROCESS_START_FAILED")]
    Start,
    #[error("TOOL_TIMEOUT")]
    Timeout,
    #[error("TOOL_CANCELLED")]
    Cancelled,
    #[error("TOOL_OUTPUT_LIMIT")]
    OutputLimit,
    #[error("TOOL_PROTOCOL_INVALID")]
    ProtocolInvalid,
    #[error("TOOL_ENCODING_INVALID")]
    EncodingInvalid,
    #[error("TOOL_ISOLATION_UNAVAILABLE")]
    IsolationUnavailable,
}

/// Controller-owned cancellation signal. It is intentionally process-local:
/// after a Controller crash the Job Object kills the child and durable
/// Operation recovery records `outcome_unknown` instead of replaying it.
#[derive(Clone, Default)]
pub struct RuntimeCancellation(Arc<AtomicBool>);
impl RuntimeCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Holds a final file handle with write/delete sharing denied. The caller
/// acquires it before hashing and keeps it through process creation so a same-
/// path replacement cannot race the verified executable identity.
#[cfg(windows)]
pub struct ExecutableLease(HANDLE);
// A Win32 file HANDLE is transferable between threads. `ExecutableLease` has
// sole ownership and closes it only in Drop, so moving an async invocation to
// a Tokio worker does not duplicate or relax the no-write/no-delete lease.
#[cfg(windows)]
unsafe impl Send for ExecutableLease {}
#[cfg(windows)]
impl Drop for ExecutableLease {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
pub fn lease_executable(path: &Path) -> Result<ExecutableLease, RuntimeError> {
    if !path.is_absolute() {
        return Err(RuntimeError::ExecutableInvalid);
    }
    let name = HSTRING::from(path.as_os_str().to_string_lossy().as_ref());
    let handle = unsafe {
        CreateFileW(
            &name,
            GENERIC_READ.0,
            FILE_SHARE_READ,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .map_err(|_| RuntimeError::ExecutableInvalid)?;
    Ok(ExecutableLease(handle))
}

/// Operation-owned Job Object. Closing this value kills its assigned process
/// tree; failure to create or assign is fail-closed rather than running an
/// unjobbed external tool.
#[cfg(windows)]
pub struct OperationJob(HANDLE);
#[cfg(windows)]
impl OperationJob {
    pub fn new() -> Result<Self, RuntimeError> {
        let handle = unsafe { CreateJobObjectW(None, None) }.map_err(|_| RuntimeError::Start)?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
        unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        }
        .map_err(|_| RuntimeError::Start)?;
        Ok(Self(handle))
    }

    pub fn assign_handle(&self, process: HANDLE) -> Result<(), RuntimeError> {
        unsafe { AssignProcessToJobObject(self.0, process) }.map_err(|_| RuntimeError::Start)
    }

    pub fn assign(&self, child: &tokio::process::Child) -> Result<(), RuntimeError> {
        let raw = child.raw_handle().ok_or(RuntimeError::Start)?;
        self.assign_handle(HANDLE(raw.cast()))
    }
}
#[cfg(windows)]
impl Drop for OperationJob {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Expands v1 manifest bindings without constructing a shell command.  Every
/// generated argv element remains an `OsString`; a NUL, unknown binding or
/// non-scalar value fails before `CreateProcess`/tokio spawn.
pub fn bind_argv(
    bindings: &[ArgvBinding],
    arguments: &serde_json::Map<String, serde_json::Value>,
) -> Result<(Vec<OsString>, Option<Vec<u8>>), RuntimeError> {
    let mut argv = Vec::new();
    let mut stdin = None;
    for binding in bindings {
        if !binding_applies(binding, arguments)? {
            continue;
        }
        let input = |binding: &ArgvBinding| -> Result<&serde_json::Value, RuntimeError> {
            binding
                .input
                .as_ref()
                .and_then(|name| arguments.get(name))
                .ok_or(RuntimeError::ProtocolInvalid)
        };
        match binding.kind.as_str() {
            "literal" | "terminator" => {
                let value = binding
                    .value
                    .as_deref()
                    .ok_or(RuntimeError::ProtocolInvalid)?;
                if binding.kind == "terminator" && value != "--" {
                    return Err(RuntimeError::ProtocolInvalid);
                }
                push_arg(&mut argv, value)?;
            }
            "positional" => push_value(&mut argv, input(binding)?)?,
            "option" => {
                push_flag(&mut argv, binding.flag.as_deref())?;
                push_value(&mut argv, input(binding)?)?;
            }
            "flag_if_true" => {
                if input(binding)?.as_bool() == Some(true) {
                    push_flag(&mut argv, binding.flag.as_deref())?;
                }
            }
            "flag_if_false" => {
                if input(binding)?.as_bool() == Some(false) {
                    push_flag(&mut argv, binding.flag.as_deref())?;
                }
            }
            "repeat" => {
                let values = input(binding)?
                    .as_array()
                    .ok_or(RuntimeError::ProtocolInvalid)?;
                for value in values {
                    if let Some(flag) = binding.flag.as_deref() {
                        push_flag(&mut argv, Some(flag))?;
                    }
                    push_value(&mut argv, value)?;
                }
            }
            "joined" => {
                let flag = binding
                    .flag
                    .as_deref()
                    .ok_or(RuntimeError::ProtocolInvalid)?;
                let separator = binding
                    .separator
                    .as_deref()
                    .ok_or(RuntimeError::ProtocolInvalid)?;
                if !matches!(separator, "=" | ":") {
                    return Err(RuntimeError::ProtocolInvalid);
                }
                let value = scalar_string(input(binding)?)?;
                push_arg(&mut argv, &format!("{flag}{separator}{value}"))?;
            }
            "stdin_text" => {
                if stdin.is_some() {
                    return Err(RuntimeError::ProtocolInvalid);
                }
                stdin = Some(scalar_string(input(binding)?)?.into_bytes());
            }
            "stdin_json" => {
                if stdin.is_some() {
                    return Err(RuntimeError::ProtocolInvalid);
                }
                let value = if binding.inputs.is_empty() {
                    serde_json::Value::Object(arguments.clone())
                } else {
                    let mut selected = serde_json::Map::new();
                    for name in &binding.inputs {
                        selected.insert(
                            name.clone(),
                            arguments
                                .get(name)
                                .cloned()
                                .ok_or(RuntimeError::ProtocolInvalid)?,
                        );
                    }
                    serde_json::Value::Object(selected)
                };
                stdin = Some(jcs_bytes(&value).map_err(|_| RuntimeError::ProtocolInvalid)?);
            }
            _ => return Err(RuntimeError::ProtocolInvalid),
        }
    }
    Ok((argv, stdin))
}

fn binding_applies(
    binding: &ArgvBinding,
    arguments: &serde_json::Map<String, serde_json::Value>,
) -> Result<bool, RuntimeError> {
    if binding.when_present == Some(true) {
        return Ok(binding
            .input
            .as_ref()
            .is_some_and(|input| arguments.contains_key(input)));
    }
    match (&binding.when_input, &binding.when_equals) {
        (Some(input), Some(expected)) => Ok(arguments.get(input) == Some(expected)),
        (None, None) => Ok(true),
        _ => Err(RuntimeError::ProtocolInvalid),
    }
}

fn push_flag(argv: &mut Vec<OsString>, flag: Option<&str>) -> Result<(), RuntimeError> {
    let flag = flag.ok_or(RuntimeError::ProtocolInvalid)?;
    if !flag.starts_with('-') || flag.len() > 64 {
        return Err(RuntimeError::ProtocolInvalid);
    }
    push_arg(argv, flag)
}

fn push_value(argv: &mut Vec<OsString>, value: &serde_json::Value) -> Result<(), RuntimeError> {
    push_arg(argv, &scalar_string(value)?)
}

fn scalar_string(value: &serde_json::Value) -> Result<String, RuntimeError> {
    match value {
        serde_json::Value::String(value) => Ok(value.clone()),
        serde_json::Value::Number(value) => Ok(value.to_string()),
        serde_json::Value::Bool(value) => Ok(value.to_string()),
        _ => Err(RuntimeError::ProtocolInvalid),
    }
}

fn push_arg(argv: &mut Vec<OsString>, value: &str) -> Result<(), RuntimeError> {
    if value.contains('\0') {
        return Err(RuntimeError::ProtocolInvalid);
    }
    argv.push(OsString::from(value));
    Ok(())
}

impl DirectExeSpec {
    pub fn validate(&self) -> Result<(), RuntimeError> {
        if !self.executable.is_absolute()
            || !self.executable.is_file()
            || !self
                .executable
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
            || !self.working_directory.is_dir()
            || self.max_stdout_bytes == 0
            || self.max_stderr_bytes == 0
        {
            return Err(RuntimeError::ExecutableInvalid);
        }
        Ok(())
    }
}

pub async fn execute_direct_exe(spec: &DirectExeSpec) -> Result<DirectExeOutcome, RuntimeError> {
    execute_direct_exe_cancellable(spec, None).await
}

#[cfg(windows)]
pub fn appcontainer_profile_folder(profile: &str) -> Result<PathBuf, RuntimeError> {
    win32_launcher::appcontainer_profile_folder(profile)
}

pub async fn execute_direct_exe_cancellable(
    spec: &DirectExeSpec,
    cancellation: Option<RuntimeCancellation>,
) -> Result<DirectExeOutcome, RuntimeError> {
    #[cfg(windows)]
    {
        win32_launcher::execute(spec.clone(), cancellation).await
    }
    #[cfg(not(windows))]
    {
        let _ = cancellation;
        execute_direct_exe_portable(spec).await
    }
}

#[cfg(not(windows))]
async fn execute_direct_exe_portable(
    spec: &DirectExeSpec,
) -> Result<DirectExeOutcome, RuntimeError> {
    spec.validate()?;
    let mut command = Command::new(&spec.executable);
    command
        .args(&spec.argv)
        .current_dir(&spec.working_directory)
        .env_clear()
        .envs(spec.environment.iter().map(|(key, value)| (key, value)))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|_| RuntimeError::Start)?;
    // Non-Windows fallback only. Production Windows calls the dedicated
    // `CreateProcessW` launcher above, before reaching this portable path.
    let job = OperationJob::new()?;
    job.assign(&child)?;
    let mut stdin = child.stdin.take().ok_or(RuntimeError::Start)?;
    let stdout = child.stdout.take().ok_or(RuntimeError::Start)?;
    let stderr = child.stderr.take().ok_or(RuntimeError::Start)?;
    let stdin_bytes = spec.stdin.clone();
    let stdin_task = tokio::spawn(async move {
        if let Some(bytes) = stdin_bytes {
            stdin
                .write_all(&bytes)
                .await
                .map_err(|_| RuntimeError::Start)?;
        }
        stdin.shutdown().await.map_err(|_| RuntimeError::Start)
    });
    let stdout_task = tokio::spawn(drain_stream(stdout, spec.max_stdout_bytes));
    let stderr_task = tokio::spawn(drain_stream(stderr, spec.max_stderr_bytes));
    let status = match tokio::time::timeout(spec.timeout, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(_)) => return Err(RuntimeError::Start),
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            let _ = stdin_task.await;
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            return Err(RuntimeError::Timeout);
        }
    };
    stdin_task.await.map_err(|_| RuntimeError::Start)??;
    let stdout = stdout_task.await.map_err(|_| RuntimeError::Start)??;
    let stderr = stderr_task.await.map_err(|_| RuntimeError::Start)??;
    if stdout.exceeded_limit || stderr.exceeded_limit {
        return Err(RuntimeError::OutputLimit);
    }
    Ok(DirectExeOutcome {
        stdout,
        stderr,
        exit_code: status.code(),
    })
}

#[cfg(not(windows))]
async fn drain_stream(
    mut stream: impl AsyncRead + Unpin,
    limit: u64,
) -> Result<CapturedStream, RuntimeError> {
    let mut buffer = [0u8; 16 * 1024];
    let mut captured = Vec::new();
    let mut total_bytes = 0u64;
    let mut exceeded_limit = false;
    loop {
        let read = stream
            .read(&mut buffer)
            .await
            .map_err(|_| RuntimeError::Start)?;
        if read == 0 {
            break;
        }
        total_bytes = total_bytes.saturating_add(read as u64);
        let remaining = limit.saturating_sub(captured.len() as u64) as usize;
        captured.extend_from_slice(&buffer[..read.min(remaining)]);
        exceeded_limit |= total_bytes > limit;
    }
    Ok(CapturedStream {
        captured,
        total_bytes,
        exceeded_limit,
    })
}

pub fn decode_stream(
    stream: &CapturedStream,
    encoding: OutputEncoding,
) -> Result<String, RuntimeError> {
    match encoding {
        OutputEncoding::Utf8 => {
            String::from_utf8(stream.captured.clone()).map_err(|_| RuntimeError::EncodingInvalid)
        }
        OutputEncoding::Oem => decode_oem(&stream.captured),
        OutputEncoding::Utf16Le => {
            let chunks = stream.captured.chunks_exact(2);
            if !chunks.remainder().is_empty() {
                return Err(RuntimeError::EncodingInvalid);
            }
            let words: Vec<u16> = chunks
                .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
                .collect();
            String::from_utf16(&words).map_err(|_| RuntimeError::EncodingInvalid)
        }
        OutputEncoding::Binary => Err(RuntimeError::EncodingInvalid),
    }
}

#[cfg(windows)]
fn decode_oem(bytes: &[u8]) -> Result<String, RuntimeError> {
    use windows::Win32::Globalization::{CP_OEMCP, MB_ERR_INVALID_CHARS, MultiByteToWideChar};

    let length = unsafe { MultiByteToWideChar(CP_OEMCP, MB_ERR_INVALID_CHARS, bytes, None) };
    if length <= 0 {
        return Err(RuntimeError::EncodingInvalid);
    }
    let mut wide = vec![0_u16; length as usize];
    let written = unsafe {
        MultiByteToWideChar(
            CP_OEMCP,
            MB_ERR_INVALID_CHARS,
            bytes,
            Some(wide.as_mut_slice()),
        )
    };
    if written != length {
        return Err(RuntimeError::EncodingInvalid);
    }
    String::from_utf16(&wide).map_err(|_| RuntimeError::EncodingInvalid)
}

#[cfg(not(windows))]
fn decode_oem(_bytes: &[u8]) -> Result<String, RuntimeError> {
    Err(RuntimeError::EncodingInvalid)
}

pub fn parse_single_json_object(
    stream: &CapturedStream,
) -> Result<serde_json::Value, RuntimeError> {
    let text = decode_stream(stream, OutputEncoding::Utf8)?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| RuntimeError::ProtocolInvalid)?;
    value
        .is_object()
        .then_some(value)
        .ok_or(RuntimeError::ProtocolInvalid)
}

pub async fn execute_star_json_stdio(
    spec: &DirectExeSpec,
    request: &ExternalToolRequest,
) -> Result<ExternalToolResponse, RuntimeError> {
    execute_star_json_stdio_cancellable(spec, request, None).await
}

pub async fn execute_star_json_stdio_cancellable(
    spec: &DirectExeSpec,
    request: &ExternalToolRequest,
    cancellation: Option<RuntimeCancellation>,
) -> Result<ExternalToolResponse, RuntimeError> {
    if request.frame != "request"
        || request.protocol_version != 1
        || request.schema_id != "star.external-tool-request"
        || request.schema_version != 1
        || jcs_bytes(&request.arguments)
            .map_err(|_| RuntimeError::ProtocolInvalid)?
            .len()
            > 4 * 1024 * 1024
    {
        return Err(RuntimeError::ProtocolInvalid);
    }
    let mut stdin = serde_json::to_vec(request).map_err(|_| RuntimeError::ProtocolInvalid)?;
    if stdin.len() > 8 * 1024 * 1024 {
        return Err(RuntimeError::ProtocolInvalid);
    }
    stdin.push(b'\n');
    let mut process_spec = spec.clone();
    process_spec.stdin = Some(stdin);
    let outcome = execute_direct_exe_cancellable(&process_spec, cancellation).await?;
    if outcome.exit_code != Some(0) {
        return Err(RuntimeError::ProtocolInvalid);
    }
    let stdout = decode_stream(&outcome.stdout, OutputEncoding::Utf8)?;
    let frames: Vec<_> = stdout.lines().filter(|line| !line.is_empty()).collect();
    if frames.is_empty() || frames.iter().any(|frame| frame.len() > 8 * 1024 * 1024) {
        return Err(RuntimeError::ProtocolInvalid);
    }
    let mut response = None;
    let mut last_sequence = 0_u64;
    let mut last_progress = 0_u64;
    let mut last_total = None;
    const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
    for (index, frame) in frames.iter().enumerate() {
        let value = parse_no_duplicate_keys(frame).map_err(|_| RuntimeError::ProtocolInvalid)?;
        match value.get("frame").and_then(serde_json::Value::as_str) {
            Some("progress") if response.is_none() => {
                let progress: ExternalToolProgress =
                    serde_json::from_value(value).map_err(|_| RuntimeError::ProtocolInvalid)?;
                if progress.protocol_version != 1
                    || progress.request_id != request.request_id
                    || progress.sequence == 0
                    || progress.sequence > MAX_SAFE_INTEGER
                    || progress.sequence <= last_sequence
                    || progress.progress > MAX_SAFE_INTEGER
                    || progress.progress < last_progress
                    || progress.total.is_some_and(|total| {
                        total == 0
                            || total > MAX_SAFE_INTEGER
                            || progress.progress > total
                            || last_total.is_some_and(|previous| total < previous)
                    })
                    || progress
                        .message
                        .as_ref()
                        .is_some_and(|message| message.chars().count() > 500)
                {
                    return Err(RuntimeError::ProtocolInvalid);
                }
                last_sequence = progress.sequence;
                last_progress = progress.progress;
                if progress.total.is_some() {
                    last_total = progress.total;
                }
            }
            Some("result") if response.is_none() && index + 1 == frames.len() => {
                response =
                    Some(serde_json::from_value(value).map_err(|_| RuntimeError::ProtocolInvalid)?);
            }
            _ => return Err(RuntimeError::ProtocolInvalid),
        }
    }
    let response: ExternalToolResponse = response.ok_or(RuntimeError::ProtocolInvalid)?;
    if response.frame != "result"
        || response.protocol_version != 1
        || response.schema_id != "star.external-tool-response"
        || response.schema_version != 1
        || response.request_id != request.request_id
        || response.summary.is_empty()
        || response.summary.chars().count() > 1000
        || response.diagnostics.len() > 256
        || response.artifacts.len() > 256
        || matches!(response.status, ExternalToolResultStatus::Ok) && response.data.is_none()
        || !matches!(response.status, ExternalToolResultStatus::Ok) && response.data.is_some()
        || response.artifacts.iter().any(|artifact| {
            !is_safe_artifact_relative_path(Path::new(&artifact.path))
                || artifact.media_type.is_empty()
                || !matches!(
                    artifact.role.as_str(),
                    "result" | "log" | "evidence" | "debug"
                )
        })
    {
        return Err(RuntimeError::ProtocolInvalid);
    }
    validate_response_artifacts(&response, request)?;
    Ok(response)
}

fn validate_response_artifacts(
    response: &ExternalToolResponse,
    request: &ExternalToolRequest,
) -> Result<(), RuntimeError> {
    if response.artifacts.is_empty() {
        return Ok(());
    }
    let artifact_root = std::fs::canonicalize(&request.context.artifact_directory)
        .map_err(|_| RuntimeError::ProtocolInvalid)?;
    for artifact in &response.artifacts {
        let relative = Path::new(&artifact.path);
        if !is_safe_artifact_relative_path(relative) {
            return Err(RuntimeError::ProtocolInvalid);
        }
        let final_path = std::fs::canonicalize(artifact_root.join(relative))
            .map_err(|_| RuntimeError::ProtocolInvalid)?;
        if !final_path.starts_with(&artifact_root) || !final_path.is_file() {
            return Err(RuntimeError::ProtocolInvalid);
        }
        let bytes = std::fs::read(&final_path).map_err(|_| RuntimeError::ProtocolInvalid)?;
        if Sha256Hash::digest(&bytes) != artifact.sha256 {
            return Err(RuntimeError::ProtocolInvalid);
        }
    }
    Ok(())
}

pub fn is_safe_artifact_relative_path(path: &Path) -> bool {
    !path.is_absolute()
        && path.components().all(|component| {
            !matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::Prefix(_)
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_exe_and_unsafe_artifact_paths() {
        let spec = DirectExeSpec {
            executable: PathBuf::from("relative.cmd"),
            argv: vec![],
            working_directory: std::env::temp_dir(),
            environment: vec![],
            stdin: None,
            timeout: Duration::from_secs(1),
            max_stdout_bytes: 1,
            max_stderr_bytes: 1,
            appcontainer_profile: None,
        };
        assert!(matches!(
            spec.validate(),
            Err(RuntimeError::ExecutableInvalid)
        ));
        assert!(is_safe_artifact_relative_path(Path::new("result.json")));
        assert!(!is_safe_artifact_relative_path(Path::new("../escape.txt")));
    }

    #[test]
    // matrix: MCP-P029
    fn strict_encodings_do_not_replace_invalid_bytes() {
        let invalid_utf8 = CapturedStream {
            captured: vec![0xff],
            total_bytes: 1,
            exceeded_limit: false,
        };
        assert!(matches!(
            decode_stream(&invalid_utf8, OutputEncoding::Utf8),
            Err(RuntimeError::EncodingInvalid)
        ));
        let odd_utf16 = CapturedStream {
            captured: vec![0, 1, 2],
            total_bytes: 3,
            exceeded_limit: false,
        };
        assert!(matches!(
            decode_stream(&odd_utf16, OutputEncoding::Utf16Le),
            Err(RuntimeError::EncodingInvalid)
        ));
        assert!(matches!(
            decode_stream(&odd_utf16, OutputEncoding::Binary),
            Err(RuntimeError::EncodingInvalid)
        ));
        let oem_ascii = CapturedStream {
            captured: b"oem-safe-ascii".to_vec(),
            total_bytes: 14,
            exceeded_limit: false,
        };
        assert_eq!(
            decode_stream(&oem_ascii, OutputEncoding::Oem).unwrap(),
            "oem-safe-ascii"
        );
        let scalar_json = CapturedStream {
            captured: br#"["not an object"]"#.to_vec(),
            total_bytes: 17,
            exceeded_limit: false,
        };
        assert!(matches!(
            parse_single_json_object(&scalar_json),
            Err(RuntimeError::ProtocolInvalid)
        ));
    }

    #[tokio::test]
    async fn runs_an_absolute_exe_without_shell_or_path_lookup() {
        let executable = std::env::current_exe().expect("test executable");
        let working_directory = executable.parent().expect("test directory").to_path_buf();
        let outcome = execute_direct_exe(&DirectExeSpec {
            executable,
            argv: vec![OsString::from("--list")],
            working_directory,
            environment: vec![],
            stdin: None,
            timeout: Duration::from_secs(5),
            max_stdout_bytes: 1024 * 1024,
            max_stderr_bytes: 1024 * 1024,
            appcontainer_profile: None,
        })
        .await
        .expect("direct test executable runs");
        assert_eq!(outcome.exit_code, Some(0));
        assert!(!outcome.stdout.captured.is_empty());
        assert_eq!(outcome.stderr.total_bytes, 0);
    }

    #[test]
    fn executable_lease_opens_current_image_without_write_or_delete_share() {
        let executable = std::env::current_exe().expect("test executable");
        let _lease = lease_executable(&executable).expect("current image leases");
    }

    #[tokio::test]
    async fn drains_a_flood_even_when_output_limit_is_exceeded() {
        let executable = std::env::current_exe().expect("test executable");
        let working_directory = executable.parent().expect("test directory").to_path_buf();
        let error = execute_direct_exe(&DirectExeSpec {
            executable,
            argv: vec![OsString::from("--list")],
            working_directory,
            environment: vec![],
            stdin: None,
            timeout: Duration::from_secs(5),
            max_stdout_bytes: 1,
            max_stderr_bytes: 1024 * 1024,
            appcontainer_profile: None,
        })
        .await
        .expect_err("hard stdout cap fails after drain");
        assert!(matches!(error, RuntimeError::OutputLimit));
    }

    #[tokio::test]
    async fn json_stdio_rejects_bad_request_before_process_start() {
        use star_contracts::{
            Sha256Hash,
            ids::{OperationId, RequestId},
            runtime::ExternalToolContext,
        };
        let request = ExternalToolRequest {
            frame: "wrong".to_owned(),
            protocol_version: 1,
            schema_id: "star.external-tool-request".to_owned(),
            schema_version: 1,
            request_id: RequestId::new(),
            tool_id: "user.fake.tool".to_owned(),
            descriptor_hash: Sha256Hash::digest(b"descriptor"),
            arguments: serde_json::json!({}),
            context: ExternalToolContext {
                operation_id: OperationId::new(),
                project_id: None,
                goal_id: None,
                run_id: None,
                stage_id: None,
                deadline_at: "2026-07-11T00:00:00Z".to_owned(),
                artifact_directory: "artifacts".to_owned(),
                temp_directory: "temp".to_owned(),
            },
        };
        let spec = DirectExeSpec {
            executable: PathBuf::from("not-started.exe"),
            argv: vec![],
            working_directory: std::env::temp_dir(),
            environment: vec![],
            stdin: None,
            timeout: Duration::from_secs(1),
            max_stdout_bytes: 1,
            max_stderr_bytes: 1,
            appcontainer_profile: None,
        };
        assert!(matches!(
            execute_star_json_stdio(&spec, &request).await,
            Err(RuntimeError::ProtocolInvalid)
        ));
    }

    #[test]
    fn argv_binding_expands_typed_values_without_shell_syntax() {
        let binding = |kind: &str, value: Option<&str>, input: Option<&str>, flag: Option<&str>| {
            ArgvBinding {
                kind: kind.to_owned(),
                value: value.map(str::to_owned),
                input: input.map(str::to_owned),
                prefix: None,
                flag: flag.map(str::to_owned),
                separator: None,
                when_present: None,
                when_input: None,
                when_equals: None,
                inputs: vec![],
                encoding: None,
                suffix: None,
                content_kind: None,
            }
        };
        let bindings = vec![
            binding("literal", Some("--fixed"), None, None),
            binding("option", None, Some("name"), Some("--name")),
            binding("repeat", None, Some("paths"), None),
            binding("terminator", Some("--"), None, None),
        ];
        let arguments = serde_json::json!({"name":"two words","paths":["a","b"]})
            .as_object()
            .unwrap()
            .clone();
        let (argv, stdin) = bind_argv(&bindings, &arguments).unwrap();
        assert_eq!(
            argv,
            vec!["--fixed", "--name", "two words", "a", "b", "--"]
                .into_iter()
                .map(OsString::from)
                .collect::<Vec<_>>()
        );
        assert_eq!(stdin, None);
    }
}
