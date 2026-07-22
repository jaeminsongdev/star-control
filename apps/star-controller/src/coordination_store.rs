use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use serde::{Deserialize, Serialize};
use star_contracts::{
    development::{ChangeBundle, ChangeBundleHandoff},
    parse_no_duplicate_keys,
};
use star_development::coordination::validate_bundle;
use thiserror::Error;
use windows::{
    Win32::Storage::FileSystem::{REPLACEFILE_WRITE_THROUGH, ReplaceFileW},
    core::{HSTRING, PCWSTR},
};

const STORE_SCHEMA_ID: &str = "star.coordination-store";
const STORE_FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoordinationFile {
    schema_id: String,
    format_version: u32,
    generation: u64,
    bundles_by_goal: BTreeMap<String, ChangeBundle>,
    handoffs_by_goal: BTreeMap<String, ChangeBundleHandoff>,
}

impl Default for CoordinationFile {
    fn default() -> Self {
        Self {
            schema_id: STORE_SCHEMA_ID.to_owned(),
            format_version: STORE_FORMAT_VERSION,
            generation: 0,
            bundles_by_goal: BTreeMap::new(),
            handoffs_by_goal: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Error)]
pub enum CoordinationStoreError {
    #[error("LOCALAPPDATA is unavailable")]
    LocalAppDataUnavailable,
    #[error("coordination record was not found")]
    NotFound,
    #[error("coordination state conflicts with existing identity")]
    Conflict,
    #[error("coordination state is corrupt or unsupported")]
    Corrupt,
    #[error("coordination state I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("coordination state DACL update failed")]
    Dacl,
}

pub struct CoordinationStore {
    path: PathBuf,
    file: CoordinationFile,
}

impl CoordinationStore {
    pub fn default_path() -> Result<PathBuf, CoordinationStoreError> {
        Ok(PathBuf::from(
            std::env::var_os("LOCALAPPDATA")
                .ok_or(CoordinationStoreError::LocalAppDataUnavailable)?,
        )
        .join("Star-Control/state/coordination.v1.json"))
    }

    pub fn load(path: PathBuf) -> Result<Self, CoordinationStoreError> {
        let file = match fs::read(&path) {
            Ok(bytes) => {
                let text =
                    std::str::from_utf8(&bytes).map_err(|_| CoordinationStoreError::Corrupt)?;
                let value =
                    parse_no_duplicate_keys(text).map_err(|_| CoordinationStoreError::Corrupt)?;
                serde_json::from_value(value).map_err(|_| CoordinationStoreError::Corrupt)?
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => CoordinationFile::default(),
            Err(error) => return Err(CoordinationStoreError::Io(error)),
        };
        if file.schema_id != STORE_SCHEMA_ID || file.format_version != STORE_FORMAT_VERSION {
            return Err(CoordinationStoreError::Corrupt);
        }
        for (goal_id, bundle) in &file.bundles_by_goal {
            if bundle.goal_id.as_str() != goal_id || validate_bundle(bundle).is_err() {
                return Err(CoordinationStoreError::Corrupt);
            }
        }
        for (goal_id, handoff) in &file.handoffs_by_goal {
            let Some(bundle) = file.bundles_by_goal.get(goal_id) else {
                return Err(CoordinationStoreError::Corrupt);
            };
            if handoff.bundle_id != bundle.bundle_id
                || handoff.bundle_revision != bundle.revision
                || handoff.bundle_fingerprint != bundle.bundle_fingerprint
            {
                return Err(CoordinationStoreError::Corrupt);
            }
        }
        Ok(Self { path, file })
    }

    pub fn save(
        &mut self,
        bundle: ChangeBundle,
        handoff: ChangeBundleHandoff,
    ) -> Result<(), CoordinationStoreError> {
        validate_bundle(&bundle).map_err(|_| CoordinationStoreError::Conflict)?;
        if handoff.bundle_id != bundle.bundle_id
            || handoff.bundle_revision != bundle.revision
            || handoff.bundle_fingerprint != bundle.bundle_fingerprint
        {
            return Err(CoordinationStoreError::Conflict);
        }
        let goal_id = bundle.goal_id.to_string();
        if let Some(existing) = self.file.bundles_by_goal.get(&goal_id)
            && (bundle.revision < existing.revision
                || bundle.revision == existing.revision && bundle != *existing)
        {
            return Err(CoordinationStoreError::Conflict);
        }
        self.file.bundles_by_goal.insert(goal_id.clone(), bundle);
        self.file.handoffs_by_goal.insert(goal_id, handoff);
        self.file.generation = self.file.generation.saturating_add(1);
        let bytes =
            serde_json::to_vec_pretty(&self.file).map_err(|_| CoordinationStoreError::Corrupt)?;
        write_private_atomic(&self.path, &bytes)
    }

    pub fn merge_status(&self, goal_id: &str) -> Result<ChangeBundle, CoordinationStoreError> {
        self.file
            .bundles_by_goal
            .get(goal_id)
            .cloned()
            .ok_or(CoordinationStoreError::NotFound)
    }

    pub fn handoff(&self, goal_id: &str) -> Result<ChangeBundleHandoff, CoordinationStoreError> {
        self.file
            .handoffs_by_goal
            .get(goal_id)
            .cloned()
            .ok_or(CoordinationStoreError::NotFound)
    }
}

static DEFAULT_COORDINATION_STORE: OnceLock<Mutex<Option<CoordinationStore>>> = OnceLock::new();

pub fn with_default_coordination_store<T>(
    operation: impl FnOnce(&mut CoordinationStore) -> Result<T, CoordinationStoreError>,
) -> Result<T, CoordinationStoreError> {
    let cell = DEFAULT_COORDINATION_STORE.get_or_init(|| Mutex::new(None));
    let mut slot = cell.lock().map_err(|_| CoordinationStoreError::Corrupt)?;
    if slot.is_none() {
        *slot = Some(CoordinationStore::load(CoordinationStore::default_path()?)?);
    }
    operation(slot.as_mut().ok_or(CoordinationStoreError::Corrupt)?)
}

fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<(), CoordinationStoreError> {
    let parent = path.parent().ok_or(CoordinationStoreError::Corrupt)?;
    fs::create_dir_all(parent)?;
    star_ipc::key_store::apply_owner_system_dacl(parent)
        .map_err(|_| CoordinationStoreError::Dacl)?;
    let temporary = parent.join(format!(".coordination-{}.tmp", star_ipc::nonce()));
    fs::write(&temporary, bytes)?;
    let file = fs::OpenOptions::new().write(true).open(&temporary)?;
    file.sync_all()?;
    drop(file);
    star_ipc::key_store::apply_owner_system_dacl(&temporary)
        .map_err(|_| CoordinationStoreError::Dacl)?;
    if path.exists() {
        let target = HSTRING::from(path.as_os_str().to_string_lossy().as_ref());
        let replacement = HSTRING::from(temporary.as_os_str().to_string_lossy().as_ref());
        unsafe {
            ReplaceFileW(
                &target,
                &replacement,
                PCWSTR::null(),
                REPLACEFILE_WRITE_THROUGH,
                None,
                None,
            )
        }
        .map_err(|_| CoordinationStoreError::Io(io::Error::last_os_error()))?;
    } else {
        fs::rename(&temporary, path)?;
    }
    star_ipc::key_store::apply_owner_system_dacl(path).map_err(|_| CoordinationStoreError::Dacl)
}

#[cfg(test)]
mod tests {
    use star_contracts::{GoalId, ProjectId, Sha256Hash, development::BundleDependency};
    use star_development::coordination::{
        CoordinationPort, ParticipantDraft, PublishObservation, build_handoff,
        create_change_bundle, run_merge_queue,
    };

