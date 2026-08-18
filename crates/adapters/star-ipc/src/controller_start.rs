//! Verified Controller bootstrap without PATH lookup or same-Job fallback.
//!
//! A Gateway inside a restrictive outer Job cannot create a durable direct
//! child. In that case only, the verified Controller image is handed to the
//! local WMI process broker and the returned PID is rebound to the leased
//! image before the caller starts IPC readiness polling.

use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{self, Read},
    os::windows::ffi::OsStringExt,
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use star_contracts::{
    Sha256Hash,
    installation::{
        ControllerInstallManifest, RUNTIME_ACTIVATION_RECORD_SCHEMA_ID,
        RUNTIME_GENERATION_MANIFEST_SCHEMA_ID, RuntimeActivationRecord, RuntimeGenerationManifest,
    },
    parse_no_duplicate_keys,
};
use thiserror::Error;
use windows::{
    Win32::{
        Foundation::CloseHandle,
        Storage::FileSystem::{
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
            GetDriveTypeW,
        },
        System::{
            JobObjects::{
                IsProcessInJob, JOB_OBJECT_LIMIT_BREAKAWAY_OK,
                JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JobObjectExtendedLimitInformation, QueryInformationJobObject,
            },
            SystemInformation::GetSystemDirectoryW,
            Threading::{
                CREATE_BREAKAWAY_FROM_JOB, CREATE_NO_WINDOW, CREATE_SUSPENDED, CreateProcessW,
                GetCurrentProcess, PROCESS_INFORMATION, ResumeThread, STARTUPINFOW,
                TerminateProcess,
            },
        },
    },
    core::{HSTRING, PCWSTR, PWSTR},
};

