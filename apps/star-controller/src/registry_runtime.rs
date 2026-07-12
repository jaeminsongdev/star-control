use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{SecondsFormat, Utc};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::OnceLock,
    time::{Duration, Instant},
};
use thiserror::Error;

/// An editor commonly removes the destination immediately before atomically
/// renaming its temporary manifest into place.  Keep a live package through
/// that short gap; a subsequent demand scan after the debounce removes an
/// actual deletion.
const MISSING_SOURCE_DEBOUNCE: Duration = Duration::from_millis(500);
const REQUIRED_RELEASE_MANIFEST_NAME: &str = "star-control-core.toml";
const REQUIRED_RELEASE_MANIFEST: &str =
    include_str!("../../../catalog/tool-packages/star-control-core.toml");

use star_contracts::{
    ToolTrustId,
    canonical::{Sha256Hash, canonical_sha256},
    ids::ToolCacheId,
    manifest::{
        ActionDescriptor, ExecutableDescriptor, LocatorKind, ManifestError, ManifestSource,
        ToolPackageManifest, UpdatePolicy, is_forbidden_executable_name, parse_manifest_v1,
        version_requirement_matches,
    },
    registry::{PackageSnapshot, RegistrySource, SourceFileIdentity, ToolRegistryCache},
};

pub fn executable_requires_probe(executable: &ExecutableDescriptor) -> bool {
    executable.update_policy == UpdatePolicy::VersionCompatible
        || executable.interface_version_req != "*"
        || executable.product_version_req.as_deref().unwrap_or("*") != "*"
}

use crate::manifest_resources::{
    ManifestResources, SchemaLimits, load_manifest_resources, load_manifest_resources_with_limits,
};
use crate::policy_profile::{UserToolRegistryConfig, safe_user_config_path};

#[derive(Clone, Debug)]
pub struct RegistrySourceRoot {
    pub source: ManifestSource,
    pub directory: PathBuf,
}

#[derive(Clone, Debug)]
pub struct ActivePackage {
    pub manifest: ToolPackageManifest,
    pub source: ManifestSource,
    pub source_hash: Sha256Hash,
    pub source_file_identity: SourceFileIdentity,
    pub validated_at: String,
    pub cache_id: ToolCacheId,
    pub path: PathBuf,
    pub resolved_executable_hashes: BTreeMap<String, Sha256Hash>,
    pub resolved_executable_paths: BTreeMap<String, PathBuf>,
    pub probed_product_versions: BTreeMap<String, String>,
    pub probed_interface_versions: BTreeMap<String, Option<String>>,
    pub probed_capabilities: BTreeMap<String, BTreeSet<String>>,
    pub location_config_revision: Option<Sha256Hash>,
    pub fixed_working_directory_hashes: BTreeMap<String, Sha256Hash>,
    pub resources: ManifestResources,
    pub manifest_hash: OnceLock<Sha256Hash>,
    pub semantic_hash: OnceLock<Sha256Hash>,
    pub descriptor_hashes: OnceLock<BTreeMap<String, Sha256Hash>>,
}

#[derive(Clone, Debug)]
pub struct CandidateObservation {
    pub package_version: String,
    pub source: ManifestSource,
    pub path: PathBuf,
    pub state: &'static str,
    pub manifest_hash: Option<Sha256Hash>,
}

#[derive(Clone, Debug)]
pub struct SearchHit<'a> {
    pub package: &'a ActivePackage,
    pub action: &'a ActionDescriptor,
    pub score: i32,
    pub matched_fields: Vec<&'static str>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReplacementResolution {
    /// target package -> trusted active replacer package
    pub replaced_by: BTreeMap<String, String>,
    pub conflicts: BTreeSet<String>,
}

#[derive(Debug, Error)]
pub enum RegistryCacheError {
    #[error("registry cache I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("registry cache is corrupt or from another format")]
    Corrupt,
    #[error("registry cache DACL failed")]
    Dacl,
}

#[derive(Debug, Error)]
pub enum RegistryValidationError {
    #[error("tool package manifest validation failed")]
    Invalid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryCachePayload {
    format_version: u32,
    active: BTreeMap<String, CachedPackage>,
    revision: u64,
    diagnostic_revision: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryCacheEnvelope {
    schema_id: String,
    schema_version: u32,
    protection: String,
    entries: BTreeMap<String, ToolRegistryCache>,
    protected_payload: String,
    payload_sha256: Sha256Hash,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryCacheIntegrity {
    schema_id: String,
    schema_version: u32,
    cache_file: String,
    jcs_sha256: Sha256Hash,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CachedPackage {
    manifest: ToolPackageManifest,
    source: String,
    source_hash: Sha256Hash,
    path: PathBuf,
    #[serde(default)]
    resolved_executable_hashes: BTreeMap<String, Sha256Hash>,
    #[serde(default)]
    resolved_executable_paths: BTreeMap<String, PathBuf>,
    #[serde(default)]
    probed_product_versions: BTreeMap<String, String>,
    #[serde(default)]
    probed_interface_versions: BTreeMap<String, Option<String>>,
    #[serde(default)]
    probed_capabilities: BTreeMap<String, BTreeSet<String>>,
    #[serde(default)]
    location_config_revision: Option<Sha256Hash>,
    #[serde(default)]
    fixed_working_directory_hashes: BTreeMap<String, Sha256Hash>,
    #[serde(default)]
    resources: ManifestResources,
}

#[derive(Default)]
pub struct RegistryRuntime {
    active: BTreeMap<String, ActivePackage>,
    pending_compatible: BTreeMap<String, ActivePackage>,
    nonactive_candidates: BTreeMap<String, ActivePackage>,
    pub diagnostics: BTreeMap<PathBuf, String>,
    pub revision: u64,
    pub diagnostic_revision: u64,
    missing_since: BTreeMap<PathBuf, Instant>,
    cache_loaded: bool,
    locations: BTreeMap<String, PathBuf>,
    observations: BTreeMap<String, CandidateObservation>,
    last_probe_at: BTreeMap<String, String>,
    failed_probe_hashes: BTreeMap<String, Sha256Hash>,
    policy: UserToolRegistryConfig,
    policy_changed: bool,
}

impl RegistryRuntime {
    pub fn validate_manifest(
        &self,
        path: &Path,
        source: ManifestSource,
    ) -> Result<ToolPackageManifest, RegistryValidationError> {
        Self::validate_manifest_path(path, source, &self.locations)
    }

    /// Management-CLI validation through the same parser, Schema resolver and
    /// locator policy used by demand scan, stopping before trust, identity
    /// adoption and probe execution.
    pub fn validate_manifest_path(
        path: &Path,
        source: ManifestSource,
        locations: &BTreeMap<String, PathBuf>,
    ) -> Result<ToolPackageManifest, RegistryValidationError> {
        if path
            .parent()
            .is_none_or(|parent| !safe_registry_root(parent))
        {
            return Err(RegistryValidationError::Invalid);
        }
        let policy = UserToolRegistryConfig::default();
        let text = stable_candidate_texts(
            vec![path.to_path_buf()],
            &BTreeMap::new(),
            policy.max_manifest_bytes,
            Duration::from_millis(policy.stable_file_window_ms),
            Duration::from_millis(policy.stable_file_timeout_ms),
        )
        .into_iter()
        .next()
        .and_then(|(_, candidate)| match candidate {
            StableCandidateText::Stable(text, _) | StableCandidateText::Unchanged(text, _) => {
                Some(text)
            }
            StableCandidateText::Stabilizing | StableCandidateText::Invalid => None,
        })
        .ok_or(RegistryValidationError::Invalid)?;
        let manifest =
            parse_manifest_v1(&text, source).map_err(|_| RegistryValidationError::Invalid)?;
        load_manifest_resources(&manifest, path).map_err(|_| RegistryValidationError::Invalid)?;
        for executable in &manifest.executables {
            resolve_executable_path(executable, path, locations)
                .ok_or(RegistryValidationError::Invalid)?;
        }
        Ok(manifest)
    }

    pub fn load_cache(path: &Path) -> Result<Self, RegistryCacheError> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(RegistryCacheError::Io(error)),
        };
        let envelope_value = star_contracts::parse_no_duplicate_keys(
            std::str::from_utf8(&bytes).map_err(|_| RegistryCacheError::Corrupt)?,
        )
        .map_err(|_| RegistryCacheError::Corrupt)?;
        let envelope: RegistryCacheEnvelope = serde_json::from_value(envelope_value.clone())
            .map_err(|_| RegistryCacheError::Corrupt)?;
        if envelope.schema_id != "star.tool-registry-cache-envelope"
            || envelope.schema_version != 1
            || envelope.protection != "dpapi_current_user"
        {
            return Err(RegistryCacheError::Corrupt);
        }
        let integrity_bytes = fs::read(cache_integrity_path(path))?;
        let integrity_value = star_contracts::parse_no_duplicate_keys(
            std::str::from_utf8(&integrity_bytes).map_err(|_| RegistryCacheError::Corrupt)?,
        )
        .map_err(|_| RegistryCacheError::Corrupt)?;
        let integrity: RegistryCacheIntegrity =
            serde_json::from_value(integrity_value).map_err(|_| RegistryCacheError::Corrupt)?;
        let cache_file = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(RegistryCacheError::Corrupt)?;
        if integrity.schema_id != "star.tool-registry-cache-integrity"
            || integrity.schema_version != 1
            || integrity.cache_file != cache_file
            || integrity.jcs_sha256
                != canonical_sha256(&envelope_value).map_err(|_| RegistryCacheError::Corrupt)?
        {
            return Err(RegistryCacheError::Corrupt);
        }
        let mut entries = envelope.entries;
        let protected = STANDARD
            .decode(envelope.protected_payload)
            .map_err(|_| RegistryCacheError::Corrupt)?;
        let plaintext = star_ipc::dpapi::unprotect_current_user(&protected)
            .map_err(|_| RegistryCacheError::Corrupt)?;
        if Sha256Hash::digest(&plaintext) != envelope.payload_sha256 {
            return Err(RegistryCacheError::Corrupt);
        }
        let payload_value = star_contracts::parse_no_duplicate_keys(
            std::str::from_utf8(&plaintext).map_err(|_| RegistryCacheError::Corrupt)?,
        )
        .map_err(|_| RegistryCacheError::Corrupt)?;
        let cache: RegistryCachePayload =
            serde_json::from_value(payload_value).map_err(|_| RegistryCacheError::Corrupt)?;
        if cache.format_version != 1 {
            return Err(RegistryCacheError::Corrupt);
        }
        if entries.len() != cache.active.len() {
            return Err(RegistryCacheError::Corrupt);
        }
        let mut active = BTreeMap::new();
        for (package_id, package) in cache.active {
            let entry = entries
                .remove(&package_id)
                .ok_or(RegistryCacheError::Corrupt)?;
            let source = match package.source.as_str() {
                "release" => ManifestSource::Release,
                "user" => ManifestSource::User,
                "project" => ManifestSource::Project,
                _ => return Err(RegistryCacheError::Corrupt),
            };
            let active_package = ActivePackage {
                manifest: package.manifest,
                source,
                source_hash: package.source_hash,
                source_file_identity: entry.source_file_identity.clone(),
                validated_at: entry.validated_at.clone(),
                cache_id: entry.cache_id.clone(),
                path: package.path,
                resolved_executable_hashes: package.resolved_executable_hashes,
                resolved_executable_paths: package.resolved_executable_paths,
                probed_product_versions: package.probed_product_versions,
                probed_interface_versions: package.probed_interface_versions,
                probed_capabilities: package.probed_capabilities,
                location_config_revision: package.location_config_revision,
                fixed_working_directory_hashes: package.fixed_working_directory_hashes,
                resources: package.resources,
                manifest_hash: OnceLock::new(),
                semantic_hash: OnceLock::new(),
                descriptor_hashes: OnceLock::new(),
            };
            if entry.schema_id != "star.tool-registry-cache"
                || entry.schema_version != 1
                || entry.mcp_contract_version != 1
                || entry.product_version != env!("CARGO_PKG_VERSION")
                || chrono::DateTime::parse_from_rfc3339(&entry.validated_at).is_err()
                || cache_contract(&active_package, entry.trust_id.clone()) != entry
            {
                return Err(RegistryCacheError::Corrupt);
            }
            active.insert(package_id, active_package);
        }
        if !entries.is_empty() {
            return Err(RegistryCacheError::Corrupt);
        }
        Ok(Self {
            active,
            pending_compatible: BTreeMap::new(),
            nonactive_candidates: BTreeMap::new(),
            diagnostics: BTreeMap::new(),
            revision: cache.revision,
            diagnostic_revision: cache.diagnostic_revision,
            missing_since: BTreeMap::new(),
            cache_loaded: true,
            locations: BTreeMap::new(),
            observations: BTreeMap::new(),
            last_probe_at: BTreeMap::new(),
            failed_probe_hashes: BTreeMap::new(),
            policy: UserToolRegistryConfig::default(),
            policy_changed: false,
        })
    }

    pub fn persist_cache(&self, path: &Path) -> Result<(), RegistryCacheError> {
        self.persist_cache_with_trust_ids(path, &BTreeMap::new())
    }

    pub fn persist_cache_with_trust_ids(
        &self,
        path: &Path,
        trust_ids: &BTreeMap<String, ToolTrustId>,
    ) -> Result<(), RegistryCacheError> {
        let parent = path.parent().ok_or(RegistryCacheError::Corrupt)?;
        fs::create_dir_all(parent)?;
        let active = self
            .active
            .iter()
            .map(|(package_id, package)| {
                (
                    package_id.clone(),
                    CachedPackage {
                        manifest: package.manifest.clone(),
                        source: source_name(package.source).to_owned(),
                        source_hash: package.source_hash.clone(),
                        path: package.path.clone(),
                        resolved_executable_hashes: package.resolved_executable_hashes.clone(),
                        resolved_executable_paths: package.resolved_executable_paths.clone(),
                        probed_product_versions: package.probed_product_versions.clone(),
                        probed_interface_versions: package.probed_interface_versions.clone(),
                        probed_capabilities: package.probed_capabilities.clone(),
                        location_config_revision: package.location_config_revision.clone(),
                        fixed_working_directory_hashes: package
                            .fixed_working_directory_hashes
                            .clone(),
                        resources: package.resources.clone(),
                    },
                )
            })
            .collect();
        let cache = RegistryCachePayload {
            format_version: 1,
            active,
            revision: self.revision,
            diagnostic_revision: self.diagnostic_revision,
        };
        let entries: BTreeMap<_, _> = self
            .active
            .iter()
            .map(|(package_id, package)| {
                (
                    package_id.clone(),
                    cache_contract(package, trust_ids.get(package_id).cloned()),
                )
            })
            .collect();
        let payload = star_contracts::canonical::jcs_bytes(
            &serde_json::to_value(cache).map_err(|_| RegistryCacheError::Corrupt)?,
        )
        .map_err(|_| RegistryCacheError::Corrupt)?;
        let protected = star_ipc::dpapi::protect_current_user(&payload)
            .map_err(|_| RegistryCacheError::Corrupt)?;
        let envelope = serde_json::json!({
            "schema_id":"star.tool-registry-cache-envelope",
            "schema_version":1,
            "protection":"dpapi_current_user",
            "entries":entries,
            "protected_payload":STANDARD.encode(protected),
            "payload_sha256":Sha256Hash::digest(&payload)
        });
        let envelope_bytes =
            serde_json::to_vec_pretty(&envelope).map_err(|_| RegistryCacheError::Corrupt)?;
        let integrity = serde_json::json!({
            "schema_id":"star.tool-registry-cache-integrity",
            "schema_version":1,
            "cache_file":path.file_name().and_then(|value| value.to_str()).ok_or(RegistryCacheError::Corrupt)?,
            "jcs_sha256":canonical_sha256(&envelope).map_err(|_| RegistryCacheError::Corrupt)?
        });
        let integrity_bytes =
            serde_json::to_vec_pretty(&integrity).map_err(|_| RegistryCacheError::Corrupt)?;
        let temporary = parent.join(format!(".registry-cache-{}.tmp", star_ipc::nonce()));
        let integrity_temporary = parent.join(format!(
            ".registry-cache-integrity-{}.tmp",
            star_ipc::nonce()
        ));
        fs::write(&temporary, envelope_bytes)?;
        fs::write(&integrity_temporary, integrity_bytes)?;
        fs::OpenOptions::new()
            .write(true)
            .open(&temporary)?
            .sync_all()?;
        fs::OpenOptions::new()
            .write(true)
            .open(&integrity_temporary)?
            .sync_all()?;
        star_ipc::key_store::apply_owner_system_dacl(&temporary)
            .map_err(|_| RegistryCacheError::Dacl)?;
        star_ipc::key_store::apply_owner_system_dacl(&integrity_temporary)
            .map_err(|_| RegistryCacheError::Dacl)?;
        fs::rename(integrity_temporary, cache_integrity_path(path))?;
        fs::rename(temporary, path)?;
        Ok(())
    }

    pub fn active(&self) -> &BTreeMap<String, ActivePackage> {
        &self.active
    }

    pub fn watch_directories(&self) -> Vec<PathBuf> {
        let mut directories = BTreeSet::new();
        for package in self.active.values().chain(self.pending_compatible.values()) {
            if let Some(parent) = package.path.parent() {
                directories.insert(parent.to_path_buf());
                for action in &package.manifest.actions {
                    for schema in [
                        action.input_schema_file.as_deref(),
                        action.output_schema_file.as_deref(),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        if let Some(parent) = parent.join(schema).parent() {
                            directories.insert(parent.to_path_buf());
                        }
                    }
                }
            }
            for executable in &package.manifest.executables {
                if let Some(path) = package
                    .resolved_executable_paths
                    .get(&executable.executable_id)
                    && let Some(install_root) = path.parent()
                {
                    directories.insert(install_root.to_path_buf());
                    for integrity in &executable.integrity_files {
                        if let Some(parent) = install_root.join(&integrity.path).parent() {
                            directories.insert(parent.to_path_buf());
                        }
                    }
                }
            }
        }
        directories.into_iter().collect()
    }

    pub fn set_locations(&mut self, locations: BTreeMap<String, PathBuf>) {
        self.policy_changed |= self.locations != locations;
        self.locations = locations;
    }

    pub fn set_policy(&mut self, policy: UserToolRegistryConfig) {
        self.policy_changed |= self.policy != policy;
        self.locations = policy.locations.clone();
        self.policy = policy;
    }

    pub fn probe_candidate(&self, package_id: &str) -> Option<&ActivePackage> {
        self.pending_compatible
            .get(package_id)
            .or_else(|| self.active.get(package_id))
    }

    /// Returns at most one deterministic, not-yet-attempted probe. Failed
    /// candidate identities stay suppressed until their bytes or contract
    /// change; an explicit management `probe` request may still retry them.
    pub fn next_automatic_probe(&self) -> Option<(ActivePackage, String)> {
        self.pending_compatible
            .iter()
            .filter(|(package_id, package)| {
                self.failed_probe_hashes
                    .get(*package_id)
                    .is_none_or(|failed| failed != &Self::package_semantic_hash(package))
            })
            .find_map(|(_, package)| {
                package
                    .manifest
                    .executables
                    .iter()
                    .filter(|executable| executable_requires_probe(executable))
                    .find(|executable| {
                        !package
                            .probed_product_versions
                            .contains_key(&executable.executable_id)
                            || !package
                                .probed_interface_versions
                                .contains_key(&executable.executable_id)
                            || !package
                                .probed_capabilities
                                .contains_key(&executable.executable_id)
                    })
                    .map(|executable| (package.clone(), executable.executable_id.clone()))
            })
    }

    pub fn find_effective_action(
        &self,
        tool_id: &str,
        trusted_packages: &BTreeSet<String>,
    ) -> Option<(&ActivePackage, &ActionDescriptor)> {
        self.find_effective_action_with_exclusions(tool_id, trusted_packages, &BTreeSet::new())
    }

    pub fn find_effective_action_with_exclusions(
        &self,
        tool_id: &str,
        trusted_packages: &BTreeSet<String>,
        excluded_packages: &BTreeSet<String>,
    ) -> Option<(&ActivePackage, &ActionDescriptor)> {
        self.search_actions_with_policy(tool_id, trusted_packages, excluded_packages)
            .into_iter()
            .find(|hit| hit.action.tool_id == tool_id)
            .map(|hit| (hit.package, hit.action))
    }

    pub fn find_effective_describable_action(
        &self,
        tool_id: &str,
        trusted_packages: &BTreeSet<String>,
    ) -> Option<(&ActivePackage, &ActionDescriptor)> {
        self.find_effective_describable_action_with_exclusions(
            tool_id,
            trusted_packages,
            &BTreeSet::new(),
        )
    }

    pub fn find_effective_describable_action_with_exclusions(
        &self,
        tool_id: &str,
        trusted_packages: &BTreeSet<String>,
        excluded_packages: &BTreeSet<String>,
    ) -> Option<(&ActivePackage, &ActionDescriptor)> {
        self.search_describable_actions_with_policy(tool_id, trusted_packages, excluded_packages)
            .into_iter()
            .find(|hit| hit.action.tool_id == tool_id)
            .map(|hit| (hit.package, hit.action))
    }

    pub fn candidate_observation(&self, package_id: &str) -> Option<&CandidateObservation> {
        self.observations.get(package_id)
    }

    pub fn last_probe_at(&self, package_id: &str) -> Option<&str> {
        self.last_probe_at.get(package_id).map(String::as_str)
    }

    pub fn status_package_ids(&self) -> BTreeSet<String> {
        self.active
            .keys()
            .chain(self.pending_compatible.keys())
            .chain(self.observations.keys())
            .cloned()
            .collect()
    }

    pub fn accept_compatible_probe(
        &mut self,
        package_id: &str,
        executable_id: &str,
        product_version: String,
        interface_version: Option<String>,
        capabilities: BTreeSet<String>,
    ) -> bool {
        if !self.pending_compatible.contains_key(package_id) {
            let active_path = self.active.get(package_id).and_then(|package| {
                package
                    .manifest
                    .executables
                    .iter()
                    .any(|executable| {
                        executable.executable_id == executable_id && executable.probe.is_some()
                    })
                    .then(|| package.path.clone())
            });
            let Some(active_path) = active_path else {
                return false;
            };
            let had_probe_failure = self.diagnostics.get(&active_path).map(String::as_str)
                == Some("TOOL_PROBE_FAILED_LKG_RETAINED")
                || self.failed_probe_hashes.contains_key(package_id);
            if had_probe_failure
                && self.diagnostics.get(&active_path).map(String::as_str)
                    == Some("TOOL_PROBE_FAILED_LKG_RETAINED")
            {
                self.diagnostics.remove(&active_path);
            }
            if had_probe_failure {
                self.failed_probe_hashes.remove(package_id);
                if let Some(observation) = self.observations.get_mut(package_id)
                    && observation.state == "incompatible"
                {
                    observation.state = "ready";
                }
            }
            self.last_probe_at.insert(
                package_id.to_owned(),
                Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            );
            // The explicit probe changed status evidence but did not activate
            // a pending candidate or mutate the immutable active descriptor.
            self.diagnostic_revision += 1;
            return false;
        }
        let package = self
            .pending_compatible
            .get_mut(package_id)
            .expect("pending package existence was checked");
        if !package.manifest.executables.iter().any(|executable| {
            executable.executable_id == executable_id && executable_requires_probe(executable)
        }) {
            return false;
        }
        package
            .probed_product_versions
            .insert(executable_id.to_owned(), product_version);
        package
            .probed_interface_versions
            .insert(executable_id.to_owned(), interface_version);
        package
            .probed_capabilities
            .insert(executable_id.to_owned(), capabilities);
        let _ = package.semantic_hash.take();
        let _ = package.descriptor_hashes.take();
        self.last_probe_at.insert(
            package_id.to_owned(),
            Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        );
        let all_probed = package
            .manifest
            .executables
            .iter()
            .filter(|executable| executable_requires_probe(executable))
            .all(|executable| {
                package
                    .probed_product_versions
                    .contains_key(&executable.executable_id)
                    && package
                        .probed_interface_versions
                        .contains_key(&executable.executable_id)
                    && package
                        .probed_capabilities
                        .contains_key(&executable.executable_id)
            });
        if !all_probed {
            if let Some(observation) = self.observations.get_mut(package_id) {
                observation.state = "probing";
            }
            self.diagnostic_revision += 1;
            return false;
        }
        let package = self
            .pending_compatible
            .remove(package_id)
            .expect("pending package was just updated");
        let changed = self.active.get(package_id).is_none_or(|active| {
            Self::package_semantic_hash(active) != Self::package_semantic_hash(&package)
        });
        self.diagnostics.remove(&package.path);
        self.failed_probe_hashes.remove(package_id);
        self.active.insert(package_id.to_owned(), package);
        if let Some(observation) = self.observations.get_mut(package_id) {
            observation.state = "ready";
        }
        if changed {
            self.revision += 1;
        }
        self.diagnostic_revision += 1;
        true
    }

    pub fn reject_compatible_probe(&mut self, package_id: &str) -> bool {
        let Some((path, semantic_hash)) = self
            .pending_compatible
            .get(package_id)
            .or_else(|| self.active.get(package_id))
            .map(|package| (package.path.clone(), Self::package_semantic_hash(package)))
        else {
            return false;
        };
        self.diagnostics
            .insert(path, "TOOL_PROBE_FAILED_LKG_RETAINED".to_owned());
        self.failed_probe_hashes
            .insert(package_id.to_owned(), semantic_hash);
        self.last_probe_at.insert(
            package_id.to_owned(),
            Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        );
        if let Some(observation) = self.observations.get_mut(package_id) {
            observation.state = "incompatible";
        }
        // A repeated failure still changes last_probe_at, so status cursors
        // must become stale even when the diagnostic code is unchanged.
        self.diagnostic_revision += 1;
        true
    }

    pub fn core_ready(&self) -> bool {
        self.active.get("star.control.core").is_some_and(|package| {
            package.source == ManifestSource::Release && package.manifest.required
        })
    }

    /// Package hashes are deterministic and intentionally exclude paths,
    /// timestamps and diagnostics. Descriptor hashes are added by the runtime
    /// normalization stage; this early snapshot still makes scanner changes
    /// observable without pinning a process path.
    pub fn snapshot_hash(&self) -> Sha256Hash {
        let packages: Vec<_> = self
            .active
            .iter()
            .map(|(package_id, package)| {
                serde_json::json!({"package_id": package_id, "package_hash": Self::package_semantic_hash(package)})
            })
            .collect();
        canonical_sha256(&serde_json::json!({"packages": packages}))
            .expect("registry snapshot value is canonical JSON")
    }

    pub fn cache_persistence_hash(&self) -> Sha256Hash {
        let packages: Vec<_> = self
            .active
            .iter()
            .map(|(package_id, package)| {
                serde_json::json!({
                    "package_id": package_id,
                    "package_hash": Self::package_semantic_hash(package),
                    "source_hash": package.source_hash,
                    "source_file_identity": package.source_file_identity,
                    "path": package.path,
                    "cache_id": package.cache_id,
                    "validated_at": package.validated_at,
                })
            })
            .collect();
        canonical_sha256(&serde_json::json!({
            "registry_revision": self.revision,
            "diagnostic_revision": self.diagnostic_revision,
            "packages": packages,
        }))
        .expect("registry cache persistence state is canonical JSON")
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<&ActivePackage> {
        let mut seen = BTreeSet::new();
        self.search_actions(query)
            .into_iter()
            .filter_map(|hit| {
                seen.insert(hit.package.manifest.package_id.as_str())
                    .then_some(hit.package)
            })
            .take(limit)
            .collect()
    }

    /// Deterministic v1 lexical search.  Exact and prefix scores are based on
    /// the normalized whole query; summary/description scores count each
    /// distinct query token at most once per field.
    pub fn search_actions(&self, query: &str) -> Vec<SearchHit<'_>> {
        let trusted: BTreeSet<_> = self.active.keys().cloned().collect();
        self.search_actions_with_trust(query, &trusted)
    }

    pub fn search_actions_with_trust(
        &self,
        query: &str,
        trusted_packages: &BTreeSet<String>,
    ) -> Vec<SearchHit<'_>> {
        self.search_actions_with_policy(query, trusted_packages, &BTreeSet::new())
    }

    pub fn search_actions_with_policy(
        &self,
        query: &str,
        trusted_packages: &BTreeSet<String>,
        excluded_packages: &BTreeSet<String>,
    ) -> Vec<SearchHit<'_>> {
        self.search_actions_in_packages(
            query,
            trusted_packages,
            excluded_packages,
            self.active.values().collect(),
        )
    }

