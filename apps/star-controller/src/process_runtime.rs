//! Bounded direct-EXE process adapter.
//!
//! This layer accepts only a pre-resolved absolute `.exe` path and typed argv
//! values. It never invokes a shell, PATH lookup, command string or script
//! host. Descriptor/trust/lease validation is intentionally performed by the
//! caller before this adapter is reached.

use std::{
    collections::BTreeSet,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::Duration,
};

use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{SecondsFormat, Utc};
use star_contracts::{
    Sha256Hash,
    canonical::jcs_bytes,
    ids::RequestId,
    manifest::ArgvBinding,
    parse_no_duplicate_keys,
    runtime::{
        ExternalToolCancel, ExternalToolCancelAck, ExternalToolProbeRequest,
        ExternalToolProbeResponse, ExternalToolProgress, ExternalToolRequest, ExternalToolResponse,
        ExternalToolResultStatus,
    },
};
use thiserror::Error;
#[cfg(not(windows))]
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
};
use zeroize::Zeroizing;

#[cfg(windows)]
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    Storage::FileSystem::{
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_INFO, FILE_NAME_NORMALIZED, FILE_SHARE_READ,
        FileIdInfo, GETFINALPATHNAMEBYHANDLE_FLAGS, GetFileInformationByHandleEx, VOLUME_NAME_GUID,
    },
    System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
        JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION, JOB_OBJECT_LIMIT_JOB_MEMORY,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectExtendedLimitInformation, SetInformationJobObject, TerminateJobObject,
    },
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
    pub max_memory_bytes: Option<u64>,
    pub max_processes: u16,
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
    pub exit_code: Option<u32>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ProcessStartEvidence {
    pub process_id: u32,
    pub creation_time_100ns: u64,
    pub job_id: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct LeasedFileIdentity {
    pub volume_serial: String,
    pub file_id: String,
    pub size: u64,
    pub last_write: String,
    pub stable_file_id: bool,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ProcessEndEvidence {
    pub exit_code: Option<u32>,
    pub termination: String,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub stdout_limit_exceeded: bool,
    pub stderr_limit_exceeded: bool,
}

pub type ProcessStartObserver = Arc<dyn Fn(ProcessStartEvidence) -> bool + Send + Sync>;
pub type ProcessEndObserver = Arc<dyn Fn(ProcessEndEvidence) -> bool + Send + Sync>;
pub type ProgressObserver = Arc<dyn Fn(ExternalToolProgress) -> bool + Send + Sync>;
pub(super) type StdoutLineObserver = Arc<dyn Fn(&[u8]) + Send + Sync>;

pub struct JsonStdioExecutionOptions {
    pub cancellation: Option<RuntimeCancellation>,
    pub cancel_grace: Duration,
    pub send_cancel_frame: bool,
    pub process_observer: Option<ProcessStartObserver>,
    pub process_end_observer: Option<ProcessEndObserver>,
    pub progress_observer: Option<ProgressObserver>,
}

impl Default for JsonStdioExecutionOptions {
    fn default() -> Self {
        Self {
            cancellation: None,
            cancel_grace: Duration::from_secs(2),
            send_cancel_frame: true,
            process_observer: None,
            process_end_observer: None,
            progress_observer: None,
        }
    }
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
const NO_FORCE_AFTER_OVERRIDE: u32 = u32::MAX;

struct RuntimeCancellationState {
    requested: AtomicBool,
    force_after_ms: AtomicU32,
}

#[derive(Clone)]
pub struct RuntimeCancellation(Arc<RuntimeCancellationState>);

impl Default for RuntimeCancellation {
    fn default() -> Self {
        Self(Arc::new(RuntimeCancellationState {
            requested: AtomicBool::new(false),
            force_after_ms: AtomicU32::new(NO_FORCE_AFTER_OVERRIDE),
        }))
    }
}

impl RuntimeCancellation {
    pub fn cancel(&self) {
        self.0.requested.store(true, Ordering::Release);
    }
    pub fn cancel_with_force_after(&self, force_after_ms: Option<u32>) {
        self.0.force_after_ms.store(
            force_after_ms.unwrap_or(NO_FORCE_AFTER_OVERRIDE),
            Ordering::Release,
        );
        self.cancel();
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.requested.load(Ordering::Acquire)
    }
    pub fn force_after_ms(&self) -> Option<u32> {
        match self.0.force_after_ms.load(Ordering::Acquire) {
            NO_FORCE_AFTER_OVERRIDE => None,
            value => Some(value),
        }
    }
}

#[derive(Clone)]
pub(super) struct StdinCancelPlan {
    user_frame: Option<Vec<u8>>,
    deadline_frame: Option<Vec<u8>>,
    grace: Duration,
    final_frame_exit_grace: Option<Duration>,
    stdout_line_observer: Option<StdoutLineObserver>,
}

/// Holds a final file handle with write/delete sharing denied. The caller
/// acquires it before hashing and keeps it through process creation so a same-
/// path replacement cannot race the verified executable identity.
#[cfg(windows)]
pub struct ExecutableLease(std::fs::File);

#[cfg(windows)]
impl ExecutableLease {
    /// Hashes the leased file as a stream so executable size cannot turn
    /// identity verification into an unbounded allocation.
    pub fn sha256(&self) -> Result<Sha256Hash, RuntimeError> {
        use std::io::{Seek, SeekFrom};

        // Windows duplicate handles share one file pointer. Positional PE
        // header reads can therefore leave a cloned stream at the end of the
        // header instead of byte zero. Always reset the leased handle before
        // hashing; reopening by path would reintroduce the TOCTOU race that
        // the lease exists to prevent.
        let mut file = self
            .0
            .try_clone()
            .map_err(|_| RuntimeError::ExecutableInvalid)?;
        file.seek(SeekFrom::Start(0))
            .map_err(|_| RuntimeError::ExecutableInvalid)?;
        Sha256Hash::digest_reader(file).map_err(|_| RuntimeError::ExecutableInvalid)
    }

    /// Reads only the DOS header and PE signature/machine field from the same
    /// leased file object. `e_lfanew` is used as a positional offset and never
    /// as an allocation length.
    pub fn pe_architecture(&self) -> Result<&'static str, RuntimeError> {
        use std::os::windows::fs::FileExt;
        let length = self
            .0
            .metadata()
            .map_err(|_| RuntimeError::ExecutableInvalid)?
            .len();
        let mut dos = [0_u8; 64];
        if length < dos.len() as u64
            || self
                .0
                .seek_read(&mut dos, 0)
                .map_err(|_| RuntimeError::ExecutableInvalid)?
                != dos.len()
            || &dos[..2] != b"MZ"
        {
            return Err(RuntimeError::ExecutableInvalid);
        }
        let offset =
            u32::from_le_bytes(dos[0x3c..0x40].try_into().expect("fixed DOS header offset")) as u64;
        let mut header = [0_u8; 6];
        if offset
            .checked_add(header.len() as u64)
            .is_none_or(|end| end > length)
            || self
                .0
                .seek_read(&mut header, offset)
                .map_err(|_| RuntimeError::ExecutableInvalid)?
                != header.len()
            || &header[..4] != b"PE\0\0"
        {
            return Err(RuntimeError::ExecutableInvalid);
        }
        match u16::from_le_bytes([header[4], header[5]]) {
            0x8664 => Ok("x86_64"),
            0xaa64 => Ok("aarch64"),
            _ => Err(RuntimeError::ExecutableInvalid),
        }
    }

    /// Returns the final path for the same leased file object. Callers may
    /// hash this value for durable identity evidence without persisting a
    /// private absolute path.
    pub fn final_path(&self) -> Result<PathBuf, RuntimeError> {
        final_guid_path_for_handle(&self.0).ok_or(RuntimeError::ExecutableInvalid)
    }

    pub fn identity(&self) -> Result<LeasedFileIdentity, RuntimeError> {
        use std::os::windows::io::AsRawHandle;
        let metadata = self
            .0
            .metadata()
            .map_err(|_| RuntimeError::ExecutableInvalid)?;
        let mut information = FILE_ID_INFO::default();
        let stable_file_id = unsafe {
            GetFileInformationByHandleEx(
                HANDLE(self.0.as_raw_handle().cast()),
                FileIdInfo,
                (&raw mut information).cast(),
                std::mem::size_of::<FILE_ID_INFO>() as u32,
            )
        }
        .is_ok();
        let (volume_serial, file_id) = if stable_file_id {
            (
                format!("{:016x}", information.VolumeSerialNumber),
                information
                    .FileId
                    .Identifier
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
            )
        } else {
            ("unavailable".to_owned(), "unavailable".to_owned())
        };
        Ok(LeasedFileIdentity {
            volume_serial,
            file_id,
            size: metadata.len(),
            last_write: metadata
                .modified()
                .map(chrono::DateTime::<Utc>::from)
                .map_err(|_| RuntimeError::ExecutableInvalid)?
                .to_rfc3339_opts(SecondsFormat::Millis, true),
            stable_file_id,
        })
    }
}

#[cfg(windows)]
pub fn lease_executable(path: &Path) -> Result<ExecutableLease, RuntimeError> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    if !path.is_absolute()
        || !path.is_file()
        || !is_fixed_local_windows_path(path)
        || path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("exe"))
        || path
            .file_name()
            .and_then(|name| name.to_str())
            .is_none_or(star_contracts::manifest::is_forbidden_executable_name)
        || std::fs::symlink_metadata(path)
            .ok()
            .is_none_or(|metadata| metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
    {
        return Err(RuntimeError::ExecutableInvalid);
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ.0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)
        .map_err(|_| RuntimeError::ExecutableInvalid)?;
    if file.metadata().ok().is_none_or(|metadata| {
        !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }) || final_path_for_handle(&file).is_none_or(|final_path| {
        std::fs::canonicalize(path)
            .ok()
            .is_none_or(|expected| !same_windows_path(&final_path, &expected))
    }) {
        return Err(RuntimeError::ExecutableInvalid);
    }
    Ok(ExecutableLease(file))
}

#[cfg(windows)]
fn is_fixed_local_windows_path(path: &Path) -> bool {
    use std::path::{Component, Prefix};
    use windows::{
        Win32::{Storage::FileSystem::GetDriveTypeW, System::WindowsProgramming::DRIVE_FIXED},
        core::HSTRING,
    };
    let drive = match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => Some(letter),
            _ => None,
        },
        _ => None,
    };
    drive.is_some_and(|letter| {
        let root = HSTRING::from(format!("{}:\\", char::from(letter)));
        unsafe { GetDriveTypeW(&root) == DRIVE_FIXED }
    })
}

