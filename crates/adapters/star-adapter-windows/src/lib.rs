//! Windows fixed-volume file and current-user installation adapter.

#![cfg(windows)]

use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::windows::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Component, Path, PathBuf, Prefix},
};

use chrono::Utc;
use serde::Serialize;
use star_contracts::{
    InstallationId, Sha256Hash, canonical_sha256,
    installation::{
        CODEX_INTEGRATION_RECORD_SCHEMA_ID, CodexIntegrationRecord, CodexIntegrationSummary,
        ControllerInstallManifest, INSTALLATION_RECORD_SCHEMA_ID, INSTALLATION_SCHEMA_VERSION,
        InstallationRecord, RELEASE_FILE_MANIFEST_SCHEMA_ID, ReleaseFileEntry, ReleaseFileManifest,
        TargetArchitecture,
    },
    parse_no_duplicate_keys,
};
use thiserror::Error;
use windows::{
    Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, GetDriveTypeW,
        MOVE_FILE_FLAGS, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    },
    core::{HSTRING, PCWSTR},
};

const RELEASE_MANIFEST_MAX_BYTES: u64 = 4 * 1024 * 1024;
const LOCAL_RECORD_MAX_BYTES: u64 = 64 * 1024;
pub const RELEASE_MANIFEST_FILE: &str = "release-manifest.json";
pub const INSTALLATION_RECORD_FILE: &str = "installation-record.v1.json";
pub const CONTROLLER_INSTALL_MANIFEST_FILE: &str = "star-control-install.v1.json";

pub mod autostart;

#[derive(Debug, Error)]
pub enum WindowsAdapterError {
    #[error("installation path is not a regular local fixed-volume path")]
    UnsafePath,
    #[error("release-file manifest is missing, malformed or unsupported")]
    InvalidReleaseManifest,
    #[error("release-file manifest architecture does not match this executable")]
    ArchitectureMismatch,
    #[error("release-file manifest file identity does not match")]
    FileIdentityMismatch,
    #[error("another installation record owns a different install root")]
    InstallationConflict,
    #[error("installation record is missing, malformed or unsupported")]
    InvalidInstallationRecord,
    #[error("Codex integration record is malformed or unsupported")]
    InvalidIntegrationRecord,
    #[error("required current-user environment variable is unavailable")]
    Environment,
    #[error("Windows installation I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("JSON serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Serialize)]
pub struct InstallationStatus {
    pub verified: bool,
    pub install_root: String,
    pub release_manifest_path: String,
    pub installation_record_path: String,
    pub product_version: String,
    pub target_architecture: TargetArchitecture,
    pub codex_integration: Option<CodexIntegrationSummary>,
}

#[derive(Clone, Debug)]
pub struct InstallationManager {
    local_data_root: PathBuf,
}

impl InstallationManager {
    pub fn for_current_user() -> Result<Self, WindowsAdapterError> {
        let root = std::env::var_os("LOCALAPPDATA").ok_or(WindowsAdapterError::Environment)?;
        Ok(Self::new(PathBuf::from(root).join("Star-Control")))
    }

    pub fn new(local_data_root: PathBuf) -> Self {
        Self { local_data_root }
    }

    pub fn local_data_root(&self) -> &Path {
        &self.local_data_root
    }

    pub fn installation_record_path(&self) -> PathBuf {
        self.local_data_root
            .join("installation")
            .join(INSTALLATION_RECORD_FILE)
    }

    pub fn finalize(
        &self,
        install_root: &Path,
        requested_architecture: TargetArchitecture,
        replace_existing: bool,
    ) -> Result<InstallationRecord, WindowsAdapterError> {
        let install_root = canonical_fixed_directory(install_root)?;
        let manifest_bytes = read_regular_bounded(
            &install_root.join(RELEASE_MANIFEST_FILE),
            RELEASE_MANIFEST_MAX_BYTES,
        )?;
        let manifest = parse_release_manifest(&manifest_bytes)?;
        validate_release_manifest(&manifest, requested_architecture)?;
        verify_release_files(&install_root, &manifest)?;

        let record_path = self.prepared_installation_record_path()?;
        let previous = if record_path.exists() {
            Some(load_installation_record(&record_path)?)
        } else {
            None
        };
        let same_root = previous.as_ref().is_some_and(|record| {
            paths_equal_case_insensitive(Path::new(&record.install_root), &install_root)
        });
        if previous.is_some() && !same_root && !replace_existing {
            return Err(WindowsAdapterError::InstallationConflict);
        }
        write_controller_manifest(&install_root, &manifest)?;
        let now = Utc::now();
        let record = InstallationRecord {
            schema_id: INSTALLATION_RECORD_SCHEMA_ID.to_owned(),
            schema_version: INSTALLATION_SCHEMA_VERSION,
            installation_id: previous
                .as_ref()
                .filter(|_| same_root)
                .map_or_else(InstallationId::new, |record| record.installation_id.clone()),
            product_version: manifest.product_version.clone(),
            target_architecture: manifest.target_architecture,
            install_root: normal_windows_path(&install_root)
                .to_string_lossy()
                .into_owned(),
            release_manifest_sha256: Sha256Hash::digest(&manifest_bytes),
            installed_at: previous
                .as_ref()
                .filter(|_| same_root)
                .map_or(now, |record| record.installed_at),
            updated_at: now,
            codex_integration: previous
                .filter(|_| same_root)
                .and_then(|record| record.codex_integration),
        };
        atomic_write_json(&record_path, &record)?;
        Ok(record)
    }

