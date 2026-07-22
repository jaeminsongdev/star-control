//! Durable Goal/Plan/Run orchestration contracts.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{GoalId, RunId, Sha256Hash, canonical_sha256};

pub const GOAL_RECORD_SCHEMA_ID: &str = "star.goal-record";
pub const GOAL_RECORD_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Active,
    WaitingQuestion,
    Paused,
    Completed,
    Blocked,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GoalPlanItemStatus {
    Pending,
    InProgress,
    Completed,
    Blocked,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanItem {
    pub item_id: String,
    pub step: String,
    pub status: GoalPlanItemStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GoalQuestion {
    pub question_id: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GoalRunStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GoalRunState {
    pub run_id: RunId,
    pub attempt: u32,
    pub status: GoalRunStatus,
    pub continued_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GoalRecord {
    pub schema_id: String,
    pub schema_version: u32,
    pub goal_id: GoalId,
    pub revision: u64,
    pub objective: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_key: Option<String>,
    pub status: GoalStatus,
    pub plan_revision: u64,
    pub plan_items: Vec<GoalPlanItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_question: Option<GoalQuestion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<GoalRunState>,
    pub created_at: String,
    pub updated_at: String,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GoalContractError {
    #[error("goal schema identity is invalid")]
    Schema,
    #[error("goal content is empty or exceeds its bound")]
    Content,
    #[error("goal revision or lifecycle state is invalid")]
    Lifecycle,
    #[error("goal plan is invalid")]
    Plan,
    #[error("goal timestamp is invalid")]
    Timestamp,
    #[error("goal fingerprint is invalid")]
    Fingerprint,
}

impl GoalRecord {
    pub fn seal(mut self) -> Result<Self, GoalContractError> {
        self.content_fingerprint = self.expected_fingerprint()?;
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), GoalContractError> {
        if self.schema_id != GOAL_RECORD_SCHEMA_ID
            || self.schema_version != GOAL_RECORD_SCHEMA_VERSION
            || self.revision == 0
        {
            return Err(GoalContractError::Schema);
        }
        if !bounded_text(&self.objective, 4_096)
            || self
                .project_key
                .as_deref()
                .is_some_and(|value| !bounded_token(value, 128))
        {
            return Err(GoalContractError::Content);
        }
        let created = DateTime::parse_from_rfc3339(&self.created_at)
            .map_err(|_| GoalContractError::Timestamp)?;
        let updated = DateTime::parse_from_rfc3339(&self.updated_at)
            .map_err(|_| GoalContractError::Timestamp)?;
        if updated < created {
            return Err(GoalContractError::Timestamp);
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut in_progress = 0_u32;
        for item in &self.plan_items {
            if !bounded_token(&item.item_id, 128)
                || !bounded_text(&item.step, 4_096)
                || !ids.insert(item.item_id.as_str())
            {
                return Err(GoalContractError::Plan);
            }
            if item.status == GoalPlanItemStatus::InProgress {
                in_progress += 1;
            }
        }
        if in_progress > 1 || (self.plan_items.is_empty() && self.plan_revision != 0) {
            return Err(GoalContractError::Plan);
        }
        if let Some(question) = &self.pending_question
            && (!bounded_token(&question.question_id, 128)
                || !bounded_text(&question.prompt, 4_096)
                || question
                    .answer
                    .as_deref()
                    .is_some_and(|answer| !bounded_text(answer, 16_384)))
        {
            return Err(GoalContractError::Content);
        }
        let unanswered = self
            .pending_question
            .as_ref()
            .is_some_and(|question| question.answer.is_none());
        if (self.status == GoalStatus::WaitingQuestion) != unanswered
            && self.status != GoalStatus::Paused
            && self.status != GoalStatus::Cancelled
        {
            return Err(GoalContractError::Lifecycle);
        }
        if let Some(run) = &self.run {
            if run.attempt == 0 || DateTime::parse_from_rfc3339(&run.continued_at).is_err() {
                return Err(GoalContractError::Lifecycle);
            }
            if self.status == GoalStatus::Cancelled && run.status != GoalRunStatus::Cancelled {
                return Err(GoalContractError::Lifecycle);
            }
        }
        if self.expected_fingerprint()? != self.content_fingerprint {
            return Err(GoalContractError::Fingerprint);
        }
        Ok(())
    }

    fn expected_fingerprint(&self) -> Result<Sha256Hash, GoalContractError> {
        canonical_sha256(&serde_json::json!({
            "domain": GOAL_RECORD_SCHEMA_ID,
            "version": GOAL_RECORD_SCHEMA_VERSION,
            "value": {
                "goal_id": self.goal_id,
                "revision": self.revision,
                "objective": self.objective,
                "project_key": self.project_key,
                "status": self.status,
                "plan_revision": self.plan_revision,
                "plan_items": self.plan_items,
                "pending_question": self.pending_question,
                "run": self.run,
                "created_at": self.created_at,
                "updated_at": self.updated_at,
            }
        }))
        .map_err(|_| GoalContractError::Fingerprint)
    }
}

fn bounded_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.contains('\0')
}

fn bounded_token(value: &str, max: usize) -> bool {
    bounded_text(value, max)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

pub fn goal_timestamp_now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
