//! Windows fixed-volume file and current-user installation adapter.

#![cfg(windows)]

use std::{
    collections::{BTreeMap, BTreeSet},
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
use serde::{Deserialize, Serialize};
use star_contracts::{
    InstallationId, Sha256Hash, canonical_sha256,
    installation::{
        CODEX_INTEGRATION_RECORD_SCHEMA_ID, CodexIntegrationRecord, CodexIntegrationSummary,
        ControllerInstallManifest, INSTALLATION_RECORD_SCHEMA_ID, INSTALLATION_SCHEMA_VERSION,
        INTEGRATION_CANDIDATE_REVIEW_SCHEMA_ID, InstallationRecord, IntegrationCandidateClass,
        IntegrationCandidateReview, RELEASE_FILE_MANIFEST_SCHEMA_ID,
        RUNTIME_ACTIVATION_RECORD_SCHEMA_ID, RUNTIME_GENERATION_MANIFEST_SCHEMA_ID,
        ReleaseFileEntry, ReleaseFileManifest, RuntimeActivationRecord, RuntimeCandidateReview,
        RuntimeGenerationManifest, RuntimeGenerationRef, RuntimeUpdateClass, TargetArchitecture,
    },
    manifest::{ManifestSource, risk_lane},
    parse_manifest_v1, parse_no_duplicate_keys,
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
// Release payloads are verified by the manifest and can be much larger than
// the JSON manifest itself. Keep a bounded ceiling for rollback reads rather
// than using the manifest-size limit for executable files.
const RELEASE_PAYLOAD_MAX_BYTES: u64 = 512 * 1024 * 1024;
const LOCAL_RECORD_MAX_BYTES: u64 = 64 * 1024;
pub const RELEASE_MANIFEST_FILE: &str = "release-manifest.json";
pub const INSTALLATION_RECORD_FILE: &str = "installation-record.v1.json";
pub const CONTROLLER_INSTALL_MANIFEST_FILE: &str = "star-control-install.v1.json";
pub const RUNTIME_ACTIVATION_RECORD_FILE: &str = "active-runtime.v1.json";

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
    #[error("Runtime activation record is malformed, unsupported, or outside the install root")]
    InvalidRuntimeActivation,
    #[error("Runtime generation is malformed, unsupported, or fails identity verification")]
    InvalidRuntimeGeneration,
    #[error("runtime generation is already staged and cannot be overwritten")]
    RuntimeGenerationExists,
    #[error("integration candidate is not an approved Codex-integration-only release")]
    IntegrationCandidateRejected,
    #[error("integration candidate backup is malformed or does not belong to this installation")]
    InvalidIntegrationBackup,
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

/// Durable file-set backup used only while the one-shot updater applies a
/// restart-required Codex integration candidate.  The updater may use this
/// handle to restore the previous verified release if offline repair fails.
#[derive(Clone, Debug)]
pub struct IntegrationCandidateBackup {
    pub backup_root: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct IntegrationBackupManifest {
    schema_id: String,
    schema_version: u32,
    install_root: String,
    target_architecture: TargetArchitecture,
    state: String,
    files: Vec<IntegrationBackupFile>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct IntegrationBackupFile {
    path: String,
    existed: bool,
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

    pub fn runtime_activation_record_path(&self) -> PathBuf {
        self.local_data_root
            .join("installation")
            .join(RUNTIME_ACTIVATION_RECORD_FILE)
    }

    /// Atomically publishes the selector consumed by a Bootstrap Bridge v2.
    /// This does not start a Controller; the durable update supervisor owns
    /// drain, process handoff, postcheck, and rollback around this write.
    pub fn write_runtime_activation_record(
        &self,
        install_root: &Path,
        record: &RuntimeActivationRecord,
    ) -> Result<(), WindowsAdapterError> {
        let install_root = canonical_fixed_directory(install_root)?;
        validate_runtime_activation_record(&install_root, record)?;
        atomic_write_json(&self.runtime_activation_record_path(), record)
    }

    pub fn load_runtime_activation_record(
        &self,
        install_root: &Path,
    ) -> Result<RuntimeActivationRecord, WindowsAdapterError> {
        let install_root = canonical_fixed_directory(install_root)?;
        let record = load_runtime_activation_record(&self.runtime_activation_record_path())?;
        validate_runtime_activation_record(&install_root, &record)?;
        Ok(record)
    }

    /// Verifies an independently staged runtime generation and copies it into
    /// the fixed install tree without changing the active selector. A failed
    /// or repeated stage never overwrites an existing generation.
    pub fn stage_runtime_generation(
        &self,
        install_root: &Path,
        source_generation_root: &Path,
    ) -> Result<RuntimeGenerationRef, WindowsAdapterError> {
        let install_root = canonical_fixed_directory(install_root)?;
        let source_root = canonical_fixed_directory(source_generation_root)?;
        let generation = load_runtime_generation_manifest(&source_root)?;
        validate_runtime_generation(&source_root, &generation)?;

        let generations_root =
            ensure_fixed_directory(&install_root.join("runtime").join("generations"))?;
        let destination = generations_root.join(&generation.generation.generation_id);
        if destination.exists() {
            return Err(WindowsAdapterError::RuntimeGenerationExists);
        }
        std::fs::create_dir(&destination)?;
        copy_runtime_generation(&source_root, &destination, &generation)?;
        let copied = load_runtime_generation_manifest(&destination)?;
        validate_runtime_generation(&destination, &copied)?;
        if copied.generation != generation.generation {
            return Err(WindowsAdapterError::InvalidRuntimeGeneration);
        }

        Ok(RuntimeGenerationRef {
            generation_id: generation.generation.generation_id,
            runtime_root: normal_windows_path(&destination)
                .to_string_lossy()
                .into_owned(),
            release_manifest_sha256: generation.generation.release_manifest_sha256,
        })
    }

    /// Returns a deterministic, non-mutating review of a staged candidate.
    /// In particular, it never treats a package declaration as proof that a
    /// newly introduced Controller handler exists.
    pub fn inspect_runtime_candidate(
        &self,
        install_root: &Path,
        generation_id: &str,
    ) -> Result<RuntimeCandidateReview, WindowsAdapterError> {
        let install_root = canonical_fixed_directory(install_root)?;
        if generation_id.trim().is_empty()
            || Path::new(generation_id)
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(WindowsAdapterError::InvalidRuntimeGeneration);
        }
        let candidate_root = install_root
            .join("runtime")
            .join("generations")
            .join(generation_id);
        let candidate_manifest = load_runtime_generation_manifest(&candidate_root)?;
        validate_runtime_generation(&candidate_root, &candidate_manifest)?;
        let candidate = RuntimeGenerationRef {
            generation_id: candidate_manifest.generation.generation_id.clone(),
            runtime_root: normal_windows_path(&candidate_root.canonicalize()?)
                .to_string_lossy()
                .into_owned(),
            release_manifest_sha256: candidate_manifest
                .generation
                .release_manifest_sha256
                .clone(),
        };
        let candidate_actions = load_generation_actions(&candidate_root, &candidate_manifest)?;

        let active = self
            .runtime_activation_record_path()
            .exists()
            .then(|| self.load_runtime_activation_record(&install_root))
            .transpose()?;
        let active_actions = match active.as_ref() {
            Some(active) => {
                let root = PathBuf::from(&active.active.runtime_root);
                let manifest = load_runtime_generation_manifest(&root)?;
                validate_runtime_generation(&root, &manifest)?;
                load_generation_actions(&root, &manifest)?
            }
            None => BTreeMap::new(),
        };
        let comparison = compare_generation_actions(&active_actions, &candidate_actions)?;
        let handler_ready = comparison.added.is_empty() && comparison.changed.is_empty();
        let review_scope = serde_json::json!({
            "candidate": candidate,
            "update_class": RuntimeUpdateClass::RuntimeGeneration,
            "added_actions": comparison.added,
            "removed_actions": comparison.removed,
            "changed_actions": comparison.changed,
            "breaking_schema": comparison.breaking_schema,
            "risk_lane_widened": comparison.risk_lane_widened,
            "permission_widened": comparison.permission_widened,
            "handler_ready": handler_ready,
            "bridge_contract_version": candidate_manifest.bridge_contract_version,
            "active_generation_id": active.as_ref().map(|record| &record.active.generation_id),
        });
        Ok(RuntimeCandidateReview {
            schema_id: "star.runtime-candidate-review".to_owned(),
            schema_version: 1,
            candidate,
            update_class: RuntimeUpdateClass::RuntimeGeneration,
            added_actions: comparison.added,
            removed_actions: comparison.removed,
            changed_actions: comparison.changed,
            breaking_schema: comparison.breaking_schema,
            risk_lane_widened: comparison.risk_lane_widened,
            permission_widened: comparison.permission_widened,
            handler_ready,
            bridge_compatible: candidate_manifest.bridge_contract_version == 2,
            rollback_available: active.is_some(),
            requires_codex_restart: false,
            requires_new_task: false,
            hook_review_required: false,
            approval_scope_sha256: canonical_sha256(&review_scope)
                .map_err(|_| WindowsAdapterError::InvalidRuntimeGeneration)?,
        })
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
        write_controller_manifest(&install_root, &manifest, None)?;
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
        let controller_manifest =
            parse_controller_manifest(&controller_manifest, &install_root, &manifest)?;
        match (
            controller_manifest
                .runtime_activation_record_path
                .as_deref(),
            controller_manifest.bridge_contract_version,
        ) {
            (None, None) => {}
            (Some(path), Some(_))
                if paths_equal_case_insensitive(
                    Path::new(path),
                    &self.runtime_activation_record_path(),
                ) =>
            {
                self.load_runtime_activation_record(&install_root)?;
            }
            _ => return Err(WindowsAdapterError::FileIdentityMismatch),
        }
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

    /// Returns the one Runtime Generation owned by the verified release-file
    /// manifest currently installed at `install_root`.
    ///
    /// Old generations are deliberately retained for rollback, so directory
    /// enumeration cannot identify the generation delivered by a replacement
    /// installer.  The release manifest is the sole ownership boundary.
    pub fn verified_bundled_runtime_generation(
        &self,
        install_root: &Path,
    ) -> Result<RuntimeGenerationRef, WindowsAdapterError> {
        let install_root = canonical_fixed_directory(install_root)?;
        self.status(&install_root)?;
        let manifest_bytes = read_regular_bounded(
            &install_root.join(RELEASE_MANIFEST_FILE),
            RELEASE_MANIFEST_MAX_BYTES,
        )?;
        let manifest = parse_release_manifest(&manifest_bytes)?;

        let mut generation_ids = BTreeSet::new();
        for entry in &manifest.files {
            let mut components = entry.path.split('/');
            if components.next() == Some("runtime")
                && components.next() == Some("generations")
                && let Some(generation_id) = components.next()
            {
                if generation_id.is_empty()
                    || generation_id.contains('/')
                    || generation_id.contains('\\')
                    || Path::new(generation_id)
                        .components()
                        .any(|component| !matches!(component, Component::Normal(_)))
                {
                    return Err(WindowsAdapterError::InvalidRuntimeGeneration);
                }
                generation_ids.insert(generation_id.to_owned());
            }
        }
        if generation_ids.len() != 1 {
            return Err(WindowsAdapterError::InvalidRuntimeGeneration);
        }
        let generation_id = generation_ids
            .into_iter()
            .next()
            .ok_or(WindowsAdapterError::InvalidRuntimeGeneration)?;
        for required in [
            "runtime-generation.v1.json",
            "runtime-release-manifest.json",
        ] {
            let path = format!("runtime/generations/{generation_id}/{required}");
            if !manifest.files.iter().any(|entry| entry.path == path) {
                return Err(WindowsAdapterError::InvalidRuntimeGeneration);
            }
        }

        let runtime_root = canonical_fixed_directory(
            &install_root
                .join("runtime")
                .join("generations")
                .join(&generation_id),
        )?;
        let generation = load_runtime_generation_manifest(&runtime_root)?;
        validate_runtime_generation(&runtime_root, &generation)?;
        if generation.generation.generation_id != generation_id {
            return Err(WindowsAdapterError::InvalidRuntimeGeneration);
        }
        Ok(RuntimeGenerationRef {
            generation_id,
            runtime_root: normal_windows_path(&runtime_root)
                .to_string_lossy()
                .into_owned(),
            release_manifest_sha256: generation.generation.release_manifest_sha256,
        })
    }

    /// Returns the manifest-declared tool IDs for one verified Runtime
    /// Generation under this installation.  This is used by an offline
    /// installer postcheck to prove that the live Registry did not silently
    /// omit an owned release action.
    pub fn verified_runtime_tool_ids(
        &self,
        install_root: &Path,
        generation: &RuntimeGenerationRef,
    ) -> Result<BTreeSet<String>, WindowsAdapterError> {
        let install_root = canonical_fixed_directory(install_root)?;
        let expected_root = canonical_fixed_directory(
            &install_root
                .join("runtime")
                .join("generations")
                .join(&generation.generation_id),
        )?;
        let declared_root = canonical_fixed_directory(Path::new(&generation.runtime_root))?;
        if expected_root != declared_root {
            return Err(WindowsAdapterError::InvalidRuntimeGeneration);
        }
        let manifest = load_runtime_generation_manifest(&expected_root)?;
        validate_runtime_generation(&expected_root, &manifest)?;
        if manifest.generation.generation_id != generation.generation_id
            || manifest.generation.release_manifest_sha256 != generation.release_manifest_sha256
        {
            return Err(WindowsAdapterError::InvalidRuntimeGeneration);
        }
        let actions = load_generation_actions(&expected_root, &manifest)?;
        let mut tool_ids = BTreeSet::new();
        for action in actions.into_values() {
            if !tool_ids.insert(action.tool_id) {
                return Err(WindowsAdapterError::InvalidRuntimeGeneration);
            }
        }
        if tool_ids.is_empty() {
            return Err(WindowsAdapterError::InvalidRuntimeGeneration);
        }
        Ok(tool_ids)
    }

    /// Inspects a complete, separately staged release without copying it into
    /// the installation. The resulting class tells the caller whether a
    /// restart transaction may handle the candidate or whether the offline
    /// installer must replace the updater itself.
    pub fn inspect_integration_candidate(
        &self,
        install_root: &Path,
        candidate_root: &Path,
    ) -> Result<IntegrationCandidateReview, WindowsAdapterError> {
        let installed = self.status(install_root)?;
        let install_root = canonical_fixed_directory(install_root)?;
        let candidate_root = canonical_fixed_directory(candidate_root)?;
        if paths_equal_case_insensitive(&install_root, &candidate_root) {
            return Err(WindowsAdapterError::UnsafePath);
        }
        let current_bytes = read_regular_bounded(
            &install_root.join(RELEASE_MANIFEST_FILE),
            RELEASE_MANIFEST_MAX_BYTES,
        )?;
        let current = parse_release_manifest(&current_bytes)?;
        let candidate_bytes = read_regular_bounded(
            &candidate_root.join(RELEASE_MANIFEST_FILE),
            RELEASE_MANIFEST_MAX_BYTES,
        )?;
        let candidate = parse_release_manifest(&candidate_bytes)?;
        validate_release_manifest(&candidate, installed.target_architecture)?;
        verify_release_files(&candidate_root, &candidate)?;

        let current_files = current
            .files
            .iter()
            .map(|entry| (entry.path.as_str(), entry))
            .collect::<BTreeMap<_, _>>();
        let candidate_files = candidate
            .files
            .iter()
            .map(|entry| (entry.path.as_str(), entry))
            .collect::<BTreeMap<_, _>>();
        let paths = current_files
            .keys()
            .chain(candidate_files.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        let changed_files = paths
            .into_iter()
            .filter(|path| current_files.get(path) != candidate_files.get(path))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let candidate_class = classify_integration_candidate(&changed_files);
        let candidate_release_manifest_sha256 = Sha256Hash::digest(&candidate_bytes);
        let approval_scope_sha256 = canonical_sha256(&serde_json::json!({
            "candidate_release_manifest_sha256":candidate_release_manifest_sha256,
            "target_architecture":candidate.target_architecture,
            "candidate_class":candidate_class,
            "changed_files":changed_files,
        }))
        .map_err(|_| WindowsAdapterError::InvalidReleaseManifest)?;
        Ok(IntegrationCandidateReview {
            schema_id: INTEGRATION_CANDIDATE_REVIEW_SCHEMA_ID.to_owned(),
            schema_version: 1,
            candidate_release_manifest_sha256,
            target_architecture: candidate.target_architecture,
            candidate_class,
            changed_files,
            rollback_available: true,
            requires_codex_restart: matches!(
                candidate_class,
                IntegrationCandidateClass::CodexIntegrationUpdate
                    | IntegrationCandidateClass::MixedUpdate
            ),
            approval_scope_sha256,
        })
    }

    /// Replaces only the Bridge/MCP/template file set of a previously reviewed
    /// candidate.  This is deliberately unavailable for runtime, mixed, and
    /// updater candidates: those routes have different activation or offline
    /// installer ownership.  A durable backup is written before the first
    /// installation-root mutation and is returned for an explicit rollback.
    pub fn apply_codex_integration_candidate(
        &self,
        install_root: &Path,
        candidate_root: &Path,
        approval_scope_sha256: &Sha256Hash,
        operation_id: &str,
    ) -> Result<IntegrationCandidateBackup, WindowsAdapterError> {
        if !safe_update_operation_id(operation_id) {
            return Err(WindowsAdapterError::IntegrationCandidateRejected);
        }
        let installed = self.status(install_root)?;
        let review = self.inspect_integration_candidate(install_root, candidate_root)?;
        if review.candidate_class != IntegrationCandidateClass::CodexIntegrationUpdate
            || review.approval_scope_sha256 != *approval_scope_sha256
            || !review.rollback_available
        {
            return Err(WindowsAdapterError::IntegrationCandidateRejected);
        }
        let install_root = canonical_fixed_directory(install_root)?;
        let candidate_root = canonical_fixed_directory(candidate_root)?;
        let backup_parent = ensure_fixed_directory(
            &self
                .local_data_root
                .join("updates")
                .join("integration-backups"),
        )?;
        let backup_root = backup_parent.join(operation_id);
        if backup_root.exists() {
            return Err(WindowsAdapterError::IntegrationCandidateRejected);
        }
        std::fs::create_dir(&backup_root)?;
        let backup_root = canonical_fixed_directory(&backup_root)?;

        let mut paths = review.changed_files.clone();
        paths.push(RELEASE_MANIFEST_FILE.to_owned());
        paths.sort();
        paths.dedup();
        if paths
            .iter()
            .any(|path| path != RELEASE_MANIFEST_FILE && !is_codex_integration_path(path))
        {
            return Err(WindowsAdapterError::IntegrationCandidateRejected);
        }
        let mut backup_files = Vec::with_capacity(paths.len());
        for relative in &paths {
            let source = install_root.join(relative.replace('/', "\\"));
            let existed = source.exists();
            if existed {
                copy_regular_file_new(
                    &source,
                    &backup_root.join("files").join(relative.replace('/', "\\")),
                )?;
            }
            backup_files.push(IntegrationBackupFile {
                path: relative.clone(),
                existed,
            });
        }
        atomic_write_json(
            &backup_root.join("backup.v1.json"),
            &IntegrationBackupManifest {
                schema_id: "star.integration-candidate-backup".to_owned(),
                schema_version: 1,
                install_root: normal_windows_path(&install_root)
                    .to_string_lossy()
                    .into_owned(),
                target_architecture: installed.target_architecture,
                state: "prepared".to_owned(),
                files: backup_files,
            },
        )?;
        let backup = IntegrationCandidateBackup { backup_root };
        if let Err(error) = self.apply_candidate_files(
            &install_root,
            &candidate_root,
            &review.changed_files,
            installed.target_architecture,
        ) {
            let _ = self.rollback_codex_integration_candidate(&install_root, &backup);
            return Err(error);
        }
        self.set_integration_backup_state(&backup, "applied")?;
        Ok(backup)
    }

    /// Restores a backup created by `apply_codex_integration_candidate` and
    /// re-generates the derived installation/controller records from the old
    /// release manifest.  Only a backup tied to this exact fixed installation
    /// root is accepted.
    pub fn rollback_codex_integration_candidate(
        &self,
        install_root: &Path,
        backup: &IntegrationCandidateBackup,
    ) -> Result<(), WindowsAdapterError> {
        let install_root = canonical_fixed_directory(install_root)?;
        let backup_root = canonical_fixed_directory(&backup.backup_root)?;
        let manifest = load_integration_backup_manifest(&backup_root)?;
        if manifest.schema_id != "star.integration-candidate-backup"
            || manifest.schema_version != 1
            || !matches!(
                manifest.state.as_str(),
                "prepared" | "applied" | "committed" | "rolled_back"
            )
            || !paths_equal_case_insensitive(Path::new(&manifest.install_root), &install_root)
            || manifest.files.is_empty()
            || manifest.files.iter().any(|file| {
                (file.path != RELEASE_MANIFEST_FILE && !is_codex_integration_path(&file.path))
                    || !valid_manifest_relative_path(&file.path)
            })
        {
            return Err(WindowsAdapterError::InvalidIntegrationBackup);
        }
        let active = self
            .runtime_activation_record_path()
            .exists()
            .then(|| self.load_runtime_activation_record(&install_root))
            .transpose()?;
        let mut files = manifest.files;
        files.sort_by_key(|file| file.path == RELEASE_MANIFEST_FILE);
        for file in files {
            let destination = install_root.join(file.path.replace('/', "\\"));
            if file.existed {
                let bytes = read_regular_bounded(
                    &backup_root.join("files").join(file.path.replace('/', "\\")),
                    RELEASE_PAYLOAD_MAX_BYTES,
                )?;
                atomic_write(&destination, &bytes)?;
            } else if destination.exists() {
                if !destination.is_file() || has_reparse_ancestor(&destination) {
                    return Err(WindowsAdapterError::UnsafePath);
                }
                std::fs::remove_file(destination)?;
            }
        }
        self.finalize(&install_root, manifest.target_architecture, true)?;
        if let Some(active) = active {
            self.activate_runtime_bridge(&install_root, &active, active.bridge_contract_version)?;
        }
        self.status(&install_root)?;
        self.set_integration_backup_state(
            &IntegrationCandidateBackup { backup_root },
            "rolled_back",
        )?;
        Ok(())
    }

    /// Marks the file-set backup committed only after Codex Plugin repair has
    /// succeeded.  A process interruption before this point is recovered by
    /// the next updater invocation instead of being mistaken for success.
    pub fn commit_codex_integration_candidate(
        &self,
        backup: &IntegrationCandidateBackup,
    ) -> Result<(), WindowsAdapterError> {
        self.set_integration_backup_state(backup, "committed")
    }

    /// Restores an interrupted integration candidate before another updater
    /// transaction starts. Committed/rolled-back audit backups remain intact;
    /// only `prepared` or `applied` records are actionable.
    pub fn recover_interrupted_codex_integration_candidates(
        &self,
        install_root: &Path,
    ) -> Result<u32, WindowsAdapterError> {
        let install_root = canonical_fixed_directory(install_root)?;
        let parent = self
            .local_data_root
            .join("updates")
            .join("integration-backups");
        if !parent.exists() {
            return Ok(0);
        }
        let parent = canonical_fixed_directory(&parent)?;
        let mut recovered = 0_u32;
        for entry in std::fs::read_dir(parent)? {
            let entry = entry?;
            let root = entry.path();
            if !root.is_dir() {
                return Err(WindowsAdapterError::InvalidIntegrationBackup);
            }
            let backup_root = canonical_fixed_directory(&root)?;
            let manifest = load_integration_backup_manifest(&backup_root)?;
            if !paths_equal_case_insensitive(Path::new(&manifest.install_root), &install_root) {
                continue;
            }
            if matches!(manifest.state.as_str(), "prepared" | "applied") {
                self.rollback_codex_integration_candidate(
                    &install_root,
                    &IntegrationCandidateBackup { backup_root },
                )?;
                recovered = recovered.saturating_add(1);
            } else if !matches!(manifest.state.as_str(), "committed" | "rolled_back") {
                return Err(WindowsAdapterError::InvalidIntegrationBackup);
            }
        }
        Ok(recovered)
    }

    fn set_integration_backup_state(
        &self,
        backup: &IntegrationCandidateBackup,
        state: &str,
    ) -> Result<(), WindowsAdapterError> {
        if !matches!(state, "prepared" | "applied" | "committed" | "rolled_back") {
            return Err(WindowsAdapterError::InvalidIntegrationBackup);
        }
        let backup_root = canonical_fixed_directory(&backup.backup_root)?;
        let mut manifest = load_integration_backup_manifest(&backup_root)?;
        manifest.state = state.to_owned();
        atomic_write_json(&backup_root.join("backup.v1.json"), &manifest)
    }

    fn apply_candidate_files(
        &self,
        install_root: &Path,
        candidate_root: &Path,
        changed_files: &[String],
        target_architecture: TargetArchitecture,
    ) -> Result<(), WindowsAdapterError> {
        let candidate_bytes = read_regular_bounded(
            &candidate_root.join(RELEASE_MANIFEST_FILE),
            RELEASE_MANIFEST_MAX_BYTES,
        )?;
        let candidate = parse_release_manifest(&candidate_bytes)?;
        validate_release_manifest(&candidate, target_architecture)?;
        verify_release_files(candidate_root, &candidate)?;
        let candidate_files = candidate
            .files
            .iter()
            .map(|entry| (entry.path.as_str(), entry))
            .collect::<BTreeMap<_, _>>();
        let active = self
            .runtime_activation_record_path()
            .exists()
            .then(|| self.load_runtime_activation_record(install_root))
            .transpose()?;
        for relative in changed_files {
            if !is_codex_integration_path(relative) {
                return Err(WindowsAdapterError::IntegrationCandidateRejected);
            }
            let destination = install_root.join(relative.replace('/', "\\"));
            if let Some(entry) = candidate_files.get(relative.as_str()) {
                let bytes = read_regular_bounded(
                    &candidate_root.join(entry.path.replace('/', "\\")),
                    entry.size,
                )?;
                if Sha256Hash::digest(&bytes) != entry.sha256 {
                    return Err(WindowsAdapterError::FileIdentityMismatch);
                }
                atomic_write(&destination, &bytes)?;
            } else if destination.exists() {
                if !destination.is_file() || has_reparse_ancestor(&destination) {
                    return Err(WindowsAdapterError::UnsafePath);
                }
                std::fs::remove_file(destination)?;
            }
        }
        // The manifest is the commit marker: it changes only after every
        // candidate payload file has been durably replaced.
        atomic_write(&install_root.join(RELEASE_MANIFEST_FILE), &candidate_bytes)?;
        self.finalize(install_root, target_architecture, true)?;
        if let Some(active) = active {
            self.activate_runtime_bridge(install_root, &active, active.bridge_contract_version)?;
        }
        self.status(install_root)?;
        Ok(())
    }

    /// Enables Bootstrap Bridge v2 only after a validated active Runtime
    /// Generation exists. The activation record is written first; if the
    /// Bridge manifest write fails, the existing v1 bridge remains active.
    pub fn activate_runtime_bridge(
        &self,
        install_root: &Path,
        record: &RuntimeActivationRecord,
        bridge_contract_version: u32,
    ) -> Result<(), WindowsAdapterError> {
        if bridge_contract_version == 0 {
            return Err(WindowsAdapterError::InvalidRuntimeActivation);
        }
        let install_root = canonical_fixed_directory(install_root)?;
        let manifest_bytes = read_regular_bounded(
            &install_root.join(RELEASE_MANIFEST_FILE),
            RELEASE_MANIFEST_MAX_BYTES,
        )?;
        let release = parse_release_manifest(&manifest_bytes)?;
        validate_release_manifest(&release, compiled_architecture()?)?;
        verify_release_files(&install_root, &release)?;
        if record.bridge_contract_version != bridge_contract_version {
            return Err(WindowsAdapterError::InvalidRuntimeActivation);
        }
        verify_activation_generation(&record.active)?;
        if let Some(previous) = &record.previous {
            verify_activation_generation(previous)?;
        }
        self.write_runtime_activation_record(&install_root, record)?;
        write_controller_manifest(
            &install_root,
            &release,
            Some((
                &self.runtime_activation_record_path(),
                bridge_contract_version,
            )),
        )
    }

    /// One-time offline Bootstrap Bridge v2 migration. The installer has
    /// already copied one verified Runtime Generation into the release tree
    /// and stopped Codex/MCP before calling this method. Routine updates must
    /// use `stage_runtime_generation` plus the stable CLI supervisor instead.
    pub fn initialize_runtime_bridge(
        &self,
        install_root: &Path,
        state_generation_id: &str,
    ) -> Result<RuntimeActivationRecord, WindowsAdapterError> {
        if state_generation_id.trim().is_empty() || state_generation_id.chars().count() > 128 {
            return Err(WindowsAdapterError::InvalidRuntimeActivation);
        }
        let install_root = canonical_fixed_directory(install_root)?;
        if self.runtime_activation_record_path().exists() {
            let record = self.load_runtime_activation_record(&install_root)?;
            self.activate_runtime_bridge(&install_root, &record, record.bridge_contract_version)?;
            return Ok(record);
        }
        let generations_root =
            canonical_fixed_directory(&install_root.join("runtime").join("generations"))?;
        let mut generations = std::fs::read_dir(&generations_root)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect::<Vec<_>>();
        generations.sort();
        if generations.len() != 1 {
            return Err(WindowsAdapterError::InvalidRuntimeGeneration);
        }
        let runtime_root = canonical_fixed_directory(&generations[0])?;
        let manifest = load_runtime_generation_manifest(&runtime_root)?;
        validate_runtime_generation(&runtime_root, &manifest)?;
        let record = RuntimeActivationRecord {
            schema_id: RUNTIME_ACTIVATION_RECORD_SCHEMA_ID.to_owned(),
            schema_version: 1,
            activation_revision: 1,
            active: RuntimeGenerationRef {
                generation_id: manifest.generation.generation_id,
                runtime_root: normal_windows_path(&runtime_root)
                    .to_string_lossy()
                    .into_owned(),
                release_manifest_sha256: manifest.generation.release_manifest_sha256,
            },
            previous: None,
            state_generation_id: state_generation_id.to_owned(),
            bridge_contract_version: manifest.bridge_contract_version,
            activated_at: Utc::now(),
        };
        self.activate_runtime_bridge(&install_root, &record, manifest.bridge_contract_version)?;
        Ok(record)
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

fn classify_integration_candidate(changed_files: &[String]) -> IntegrationCandidateClass {
    if changed_files.is_empty() {
        return IntegrationCandidateClass::NoChange;
    }
    let updater_changed = changed_files.iter().any(|path| path == "star-updater.exe");
    let integration_changed = changed_files.iter().any(|path| {
        matches!(path.as_str(), "star.exe" | "star-mcp.exe")
            || path.starts_with("integrations/codex-plugin-template/")
    });
    let runtime_changed = changed_files.iter().any(|path| {
        !matches!(
            path.as_str(),
            "star-updater.exe" | "star.exe" | "star-mcp.exe"
        ) && !path.starts_with("integrations/codex-plugin-template/")
    });
    match (updater_changed, integration_changed, runtime_changed) {
        (true, false, false) => IntegrationCandidateClass::UpdaterUpdate,
        (false, true, false) => IntegrationCandidateClass::CodexIntegrationUpdate,
        (false, false, true) => IntegrationCandidateClass::RuntimeUpdate,
        _ => IntegrationCandidateClass::MixedUpdate,
    }
}

fn is_codex_integration_path(path: &str) -> bool {
    matches!(path, "star.exe" | "star-mcp.exe")
        || path.starts_with("integrations/codex-plugin-template/")
}

fn load_integration_backup_manifest(
    backup_root: &Path,
) -> Result<IntegrationBackupManifest, WindowsAdapterError> {
    let bytes = read_regular_bounded(&backup_root.join("backup.v1.json"), LOCAL_RECORD_MAX_BYTES)
        .map_err(|_| WindowsAdapterError::InvalidIntegrationBackup)?;
    let value = strict_value(&bytes).map_err(|_| WindowsAdapterError::InvalidIntegrationBackup)?;
    serde_json::from_value(value).map_err(|_| WindowsAdapterError::InvalidIntegrationBackup)
}

fn safe_update_operation_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
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

pub fn load_runtime_activation_record(
    path: &Path,
) -> Result<RuntimeActivationRecord, WindowsAdapterError> {
    let bytes = read_regular_bounded(path, LOCAL_RECORD_MAX_BYTES)
        .map_err(|_| WindowsAdapterError::InvalidRuntimeActivation)?;
    let value = strict_value(&bytes).map_err(|_| WindowsAdapterError::InvalidRuntimeActivation)?;
    let record: RuntimeActivationRecord =
        serde_json::from_value(value).map_err(|_| WindowsAdapterError::InvalidRuntimeActivation)?;
    if record.schema_id != RUNTIME_ACTIVATION_RECORD_SCHEMA_ID
        || record.schema_version != 1
        || record.activation_revision == 0
        || record.bridge_contract_version == 0
        || record.state_generation_id.trim().is_empty()
        || record.active.generation_id.trim().is_empty()
        || record.active.runtime_root.trim().is_empty()
    {
        return Err(WindowsAdapterError::InvalidRuntimeActivation);
    }
    if record
        .previous
        .as_ref()
        .is_some_and(|previous| previous.generation_id == record.active.generation_id)
    {
        return Err(WindowsAdapterError::InvalidRuntimeActivation);
    }
    Ok(record)
}

fn validate_runtime_activation_record(
    install_root: &Path,
    record: &RuntimeActivationRecord,
) -> Result<(), WindowsAdapterError> {
    let runtime_root = canonical_fixed_directory(Path::new(&record.active.runtime_root))?;
    let generations_root =
        canonical_fixed_directory(&install_root.join("runtime").join("generations"))?;
    if !path_is_within(&runtime_root, &generations_root) {
        return Err(WindowsAdapterError::InvalidRuntimeActivation);
    }
    if let Some(previous) = &record.previous {
        let previous_root = canonical_fixed_directory(Path::new(&previous.runtime_root))?;
        if !path_is_within(&previous_root, &generations_root) {
            return Err(WindowsAdapterError::InvalidRuntimeActivation);
        }
    }
    Ok(())
}

fn verify_activation_generation(
    reference: &RuntimeGenerationRef,
) -> Result<(), WindowsAdapterError> {
    let root = canonical_fixed_directory(Path::new(&reference.runtime_root))
        .map_err(|_| WindowsAdapterError::InvalidRuntimeActivation)?;
    let manifest = load_runtime_generation_manifest(&root)
        .map_err(|_| WindowsAdapterError::InvalidRuntimeActivation)?;
    validate_runtime_generation(&root, &manifest)
        .map_err(|_| WindowsAdapterError::InvalidRuntimeActivation)?;
    if manifest.generation.generation_id != reference.generation_id
        || manifest.generation.release_manifest_sha256 != reference.release_manifest_sha256
    {
        return Err(WindowsAdapterError::InvalidRuntimeActivation);
    }
    Ok(())
}

pub fn load_runtime_generation_manifest(
    runtime_root: &Path,
) -> Result<RuntimeGenerationManifest, WindowsAdapterError> {
    let bytes = read_regular_bounded(
        &runtime_root.join("runtime-generation.v1.json"),
        LOCAL_RECORD_MAX_BYTES,
    )
    .map_err(|_| WindowsAdapterError::InvalidRuntimeGeneration)?;
    let value = strict_value(&bytes).map_err(|_| WindowsAdapterError::InvalidRuntimeGeneration)?;
    let manifest: RuntimeGenerationManifest =
        serde_json::from_value(value).map_err(|_| WindowsAdapterError::InvalidRuntimeGeneration)?;
    if manifest.schema_id != RUNTIME_GENERATION_MANIFEST_SCHEMA_ID
        || manifest.schema_version != 1
        || manifest.generation.generation_id.trim().is_empty()
        || manifest.generation.runtime_root != "."
        || semver::Version::parse(&manifest.product_version).is_err()
        || manifest.target_architecture != compiled_architecture()?
        || manifest.controller_path != "star-controller.exe"
        || manifest.cli_runtime_path != "star-cli-runtime.exe"
        || manifest.catalog_path != "catalog"
        || manifest.schemas_root != "schemas/v1"
        || manifest.bridge_contract_version != 2
    {
        return Err(WindowsAdapterError::InvalidRuntimeGeneration);
    }
    Ok(manifest)
}

fn validate_runtime_generation(
    runtime_root: &Path,
    generation: &RuntimeGenerationManifest,
) -> Result<(), WindowsAdapterError> {
    let root = canonical_fixed_directory(runtime_root)?;
    if root.file_name().and_then(OsStr::to_str) != Some(&generation.generation.generation_id) {
        return Err(WindowsAdapterError::InvalidRuntimeGeneration);
    }
    let release_bytes = read_regular_bounded(
        &root.join("runtime-release-manifest.json"),
        RELEASE_MANIFEST_MAX_BYTES,
    )
    .map_err(|_| WindowsAdapterError::InvalidRuntimeGeneration)?;
    if Sha256Hash::digest(&release_bytes) != generation.generation.release_manifest_sha256 {
        return Err(WindowsAdapterError::InvalidRuntimeGeneration);
    }
    let release = parse_release_manifest(&release_bytes)
        .map_err(|_| WindowsAdapterError::InvalidRuntimeGeneration)?;
    validate_runtime_release_manifest(&release, generation.target_architecture)?;
    verify_release_files(&root, &release)
        .map_err(|_| WindowsAdapterError::InvalidRuntimeGeneration)?;

    let controller = root.join(&generation.controller_path);
    if Sha256Hash::digest_reader(
        open_regular_local_file(&controller)
            .map_err(|_| WindowsAdapterError::InvalidRuntimeGeneration)?,
    )
    .map_err(|_| WindowsAdapterError::InvalidRuntimeGeneration)?
        != generation.controller_sha256
    {
        return Err(WindowsAdapterError::InvalidRuntimeGeneration);
    }
    for required in [
        &generation.controller_path,
        &generation.cli_runtime_path,
        &generation.catalog_path,
        &generation.schemas_root,
    ] {
        let path = root.join(required.replace('/', "\\\\"));
        if !path.exists() || has_reparse_ancestor(&path) {
            return Err(WindowsAdapterError::InvalidRuntimeGeneration);
        }
    }
    Ok(())
}

fn validate_runtime_release_manifest(
    manifest: &ReleaseFileManifest,
    requested_architecture: TargetArchitecture,
) -> Result<(), WindowsAdapterError> {
    if manifest.schema_id != RELEASE_FILE_MANIFEST_SCHEMA_ID
        || manifest.schema_version != INSTALLATION_SCHEMA_VERSION
        || semver::Version::parse(&manifest.product_version).is_err()
        || manifest.source_revision.is_empty()
        || manifest.source_revision.len() > 256
        || manifest.target_architecture != requested_architecture
        || manifest.signing != star_contracts::installation::PackageSigningState::UnsignedLocal
        || manifest.generated_files != ["runtime-generation.v1.json"]
        || manifest.files.is_empty()
    {
        return Err(WindowsAdapterError::InvalidRuntimeGeneration);
    }
    let mut previous: Option<&str> = None;
    let mut casefolded = BTreeSet::new();
    for entry in &manifest.files {
        if !valid_manifest_relative_path(&entry.path)
            || previous.is_some_and(|value| value >= entry.path.as_str())
            || !casefolded.insert(entry.path.to_ascii_lowercase())
        {
            return Err(WindowsAdapterError::InvalidRuntimeGeneration);
        }
        previous = Some(&entry.path);
    }
    for required in ["star-cli-runtime.exe", "star-controller.exe"] {
        if !manifest.files.iter().any(|entry| entry.path == required) {
            return Err(WindowsAdapterError::InvalidRuntimeGeneration);
        }
    }
    let value = serde_json::to_value(&manifest.files)?;
    if canonical_sha256(&value).map_err(|_| WindowsAdapterError::InvalidRuntimeGeneration)?
        != manifest.set_sha256
    {
        return Err(WindowsAdapterError::InvalidRuntimeGeneration);
    }
    Ok(())
}

fn copy_runtime_generation(
    source_root: &Path,
    destination_root: &Path,
    generation: &RuntimeGenerationManifest,
) -> Result<(), WindowsAdapterError> {
    let release_bytes = read_regular_bounded(
        &source_root.join("runtime-release-manifest.json"),
        RELEASE_MANIFEST_MAX_BYTES,
    )?;
    let release = parse_release_manifest(&release_bytes)?;
    for entry in &release.files {
        copy_regular_file_new(
            &source_root.join(entry.path.replace('/', "\\\\")),
            &destination_root.join(entry.path.replace('/', "\\\\")),
        )?;
    }
    for name in [
        "runtime-release-manifest.json",
        "runtime-generation.v1.json",
    ] {
        copy_regular_file_new(&source_root.join(name), &destination_root.join(name))?;
    }
    let copied = load_runtime_generation_manifest(destination_root)?;
    if copied.generation != generation.generation {
        return Err(WindowsAdapterError::InvalidRuntimeGeneration);
    }
    Ok(())
}

fn copy_regular_file_new(source: &Path, destination: &Path) -> Result<(), WindowsAdapterError> {
    let mut input = open_regular_local_file(source)?;
    let parent = destination
        .parent()
        .ok_or(WindowsAdapterError::UnsafePath)?;
    let parent = ensure_fixed_directory(parent)?;
    let destination = parent.join(
        destination
            .file_name()
            .ok_or(WindowsAdapterError::UnsafePath)?,
    );
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    Ok(())
}

#[derive(Default)]
struct ActionComparison {
    added: Vec<String>,
    removed: Vec<String>,
    changed: Vec<String>,
    breaking_schema: bool,
    risk_lane_widened: bool,
    permission_widened: bool,
}

fn load_generation_actions(
    runtime_root: &Path,
    generation: &RuntimeGenerationManifest,
) -> Result<BTreeMap<String, star_contracts::manifest::ActionDescriptor>, WindowsAdapterError> {
    let package_root = runtime_root
        .join(&generation.catalog_path)
        .join("tool-packages");
    let package_root = canonical_fixed_directory(&package_root)
        .map_err(|_| WindowsAdapterError::InvalidRuntimeGeneration)?;
    let mut actions = BTreeMap::new();
    for entry in std::fs::read_dir(package_root)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(OsStr::to_str) != Some("toml") {
            continue;
        }
        let bytes = read_regular_bounded(&path, LOCAL_RECORD_MAX_BYTES)
            .map_err(|_| WindowsAdapterError::InvalidRuntimeGeneration)?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| WindowsAdapterError::InvalidRuntimeGeneration)?;
        let manifest = parse_manifest_v1(text, ManifestSource::Release)
            .map_err(|_| WindowsAdapterError::InvalidRuntimeGeneration)?;
        for action in manifest.actions {
            let action_id = format!("{}:{}", manifest.package_id, action.tool_id);
            if actions.insert(action_id, action).is_some() {
                return Err(WindowsAdapterError::InvalidRuntimeGeneration);
            }
        }
    }
    Ok(actions)
}

fn compare_generation_actions(
    active: &BTreeMap<String, star_contracts::manifest::ActionDescriptor>,
    candidate: &BTreeMap<String, star_contracts::manifest::ActionDescriptor>,
) -> Result<ActionComparison, WindowsAdapterError> {
    let mut comparison = ActionComparison::default();
    for (id, candidate_action) in candidate {
        let Some(active_action) = active.get(id) else {
            comparison.added.push(id.clone());
            comparison.permission_widened |= !candidate_action.permission_actions.is_empty();
            comparison.risk_lane_widened |= !candidate_action.permission_actions.is_empty();
            continue;
        };
        let active_value = serde_json::to_value(active_action)?;
        let candidate_value = serde_json::to_value(candidate_action)?;
        if canonical_sha256(&active_value)
            .map_err(|_| WindowsAdapterError::InvalidRuntimeGeneration)?
            != canonical_sha256(&candidate_value)
                .map_err(|_| WindowsAdapterError::InvalidRuntimeGeneration)?
        {
            comparison.changed.push(id.clone());
        }
        comparison.breaking_schema |= active_action.input_schema_file
            != candidate_action.input_schema_file
            || active_action.output_schema_file != candidate_action.output_schema_file;
        let active_permissions = active_action
            .permission_actions
            .iter()
            .collect::<BTreeSet<_>>();
        let candidate_permissions = candidate_action
            .permission_actions
            .iter()
            .collect::<BTreeSet<_>>();
        if !candidate_permissions.is_subset(&active_permissions) {
            comparison.permission_widened = true;
            comparison.risk_lane_widened |= risk_lane(&candidate_action.permission_actions)
                .map_err(|_| WindowsAdapterError::InvalidRuntimeGeneration)?
                != risk_lane(&active_action.permission_actions)
                    .map_err(|_| WindowsAdapterError::InvalidRuntimeGeneration)?;
        }
    }
    for id in active.keys() {
        if !candidate.contains_key(id) {
            comparison.removed.push(id.clone());
            comparison.breaking_schema = true;
        }
    }
    Ok(comparison)
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
    for required in [
        "star.exe",
        "star-controller.exe",
        "star-mcp.exe",
        "star-updater.exe",
    ] {
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
    runtime_activation: Option<(&Path, u32)>,
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
        runtime_activation_record_path: runtime_activation
            .map(|(path, _)| normal_windows_path(path).to_string_lossy().into_owned()),
        bridge_contract_version: runtime_activation.map(|(_, version)| version),
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

fn path_is_within(path: &Path, root: &Path) -> bool {
    path.ancestors()
        .any(|ancestor| paths_equal_case_insensitive(ancestor, root))
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
        for name in [
            "star.exe",
            "star-controller.exe",
            "star-mcp.exe",
            "star-updater.exe",
        ] {
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
            "star-updater.exe",
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

    fn refresh_release_fixture(root: &Path, mut manifest: ReleaseFileManifest) {
        for entry in &mut manifest.files {
            let bytes = std::fs::read(root.join(entry.path.replace('/', "\\"))).unwrap();
            entry.size = bytes.len() as u64;
            entry.sha256 = Sha256Hash::digest(&bytes);
        }
        manifest.set_sha256 =
            canonical_sha256(&serde_json::to_value(&manifest.files).unwrap()).unwrap();
        atomic_write_json(&root.join(RELEASE_MANIFEST_FILE), &manifest).unwrap();
    }

    fn bind_runtime_generations_to_release_fixture(
        root: &Path,
        mut manifest: ReleaseFileManifest,
        generation_ids: &[&str],
    ) -> ReleaseFileManifest {
        for generation_id in generation_ids {
            for name in [
                "runtime-generation.v1.json",
                "runtime-release-manifest.json",
            ] {
                let relative = format!("runtime/generations/{generation_id}/{name}");
                let bytes = std::fs::read(root.join(relative.replace('/', "\\"))).unwrap();
                manifest.files.push(ReleaseFileEntry {
                    path: relative,
                    size: bytes.len() as u64,
                    sha256: Sha256Hash::digest(&bytes),
                });
            }
        }
        manifest
            .files
            .sort_by(|left, right| left.path.cmp(&right.path));
        manifest.set_sha256 =
            canonical_sha256(&serde_json::to_value(&manifest.files).unwrap()).unwrap();
        atomic_write_json(&root.join(RELEASE_MANIFEST_FILE), &manifest).unwrap();
        manifest
    }

    fn write_runtime_generation_fixture(root: &Path, generation_id: &str) -> PathBuf {
        let runtime = root.join(generation_id);
        std::fs::create_dir_all(runtime.join("catalog")).unwrap();
        std::fs::create_dir_all(runtime.join("catalog/tool-packages")).unwrap();
        std::fs::create_dir_all(runtime.join("schemas/v1")).unwrap();
        std::fs::write(runtime.join("catalog/projects.toml"), b"fixture = true\n").unwrap();
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../catalog/tool-packages/star-control-core.toml"),
            runtime.join("catalog/tool-packages/star-control-core.toml"),
        )
        .unwrap();
        std::fs::write(runtime.join("schemas/v1/fixture.schema.json"), b"{}\n").unwrap();
        let binary = std::env::current_exe().unwrap();
        for name in ["star-cli-runtime.exe", "star-controller.exe"] {
            std::fs::copy(&binary, runtime.join(name)).unwrap();
        }
        let mut files = Vec::new();
        for relative in [
            "catalog/projects.toml",
            "catalog/tool-packages/star-control-core.toml",
            "schemas/v1/fixture.schema.json",
            "star-cli-runtime.exe",
            "star-controller.exe",
        ] {
            let bytes = std::fs::read(runtime.join(relative.replace('/', "\\\\"))).unwrap();
            files.push(ReleaseFileEntry {
                path: relative.to_owned(),
                size: bytes.len() as u64,
                sha256: Sha256Hash::digest(&bytes),
            });
        }
        let release = ReleaseFileManifest {
            schema_id: RELEASE_FILE_MANIFEST_SCHEMA_ID.to_owned(),
            schema_version: 1,
            product_version: env!("CARGO_PKG_VERSION").to_owned(),
            target_architecture: compiled_architecture().unwrap(),
            created_at: Utc::now(),
            source_revision: "test:runtime-generation".to_owned(),
            set_sha256: canonical_sha256(&serde_json::to_value(&files).unwrap()).unwrap(),
            files,
            generated_files: vec!["runtime-generation.v1.json".to_owned()],
            signing: PackageSigningState::UnsignedLocal,
        };
        atomic_write_json(&runtime.join("runtime-release-manifest.json"), &release).unwrap();
        let release_bytes = std::fs::read(runtime.join("runtime-release-manifest.json")).unwrap();
        let controller_bytes = std::fs::read(runtime.join("star-controller.exe")).unwrap();
        let generation = RuntimeGenerationManifest {
            schema_id: RUNTIME_GENERATION_MANIFEST_SCHEMA_ID.to_owned(),
            schema_version: 1,
            generation: RuntimeGenerationRef {
                generation_id: generation_id.to_owned(),
                runtime_root: ".".to_owned(),
                release_manifest_sha256: Sha256Hash::digest(&release_bytes),
            },
            product_version: env!("CARGO_PKG_VERSION").to_owned(),
            target_architecture: compiled_architecture().unwrap(),
            controller_path: "star-controller.exe".to_owned(),
            controller_sha256: Sha256Hash::digest(&controller_bytes),
            cli_runtime_path: "star-cli-runtime.exe".to_owned(),
            catalog_path: "catalog".to_owned(),
            schemas_root: "schemas/v1".to_owned(),
            bridge_contract_version: 2,
        };
        atomic_write_json(&runtime.join("runtime-generation.v1.json"), &generation).unwrap();
        runtime
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
    fn staging_a_runtime_generation_verifies_then_never_overwrites_it() {
        let container = fixture_root("runtime-stage");
        let source = write_runtime_generation_fixture(&container, "rt_fixture");
        let install_root = container.join("install");
        std::fs::create_dir_all(&install_root).unwrap();
        let manager = InstallationManager::new(fixture_root("runtime-stage-data"));

        let staged = manager
            .stage_runtime_generation(&install_root, &source)
            .unwrap();
        assert_eq!(staged.generation_id, "rt_fixture");
        let copied = PathBuf::from(&staged.runtime_root);
        assert!(copied.join("runtime-generation.v1.json").exists());
        assert!(matches!(
            manager.stage_runtime_generation(&install_root, &source),
            Err(WindowsAdapterError::RuntimeGenerationExists)
        ));

        std::fs::write(source.join("star-controller.exe"), b"tampered").unwrap();
        let tampered = write_runtime_generation_fixture(&container, "rt_tampered");
        std::fs::write(tampered.join("star-controller.exe"), b"tampered").unwrap();
        assert!(matches!(
            manager.stage_runtime_generation(&install_root, &tampered),
            Err(WindowsAdapterError::InvalidRuntimeGeneration)
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
    fn integration_candidate_classifies_full_stage_changes_without_mutating_installation() {
        let installed = fixture_root("integration-inspect-installed");
        let candidate = fixture_root("integration-inspect-candidate");
        write_release_fixture(&installed);
        let candidate_manifest = write_release_fixture(&candidate);
        let manager = InstallationManager::new(fixture_root("integration-inspect-data"));
        manager
            .finalize(&installed, compiled_architecture().unwrap(), false)
            .unwrap();

        std::fs::write(candidate.join("star-mcp.exe"), b"candidate-mcp").unwrap();
        refresh_release_fixture(&candidate, candidate_manifest);
        let review = manager
            .inspect_integration_candidate(&installed, &candidate)
            .unwrap();
        assert_eq!(
            review.candidate_class,
            IntegrationCandidateClass::CodexIntegrationUpdate
        );
        assert_eq!(review.changed_files, vec!["star-mcp.exe"]);
        assert!(review.requires_codex_restart);
        assert!(review.rollback_available);
        assert!(manager.status(&installed).unwrap().verified);
    }

    #[test]
    fn integration_candidate_apply_is_manifest_committed_and_rolls_back() {
        let installed = fixture_root("integration-apply-installed");
        let candidate = fixture_root("integration-apply-candidate");
        let data = fixture_root("integration-apply-data");
        write_release_fixture(&installed);
        let candidate_manifest = write_release_fixture(&candidate);
        let manager = InstallationManager::new(data);
        manager
            .finalize(&installed, compiled_architecture().unwrap(), false)
            .unwrap();
        std::fs::write(candidate.join("star-mcp.exe"), b"candidate-mcp").unwrap();
        refresh_release_fixture(&candidate, candidate_manifest);
        let review = manager
            .inspect_integration_candidate(&installed, &candidate)
            .unwrap();
        let backup = manager
            .apply_codex_integration_candidate(
                &installed,
                &candidate,
                &review.approval_scope_sha256,
                "upd_test_apply",
            )
            .unwrap();
        assert!(manager.status(&installed).unwrap().verified);
        assert_eq!(
            manager
                .inspect_integration_candidate(&installed, &candidate)
                .unwrap()
                .candidate_class,
            IntegrationCandidateClass::NoChange
        );
        manager
            .rollback_codex_integration_candidate(&installed, &backup)
            .unwrap();
        assert!(manager.status(&installed).unwrap().verified);
        assert_eq!(
            manager
                .inspect_integration_candidate(&installed, &candidate)
                .unwrap()
                .candidate_class,
            IntegrationCandidateClass::CodexIntegrationUpdate
        );
    }

    #[test]
    fn interrupted_integration_candidate_is_recovered_before_next_transaction() {
        let installed = fixture_root("integration-recover-installed");
        let candidate = fixture_root("integration-recover-candidate");
        let data = fixture_root("integration-recover-data");
        write_release_fixture(&installed);
        let candidate_manifest = write_release_fixture(&candidate);
        let manager = InstallationManager::new(data);
        manager
            .finalize(&installed, compiled_architecture().unwrap(), false)
            .unwrap();
        std::fs::write(candidate.join("star-mcp.exe"), b"candidate-mcp").unwrap();
        refresh_release_fixture(&candidate, candidate_manifest);
        let review = manager
            .inspect_integration_candidate(&installed, &candidate)
            .unwrap();
        manager
            .apply_codex_integration_candidate(
                &installed,
                &candidate,
                &review.approval_scope_sha256,
                "upd_test_recover",
            )
            .unwrap();
        assert_eq!(
            manager
                .recover_interrupted_codex_integration_candidates(&installed)
                .unwrap(),
            1
        );
        assert_eq!(
            manager
                .inspect_integration_candidate(&installed, &candidate)
                .unwrap()
                .candidate_class,
            IntegrationCandidateClass::CodexIntegrationUpdate
        );
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

    #[test]
    fn runtime_activation_record_is_atomic_and_rejects_a_generation_outside_install_root() {
        use star_contracts::installation::RuntimeGenerationRef;

        let root = fixture_root("runtime-activation");
        let data = fixture_root("runtime-activation-data");
        let active_root = root.join("runtime").join("generations").join("rt_active");
        let previous_root = root.join("runtime").join("generations").join("rt_previous");
        std::fs::create_dir_all(&active_root).unwrap();
        std::fs::create_dir_all(&previous_root).unwrap();
        let record = RuntimeActivationRecord {
            schema_id: RUNTIME_ACTIVATION_RECORD_SCHEMA_ID.to_owned(),
            schema_version: 1,
            activation_revision: 4,
            active: RuntimeGenerationRef {
                generation_id: "rt_active".to_owned(),
                runtime_root: active_root.canonicalize().unwrap().display().to_string(),
                release_manifest_sha256: Sha256Hash::digest(b"active"),
            },
            previous: Some(RuntimeGenerationRef {
                generation_id: "rt_previous".to_owned(),
                runtime_root: previous_root.canonicalize().unwrap().display().to_string(),
                release_manifest_sha256: Sha256Hash::digest(b"previous"),
            }),
            state_generation_id: "state_4".to_owned(),
            bridge_contract_version: 2,
            activated_at: Utc::now(),
        };
        let manager = InstallationManager::new(data);
        manager
            .write_runtime_activation_record(&root, &record)
            .unwrap();
        assert_eq!(
            manager
                .load_runtime_activation_record(&root)
                .unwrap()
                .activation_revision,
            4
        );

        let mut outside = record;
        outside.active.runtime_root = root.canonicalize().unwrap().display().to_string();
        assert!(matches!(
            manager.write_runtime_activation_record(&root, &outside),
            Err(WindowsAdapterError::InvalidRuntimeActivation)
        ));
    }

    #[test]
    fn bridge_v2_migration_keeps_the_root_gateway_and_binds_the_activation_record() {
        let root = fixture_root("bridge-v2");
        let data = fixture_root("bridge-v2-data");
        write_release_fixture(&root);
        let manager = InstallationManager::new(data);
        manager
            .finalize(&root, compiled_architecture().unwrap(), false)
            .unwrap();
        let source_container = fixture_root("bridge-v2-source");
        let active_source = write_runtime_generation_fixture(&source_container, "rt_active");
        let previous_source = write_runtime_generation_fixture(&source_container, "rt_previous");
        let active = manager
            .stage_runtime_generation(&root, &active_source)
            .unwrap();
        let previous = manager
            .stage_runtime_generation(&root, &previous_source)
            .unwrap();
        let record = RuntimeActivationRecord {
            schema_id: RUNTIME_ACTIVATION_RECORD_SCHEMA_ID.to_owned(),
            schema_version: 1,
            activation_revision: 1,
            active,
            previous: Some(previous),
            state_generation_id: "state_1".to_owned(),
            bridge_contract_version: 2,
            activated_at: Utc::now(),
        };
        manager.activate_runtime_bridge(&root, &record, 2).unwrap();
        let bytes =
            read_regular_bounded(&root.join(CONTROLLER_INSTALL_MANIFEST_FILE), 64 * 1024).unwrap();
        let release = parse_release_manifest(
            &read_regular_bounded(
                &root.join(RELEASE_MANIFEST_FILE),
                RELEASE_MANIFEST_MAX_BYTES,
            )
            .unwrap(),
        )
        .unwrap();
        let manifest =
            parse_controller_manifest(&bytes, &root.canonicalize().unwrap(), &release).unwrap();
        assert_eq!(manifest.bridge_contract_version, Some(2));
        let expected_activation_path =
            normal_windows_path(&manager.runtime_activation_record_path())
                .to_string_lossy()
                .into_owned();
        assert_eq!(
            manifest.runtime_activation_record_path.as_deref(),
            Some(expected_activation_path.as_str())
        );
        assert!(manager.status(&root).unwrap().verified);
    }

    #[test]
    fn offline_bootstrap_initialization_selects_the_single_staged_generation() {
        let root = fixture_root("bridge-initialize");
        let data = fixture_root("bridge-initialize-data");
        write_release_fixture(&root);
        let manager = InstallationManager::new(data);
        manager
            .finalize(&root, compiled_architecture().unwrap(), false)
            .unwrap();
        let source_container = fixture_root("bridge-initialize-source");
        let source = write_runtime_generation_fixture(&source_container, "rt_initial");
        manager.stage_runtime_generation(&root, &source).unwrap();

        let record = manager
            .initialize_runtime_bridge(&root, "bootstrap_v2")
            .unwrap();
        assert_eq!(record.active.generation_id, "rt_initial");
        assert!(record.previous.is_none());
        assert_eq!(
            manager
                .load_runtime_activation_record(&root)
                .unwrap()
                .state_generation_id,
            "bootstrap_v2"
        );
        assert_eq!(
            manager
                .initialize_runtime_bridge(&root, "ignored_after_initialize")
                .unwrap()
                .activation_revision,
            1
        );
    }

    #[test]
    fn replacement_installer_selects_only_the_manifest_owned_runtime_generation() {
        let root = fixture_root("bundled-runtime-selection");
        let data = fixture_root("bundled-runtime-selection-data");
        let release = write_release_fixture(&root);
        let manager = InstallationManager::new(data);
        let source_container = fixture_root("bundled-runtime-selection-source");
        let retained_source = write_runtime_generation_fixture(&source_container, "rt_retained");
        let bundled_source = write_runtime_generation_fixture(&source_container, "rt_bundled");
        let retained = manager
            .stage_runtime_generation(&root, &retained_source)
            .unwrap();
        manager
            .stage_runtime_generation(&root, &bundled_source)
            .unwrap();
        bind_runtime_generations_to_release_fixture(&root, release, &["rt_bundled"]);
        manager
            .finalize(&root, compiled_architecture().unwrap(), false)
            .unwrap();
        manager
            .activate_runtime_bridge(
                &root,
                &RuntimeActivationRecord {
                    schema_id: RUNTIME_ACTIVATION_RECORD_SCHEMA_ID.to_owned(),
                    schema_version: 1,
                    activation_revision: 1,
                    active: retained,
                    previous: None,
                    state_generation_id: "prior_state".to_owned(),
                    bridge_contract_version: 2,
                    activated_at: Utc::now(),
                },
                2,
            )
            .unwrap();

        let bundled = manager.verified_bundled_runtime_generation(&root).unwrap();
        assert_eq!(bundled.generation_id, "rt_bundled");
        let tool_ids = manager.verified_runtime_tool_ids(&root, &bundled).unwrap();
        assert_eq!(tool_ids.len(), 17);
        assert!(tool_ids.contains("star.core.goal.start"));
        assert!(tool_ids.contains("star.core.validation.run"));
    }

    #[test]
    fn replacement_installer_rejects_ambiguous_manifest_owned_generations() {
        let root = fixture_root("bundled-runtime-ambiguous");
        let data = fixture_root("bundled-runtime-ambiguous-data");
        let release = write_release_fixture(&root);
        let manager = InstallationManager::new(data);
        let source_container = fixture_root("bundled-runtime-ambiguous-source");
        for generation_id in ["rt_first", "rt_second"] {
            let source = write_runtime_generation_fixture(&source_container, generation_id);
            manager.stage_runtime_generation(&root, &source).unwrap();
        }
        bind_runtime_generations_to_release_fixture(&root, release, &["rt_first", "rt_second"]);
        manager
            .finalize(&root, compiled_architecture().unwrap(), false)
            .unwrap();

        assert!(matches!(
            manager.verified_bundled_runtime_generation(&root),
            Err(WindowsAdapterError::InvalidRuntimeGeneration)
        ));
    }

    #[test]
    fn candidate_review_compares_release_manifests_without_authorizing_mutation() {
        let root = fixture_root("candidate-review");
        let data = fixture_root("candidate-review-data");
        write_release_fixture(&root);
        let manager = InstallationManager::new(data);
        manager
            .finalize(&root, compiled_architecture().unwrap(), false)
            .unwrap();
        let source_container = fixture_root("candidate-review-source");
        let active_source = write_runtime_generation_fixture(&source_container, "rt_active");
        let candidate_source = write_runtime_generation_fixture(&source_container, "rt_candidate");
        let active = manager
            .stage_runtime_generation(&root, &active_source)
            .unwrap();
        let candidate = manager
            .stage_runtime_generation(&root, &candidate_source)
            .unwrap();
        manager
            .activate_runtime_bridge(
                &root,
                &RuntimeActivationRecord {
                    schema_id: RUNTIME_ACTIVATION_RECORD_SCHEMA_ID.to_owned(),
                    schema_version: 1,
                    activation_revision: 1,
                    active,
                    previous: Some(candidate),
                    state_generation_id: "state_fixture".to_owned(),
                    bridge_contract_version: 2,
                    activated_at: Utc::now(),
                },
                2,
            )
            .unwrap();
        let review = manager
            .inspect_runtime_candidate(&root, "rt_candidate")
            .unwrap();
        assert!(review.added_actions.is_empty());
        assert!(review.removed_actions.is_empty());
        assert!(review.changed_actions.is_empty());
        assert!(review.handler_ready);
        assert!(review.rollback_available);
        assert!(!review.requires_codex_restart);
    }
}