    pub fn status(&self, install_root: &Path) -> Result<InstallationStatus, WindowsAdapterError> {
        let install_root = canonical_fixed_directory(install_root)?;
        let record_path = self.existing_installation_record_path()?;
        let record = load_installation_record(&record_path)?;
        if !paths_equal_case_insensitive(Path::new(&record.install_root), &install_root) {
            return Err(WindowsAdapterError::InstallationConflict);
        }
        let manifest_path = install_root.join(RELEASE_MANIFEST_FILE);
        let manifest_bytes = read_regular_bounded(&manifest_path, RELEASE_MANIFEST_MAX_BYTES)?;
        if Sha256Hash::digest(&manifest_bytes) != record.release_manifest_sha256 {
            return Err(WindowsAdapterError::FileIdentityMismatch);
        }
        let manifest = parse_release_manifest(&manifest_bytes)?;
        validate_release_manifest(&manifest, record.target_architecture)?;
        if record.product_version != manifest.product_version {
            return Err(WindowsAdapterError::FileIdentityMismatch);
        }
        verify_release_files(&install_root, &manifest)?;
        let controller_manifest = read_regular_bounded(
            &install_root.join(CONTROLLER_INSTALL_MANIFEST_FILE),
            LOCAL_RECORD_MAX_BYTES,
        )?;
        parse_controller_manifest(&controller_manifest, &install_root, &manifest)?;
        Ok(InstallationStatus {
            verified: true,
            install_root: normal_windows_path(&install_root)
                .to_string_lossy()
                .into_owned(),
            release_manifest_path: normal_windows_path(&manifest_path)
                .to_string_lossy()
                .into_owned(),
            installation_record_path: normal_windows_path(&record_path)
                .to_string_lossy()
                .into_owned(),
            product_version: record.product_version,
            target_architecture: record.target_architecture,
            codex_integration: record.codex_integration,
        })
    }

    pub fn set_codex_integration(
        &self,
        install_root: &Path,
        summary: Option<CodexIntegrationSummary>,
    ) -> Result<InstallationRecord, WindowsAdapterError> {
        let install_root = canonical_fixed_directory(install_root)?;
        let path = self.existing_installation_record_path()?;
        let mut record = load_installation_record(&path)?;
        if !paths_equal_case_insensitive(Path::new(&record.install_root), &install_root) {
            return Err(WindowsAdapterError::InstallationConflict);
        }
        record.codex_integration = summary;
        record.updated_at = Utc::now();
        atomic_write_json(&path, &record)?;
        Ok(record)
    }

    fn prepared_installation_record_path(&self) -> Result<PathBuf, WindowsAdapterError> {
        Ok(
            ensure_fixed_directory(&self.local_data_root.join("installation"))?
                .join(INSTALLATION_RECORD_FILE),
        )
    }

