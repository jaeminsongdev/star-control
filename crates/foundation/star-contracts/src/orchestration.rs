//! Durable Goal/Plan/Run orchestration contracts.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{GoalId, RunId, Sha256Hash, canonical_sha256, evidence::DocumentRef};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_evidence_ref: Option<DocumentRef>,
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
        if in_progress > 1
            || self.plan_items.is_empty() != (self.plan_revision == 0)
            || (self.status == GoalStatus::Completed
                && (!self
                    .plan_items
                    .iter()
                    .all(|item| item.status == GoalPlanItemStatus::Completed)
                    || self
                        .run
                        .as_ref()
                        .is_none_or(|run| run.status != GoalRunStatus::Completed)
                    || self
                        .completion_evidence_ref
                        .as_ref()
                        .is_none_or(|reference| {
                            reference.schema_id != "star.evidence-bundle"
                                || !bounded_token(&reference.document_id, 192)
                                || reference.revision == 0
                                || reference.sha256 == Sha256Hash::digest(b"")
                        })))
            || (self.status == GoalStatus::Blocked
                && !self
                    .plan_items
                    .iter()
                    .any(|item| item.status == GoalPlanItemStatus::Blocked))
            || (self.status != GoalStatus::Completed && self.completion_evidence_ref.is_some())
        {
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
            if matches!(
                self.status,
                GoalStatus::Active | GoalStatus::WaitingQuestion | GoalStatus::Paused
            ) && run.status != GoalRunStatus::Running
            {
                return Err(GoalContractError::Lifecycle);
            }
        } else if self.status == GoalStatus::Completed {
            return Err(GoalContractError::Lifecycle);
        }
        if self.expected_fingerprint()? != self.content_fingerprint
            && (self.completion_evidence_ref.is_some()
                || self.legacy_fingerprint_without_completion_evidence()?
                    != self.content_fingerprint)
        {
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
                "completion_evidence_ref": self.completion_evidence_ref,
                "created_at": self.created_at,
                "updated_at": self.updated_at,
            }
        }))
        .map_err(|_| GoalContractError::Fingerprint)
    }

    fn legacy_fingerprint_without_completion_evidence(
        &self,
    ) -> Result<Sha256Hash, GoalContractError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn completed_goal() -> GoalRecord {
        let timestamp = goal_timestamp_now();
        GoalRecord {
            schema_id: GOAL_RECORD_SCHEMA_ID.to_owned(),
            schema_version: GOAL_RECORD_SCHEMA_VERSION,
            goal_id: GoalId::new(),
            revision: 3,
            objective: "complete with current evidence".to_owned(),
            project_key: Some("star-control".to_owned()),
            status: GoalStatus::Completed,
            plan_revision: 1,
            plan_items: vec![GoalPlanItem {
                item_id: "validate".to_owned(),
                step: "run the exact required gate".to_owned(),
                status: GoalPlanItemStatus::Completed,
            }],
            pending_question: None,
            run: Some(GoalRunState {
                run_id: RunId::new(),
                attempt: 1,
                status: GoalRunStatus::Completed,
                continued_at: timestamp.clone(),
            }),
            completion_evidence_ref: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            content_fingerprint: Sha256Hash::digest(b"unsealed"),
        }
    }

    #[test]
    fn goal_completion_requires_exact_evidence_reference() {
        let mut goal = completed_goal();
        assert_eq!(goal.clone().seal(), Err(GoalContractError::Plan));

        goal.completion_evidence_ref = Some(DocumentRef {
            schema_id: "star.evidence-bundle".to_owned(),
            document_id: "evb_current".to_owned(),
            revision: 1,
            sha256: Sha256Hash::digest(b"current-evidence"),
        });
        goal.seal().unwrap().validate().unwrap();
    }

    #[test]
    fn goal_nonterminal_state_cannot_carry_completion_evidence() {
        let mut goal = completed_goal();
        goal.status = GoalStatus::Active;
        goal.run.as_mut().unwrap().status = GoalRunStatus::Running;
        goal.completion_evidence_ref = Some(DocumentRef {
            schema_id: "star.evidence-bundle".to_owned(),
            document_id: "evb_stale".to_owned(),
            revision: 1,
            sha256: Sha256Hash::digest(b"stale-evidence"),
        });
        assert_eq!(goal.seal(), Err(GoalContractError::Plan));
    }

    #[test]
    fn exact_legacy_goal_fingerprint_without_completion_evidence_remains_valid() {
        let timestamp = goal_timestamp_now();
        let mut goal = GoalRecord {
            schema_id: GOAL_RECORD_SCHEMA_ID.to_owned(),
            schema_version: GOAL_RECORD_SCHEMA_VERSION,
            goal_id: GoalId::new(),
            revision: 2,
            objective: "read a goal written before completion evidence was added".to_owned(),
            project_key: Some("star-control".to_owned()),
            status: GoalStatus::Cancelled,
            plan_revision: 0,
            plan_items: Vec::new(),
            pending_question: None,
            run: Some(GoalRunState {
                run_id: RunId::new(),
                attempt: 1,
                status: GoalRunStatus::Cancelled,
                continued_at: timestamp.clone(),
            }),
            completion_evidence_ref: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            content_fingerprint: Sha256Hash::digest(b"unsealed"),
        };
        goal.content_fingerprint = goal
            .legacy_fingerprint_without_completion_evidence()
            .unwrap();

        let serialized = serde_json::to_value(&goal).unwrap();
        assert!(serialized.get("completion_evidence_ref").is_none());
        let reloaded: GoalRecord = serde_json::from_value(serialized).unwrap();
        reloaded.validate().unwrap();

        let resealed = reloaded.clone().seal().unwrap();
        assert_ne!(resealed.content_fingerprint, reloaded.content_fingerprint);
        resealed.validate().unwrap();

        let mut tampered = reloaded;
        tampered.objective.push_str(" with tampering");
        assert_eq!(tampered.validate(), Err(GoalContractError::Fingerprint));
    }
}
