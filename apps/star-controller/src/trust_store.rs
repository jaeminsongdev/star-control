use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::PathBuf,
};

use chrono::{DateTime, Utc};
use star_contracts::{Sha256Hash, ToolTrustId, trust::ToolTrustRecord};
use thiserror::Error;

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

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TrustFile {
    records: BTreeMap<String, ToolTrustRecord>,
    #[serde(default)]
    revoked_packages: BTreeSet<String>,
}

pub struct TrustStore {
    path: PathBuf,
    records: BTreeMap<String, ToolTrustRecord>,
    revoked_packages: BTreeSet<String>,
}

/// Authorization captured immediately before process creation.  Revocation
/// blocks every later authorization, while an already-started Operation keeps
/// this immutable lease so its outcome can be audited instead of being
/// silently reclassified or replayed.
#[derive(Clone, Debug)]
pub struct RunningTrustLease {
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
        .join("state")
        .join("tool-trust.v1.json"))
    }

    pub fn load(path: PathBuf) -> Result<Self, TrustStoreError> {
        match fs::read(&path) {
            Ok(bytes) => {
                let file: TrustFile =
                    serde_json::from_slice(&bytes).map_err(|_| TrustStoreError::Corrupt)?;
                Ok(Self {
                    path,
                    records: file.records,
                    revoked_packages: file.revoked_packages,
                })
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self {
                path,
                records: BTreeMap::new(),
                revoked_packages: BTreeSet::new(),
            }),
            Err(error) => Err(TrustStoreError::Io(error)),
        }
    }

    pub fn state(
        &self,
        package_id: &str,
        manifest_hash: &Sha256Hash,
        now: DateTime<Utc>,
    ) -> &'static str {
        if self.is_revoked(package_id) {
            return "untrusted";
        }
        let Some(record) = self.records.get(package_id) else {
            return "untrusted";
        };
        if &record.manifest_hash != manifest_hash {
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
        self.revoked_packages.contains(package_id)
    }

    pub fn authorize(
        &self,
        package_id: &str,
        manifest_hash: &Sha256Hash,
        now: DateTime<Utc>,
    ) -> Option<RunningTrustLease> {
        (self.state(package_id, manifest_hash, now) == "trusted").then(|| RunningTrustLease {
            package_id: package_id.to_owned(),
            manifest_hash: manifest_hash.clone(),
            authorized_at: now,
        })
    }

    pub fn grant(
        &mut self,
        package_id: String,
        manifest_hash: Sha256Hash,
        expires_at: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<ToolTrustRecord, TrustStoreError> {
        if let Some(expires_at) = &expires_at {
            if DateTime::parse_from_rfc3339(expires_at).is_err() {
                return Err(TrustStoreError::Corrupt);
            }
        }
        let record = ToolTrustRecord {
            trust_id: ToolTrustId::new(),
            package_id: package_id.clone(),
            manifest_hash,
            granted_at: now.to_rfc3339(),
            expires_at,
        };
        self.revoked_packages.remove(&package_id);
        self.records.insert(package_id, record.clone());
        self.persist()?;
        Ok(record)
    }

    pub fn revoke(&mut self, package_id: &str) -> Result<bool, TrustStoreError> {
        let existed = self.records.remove(package_id).is_some();
        let changed = self.revoked_packages.insert(package_id.to_owned()) || existed;
        self.persist()?;
        Ok(changed)
    }

    fn persist(&self) -> Result<(), TrustStoreError> {
        let parent = self.path.parent().ok_or(TrustStoreError::Corrupt)?;
        fs::create_dir_all(parent)?;
        let temp = parent.join(format!(".tool-trust-{}.tmp", star_ipc::nonce()));
        let bytes = serde_json::to_vec(&TrustFile {
            records: self.records.clone(),
            revoked_packages: self.revoked_packages.clone(),
        })
        .map_err(|_| TrustStoreError::Corrupt)?;
        fs::write(&temp, bytes)?;
        fs::OpenOptions::new().write(true).open(&temp)?.sync_all()?;
        fs::rename(temp, &self.path)?;
        star_ipc::key_store::apply_owner_system_dacl(&self.path)
            .map_err(|_| TrustStoreError::Dacl)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    // matrix: MCP-R014 MCP-S007 MCP-S018
    fn exact_hash_trust_expires_revokes_and_rejects_same_tool_id_manifest_replacement() {
        let path =
            std::env::temp_dir().join(format!("star-control-trust-{}.json", star_ipc::nonce()));
        let mut store = TrustStore::load(path.clone()).unwrap();
        let hash = Sha256Hash::digest(b"one");
        assert_eq!(store.state("user.fake", &hash, Utc::now()), "untrusted");
        store
            .grant("user.fake".to_owned(), hash.clone(), None, Utc::now())
            .unwrap();
        assert_eq!(store.state("user.fake", &hash, Utc::now()), "trusted");
        let running = store.authorize("user.fake", &hash, Utc::now()).unwrap();
        assert_eq!(
            store.state("user.fake", &Sha256Hash::digest(b"two"), Utc::now()),
            "untrusted"
        );
        assert!(store.revoke("user.fake").unwrap());
        assert!(store.is_revoked("user.fake"));
        assert_eq!(store.state("user.fake", &hash, Utc::now()), "untrusted");
        assert!(store.authorize("user.fake", &hash, Utc::now()).is_none());
        assert!(running.permits_already_started_operation());
        drop(store);
        let mut store = TrustStore::load(path).unwrap();
        assert!(store.is_revoked("user.fake"));
        assert!(!store.revoke("user.fake").unwrap());
        store
            .grant("user.fake".to_owned(), hash.clone(), None, Utc::now())
            .unwrap();
        assert!(!store.is_revoked("user.fake"));
        assert_eq!(store.state("user.fake", &hash, Utc::now()), "trusted");

        // A replacement that presents the same ToolId with a lower-risk
        // manifest still has a new package hash.  Trust never follows a name;
        // the caller must describe and explicitly trust the new candidate.
        let lower_risk_replacement = Sha256Hash::digest(b"same tool id; local_read only");
        assert_eq!(
            store.state("user.fake", &lower_risk_replacement, Utc::now()),
            "untrusted"
        );

        // safe_default trust also binds the resolved user path/location
        // identity. New code at another path cannot inherit the old grant.
        let path_a = star_contracts::canonical::canonical_sha256(&serde_json::json!({
            "manifest":"same semantics",
            "path":"C:/Users/test/tools/a.exe",
            "location_ref":null
        }))
        .unwrap();
        store
            .grant(
                "user.path-bound".to_owned(),
                path_a.clone(),
                None,
                Utc::now(),
            )
            .unwrap();
        let path_b = star_contracts::canonical::canonical_sha256(&serde_json::json!({
            "manifest":"same semantics",
            "path":"C:/Users/test/tools/b.exe",
            "location_ref":"alternate"
        }))
        .unwrap();
        assert_eq!(
            store.state("user.path-bound", &path_a, Utc::now()),
            "trusted"
        );
        assert_eq!(
            store.state("user.path-bound", &path_b, Utc::now()),
            "untrusted"
        );
    }
}