    use super::*;

    struct LocalPort;

    impl CoordinationPort for LocalPort {
        fn prepare_owned_worktree(
            &mut self,
            _participant: &star_contracts::development::ChangeBundleParticipant,
        ) -> Result<(), star_development::DevelopmentError> {
            Ok(())
        }

        fn merge_local(
            &mut self,
            _participant: &star_contracts::development::ChangeBundleParticipant,
        ) -> Result<String, star_development::DevelopmentError> {
            Ok("a".repeat(40))
        }

        fn publish_remote(&mut self, _bundle: &ChangeBundle) -> PublishObservation {
            PublishObservation::Verified(Sha256Hash::digest(b"remote"))
        }

        fn reconcile_remote(&mut self, _bundle: &ChangeBundle) -> PublishObservation {
            panic!("verified publish must not reconcile")
        }
    }

    #[test]
    fn bundle_and_handoff_persist_atomically_by_goal() {
        let goal_id = GoalId::new();
        let bundle = create_change_bundle(
            "store-fixture",
            goal_id.clone(),
            vec![ParticipantDraft {
                participant_id: "core".to_owned(),
                project_id: ProjectId::new(),
                checkout_revision: "b".repeat(40),
                patch_fingerprint: Sha256Hash::digest(b"patch"),
                gate_fingerprint: Sha256Hash::digest(b"gate"),
            }],
            Vec::<BundleDependency>::new(),
        )
        .unwrap();
        let bundle = run_merge_queue(bundle, true, &mut LocalPort).unwrap();
        let handoff = build_handoff(&bundle).unwrap();
        let path = std::env::temp_dir().join(format!(
            "star-coordination-store-{}.json",
            star_ipc::nonce()
        ));
        let mut store = CoordinationStore::load(path.clone()).unwrap();
        store.save(bundle.clone(), handoff.clone()).unwrap();
        let loaded = CoordinationStore::load(path).unwrap();
        assert_eq!(loaded.merge_status(goal_id.as_str()).unwrap(), bundle);
        assert_eq!(loaded.handoff(goal_id.as_str()).unwrap(), handoff);
    }
}