    pub fn search_describable_actions_with_trust(
        &self,
        query: &str,
        trusted_packages: &BTreeSet<String>,
    ) -> Vec<SearchHit<'_>> {
        self.search_describable_actions_with_policy(query, trusted_packages, &BTreeSet::new())
    }

    pub fn search_describable_actions_with_policy(
        &self,
        query: &str,
        trusted_packages: &BTreeSet<String>,
        excluded_packages: &BTreeSet<String>,
    ) -> Vec<SearchHit<'_>> {
        let mut packages: BTreeMap<&str, &ActivePackage> = self
            .active
            .values()
            .map(|package| (package.manifest.package_id.as_str(), package))
            .collect();
        for package in self
            .pending_compatible
            .values()
            .chain(self.nonactive_candidates.values())
        {
            packages
                .entry(package.manifest.package_id.as_str())
                .or_insert(package);
        }
        self.search_actions_in_packages(
            query,
            trusted_packages,
            excluded_packages,
            packages.into_values().collect(),
        )
    }

    fn search_actions_in_packages<'a>(
        &'a self,
        query: &str,
        trusted_packages: &BTreeSet<String>,
        excluded_packages: &BTreeSet<String>,
        packages: Vec<&'a ActivePackage>,
    ) -> Vec<SearchHit<'a>> {
        let normalized_query = normalize_search_text(query).trim().to_owned();
        let query_tokens = search_tokens(&normalized_query);
        let replacements =
            self.resolve_replacements_with_exclusions(trusted_packages, excluded_packages);
        let packages: Vec<_> = packages
            .into_iter()
            .filter(|package| {
                !excluded_packages.contains(&package.manifest.package_id)
                    && !replacements
                        .replaced_by
                        .contains_key(&package.manifest.package_id)
                    && !replacements
                        .conflicts
                        .contains(&package.manifest.package_id)
            })
            .collect();

        // ToolId is a global action identity. A trusted replacement may hide
        // its target package, but an unrelated package must never win merely
        // because its PackageId sorts first. Ambiguous ToolIds therefore fail
        // closed. Required release actions retain their frozen ownership and
        // cannot be shadowed by user or project manifests.
        let mut tool_owners: BTreeMap<&str, (Option<&str>, bool)> = BTreeMap::new();
        for package in &packages {
            let package_id = package.manifest.package_id.as_str();
            let required_release =
                package.source == ManifestSource::Release && package.manifest.required;
            for action in &package.manifest.actions {
                let owner = tool_owners
                    .entry(action.tool_id.as_str())
                    .or_insert((Some(package_id), required_release));
                if owner.0 == Some(package_id) {
                    continue;
                }
                if owner.1 {
                    continue;
                }
                if required_release {
                    *owner = (Some(package_id), true);
                } else {
                    *owner = (None, false);
                }
            }
        }

        let mut hits = Vec::new();
        for package in packages {
            let package_id = package.manifest.package_id.as_str();
            for action in &package.manifest.actions {
                if tool_owners
                    .get(action.tool_id.as_str())
                    .and_then(|(owner, _)| *owner)
                    != Some(package_id)
                {
                    continue;
                }
                let tool_id = normalize_search_text(&action.tool_id);
                let aliases: Vec<_> = action
                    .aliases
                    .iter()
                    .map(|alias| normalize_search_text(alias))
                    .collect();
                let tags: BTreeSet<_> = action
                    .tags
                    .iter()
                    .chain(&action.task_kinds)
                    .map(|value| normalize_search_text(value))
                    .collect();
                let summary_tokens = search_tokens(&action.summary);
                let description_tokens = search_tokens(&action.description);
                let mut score = 0;
                let mut matched_fields = Vec::new();
                if normalized_query.is_empty() {
                    score = 1;
                } else {
                    if tool_id == normalized_query {
                        score += 1_000;
                        matched_fields.push("tool_id");
                    } else if tool_id.starts_with(&normalized_query) {
                        score += 600;
                        matched_fields.push("tool_id");
                    }
                    let alias_score = aliases.iter().fold(0, |best, alias| {
                        best.max(if alias == &normalized_query {
                            800
                        } else if alias.starts_with(&normalized_query) {
                            500
                        } else {
                            0
                        })
                    });
                    if alias_score > 0 {
                        score += alias_score;
                        matched_fields.push("alias");
                    }
                    if tags.contains(&normalized_query)
                        || query_tokens.iter().any(|token| tags.contains(token))
                    {
                        score += 300;
                        if action
                            .tags
                            .iter()
                            .map(|value| normalize_search_text(value))
                            .any(|value| value == normalized_query || query_tokens.contains(&value))
                        {
                            matched_fields.push("tag");
                        }
                        if action
                            .task_kinds
                            .iter()
                            .map(|value| normalize_search_text(value))
                            .any(|value| value == normalized_query || query_tokens.contains(&value))
                        {
                            matched_fields.push("task_kind");
                        }
                    }
                    let summary_matches = query_tokens.intersection(&summary_tokens).count() as i32;
                    if summary_matches > 0 {
                        score += summary_matches * 40;
                        matched_fields.push("summary");
                    }
                    let description_matches =
                        query_tokens.intersection(&description_tokens).count() as i32;
                    if description_matches > 0 {
                        score += description_matches * 10;
                        matched_fields.push("description");
                    }
                }
                if score > 0 {
                    matched_fields.sort_unstable();
                    matched_fields.dedup();
                    hits.push(SearchHit {
                        package,
                        action,
                        score,
                        matched_fields,
                    });
                }
            }
        }
        hits.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.action.tool_id.cmp(&right.action.tool_id))
        });
        hits
    }

    pub fn resolve_replacements(
        &self,
        trusted_packages: &BTreeSet<String>,
    ) -> ReplacementResolution {
        self.resolve_replacements_with_exclusions(trusted_packages, &BTreeSet::new())
    }

    pub fn resolve_replacements_with_exclusions(
        &self,
        trusted_packages: &BTreeSet<String>,
        excluded_packages: &BTreeSet<String>,
    ) -> ReplacementResolution {
        let mut candidates: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut conflicts = BTreeSet::new();
        for replacer in self.active.values() {
            let replacer_id = &replacer.manifest.package_id;
            if !trusted_packages.contains(replacer_id) || excluded_packages.contains(replacer_id) {
                continue;
            }
            for replacement in &replacer.manifest.replaces {
                let Some(target) = self.active.get(&replacement.package_id) else {
                    conflicts.insert(replacer_id.clone());
                    continue;
                };
                if excluded_packages.contains(&replacement.package_id)
                    || (target.source == ManifestSource::Release && target.manifest.required)
                    || !version_requirement_matches(
                        &replacement.version_req,
                        &target.manifest.package_version,
                    )
                {
                    conflicts.insert(replacer_id.clone());
                    continue;
                }
                candidates
                    .entry(replacement.package_id.clone())
                    .or_default()
                    .push(replacer_id.clone());
            }
        }
        let mut replaced_by = BTreeMap::new();
        for (target, mut replacers) in candidates {
            replacers.sort();
            replacers.dedup();
            if replacers.len() != 1 {
                conflicts.extend(replacers);
            } else {
                replaced_by.insert(target, replacers.remove(0));
            }
        }

        for start in replaced_by.keys().cloned().collect::<Vec<_>>() {
            let mut chain = Vec::new();
            let mut current = start;
            while let Some(next) = replaced_by.get(&current).cloned() {
                if chain.contains(&current) || chain.contains(&next) || chain.len() >= 8 {
                    conflicts.extend(chain.clone());
                    conflicts.insert(current);
                    conflicts.insert(next);
                    break;
                }
                chain.push(current);
                current = next;
            }
        }
        replaced_by.retain(|target, replacer| {
            !conflicts.contains(target) && !conflicts.contains(replacer)
        });
        ReplacementResolution {
            replaced_by,
            conflicts,
        }
    }

    #[cfg(test)]
    fn find_action(&self, tool_id: &str) -> Option<(&ActivePackage, &ActionDescriptor)> {
        self.active.values().find_map(|package| {
            package
                .manifest
                .actions
                .iter()
                .find(|action| action.tool_id == tool_id)
                .map(|action| (package, action))
        })
    }

    pub fn descriptor_hash(package: &ActivePackage, action: &ActionDescriptor) -> Sha256Hash {
        package
            .descriptor_hashes
            .get_or_init(|| {
                package
                    .manifest
                    .actions
                    .iter()
                    .map(|action| {
                        (
                            action.tool_id.clone(),
                            Self::compute_descriptor_hash(package, action),
                        )
                    })
                    .collect()
            })
            .get(&action.tool_id)
            .cloned()
            .unwrap_or_else(|| Self::compute_descriptor_hash(package, action))
    }

    fn compute_descriptor_hash(package: &ActivePackage, action: &ActionDescriptor) -> Sha256Hash {
        let action = normalized_action(action);
        let executable_identity = package.resolved_executable_hashes.get(&action.backend_ref);
        let executable_path_hash = package
            .resolved_executable_paths
            .get(&action.backend_ref)
            .map(|path| Sha256Hash::digest(path.as_os_str().to_string_lossy().as_bytes()));
        let schemas = package.resources.action_schemas.get(&action.tool_id);
        canonical_sha256(&serde_json::json!({
            "package_id": package.manifest.package_id,
            "package_version": package.manifest.package_version,
            "source": source_name(package.source),
            "package_hash": Self::package_semantic_hash(package),
            "action": action,
            "resolved_schemas": schemas,
            "resolved_executable_path_hash": executable_path_hash,
            "resolved_executable_identity": executable_identity,
        }))
        .expect("normalized descriptor is canonical JSON")
    }

    /// Descriptor and snapshot identity use parsed manifest semantics, never
    /// source formatting. `source_hash` remains separately available for
    /// candidate provenance and trust decisions.
    pub fn package_semantic_hash(package: &ActivePackage) -> Sha256Hash {
        package
            .semantic_hash
            .get_or_init(|| {
                canonical_sha256(&serde_json::json!({
                    "source": source_name(package.source),
                    "manifest_hash": Self::manifest_hash(package),
                    "schema_hashes": package.resources.schema_hashes,
                    "resolved_executable_hashes": package.resolved_executable_hashes,
                    "resolved_executable_path_hashes": package.resolved_executable_paths.iter().map(|(id, path)| (id, Sha256Hash::digest(path.as_os_str().to_string_lossy().as_bytes()))).collect::<BTreeMap<_, _>>(),
                    "probed_product_versions":package.probed_product_versions,
                    "probed_interface_versions":package.probed_interface_versions,
                    "probed_capabilities":package.probed_capabilities,
                    "location_config_revision":package.location_config_revision,
                    "fixed_working_directory_hashes":package.fixed_working_directory_hashes,
                }))
                .expect("parsed manifest is canonical JSON")
            })
            .clone()
    }

    pub fn manifest_hash(package: &ActivePackage) -> Sha256Hash {
        package
            .manifest_hash
            .get_or_init(|| manifest_value_hash(&package.manifest))
            .clone()
    }

    /// Reads all source roots on every request as the watcher-loss fallback.
    /// A syntactically invalid replacement keeps the old package only when the
    /// same candidate file still exists. A deleted source file never revives
    /// from last-known-good state, and duplicate PackageId candidates are a
    /// conflict rather than a source-order override.
    pub fn demand_scan(&mut self, roots: &[RegistrySourceRoot]) {
        let force_revalidate = std::mem::take(&mut self.policy_changed);
        let before_active: BTreeMap<_, _> = self
            .active
            .iter()
            .map(|(id, package)| (id.clone(), Self::package_semantic_hash(package)))
            .collect();
        let before_diagnostics = self.diagnostics.clone();
        let mut candidates: BTreeMap<String, Vec<ActivePackage>> = BTreeMap::new();
        let mut present_files = BTreeSet::new();
        let mut valid_path_to_id = BTreeMap::new();
        let mut diagnostics = BTreeMap::new();
        let mut executable_hash_cache = BTreeMap::new();
        let mut executable_path_cache = BTreeMap::new();
        let mut working_directory_hash_cache = BTreeMap::new();
        let mut observations = BTreeMap::new();
        let mut nonactive_candidates = BTreeMap::new();
        let mut seen_package_paths: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
        let known_packages: BTreeMap<_, _> = self
            .active
            .values()
            .chain(self.pending_compatible.values())
            .chain(self.nonactive_candidates.values())
            .map(|package| (package.path.clone(), package))
            .collect();
        let known_files: BTreeMap<_, _> = known_packages
            .values()
            .filter(|_| !force_revalidate)
            .map(|package| {
                (
                    package.path.clone(),
                    (
                        package.source_file_identity.clone(),
                        package.source_hash.clone(),
                    ),
                )
            })
            .collect();
        let mut remaining_new_candidates = self
            .policy
            .max_packages
            .max(1)
            .saturating_sub(known_packages.len());
        let scan_batches: Vec<_> = roots
            .iter()
            .map(|root| {
                let preferred: BTreeSet<_> = known_packages
                    .keys()
                    .filter(|path| path.parent() == Some(root.directory.as_path()))
                    .cloned()
                    .collect();
                let (manifest_files, overflowed) =
                    manifest_files(&root.directory, &preferred, remaining_new_candidates);
                let selected_new_candidates = manifest_files
                    .iter()
                    .filter(|path| !preferred.contains(*path))
                    .count();
                remaining_new_candidates =
                    remaining_new_candidates.saturating_sub(selected_new_candidates);
                (
                    root,
                    overflowed,
                    stable_candidate_texts(
                        manifest_files,
                        &known_files,
                        self.policy.max_manifest_bytes,
                        Duration::from_millis(
                            self.policy
                                .stable_file_window_ms
                                .max(self.policy.reload_debounce_ms),
                        ),
                        Duration::from_millis(self.policy.stable_file_timeout_ms),
                    ),
                )
            })
            .collect();
        let cached_current = !force_revalidate
            && !self.cache_loaded
            && self.missing_since.is_empty()
            && cached_scan_is_current(
                &scan_batches,
                &known_packages,
                &self.locations,
                &self.policy,
            );
        if cached_current {
            return;
        }

        for (root, overflowed, candidate_texts) in scan_batches {
            if overflowed {
                diagnostics.insert(root.directory.clone(), "TOOL_REGISTRY_LIMIT".to_owned());
            }
            for (path, text) in candidate_texts {
                present_files.insert(path.clone());
                let (text, source_file_identity, cached_package) = match text {
                    StableCandidateText::Stable(text, identity) => (text, identity, None),
                    StableCandidateText::Unchanged(text, identity) => {
                        let cached = known_packages
                            .get(&path)
                            .copied()
                            .filter(|package| package.source == root.source)
                            .cloned();
                        (text, identity, cached)
                    }
                    StableCandidateText::Stabilizing => {
                        diagnostics.insert(path, "TOOL_MANIFEST_STABILIZING".to_owned());
                        continue;
                    }
                    StableCandidateText::Invalid => {
                        diagnostics.insert(path, "TOOL_MANIFEST_INVALID".to_owned());
                        continue;
                    }
                };
                if root.source == ManifestSource::Release
                    && !release_manifest_integrity_matches(&path, &text)
                {
                    diagnostics.insert(path, "TOOL_INTEGRITY_INVALID".to_owned());
                    continue;
                }
                let mut manifest = if let Some(package) = &cached_package {
                    package.manifest.clone()
                } else {
                    match parse_manifest_v1(&text, root.source) {
                        Ok(manifest) => manifest,
                        Err(ManifestError::FutureFormatVersion(_)) => {
                            diagnostics.insert(path, "TOOL_MANIFEST_FUTURE_VERSION".to_owned());
                            continue;
                        }
                        Err(_) => {
                            diagnostics.insert(path, "TOOL_MANIFEST_INVALID".to_owned());
                            continue;
                        }
                    }
                };
                if !apply_registry_policy(&mut manifest, root.source, &self.policy) {
                    diagnostics.insert(path, "TOOL_UPDATE_POLICY_DENIED".to_owned());
                    continue;
                }
                let location_config_revision = manifest
                    .executables
                    .iter()
                    .any(|executable| executable.locator_kind == LocatorKind::LocationRef)
                    .then(|| self.policy.config_revision.clone());
                let Some(fixed_working_directory_hashes) =
                    resolve_fixed_working_directory_hashes_cached(
                        &manifest,
                        &mut working_directory_hash_cache,
                    )
                else {
                    diagnostics.insert(path, "TOOL_MANIFEST_INVALID".to_owned());
                    continue;
                };
                let package_id = manifest.package_id.clone();
                seen_package_paths
                    .entry(package_id.clone())
                    .or_default()
                    .push(path.clone());
                observations.insert(
                    package_id.clone(),
                    CandidateObservation {
                        package_version: manifest.package_version.clone(),
                        source: root.source,
                        path: path.clone(),
                        state: "validating",
                        manifest_hash: Some(
                            cached_package.as_ref().map_or_else(
                                || manifest_value_hash(&manifest),
                                Self::manifest_hash,
                            ),
                        ),
                    },
                );
                let resources = if let Some(package) = &cached_package
                    && !manifest_uses_external_schemas(&manifest)
                {
                    Ok(package.resources.clone())
                } else {
                    load_manifest_resources_with_limits(
                        &manifest,
                        &path,
                        SchemaLimits {
                            package_bytes: self.policy.max_schema_bytes,
                            action_resolved_bytes: self.policy.max_action_schema_bytes,
                            max_ref_depth: self.policy.max_schema_depth,
                        },
                    )
                };
                let resources = match resources {
                    Ok(resources) => resources,
                    Err(_) => {
                        if let Some(observation) = observations.get_mut(&package_id) {
                            observation.state = "invalid";
                        }
                        diagnostics.insert(path, "TOOL_MANIFEST_SCHEMA_INVALID".to_owned());
                        continue;
                    }
                };
                if !manifest.enabled {
                    if let Some(observation) = observations.get_mut(&package_id) {
                        observation.state = "disabled";
                    }
                    diagnostics.insert(path, "TOOL_PACKAGE_DISABLED".to_owned());
                    continue;
                }
                let Some((resolved_executable_paths, resolved_executable_hashes)) =
                    resolve_executables(
                        &manifest,
                        &path,
                        &self.locations,
                        &mut executable_path_cache,
                        &mut executable_hash_cache,
                    )
                else {
                    if let Some(observation) = observations.get_mut(&package_id) {
                        observation.state = "unavailable";
                    }
                    nonactive_candidates.insert(
                        package_id,
                        ActivePackage {
                            source_hash: Sha256Hash::digest(text.as_bytes()),
                            source_file_identity: source_file_identity.clone(),
                            validated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
                            cache_id: ToolCacheId::new(),
                            source: root.source,
                            path: path.clone(),
                            resolved_executable_hashes: BTreeMap::new(),
                            resolved_executable_paths: BTreeMap::new(),
                            probed_product_versions: BTreeMap::new(),
                            probed_interface_versions: BTreeMap::new(),
                            probed_capabilities: BTreeMap::new(),
                            location_config_revision: location_config_revision.clone(),
                            fixed_working_directory_hashes: fixed_working_directory_hashes.clone(),
                            resources,
                            manifest,
                            manifest_hash: cached_package
                                .as_ref()
                                .map_or_else(OnceLock::new, |package| {
                                    package.manifest_hash.clone()
                                }),
                            semantic_hash: OnceLock::new(),
                            descriptor_hashes: OnceLock::new(),
                        },
                    );
                    diagnostics.insert(path, "TOOL_EXECUTABLE_UNAVAILABLE".to_owned());
                    continue;
                };
                if let Some(observation) = observations.get_mut(&package_id) {
                    observation.state = "ready";
                }
                valid_path_to_id.insert(path.clone(), manifest.package_id.clone());
                let source_hash = cached_package.as_ref().map_or_else(
                    || Sha256Hash::digest(text.as_bytes()),
                    |package| package.source_hash.clone(),
                );
                let (
                    validated_at,
                    cache_id,
                    probed_product_versions,
                    probed_interface_versions,
                    probed_capabilities,
                ) = cached_package.as_ref().map_or_else(
                    || {
                        (
                            Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
                            ToolCacheId::new(),
                            BTreeMap::new(),
                            BTreeMap::new(),
                            BTreeMap::new(),
                        )
                    },
                    |package| {
                        (
                            package.validated_at.clone(),
                            package.cache_id.clone(),
                            package.probed_product_versions.clone(),
                            package.probed_interface_versions.clone(),
                            package.probed_capabilities.clone(),
                        )
                    },
                );
                let manifest_hash = cached_package
                    .as_ref()
                    .map_or_else(OnceLock::new, |package| package.manifest_hash.clone());
                let semantic_hash = cached_package
                    .as_ref()
                    .filter(|package| {
                        package.resolved_executable_hashes == resolved_executable_hashes
                            && package.resolved_executable_paths == resolved_executable_paths
                            && package.probed_product_versions == probed_product_versions
                            && package.probed_interface_versions == probed_interface_versions
                            && package.probed_capabilities == probed_capabilities
                            && package.location_config_revision == location_config_revision
                            && package.fixed_working_directory_hashes
                                == fixed_working_directory_hashes
                            && package.resources == resources
                    })
                    .map_or_else(OnceLock::new, |package| package.semantic_hash.clone());
                let descriptor_hashes = cached_package
                    .as_ref()
                    .filter(|package| semantic_hash.get() == package.semantic_hash.get())
                    .map_or_else(OnceLock::new, |package| package.descriptor_hashes.clone());
                candidates
                    .entry(manifest.package_id.clone())
                    .or_default()
                    .push(ActivePackage {
                        source_hash,
                        source_file_identity,
                        validated_at,
                        cache_id,
                        source: root.source,
                        path,
                        resolved_executable_hashes,
                        resolved_executable_paths,
                        probed_product_versions,
                        probed_interface_versions,
                        probed_capabilities,
                        location_config_revision,
                        fixed_working_directory_hashes,
                        resources,
                        manifest,
                        manifest_hash,
                        semantic_hash,
                        descriptor_hashes,
                    });
            }
        }
        drop(known_packages);

        let conflicts: BTreeSet<String> = seen_package_paths
            .iter()
            .filter(|(_, paths)| paths.len() > 1)
            .map(|(package_id, paths)| {
                for path in paths {
                    diagnostics.insert(path.clone(), "TOOL_PACKAGE_CONFLICT".to_owned());
                }
                if let Some(observation) = observations.get_mut(package_id) {
                    observation.state = "invalid";
                }
                nonactive_candidates.remove(package_id);
                package_id.clone()
            })
            .collect();
        let desired: BTreeMap<String, ActivePackage> = candidates
            .into_iter()
            .filter_map(|(package_id, mut candidates)| {
                (candidates.len() == 1 && !conflicts.contains(&package_id))
                    .then(|| (package_id, candidates.remove(0)))
            })
            .collect();
        // At capacity, an already active valid package wins over a newly
        // discovered package regardless of lexical path/ID order.  This
        // prevents adding one candidate from evicting a working tool or
        // accidentally publishing more than the documented limit.
        let active_ids: BTreeSet<_> = self.active.keys().cloned().collect();
        let mut candidates: Vec<_> = desired.into_iter().collect();
        candidates.sort_by(|(left_id, _), (right_id, _)| {
            active_ids
                .contains(right_id)
                .cmp(&active_ids.contains(left_id))
                .then_with(|| left_id.cmp(right_id))
        });
        let mut selected_packages = 0usize;
        let mut selected_actions = 0usize;
        let mut desired = BTreeMap::new();
        for (package_id, package) in candidates {
            let action_count = package.manifest.actions.len();
            if action_count > self.policy.max_actions_per_package
                || selected_packages >= self.policy.max_packages
                || selected_actions.saturating_add(action_count) > self.policy.max_tools
            {
                if let Some(observation) = observations.get_mut(&package_id) {
                    observation.state = "invalid";
                }
                diagnostics.insert(package.path.clone(), "TOOL_REGISTRY_LIMIT".to_owned());
            } else {
                selected_packages += 1;
                selected_actions += action_count;
                desired.insert(package_id, package);
            }
        }

        // A version-compatible byte identity is only a candidate until its
        // declared probe (and signature policy in the Controller) succeeds.
        // The previously active package therefore remains the LKG. FollowPath
        // is intentionally different: its new full hash is published at once
        // so an old descriptor becomes stale and forces a fresh describe.
        let mut compatible_seen = BTreeSet::new();
        let mut probe_ready = BTreeMap::new();
        for (package_id, mut package) in desired {
            if let Some(previous) = self
                .pending_compatible
                .get(&package_id)
                .or_else(|| self.active.get(&package_id))
                .filter(|previous| same_probe_candidate(previous, &package))
            {
                package.probed_product_versions = previous.probed_product_versions.clone();
                package.probed_interface_versions = previous.probed_interface_versions.clone();
                package.probed_capabilities = previous.probed_capabilities.clone();
            }
            let requires_probe = package
                .manifest
                .executables
                .iter()
                .any(executable_requires_probe);
            let already_active = self.active.get(&package_id).is_some_and(|active| {
                Self::package_semantic_hash(active) == Self::package_semantic_hash(&package)
            });
            if requires_probe && !already_active {
                compatible_seen.insert(package_id.clone());
                let candidate_hash = Self::package_semantic_hash(&package);
                let failed = self
                    .failed_probe_hashes
                    .get(&package_id)
                    .is_some_and(|hash| hash == &candidate_hash);
                if !failed {
                    self.failed_probe_hashes.remove(&package_id);
                }
                if let Some(observation) = observations.get_mut(&package_id) {
                    observation.state = if failed { "incompatible" } else { "probing" };
                }
                diagnostics.insert(
                    package.path.clone(),
                    if failed {
                        "TOOL_PROBE_FAILED_LKG_RETAINED"
                    } else {
                        "TOOL_PROBE_REQUIRED"
                    }
                    .to_owned(),
                );
                self.pending_compatible.insert(package_id, package);
            } else {
                probe_ready.insert(package_id, package);
            }
        }
        self.pending_compatible
            .retain(|package_id, _| compatible_seen.contains(package_id));
        let desired = probe_ready;

        // An invalid replacement is the LKG case.  A freshly missing path is
        // also retained briefly so editor temp-rename saves do not cause a
        // transient tool disappearance.  Cache recovery is deliberately
        // different: a source absent on its first post-restart scan is a real
        // deletion and must never be revived from the cache.
        let cache_first_scan = self.cache_loaded;
        self.cache_loaded = false;
        let mut missing_since = std::mem::take(&mut self.missing_since);
        self.active.retain(|package_id, package| {
            if conflicts.contains(package_id) {
                return false;
            }
            let source_is_configured = roots.iter().any(|root| {
                root.source == package.source
                    && package.path.parent() == Some(root.directory.as_path())
            });
            if !source_is_configured {
                return false;
            }
            if !present_files.contains(&package.path) {
                if cache_first_scan {
                    return false;
                }
                let first_missing = missing_since
                    .entry(package.path.clone())
                    .or_insert_with(Instant::now);
                if first_missing.elapsed() < MISSING_SOURCE_DEBOUNCE {
                    diagnostics.insert(package.path.clone(), "TOOL_MANIFEST_RENAMING".to_owned());
                    return true;
                }
                return false;
            }
            missing_since.remove(&package.path);
            match valid_path_to_id.get(&package.path) {
                Some(candidate_id) => candidate_id == package_id,
                None => {
                    self.policy.persist_last_known_good
                        && matches!(
                            diagnostics.get(&package.path).map(String::as_str),
                            Some(
                                "TOOL_MANIFEST_INVALID"
                                    | "TOOL_MANIFEST_SCHEMA_INVALID"
                                    | "TOOL_MANIFEST_FUTURE_VERSION"
                                    | "TOOL_EXECUTABLE_UNAVAILABLE"
                                    | "TOOL_MANIFEST_STABILIZING"
                                    | "TOOL_INTEGRITY_INVALID"
                                    | "TOOL_REGISTRY_LIMIT"
                            )
                        )
                }
            }
        });
        missing_since.retain(|path, _| self.active.values().any(|package| &package.path == path));
        self.missing_since = missing_since;
        for (package_id, mut package) in desired {
            if let Some(active) = self.active.get(&package_id)
                && active.source_hash == package.source_hash
                && active.source_file_identity == package.source_file_identity
                && Self::package_semantic_hash(active) == Self::package_semantic_hash(&package)
            {
                package.cache_id = active.cache_id.clone();
                package.validated_at = active.validated_at.clone();
            }
            self.active.insert(package_id, package);
        }

        for (package_id, package) in &self.active {
            if observations.contains_key(package_id) {
                continue;
            }
            let Some(code) = diagnostics.get(&package.path) else {
                continue;
            };
            let state = match code.as_str() {
                "TOOL_MANIFEST_STABILIZING" | "TOOL_MANIFEST_RENAMING" => "stabilizing",
                "TOOL_EXECUTABLE_UNAVAILABLE" => "unavailable",
                "TOOL_PROBE_FAILED_LKG_RETAINED" => "incompatible",
                "TOOL_PACKAGE_DISABLED" => "disabled",
                _ => "invalid",
            };
            observations.insert(
                package_id.clone(),
                CandidateObservation {
                    package_version: package.manifest.package_version.clone(),
                    source: package.source,
                    path: package.path.clone(),
                    state,
                    manifest_hash: None,
                },
            );
        }
        // `max_tools` is the discovery surface bound, not merely the active
        // execution-snapshot bound. Pending probe and unavailable descriptors
        // without an active LKG therefore share the remaining global budget.
        // A same-PackageId candidate does not add a second searchable entry
        // because the active descriptor remains the discovery owner.
        let mut searchable_actions = self
            .active
            .values()
            .map(|package| package.manifest.actions.len())
            .sum::<usize>();
        let mut bounded_pending = BTreeMap::new();
        for (package_id, package) in std::mem::take(&mut self.pending_compatible) {
            let action_count = package.manifest.actions.len();
            if self.active.contains_key(&package_id)
                || searchable_actions.saturating_add(action_count) <= self.policy.max_tools
            {
                if !self.active.contains_key(&package_id) {
                    searchable_actions += action_count;
                }
                bounded_pending.insert(package_id, package);
            } else {
                if let Some(observation) = observations.get_mut(&package_id) {
                    observation.state = "invalid";
                }
                diagnostics.insert(package.path.clone(), "TOOL_REGISTRY_LIMIT".to_owned());
            }
        }
        self.pending_compatible = bounded_pending;
        let mut bounded_nonactive = BTreeMap::new();
        for (package_id, package) in nonactive_candidates {
            let action_count = package.manifest.actions.len();
            if self.active.contains_key(&package_id)
                || searchable_actions.saturating_add(action_count) <= self.policy.max_tools
            {
                if !self.active.contains_key(&package_id) {
                    searchable_actions += action_count;
                }
                bounded_nonactive.insert(package_id, package);
            } else {
                if let Some(observation) = observations.get_mut(&package_id) {
                    observation.state = "invalid";
                }
                diagnostics.insert(package.path.clone(), "TOOL_REGISTRY_LIMIT".to_owned());
            }
        }
        self.observations = observations;
        self.nonactive_candidates = bounded_nonactive;

        let after_active: BTreeMap<_, _> = self
            .active
            .iter()
            .map(|(id, package)| (id.clone(), Self::package_semantic_hash(package)))
            .collect();
        if before_active != after_active {
            self.revision += 1;
        }
        if before_diagnostics != diagnostics {
            self.diagnostic_revision += 1;
        }
        self.diagnostics = diagnostics;
    }
}

