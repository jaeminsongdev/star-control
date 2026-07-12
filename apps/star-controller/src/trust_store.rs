use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::PathBuf,
};

use chrono::{DateTime, SecondsFormat, Utc};
use star_contracts::{
    Sha256Hash, ToolTrustId,
    canonical::canonical_sha256,
    parse_no_duplicate_keys,
    registry::RegistrySource,
    runtime::IsolationProfile,
    trust::{ToolTrustRecord, TrustMode, TrustedExecutable},
};
use thiserror::Error;

use crate::registry_runtime::{ActivePackage, RegistryRuntime};

const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum TrustStoreError {
    #[error("LOCALAPPDATA is not available")]
    LocalAppDataUnavailable,
    #[error("trust store I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("trust store is corrupt")]
    Corrupt,
    #[error("trust store DACL failed")]
    Dacl,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TrustFile {
    schema_id: String,
    schema_version: u32,
    format_version: u32,
    records: BTreeMap<String, ToolTrustRecord>,
    #[serde(default)]
    history: Vec<ToolTrustRecord>,
    #[serde(default)]
    revoked_packages: BTreeSet<String>,
}

impl Default for TrustFile {
    fn default() -> Self {
        Self {
            schema_id: "star.tool-trust-store".to_owned(),
            schema_version: 1,
            format_version: FORMAT_VERSION,
            records: BTreeMap::new(),
            history: Vec::new(),
            revoked_packages: BTreeSet::new(),
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyTrustFile {
    records: BTreeMap<String, LegacyTrustRecord>,
    #[serde(default)]
    revoked_packages: BTreeSet<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyTrustRecord {
    package_id: String,
}

pub struct TrustStore {
    path: PathBuf,
    file: TrustFile,
}

/// Authorization captured immediately before process creation. Revocation
/// blocks later authorization, while already-started work keeps this immutable
/// evidence lease and is never replayed.
#[derive(Clone, Debug)]
pub struct RunningTrustLease {
    pub trust_id: ToolTrustId,
    pub package_id: String,
    pub manifest_hash: Sha256Hash,
    pub authorized_at: DateTime<Utc>,
}

impl RunningTrustLease {
    pub fn permits_already_started_operation(&self) -> bool {
        true
    }
}

impl TrustStore {
    pub fn default_path() -> Result<PathBuf, TrustStoreError> {
        Ok(PathBuf::from(
            std::env::var_os("LOCALAPPDATA").ok_or(TrustStoreError::LocalAppDataUnavailable)?,
        )
        .join("Star-Control")
        .join("trust")
        .join("tool-trust.v1.json"))
    }

    pub fn load(path: PathBuf) -> Result<Self, TrustStoreError> {
        let file = match fs::read(&path) {
            Ok(bytes) => {
                let text = std::str::from_utf8(&bytes).map_err(|_| TrustStoreError::Corrupt)?;
                let value = parse_no_duplicate_keys(text).map_err(|_| TrustStoreError::Corrupt)?;
                match serde_json::from_value::<TrustFile>(value.clone()) {
                    Ok(file)
                        if file.schema_id == "star.tool-trust-store"
                            && file.schema_version == 1
                            && file.format_version == FORMAT_VERSION =>
                    {
                        file
                    }
                    Ok(_) => return Err(TrustStoreError::Corrupt),
                    Err(_) => {
                        // The pre-contract store contained only manifest hashes.
                        // It cannot authorize the wider v1 scope, so preserve the
                        // package IDs as revoked instead of silently upgrading it.
                        let legacy: LegacyTrustFile =
                            serde_json::from_value(value).map_err(|_| TrustStoreError::Corrupt)?;
                        let mut file = TrustFile::default();
                        file.revoked_packages.extend(legacy.revoked_packages);
                        file.revoked_packages
                            .extend(legacy.records.into_values().map(|record| record.package_id));
                        file
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => TrustFile::default(),
            Err(error) => return Err(TrustStoreError::Io(error)),
        };
        Ok(Self { path, file })
    }

    pub fn state(&self, package: &ActivePackage, now: DateTime<Utc>) -> &'static str {
        if self.is_revoked(&package.manifest.package_id) {
            return "untrusted";
        }
        let Some(record) = self.file.records.get(&package.manifest.package_id) else {
            return "untrusted";
        };
        if record.revoked_at.is_some() || !record_matches_package(record, package) {
            return "untrusted";
        }
        match record.expires_at.as_deref() {
            Some(expires_at)
                if DateTime::parse_from_rfc3339(expires_at)
                    .ok()
                    .is_some_and(|expires_at| expires_at.with_timezone(&Utc) <= now) =>
            {
                "expired"
            }
            _ => "trusted",
        }
    }

    pub fn is_revoked(&self, package_id: &str) -> bool {
        self.file.revoked_packages.contains(package_id)
    }

    pub fn trust_id(&self, package: &ActivePackage, now: DateTime<Utc>) -> Option<ToolTrustId> {
        (self.state(package, now) == "trusted")
            .then(|| self.file.records.get(&package.manifest.package_id))
            .flatten()
            .map(|record| record.trust_id.clone())
    }

    pub fn authorize(
        &self,
        package: &ActivePackage,
        now: DateTime<Utc>,
    ) -> Option<RunningTrustLease> {
        let record = (self.state(package, now) == "trusted")
            .then(|| self.file.records.get(&package.manifest.package_id))
            .flatten()?;
        Some(RunningTrustLease {
            trust_id: record.trust_id.clone(),
            package_id: package.manifest.package_id.clone(),
            manifest_hash: record.manifest_hash.clone(),
            authorized_at: now,
        })
    }

    pub fn grant(
        &mut self,
        package: &ActivePackage,
        trust_mode: TrustMode,
        expires_at: Option<String>,
        granted_by: serde_json::Value,
        now: DateTime<Utc>,
    ) -> Result<ToolTrustRecord, TrustStoreError> {
        if let Some(expires_at) = &expires_at
            && DateTime::parse_from_rfc3339(expires_at)
                .ok()
                .is_none_or(|value| value.with_timezone(&Utc) <= now)
        {
            return Err(TrustStoreError::Corrupt);
        }
        if !self.is_revoked(&package.manifest.package_id)
            && self
                .file
                .records
                .get(&package.manifest.package_id)
                .is_some_and(|record| {
                    record.trust_mode == trust_mode
                        && record.expires_at == expires_at
                        && record_matches_package(record, package)
                })
        {
            return Ok(self.file.records[&package.manifest.package_id].clone());
        }
        let mut record = package_record(package, trust_mode, granted_by, now)?;
        record.expires_at = expires_at;
        if let Some(previous) = self
            .file
            .records
            .insert(package.manifest.package_id.clone(), record.clone())
        {
            self.file.history.push(previous);
        }
        self.file
            .revoked_packages
            .remove(&package.manifest.package_id);
        self.persist()?;
        Ok(record)
    }

    pub fn revoke(
        &mut self,
        package_id: &str,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<bool, TrustStoreError> {
        if reason.trim().is_empty() || reason.chars().count() > 1_000 {
            return Err(TrustStoreError::Corrupt);
        }
        let mut changed = self.file.revoked_packages.insert(package_id.to_owned());
        if let Some(record) = self.file.records.get_mut(package_id)
            && record.revoked_at.is_none()
        {
            record.revoked_at = Some(now.to_rfc3339_opts(SecondsFormat::Millis, true));
            record.revoke_reason = Some(reason.to_owned());
            changed = true;
        }
        self.persist()?;
        Ok(changed)
    }

    pub fn snapshot_hash<'a>(
        &self,
        packages: impl IntoIterator<Item = &'a ActivePackage>,
        now: DateTime<Utc>,
    ) -> Sha256Hash {
        let entries: BTreeMap<_, _> = packages
            .into_iter()
            .map(|package| {
                let trust_id = self.trust_id(package, now);
                (
                    package.manifest.package_id.clone(),
                    serde_json::json!({
                        "package_hash": RegistryRuntime::package_semantic_hash(package),
                        "trust_id": trust_id,
                        "trust_state": self.state(package, now),
                    }),
                )
            })
            .collect();
        canonical_sha256(&serde_json::json!({"packages":entries}))
            .expect("trust snapshot is canonical JSON")
    }

    fn persist(&self) -> Result<(), TrustStoreError> {
        let parent = self.path.parent().ok_or(TrustStoreError::Corrupt)?;
        fs::create_dir_all(parent)?;
        let temp = parent.join(format!(".tool-trust-{}.tmp", star_ipc::nonce()));
        let bytes = serde_json::to_vec(&self.file).map_err(|_| TrustStoreError::Corrupt)?;
        fs::write(&temp, bytes)?;
        fs::OpenOptions::new().write(true).open(&temp)?.sync_all()?;
        fs::rename(temp, &self.path)?;
        star_ipc::key_store::apply_owner_system_dacl(&self.path).map_err(|_| TrustStoreError::Dacl)
    }
}

fn source_kind(package: &ActivePackage) -> RegistrySource {
    match package.source {
        star_contracts::manifest::ManifestSource::Release => RegistrySource::Release,
        star_contracts::manifest::ManifestSource::User => RegistrySource::User,
        star_contracts::manifest::ManifestSource::Project => RegistrySource::Project,
    }
}

fn locator_hash(package: &ActivePackage, executable_id: &str) -> Option<Sha256Hash> {
    package
        .resolved_executable_paths
        .get(executable_id)
        .map(|path| {
            Sha256Hash::digest(
                path.as_os_str()
                    .to_string_lossy()
                    .replace('\\', "/")
                    .to_lowercase()
                    .as_bytes(),
            )
        })
}

fn trusted_executables(
    package: &ActivePackage,
    trust_mode: TrustMode,
) -> Result<Vec<TrustedExecutable>, TrustStoreError> {
    let mut values = Vec::new();
    for executable in &package.manifest.executables {
        let exact_hash = match trust_mode {
            TrustMode::Exact => package
                .resolved_executable_hashes
                .get(&executable.executable_id)
                .cloned(),
            TrustMode::Compatible | TrustMode::ManagedPath => None,
        };
        values.push(TrustedExecutable {
            executable_id: executable.executable_id.clone(),
            locator_hash: locator_hash(package, &executable.executable_id)
                .ok_or(TrustStoreError::Corrupt)?,
            config_revision: if executable.locator_kind
                == star_contracts::manifest::LocatorKind::LocationRef
            {
                Some(
                    package
                        .location_config_revision
                        .clone()
                        .ok_or(TrustStoreError::Corrupt)?,
                )
            } else {
                None
            },
            fixed_working_directory_hash: if executable.working_directory == "fixed" {
                Some(
                    package
                        .fixed_working_directory_hashes
                        .get(&executable.executable_id)
                        .cloned()
                        .ok_or(TrustStoreError::Corrupt)?,
                )
            } else {
                None
            },
            update_policy: executable.update_policy,
            exact_hash,
            publisher_subject: executable.authenticode_subject.clone(),
            product_version_req: executable
                .product_version_req
                .clone()
                .unwrap_or_else(|| "*".to_owned()),
            interface_version_req: executable.interface_version_req.clone(),
        });
    }
    values.sort_by(|left, right| left.executable_id.cmp(&right.executable_id));
    Ok(values)
}

fn permission_actions(package: &ActivePackage) -> Vec<String> {
    let mut values: Vec<_> = package
        .manifest
        .actions
        .iter()
        .flat_map(|action| action.permission_actions.iter().cloned())
        .collect();
    values.sort();
    values.dedup();
    values
}

fn isolation_profiles(package: &ActivePackage) -> Vec<IsolationProfile> {
    let mut trusted = false;
    let mut appcontainer = false;
    for value in package
        .manifest
        .executables
        .iter()
        .flat_map(|executable| &executable.isolation_compatibility)
    {
        match value.as_str() {
            "trusted_desktop" => trusted = true,
            "appcontainer_adapter" => appcontainer = true,
            _ => {}
        }
    }
    let mut values = Vec::new();
    if appcontainer {
        values.push(IsolationProfile::AppcontainerAdapter);
    }
    if trusted {
        values.push(IsolationProfile::TrustedDesktop);
    }
    values
}

fn package_record(
    package: &ActivePackage,
    trust_mode: TrustMode,
    granted_by: serde_json::Value,
    now: DateTime<Utc>,
) -> Result<ToolTrustRecord, TrustStoreError> {
    Ok(ToolTrustRecord {
        schema_id: "star.tool-trust-record".to_owned(),
        schema_version: 1,
        trust_id: ToolTrustId::new(),
        package_id: package.manifest.package_id.clone(),
        package_version: package.manifest.package_version.clone(),
        source_kind: source_kind(package),
        source_id_hash: source_id_hash(package),
        manifest_hash: RegistryRuntime::manifest_hash(package),
        schema_hashes: package.resources.schema_hashes.clone(),
        trust_mode,
        executables: trusted_executables(package, trust_mode)?,
        permission_actions: permission_actions(package),
        isolation_profiles: isolation_profiles(package),
        granted_by,
        granted_at: now.to_rfc3339_opts(SecondsFormat::Millis, true),
        expires_at: None,
        revoked_at: None,
        revoke_reason: None,
    })
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

fn record_matches_package(record: &ToolTrustRecord, package: &ActivePackage) -> bool {
    if record.schema_id != "star.tool-trust-record"
        || record.schema_version != 1
        || record.package_id != package.manifest.package_id
        || record.package_version != package.manifest.package_version
        || record.source_kind != source_kind(package)
        || record.source_id_hash != source_id_hash(package)
        || record.manifest_hash != RegistryRuntime::manifest_hash(package)
        || record.schema_hashes != package.resources.schema_hashes
        || record.permission_actions != permission_actions(package)
        || record.isolation_profiles != isolation_profiles(package)
    {
        return false;
    }
    trusted_executables(package, record.trust_mode)
        .is_ok_and(|executables| executables == record.executables)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest_resources::ManifestResources;
    use star_contracts::{manifest::parse_manifest_v1, trust::TrustMode};

    #[test]
    fn durable_trust_json_rejects_duplicate_keys() {
        let path = std::env::temp_dir().join(format!(
            "star-control-trust-duplicate-{}.json",
            star_ipc::nonce()
        ));
        fs::write(
            &path,
            br#"{"schema_id":"star.tool-trust-store","schema_version":1,"format_version":1,"format_version":1,"records":{},"history":[],"revoked_packages":[]}"#,
        )
        .unwrap();
        assert!(matches!(
            TrustStore::load(path),
            Err(TrustStoreError::Corrupt)
        ));
    }

    fn package(path: &std::path::Path, executable_hash: Sha256Hash) -> ActivePackage {
        let manifest = parse_manifest_v1(
            include_str!("../../../specs/examples/valid/tool-package-manifest-v1.toml"),
            star_contracts::manifest::ManifestSource::User,
        )
        .unwrap();
        ActivePackage {
            manifest,
            source: star_contracts::manifest::ManifestSource::User,
            source_hash: Sha256Hash::digest(b"source"),
            source_file_identity: star_contracts::registry::SourceFileIdentity {
                volume_serial: "test".to_owned(),
                file_id: "test".to_owned(),
                size: 6,
                last_write: Utc::now().to_rfc3339(),
            },
            validated_at: Utc::now().to_rfc3339(),
            cache_id: star_contracts::ids::ToolCacheId::new(),
            path: path.join("fake.toml"),
            resolved_executable_hashes: BTreeMap::from([("fake-echo".to_owned(), executable_hash)]),
            resolved_executable_paths: BTreeMap::from([(
                "fake-echo".to_owned(),
                path.join("fake.exe"),
            )]),
            probed_product_versions: BTreeMap::new(),
            probed_interface_versions: BTreeMap::new(),
            probed_capabilities: BTreeMap::new(),
            location_config_revision: None,
            fixed_working_directory_hashes: BTreeMap::new(),
            resources: ManifestResources::default(),
            manifest_hash: std::sync::OnceLock::new(),
            semantic_hash: std::sync::OnceLock::new(),
            descriptor_hashes: std::sync::OnceLock::new(),
        }
    }

    #[test]
    // matrix: MCP-R014 MCP-S007 MCP-S015 MCP-S018
    fn full_scope_trust_expires_revokes_and_rejects_identity_or_schema_replacement() {
        let directory =
            std::env::temp_dir().join(format!("star-control-trust-{}", star_ipc::nonce()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("trust.json");
        let mut store = TrustStore::load(path.clone()).unwrap();
        let first = package(&directory, Sha256Hash::digest(b"one"));
        assert_eq!(store.state(&first, Utc::now()), "untrusted");
        let record = store
            .grant(
                &first,
                TrustMode::Exact,
                None,
                serde_json::json!({"kind":"test"}),
                Utc::now(),
            )
            .unwrap();
        let dacl = star_ipc::key_store::file_dacl_sddl(&path).unwrap();
        assert!(dacl.starts_with("D:P"));
        assert_eq!(dacl.matches("(A;").count(), 2);
        assert!(!dacl.contains(";;;WD)"));
        assert!(!dacl.contains(";;;BU)"));
        assert!(!dacl.contains(";;;AU)"));
        assert_eq!(store.state(&first, Utc::now()), "trusted");
        let running = store.authorize(&first, Utc::now()).unwrap();
        assert_eq!(running.trust_id, record.trust_id);
        let mut source_moved = first.clone();
        source_moved.path = directory.join("different-source").join("fake.toml");
        assert_eq!(store.state(&source_moved, Utc::now()), "untrusted");

        let replaced = package(&directory, Sha256Hash::digest(b"two"));
        assert_eq!(store.state(&replaced, Utc::now()), "untrusted");
        let mut schema_changed = first.clone();
        schema_changed
            .resources
            .schema_hashes
            .insert("input.json".to_owned(), Sha256Hash::digest(b"schema"));
        assert_eq!(store.state(&schema_changed, Utc::now()), "untrusted");

        let mut location = first.clone();
        location.manifest.package_id = "user.fake.location".to_owned();
        location.manifest.actions[0].tool_id = "user.fake.location.run".to_owned();
        location.manifest.executables[0].locator_kind =
            star_contracts::manifest::LocatorKind::LocationRef;
        location.manifest.executables[0].path = None;
        location.manifest.executables[0].location_ref = Some("configured-tool".to_owned());
        location.location_config_revision = Some(Sha256Hash::digest(b"config revision one"));
        store
            .grant(
                &location,
                TrustMode::Exact,
                None,
                serde_json::json!({"kind":"test"}),
                Utc::now(),
            )
            .unwrap();
        assert_eq!(store.state(&location, Utc::now()), "trusted");
        let mut changed_config = location.clone();
        changed_config.location_config_revision = Some(Sha256Hash::digest(b"config revision two"));
        assert_eq!(store.state(&changed_config, Utc::now()), "untrusted");

        assert!(
            store
                .revoke("user.fake.echo", "test revoke", Utc::now())
                .unwrap()
        );
        assert_eq!(store.state(&first, Utc::now()), "untrusted");
        assert!(running.permits_already_started_operation());
        drop(store);
        let store = TrustStore::load(path).unwrap();
        assert!(store.is_revoked("user.fake.echo"));
    }
}
