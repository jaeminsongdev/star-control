//! Verified Controller bootstrap without PATH lookup or same-Job fallback.

use std::{
    fs::{File, OpenOptions},
    io::{self, Read},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use star_contracts::{Sha256Hash, parse_no_duplicate_keys};
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControllerInstallManifest {
    pub schema_id: String,
    pub schema_version: u32,
    pub product_version: String,
    pub gateway_sha256: Sha256Hash,
    pub controller_path: String,
    pub controller_sha256: Sha256Hash,
}

pub struct VerifiedControllerImage {
    path: PathBuf,
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
        Self::open(&controller, &manifest.controller_sha256)
    }

    pub fn open(path: &Path, expected_hash: &Sha256Hash) -> Result<Self, ControllerStartError> {
        let path = path.canonicalize()?;
        let lease = open_regular_local_file(&path)?;
        let actual = Sha256Hash::digest_reader(lease.try_clone()?)?;
        if &actual != expected_hash {
            return Err(ControllerStartError::IdentityMismatch);
        }
        Ok(Self {
            path,
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
        let flags = launch_flags(policy)? | CREATE_SUSPENDED.0;
        let application = wide_nul(&self.path.as_os_str().to_string_lossy())?;
        let mut command_line = wide_nul(&format!(
            "\"{}\" --background",
            self.path.as_os_str().to_string_lossy()
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
}

fn load_install_manifest(
    install_directory: &Path,
) -> Result<ControllerInstallManifest, ControllerStartError> {
    let manifest_path = install_directory.join("star-control-install.v1.json");
    let file = open_regular_local_file(&manifest_path)
        .map_err(|_| ControllerStartError::InstallManifest)?;
    let length = file
        .metadata()
        .map_err(|_| ControllerStartError::InstallManifest)?
        .len();
    if length == 0 || length > 64 * 1024 {
        return Err(ControllerStartError::InstallManifest);
    }
    let mut bytes = Vec::with_capacity(length as usize);
    file.take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ControllerStartError::InstallManifest)?;
    if bytes.len() as u64 != length {
        return Err(ControllerStartError::InstallManifest);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| ControllerStartError::InstallManifest)?;
    let value = parse_no_duplicate_keys(text).map_err(|_| ControllerStartError::InstallManifest)?;
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
}