fn release_manifest_integrity_matches(path: &Path, text: &str) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(REQUIRED_RELEASE_MANIFEST_NAME))
        && Sha256Hash::digest(text.as_bytes())
            == Sha256Hash::digest(REQUIRED_RELEASE_MANIFEST.as_bytes())
}

fn same_probe_candidate(left: &ActivePackage, right: &ActivePackage) -> bool {
    left.source == right.source
        && left.path == right.path
        && RegistryRuntime::manifest_hash(left) == RegistryRuntime::manifest_hash(right)
        && left.resolved_executable_hashes == right.resolved_executable_hashes
        && left.resolved_executable_paths == right.resolved_executable_paths
        && left.location_config_revision == right.location_config_revision
        && left.fixed_working_directory_hashes == right.fixed_working_directory_hashes
        && left.resources == right.resources
}

/// Normalize only fields documented as sets. Argument bindings, examples,
/// parameter order and all process sequences deliberately retain their source
/// order because they alter the execution contract.
fn manifest_value_hash(manifest: &ToolPackageManifest) -> Sha256Hash {
    canonical_sha256(
        &serde_json::to_value(normalized_manifest(manifest))
            .expect("normalized manifest serializes"),
    )
    .expect("normalized manifest is canonical JSON")
}

fn normalized_manifest(manifest: &ToolPackageManifest) -> ToolPackageManifest {
    let mut manifest = manifest.clone();
    let protocols: BTreeMap<_, _> = manifest
        .executables
        .iter()
        .map(|executable| (executable.executable_id.clone(), executable.protocol))
        .collect();
    manifest.backend_kinds.sort_by_key(|kind| match kind {
        star_contracts::manifest::BackendKind::Process => "process",
        star_contracts::manifest::BackendKind::ControllerCommand => "controller_command",
    });
    for executable in &mut manifest.executables {
        executable
            .product_version_req
            .get_or_insert_with(|| "*".to_owned());
        executable.architectures.sort();
        executable.isolation_compatibility.sort();
        executable.environment_allow.sort();
        if let Some(path) = &mut executable.path {
            *path = normalize_manifest_path(path);
        }
        if let Some(path) = &mut executable.fixed_working_directory {
            *path = normalize_manifest_path(path);
        }
        for integrity in &mut executable.integrity_files {
            integrity.path = normalize_manifest_path(&integrity.path);
        }
    }
    for action in &mut manifest.actions {
        if action.backend_kind == star_contracts::manifest::BackendKind::Process {
            let default_cancel = match protocols.get(&action.backend_ref) {
                Some(star_contracts::manifest::ManifestProtocol::StarJsonStdioV1) => "stdin_frame",
                _ => "terminate_job",
            };
            action
                .cancel_mode
                .get_or_insert_with(|| default_cancel.to_owned());
            action
                .cancel
                .get_or_insert(star_contracts::manifest::CancelContract { grace_ms: 2_000 });
            action
                .concurrency
                .get_or_insert(star_contracts::manifest::ConcurrencyContract {
                    max_parallel: 1,
                    exclusive_scope: "none".to_owned(),
                    lock_key_inputs: Vec::new(),
                    queue_timeout_ms: 30_000,
                });
        }
        if let Some(output) = &mut action.output
            && output.format == "jsonl"
        {
            output.max_items.get_or_insert(5_000);
        }
        for parameter in &mut action.parameters {
            if matches!(
                parameter.parameter_type.as_str(),
                "project_path" | "project_path_array"
            ) {
                parameter.must_exist.get_or_insert(true);
            }
        }
        *action = normalized_action(action);
        if let Some(path) = &mut action.input_schema_file {
            *path = normalize_manifest_path(path);
        }
        if let Some(path) = &mut action.output_schema_file {
            *path = normalize_manifest_path(path);
        }
    }
    manifest
}