#[cfg(windows)]
fn final_path_for_handle(file: &std::fs::File) -> Option<PathBuf> {
    use std::{
        ffi::OsString,
        os::windows::{ffi::OsStringExt, io::AsRawHandle},
    };
    use windows::Win32::Storage::FileSystem::{
        GETFINALPATHNAMEBYHANDLE_FLAGS, GetFinalPathNameByHandleW,
    };
    let mut buffer = vec![0_u16; 32_768];
    let length = unsafe {
        GetFinalPathNameByHandleW(
            HANDLE(file.as_raw_handle().cast()),
            &mut buffer,
            GETFINALPATHNAMEBYHANDLE_FLAGS(0),
        )
    } as usize;
    if length == 0 || length >= buffer.len() {
        return None;
    }
    buffer.truncate(length);
    Some(PathBuf::from(OsString::from_wide(&buffer)))
}

#[cfg(windows)]
fn final_guid_path_for_handle(file: &std::fs::File) -> Option<PathBuf> {
    use std::{
        ffi::OsString,
        os::windows::{ffi::OsStringExt, io::AsRawHandle},
    };
    use windows::Win32::Storage::FileSystem::GetFinalPathNameByHandleW;
    let mut buffer = vec![0_u16; 32_768];
    let length = unsafe {
        GetFinalPathNameByHandleW(
            HANDLE(file.as_raw_handle().cast()),
            &mut buffer,
            GETFINALPATHNAMEBYHANDLE_FLAGS(FILE_NAME_NORMALIZED.0 | VOLUME_NAME_GUID.0),
        )
    } as usize;
    if length == 0 || length >= buffer.len() {
        return None;
    }
    buffer.truncate(length);
    Some(PathBuf::from(OsString::from_wide(&buffer)))
}