#[derive(Debug, Error)]
pub enum ControllerStartError {
    #[error("installed Controller image identity does not match")]
    IdentityMismatch,
    #[error("installed Controller image cannot be leased")]
    Lease(#[from] io::Error),
    #[error("installed Controller manifest is missing or invalid")]
    InstallManifest,
    #[error("active Runtime Generation record is missing, incompatible, or invalid")]
    RuntimeActivation,
    #[error("outer Job does not allow a durable Controller breakaway")]
    OuterJobDenied,
    #[error("Controller process could not start")]
    Start,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OuterJobPolicy {
    NotInJob,
    BreakawayAllowed,
    Denied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ControllerStartRoute {
    Direct(OuterJobPolicy),
    LocalWmiBroker,
}

const LOCAL_WMI_BROKER_TIMEOUT: Duration = Duration::from_secs(2);
const BROKER_PROCESS_IDENTITY_ATTEMPTS: usize = 20;
const BROKER_PROCESS_IDENTITY_POLL: Duration = Duration::from_millis(50);
const CONTROLLER_BROKER_COMMAND_ENV: &str = "STAR_CONTROL_CONTROLLER_COMMAND";

pub struct VerifiedControllerImage {
    path: PathBuf,
    bootstrap_install_directory: PathBuf,
    lease: File,
    hash: Sha256Hash,
}

impl VerifiedControllerImage {
    pub fn from_install_manifest(gateway: &Path) -> Result<Self, ControllerStartError> {
        let gateway = gateway.canonicalize()?;
        let install_directory = gateway
            .parent()
            .ok_or(ControllerStartError::InstallManifest)?;
        let manifest = load_install_manifest(install_directory)?;
        let gateway_file = open_regular_local_file(&gateway)?;
        let gateway_hash = Sha256Hash::digest_reader(gateway_file)?;
        if gateway_hash != manifest.gateway_sha256 {
            return Err(ControllerStartError::IdentityMismatch);
        }
        Self::from_validated_manifest(install_directory, manifest)
    }

    /// Loads the same frozen install manifest for the management CLI. The CLI
    /// is not the Gateway image named by `gateway_sha256`, but the Controller
    /// path and hash are still selected only from the installed manifest and
    /// held by the same final-handle lease through process creation.
    pub fn from_install_directory(install_directory: &Path) -> Result<Self, ControllerStartError> {
        let install_directory = install_directory.canonicalize()?;
        let manifest = load_install_manifest(&install_directory)?;
        Self::from_validated_manifest(&install_directory, manifest)
    }

    fn from_validated_manifest(
        install_directory: &Path,
        manifest: ControllerInstallManifest,
    ) -> Result<Self, ControllerStartError> {
        match (
            manifest.runtime_activation_record_path.as_deref(),
            manifest.bridge_contract_version,
        ) {
            (Some(record_path), Some(bridge_contract_version)) => {
                return Self::from_runtime_activation(
                    install_directory,
                    Path::new(record_path),
                    bridge_contract_version,
                );
            }
            (None, None) => {}
            _ => return Err(ControllerStartError::InstallManifest),
        }
        let controller = PathBuf::from(&manifest.controller_path);
        if !controller.is_absolute()
            || controller
                .file_name()
                .and_then(|name| name.to_str())
                .is_none_or(|name| !name.eq_ignore_ascii_case("star-controller.exe"))
        {
            return Err(ControllerStartError::InstallManifest);
        }
        let controller = controller.canonicalize()?;
        if controller.parent() != Some(install_directory) {
            return Err(ControllerStartError::InstallManifest);
        }
        Self::open_with_bootstrap(&controller, &manifest.controller_sha256, install_directory)
    }

    fn from_runtime_activation(
        install_directory: &Path,
        activation_record_path: &Path,
        bridge_contract_version: u32,
    ) -> Result<Self, ControllerStartError> {
        let activation = load_runtime_activation(activation_record_path)?;
        if activation.bridge_contract_version != bridge_contract_version {
            return Err(ControllerStartError::RuntimeActivation);
        }
        let runtime_root = canonical_runtime_directory(Path::new(&activation.active.runtime_root))?;
        let generations_root = install_directory.join("runtime").join("generations");
        let generations_root = canonical_runtime_directory(&generations_root)?;
        if !path_is_within(&runtime_root, &generations_root) {
            return Err(ControllerStartError::RuntimeActivation);
        }
        let runtime_manifest = load_runtime_generation_manifest(&runtime_root)?;
        if runtime_manifest.schema_id != RUNTIME_GENERATION_MANIFEST_SCHEMA_ID
            || runtime_manifest.schema_version != 1
            || runtime_manifest.generation.generation_id != activation.active.generation_id
            || runtime_manifest.generation.release_manifest_sha256
                != activation.active.release_manifest_sha256
            || runtime_manifest.bridge_contract_version != bridge_contract_version
        {
            return Err(ControllerStartError::RuntimeActivation);
        }
        let supplied_controller = PathBuf::from(&runtime_manifest.controller_path);
        if supplied_controller
            .file_name()
            .and_then(|name| name.to_str())
            .is_none_or(|name| !name.eq_ignore_ascii_case("star-controller.exe"))
        {
            return Err(ControllerStartError::RuntimeActivation);
        }
        let controller = if supplied_controller.is_absolute() {
            supplied_controller
        } else if supplied_controller
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
        {
            runtime_root.join(supplied_controller)
        } else {
            return Err(ControllerStartError::RuntimeActivation);
        };
        let controller = controller
            .canonicalize()
            .map_err(|_| ControllerStartError::RuntimeActivation)?;
        if !path_is_within(&controller, &runtime_root) {
            return Err(ControllerStartError::RuntimeActivation);
        }
        Self::open_with_bootstrap(
            &controller,
            &runtime_manifest.controller_sha256,
            install_directory,
        )
    }

    pub fn open(path: &Path, expected_hash: &Sha256Hash) -> Result<Self, ControllerStartError> {
        let bootstrap = path.parent().ok_or(ControllerStartError::InstallManifest)?;
        Self::open_with_bootstrap(path, expected_hash, bootstrap)
    }

    fn open_with_bootstrap(
        path: &Path,
        expected_hash: &Sha256Hash,
        bootstrap_install_directory: &Path,
    ) -> Result<Self, ControllerStartError> {
        let path = path.canonicalize()?;
        let bootstrap_install_directory = bootstrap_install_directory.canonicalize()?;
        let lease = open_regular_local_file(&path)?;
        let actual = Sha256Hash::digest_reader(lease.try_clone()?)?;
        if &actual != expected_hash {
            return Err(ControllerStartError::IdentityMismatch);
        }
        Ok(Self {
            path,
            bootstrap_install_directory,
            lease,
            hash: actual,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn hash(&self) -> &Sha256Hash {
        &self.hash
    }

    pub fn start_background(&self) -> Result<u32, ControllerStartError> {
        let policy = current_outer_job_policy()?;
        self.start_background_direct(policy)
    }

    pub fn start_background_durable(&self) -> Result<u32, ControllerStartError> {
        match controller_start_route(current_outer_job_policy()?) {
            ControllerStartRoute::Direct(policy) => self.start_background_direct(policy),
            ControllerStartRoute::LocalWmiBroker => self.start_background_via_local_wmi(),
        }
    }

    fn start_background_direct(&self, policy: OuterJobPolicy) -> Result<u32, ControllerStartError> {
        let flags = launch_flags(policy)? | CREATE_SUSPENDED.0;
        let application = wide_nul(&self.path.as_os_str().to_string_lossy())?;
        let mut command_line = wide_nul(&format!(
            "\"{}\" --background --bootstrap-install-root \"{}\"",
            self.path.as_os_str().to_string_lossy(),
            self.bootstrap_install_directory
                .as_os_str()
                .to_string_lossy(),
        ))?;
        let startup = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut process = PROCESS_INFORMATION::default();
        unsafe {
            CreateProcessW(
                PCWSTR::from_raw(application.as_ptr()),
                Some(PWSTR::from_raw(command_line.as_mut_ptr())),
                None,
                None,
                false,
                windows::Win32::System::Threading::PROCESS_CREATION_FLAGS(flags),
                None,
                PCWSTR::null(),
                &raw const startup,
                &mut process,
            )
        }
        .map_err(|_| ControllerStartError::Start)?;

        let result = (|| {
            let actual = crate::process_identity::process_image(process.dwProcessId)
                .map_err(|_| ControllerStartError::IdentityMismatch)?
                .canonicalize()
                .map_err(|_| ControllerStartError::IdentityMismatch)?;
            if !actual
                .as_os_str()
                .eq_ignore_ascii_case(self.path.as_os_str())
            {
                return Err(ControllerStartError::IdentityMismatch);
            }
            if unsafe { ResumeThread(process.hThread) } == u32::MAX {
                return Err(ControllerStartError::Start);
            }
            Ok(process.dwProcessId)
        })();
        if result.is_err() {
            unsafe {
                let _ = TerminateProcess(process.hProcess, 1);
            }
        }
        unsafe {
            let _ = CloseHandle(process.hThread);
            let _ = CloseHandle(process.hProcess);
        }
        // The no-write/no-delete lease remains held through image creation and
        // actual-image verification, then may be released by the caller.
        let _ = &self.lease;
        result
    }

    fn start_background_via_local_wmi(&self) -> Result<u32, ControllerStartError> {
        let command_line = windows_command_line(&[
            self.path.as_os_str().to_string_lossy().into_owned(),
            "--background".to_owned(),
            "--bootstrap-install-root".to_owned(),
            self.bootstrap_install_directory
                .as_os_str()
                .to_string_lossy()
                .into_owned(),
        ]);
        let powershell = fixed_system_directory()?
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe")
            .canonicalize()
            .map_err(|_| ControllerStartError::Start)?;
        let _powershell_lease =
            open_regular_local_file(&powershell).map_err(|_| ControllerStartError::Start)?;
        let broker_script = concat!(
            "$commandLine=[Environment]::GetEnvironmentVariable('",
            "STAR_CONTROL_CONTROLLER_COMMAND",
            "','Process');",
            "$result=Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{CommandLine=$commandLine};",
            "if($result.ReturnValue -ne 0){exit [int]$result.ReturnValue};",
            "[Console]::Write($result.ProcessId)"
        );
        let mut child = Command::new(&powershell)
            .args(["-NoProfile", "-NonInteractive", "-Command", broker_script])
            .env(CONTROLLER_BROKER_COMMAND_ENV, command_line)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW.0)
            .spawn()
            .map_err(|_| ControllerStartError::Start)?;
        let deadline = Instant::now() + LOCAL_WMI_BROKER_TIMEOUT;
        loop {
            if child
                .try_wait()
                .map_err(|_| ControllerStartError::Start)?
                .is_some()
            {
                break;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ControllerStartError::Start);
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let output = child
            .wait_with_output()
            .map_err(|_| ControllerStartError::Start)?;
        if !output.status.success() {
            return Err(ControllerStartError::Start);
        }
        let pid = std::str::from_utf8(&output.stdout)
            .ok()
            .and_then(|stdout| stdout.trim().parse::<u32>().ok())
            .filter(|pid| *pid != 0)
            .ok_or(ControllerStartError::Start)?;
        for _ in 0..BROKER_PROCESS_IDENTITY_ATTEMPTS {
            match crate::process_identity::process_image(pid) {
                Ok(actual) => match actual.canonicalize() {
                    Ok(actual)
                        if actual
                            .as_os_str()
                            .eq_ignore_ascii_case(self.path.as_os_str()) =>
                    {
                        return Ok(pid);
                    }
                    Ok(_) => return Err(ControllerStartError::IdentityMismatch),
                    Err(_) => std::thread::sleep(BROKER_PROCESS_IDENTITY_POLL),
                },
                Err(_) => std::thread::sleep(BROKER_PROCESS_IDENTITY_POLL),
            }
        }
        Err(ControllerStartError::Start)
    }
}

fn fixed_system_directory() -> Result<PathBuf, ControllerStartError> {
    // Windows paths are bounded below the 32,767 UTF-16 code-unit extended
    // path limit. Resolve the native system directory from Kernel32 instead
    // of trusting an inherited SystemRoot environment value.
    let mut buffer = vec![0_u16; 32_768];
    let length = unsafe { GetSystemDirectoryW(Some(&mut buffer)) } as usize;
    if length == 0 || length >= buffer.len() {
        return Err(ControllerStartError::Start);
    }
    buffer.truncate(length);
    let directory = PathBuf::from(OsString::from_wide(&buffer));
    directory
        .canonicalize()
        .map_err(|_| ControllerStartError::Start)
}

fn load_install_manifest(
    install_directory: &Path,
) -> Result<ControllerInstallManifest, ControllerStartError> {
    let manifest_path = install_directory.join("star-control-install.v1.json");
    let value =
        load_strict_json(&manifest_path).map_err(|_| ControllerStartError::InstallManifest)?;
    let manifest: ControllerInstallManifest =
        serde_json::from_value(value).map_err(|_| ControllerStartError::InstallManifest)?;
    if manifest.schema_id != "star.controller-install-manifest"
        || manifest.schema_version != 1
        || manifest.product_version != env!("CARGO_PKG_VERSION")
        || semver::Version::parse(&manifest.product_version).is_err()
    {
        return Err(ControllerStartError::InstallManifest);
    }
    Ok(manifest)
}

fn load_runtime_activation(path: &Path) -> Result<RuntimeActivationRecord, ControllerStartError> {
    let value = load_strict_json(path).map_err(|_| ControllerStartError::RuntimeActivation)?;
    let record: RuntimeActivationRecord =
        serde_json::from_value(value).map_err(|_| ControllerStartError::RuntimeActivation)?;
    if record.schema_id != RUNTIME_ACTIVATION_RECORD_SCHEMA_ID || record.schema_version != 1 {
        return Err(ControllerStartError::RuntimeActivation);
    }
    Ok(record)
}

fn load_runtime_generation_manifest(
    runtime_root: &Path,
) -> Result<RuntimeGenerationManifest, ControllerStartError> {
    let value = load_strict_json(&runtime_root.join("runtime-generation.v1.json"))
        .map_err(|_| ControllerStartError::RuntimeActivation)?;
    serde_json::from_value(value).map_err(|_| ControllerStartError::RuntimeActivation)
}

fn load_strict_json(path: &Path) -> Result<serde_json::Value, ControllerStartError> {
    let file =
        open_regular_local_file(path).map_err(|_| ControllerStartError::RuntimeActivation)?;
    let length = file
        .metadata()
        .map_err(|_| ControllerStartError::RuntimeActivation)?
        .len();
    if length == 0 || length > 64 * 1024 {
        return Err(ControllerStartError::RuntimeActivation);
    }
    let mut bytes = Vec::with_capacity(length as usize);
    file.take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ControllerStartError::RuntimeActivation)?;
    if bytes.len() as u64 != length {
        return Err(ControllerStartError::RuntimeActivation);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| ControllerStartError::RuntimeActivation)?;
    parse_no_duplicate_keys(text).map_err(|_| ControllerStartError::RuntimeActivation)
}

fn canonical_runtime_directory(path: &Path) -> Result<PathBuf, ControllerStartError> {
    if !path.is_absolute()
        || !path.is_dir()
        || !is_fixed_drive_path(path)
        || std::fs::symlink_metadata(path).ok().is_none_or(|metadata| {
            !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
        })
    {
        return Err(ControllerStartError::RuntimeActivation);
    }
    path.canonicalize()
        .map_err(|_| ControllerStartError::RuntimeActivation)
}

fn path_is_within(path: &Path, root: &Path) -> bool {
    path.ancestors()
        .any(|ancestor| ancestor.as_os_str().eq_ignore_ascii_case(root.as_os_str()))
}

fn open_regular_local_file(path: &Path) -> Result<File, io::Error> {
    if !path.is_absolute()
        || !path.is_file()
        || !is_fixed_drive_path(path)
        || std::fs::symlink_metadata(path).ok().is_none_or(|metadata| {
            !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "installed file is not a regular local fixed-volume file",
        ));
    }
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ.0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)?;
    if file.metadata().ok().is_none_or(|metadata| {
        !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
    }) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "installed file identity changed while opening",
        ));
    }
    Ok(file)
}