    fn existing_installation_record_path(&self) -> Result<PathBuf, WindowsAdapterError> {
        Ok(
            canonical_fixed_directory(&self.local_data_root.join("installation"))?
                .join(INSTALLATION_RECORD_FILE),
        )
    }
}

pub fn compiled_architecture() -> Result<TargetArchitecture, WindowsAdapterError> {
    match std::env::consts::ARCH {
        "x86_64" => Ok(TargetArchitecture::X64),
        "aarch64" => Ok(TargetArchitecture::Arm64),
        _ => Err(WindowsAdapterError::ArchitectureMismatch),
    }
}

pub fn load_installation_record(path: &Path) -> Result<InstallationRecord, WindowsAdapterError> {
    let bytes = read_regular_bounded(path, LOCAL_RECORD_MAX_BYTES)
        .map_err(|_| WindowsAdapterError::InvalidInstallationRecord)?;
    let value = strict_value(&bytes).map_err(|_| WindowsAdapterError::InvalidInstallationRecord)?;
    let record: InstallationRecord = serde_json::from_value(value)
        .map_err(|_| WindowsAdapterError::InvalidInstallationRecord)?;
    if record.schema_id != INSTALLATION_RECORD_SCHEMA_ID
        || record.schema_version != INSTALLATION_SCHEMA_VERSION
        || semver::Version::parse(&record.product_version).is_err()
        || !Path::new(&record.install_root).is_absolute()
    {
        return Err(WindowsAdapterError::InvalidInstallationRecord);
    }
    Ok(record)
}

pub fn load_codex_integration_record(
    path: &Path,
) -> Result<CodexIntegrationRecord, WindowsAdapterError> {
    let bytes = read_regular_bounded(path, LOCAL_RECORD_MAX_BYTES)
        .map_err(|_| WindowsAdapterError::InvalidIntegrationRecord)?;
    let value = strict_value(&bytes).map_err(|_| WindowsAdapterError::InvalidIntegrationRecord)?;
    let record: CodexIntegrationRecord =
        serde_json::from_value(value).map_err(|_| WindowsAdapterError::InvalidIntegrationRecord)?;
    if record.schema_id != CODEX_INTEGRATION_RECORD_SCHEMA_ID
        || record.schema_version != INSTALLATION_SCHEMA_VERSION
        || semver::Version::parse(&record.product_version).is_err()
        || !Path::new(&record.install_root).is_absolute()
        || !Path::new(&record.integration_root).is_absolute()
        || !Path::new(&record.marketplace_root).is_absolute()
    {
        return Err(WindowsAdapterError::InvalidIntegrationRecord);
    }
    Ok(record)
}

pub fn atomic_write_json(path: &Path, value: &impl Serialize) -> Result<(), WindowsAdapterError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    atomic_write(path, &bytes)
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), WindowsAdapterError> {
    let parent = path.parent().ok_or(WindowsAdapterError::UnsafePath)?;
    let parent = ensure_fixed_directory(parent)?;
    let mut temporary = None;
    for sequence in 0..32_u32 {
        let name = format!(
            ".{}.{}.{}.tmp",
            path.file_name()
                .and_then(OsStr::to_str)
                .unwrap_or("star-control"),
            std::process::id(),
            sequence
        );
        let candidate = parent.join(name);
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                file.write_all(bytes)?;
                file.sync_all()?;
                temporary = Some(candidate);
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    let temporary = temporary.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "no atomic temp name available",
        )
    })?;
    let result = move_replace(&temporary, path);
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

pub fn open_regular_local_file(path: &Path) -> Result<File, io::Error> {
    if !path.is_absolute()
        || !path.is_file()
        || !is_fixed_drive_path(path)
        || has_reparse_ancestor(path)
        || std::fs::symlink_metadata(path).ok().is_none_or(|metadata| {
            !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "file is not a regular local fixed-volume file",
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
            "file identity changed while opening",
        ));
    }
    Ok(file)
}

