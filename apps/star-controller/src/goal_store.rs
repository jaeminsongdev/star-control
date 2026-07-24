use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use serde::{Deserialize, Serialize};
use star_contracts::{
    GoalId, RunId, Sha256Hash, canonical_sha256,
    config_v1::{ConfigLayerV1, ConfigOverrideV1, ConfigSourceKindV1, ConfigSourceRefV1},
    orchestration::{
        GOAL_RECORD_SCHEMA_ID, GOAL_RECORD_SCHEMA_VERSION, GoalPlanItem, GoalPlanItemStatus,
        GoalQuestion, GoalRecord, GoalRunState, GoalRunStatus, GoalStatus, goal_timestamp_now,
    },
    parse_no_duplicate_keys,
};
use thiserror::Error;
use windows::{
    Win32::Storage::FileSystem::{REPLACEFILE_WRITE_THROUGH, ReplaceFileW},
    core::{HSTRING, PCWSTR},
};

const STORE_SCHEMA_ID: &str = "star.goal-store";
const STORE_FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GoalStoreFile {
    schema_id: String,
    format_version: u32,
    generation: u64,
    goals: BTreeMap<String, GoalRecord>,
    idempotency: BTreeMap<String, GoalStartReplay>,
    #[serde(default)]
    goal_configs: BTreeMap<String, GoalConfigRecord>,
}