fn is_fixed_drive_path(path: &Path) -> bool {
    use std::path::{Component, Prefix};
    let drive = match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => Some(letter),
            _ => None,
        },
        _ => None,
    };
    drive.is_some_and(|letter| {
        let root = HSTRING::from(format!("{}:\\", char::from(letter)));
        unsafe { GetDriveTypeW(&root) == windows::Win32::System::WindowsProgramming::DRIVE_FIXED }
    })
}

fn wide_nul(value: &str) -> Result<Vec<u16>, ControllerStartError> {
    if value.contains('\0') {
        return Err(ControllerStartError::Start);
    }
    Ok(value.encode_utf16().chain(std::iter::once(0)).collect())
}

pub fn classify_outer_job(in_job: bool, limit_flags: u32) -> OuterJobPolicy {
    if !in_job {
        OuterJobPolicy::NotInJob
    } else if limit_flags
        & (JOB_OBJECT_LIMIT_BREAKAWAY_OK.0 | JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK.0)
        != 0
    {
        OuterJobPolicy::BreakawayAllowed
    } else {
        OuterJobPolicy::Denied
    }
}

fn controller_start_route(policy: OuterJobPolicy) -> ControllerStartRoute {
    match policy {
        OuterJobPolicy::NotInJob | OuterJobPolicy::BreakawayAllowed => {
            ControllerStartRoute::Direct(policy)
        }
        OuterJobPolicy::Denied => ControllerStartRoute::LocalWmiBroker,
    }
}