fn apply_registry_policy(
    manifest: &mut ToolPackageManifest,
    source: ManifestSource,
    policy: &UserToolRegistryConfig,
) -> bool {
    if source == ManifestSource::Release {
        return true;
    }
    for executable in &mut manifest.executables {
        let protocol = match executable.protocol {
            star_contracts::manifest::ManifestProtocol::ArgvV1 => "argv_v1",
            star_contracts::manifest::ManifestProtocol::StarJsonStdioV1 => "star_json_stdio_v1",
        };
        if !policy
            .allowed_process_protocols
            .iter()
            .any(|allowed| allowed == protocol)
            || (source == ManifestSource::User
                && executable.update_policy == UpdatePolicy::FollowPath
                && !policy.allow_follow_path_user)
        {
            return false;
        }
        executable.isolation_compatibility.retain(|profile| {
            policy
                .allowed_isolation_profiles
                .iter()
                .any(|allowed| allowed == profile)
        });
        if executable.isolation_compatibility.is_empty() {
            return false;
        }
    }
    true
}

#[cfg(test)]
fn resolve_fixed_working_directory_hashes(
    manifest: &ToolPackageManifest,
) -> Option<BTreeMap<String, Sha256Hash>> {
    resolve_fixed_working_directory_hashes_cached(manifest, &mut BTreeMap::new())
}

fn resolve_fixed_working_directory_hashes_cached(
    manifest: &ToolPackageManifest,
    cache: &mut BTreeMap<PathBuf, Sha256Hash>,
) -> Option<BTreeMap<String, Sha256Hash>> {
    let mut hashes = BTreeMap::new();
    for executable in &manifest.executables {
        if executable.working_directory != "fixed" {
            continue;
        }
        let path = PathBuf::from(executable.fixed_working_directory.as_deref()?);
        let hash = if let Some(hash) = cache.get(&path) {
            hash.clone()
        } else {
            if !safe_user_config_path(&path) {
                return None;
            }
            let final_path = path.canonicalize().ok()?;
            if !final_path.is_dir() || !safe_user_config_path(&final_path) {
                return None;
            }
            let hash = Sha256Hash::digest(
                final_path
                    .as_os_str()
                    .to_string_lossy()
                    .replace('\\', "/")
                    .to_lowercase()
                    .as_bytes(),
            );
            cache.insert(path, hash.clone());
            hash
        };
        hashes.insert(executable.executable_id.clone(), hash);
    }
    Some(hashes)
}

fn manifest_uses_external_schemas(manifest: &ToolPackageManifest) -> bool {
    manifest
        .actions
        .iter()
        .any(|action| action.input_schema_file.is_some() || action.output_schema_file.is_some())
}

fn normalize_manifest_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn normalized_action(action: &ActionDescriptor) -> ActionDescriptor {
    let mut action = action.clone();
    action.aliases.sort();
    action.tags.sort();
    action.task_kinds.sort();
    action.permission_actions.sort();
    action
}

fn resolve_executables(
    manifest: &ToolPackageManifest,
    manifest_path: &Path,
    locations: &BTreeMap<String, PathBuf>,
    path_cache: &mut BTreeMap<PathBuf, PathBuf>,
    hash_cache: &mut BTreeMap<PathBuf, Sha256Hash>,
) -> Option<(BTreeMap<String, PathBuf>, BTreeMap<String, Sha256Hash>)> {
    let mut paths = BTreeMap::new();
    let mut identities = BTreeMap::new();
    for executable in &manifest.executables {
        if current_windows_build().is_none_or(|build| build < executable.minimum_windows_build) {
            return None;
        }
        let cache_key = match executable.locator_kind {
            LocatorKind::Absolute => Some(PathBuf::from(executable.path.as_deref()?)),
            LocatorKind::LocationRef => {
                Some(locations.get(executable.location_ref.as_deref()?)?.clone())
            }
            LocatorKind::AnchorRelative => None,
        };
        let path = if let Some(cache_key) = cache_key {
            if let Some(path) = path_cache.get(&cache_key) {
                path.clone()
            } else {
                let path = resolve_executable_path(executable, manifest_path, locations)?;
                path_cache.insert(cache_key, path.clone());
                path
            }
        } else {
            resolve_executable_path(executable, manifest_path, locations)?
        };
        let hash = if let Some(hash) = hash_cache.get(&path) {
            hash.clone()
        } else {
            let file = open_manifest_candidate(&path).ok()?;
            let before = stable_file_stamp(&file, u64::MAX)?;
            let hash = Sha256Hash::digest_reader(file.try_clone().ok()?).ok()?;
            if stable_file_stamp(&file, u64::MAX).as_ref() != Some(&before) {
                return None;
            }
            hash_cache.insert(path.clone(), hash.clone());
            hash
        };
        if executable.update_policy == UpdatePolicy::PinnedHash
            && executable.sha256.as_ref() != Some(&hash)
        {
            return None;
        }
        paths.insert(executable.executable_id.clone(), path);
        identities.insert(executable.executable_id.clone(), hash);
    }
    Some((paths, identities))
}