impl Default for GoalStoreFile {
    fn default() -> Self {
        Self {
            schema_id: STORE_SCHEMA_ID.to_owned(),
            format_version: STORE_FORMAT_VERSION,
            generation: 0,
            goals: BTreeMap::new(),
            idempotency: BTreeMap::new(),
            goal_configs: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GoalStartReplay {
    goal_id: GoalId,
    input_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GoalConfigRecord {
    goal_id: GoalId,
    overrides: Vec<ConfigOverrideV1>,
    config_fingerprint: Sha256Hash,
}

impl GoalConfigRecord {
    fn seal(goal_id: GoalId, mut overrides: Vec<ConfigOverrideV1>) -> Result<Self, GoalStoreError> {
        overrides.sort_by(|left, right| left.key.cmp(&right.key));
        if overrides.is_empty()
            || overrides.len() > 128
            || overrides.windows(2).any(|pair| pair[0].key == pair[1].key)
            || serde_json::to_value(&overrides)
                .ok()
                .and_then(|value| star_contracts::canonical::jcs_bytes(&value).ok())
                .map(|bytes| bytes.len() > 64 * 1024)
                .unwrap_or(true)
        {
            return Err(GoalStoreError::Invalid);
        }
        let mut record = Self {
            goal_id,
            overrides,
            config_fingerprint: Sha256Hash::digest(b"star.goal-config.pending"),
        };
        record.config_fingerprint = record.expected_fingerprint()?;
        Ok(record)
    }

    fn validate(&self) -> Result<(), GoalStoreError> {
        if self.overrides.is_empty()
            || self.overrides.len() > 128
            || self
                .overrides
                .windows(2)
                .any(|pair| pair[0].key >= pair[1].key)
            || serde_json::to_value(&self.overrides)
                .ok()
                .and_then(|value| star_contracts::canonical::jcs_bytes(&value).ok())
                .map(|bytes| bytes.len() > 64 * 1024)
                .unwrap_or(true)
            || self.expected_fingerprint()? != self.config_fingerprint
        {
            return Err(GoalStoreError::Corrupt);
        }
        Ok(())
    }

    fn expected_fingerprint(&self) -> Result<Sha256Hash, GoalStoreError> {
        canonical_sha256(&serde_json::json!({
            "domain":"star.goal-config",
            "version":1,
            "goal_id":self.goal_id,
            "overrides":self.overrides,
        }))
        .map_err(|_| GoalStoreError::Invalid)
    }
}

#[derive(Clone, Debug)]
pub struct GoalStartRequest {
    pub objective: String,
    pub project_key: Option<String>,
    pub question: Option<(String, String)>,
    pub idempotency_key: String,
    pub config_overrides: Vec<ConfigOverrideV1>,
}

#[derive(Debug, Error)]
pub enum GoalStoreError {
    #[error("LOCALAPPDATA is unavailable")]
    LocalAppDataUnavailable,
    #[error("goal state input is invalid")]
    Invalid,
    #[error("goal was not found")]
    NotFound,
    #[error("goal revision changed")]
    RevisionConflict,
    #[error("goal lifecycle transition is invalid")]
    Lifecycle,
    #[error("idempotency key was reused for different input")]
    IdempotencyConflict,
    #[error("goal state is corrupt or from an unsupported version")]
    Corrupt,
    #[error("goal state I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("goal state DACL update failed")]
    Dacl,
}

pub struct GoalStore {
    path: PathBuf,
    file: GoalStoreFile,
}

impl GoalStore {
    pub fn default_path() -> Result<PathBuf, GoalStoreError> {
        Ok(PathBuf::from(
            std::env::var_os("LOCALAPPDATA").ok_or(GoalStoreError::LocalAppDataUnavailable)?,
        )
        .join("Star-Control/state/goals.v1.json"))
    }

    pub fn load(path: PathBuf) -> Result<Self, GoalStoreError> {
        let file = match fs::read(&path) {
            Ok(bytes) => {
                let text = std::str::from_utf8(&bytes).map_err(|_| GoalStoreError::Corrupt)?;
                let value = parse_no_duplicate_keys(text).map_err(|_| GoalStoreError::Corrupt)?;
                serde_json::from_value(value).map_err(|_| GoalStoreError::Corrupt)?
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => GoalStoreFile::default(),
            Err(error) => return Err(GoalStoreError::Io(error)),
        };
        if file.schema_id != STORE_SCHEMA_ID || file.format_version != STORE_FORMAT_VERSION {
            return Err(GoalStoreError::Corrupt);
        }
        if file
            .goals
            .iter()
            .any(|(key, goal)| key != goal.goal_id.as_str() || goal.validate().is_err())
            || file
                .idempotency
                .values()
                .any(|replay| !file.goals.contains_key(replay.goal_id.as_str()))
            || file.goal_configs.iter().any(|(key, config)| {
                key != config.goal_id.as_str()
                    || !file.goals.contains_key(key)
                    || config.validate().is_err()
            })
        {
            return Err(GoalStoreError::Corrupt);
        }
        Ok(Self { path, file })
    }

    pub fn start(&mut self, request: GoalStartRequest) -> Result<GoalRecord, GoalStoreError> {
        let objective = request.objective.trim().to_owned();
        let project_key = request
            .project_key
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if objective.is_empty()
            || objective.len() > 4_096
            || request.idempotency_key.trim().is_empty()
            || request.idempotency_key.len() > 256
        {
            return Err(GoalStoreError::Invalid);
        }
        let question = request.question.map(|(question_id, prompt)| GoalQuestion {
            question_id: question_id.trim().to_owned(),
            prompt: prompt.trim().to_owned(),
            answer: None,
        });
        let input_fingerprint_payload = if request.config_overrides.is_empty() {
            serde_json::json!({
                "objective": objective,
                "project_key": project_key,
                "question": question,
            })
        } else {
            serde_json::json!({
                "domain":"star.goal-start-input",
                "version":2,
                "objective": objective,
                "project_key": project_key,
                "question": question,
                "config_overrides": request.config_overrides,
            })
        };
        let input_fingerprint =
            canonical_sha256(&input_fingerprint_payload).map_err(|_| GoalStoreError::Invalid)?;
        if let Some(replay) = self.file.idempotency.get(&request.idempotency_key) {
            if replay.input_fingerprint != input_fingerprint {
                return Err(GoalStoreError::IdempotencyConflict);
            }
            return self
                .file
                .goals
                .get(replay.goal_id.as_str())
                .cloned()
                .ok_or(GoalStoreError::Corrupt);
        }
        let timestamp = goal_timestamp_now();
        let goal_id = GoalId::new();
        let goal = GoalRecord {
            schema_id: GOAL_RECORD_SCHEMA_ID.to_owned(),
            schema_version: GOAL_RECORD_SCHEMA_VERSION,
            goal_id: goal_id.clone(),
            revision: 1,
            objective,
            project_key,
            status: if question.is_some() {
                GoalStatus::WaitingQuestion
            } else {
                GoalStatus::Active
            },
            plan_revision: 0,
            plan_items: Vec::new(),
            pending_question: question,
            run: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            content_fingerprint: Sha256Hash::digest(b"unsealed"),
        }
        .seal()
        .map_err(|_| GoalStoreError::Invalid)?;
        self.file.goals.insert(goal_id.to_string(), goal.clone());
        if !request.config_overrides.is_empty() {
            let config = GoalConfigRecord::seal(goal_id.clone(), request.config_overrides)?;
            self.file.goal_configs.insert(goal_id.to_string(), config);
        }
        self.file.idempotency.insert(
            request.idempotency_key,
            GoalStartReplay {
                goal_id,
                input_fingerprint,
            },
        );
        self.commit()?;
        Ok(goal)
    }

    pub fn get(&self, goal_id: &str) -> Result<GoalRecord, GoalStoreError> {
        self.file
            .goals
            .get(goal_id)
            .cloned()
            .ok_or(GoalStoreError::NotFound)
    }

    pub fn config_layer(&self, goal_id: &str) -> Result<Option<ConfigLayerV1>, GoalStoreError> {
        let goal = self
            .file
            .goals
            .get(goal_id)
            .ok_or(GoalStoreError::NotFound)?;
        Ok(self
            .file
            .goal_configs
            .get(goal_id)
            .map(|record| ConfigLayerV1 {
                source: ConfigSourceRefV1 {
                    source_kind: ConfigSourceKindV1::Goal,
                    source_id: format!("goal:{}", goal.goal_id),
                    source_fingerprint: record.config_fingerprint.clone(),
                },
                overrides: record.overrides.clone(),
            }))
    }

    pub fn answer(
        &mut self,
        goal_id: &str,
        expected_revision: u64,
        question_id: &str,
        answer: &str,
    ) -> Result<GoalRecord, GoalStoreError> {
        if answer.trim().is_empty() || answer.len() > 16_384 {
            return Err(GoalStoreError::Invalid);
        }
        self.mutate(goal_id, expected_revision, |goal| {
            if goal.status != GoalStatus::WaitingQuestion {
                return Err(GoalStoreError::Lifecycle);
            }
            let question = goal
                .pending_question
                .as_mut()
                .filter(|question| question.question_id == question_id && question.answer.is_none())
                .ok_or(GoalStoreError::Lifecycle)?;
            question.answer = Some(answer.trim().to_owned());
            goal.status = GoalStatus::Active;
            Ok(())
        })
    }

    pub fn update_plan(
        &mut self,
        goal_id: &str,
        expected_revision: u64,
        mut items: Vec<GoalPlanItem>,
    ) -> Result<GoalRecord, GoalStoreError> {
        if items.is_empty() || items.len() > 256 {
            return Err(GoalStoreError::Invalid);
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut in_progress = 0;
        for item in &mut items {
            item.item_id = item.item_id.trim().to_owned();
            item.step = item.step.trim().to_owned();
            if item.item_id.is_empty()
                || item.step.is_empty()
                || item.step.len() > 4_096
                || !ids.insert(item.item_id.clone())
            {
                return Err(GoalStoreError::Invalid);
            }
            if item.status == GoalPlanItemStatus::InProgress {
                in_progress += 1;
            }
        }
        if in_progress > 1 {
            return Err(GoalStoreError::Invalid);
        }
        self.mutate(goal_id, expected_revision, |goal| {
            if matches!(
                goal.status,
                GoalStatus::WaitingQuestion | GoalStatus::Completed | GoalStatus::Cancelled
            ) || goal
                .run
                .as_ref()
                .is_some_and(|run| run.status == GoalRunStatus::Running)
            {
                return Err(GoalStoreError::Lifecycle);
            }
            goal.plan_revision = goal.plan_revision.saturating_add(1);
            goal.plan_items = items;
            if goal.status == GoalStatus::Blocked
                && !goal
                    .plan_items
                    .iter()
                    .any(|item| item.status == GoalPlanItemStatus::Blocked)
            {
                goal.status = GoalStatus::Active;
            }
            Ok(())
        })
    }

    pub fn continue_run(
        &mut self,
        goal_id: &str,
        expected_revision: u64,
    ) -> Result<GoalRecord, GoalStoreError> {
        self.mutate(goal_id, expected_revision, |goal| {
            if goal.status != GoalStatus::Active
                || goal.plan_items.is_empty()
                || goal
                    .plan_items
                    .iter()
                    .any(|item| item.status == GoalPlanItemStatus::Blocked)
            {
                return Err(GoalStoreError::Lifecycle);
            }
            let timestamp = goal_timestamp_now();
            goal.run = Some(match goal.run.take() {
                Some(mut run) if run.status == GoalRunStatus::Running => {
                    run.attempt = run.attempt.saturating_add(1);
                    run.continued_at = timestamp;
                    run
                }
                _ => GoalRunState {
                    run_id: RunId::new(),
                    attempt: 1,
                    status: GoalRunStatus::Running,
                    continued_at: timestamp,
                },
            });
            Ok(())
        })
    }

    pub fn pause(
        &mut self,
        goal_id: &str,
        expected_revision: u64,
    ) -> Result<GoalRecord, GoalStoreError> {
        if self.get(goal_id)?.status == GoalStatus::Paused {
            return self.get(goal_id);
        }
        self.mutate(goal_id, expected_revision, |goal| {
            if matches!(goal.status, GoalStatus::Completed | GoalStatus::Cancelled) {
                return Err(GoalStoreError::Lifecycle);
            }
            goal.status = GoalStatus::Paused;
            Ok(())
        })
    }

    pub fn resume(
        &mut self,
        goal_id: &str,
        expected_revision: u64,
    ) -> Result<GoalRecord, GoalStoreError> {
        let current = self.get(goal_id)?;
        if current.status != GoalStatus::Paused {
            if matches!(
                current.status,
                GoalStatus::Active | GoalStatus::WaitingQuestion
            ) {
                return Ok(current);
            }
            return Err(GoalStoreError::Lifecycle);
        }
        self.mutate(goal_id, expected_revision, |goal| {
            goal.status = if goal
                .pending_question
                .as_ref()
                .is_some_and(|question| question.answer.is_none())
            {
                GoalStatus::WaitingQuestion
            } else {
                GoalStatus::Active
            };
            Ok(())
        })
    }

    pub fn cancel(
        &mut self,
        goal_id: &str,
        expected_revision: u64,
    ) -> Result<GoalRecord, GoalStoreError> {
        if self.get(goal_id)?.status == GoalStatus::Cancelled {
            return self.get(goal_id);
        }
        self.mutate(goal_id, expected_revision, |goal| {
            if goal.status == GoalStatus::Completed {
                return Err(GoalStoreError::Lifecycle);
            }
            goal.status = GoalStatus::Cancelled;
            if let Some(run) = goal.run.as_mut() {
                run.status = GoalRunStatus::Cancelled;
            }
            Ok(())
        })
    }

    fn mutate(
        &mut self,
        goal_id: &str,
        expected_revision: u64,
        change: impl FnOnce(&mut GoalRecord) -> Result<(), GoalStoreError>,
    ) -> Result<GoalRecord, GoalStoreError> {
        let mut goal = self.get(goal_id)?;
        if goal.revision != expected_revision {
            return Err(GoalStoreError::RevisionConflict);
        }
        change(&mut goal)?;
        goal.revision = goal.revision.saturating_add(1);
        goal.updated_at = goal_timestamp_now();
        goal = goal.seal().map_err(|_| GoalStoreError::Invalid)?;
        self.file.goals.insert(goal_id.to_owned(), goal.clone());
        self.commit()?;
        Ok(goal)
    }

    fn commit(&mut self) -> Result<(), GoalStoreError> {
        self.file.generation = self.file.generation.saturating_add(1);
        let bytes = serde_json::to_vec_pretty(&self.file).map_err(|_| GoalStoreError::Corrupt)?;
        write_private_atomic(&self.path, &bytes)
    }
}

static DEFAULT_GOAL_STORE: OnceLock<Mutex<Option<GoalStore>>> = OnceLock::new();

pub fn with_default_goal_store<T>(
    operation: impl FnOnce(&mut GoalStore) -> Result<T, GoalStoreError>,
) -> Result<T, GoalStoreError> {
    let cell = DEFAULT_GOAL_STORE.get_or_init(|| Mutex::new(None));
    let mut slot = cell.lock().map_err(|_| GoalStoreError::Corrupt)?;
    if slot.is_none() {
        *slot = Some(GoalStore::load(GoalStore::default_path()?)?);
    }
    operation(slot.as_mut().ok_or(GoalStoreError::Corrupt)?)
}

fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<(), GoalStoreError> {
    let parent = path.parent().ok_or(GoalStoreError::Corrupt)?;
    fs::create_dir_all(parent)?;
    star_ipc::key_store::apply_owner_system_dacl(parent).map_err(|_| GoalStoreError::Dacl)?;
    let temporary = parent.join(format!(".goals-{}.tmp", star_ipc::nonce()));
    fs::write(&temporary, bytes)?;
    let file = fs::OpenOptions::new().write(true).open(&temporary)?;
    file.sync_all()?;
    drop(file);
    star_ipc::key_store::apply_owner_system_dacl(&temporary).map_err(|_| GoalStoreError::Dacl)?;
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
        .map_err(|_| GoalStoreError::Io(io::Error::last_os_error()))?;
    } else {
        fs::rename(&temporary, path)?;
    }
    star_ipc::key_store::apply_owner_system_dacl(path).map_err(|_| GoalStoreError::Dacl)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(name: &str) -> GoalStore {
        GoalStore::load(
            std::env::temp_dir().join(format!("star-goal-store-{name}-{}.json", star_ipc::nonce())),
        )
        .unwrap()
    }

    fn start(store: &mut GoalStore, key: &str) -> GoalRecord {
        store
            .start(GoalStartRequest {
                objective: "ship the release".to_owned(),
                project_key: Some("star-control".to_owned()),
                question: None,
                idempotency_key: key.to_owned(),
                config_overrides: Vec::new(),
            })
            .unwrap()
    }

    #[test]
    fn start_is_replay_safe_and_conflict_detecting() {
        let mut store = store("start");
        let first = start(&mut store, "idem-1");
        let replay = start(&mut store, "idem-1");
        assert_eq!(first, replay);
        let error = store
            .start(GoalStartRequest {
                objective: "different".to_owned(),
                project_key: None,
                question: None,
                idempotency_key: "idem-1".to_owned(),
                config_overrides: Vec::new(),
            })
            .unwrap_err();
        assert!(matches!(error, GoalStoreError::IdempotencyConflict));
    }

    #[test]
    fn goal_config_is_fingerprinted_persistent_and_idempotency_bound() {
        let mut store = store("config");
        let path = store.path.clone();
        let config_overrides = vec![ConfigOverrideV1 {
            key: "scan.max_files".to_owned(),
            value: star_contracts::config_v1::ConfigValueV1::Integer(250),
        }];
        let goal = store
            .start(GoalStartRequest {
                objective: "bounded scan".to_owned(),
                project_key: Some("star-control".to_owned()),
                question: None,
                idempotency_key: "goal-config".to_owned(),
                config_overrides: config_overrides.clone(),
            })
            .unwrap();
        let layer = store.config_layer(goal.goal_id.as_str()).unwrap().unwrap();
        assert_eq!(layer.source.source_kind, ConfigSourceKindV1::Goal);
        assert_eq!(layer.overrides, config_overrides);
        drop(store);

        let mut reopened = GoalStore::load(path).unwrap();
        assert_eq!(
            reopened
                .config_layer(goal.goal_id.as_str())
                .unwrap()
                .unwrap()
                .overrides,
            config_overrides
        );
        let conflict = reopened
            .start(GoalStartRequest {
                objective: "bounded scan".to_owned(),
                project_key: Some("star-control".to_owned()),
                question: None,
                idempotency_key: "goal-config".to_owned(),
                config_overrides: vec![ConfigOverrideV1 {
                    key: "scan.max_files".to_owned(),
                    value: star_contracts::config_v1::ConfigValueV1::Integer(125),
                }],
            })
            .unwrap_err();
        assert!(matches!(conflict, GoalStoreError::IdempotencyConflict));
    }

    #[test]
    fn question_plan_run_and_lifecycle_are_revision_guarded() {
        let mut store = store("lifecycle");
        let mut goal = store
            .start(GoalStartRequest {
                objective: "release".to_owned(),
                project_key: None,
                question: Some(("q1".to_owned(), "Proceed?".to_owned())),
                idempotency_key: "idem-q".to_owned(),
                config_overrides: Vec::new(),
            })
            .unwrap();
        assert_eq!(goal.status, GoalStatus::WaitingQuestion);
        goal = store
            .answer(goal.goal_id.as_str(), goal.revision, "q1", "yes")
            .unwrap();
        goal = store
            .update_plan(
                goal.goal_id.as_str(),
                goal.revision,
                vec![GoalPlanItem {
                    item_id: "p1".to_owned(),
                    step: "validate".to_owned(),
                    status: GoalPlanItemStatus::InProgress,
                }],
            )
            .unwrap();
        assert!(matches!(
            store.continue_run(goal.goal_id.as_str(), goal.revision - 1),
            Err(GoalStoreError::RevisionConflict)
        ));
        goal = store
            .continue_run(goal.goal_id.as_str(), goal.revision)
            .unwrap();
        assert_eq!(goal.run.as_ref().unwrap().attempt, 1);
        goal = store.pause(goal.goal_id.as_str(), goal.revision).unwrap();
        let replay = store
            .pause(goal.goal_id.as_str(), goal.revision - 1)
            .unwrap();
        assert_eq!(replay.revision, goal.revision);
        goal = store.resume(goal.goal_id.as_str(), goal.revision).unwrap();
        goal = store.cancel(goal.goal_id.as_str(), goal.revision).unwrap();
        assert_eq!(goal.status, GoalStatus::Cancelled);
        assert_eq!(goal.run.unwrap().status, GoalRunStatus::Cancelled);
    }

    #[test]
    fn persisted_state_reloads_and_future_version_is_rejected() {
        let path =
            std::env::temp_dir().join(format!("star-goal-store-reload-{}.json", star_ipc::nonce()));
        let mut store = GoalStore::load(path.clone()).unwrap();
        let goal = start(&mut store, "reload");
        let reloaded = GoalStore::load(path.clone()).unwrap();
        assert_eq!(reloaded.get(goal.goal_id.as_str()).unwrap(), goal);
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value["format_version"] = serde_json::json!(2);
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            GoalStore::load(path),
            Err(GoalStoreError::Corrupt)
        ));
    }
}