#[cfg(windows)]
fn same_windows_path(left: &Path, right: &Path) -> bool {
    left.as_os_str()
        .to_string_lossy()
        .replace('/', "\\")
        .trim_start_matches(r"\\?\")
        .eq_ignore_ascii_case(
            right
                .as_os_str()
                .to_string_lossy()
                .replace('/', "\\")
                .trim_start_matches(r"\\?\"),
        )
}

/// Operation-owned Job Object. Closing this value kills its assigned process
/// tree; failure to create or assign is fail-closed rather than running an
/// unjobbed external tool.
#[cfg(windows)]
pub struct OperationJob(HANDLE);
#[cfg(windows)]
impl OperationJob {
    pub fn new() -> Result<Self, RuntimeError> {
        Self::new_with_limits(None, u16::MAX)
    }

    pub fn new_with_limits(
        max_memory_bytes: Option<u64>,
        max_processes: u16,
    ) -> Result<Self, RuntimeError> {
        if max_processes == 0 {
            return Err(RuntimeError::Start);
        }
        let handle = unsafe { CreateJobObjectW(None, None) }.map_err(|_| RuntimeError::Start)?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        limits.BasicLimitInformation.ActiveProcessLimit = u32::from(max_processes);
        if let Some(memory) = max_memory_bytes {
            limits.JobMemoryLimit = usize::try_from(memory).map_err(|_| RuntimeError::Start)?;
            limits.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_JOB_MEMORY;
        }
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

    pub fn terminate(&self, exit_code: u32) -> Result<(), RuntimeError> {
        unsafe { TerminateJobObject(self.0, exit_code) }.map_err(|_| RuntimeError::Start)
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
pub struct BoundArguments {
    argv: Vec<OsString>,
    stdin: Option<Vec<u8>>,
    temp_files: Vec<PathBuf>,
}

impl BoundArguments {
    pub fn argv(&self) -> &[OsString] {
        &self.argv
    }

    pub fn stdin(&self) -> Option<&[u8]> {
        self.stdin.as_deref()
    }
}

impl Drop for BoundArguments {
    fn drop(&mut self) {
        for path in &self.temp_files {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub fn bind_argv(
    bindings: &[ArgvBinding],
    arguments: &serde_json::Map<String, serde_json::Value>,
    temp_directory: &Path,
    secret_inputs: &BTreeSet<String>,
) -> Result<BoundArguments, RuntimeError> {
    std::fs::create_dir_all(temp_directory).map_err(|_| RuntimeError::Start)?;
    let mut bound = BoundArguments {
        argv: Vec::new(),
        stdin: None,
        temp_files: Vec::new(),
    };
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
                push_arg(&mut bound.argv, value)?;
            }
            "positional" => push_value(&mut bound.argv, input(binding)?)?,
            "option" => {
                push_flag(&mut bound.argv, binding.flag.as_deref())?;
                push_value(&mut bound.argv, input(binding)?)?;
            }
            "flag_if_true" => {
                if input(binding)?.as_bool() == Some(true) {
                    push_flag(&mut bound.argv, binding.flag.as_deref())?;
                }
            }
            "flag_if_false" => {
                if input(binding)?.as_bool() == Some(false) {
                    push_flag(&mut bound.argv, binding.flag.as_deref())?;
                }
            }
            "repeat" => {
                let values = input(binding)?
                    .as_array()
                    .ok_or(RuntimeError::ProtocolInvalid)?;
                for value in values {
                    if let Some(flag) = binding.flag.as_deref() {
                        push_flag(&mut bound.argv, Some(flag))?;
                    }
                    push_value(&mut bound.argv, value)?;
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
                push_arg(&mut bound.argv, &format!("{flag}{separator}{value}"))?;
            }
            "stdin_text" => {
                if bound.stdin.is_some() {
                    return Err(RuntimeError::ProtocolInvalid);
                }
                let text = input(binding)?
                    .as_str()
                    .ok_or(RuntimeError::ProtocolInvalid)?;
                bound.stdin = Some(encode_binding_text(
                    text,
                    binding.encoding.as_deref().unwrap_or("utf8"),
                )?);
            }
            "stdin_json" => {
                if bound.stdin.is_some() {
                    return Err(RuntimeError::ProtocolInvalid);
                }
                let value = if binding.inputs.is_empty() {
                    serde_json::Value::Object(
                        arguments
                            .iter()
                            .filter(|(name, _)| !secret_inputs.contains(*name))
                            .map(|(name, value)| (name.clone(), value.clone()))
                            .collect(),
                    )
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
                bound.stdin = Some(jcs_bytes(&value).map_err(|_| RuntimeError::ProtocolInvalid)?);
            }
            "temp_file" => {
                let value = input(binding)?;
                let content_kind = binding.content_kind.as_deref().unwrap_or("text");
                let bytes = Zeroizing::new(match content_kind {
                    "text" => encode_binding_text(
                        value.as_str().ok_or(RuntimeError::ProtocolInvalid)?,
                        binding.encoding.as_deref().unwrap_or("utf8"),
                    )?,
                    "json" => {
                        let json = jcs_bytes(value).map_err(|_| RuntimeError::ProtocolInvalid)?;
                        match binding.encoding.as_deref().unwrap_or("utf8") {
                            "utf8" => json,
                            "utf16le" => encode_utf16le(
                                std::str::from_utf8(&json)
                                    .map_err(|_| RuntimeError::ProtocolInvalid)?,
                            ),
                            _ => return Err(RuntimeError::ProtocolInvalid),
                        }
                    }
                    "base64" if binding.encoding.is_none() => STANDARD
                        .decode(value.as_str().ok_or(RuntimeError::ProtocolInvalid)?)
                        .map_err(|_| RuntimeError::ProtocolInvalid)?,
                    _ => return Err(RuntimeError::ProtocolInvalid),
                });
                if bytes.len() > 4 * 1024 * 1024 {
                    return Err(RuntimeError::ProtocolInvalid);
                }
                let suffix = binding.suffix.as_deref().unwrap_or(".tmp");
                let path = temp_directory.join(format!("input-{}{}", star_ipc::nonce(), suffix));
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(|_| RuntimeError::Start)?;
                std::io::Write::write_all(&mut file, &bytes).map_err(|_| RuntimeError::Start)?;
                file.sync_all().map_err(|_| RuntimeError::Start)?;
                star_ipc::key_store::apply_owner_system_dacl(&path)
                    .map_err(|_| RuntimeError::Start)?;
                push_arg(&mut bound.argv, &path.to_string_lossy())?;
                bound.temp_files.push(path);
            }
            _ => return Err(RuntimeError::ProtocolInvalid),
        }
    }
    Ok(bound)
}

fn encode_binding_text(text: &str, encoding: &str) -> Result<Vec<u8>, RuntimeError> {
    match encoding {
        "utf8" => Ok(text.as_bytes().to_vec()),
        "utf16le" => Ok(encode_utf16le(text)),
        _ => Err(RuntimeError::ProtocolInvalid),
    }
}

fn encode_utf16le(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>()
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
            || self
                .executable
                .file_name()
                .and_then(|name| name.to_str())
                .is_none_or(star_contracts::manifest::is_forbidden_executable_name)
            || !self.working_directory.is_dir()
            || self.max_stdout_bytes == 0
            || self.max_stderr_bytes == 0
            || self.max_processes == 0
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

#[cfg(windows)]
pub fn appcontainer_profile_sid_string(profile: &str) -> Result<String, RuntimeError> {
    win32_launcher::appcontainer_profile_sid_string(profile)
}

pub async fn execute_direct_exe_cancellable(
    spec: &DirectExeSpec,
    cancellation: Option<RuntimeCancellation>,
) -> Result<DirectExeOutcome, RuntimeError> {
    execute_direct_exe_with_cancel_plan(spec, cancellation, None, None, None).await
}

pub async fn execute_direct_exe_cancellable_with_grace(
    spec: &DirectExeSpec,
    cancellation: Option<RuntimeCancellation>,
    grace: Duration,
    process_observer: Option<ProcessStartObserver>,
    process_end_observer: Option<ProcessEndObserver>,
) -> Result<DirectExeOutcome, RuntimeError> {
    execute_direct_exe_with_cancel_plan(
        spec,
        cancellation,
        Some(StdinCancelPlan {
            user_frame: None,
            deadline_frame: None,
            grace,
            final_frame_exit_grace: None,
            stdout_line_observer: None,
        }),
        process_observer,
        process_end_observer,
    )
    .await
}

async fn execute_direct_exe_with_cancel_plan(
    spec: &DirectExeSpec,
    cancellation: Option<RuntimeCancellation>,
    cancel_plan: Option<StdinCancelPlan>,
    process_observer: Option<ProcessStartObserver>,
    process_end_observer: Option<ProcessEndObserver>,
) -> Result<DirectExeOutcome, RuntimeError> {
    #[cfg(windows)]
    {
        win32_launcher::execute(
            spec.clone(),
            cancellation,
            cancel_plan,
            process_observer,
            process_end_observer,
        )
        .await
    }
    #[cfg(not(windows))]
    {
        let _ = cancel_plan;
        let _ = cancellation;
        let _ = process_observer;
        let _ = process_end_observer;
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
        exit_code: status.code().map(|code| code as u32),
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
pub fn oem_code_page() -> u32 {
    unsafe { windows::Win32::Globalization::GetOEMCP() }
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
    let value = parse_no_duplicate_keys(&text).map_err(|_| RuntimeError::ProtocolInvalid)?;
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
    execute_star_json_stdio_cancellable_with_grace(
        spec,
        request,
        cancellation,
        Duration::from_secs(2),
    )
    .await
    .map(|outcome| outcome.response)
}

pub struct StarJsonStdioOutcome {
    pub response: ExternalToolResponse,
    pub progress: Vec<ExternalToolProgress>,
}

const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Default)]
struct ProgressValidationState {
    last_sequence: u64,
    last_progress: u64,
    last_total: Option<u64>,
    count: usize,
}

impl ProgressValidationState {
    fn accept(
        &mut self,
        progress: &ExternalToolProgress,
        request_id: &RequestId,
    ) -> Result<(), RuntimeError> {
        if self.count >= 4_096 {
            return Err(RuntimeError::OutputLimit);
        }
        if progress.frame != "progress"
            || progress.protocol_version != 1
            || progress.request_id != *request_id
            || progress.sequence == 0
            || progress.sequence > MAX_SAFE_JSON_INTEGER
            || progress.sequence <= self.last_sequence
            || progress.progress > MAX_SAFE_JSON_INTEGER
            || progress.progress < self.last_progress
            || progress.total.is_some_and(|total| {
                total == 0
                    || total > MAX_SAFE_JSON_INTEGER
                    || progress.progress > total
                    || self.last_total.is_some_and(|previous| total < previous)
            })
            || progress.message.as_ref().is_some_and(|message| {
                message.chars().count() > 500 || contains_windows_absolute_path(message)
            })
        {
            return Err(RuntimeError::ProtocolInvalid);
        }
        self.last_sequence = progress.sequence;
        self.last_progress = progress.progress;
        if progress.total.is_some() {
            self.last_total = progress.total;
        }
        self.count += 1;
        Ok(())
    }
}

fn parse_no_duplicate_keys_bytes(bytes: &[u8]) -> Result<serde_json::Value, RuntimeError> {
    let text = std::str::from_utf8(bytes).map_err(|_| RuntimeError::ProtocolInvalid)?;
    parse_no_duplicate_keys(text).map_err(|_| RuntimeError::ProtocolInvalid)
}

fn contains_windows_absolute_path(message: &str) -> bool {
    let bytes = message.as_bytes();
    if bytes.windows(2).any(|pair| pair == b"\\\\") {
        return true;
    }
    bytes.windows(3).enumerate().any(|(index, triplet)| {
        let boundary = index == 0 || !bytes[index - 1].is_ascii_alphanumeric();
        boundary
            && triplet[0].is_ascii_alphabetic()
            && triplet[1] == b':'
            && matches!(triplet[2], b'\\' | b'/')
    })
}

pub async fn execute_star_json_stdio_cancellable_with_grace(
    spec: &DirectExeSpec,
    request: &ExternalToolRequest,
    cancellation: Option<RuntimeCancellation>,
    cancel_grace: Duration,
) -> Result<StarJsonStdioOutcome, RuntimeError> {
    execute_star_json_stdio_cancellable_with_cancel_mode(
        spec,
        request,
        JsonStdioExecutionOptions {
            cancellation,
            cancel_grace,
            ..Default::default()
        },
    )
    .await
}

pub async fn execute_star_json_stdio_cancellable_with_cancel_mode(
    spec: &DirectExeSpec,
    request: &ExternalToolRequest,
    options: JsonStdioExecutionOptions,
) -> Result<StarJsonStdioOutcome, RuntimeError> {
    let JsonStdioExecutionOptions {
        cancellation,
        cancel_grace,
        send_cancel_frame,
        process_observer,
        process_end_observer,
        progress_observer,
    } = options;
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
    let cancel_frame = |reason: &str| -> Result<Vec<u8>, RuntimeError> {
        let mut frame = jcs_bytes(
            &serde_json::to_value(ExternalToolCancel {
                frame: "cancel".to_owned(),
                protocol_version: 1,
                request_id: request.request_id.clone(),
                reason: reason.to_owned(),
            })
            .map_err(|_| RuntimeError::ProtocolInvalid)?,
        )
        .map_err(|_| RuntimeError::ProtocolInvalid)?;
        frame.push(b'\n');
        Ok(frame)
    };
    let live_progress_invalid = Arc::new(AtomicBool::new(false));
    let stdout_line_observer: Option<StdoutLineObserver> = progress_observer.map(|observer| {
        let request_id = request.request_id.clone();
        let state = Arc::new(std::sync::Mutex::new(ProgressValidationState::default()));
        let invalid = Arc::clone(&live_progress_invalid);
        Arc::new(move |line: &[u8]| {
            let Ok(value) = parse_no_duplicate_keys_bytes(line) else {
                invalid.store(true, Ordering::Release);
                return;
            };
            match value.get("frame").and_then(serde_json::Value::as_str) {
                Some("progress") => {
                    let Ok(progress) = serde_json::from_value::<ExternalToolProgress>(value) else {
                        invalid.store(true, Ordering::Release);
                        return;
                    };
                    let valid = state
                        .lock()
                        .ok()
                        .is_some_and(|mut state| state.accept(&progress, &request_id).is_ok());
                    if !valid || !observer(progress) {
                        invalid.store(true, Ordering::Release);
                    }
                }
                Some("cancel_ack" | "result") => {}
                _ => invalid.store(true, Ordering::Release),
            }
        }) as StdoutLineObserver
    });
    let outcome = execute_direct_exe_with_cancel_plan(
        &process_spec,
        cancellation,
        Some(StdinCancelPlan {
            user_frame: send_cancel_frame
                .then(|| cancel_frame("user_requested"))
                .transpose()?,
            deadline_frame: send_cancel_frame
                .then(|| cancel_frame("deadline"))
                .transpose()?,
            grace: cancel_grace,
            final_frame_exit_grace: Some(Duration::from_secs(5)),
            stdout_line_observer,
        }),
        process_observer,
        process_end_observer,
    )
    .await?;
    if live_progress_invalid.load(Ordering::Acquire) {
        return Err(RuntimeError::ProtocolInvalid);
    }
    if outcome.exit_code != Some(0) {
        return Err(RuntimeError::ProtocolInvalid);
    }
    let stdout = decode_stream(&outcome.stdout, OutputEncoding::Utf8)?;
    validate_star_json_stdio_output(&stdout, request)
}

pub fn validate_star_json_stdio_output(
    stdout: &str,
    request: &ExternalToolRequest,
) -> Result<StarJsonStdioOutcome, RuntimeError> {
    if stdout.len() > 64 * 1024 * 1024 {
        return Err(RuntimeError::OutputLimit);
    }
    let mut frames = stdout.lines().peekable();
    if frames.peek().is_none() {
        return Err(RuntimeError::ProtocolInvalid);
    }
    let mut response = None;
    let mut progress_events = Vec::new();
    let mut progress_state = ProgressValidationState::default();
    let mut cancel_ack_seen = false;
    while let Some(frame) = frames.next() {
        if frame.is_empty() || frame.len() > 8 * 1024 * 1024 {
            return Err(RuntimeError::ProtocolInvalid);
        }
        let is_last = frames.peek().is_none();
        let value = parse_no_duplicate_keys(frame).map_err(|_| RuntimeError::ProtocolInvalid)?;
        match value.get("frame").and_then(serde_json::Value::as_str) {
            Some("progress") if response.is_none() => {
                let progress: ExternalToolProgress =
                    serde_json::from_value(value).map_err(|_| RuntimeError::ProtocolInvalid)?;
                progress_state.accept(&progress, &request.request_id)?;
                progress_events.push(progress);
            }
            Some("cancel_ack") if response.is_none() && !cancel_ack_seen => {
                let ack: ExternalToolCancelAck =
                    serde_json::from_value(value).map_err(|_| RuntimeError::ProtocolInvalid)?;
                if ack.frame != "cancel_ack"
                    || ack.protocol_version != 1
                    || ack.request_id != request.request_id
                {
                    return Err(RuntimeError::ProtocolInvalid);
                }
                cancel_ack_seen = true;
            }
            Some("result") if response.is_none() && is_last => {
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
        || matches!(response.status, ExternalToolResultStatus::Error) != response.error.is_some()
        || matches!(response.status, ExternalToolResultStatus::Cancelled)
            && response.error.is_some()
        || response.error.as_ref().is_some_and(|error| {
            error.code.is_empty()
                || error.code.chars().count() > 128
                || error.message.is_empty()
                || error.message.chars().count() > 1_000
        })
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
    Ok(StarJsonStdioOutcome {
        response,
        progress: progress_events,
    })
}

pub async fn execute_star_json_probe(
    spec: &DirectExeSpec,
    request_id: RequestId,
) -> Result<ExternalToolProbeResponse, RuntimeError> {
    let mut stdin = jcs_bytes(
        &serde_json::to_value(ExternalToolProbeRequest {
            frame: "probe".to_owned(),
            protocol_version: 1,
            request_id: request_id.clone(),
        })
        .map_err(|_| RuntimeError::ProtocolInvalid)?,
    )
    .map_err(|_| RuntimeError::ProtocolInvalid)?;
    stdin.push(b'\n');
    let mut spec = spec.clone();
    spec.stdin = Some(stdin);
    let outcome = execute_direct_exe_with_cancel_plan(
        &spec,
        None,
        Some(StdinCancelPlan {
            user_frame: None,
            deadline_frame: None,
            grace: Duration::from_secs(2),
            final_frame_exit_grace: Some(Duration::from_secs(5)),
            stdout_line_observer: None,
        }),
        None,
        None,
    )
    .await?;
    if outcome.exit_code != Some(0) {
        return Err(RuntimeError::ProtocolInvalid);
    }
    let stdout = decode_stream(&outcome.stdout, OutputEncoding::Utf8)?;
    let mut lines = stdout.lines();
    let line = lines.next().ok_or(RuntimeError::ProtocolInvalid)?;
    if lines.next().is_some() || line.len() > 8 * 1024 * 1024 {
        return Err(RuntimeError::ProtocolInvalid);
    }
    let value = parse_no_duplicate_keys(line).map_err(|_| RuntimeError::ProtocolInvalid)?;
    let response: ExternalToolProbeResponse =
        serde_json::from_value(value).map_err(|_| RuntimeError::ProtocolInvalid)?;
    let valid_capability =
        regex::Regex::new(r"^[a-z][a-z0-9_-]{0,63}$").expect("static capability regex");
    let unique: BTreeSet<_> = response.capabilities.iter().collect();
    if response.frame != "probe_result"
        || response.protocol_version != 1
        || response.request_id != request_id
        || response.capabilities.len() > 32
        || unique.len() != response.capabilities.len()
        || response
            .capabilities
            .iter()
            .any(|capability| !valid_capability.is_match(capability))
        || !star_contracts::manifest::version_requirement_matches("*", &response.product_version)
        || response.interface_version.as_ref().is_some_and(|version| {
            !star_contracts::manifest::version_requirement_matches("*", version)
        })
    {
        return Err(RuntimeError::ProtocolInvalid);
    }
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
    let mut total_bytes = 0_u64;
    #[cfg(windows)]
    let mut leases = Vec::new();
    for artifact in &response.artifacts {
        let relative = Path::new(&artifact.path);
        if !is_safe_artifact_relative_path(relative) {
            return Err(RuntimeError::ProtocolInvalid);
        }
        let candidate = artifact_root.join(relative);
        #[cfg(windows)]
        let (file, final_path) = lease_regular_artifact(&candidate)?;
        #[cfg(not(windows))]
        let (file, final_path) = {
            let final_path =
                std::fs::canonicalize(&candidate).map_err(|_| RuntimeError::ProtocolInvalid)?;
            let file =
                std::fs::File::open(&final_path).map_err(|_| RuntimeError::ProtocolInvalid)?;
            (file, final_path)
        };
        if !final_path.starts_with(&artifact_root) {
            return Err(RuntimeError::ProtocolInvalid);
        }
        let length = file
            .metadata()
            .map_err(|_| RuntimeError::ProtocolInvalid)?
            .len();
        total_bytes = total_bytes
            .checked_add(length)
            .filter(|total| *total <= 1_073_741_824)
            .ok_or(RuntimeError::OutputLimit)?;
        let hash = Sha256Hash::digest_reader(
            file.try_clone()
                .map_err(|_| RuntimeError::ProtocolInvalid)?,
        )
        .map_err(|_| RuntimeError::ProtocolInvalid)?;
        if hash != artifact.sha256 {
            return Err(RuntimeError::ProtocolInvalid);
        }
        #[cfg(windows)]
        leases.push(file);
    }
    Ok(())
}

#[cfg(windows)]
fn lease_regular_artifact(path: &Path) -> Result<(std::fs::File, PathBuf), RuntimeError> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ.0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)
        .map_err(|_| RuntimeError::ProtocolInvalid)?;
    let metadata = file.metadata().map_err(|_| RuntimeError::ProtocolInvalid)?;
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(RuntimeError::ProtocolInvalid);
    }
    let final_path = final_path_for_handle(&file).ok_or(RuntimeError::ProtocolInvalid)?;
    let expected = std::fs::canonicalize(path).map_err(|_| RuntimeError::ProtocolInvalid)?;
    if !same_windows_path(&final_path, &expected) {
        return Err(RuntimeError::ProtocolInvalid);
    }
    Ok((file, final_path))
}

pub fn is_safe_artifact_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path.components().all(|component| {
            let std::path::Component::Normal(component) = component else {
                return false;
            };
            let Some(component) = component.to_str() else {
                return false;
            };
            !component.is_empty()
                && !component.contains([':', '\0'])
                && !component.ends_with(['.', ' '])
                && !is_windows_device_component(component)
        })
}

fn is_windows_device_component(component: &str) -> bool {
    let stem = component
        .split_once('.')
        .map_or(component, |(stem, _)| stem)
        .trim_end_matches(['.', ' ']);
    matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_memory_limit_applies_to_the_whole_job_tree() {
        use windows::Win32::System::JobObjects::{
            JOB_OBJECT_LIMIT_JOB_MEMORY, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JobObjectExtendedLimitInformation, QueryInformationJobObject,
        };

        let expected = 64 * 1024 * 1024_u64;
        let job = OperationJob::new_with_limits(Some(expected), 4).unwrap();
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        unsafe {
            QueryInformationJobObject(
                Some(job.0),
                JobObjectExtendedLimitInformation,
                (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                None,
            )
        }
        .unwrap();
        assert_ne!(
            limits.BasicLimitInformation.LimitFlags & JOB_OBJECT_LIMIT_JOB_MEMORY,
            Default::default()
        );
        assert_eq!(limits.JobMemoryLimit as u64, expected);
    }

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
            max_memory_bytes: None,
            max_processes: 16,
            appcontainer_profile: None,
        };
        assert!(matches!(
            spec.validate(),
            Err(RuntimeError::ExecutableInvalid)
        ));
        assert!(is_safe_artifact_relative_path(Path::new("result.json")));
        assert!(!is_safe_artifact_relative_path(Path::new("../escape.txt")));
        assert!(!is_safe_artifact_relative_path(Path::new(
            "result.txt:secret"
        )));
        assert!(!is_safe_artifact_relative_path(Path::new("NUL.txt")));
        assert!(!is_safe_artifact_relative_path(Path::new("./result.json")));
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
            max_memory_bytes: None,
            max_processes: 16,
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
        let expected =
            Sha256Hash::digest_reader(std::fs::File::open(&executable).unwrap()).unwrap();
        let lease = lease_executable(&executable).expect("current image leases");
        lease.pe_architecture().expect("current image is a PE");
        assert_eq!(lease.sha256().unwrap(), expected);
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
            max_memory_bytes: None,
            max_processes: 16,
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
            max_memory_bytes: None,
            max_processes: 16,
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
        let temp = std::env::temp_dir().join(format!("star-bind-{}", star_ipc::nonce()));
        let bound = bind_argv(&bindings, &arguments, &temp, &BTreeSet::new()).unwrap();
        assert_eq!(
            bound.argv,
            vec!["--fixed", "--name", "two words", "a", "b", "--"]
                .into_iter()
                .map(OsString::from)
                .collect::<Vec<_>>()
        );
        assert_eq!(bound.stdin, None);
    }

    #[test]
    // matrix: MCP-P001 MCP-P028
    fn temp_file_and_stdin_bindings_materialize_exact_content_and_clean_up() {
        let temp = std::env::temp_dir().join(format!("star-bind-temp-{}", star_ipc::nonce()));
        let temp_binding = ArgvBinding {
            kind: "temp_file".to_owned(),
            value: None,
            input: Some("payload".to_owned()),
            flag: None,
            separator: None,
            when_present: None,
            when_input: None,
            when_equals: None,
            inputs: vec![],
            encoding: Some("utf16le".to_owned()),
            suffix: Some(".json".to_owned()),
            content_kind: Some("json".to_owned()),
        };
        let stdin_binding = ArgvBinding {
            kind: "stdin_json".to_owned(),
            inputs: vec![],
            ..temp_binding.clone()
        };
        let arguments = serde_json::json!({
            "payload":{"value":"한글"},
            "secret":"env:PRIVATE"
        })
        .as_object()
        .unwrap()
        .clone();
        let bound = bind_argv(
            &[temp_binding, stdin_binding],
            &arguments,
            &temp,
            &BTreeSet::from(["secret".to_owned()]),
        )
        .unwrap();
        let path = PathBuf::from(&bound.argv[0]);
        assert!(path.is_file());
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes.len() % 2, 0);
        assert_eq!(
            std::str::from_utf8(bound.stdin.as_ref().unwrap()).unwrap(),
            r#"{"payload":{"value":"한글"}}"#
        );
        drop(bound);
        assert!(!path.exists());
    }
}