pub fn is_fixed_drive_path(path: &Path) -> bool {
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

pub fn normal_windows_path(path: &Path) -> PathBuf {
    let value = path.as_os_str().to_string_lossy();
    value
        .strip_prefix(r"\\?\")
        .filter(|rest| {
            rest.as_bytes().get(1) == Some(&b':')
                && rest.as_bytes().get(2).is_some_and(|value| *value == b'\\')
        })
        .map_or_else(|| path.to_path_buf(), PathBuf::from)
}

fn parse_release_manifest(bytes: &[u8]) -> Result<ReleaseFileManifest, WindowsAdapterError> {
    let value = strict_value(bytes).map_err(|_| WindowsAdapterError::InvalidReleaseManifest)?;
    serde_json::from_value(value).map_err(|_| WindowsAdapterError::InvalidReleaseManifest)
}

fn validate_release_manifest(
    manifest: &ReleaseFileManifest,
    requested_architecture: TargetArchitecture,
) -> Result<(), WindowsAdapterError> {
    if manifest.schema_id != RELEASE_FILE_MANIFEST_SCHEMA_ID
        || manifest.schema_version != INSTALLATION_SCHEMA_VERSION
        || manifest.product_version != env!("CARGO_PKG_VERSION")
        || semver::Version::parse(&manifest.product_version).is_err()
        || manifest.source_revision.is_empty()
        || manifest.source_revision.len() > 256
        || manifest.target_architecture != requested_architecture
        || manifest.target_architecture != compiled_architecture()?
        || manifest.signing != star_contracts::installation::PackageSigningState::UnsignedLocal
        || manifest.generated_files != [CONTROLLER_INSTALL_MANIFEST_FILE]
        || manifest.files.is_empty()
    {
        return Err(WindowsAdapterError::InvalidReleaseManifest);
    }
    let mut previous: Option<&str> = None;
    let mut casefolded = BTreeSet::new();
    for entry in &manifest.files {
        if !valid_manifest_relative_path(&entry.path)
            || previous.is_some_and(|value| value >= entry.path.as_str())
            || !casefolded.insert(entry.path.to_ascii_lowercase())
        {
            return Err(WindowsAdapterError::InvalidReleaseManifest);
        }
        previous = Some(&entry.path);
    }
    for required in ["star.exe", "star-controller.exe", "star-mcp.exe"] {
        if !manifest.files.iter().any(|entry| entry.path == required) {
            return Err(WindowsAdapterError::InvalidReleaseManifest);
        }
    }
    let value = serde_json::to_value(&manifest.files)?;
    if canonical_sha256(&value).map_err(|_| WindowsAdapterError::InvalidReleaseManifest)?
        != manifest.set_sha256
    {
        return Err(WindowsAdapterError::InvalidReleaseManifest);
    }
    Ok(())
}

fn verify_release_files(
    install_root: &Path,
    manifest: &ReleaseFileManifest,
) -> Result<(), WindowsAdapterError> {
    for entry in &manifest.files {
        let path = install_root.join(entry.path.replace('/', "\\"));
        let file = open_regular_local_file(&path)
            .map_err(|_| WindowsAdapterError::FileIdentityMismatch)?;
        let metadata = file
            .metadata()
            .map_err(|_| WindowsAdapterError::FileIdentityMismatch)?;
        if metadata.len() != entry.size
            || Sha256Hash::digest_reader(file)
                .map_err(|_| WindowsAdapterError::FileIdentityMismatch)?
                != entry.sha256
        {
            return Err(WindowsAdapterError::FileIdentityMismatch);
        }
    }
    Ok(())
}

fn write_controller_manifest(
    install_root: &Path,
    release: &ReleaseFileManifest,
) -> Result<(), WindowsAdapterError> {
    let entry = |name: &str| -> Result<&ReleaseFileEntry, WindowsAdapterError> {
        release
            .files
            .iter()
            .find(|entry| entry.path == name)
            .ok_or(WindowsAdapterError::InvalidReleaseManifest)
    };
    let controller_path = install_root
        .join("star-controller.exe")
        .canonicalize()
        .map_err(|_| WindowsAdapterError::FileIdentityMismatch)?;
    let manifest = ControllerInstallManifest {
        schema_id: "star.controller-install-manifest".to_owned(),
        schema_version: 1,
        product_version: release.product_version.clone(),
        gateway_sha256: entry("star-mcp.exe")?.sha256.clone(),
        controller_path: normal_windows_path(&controller_path)
            .to_string_lossy()
            .into_owned(),
        controller_sha256: entry("star-controller.exe")?.sha256.clone(),
    };
    atomic_write_json(
        &install_root.join(CONTROLLER_INSTALL_MANIFEST_FILE),
        &manifest,
    )
}

fn parse_controller_manifest(
    bytes: &[u8],
    install_root: &Path,
    release: &ReleaseFileManifest,
) -> Result<ControllerInstallManifest, WindowsAdapterError> {
    let value = strict_value(bytes).map_err(|_| WindowsAdapterError::FileIdentityMismatch)?;
    let manifest: ControllerInstallManifest =
        serde_json::from_value(value).map_err(|_| WindowsAdapterError::FileIdentityMismatch)?;
    let expected_controller = install_root
        .join("star-controller.exe")
        .canonicalize()
        .map_err(|_| WindowsAdapterError::FileIdentityMismatch)?;
    let gateway = release
        .files
        .iter()
        .find(|entry| entry.path == "star-mcp.exe")
        .ok_or(WindowsAdapterError::InvalidReleaseManifest)?;
    let controller = release
        .files
        .iter()
        .find(|entry| entry.path == "star-controller.exe")
        .ok_or(WindowsAdapterError::InvalidReleaseManifest)?;
    if manifest.schema_id != "star.controller-install-manifest"
        || manifest.schema_version != 1
        || manifest.product_version != release.product_version
        || manifest.gateway_sha256 != gateway.sha256
        || manifest.controller_sha256 != controller.sha256
        || !paths_equal_case_insensitive(Path::new(&manifest.controller_path), &expected_controller)
    {
        return Err(WindowsAdapterError::FileIdentityMismatch);
    }
    Ok(manifest)
}

pub fn ensure_fixed_directory(path: &Path) -> Result<PathBuf, WindowsAdapterError> {
    if !path.is_absolute() || !is_fixed_drive_path(path) {
        return Err(WindowsAdapterError::UnsafePath);
    }
    let existing_ancestor = path
        .ancestors()
        .find(|ancestor| ancestor.exists())
        .ok_or(WindowsAdapterError::UnsafePath)?;
    canonical_fixed_directory(existing_ancestor)?;
    std::fs::create_dir_all(path)?;
    canonical_fixed_directory(path)
}

pub fn canonical_fixed_directory(path: &Path) -> Result<PathBuf, WindowsAdapterError> {
    if !path.is_absolute()
        || !path.is_dir()
        || !is_fixed_drive_path(path)
        || has_reparse_ancestor(path)
    {
        return Err(WindowsAdapterError::UnsafePath);
    }
    let canonical = path.canonicalize()?;
    if !is_fixed_drive_path(&canonical) || has_reparse_ancestor(&canonical) {
        return Err(WindowsAdapterError::UnsafePath);
    }
    Ok(canonical)
}

fn has_reparse_ancestor(path: &Path) -> bool {
    path.ancestors()
        .filter(|ancestor| !ancestor.as_os_str().is_empty())
        .any(|ancestor| {
            std::fs::symlink_metadata(ancestor)
                .ok()
                .is_none_or(|metadata| {
                    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
                })
        })
}

fn valid_manifest_relative_path(value: &str) -> bool {
    !value.is_empty()
        && !value.contains('\\')
        && !value.contains(':')
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn paths_equal_case_insensitive(left: &Path, right: &Path) -> bool {
    let left = left.canonicalize().unwrap_or_else(|_| left.to_path_buf());
    let right = right.canonicalize().unwrap_or_else(|_| right.to_path_buf());
    left.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
}

fn read_regular_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, WindowsAdapterError> {
    let mut file = open_regular_local_file(path)?;
    let length = file.metadata()?.len();
    if length == 0 || length > maximum {
        return Err(WindowsAdapterError::InvalidReleaseManifest);
    }
    let mut bytes = Vec::with_capacity(length as usize);
    Read::by_ref(&mut file)
        .take(maximum + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 != length {
        return Err(WindowsAdapterError::FileIdentityMismatch);
    }
    Ok(bytes)
}

fn strict_value(bytes: &[u8]) -> Result<serde_json::Value, WindowsAdapterError> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| WindowsAdapterError::InvalidReleaseManifest)?;
    parse_no_duplicate_keys(text).map_err(|_| WindowsAdapterError::InvalidReleaseManifest)
}

fn move_replace(source: &Path, destination: &Path) -> Result<(), WindowsAdapterError> {
    let source = wide_nul(source.as_os_str());
    let destination = wide_nul(destination.as_os_str());
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVE_FILE_FLAGS(MOVEFILE_REPLACE_EXISTING.0 | MOVEFILE_WRITE_THROUGH.0),
        )
    }
    .map_err(|error| io::Error::from_raw_os_error(error.code().0).into())
}