#[cfg(windows)]
fn current_windows_build() -> Option<u32> {
    use std::sync::OnceLock;
    use windows::{
        Wdk::System::SystemServices::RtlGetVersion,
        Win32::System::SystemInformation::OSVERSIONINFOW,
    };
    static BUILD: OnceLock<Option<u32>> = OnceLock::new();
    *BUILD.get_or_init(|| {
        let mut version = OSVERSIONINFOW {
            dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
            ..Default::default()
        };
        let status = unsafe { RtlGetVersion(&mut version) };
        (status.0 >= 0).then_some(version.dwBuildNumber)
    })
}

#[cfg(not(windows))]
fn current_windows_build() -> Option<u32> {
    None
}

fn resolve_executable_path(
    executable: &star_contracts::manifest::ExecutableDescriptor,
    manifest_path: &Path,
    locations: &BTreeMap<String, PathBuf>,
) -> Option<PathBuf> {
    let unresolved = match executable.locator_kind {
        LocatorKind::Absolute => PathBuf::from(executable.path.as_deref()?),
        LocatorKind::AnchorRelative => {
            let base = match executable.anchor.as_deref()? {
                "program_files" => PathBuf::from(std::env::var_os("ProgramFiles")?),
                "local_app_data" => PathBuf::from(std::env::var_os("LOCALAPPDATA")?),
                "user_tools" => PathBuf::from(std::env::var_os("APPDATA")?)
                    .join("Star-Control")
                    .join("tools"),
                "package_dir" => manifest_path.parent()?.to_path_buf(),
                _ => return None,
            };
            let base = base.canonicalize().ok()?;
            let candidate = base.join(executable.path.as_deref()?);
            let final_path = candidate.canonicalize().ok()?;
            if !final_path.starts_with(&base) {
                return None;
            }
            final_path
        }
        LocatorKind::LocationRef => locations
            .get(executable.location_ref.as_deref()?)
            .cloned()?,
    };
    if !safe_unresolved_executable(&unresolved) {
        return None;
    }
    let path = unresolved.canonicalize().ok()?;
    safe_final_executable(&path).then_some(path)
}

fn safe_unresolved_executable(path: &Path) -> bool {
    if !path.is_absolute() || !path.is_file() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if fs::symlink_metadata(path)
            .ok()
            .is_none_or(|metadata| metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
        {
            return false;
        }
    }
    true
}

fn safe_final_executable(path: &Path) -> bool {
    if !path.is_absolute()
        || !path.is_file()
        || !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        || path
            .file_name()
            .and_then(|name| name.to_str())
            .is_none_or(is_forbidden_executable_name)
    {
        return false;
    }
    #[cfg(windows)]
    {
        use std::{os::windows::fs::MetadataExt, path::Prefix};
        use windows::{
            Win32::{Storage::FileSystem::GetDriveTypeW, System::WindowsProgramming::DRIVE_FIXED},
            core::HSTRING,
        };
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        let drive = match path.components().next() {
            Some(std::path::Component::Prefix(prefix)) => match prefix.kind() {
                Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => Some(letter),
                _ => None,
            },
            _ => None,
        };
        let local_disk = drive.is_some_and(|letter| {
            let root = HSTRING::from(format!("{}:\\", char::from(letter)));
            unsafe { GetDriveTypeW(&root) == DRIVE_FIXED }
        });
        if !local_disk
            || fs::symlink_metadata(path).ok().is_none_or(|metadata| {
                metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            })
        {
            return false;
        }
    }
    true
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StableFileStamp {
    size: u64,
    modified: std::time::SystemTime,
    #[cfg(windows)]
    volume_serial: u64,
    #[cfg(windows)]
    file_id: [u8; 16],
}

fn stable_file_stamp(file: &fs::File, max_manifest_bytes: u64) -> Option<StableFileStamp> {
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > max_manifest_bytes {
        return None;
    }
    #[cfg(windows)]
    let (volume_serial, file_id) = {
        use std::os::windows::{fs::MetadataExt, io::AsRawHandle};
        use windows::Win32::{
            Foundation::HANDLE,
            Storage::FileSystem::{FILE_ID_INFO, FileIdInfo, GetFileInformationByHandleEx},
        };
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return None;
        }
        let mut information = FILE_ID_INFO::default();
        unsafe {
            GetFileInformationByHandleEx(
                HANDLE(file.as_raw_handle().cast()),
                FileIdInfo,
                (&raw mut information).cast(),
                std::mem::size_of::<FILE_ID_INFO>() as u32,
            )
        }
        .ok()?;
        (
            information.VolumeSerialNumber,
            information.FileId.Identifier,
        )
    };
    Some(StableFileStamp {
        size: metadata.len(),
        modified: metadata.modified().ok()?,
        #[cfg(windows)]
        volume_serial,
        #[cfg(windows)]
        file_id,
    })
}

enum StableCandidateText {
    Stable(String, SourceFileIdentity),
    Unchanged(String, SourceFileIdentity),
    Stabilizing,
    Invalid,
}

type RegistryScanBatch<'a> = (
    &'a RegistrySourceRoot,
    bool,
    Vec<(PathBuf, StableCandidateText)>,
);

fn source_file_identity(stamp: &StableFileStamp) -> SourceFileIdentity {
    #[cfg(windows)]
    let (volume_serial, file_id) = (
        format!("{:016x}", stamp.volume_serial),
        stamp
            .file_id
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    );
    #[cfg(not(windows))]
    let (volume_serial, file_id) = ("unsupported".to_owned(), "unsupported".to_owned());
    SourceFileIdentity {
        volume_serial,
        file_id,
        size: stamp.size,
        last_write: chrono::DateTime::<Utc>::from(stamp.modified)
            .to_rfc3339_opts(SecondsFormat::Millis, true),
    }
}

#[cfg(windows)]
fn open_manifest_candidate(path: &Path) -> io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Storage::FileSystem::{FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ};

    fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ.0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)
}

#[cfg(not(windows))]
fn open_manifest_candidate(path: &Path) -> io::Result<fs::File> {
    fs::File::open(path)
}

fn read_stable_manifest(
    path: &Path,
    expected: &StableFileStamp,
    max_manifest_bytes: u64,
) -> Result<Option<String>, ()> {
    let Ok(file) = open_manifest_candidate(path) else {
        return Ok(None);
    };
    read_stable_manifest_file(&file, expected, max_manifest_bytes)
}

fn read_stable_manifest_file(
    file: &fs::File,
    expected: &StableFileStamp,
    max_manifest_bytes: u64,
) -> Result<Option<String>, ()> {
    if stable_file_stamp(file, max_manifest_bytes).as_ref() != Some(expected) {
        return Ok(None);
    }
    let mut bytes = Vec::with_capacity(expected.size as usize);
    if file
        .take(max_manifest_bytes + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return Ok(None);
    }
    if bytes.len() as u64 != expected.size || bytes.len() as u64 > max_manifest_bytes {
        return Ok(None);
    }
    if stable_file_stamp(file, max_manifest_bytes).as_ref() != Some(expected) {
        return Ok(None);
    }
    String::from_utf8(bytes).map(Some).map_err(|_| ())
}

/// Observe file identity, size and last-write twice, retrying a changing save
/// for at most five seconds. Every accepted candidate is then read through one
/// handle whose identity is rechecked before the bounded read.
fn stable_candidate_texts(
    paths: Vec<PathBuf>,
    known_files: &BTreeMap<PathBuf, (SourceFileIdentity, Sha256Hash)>,
    max_manifest_bytes: u64,
    stable_file_window: Duration,
    stable_file_timeout: Duration,
) -> Vec<(PathBuf, StableCandidateText)> {
    let mut candidates: Vec<_> = paths
        .into_iter()
        .map(|path| {
            let file = open_manifest_candidate(&path).ok();
            let stamp = file
                .as_ref()
                .and_then(|file| stable_file_stamp(file, max_manifest_bytes));
            let unchanged = stamp.as_ref().and_then(|stamp| {
                let (known_identity, known_hash) = known_files.get(&path)?;
                if &source_file_identity(stamp) != known_identity {
                    return None;
                }
                let text = read_stable_manifest_file(file.as_ref()?, stamp, max_manifest_bytes)
                    .ok()
                    .flatten()?;
                (Sha256Hash::digest(text.as_bytes()) == *known_hash)
                    .then(|| StableCandidateText::Unchanged(text, known_identity.clone()))
            });
            (path, stamp, unchanged)
        })
        .collect();
    let deadline = Instant::now() + stable_file_timeout;
    while candidates.iter().any(|(_, _, result)| result.is_none()) && Instant::now() < deadline {
        std::thread::sleep(stable_file_window);
        for (path, previous, result) in &mut candidates {
            if result.is_some() {
                continue;
            }
            let current = open_manifest_candidate(path)
                .ok()
                .as_ref()
                .and_then(|file| stable_file_stamp(file, max_manifest_bytes));
            if let (Some(previous), Some(current)) = (previous.as_ref(), current.as_ref())
                && previous == current
            {
                match read_stable_manifest(path, current, max_manifest_bytes) {
                    Ok(Some(text)) => {
                        *result = Some(StableCandidateText::Stable(
                            text,
                            source_file_identity(current),
                        ));
                        continue;
                    }
                    Err(()) => {
                        *result = Some(StableCandidateText::Invalid);
                        continue;
                    }
                    Ok(None) => {}
                }
            }
            *previous = current;
        }
    }
    candidates
        .into_iter()
        .map(|(path, _, result)| (path, result.unwrap_or(StableCandidateText::Stabilizing)))
        .collect()
}

fn cached_scan_is_current(
    scan_batches: &[RegistryScanBatch<'_>],
    known_packages: &BTreeMap<PathBuf, &ActivePackage>,
    locations: &BTreeMap<String, PathBuf>,
    policy: &UserToolRegistryConfig,
) -> bool {
    if scan_batches
        .iter()
        .map(|(_, _, candidates)| candidates.len())
        .sum::<usize>()
        != known_packages.len()
        || scan_batches.iter().any(|(_, overflowed, _)| *overflowed)
    {
        return false;
    }

    let mut seen = BTreeSet::new();
    let mut executable_path_cache = BTreeMap::new();
    let mut executable_hash_cache = BTreeMap::new();
    let mut working_directory_hash_cache = BTreeMap::new();
    for (root, _, candidates) in scan_batches {
        for (path, candidate) in candidates {
            let StableCandidateText::Unchanged(text, identity) = candidate else {
                return false;
            };
            if root.source == ManifestSource::Release
                && !release_manifest_integrity_matches(path, text)
            {
                return false;
            }
            let Some(package) = known_packages.get(path).copied() else {
                return false;
            };
            if !seen.insert(path.clone())
                || package.source != root.source
                || package.source_file_identity != *identity
            {
                return false;
            }
            let Some(working_directory_hashes) = resolve_fixed_working_directory_hashes_cached(
                &package.manifest,
                &mut working_directory_hash_cache,
            ) else {
                return false;
            };
            if working_directory_hashes != package.fixed_working_directory_hashes {
                return false;
            }
            if manifest_uses_external_schemas(&package.manifest) {
                let Ok(resources) = load_manifest_resources_with_limits(
                    &package.manifest,
                    path,
                    SchemaLimits {
                        package_bytes: policy.max_schema_bytes,
                        action_resolved_bytes: policy.max_action_schema_bytes,
                        max_ref_depth: policy.max_schema_depth,
                    },
                ) else {
                    return false;
                };
                if resources != package.resources {
                    return false;
                }
            }
            let Some((resolved_paths, resolved_hashes)) = resolve_executables(
                &package.manifest,
                path,
                locations,
                &mut executable_path_cache,
                &mut executable_hash_cache,
            ) else {
                return false;
            };
            if resolved_paths != package.resolved_executable_paths
                || resolved_hashes != package.resolved_executable_hashes
            {
                return false;
            }
        }
    }
    seen.len() == known_packages.len()
}

fn source_name(source: ManifestSource) -> &'static str {
    match source {
        ManifestSource::Release => "release",
        ManifestSource::User => "user",
        ManifestSource::Project => "project",
    }
}

fn registry_source(source: ManifestSource) -> RegistrySource {
    match source {
        ManifestSource::Release => RegistrySource::Release,
        ManifestSource::User => RegistrySource::User,
        ManifestSource::Project => RegistrySource::Project,
    }
}

fn source_id_hash(package: &ActivePackage) -> Sha256Hash {
    Sha256Hash::digest(
        package
            .path
            .parent()
            .unwrap_or(&package.path)
            .as_os_str()
            .to_string_lossy()
            .replace('\\', "/")
            .to_lowercase()
            .as_bytes(),
    )
}

fn package_snapshot(package: &ActivePackage, trust_id: Option<ToolTrustId>) -> PackageSnapshot {
    PackageSnapshot {
        package_id: package.manifest.package_id.clone(),
        package_version: package.manifest.package_version.clone(),
        source_kind: registry_source(package.source),
        manifest_hash: RegistryRuntime::manifest_hash(package),
        schema_hashes: package.resources.schema_hashes.clone(),
        executable_identities: package.resolved_executable_hashes.clone(),
        tool_descriptor_hashes: package
            .manifest
            .actions
            .iter()
            .map(|action| {
                (
                    action.tool_id.clone(),
                    RegistryRuntime::descriptor_hash(package, action),
                )
            })
            .collect(),
        trust_id,
        package_hash: RegistryRuntime::package_semantic_hash(package),
    }
}

fn cache_contract(package: &ActivePackage, trust_id: Option<ToolTrustId>) -> ToolRegistryCache {
    ToolRegistryCache {
        schema_id: "star.tool-registry-cache".to_owned(),
        schema_version: 1,
        cache_id: package.cache_id.clone(),
        package_id: package.manifest.package_id.clone(),
        package_version: package.manifest.package_version.clone(),
        source_kind: registry_source(package.source),
        source_id_hash: source_id_hash(package),
        source_file_identity: package.source_file_identity.clone(),
        source_content_hash: package.source_hash.clone(),
        manifest_hash: RegistryRuntime::manifest_hash(package),
        package_snapshot: package_snapshot(package, trust_id.clone()),
        trust_id,
        mcp_contract_version: 1,
        product_version: env!("CARGO_PKG_VERSION").to_owned(),
        validated_at: package.validated_at.clone(),
    }
}

fn cache_integrity_path(path: &Path) -> PathBuf {
    path.with_extension("integrity.json")
}

#[cfg(windows)]
#[link(name = "Normaliz")]
unsafe extern "system" {
    fn NormalizeString(
        norm_form: i32,
        source: *const u16,
        source_length: i32,
        destination: *mut u16,
        destination_length: i32,
    ) -> i32;
}

/// Windows NormalizationKC plus Unicode lowercase.  Search metadata and the
/// query take the same path, while descriptor/arguments hashing deliberately
/// preserves original Unicode bytes.
pub fn normalize_search_text(value: &str) -> String {
    #[cfg(windows)]
    {
        const NORMALIZATION_KC: i32 = 5;
        let source: Vec<u16> = value.encode_utf16().collect();
        let source_length = i32::try_from(source.len()).unwrap_or(i32::MAX);
        let required = unsafe {
            NormalizeString(
                NORMALIZATION_KC,
                source.as_ptr(),
                source_length,
                std::ptr::null_mut(),
                0,
            )
        };
        if required > 0 {
            let mut destination = vec![0u16; required as usize];
            let written = unsafe {
                NormalizeString(
                    NORMALIZATION_KC,
                    source.as_ptr(),
                    source_length,
                    destination.as_mut_ptr(),
                    required,
                )
            };
            if written > 0 {
                destination.truncate(written as usize);
                if let Ok(normalized) = String::from_utf16(&destination) {
                    return normalized.to_lowercase();
                }
            }
        }
    }
    value.to_lowercase()
}