fn windows_command_line(arguments: &[String]) -> String {
    arguments
        .iter()
        .map(|argument| quote_windows_argument(argument))
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote_windows_argument(argument: &str) -> String {
    if !argument.is_empty() && !argument.contains([' ', '\t', '\n', '\r', '"']) {
        return argument.to_owned();
    }
    let mut quoted = String::from("\"");
    let mut backslashes = 0;
    for character in argument.chars() {
        if character == '\\' {
            backslashes += 1;
        } else if character == '"' {
            quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
            quoted.push('"');
            backslashes = 0;
        } else {
            quoted.push_str(&"\\".repeat(backslashes));
            backslashes = 0;
            quoted.push(character);
        }
    }
    quoted.push_str(&"\\".repeat(backslashes * 2));
    quoted.push('"');
    quoted
}

pub fn launch_flags(policy: OuterJobPolicy) -> Result<u32, ControllerStartError> {
    Ok(match policy {
        OuterJobPolicy::NotInJob => CREATE_NO_WINDOW.0,
        OuterJobPolicy::BreakawayAllowed => CREATE_NO_WINDOW.0 | CREATE_BREAKAWAY_FROM_JOB.0,
        OuterJobPolicy::Denied => return Err(ControllerStartError::OuterJobDenied),
    })
}

pub fn current_outer_job_policy() -> Result<OuterJobPolicy, ControllerStartError> {
    let mut in_job = windows::core::BOOL(0);
    unsafe { IsProcessInJob(GetCurrentProcess(), None, &mut in_job) }
        .map_err(|_| ControllerStartError::OuterJobDenied)?;
    if !in_job.as_bool() {
        return Ok(OuterJobPolicy::NotInJob);
    }
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    unsafe {
        QueryInformationJobObject(
            None,
            JobObjectExtendedLimitInformation,
            (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            None,
        )
    }
    .map_err(|_| ControllerStartError::OuterJobDenied)?;
    Ok(classify_outer_job(
        true,
        limits.BasicLimitInformation.LimitFlags.0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_json(product_version: &str, extra: &str) -> String {
        format!(
            r#"{{
                "schema_id":"star.controller-install-manifest",
                "schema_version":1,
                "product_version":"{product_version}",
                "gateway_sha256":"sha256:{zero}",
                "controller_path":"C:\\Program Files\\Star-Control\\star-controller.exe",
                "controller_sha256":"sha256:{zero}"{extra}
            }}"#,
            zero = "0".repeat(64)
        )
    }

    fn manifest_directory() -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("star-controller-manifest-{}", crate::nonce()));
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    // matrix: MCP-I013
    fn verified_image_lease_prevents_path_replacement_and_mismatch_never_starts() {
        let directory =
            std::env::temp_dir().join(format!("star-controller-image-{}", crate::nonce()));
        std::fs::create_dir_all(&directory).unwrap();
        let installed = directory.join("star-controller.exe");
        let replacement = directory.join("replacement.exe");
        std::fs::copy(std::env::current_exe().unwrap(), &installed).unwrap();
        std::fs::write(&replacement, b"different executable bytes").unwrap();
        let expected = Sha256Hash::digest_reader(File::open(&installed).unwrap()).unwrap();
        let lease = VerifiedControllerImage::open(&installed, &expected).unwrap();
        assert_eq!(lease.path(), installed.canonicalize().unwrap());
        assert_eq!(lease.hash(), &expected);
        assert!(std::fs::rename(&replacement, &installed).is_err());
        assert!(matches!(
            VerifiedControllerImage::open(&installed, &Sha256Hash::digest(b"wrong")),
            Err(ControllerStartError::IdentityMismatch)
        ));
        drop(lease);
        assert!(std::fs::rename(&replacement, &installed).is_ok());
    }

    #[test]
    // matrix: MCP-I014
    fn outer_job_policy_uses_breakaway_or_fails_before_same_job_start() {
        assert_eq!(
            launch_flags(classify_outer_job(false, 0)).unwrap(),
            CREATE_NO_WINDOW.0
        );
        assert_ne!(
            launch_flags(classify_outer_job(true, JOB_OBJECT_LIMIT_BREAKAWAY_OK.0)).unwrap()
                & CREATE_BREAKAWAY_FROM_JOB.0,
            0
        );
        assert!(matches!(
            launch_flags(classify_outer_job(true, 0)),
            Err(ControllerStartError::OuterJobDenied)
        ));
        assert_eq!(
            controller_start_route(classify_outer_job(true, 0)),
            ControllerStartRoute::LocalWmiBroker
        );
        assert!(matches!(
            controller_start_route(classify_outer_job(true, JOB_OBJECT_LIMIT_BREAKAWAY_OK.0)),
            ControllerStartRoute::Direct(OuterJobPolicy::BreakawayAllowed)
        ));
    }

    #[test]
    fn local_broker_command_line_quotes_only_the_verified_controller_inputs() {
        let command = windows_command_line(&[
            r"C:\Program Files\Star-Control\star-controller.exe".to_owned(),
            "--background".to_owned(),
            "--bootstrap-install-root".to_owned(),
            r"D:\개발 도구\Star-Control".to_owned(),
        ]);
        assert_eq!(
            command,
            r#""C:\Program Files\Star-Control\star-controller.exe" --background --bootstrap-install-root "D:\개발 도구\Star-Control""#
        );
        assert!(!command.contains("cmd.exe"));
        assert!(!command.contains("powershell.exe"));
    }

    #[test]
    // matrix: MCP-I013
    fn install_manifest_rejects_wrong_product_unknown_and_duplicate_fields() {
        let directory = manifest_directory();
        let path = directory.join("star-control-install.v1.json");

        std::fs::write(&path, manifest_json("999.0.0", "")).unwrap();
        assert!(matches!(
            load_install_manifest(&directory),
            Err(ControllerStartError::InstallManifest)
        ));

        std::fs::write(
            &path,
            manifest_json(env!("CARGO_PKG_VERSION"), r#", "unknown":true"#),
        )
        .unwrap();
        assert!(matches!(
            load_install_manifest(&directory),
            Err(ControllerStartError::InstallManifest)
        ));

        std::fs::write(
            &path,
            manifest_json(env!("CARGO_PKG_VERSION"), r#", "schema_version":1"#),
        )
        .unwrap();
        assert!(matches!(
            load_install_manifest(&directory),
            Err(ControllerStartError::InstallManifest)
        ));

        std::fs::write(&path, manifest_json(env!("CARGO_PKG_VERSION"), "")).unwrap();
        let manifest = load_install_manifest(&directory).unwrap();
        assert_eq!(manifest.product_version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    // matrix: MCP-I013 MCP-S006
    fn install_manifest_is_bounded_and_must_be_a_regular_fixed_volume_file() {
        let directory = manifest_directory();
        let path = directory.join("star-control-install.v1.json");

        std::fs::write(&path, vec![b' '; 64 * 1024 + 1]).unwrap();
        assert!(matches!(
            load_install_manifest(&directory),
            Err(ControllerStartError::InstallManifest)
        ));

        std::fs::write(&path, manifest_json(env!("CARGO_PKG_VERSION"), "")).unwrap();
        assert!(load_install_manifest(&directory).is_ok());
        assert!(open_regular_local_file(Path::new(r"\\server\share\controller.exe")).is_err());
    }

    #[test]
    fn runtime_activation_selects_only_a_generation_under_the_install_root() {
        use star_contracts::installation::{RuntimeGenerationRef, TargetArchitecture};

        let install_root = manifest_directory();
        let runtime_root = install_root
            .join("runtime")
            .join("generations")
            .join("rt_active");
        std::fs::create_dir_all(&runtime_root).unwrap();
        let controller = runtime_root.join("star-controller.exe");
        std::fs::copy(std::env::current_exe().unwrap(), &controller).unwrap();
        let controller_sha256 =
            Sha256Hash::digest_reader(File::open(&controller).unwrap()).unwrap();
        let release_manifest_sha256 = Sha256Hash::digest(b"release-manifest");
        let generation = RuntimeGenerationRef {
            generation_id: "rt_active".to_owned(),
            runtime_root: runtime_root.canonicalize().unwrap().display().to_string(),
            release_manifest_sha256,
        };
        let generation_manifest = RuntimeGenerationManifest {
            schema_id: RUNTIME_GENERATION_MANIFEST_SCHEMA_ID.to_owned(),
            schema_version: 1,
            generation: generation.clone(),
            product_version: env!("CARGO_PKG_VERSION").to_owned(),
            target_architecture: TargetArchitecture::X64,
            controller_path: controller.canonicalize().unwrap().display().to_string(),
            controller_sha256,
            cli_runtime_path: runtime_root
                .join("star-cli-runtime.exe")
                .display()
                .to_string(),
            catalog_path: runtime_root.join("catalog").display().to_string(),
            schemas_root: runtime_root.join("schemas").display().to_string(),
            bridge_contract_version: 2,
        };
        std::fs::write(
            runtime_root.join("runtime-generation.v1.json"),
            serde_json::to_vec(&generation_manifest).unwrap(),
        )
        .unwrap();
        let activation: RuntimeActivationRecord = serde_json::from_value(serde_json::json!({
            "schema_id":RUNTIME_ACTIVATION_RECORD_SCHEMA_ID,
            "schema_version":1,
            "activation_revision":1,
            "active":generation,
            "previous":null,
            "state_generation_id":"state_1",
            "bridge_contract_version":2,
            "activated_at":"2026-07-18T00:00:00Z"
        }))
        .unwrap();
        let activation_path = install_root.join("active-runtime.v1.json");
        std::fs::write(&activation_path, serde_json::to_vec(&activation).unwrap()).unwrap();

        let image = VerifiedControllerImage::from_runtime_activation(
            &install_root.canonicalize().unwrap(),
            &activation_path,
            2,
        )
        .unwrap();
        assert_eq!(image.path(), controller.canonicalize().unwrap());

        let mut outside = activation;
        outside.active.runtime_root = install_root.display().to_string();
        std::fs::write(&activation_path, serde_json::to_vec(&outside).unwrap()).unwrap();
        assert!(matches!(
            VerifiedControllerImage::from_runtime_activation(
                &install_root.canonicalize().unwrap(),
                &activation_path,
                2,
            ),
            Err(ControllerStartError::RuntimeActivation)
        ));
    }
}