fn wide_nul(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::installation::PackageSigningState;

    fn fixture_root(name: &str) -> PathBuf {
        let temp_root = std::env::temp_dir();
        // Codex may expose TEMP through a junction. Keep the production reparse-point
        // rejection intact while placing test fixtures under the resolved fixed volume.
        let temp_root = temp_root.canonicalize().unwrap_or(temp_root);
        let root = temp_root.join(format!(
            "star-adapter-windows-{name}-{}-{}",
            std::process::id(),
            InstallationId::new()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn write_release_fixture(root: &Path) -> ReleaseFileManifest {
        let binary = std::env::current_exe().unwrap();
        for name in ["star.exe", "star-controller.exe", "star-mcp.exe"] {
            std::fs::copy(&binary, root.join(name)).unwrap();
        }
        std::fs::create_dir_all(root.join("integrations/codex-plugin-template")).unwrap();
        std::fs::write(
            root.join("integrations/codex-plugin-template/readme.txt"),
            b"fixture",
        )
        .unwrap();
        let mut files = Vec::new();
        for relative in [
            "integrations/codex-plugin-template/readme.txt",
            "star-controller.exe",
            "star-mcp.exe",
            "star.exe",
        ] {
            let path = root.join(relative.replace('/', "\\"));
            let bytes = std::fs::read(&path).unwrap();
            files.push(ReleaseFileEntry {
                path: relative.to_owned(),
                size: bytes.len() as u64,
                sha256: Sha256Hash::digest(&bytes),
            });
        }
        let set_sha256 = canonical_sha256(&serde_json::to_value(&files).unwrap()).unwrap();
        let manifest = ReleaseFileManifest {
            schema_id: RELEASE_FILE_MANIFEST_SCHEMA_ID.to_owned(),
            schema_version: 1,
            product_version: env!("CARGO_PKG_VERSION").to_owned(),
            target_architecture: compiled_architecture().unwrap(),
            created_at: Utc::now(),
            source_revision: "test:fixture".to_owned(),
            files,
            generated_files: vec![CONTROLLER_INSTALL_MANIFEST_FILE.to_owned()],
            set_sha256,
            signing: PackageSigningState::UnsignedLocal,
        };
        atomic_write_json(&root.join(RELEASE_MANIFEST_FILE), &manifest).unwrap();
        manifest
    }

    #[test]
    fn finalize_status_and_tamper_detection() {
        let root = fixture_root("lifecycle");
        let data = fixture_root("data");
        write_release_fixture(&root);
        let manager = InstallationManager::new(data);
        let record = manager
            .finalize(&root, compiled_architecture().unwrap(), false)
            .unwrap();
        assert_eq!(
            record.install_root,
            normal_windows_path(&root.canonicalize().unwrap()).to_string_lossy()
        );
        assert!(root.join(CONTROLLER_INSTALL_MANIFEST_FILE).exists());
        assert!(manager.status(&root).unwrap().verified);

        std::fs::write(root.join("star-mcp.exe"), b"tampered").unwrap();
        assert!(matches!(
            manager.status(&root),
            Err(WindowsAdapterError::FileIdentityMismatch)
        ));
    }

    #[test]
    fn installation_record_does_not_silently_change_roots() {
        let first = fixture_root("first");
        let second = fixture_root("second");
        let data = fixture_root("conflict-data");
        write_release_fixture(&first);
        write_release_fixture(&second);
        let manager = InstallationManager::new(data);
        manager
            .finalize(&first, compiled_architecture().unwrap(), false)
            .unwrap();
        assert!(matches!(
            manager.finalize(&second, compiled_architecture().unwrap(), false),
            Err(WindowsAdapterError::InstallationConflict)
        ));
        assert!(!second.join(CONTROLLER_INSTALL_MANIFEST_FILE).exists());
        manager
            .finalize(&second, compiled_architecture().unwrap(), true)
            .unwrap();
    }

    #[test]
    fn manifest_paths_are_closed_and_case_insensitive_unique() {
        for invalid in ["", "/x", "../x", "a/../x", "a\\x", "C:/x", "a//x"] {
            assert!(!valid_manifest_relative_path(invalid), "{invalid}");
        }
        assert!(valid_manifest_relative_path(
            "catalog/tool-packages/core.toml"
        ));
    }

    #[test]
    fn locally_unverified_signed_manifest_is_rejected() {
        let root = fixture_root("signed");
        let data = fixture_root("signed-data");
        let mut manifest = write_release_fixture(&root);
        manifest.signing = PackageSigningState::Signed;
        atomic_write_json(&root.join(RELEASE_MANIFEST_FILE), &manifest).unwrap();
        let manager = InstallationManager::new(data);
        assert!(matches!(
            manager.finalize(&root, compiled_architecture().unwrap(), false),
            Err(WindowsAdapterError::InvalidReleaseManifest)
        ));
    }

    #[test]
    fn unsafe_relative_directory_is_rejected_before_creation() {
        let relative = PathBuf::from(format!(
            "star-adapter-windows-relative-{}-{}",
            std::process::id(),
            InstallationId::new()
        ));
        assert!(!relative.exists());
        assert!(matches!(
            ensure_fixed_directory(&relative),
            Err(WindowsAdapterError::UnsafePath)
        ));
        assert!(!relative.exists());
    }
}