fn search_tokens(value: &str) -> BTreeSet<String> {
    let normalized = normalize_search_text(value);
    let mut tokens = BTreeSet::new();
    let mut current = String::new();
    for character in normalized.chars() {
        if character.is_alphanumeric() {
            current.push(character);
        } else if !current.is_empty() {
            tokens.insert(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.insert(current);
    }
    tokens
}

#[cfg(windows)]
pub(crate) fn safe_registry_root(root: &Path) -> bool {
    use std::{os::windows::fs::MetadataExt, path::Prefix};
    use windows::{
        Win32::{Storage::FileSystem::GetDriveTypeW, System::WindowsProgramming::DRIVE_FIXED},
        core::HSTRING,
    };
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

    let metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };
    if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return false;
    }
    let drive = match root.components().next() {
        Some(std::path::Component::Prefix(prefix)) => match prefix.kind() {
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

#[cfg(not(windows))]
pub(crate) fn safe_registry_root(root: &Path) -> bool {
    root.is_dir()
}

fn manifest_files(
    root: &Path,
    preferred: &BTreeSet<PathBuf>,
    candidate_limit: usize,
) -> (Vec<PathBuf>, bool) {
    let mut preferred_files = BTreeSet::new();
    let mut candidates = BTreeSet::new();
    let mut eligible_candidates = 0usize;
    if !safe_registry_root(root) {
        return (Vec::new(), false);
    }
    let Ok(entries) = fs::read_dir(root) else {
        return (Vec::new(), false);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().ok().is_some_and(|kind| kind.is_file())
            && path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
        {
            if preferred.contains(&path) {
                preferred_files.insert(path);
            } else {
                eligible_candidates = eligible_candidates.saturating_add(1);
                candidates.insert(path);
                if candidates.len() > candidate_limit
                    && let Some(last) = candidates.iter().next_back().cloned()
                {
                    candidates.remove(&last);
                }
            }
        }
    }
    preferred_files.extend(candidates);
    (
        preferred_files.into_iter().collect(),
        eligible_candidates > candidate_limit,
    )
}

#[cfg(test)]
#[allow(clippy::cloned_ref_to_slice_refs)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    fn fixture() -> String {
        static FIXTURE: OnceLock<String> = OnceLock::new();
        FIXTURE
            .get_or_init(|| {
                let executable = std::env::current_exe().unwrap();
                let path = executable.display().to_string().replace('\\', "\\\\");
                let hash = Sha256Hash::digest_reader(fs::File::open(executable).unwrap()).unwrap();
                include_str!("../../../specs/examples/valid/tool-package-manifest-v1.toml")
                    .replace(r"C:\\Tools\\fake-echo.exe", &path)
                    .replace(
                        "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                        hash.as_str(),
                    )
            })
            .clone()
    }

    fn replace_fixture_executable(
        source: String,
        escaped_path: &str,
        sha_replacement: Option<&str>,
    ) -> String {
        let mut output = String::new();
        for line in source.lines() {
            if line.starts_with("path = ") {
                output.push_str(&format!("path = \"{escaped_path}\"\n"));
            } else if line.starts_with("sha256 = ") {
                if let Some(replacement) = sha_replacement {
                    output.push_str(replacement);
                    output.push('\n');
                }
            } else {
                output.push_str(line);
                output.push('\n');
            }
        }
        output
    }

    fn temp_root(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "star-control-registry-{name}-{}",
            star_ipc::nonce()
        ));
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn manifest_enumeration_is_bounded_and_preserves_known_candidates() {
        let directory = temp_root("manifest-enumeration-bound");
        for index in 0..20 {
            fs::write(directory.join(format!("candidate-{index:02}.toml")), "").unwrap();
        }
        let preferred = directory.join("preferred.toml");
        fs::write(&preferred, "").unwrap();
        let preferred_set = BTreeSet::from([preferred.clone()]);

        let (files, overflowed) = manifest_files(&directory, &preferred_set, 8);

        assert!(overflowed);
        assert_eq!(files.len(), 9);
        assert!(files.contains(&preferred));
        for index in 0..8 {
            assert!(files.contains(&directory.join(format!("candidate-{index:02}.toml"))));
        }
        assert!(!files.contains(&directory.join("candidate-08.toml")));
    }

    #[test]
    fn demand_scan_applies_the_manifest_candidate_budget_globally() {
        let user_directory = temp_root("global-candidate-budget-user");
        let project_directory = temp_root("global-candidate-budget-project");
        for index in 0..10 {
            fs::write(user_directory.join(format!("user-{index:02}.toml")), "").unwrap();
            fs::write(
                project_directory.join(format!("project-{index:02}.toml")),
                "",
            )
            .unwrap();
        }
        let policy = UserToolRegistryConfig {
            max_packages: 8,
            ..Default::default()
        };
        let mut registry = RegistryRuntime::default();
        registry.set_policy(policy);
        registry.demand_scan(&[
            RegistrySourceRoot {
                source: ManifestSource::User,
                directory: user_directory.clone(),
            },
            RegistrySourceRoot {
                source: ManifestSource::Project,
                directory: project_directory.clone(),
            },
        ]);

        let file_diagnostic_count = registry
            .diagnostics
            .keys()
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "toml")
            })
            .count();
        assert_eq!(file_diagnostic_count, 8);
        assert_eq!(registry.diagnostics[&user_directory], "TOOL_REGISTRY_LIMIT");
        assert_eq!(
            registry.diagnostics[&project_directory],
            "TOOL_REGISTRY_LIMIT"
        );
        assert!(
            !registry
                .diagnostics
                .contains_key(&project_directory.join("project-00.toml"))
        );
    }

    #[test]
    fn unavailable_descriptors_share_the_global_searchable_action_budget() {
        let directory = temp_root("unavailable-action-budget");
        let zero_hash = format!("sha256 = \"sha256:{}\"", "0".repeat(64));
        for index in 0..3 {
            let manifest = replace_fixture_executable(
                package_with_actions(index, 64),
                r"C:\\definitely-missing\\tool.exe",
                Some(&zero_hash),
            );
            fs::write(directory.join(format!("package-{index}.toml")), manifest).unwrap();
        }
        let policy = UserToolRegistryConfig {
            max_tools: 100,
            ..Default::default()
        };
        let mut registry = RegistryRuntime::default();
        registry.set_policy(policy);
        registry.demand_scan(&[RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        }]);

        let hits = registry.search_describable_actions_with_trust("", &BTreeSet::new());
        assert_eq!(hits.len(), 64);
        assert!(
            registry
                .diagnostics
                .values()
                .any(|diagnostic| diagnostic == "TOOL_EXECUTABLE_UNAVAILABLE")
        );
        assert!(
            registry
                .diagnostics
                .values()
                .any(|diagnostic| diagnostic == "TOOL_REGISTRY_LIMIT")
        );
    }

    #[test]
    // matrix: MCP-R006 MCP-R009 MCP-R013
    fn invalid_replacement_keeps_last_known_good_but_delete_removes_it() {
        let directory = temp_root("lkg");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let original = registry.active()["user.fake.echo"].source_hash.clone();
        fs::write(&path, "not = [valid").unwrap();
        registry.demand_scan(&[root.clone()]);
        assert_eq!(registry.active()["user.fake.echo"].source_hash, original);
        assert_eq!(registry.diagnostics[&path], "TOOL_MANIFEST_INVALID");
        fs::remove_file(&path).unwrap();
        registry.demand_scan(&[root.clone()]);
        assert!(registry.active().contains_key("user.fake.echo"));
        assert_eq!(registry.diagnostics[&path], "TOOL_MANIFEST_RENAMING");
        std::thread::sleep(MISSING_SOURCE_DEBOUNCE + Duration::from_millis(100));
        registry.demand_scan(&[root]);
        assert!(registry.active().is_empty());
    }

    #[test]
    fn removing_a_configured_source_revokes_its_lkg_without_rename_debounce() {
        let directory = temp_root("source-disabled");
        fs::write(directory.join("fake.toml"), fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(std::slice::from_ref(&root));
        assert!(registry.active().contains_key("user.fake.echo"));
        registry.demand_scan(&[]);
        assert!(registry.active().is_empty());
    }

    #[test]
    // matrix: MCP-R004
    fn editor_temp_rename_never_transiently_removes_the_active_tool() {
        let directory = temp_root("editor-rename");
        let path = directory.join("fake.toml");
        let temporary = directory.join(".fake-editor.tmp");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        fs::write(
            &temporary,
            fixture().replace("Echoes a value.", "Echoes an updated value."),
        )
        .unwrap();
        fs::rename(&path, root.directory.join(".fake-old.tmp")).unwrap();
        registry.demand_scan(&[root.clone()]);
        assert!(registry.active().contains_key("user.fake.echo"));
        assert_eq!(registry.diagnostics[&path], "TOOL_MANIFEST_RENAMING");
        fs::rename(&temporary, &path).unwrap();
        registry.demand_scan(&[root]);
        let (_, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_eq!(action.summary, "Echoes an updated value.");
    }

    #[test]
    // matrix: MCP-R011
    fn demand_scan_finds_a_change_even_when_no_watcher_event_is_delivered() {
        let directory = temp_root("watcher-missed-event");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let before = registry.snapshot_hash();
        // Do not create or poll a watcher here: this is the lost-event path.
        fs::write(
            &path,
            fixture().replace("Echoes a value.", "Echoes after a missed watcher event."),
        )
        .unwrap();
        registry.demand_scan(&[root]);
        assert_ne!(before, registry.snapshot_hash());
    }

    #[test]
    // matrix: MCP-R007
    fn invalid_new_candidate_does_not_hide_another_valid_package() {
        let directory = temp_root("invalid-new");
        let valid = directory.join("valid.toml");
        let invalid = directory.join("invalid.toml");
        fs::write(&valid, fixture()).unwrap();
        fs::write(&invalid, "format_version = [").unwrap();
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        }]);
        assert!(registry.active().contains_key("user.fake.echo"));
        assert_eq!(registry.diagnostics[&invalid], "TOOL_MANIFEST_INVALID");
    }

    #[test]
    // matrix: MCP-R012
    fn restart_with_invalid_source_restores_durable_last_known_good() {
        let directory = temp_root("durable-lkg");
        let path = directory.join("fake.toml");
        let cache = directory.join("registry-cache.v1.json");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let original = registry.active()["user.fake.echo"].source_hash.clone();
        registry.persist_cache(&cache).unwrap();
        fs::write(&path, "format_version = [").unwrap();
        let mut restarted = RegistryRuntime::load_cache(&cache).unwrap();
        restarted.demand_scan(&[root]);
        assert_eq!(restarted.active()["user.fake.echo"].source_hash, original);
        assert_eq!(restarted.diagnostics[&path], "TOOL_MANIFEST_INVALID");
    }

    #[test]
    // matrix: MCP-S015
    fn durable_cache_publishes_exact_redacted_contract_entries_with_private_dacl() {
        let directory = temp_root("cache-contract");
        let path = directory.join("fake.toml");
        let cache = directory.join("registry-cache.v1.json");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory: directory.clone(),
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let first_cache_id = registry.active()["user.fake.echo"].cache_id.clone();
        let trust_id = ToolTrustId::new();
        registry
            .persist_cache_with_trust_ids(
                &cache,
                &BTreeMap::from([("user.fake.echo".to_owned(), trust_id.clone())]),
            )
            .unwrap();

        let outer_text = fs::read_to_string(&cache).unwrap();
        assert!(!outer_text.contains(&directory.to_string_lossy().to_string()));
        let outer: RegistryCacheEnvelope = serde_json::from_str(&outer_text).unwrap();
        let entry = &outer.entries["user.fake.echo"];
        assert_eq!(entry.schema_id, "star.tool-registry-cache");
        assert_eq!(entry.schema_version, 1);
        assert_eq!(entry.mcp_contract_version, 1);
        assert_eq!(entry.cache_id, first_cache_id);
        assert_eq!(entry.trust_id.as_ref(), Some(&trust_id));
        assert_eq!(entry.package_snapshot.trust_id.as_ref(), Some(&trust_id));
        assert_eq!(entry.source_file_identity.file_id.len(), 32);
        assert_eq!(entry.source_file_identity.volume_serial.len(), 16);

        for protected in [&cache, &cache_integrity_path(&cache)] {
            let dacl = star_ipc::key_store::file_dacl_sddl(protected).unwrap();
            assert!(dacl.starts_with("D:P"));
            assert_eq!(dacl.matches("(A;").count(), 2);
            assert!(!dacl.contains(";;;WD)"));
            assert!(!dacl.contains(";;;BU)"));
            assert!(!dacl.contains(";;;AU)"));
        }

        registry.demand_scan(&[root]);
        assert_eq!(
            registry.active()["user.fake.echo"].cache_id,
            first_cache_id,
            "an unchanged immutable cache entry must retain its cache ID"
        );
        RegistryRuntime::load_cache(&cache).unwrap();
    }

    #[test]
    // matrix: MCP-R013
    fn restart_with_deleted_source_never_revives_the_cache() {
        let directory = temp_root("durable-delete");
        let path = directory.join("fake.toml");
        let cache = directory.join("registry-cache.v1.json");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        registry.persist_cache(&cache).unwrap();
        fs::remove_file(&path).unwrap();
        let mut restarted = RegistryRuntime::load_cache(&cache).unwrap();
        restarted.demand_scan(&[root]);
        assert!(restarted.active().is_empty());
    }

    #[test]
    // matrix: MCP-R015
    fn duplicate_package_id_is_conflict_not_source_override() {
        let release = temp_root("duplicate-first-source");
        let user = temp_root("user");
        let release_path = release.join("fake.toml");
        let user_path = user.join("fake.toml");
        fs::write(&release_path, fixture()).unwrap();
        fs::write(
            &user_path,
            fixture().replace("package_version = \"1.0.0\"", "package_version = \"1.1.0\""),
        )
        .unwrap();
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[
            RegistrySourceRoot {
                source: ManifestSource::User,
                directory: release,
            },
            RegistrySourceRoot {
                source: ManifestSource::Project,
                directory: user,
            },
        ]);
        assert!(registry.active().is_empty());
        assert_eq!(registry.diagnostics[&release_path], "TOOL_PACKAGE_CONFLICT");
        assert_eq!(registry.diagnostics[&user_path], "TOOL_PACKAGE_CONFLICT");
    }

    #[test]
    // matrix: MCP-R001
    fn project_tools_directory_is_a_distinct_live_source() {
        let directory = temp_root("project-source");
        fs::write(directory.join("fake.toml"), fixture()).unwrap();
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[RegistrySourceRoot {
            source: ManifestSource::Project,
            directory,
        }]);
        let package = registry
            .active()
            .get("user.fake.echo")
            .expect("project manifest is live");
        assert_eq!(package.source, ManifestSource::Project);
    }

    #[test]
    // matrix: MCP-H007
    fn snapshot_hash_is_stable_for_identical_scan() {
        let directory = temp_root("hash");
        fs::write(directory.join("fake.toml"), fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let first = registry.snapshot_hash();
        registry.diagnostics.insert(
            PathBuf::from("diagnostic-only"),
            "changed wording".to_owned(),
        );
        registry.diagnostic_revision += 1;
        assert_eq!(first, registry.snapshot_hash());
        registry.demand_scan(&[root]);
        assert_eq!(first, registry.snapshot_hash());
    }

    #[test]
    // matrix: MCP-R002 MCP-H005
    fn descriptor_hash_changes_with_live_manifest_candidate() {
        let directory = temp_root("descriptor");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let first = RegistryRuntime::descriptor_hash(package, action);
        fs::write(
            &path,
            fixture().replace("Contract fixture action.", "Changed fixture action."),
        )
        .unwrap();
        registry.demand_scan(&[root]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_ne!(first, RegistryRuntime::descriptor_hash(package, action));
    }

    #[test]
    // matrix: MCP-R006 MCP-R011 MCP-H005
    fn schema_only_change_republishes_descriptor_and_invalid_schema_keeps_lkg() {
        let directory = temp_root("schema-reload");
        let manifest_path = directory.join("fake.toml");
        let schema_path = directory.join("input.json");
        let manifest = fixture()
            .replace(
                "[[actions.parameters]]\nname = \"value\"\ntype = \"string\"\ndescription = \"Value to echo\"\nrequired = true\n",
                "input_schema_file = \"input.json\"\n",
            )
            .replace(
                "[[actions.argv]]\nkind = \"positional\"\ninput = \"value\"\n",
                "[[actions.argv]]\nkind = \"literal\"\nvalue = \"fixed\"\n",
            );
        fs::write(&manifest_path, manifest).unwrap();
        fs::write(
            &schema_path,
            r#"{"type":"object","additionalProperties":false,"properties":{"value":{"type":"string"}}}"#,
        )
        .unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let first_descriptor = RegistryRuntime::descriptor_hash(package, action);
        let first_snapshot = registry.snapshot_hash();

        fs::write(
            &schema_path,
            r#"{"type":"object","additionalProperties":false,"properties":{"value":{"type":"string","maxLength":8}}}"#,
        )
        .unwrap();
        registry.demand_scan(&[root.clone()]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let second_descriptor = RegistryRuntime::descriptor_hash(package, action);
        assert_ne!(first_descriptor, second_descriptor);
        assert_ne!(first_snapshot, registry.snapshot_hash());

        fs::write(
            &schema_path,
            r#"{"type":"object","additionalProperties":false,"properties":{"value":{"type":"invalid"}}}"#,
        )
        .unwrap();
        registry.demand_scan(&[root]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_eq!(
            second_descriptor,
            RegistryRuntime::descriptor_hash(package, action)
        );
        assert_eq!(
            registry.diagnostics[&manifest_path],
            "TOOL_MANIFEST_SCHEMA_INVALID"
        );
    }

    #[test]
    // matrix: MCP-H015
    fn whitespace_changes_raw_candidate_hash_but_not_semantic_descriptor_or_snapshot() {
        let directory = temp_root("semantic-hash");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let descriptor = RegistryRuntime::descriptor_hash(package, action);
        let snapshot = registry.snapshot_hash();
        let cache_persistence = registry.cache_persistence_hash();
        let raw = package.source_hash.clone();
        fs::write(
            &path,
            fixture().replace(
                "display_name = \"Fake Echo\"",
                "display_name = \"Fake Echo\"   ",
            ),
        )
        .unwrap();
        registry.demand_scan(&[root]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_ne!(raw, package.source_hash);
        assert_eq!(
            descriptor,
            RegistryRuntime::descriptor_hash(package, action)
        );
        assert_eq!(snapshot, registry.snapshot_hash());
        assert_ne!(cache_persistence, registry.cache_persistence_hash());
    }

    #[test]
    // matrix: MCP-H002
    fn toml_key_order_and_crlf_do_not_change_semantic_hashes() {
        let directory = temp_root("toml-order");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let descriptor = RegistryRuntime::descriptor_hash(package, action);
        let snapshot = registry.snapshot_hash();
        let reordered = fixture()
            .replacen(
                "package_id = \"user.fake.echo\"\npackage_version = \"1.0.0\"",
                "package_version = \"1.0.0\"\npackage_id = \"user.fake.echo\"",
                1,
            )
            .replace('\n', "\r\n");
        fs::write(&path, reordered).unwrap();
        registry.demand_scan(&[root]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_eq!(
            descriptor,
            RegistryRuntime::descriptor_hash(package, action)
        );
        assert_eq!(snapshot, registry.snapshot_hash());
    }

    #[test]
    // matrix: MCP-H003
    fn set_order_changes_do_not_change_semantic_descriptor_or_snapshot() {
        let directory = temp_root("set-hash");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let descriptor = RegistryRuntime::descriptor_hash(package, action);
        let snapshot = registry.snapshot_hash();
        fs::write(
            &path,
            fixture().replace(
                "permission_actions = [\"local_read\", \"process_run\"]",
                "permission_actions = [\"process_run\", \"local_read\"]",
            ),
        )
        .unwrap();
        registry.demand_scan(&[root]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_eq!(
            descriptor,
            RegistryRuntime::descriptor_hash(package, action)
        );
        assert_eq!(snapshot, registry.snapshot_hash());
    }

    #[test]
    // matrix: MCP-H004
    fn argv_binding_order_changes_the_descriptor_hash() {
        let directory = temp_root("argv-order");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let descriptor = RegistryRuntime::descriptor_hash(package, action);
        fs::write(
            &path,
            format!(
                "{}\n[[actions.argv]]\nkind = \"literal\"\nvalue = \"--after\"\n",
                fixture()
            ),
        )
        .unwrap();
        registry.demand_scan(&[root]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_ne!(
            descriptor,
            RegistryRuntime::descriptor_hash(package, action)
        );
    }

    #[test]
    // matrix: MCP-P010
    fn pinned_hash_mismatch_keeps_last_known_good_candidate() {
        let directory = temp_root("executable-identity");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let descriptor = RegistryRuntime::descriptor_hash(package, action);
        let original = fixture();
        let pinned_hash_line = original
            .lines()
            .find(|line| line.starts_with("sha256 = "))
            .unwrap();
        fs::write(
            &path,
            original.replace(
                pinned_hash_line,
                "sha256 = \"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"",
            ),
        )
        .unwrap();
        registry.demand_scan(&[root]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_eq!(
            descriptor,
            RegistryRuntime::descriptor_hash(package, action)
        );
        assert_eq!(registry.diagnostics[&path], "TOOL_EXECUTABLE_UNAVAILABLE");
    }

    #[test]
    fn unavailable_package_without_lkg_is_still_describable_and_searchable_by_readiness() {
        let directory = temp_root("unavailable-discovery");
        let path = directory.join("fake.toml");
        let original = fixture();
        let pinned_hash_line = original
            .lines()
            .find(|line| line.starts_with("sha256 = "))
            .unwrap();
        fs::write(
            &path,
            original.replace(
                pinned_hash_line,
                "sha256 = \"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"",
            ),
        )
        .unwrap();
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        }]);
        let trusted: BTreeSet<_> = ["user.fake.echo".to_owned()].into_iter().collect();
        assert!(registry.search_actions_with_trust("", &trusted).is_empty());
        let hits = registry.search_describable_actions_with_trust("", &trusted);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].action.tool_id, "user.fake.echo.run");
        assert!(
            registry
                .find_effective_describable_action("user.fake.echo.run", &trusted)
                .is_some()
        );
        assert_eq!(
            registry
                .candidate_observation("user.fake.echo")
                .unwrap()
                .state,
            "unavailable"
        );
    }

    #[test]
    // matrix: MCP-H007
    fn diagnostic_text_and_revision_do_not_change_snapshot_hash() {
        let directory = temp_root("diagnostic-snapshot");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root]);
        let snapshot = registry.snapshot_hash();
        registry
            .diagnostics
            .insert(path, "changed diagnostic wording".to_owned());
        registry.diagnostic_revision += 1;
        assert_eq!(snapshot, registry.snapshot_hash());
    }

    #[test]
    // matrix: MCP-R005
    fn changing_candidate_stays_last_known_good_until_stable() {
        let directory = temp_root("stabilizing");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let hash = registry.active()["user.fake.echo"].source_hash.clone();
        let writer_path = path.clone();
        let started = std::sync::Arc::new(std::sync::Barrier::new(2));
        let writer_started = std::sync::Arc::clone(&started);
        let writer = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_millis(5_250);
            let mut sequence = 0_u64;
            fs::write(&writer_path, format!("not = [valid # {sequence}")).unwrap();
            writer_started.wait();
            while Instant::now() < deadline {
                sequence += 1;
                if let Err(error) = fs::write(&writer_path, format!("not = [valid # {sequence}")) {
                    // The production scanner intentionally holds a
                    // share-read-only handle. A sharing violation proves that
                    // the writer lost this race; it is not a flaky test
                    // failure, so retry the next mutation interval.
                    assert_eq!(error.raw_os_error(), Some(32));
                }
                std::thread::sleep(Duration::from_millis(75));
            }
        });
        started.wait();
        registry.demand_scan(&[root]);
        writer.join().unwrap();
        assert_eq!(registry.active()["user.fake.echo"].source_hash, hash);
        assert_eq!(registry.diagnostics[&path], "TOOL_MANIFEST_STABILIZING");
    }

    fn search_fixture(
        package_id: &str,
        tool_id: &str,
        alias: &str,
        summary: &str,
        description: &str,
    ) -> String {
        let source = fixture().replace("user.fake.echo", package_id);
        source
            .replace(&format!("{package_id}.run"), tool_id)
            .replace("user.fake.echo.run", tool_id)
            .replace(
                "description = \"Contract fixture action.\"",
                &format!(
                    "description = \"{description}\"\naliases = [\"{alias}\"]\ntags = [\"knowledge\"]\ntask_kinds = [\"inspect\"]"
                ),
            )
            .replace("summary = \"Echoes a value.\"", &format!("summary = \"{summary}\""))
    }

    fn package_with_actions(package_index: usize, action_count: usize) -> String {
        let package_id = format!("user.capacity.pkg{package_index:03}");
        let manifest = fixture().replace("user.fake.echo", &package_id);
        let (header, action) = manifest
            .split_once("[[actions]]")
            .expect("fixture has one action");
        let mut output = header.to_owned();
        for action_index in 0..action_count {
            output.push_str("[[actions]]");
            output.push_str(&action.replace(
                &format!("{package_id}.run"),
                &format!("{package_id}.action{action_index:03}"),
            ));
        }
        output
    }

    #[test]
    // matrix: MCP-R019 MCP-R020
    fn registry_enforces_128_package_512_action_limits_without_evicting_active_packages() {
        let directory = temp_root("capacity");
        for package_index in 0..128 {
            fs::write(
                directory.join(format!("package-{package_index:03}.toml")),
                package_with_actions(package_index, 4),
            )
            .unwrap();
        }
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory: directory.clone(),
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        assert_eq!(registry.active().len(), 128);
        assert_eq!(registry.search_actions("").len(), 512);
        let snapshot = registry.snapshot_hash();

        let excess = directory.join("00-excess.toml");
        fs::write(&excess, package_with_actions(999, 1)).unwrap();
        registry.demand_scan(&[root]);
        assert_eq!(registry.active().len(), 128);
        assert_eq!(registry.search_actions("").len(), 512);
        assert_eq!(registry.snapshot_hash(), snapshot);
        assert_eq!(registry.diagnostics[&directory], "TOOL_REGISTRY_LIMIT");
        assert!(!registry.diagnostics.contains_key(&excess));
        assert!(!registry.active().contains_key("user.capacity.pkg999"));
    }

    #[test]
    // matrix: MCP-H011 MCP-H013 MCP-H014
    fn deterministic_search_uses_the_frozen_multilingual_score_and_tie_break() {
        let directory = temp_root("deterministic-search");
        fs::write(
            directory.join("alpha.toml"),
            search_fixture(
                "user.search.alpha",
                "user.search.alpha.run",
                "별찾기",
                "한국어 English document",
                "계약 설명",
            ),
        )
        .unwrap();
        fs::write(
            directory.join("beta.toml"),
            search_fixture(
                "user.search.beta",
                "user.search.beta.run",
                "second",
                "shared",
                "다른 설명",
            ),
        )
        .unwrap();
        fs::write(
            directory.join("gamma.toml"),
            search_fixture(
                "user.search.gamma",
                "user.search.gamma.run",
                "third",
                "shared",
                "또 다른 설명",
            ),
        )
        .unwrap();
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        }]);

        let cases = [
            ("user.search.alpha.run", 1_000, "user.search.alpha.run"),
            ("별찾기", 800, "user.search.alpha.run"),
            ("knowledge", 300, "user.search.alpha.run"),
            ("한국어", 40, "user.search.alpha.run"),
            ("english", 40, "user.search.alpha.run"),
        ];
        for (query, score, tool_id) in cases {
            let hits = registry.search_actions(query);
            assert_eq!(hits[0].score, score, "query={query}");
            assert_eq!(hits[0].action.tool_id, tool_id, "query={query}");
            let repeated: Vec<_> = registry
                .search_actions(query)
                .iter()
                .map(|hit| (hit.score, hit.action.tool_id.as_str()))
                .collect();
            let first: Vec<_> = hits
                .iter()
                .map(|hit| (hit.score, hit.action.tool_id.as_str()))
                .collect();
            assert_eq!(first, repeated);
        }
        let tied = registry.search_actions("shared");
        assert_eq!(tied[0].score, 40);
        assert_eq!(tied[0].action.tool_id, "user.search.beta.run");
        assert_eq!(tied[1].action.tool_id, "user.search.gamma.run");
    }

    #[test]
    // matrix: MCP-H006 MCP-P013
    fn follow_path_executable_byte_identity_changes_descriptor_and_snapshot() {
        let directory = temp_root("follow-path-identity");
        let executable = directory.join("live.exe");
        fs::write(&executable, b"first executable identity").unwrap();
        let path = executable.display().to_string().replace('\\', "\\\\");
        let manifest = replace_fixture_executable(fixture(), &path, None).replace(
            "update_policy = \"pinned_hash\"",
            "update_policy = \"follow_path\"",
        );
        fs::write(directory.join("follow.toml"), manifest).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let descriptor = RegistryRuntime::descriptor_hash(package, action);
        let snapshot = registry.snapshot_hash();
        let revision = registry.revision;

        fs::write(&executable, b"second executable identity").unwrap();
        registry.demand_scan(&[root]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_ne!(
            descriptor,
            RegistryRuntime::descriptor_hash(package, action)
        );
        assert_ne!(snapshot, registry.snapshot_hash());
        assert!(registry.revision > revision);
    }

    #[test]
    // matrix: MCP-H006
    fn follow_path_detects_same_path_same_size_executable_swap_with_preserved_timestamp() {
        let directory = temp_root("follow-path-preserved-timestamp");
        let executable = directory.join("live.exe");
        fs::write(&executable, b"identity-version-alpha").unwrap();
        let original_modified = fs::metadata(&executable).unwrap().modified().unwrap();
        let path = executable.display().to_string().replace('\\', "\\\\");
        let manifest = replace_fixture_executable(fixture(), &path, None).replace(
            "update_policy = \"pinned_hash\"",
            "update_policy = \"follow_path\"",
        );
        fs::write(directory.join("follow.toml"), manifest).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(std::slice::from_ref(&root));
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let descriptor = RegistryRuntime::descriptor_hash(package, action);

        fs::write(&executable, b"identity-version-omega").unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&executable)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(original_modified))
            .unwrap();
        assert_eq!(
            fs::metadata(&executable).unwrap().modified().unwrap(),
            original_modified
        );
        registry.demand_scan(&[root]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_ne!(
            descriptor,
            RegistryRuntime::descriptor_hash(package, action)
        );
    }

    #[test]
    fn registry_executable_hash_handle_denies_in_place_writers() {
        let directory = temp_root("scan-executable-lease");
        let executable = directory.join("leased.exe");
        fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
        let lease = open_manifest_candidate(&executable).unwrap();
        assert!(stable_file_stamp(&lease, u64::MAX).is_some());
        assert!(
            fs::OpenOptions::new()
                .write(true)
                .open(&executable)
                .is_err(),
            "scan hash handle must deny an in-place writer"
        );
    }

    #[test]
    fn location_config_revision_changes_descriptor_even_when_path_is_unchanged() {
        let directory = temp_root("location-config-revision");
        let manifest_path = directory.join("location.toml");
        let manifest = fixture()
            .lines()
            .filter_map(|line| {
                if line.starts_with("locator_kind = ") {
                    Some("locator_kind = \"location_ref\"\nlocation_ref = \"test-location\"")
                } else if line.starts_with("path = ") {
                    None
                } else {
                    Some(line)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&manifest_path, manifest).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut policy = UserToolRegistryConfig::default();
        policy
            .locations
            .insert("test-location".to_owned(), std::env::current_exe().unwrap());
        policy.config_revision = Sha256Hash::digest(b"config revision one");
        let mut registry = RegistryRuntime::default();
        registry.set_policy(policy.clone());
        registry.demand_scan(std::slice::from_ref(&root));
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let first = RegistryRuntime::descriptor_hash(package, action);

        policy.config_revision = Sha256Hash::digest(b"config revision two");
        registry.set_policy(policy);
        registry.demand_scan(std::slice::from_ref(&root));
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_ne!(first, RegistryRuntime::descriptor_hash(package, action));
        assert_eq!(
            package.location_config_revision,
            Some(Sha256Hash::digest(b"config revision two"))
        );
    }

    #[test]
    fn fixed_working_directory_final_path_is_hashed_and_unsafe_paths_are_rejected() {
        let mut manifest = parse_manifest_v1(&fixture(), ManifestSource::User).unwrap();
        manifest.executables[0].working_directory = "fixed".to_owned();
        manifest.executables[0].fixed_working_directory =
            Some(std::env::temp_dir().display().to_string());
        let hashes = resolve_fixed_working_directory_hashes(&manifest).unwrap();
        assert!(hashes.contains_key("fake-echo"));

        manifest.executables[0].fixed_working_directory =
            Some(r"\\server\share\working".to_owned());
        assert!(resolve_fixed_working_directory_hashes(&manifest).is_none());
    }

    #[test]
    // matrix: MCP-P011 MCP-P012
    fn compatible_candidate_requires_probe_success_and_failure_keeps_lkg() {
        let directory = temp_root("compatible-probe-lkg");
        let manifest_path = directory.join("tool.toml");
        fs::write(&manifest_path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory: directory.clone(),
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(std::slice::from_ref(&root));
        let lkg_hash = RegistryRuntime::package_semantic_hash(
            registry.active().get("user.fake.echo").unwrap(),
        );

        let executable = directory.join("compatible.exe");
        fs::write(&executable, b"compatible candidate bytes").unwrap();
        let path = executable.display().to_string().replace('\\', "\\\\");
        let compatible = replace_fixture_executable(
            fixture(),
            &path,
            Some("product_version_req = \"^1\"\nauthenticode_policy = \"require_subject\"\nauthenticode_subject = \"Test Publisher\""),
        )
            .replace("update_policy = \"pinned_hash\"", "update_policy = \"version_compatible\"")
            .replace(
                "architectures = [\"x86_64\"]",
                "architectures = [\"x86_64\"]\n\n[executables.probe]\nkind = \"argv\"\nargs = [\"probe-version\"]\noutput_format = \"semver_line\"\nversion_pattern = '^(?P<product>[0-9]+\\.[0-9]+\\.[0-9]+) interface=(?P<interface>[0-9]+\\.[0-9]+\\.[0-9]+)$'",
            );
        fs::write(&manifest_path, compatible).unwrap();
        registry.demand_scan(std::slice::from_ref(&root));
        assert_eq!(
            RegistryRuntime::package_semantic_hash(
                registry.active().get("user.fake.echo").unwrap(),
            ),
            lkg_hash,
            "unprobed candidate must not replace the active package"
        );
        assert!(registry.probe_candidate("user.fake.echo").is_some());
        assert_eq!(registry.next_automatic_probe().unwrap().1, "fake-echo");
        assert_eq!(registry.diagnostics[&manifest_path], "TOOL_PROBE_REQUIRED");

        assert!(registry.reject_compatible_probe("user.fake.echo"));
        assert!(registry.next_automatic_probe().is_none());
        assert_eq!(
            RegistryRuntime::package_semantic_hash(
                registry.active().get("user.fake.echo").unwrap(),
            ),
            lkg_hash
        );
        assert_eq!(
            registry.diagnostics[&manifest_path],
            "TOOL_PROBE_FAILED_LKG_RETAINED"
        );

        let pending = registry
            .pending_compatible
            .get_mut("user.fake.echo")
            .unwrap();
        let mut second = pending.manifest.executables[0].clone();
        second.executable_id = "second-compatible".to_owned();
        pending.manifest.executables.push(second);
        pending.resolved_executable_hashes.insert(
            "second-compatible".to_owned(),
            pending.resolved_executable_hashes["fake-echo"].clone(),
        );
        pending.resolved_executable_paths.insert(
            "second-compatible".to_owned(),
            pending.resolved_executable_paths["fake-echo"].clone(),
        );
        let _ = pending.manifest_hash.take();
        let _ = pending.semantic_hash.take();
        let _ = pending.descriptor_hashes.take();

        assert!(!registry.accept_compatible_probe(
            "user.fake.echo",
            "fake-echo",
            "1.2.3".to_owned(),
            Some("1.0.0".to_owned()),
            BTreeSet::from(["progress".to_owned(), "stdin_cancel".to_owned()]),
        ));
        assert_eq!(
            RegistryRuntime::package_semantic_hash(
                registry.active().get("user.fake.echo").unwrap()
            ),
            lkg_hash,
            "one successful probe cannot activate an unprobed executable"
        );
        assert!(registry.accept_compatible_probe(
            "user.fake.echo",
            "second-compatible",
            "2.0.0".to_owned(),
            Some("1.0.0".to_owned()),
            BTreeSet::new(),
        ));
        let active = registry.active().get("user.fake.echo").unwrap();
        assert_ne!(RegistryRuntime::package_semantic_hash(active), lkg_hash);
        assert_eq!(
            active.resolved_executable_hashes["fake-echo"],
            Sha256Hash::digest(b"compatible candidate bytes")
        );
        assert_eq!(active.probed_product_versions["fake-echo"], "1.2.3");
        assert_eq!(active.probed_product_versions["second-compatible"], "2.0.0");
        assert!(active.probed_capabilities["fake-echo"].contains("stdin_cancel"));
    }

    #[test]
    // matrix: MCP-P011
    fn pinned_version_constraint_also_requires_probe_before_activation() {
        let directory = temp_root("pinned-required-probe");
        let manifest_path = directory.join("fake.toml");
        let manifest = fixture().replace(
            "architectures = [\"x86_64\"]",
            "architectures = [\"x86_64\"]\nproduct_version_req = \"^1\"\n\n[executables.probe]\nkind = \"argv\"\nargs = [\"probe-version\"]\noutput_format = \"semver_line\"\nversion_pattern = '^(?P<product>[0-9]+\\.[0-9]+\\.[0-9]+)$'",
        );
        fs::write(&manifest_path, manifest).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(std::slice::from_ref(&root));
        assert!(registry.active().is_empty());
        assert!(registry.probe_candidate("user.fake.echo").is_some());
        assert_eq!(registry.diagnostics[&manifest_path], "TOOL_PROBE_REQUIRED");

        assert!(registry.accept_compatible_probe(
            "user.fake.echo",
            "fake-echo",
            "1.2.3".to_owned(),
            None,
            BTreeSet::from(["progress".to_owned()]),
        ));
        let active = registry.active().get("user.fake.echo").unwrap();
        assert_eq!(active.probed_product_versions["fake-echo"], "1.2.3");
        assert_eq!(
            active.probed_capabilities["fake-echo"],
            BTreeSet::from(["progress".to_owned()])
        );
    }

    #[test]
    fn explicit_probe_of_an_active_package_updates_status_and_failure_diagnostics() {
        let directory = temp_root("active-explicit-probe");
        let manifest_path = directory.join("fake.toml");
        let manifest = fixture().replace(
            "architectures = [\"x86_64\"]",
            "architectures = [\"x86_64\"]\n\n[executables.probe]\nkind = \"argv\"\nargs = [\"probe-version\"]\noutput_format = \"semver_line\"\nversion_pattern = '^(?P<product>[0-9]+\\.[0-9]+\\.[0-9]+)$'",
        );
        fs::write(&manifest_path, manifest).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(std::slice::from_ref(&root));
        assert!(registry.active().contains_key("user.fake.echo"));

        let revision = registry.revision;
        let diagnostic_revision = registry.diagnostic_revision;
        assert!(!registry.accept_compatible_probe(
            "user.fake.echo",
            "fake-echo",
            "1.2.3".to_owned(),
            Some("1.0.0".to_owned()),
            BTreeSet::new(),
        ));
        assert_eq!(registry.revision, revision);
        assert_eq!(registry.diagnostic_revision, diagnostic_revision + 1);
        assert!(registry.last_probe_at("user.fake.echo").is_some());
        assert_eq!(
            registry
                .candidate_observation("user.fake.echo")
                .map(|candidate| candidate.state),
            Some("ready")
        );

        let diagnostic_revision = registry.diagnostic_revision;
        assert!(registry.reject_compatible_probe("user.fake.echo"));
        assert_eq!(registry.revision, revision);
        assert_eq!(registry.diagnostic_revision, diagnostic_revision + 1);
        assert_eq!(
            registry.diagnostics[&manifest_path],
            "TOOL_PROBE_FAILED_LKG_RETAINED"
        );
        assert_eq!(
            registry
                .candidate_observation("user.fake.echo")
                .map(|candidate| candidate.state),
            Some("incompatible")
        );

        let diagnostic_revision = registry.diagnostic_revision;
        assert!(!registry.accept_compatible_probe(
            "user.fake.echo",
            "fake-echo",
            "1.2.3".to_owned(),
            Some("1.0.0".to_owned()),
            BTreeSet::new(),
        ));
        assert_eq!(registry.diagnostic_revision, diagnostic_revision + 1);
        assert!(!registry.diagnostics.contains_key(&manifest_path));
        assert_eq!(
            registry
                .candidate_observation("user.fake.echo")
                .map(|candidate| candidate.state),
            Some("ready")
        );

        fs::write(&manifest_path, "format_version = [invalid").unwrap();
        registry.demand_scan(std::slice::from_ref(&root));
        assert_eq!(
            registry
                .candidate_observation("user.fake.echo")
                .map(|candidate| candidate.state),
            Some("invalid")
        );
        let invalid_diagnostic = registry.diagnostics[&manifest_path].clone();
        assert!(!registry.accept_compatible_probe(
            "user.fake.echo",
            "fake-echo",
            "1.2.3".to_owned(),
            Some("1.0.0".to_owned()),
            BTreeSet::new(),
        ));
        assert_eq!(
            registry
                .candidate_observation("user.fake.echo")
                .map(|candidate| candidate.state),
            Some("invalid")
        );
        assert_eq!(registry.diagnostics[&manifest_path], invalid_diagnostic);
    }

    #[test]
    fn configured_protocol_intersection_is_enforced_before_package_activation() {
        let directory = temp_root("protocol-policy");
        let manifest_path = directory.join("fake.toml");
        fs::write(&manifest_path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let policy = UserToolRegistryConfig {
            allowed_process_protocols: vec!["star_json_stdio_v1".to_owned()],
            ..Default::default()
        };
        let mut registry = RegistryRuntime::default();
        registry.set_policy(policy);
        registry.demand_scan(std::slice::from_ref(&root));
        assert!(registry.active().is_empty());
        assert_eq!(
            registry.diagnostics[&manifest_path],
            "TOOL_UPDATE_POLICY_DENIED"
        );
    }

    #[test]
    // matrix: MCP-H008 MCP-S004
    fn descriptor_contains_secret_reference_identity_but_never_secret_value() {
        let directory = temp_root("secret-ref-hash");
        let path = directory.join("secret.toml");
        let with_ref = fixture().replace(
            "architectures = [\"x86_64\"]",
            "architectures = [\"x86_64\"]\n\n[[executables.environment_values]]\nname = \"STAR_SECRET\"\nsecret_ref = \"env:SECRET_A\"",
        );
        fs::write(&path, &with_ref).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let first = RegistryRuntime::descriptor_hash(package, action);
        let serialized = serde_json::to_string(&normalized_manifest(&package.manifest)).unwrap();
        assert!(serialized.contains("env:SECRET_A"));
        assert!(!serialized.contains("actual-secret-value"));

        fs::write(&path, with_ref.replace("env:SECRET_A", "env:SECRET_B")).unwrap();
        registry.demand_scan(&[root]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_ne!(first, RegistryRuntime::descriptor_hash(package, action));
    }

    #[test]
    // matrix: MCP-R008
    fn invalid_required_core_blocks_core_readiness_without_hiding_external_packages() {
        let release = temp_root("required-core-invalid");
        let user = temp_root("required-core-external");
        fs::write(
            release.join("star-control-core.toml"),
            "format_version = 1\npackage_id = \"star.control.core\"\nrequired = true\ninvalid = [",
        )
        .unwrap();
        fs::write(user.join("external.toml"), fixture()).unwrap();
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[
            RegistrySourceRoot {
                source: ManifestSource::Release,
                directory: release,
            },
            RegistrySourceRoot {
                source: ManifestSource::User,
                directory: user,
            },
        ]);
        assert!(!registry.core_ready());
        assert!(registry.find_action("user.fake.echo.run").is_some());
        assert_eq!(registry.search_actions("user.fake.echo.run").len(), 1);
    }

    fn replacement_fixture(package_id: &str, target: Option<&str>) -> String {
        let source = search_fixture(
            package_id,
            &format!("{package_id}.run"),
            "replacement",
            "Replacement action",
            "Replacement test package",
        );
        target.map_or(source.clone(), |target| {
            source.replace(
                "backend_kinds = [\"process\"]",
                &format!(
                    "backend_kinds = [\"process\"]\nreplaces = [{{ package_id = \"{target}\", version_req = \"^1.0\" }}]"
                ),
            )
        })
    }

    #[test]
    // matrix: MCP-R016
    fn replacement_graph_accepts_one_trusted_bounded_replacer_and_rejects_conflicts() {
        let directory = temp_root("replacement-valid");
        fs::write(
            directory.join("target.toml"),
            replacement_fixture("user.replace.target", None)
                .replace("user.replace.target.run", "user.replace.shared.run"),
        )
        .unwrap();
        fs::write(
            directory.join("replacer.toml"),
            replacement_fixture("user.replace.new", Some("user.replace.target"))
                .replace("user.replace.new.run", "user.replace.shared.run"),
        )
        .unwrap();
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        }]);
        let trusted: BTreeSet<_> = ["user.replace.new".to_owned()].into_iter().collect();
        let resolution = registry.resolve_replacements(&trusted);
        assert_eq!(
            resolution.replaced_by.get("user.replace.target"),
            Some(&"user.replace.new".to_owned())
        );
        assert!(resolution.conflicts.is_empty());
        let visible: Vec<_> = registry
            .search_actions_with_trust("replacement", &trusted)
            .iter()
            .map(|hit| hit.package.manifest.package_id.as_str())
            .collect();
        assert_eq!(visible, vec!["user.replace.new"]);
        let (effective, action) = registry
            .find_effective_action("user.replace.shared.run", &trusted)
            .expect("the trusted replacer owns the shared ToolId");
        assert_eq!(effective.manifest.package_id, "user.replace.new");
        assert_eq!(action.tool_id, "user.replace.shared.run");

        let directory = temp_root("replacement-multiple");
        for (name, target) in [
            ("user.multi.target", None),
            ("user.multi.first", Some("user.multi.target")),
            ("user.multi.second", Some("user.multi.target")),
        ] {
            fs::write(
                directory.join(format!("{name}.toml")),
                replacement_fixture(name, target),
            )
            .unwrap();
        }
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        }]);
        let trusted: BTreeSet<_> = registry.active.keys().cloned().collect();
        let resolution = registry.resolve_replacements(&trusted);
        assert!(resolution.replaced_by.is_empty());
        assert!(resolution.conflicts.contains("user.multi.first"));
        assert!(resolution.conflicts.contains("user.multi.second"));

        let directory = temp_root("replacement-cycle");
        fs::write(
            directory.join("a.toml"),
            replacement_fixture("user.cycle.a", Some("user.cycle.b")),
        )
        .unwrap();
        fs::write(
            directory.join("b.toml"),
            replacement_fixture("user.cycle.b", Some("user.cycle.a")),
        )
        .unwrap();
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        }]);
        let trusted: BTreeSet<_> = registry.active.keys().cloned().collect();
        let resolution = registry.resolve_replacements(&trusted);
        assert!(resolution.replaced_by.is_empty());
        assert_eq!(resolution.conflicts.len(), 2);
    }

    #[test]
    fn unrelated_tool_id_collisions_fail_closed_and_required_release_ownership_wins() {
        let directory = temp_root("tool-id-collision");
        for package_id in ["user.collision.alpha", "user.collision.beta"] {
            fs::write(
                directory.join(format!("{package_id}.toml")),
                search_fixture(
                    package_id,
                    "shared.collision.run",
                    "collision",
                    "Shared collision",
                    "Unrelated duplicate ToolId",
                ),
            )
            .unwrap();
        }
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        }]);
        let trusted: BTreeSet<_> = registry.active.keys().cloned().collect();
        assert!(
            registry
                .search_actions_with_trust("shared.collision.run", &trusted)
                .is_empty()
        );
        assert!(
            registry
                .find_effective_action("shared.collision.run", &trusted)
                .is_none()
        );
        let excluded = BTreeSet::from(["user.collision.alpha".to_owned()]);
        let visible =
            registry.search_actions_with_policy("shared.collision.run", &trusted, &excluded);
        assert_eq!(visible.len(), 1);
        assert_eq!(
            visible[0].package.manifest.package_id,
            "user.collision.beta"
        );

        let release = temp_root("tool-id-required-release");
        let user = temp_root("tool-id-required-user");
        fs::write(
            release.join(REQUIRED_RELEASE_MANIFEST_NAME),
            REQUIRED_RELEASE_MANIFEST,
        )
        .unwrap();
        fs::write(
            user.join("shadow.toml"),
            search_fixture(
                "user.required.shadow",
                "star.core.goal.start",
                "shadow",
                "Untrusted shadow",
                "Must not shadow required release",
            ),
        )
        .unwrap();
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[
            RegistrySourceRoot {
                source: ManifestSource::Release,
                directory: release,
            },
            RegistrySourceRoot {
                source: ManifestSource::User,
                directory: user,
            },
        ]);
        let trusted: BTreeSet<_> = registry.active.keys().cloned().collect();
        let (package, _) = registry
            .find_effective_action("star.core.goal.start", &trusted)
            .expect("required release ToolId remains effective");
        assert_eq!(package.manifest.package_id, "star.control.core");
    }

    #[test]
    fn release_catalog_requires_the_embedded_raw_checksum_and_keeps_lkg_on_tamper() {
        let directory = temp_root("release-integrity");
        let path = directory.join(REQUIRED_RELEASE_MANIFEST_NAME);
        fs::write(&path, REQUIRED_RELEASE_MANIFEST).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::Release,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(std::slice::from_ref(&root));
        assert!(registry.core_ready());
        let snapshot = registry.snapshot_hash();

        fs::write(&path, format!("{REQUIRED_RELEASE_MANIFEST}\n")).unwrap();
        registry.demand_scan(std::slice::from_ref(&root));

        assert!(registry.core_ready());
        assert_eq!(registry.snapshot_hash(), snapshot);
        assert_eq!(registry.diagnostics[&path], "TOOL_INTEGRITY_INVALID");

        let unknown = root.directory.join("unlisted-release.toml");
        fs::write(&unknown, fixture()).unwrap();
        registry.demand_scan(&[root]);
        assert_eq!(registry.diagnostics[&unknown], "TOOL_INTEGRITY_INVALID");
        assert!(!registry.active().contains_key("user.fake.echo"));
    }

    fn bounded_registry_fixture(index: usize) -> String {
        let package_id = format!("user.limit.p{index:03}");
        let source = fixture().replace("user.fake.echo", &package_id);
        let (prefix, action) = source.split_once("[[actions]]").unwrap();
        let mut result = prefix.to_owned();
        for action_index in 0..4 {
            result.push_str("[[actions]]");
            result.push_str(&action.replace(
                &format!("tool_id = \"{package_id}.run\""),
                &format!("tool_id = \"{package_id}.action{action_index}\""),
            ));
        }
        result
    }

    #[test]
    // matrix: MCP-R019 MCP-R020
    fn registry_caps_128_packages_and_512_actions_without_evicting_active_entries() {
        let directory = temp_root("registry-capacity");
        for index in 0..128 {
            fs::write(
                directory.join(format!("p{index:03}.toml")),
                bounded_registry_fixture(index),
            )
            .unwrap();
        }
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory: directory.clone(),
        };
        let mut registry = RegistryRuntime::default();
        let started = std::time::Instant::now();
        registry.demand_scan(&[root.clone()]);
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(registry.active().len(), 128);
        assert_eq!(
            registry
                .active()
                .values()
                .map(|package| package.manifest.actions.len())
                .sum::<usize>(),
            512
        );
        let first: Vec<_> = registry
            .search_actions("")
            .iter()
            .map(|hit| hit.action.tool_id.clone())
            .collect();
        let second: Vec<_> = registry
            .search_actions("")
            .iter()
            .map(|hit| hit.action.tool_id.clone())
            .collect();
        assert_eq!(first, second);
        assert_eq!(first.len(), 512);

        fs::write(
            directory.join("overflow.toml"),
            replacement_fixture("user.limit.overflow", None),
        )
        .unwrap();
        registry.demand_scan(&[root]);
        assert_eq!(registry.active().len(), 128);
        assert!(!registry.active().contains_key("user.limit.overflow"));
        assert!(
            registry
                .diagnostics
                .values()
                .any(|diagnostic| diagnostic == "TOOL_REGISTRY_LIMIT")
        );
    }
}
