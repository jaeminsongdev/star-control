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
    stage::{
        STAGE_GRAPH_SCHEMA_ID, StageGraphV1, StageResultOutcomeV1, StageResultV1, StageStateV1,
    },
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
    #[serde(default)]
    stage_graphs: BTreeMap<String, StageGraphV1>,
    #[serde(default)]
    stage_graph_history: BTreeMap<String, BTreeMap<u64, StageGraphV1>>,
    #[serde(default)]
    stage_results: BTreeMap<String, StageResultV1>,
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
            stage_graphs: BTreeMap::new(),
            stage_graph_history: BTreeMap::new(),
            stage_results: BTreeMap::new(),
        }
    }
}

fn graph_result_refs_are_valid(file: &GoalStoreFile, graph: &StageGraphV1) -> bool {
    graph.stages.iter().all(|stage| {
        let Some(reference) = stage.result_ref.as_ref() else {
            return true;
        };
        file.stage_results
            .get(&reference.document_id)
            .is_some_and(|result| {
                reference == &result.reference()
                    && result.goal_id == graph.goal_id
                    && result.stage_ref.document_id == stage.stage_id.as_str()
                    && stage.state == result.terminal_state()
            })
    })
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
        let mut file = match fs::read(&path) {
            Ok(bytes) => {
                let text = std::str::from_utf8(&bytes).map_err(|_| GoalStoreError::Corrupt)?;
                let value = parse_no_duplicate_keys(text).map_err(|_| GoalStoreError::Corrupt)?;
                serde_json::from_value(value).map_err(|_| GoalStoreError::Corrupt)?
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => GoalStoreFile::default(),
            Err(error) => return Err(GoalStoreError::Io(error)),
        };
        // v1 stores written before StageGraph history was introduced contain
        // only the current graph. Preserve that graph as the first locally
        // available immutable revision instead of fabricating missing history.
        for (graph_id, graph) in &file.stage_graphs {
            file.stage_graph_history
                .entry(graph_id.clone())
                .or_default()
                .entry(graph.plan_revision)
                .or_insert_with(|| graph.clone());
        }
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
            || file.stage_graphs.iter().any(|(key, graph)| {
                key != graph.stage_graph_id.as_str()
                    || !file.goals.contains_key(graph.goal_id.as_str())
                    || graph.verify().is_err()
                    || !graph_result_refs_are_valid(&file, graph)
            })
            || file.stage_graph_history.len() != file.stage_graphs.len()
            || file.stage_graph_history.iter().any(|(key, history)| {
                let Some(current) = file.stage_graphs.get(key) else {
                    return true;
                };
                if history.is_empty()
                    || history.last_key_value().map(|(_, graph)| graph) != Some(current)
                {
                    return true;
                }
                let mut previous: Option<&StageGraphV1> = None;
                for (revision, graph) in history {
                    if *revision != graph.plan_revision
                        || key != graph.stage_graph_id.as_str()
                        || !file.goals.contains_key(graph.goal_id.as_str())
                        || graph.verify().is_err()
                        || !graph_result_refs_are_valid(&file, graph)
                    {
                        return true;
                    }
                    if let Some(previous) = previous
                        && (graph.plan_revision != previous.plan_revision.saturating_add(1)
                            || graph.previous_graph_fingerprint.as_ref()
                                != Some(&previous.graph_fingerprint))
                    {
                        return true;
                    }
                    previous = Some(graph);
                }
                false
            })
            || file.stage_results.iter().any(|(key, result)| {
                if key != result.stage_result_id.as_str() || result.verify().is_err() {
                    return true;
                }
                let Some(history) = file
                    .stage_graph_history
                    .get(&result.stage_graph_ref.document_id)
                else {
                    return true;
                };
                let Some(graph) = history.get(&result.stage_graph_ref.revision) else {
                    return true;
                };
                let Some(stage) = graph
                    .stages
                    .iter()
                    .find(|stage| stage.stage_id.as_str() == result.stage_ref.document_id)
                else {
                    return true;
                };
                result.verify_against(graph, stage).is_err()
                    || !history.values().any(|candidate_graph| {
                        candidate_graph.stages.iter().any(|candidate_stage| {
                            candidate_stage.stage_id.as_str() == result.stage_ref.document_id
                                && candidate_stage.result_ref.as_ref() == Some(&result.reference())
                                && candidate_stage.state == result.terminal_state()
                        })
                    })
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
            completion_evidence_ref: None,
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

    pub fn stage_graph(&self, stage_graph_id: &str) -> Result<StageGraphV1, GoalStoreError> {
        self.file
            .stage_graphs
            .get(stage_graph_id)
            .cloned()
            .ok_or(GoalStoreError::NotFound)
    }

    pub fn stage_graph_revision(
        &self,
        stage_graph_id: &str,
        plan_revision: u64,
    ) -> Result<StageGraphV1, GoalStoreError> {
        self.file
            .stage_graph_history
            .get(stage_graph_id)
            .and_then(|history| history.get(&plan_revision))
            .cloned()
            .ok_or(GoalStoreError::NotFound)
    }

    pub fn stage_result(&self, stage_result_id: &str) -> Result<StageResultV1, GoalStoreError> {
        self.file
            .stage_results
            .get(stage_result_id)
            .cloned()
            .ok_or(GoalStoreError::NotFound)
    }

    pub fn publish_stage_graph(
        &mut self,
        graph: StageGraphV1,
        expected_previous_revision: Option<u64>,
    ) -> Result<StageGraphV1, GoalStoreError> {
        graph.verify().map_err(|_| GoalStoreError::Invalid)?;
        self.get(graph.goal_id.as_str())?;
        match self.file.stage_graphs.get(graph.stage_graph_id.as_str()) {
            Some(current) if current == &graph => return Ok(current.clone()),
            Some(current) if expected_previous_revision != Some(current.plan_revision) => {
                return Err(GoalStoreError::RevisionConflict);
            }
            Some(current)
                if graph.plan_revision != current.plan_revision.saturating_add(1)
                    || graph.previous_graph_fingerprint.as_ref()
                        != Some(&current.graph_fingerprint) =>
            {
                return Err(GoalStoreError::RevisionConflict);
            }
            None if expected_previous_revision.is_some() => return Err(GoalStoreError::NotFound),
            None => {}
            Some(_) => {}
        }
        let history = self
            .file
            .stage_graph_history
            .entry(graph.stage_graph_id.to_string())
            .or_default();
        if history
            .get(&graph.plan_revision)
            .is_some_and(|existing| existing != &graph)
        {
            return Err(GoalStoreError::RevisionConflict);
        }
        history.insert(graph.plan_revision, graph.clone());
        self.file
            .stage_graphs
            .insert(graph.stage_graph_id.to_string(), graph.clone());
        self.commit()?;
        Ok(graph)
    }

    pub fn record_stage_result(
        &mut self,
        result: StageResultV1,
        expected_graph_revision: u64,
        expected_stage_revision: u64,
    ) -> Result<(StageResultV1, StageGraphV1), GoalStoreError> {
        result.verify().map_err(|_| GoalStoreError::Invalid)?;
        if result.stage_graph_ref.schema_id != STAGE_GRAPH_SCHEMA_ID {
            return Err(GoalStoreError::Invalid);
        }
        let graph_id = result.stage_graph_ref.document_id.clone();
        if let Some(existing) = self.file.stage_results.get(result.stage_result_id.as_str()) {
            if existing != &result {
                return Err(GoalStoreError::RevisionConflict);
            }
            let latest = self.stage_graph(&graph_id)?;
            if latest.stages.iter().any(|candidate| {
                candidate.stage_id.as_str() == result.stage_ref.document_id
                    && candidate.result_ref.as_ref() == Some(&result.reference())
                    && candidate.state == result.terminal_state()
            }) {
                return Ok((existing.clone(), latest));
            }
            return Err(GoalStoreError::Corrupt);
        }
        let current = self.stage_graph(&graph_id)?;
        if current.plan_revision != expected_graph_revision
            || result.stage_graph_ref.revision != current.plan_revision
            || result.stage_graph_ref.sha256 != current.graph_fingerprint
        {
            return Err(GoalStoreError::RevisionConflict);
        }
        let stage = current
            .stages
            .iter()
            .find(|stage| stage.stage_id.as_str() == result.stage_ref.document_id)
            .cloned()
            .ok_or(GoalStoreError::NotFound)?;
        if stage.revision != expected_stage_revision {
            return Err(GoalStoreError::RevisionConflict);
        }
        result
            .verify_against(&current, &stage)
            .map_err(|_| GoalStoreError::Invalid)?;
        if result.outcome == StageResultOutcomeV1::Completed
            && (!matches!(stage.state, StageStateV1::Ready | StageStateV1::Running)
                || stage.dependencies.iter().any(|dependency| {
                    current
                        .stages
                        .iter()
                        .find(|candidate| &candidate.stage_id == dependency)
                        .is_none_or(|candidate| candidate.state != StageStateV1::Completed)
                }))
        {
            return Err(GoalStoreError::Lifecycle);
        }

        let mut next = current.clone();
        next.plan_revision = next
            .plan_revision
            .checked_add(1)
            .ok_or(GoalStoreError::RevisionConflict)?;
        next.previous_graph_fingerprint = Some(current.graph_fingerprint.clone());
        next.replan_reason = Some(format!("stage_result:{}", result.stage_result_id));
        let next_stage = next
            .stages
            .iter_mut()
            .find(|candidate| candidate.stage_id == stage.stage_id)
            .ok_or(GoalStoreError::Corrupt)?;
        next_stage.revision = next_stage
            .revision
            .checked_add(1)
            .ok_or(GoalStoreError::RevisionConflict)?;
        next_stage.state = result.terminal_state();
        next_stage.result_ref = Some(result.reference());
        next.graph_fingerprint = Sha256Hash::digest(b"unsealed-stage-result-transition");
        next = next.seal().map_err(|_| GoalStoreError::Invalid)?;

        let history = self
            .file
            .stage_graph_history
            .get_mut(&graph_id)
            .ok_or(GoalStoreError::Corrupt)?;
        if history.contains_key(&next.plan_revision) {
            return Err(GoalStoreError::RevisionConflict);
        }
        history.insert(next.plan_revision, next.clone());
        self.file
            .stage_results
            .insert(result.stage_result_id.to_string(), result.clone());
        self.file.stage_graphs.insert(graph_id, next.clone());
        self.commit()?;
        Ok((result, next))
    }

    pub fn complete_goal(
        &mut self,
        goal_id: &str,
        expected_revision: u64,
        stage_graph_id: &str,
        evidence_ref: star_contracts::evidence::DocumentRef,
    ) -> Result<GoalRecord, GoalStoreError> {
        let graph = self.stage_graph(stage_graph_id)?;
        if graph.goal_id.as_str() != goal_id
            || graph.stages.is_empty()
            || graph
                .stages
                .iter()
                .any(|stage| stage.state != StageStateV1::Completed)
        {
            return Err(GoalStoreError::Lifecycle);
        }
        let completion_stage_ids = if let Some(stage_id) = &graph.integration_stage_id {
            vec![stage_id.clone()]
        } else if let Some(stage_id) = graph.critical_path.last() {
            vec![stage_id.clone()]
        } else {
            graph
                .stages
                .iter()
                .map(|stage| stage.stage_id.clone())
                .collect::<Vec<_>>()
        };
        let evidence_is_bound = completion_stage_ids.iter().any(|stage_id| {
            graph
                .stages
                .iter()
                .find(|stage| &stage.stage_id == stage_id)
                .and_then(|stage| stage.result_ref.as_ref())
                .and_then(|reference| {
                    self.file
                        .stage_results
                        .get(&reference.document_id)
                        .map(|result| (reference, result))
                })
                .is_some_and(|(reference, result)| {
                    reference == &result.reference()
                        && result.outcome == StageResultOutcomeV1::Completed
                        && result
                            .project_evidence
                            .iter()
                            .any(|project| project.evidence_bundle_ref == evidence_ref)
                })
        });
        if !evidence_is_bound {
            return Err(GoalStoreError::Invalid);
        }
        let current = self.get(goal_id)?;
        if current.revision != expected_revision
            || current.plan_items.is_empty()
            || current
                .plan_items
                .iter()
                .any(|item| item.status != GoalPlanItemStatus::Completed)
            || current
                .run
                .as_ref()
                .is_none_or(|run| run.status != GoalRunStatus::Running)
        {
            return Err(GoalStoreError::Lifecycle);
        }
        self.mutate(goal_id, expected_revision, |goal| {
            goal.status = GoalStatus::Completed;
            goal.run.as_mut().ok_or(GoalStoreError::Lifecycle)?.status = GoalRunStatus::Completed;
            goal.completion_evidence_ref = Some(evidence_ref);
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
    use chrono::Utc;
    use star_contracts::{
        ProjectId, StageGraphId, StageId, StageResultId,
        evidence::DocumentRef,
        stage::{
            STAGE_CONTRACT_VERSION, STAGE_GRAPH_SCHEMA_ID, STAGE_RESULT_SCHEMA_ID,
            STAGE_SPEC_SCHEMA_ID, StageExecutorKindV1, StageFailurePolicyV1, StageGraphV1,
            StageModeV1, StageProjectEvidenceV1, StageResultOutcomeV1, StageResultV1, StageSpecV1,
            StageStateV1,
        },
    };

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

    fn stage_graph(
        goal_id: &GoalId,
        plan_revision: u64,
        previous: Option<&StageGraphV1>,
    ) -> StageGraphV1 {
        let stage = StageSpecV1 {
            schema_id: STAGE_SPEC_SCHEMA_ID.to_owned(),
            schema_version: STAGE_CONTRACT_VERSION,
            stage_id: StageId::from_stable_bytes(format!("{}:stage", goal_id).as_bytes()),
            revision: plan_revision,
            goal_id: goal_id.clone(),
            task_spec_ref: None,
            scope_revision_ref: None,
            title: "persisted stage".to_owned(),
            objective: format!("persist immutable graph revision {plan_revision}"),
            stage_mode: StageModeV1::Plan,
            executor_kind: StageExecutorKindV1::DeterministicLocal,
            work_profile_id: "project_understanding".to_owned(),
            work_profile_version: "1.1.0".to_owned(),
            work_profile_definition_hash: Some(Sha256Hash::digest(b"profile-definition")),
            profile_catalog_fingerprint: Some(Sha256Hash::digest(b"profile-catalog")),
            profile_resolution_fingerprint: Some(Sha256Hash::digest(b"profile-resolution")),
            project_ids: vec![ProjectId::from_stable_bytes(goal_id.as_str().as_bytes())],
            included_work: vec!["stage_graph_history".to_owned()],
            excluded_work: vec!["external_publish".to_owned()],
            expected_change_scope: vec!["state/stage-graph".to_owned()],
            dependencies: Vec::new(),
            parallel_group: None,
            completion_criteria: vec!["history_reloads".to_owned()],
            failure_policy: StageFailurePolicyV1::Block,
            route_decision_ref: None,
            permission_plan_ref: None,
            validation_plan_ref: None,
            impact_analysis_ref: None,
            change_plan_refs: Vec::new(),
            result_ref: None,
            state: StageStateV1::Ready,
            stage_fingerprint: Sha256Hash::digest(b"unsealed-stage"),
        };
        StageGraphV1 {
            schema_id: STAGE_GRAPH_SCHEMA_ID.to_owned(),
            schema_version: STAGE_CONTRACT_VERSION,
            stage_graph_id: StageGraphId::from_stable_bytes(
                format!("{}:stage-graph", goal_id).as_bytes(),
            ),
            goal_id: goal_id.clone(),
            plan_revision,
            stages: vec![stage],
            edges: Vec::new(),
            parallel_groups: Vec::new(),
            critical_path: Vec::new(),
            integration_stage_id: None,
            previous_graph_fingerprint: previous.map(|graph| graph.graph_fingerprint.clone()),
            replan_reason: previous.map(|_| "test immutable history".to_owned()),
            graph_fingerprint: Sha256Hash::digest(b"unsealed-graph"),
        }
        .seal()
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

    #[test]
    fn paused_goal_resumes_without_losing_the_accepted_plan() {
        let path =
            std::env::temp_dir().join(format!("star-goal-store-resume-{}.json", star_ipc::nonce()));
        let mut store = GoalStore::load(path.clone()).unwrap();
        let mut goal = start(&mut store, "resume-after-restart");
        goal = store
            .update_plan(
                goal.goal_id.as_str(),
                goal.revision,
                vec![GoalPlanItem {
                    item_id: "accepted-plan".to_owned(),
                    step: "preserve this exact plan".to_owned(),
                    status: GoalPlanItemStatus::InProgress,
                }],
            )
            .unwrap();
        goal = store
            .continue_run(goal.goal_id.as_str(), goal.revision)
            .unwrap();
        let paused = store.pause(goal.goal_id.as_str(), goal.revision).unwrap();
        let accepted_plan = paused.plan_items.clone();
        let plan_revision = paused.plan_revision;
        drop(store);

        let mut reopened = GoalStore::load(path).unwrap();
        let resumed = reopened
            .resume(paused.goal_id.as_str(), paused.revision)
            .unwrap();
        assert_eq!(resumed.status, GoalStatus::Active);
        assert_eq!(resumed.plan_revision, plan_revision);
        assert_eq!(resumed.plan_items, accepted_plan);
        assert_eq!(resumed.run.unwrap().status, GoalRunStatus::Running);
    }

    #[test]
    fn stage_graph_revision_history_survives_replan_and_restart() {
        let mut store = store("stage-graph-history");
        let path = store.path.clone();
        let goal = start(&mut store, "stage-graph-history");
        let first = stage_graph(&goal.goal_id, 1, None);
        store.publish_stage_graph(first.clone(), None).unwrap();
        let second = stage_graph(&goal.goal_id, 2, Some(&first));
        store
            .publish_stage_graph(second.clone(), Some(first.plan_revision))
            .unwrap();
        drop(store);

        let reopened = GoalStore::load(path).unwrap();
        assert_eq!(
            reopened
                .stage_graph_revision(first.stage_graph_id.as_str(), 1)
                .unwrap(),
            first
        );
        assert_eq!(
            reopened
                .stage_graph(second.stage_graph_id.as_str())
                .unwrap(),
            second
        );
    }

    #[test]
    fn stage_result_and_goal_completion_are_evidence_bound_and_restart_safe() {
        let mut store = store("stage-result-completion");
        let path = store.path.clone();
        let mut goal = start(&mut store, "stage-result-completion");
        goal = store
            .update_plan(
                goal.goal_id.as_str(),
                goal.revision,
                vec![GoalPlanItem {
                    item_id: "verified-stage".to_owned(),
                    step: "record the evidence-bound stage result".to_owned(),
                    status: GoalPlanItemStatus::Completed,
                }],
            )
            .unwrap();
        goal = store
            .continue_run(goal.goal_id.as_str(), goal.revision)
            .unwrap();
        let graph = stage_graph(&goal.goal_id, 1, None);
        store.publish_stage_graph(graph.clone(), None).unwrap();
        let stage = graph.stages[0].clone();
        let evidence_ref = DocumentRef {
            schema_id: "star.evidence-bundle".to_owned(),
            document_id: "evb_current_stage".to_owned(),
            revision: 1,
            sha256: Sha256Hash::digest(b"current-stage-evidence"),
        };
        let result = StageResultV1 {
            schema_id: STAGE_RESULT_SCHEMA_ID.to_owned(),
            schema_version: STAGE_CONTRACT_VERSION,
            stage_result_id: StageResultId::from_stable_bytes(b"stage-result-completion"),
            revision: 1,
            goal_id: goal.goal_id.clone(),
            stage_graph_ref: DocumentRef {
                schema_id: STAGE_GRAPH_SCHEMA_ID.to_owned(),
                document_id: graph.stage_graph_id.to_string(),
                revision: graph.plan_revision,
                sha256: graph.graph_fingerprint.clone(),
            },
            stage_ref: DocumentRef {
                schema_id: STAGE_SPEC_SCHEMA_ID.to_owned(),
                document_id: stage.stage_id.to_string(),
                revision: stage.revision,
                sha256: stage.stage_fingerprint.clone(),
            },
            outcome: StageResultOutcomeV1::Completed,
            completed_criteria: stage.completion_criteria.clone(),
            project_evidence: vec![StageProjectEvidenceV1 {
                project_id: stage.project_ids[0].clone(),
                evidence_bundle_ref: evidence_ref.clone(),
            }],
            execution_record_ref: None,
            failure_code: None,
            recovery_action: None,
            source_effect_may_have_started: false,
            recorded_at: Utc::now(),
            result_fingerprint: Sha256Hash::digest(b"unsealed-stage-result"),
        }
        .seal()
        .unwrap();
        let (persisted_result, completed_graph) = store
            .record_stage_result(result.clone(), graph.plan_revision, stage.revision)
            .unwrap();
        assert_eq!(persisted_result, result);
        assert_eq!(completed_graph.plan_revision, 2);
        assert_eq!(completed_graph.stages[0].state, StageStateV1::Completed);
        let (replayed_result, replayed_graph) = store
            .record_stage_result(result.clone(), graph.plan_revision, stage.revision)
            .unwrap();
        assert_eq!(replayed_result, result);
        assert_eq!(replayed_graph, completed_graph);

        let goal = store
            .complete_goal(
                goal.goal_id.as_str(),
                goal.revision,
                graph.stage_graph_id.as_str(),
                evidence_ref,
            )
            .unwrap();
        assert_eq!(goal.status, GoalStatus::Completed);
        assert_eq!(goal.run.as_ref().unwrap().status, GoalRunStatus::Completed);
        drop(store);

        let mut reopened = GoalStore::load(path.clone()).unwrap();
        assert_eq!(
            reopened
                .stage_result(result.stage_result_id.as_str())
                .unwrap(),
            result
        );
        assert_eq!(
            reopened.stage_graph(graph.stage_graph_id.as_str()).unwrap(),
            completed_graph
        );
        assert_eq!(
            reopened.get(goal.goal_id.as_str()).unwrap().status,
            GoalStatus::Completed
        );

        let mut tampered = reopened.stage_graph(graph.stage_graph_id.as_str()).unwrap();
        tampered.stages[0].result_ref.as_mut().unwrap().sha256 =
            Sha256Hash::digest(b"tampered-stage-result-reference");
        tampered.graph_fingerprint = Sha256Hash::digest(b"unsealed-tampered-graph");
        tampered = tampered.seal().unwrap();
        reopened
            .file
            .stage_graphs
            .insert(tampered.stage_graph_id.to_string(), tampered.clone());
        reopened
            .file
            .stage_graph_history
            .get_mut(tampered.stage_graph_id.as_str())
            .unwrap()
            .insert(tampered.plan_revision, tampered);
        reopened.commit().unwrap();
        drop(reopened);
        assert!(matches!(
            GoalStore::load(path),
            Err(GoalStoreError::Corrupt)
        ));
    }
}
