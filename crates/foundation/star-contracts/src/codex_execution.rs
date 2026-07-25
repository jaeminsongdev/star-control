//! Durable, transport-neutral Codex App Server execution records.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ApprovalId, CodexExecutionId, GoalId, OperationId, RouteDecisionId, RunId, Sha256Hash, StageId,
    canonical_sha256, evidence::DocumentRef,
};

pub const CODEX_EXECUTION_RECORD_SCHEMA_ID: &str = "star.codex-execution-record";
pub const CODEX_EXECUTION_CONTRACT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CodexExecutionOperationV1 {
    Start,
    Resume,
    Fork,
    Interrupt,
    Status,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CodexExecutionStateV1 {
    Initializing,
    ThreadReady,
    Running,
    InterruptRequested,
    Interrupted,
    Completed,
    Failed,
    OutcomeUnknown,
    RecoveryRequired,
}

impl CodexExecutionStateV1 {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Interrupted
                | Self::Completed
                | Self::Failed
                | Self::OutcomeUnknown
                | Self::RecoveryRequired
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CodexThreadRefV1 {
    pub app_server_instance_id: String,
    pub protocol_version: String,
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_thread_id: Option<String>,
    pub capability_snapshot_ref: DocumentRef,
    pub thread_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CodexTurnRefV1 {
    pub thread_id: String,
    pub turn_id: String,
    pub turn_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CodexExecutionRecordV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub codex_execution_id: CodexExecutionId,
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_execution_id: Option<CodexExecutionId>,
    pub goal_id: GoalId,
    pub run_id: RunId,
    pub stage_id: StageId,
    pub stage_revision: u64,
    pub route_decision_id: RouteDecisionId,
    pub route_decision_ref: DocumentRef,
    pub context_pack_ref: DocumentRef,
    pub permission_plan_ref: DocumentRef,
    pub gate_decision_ref: DocumentRef,
    pub approval_id: ApprovalId,
    pub controller_operation_id: OperationId,
    pub tool_id: String,
    pub descriptor_hash: Sha256Hash,
    pub arguments_hash: Sha256Hash,
    pub executable_sha256: Sha256Hash,
    pub operation: CodexExecutionOperationV1,
    pub state: CodexExecutionStateV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_ref: Option<CodexThreadRefV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_ref: Option<CodexTurnRefV1>,
    pub instruction_fingerprint: Sha256Hash,
    pub last_event_sequence: u64,
    pub last_event_kind: String,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_summary: Option<String>,
    #[serde(default)]
    pub output_artifact_refs: Vec<DocumentRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redacted_error: Option<String>,
    pub outcome_unknown: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_effect_receipt_ref: Option<DocumentRef>,
    pub execution_fingerprint: Sha256Hash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CodexExecutionContractError {
    #[error("Codex execution identity is invalid")]
    Identity,
    #[error("Codex execution lifecycle is invalid")]
    Lifecycle,
    #[error("Codex execution reference is invalid")]
    Reference,
    #[error("Codex execution content is invalid")]
    Content,
    #[error("Codex execution fingerprint could not be calculated")]
    Fingerprint,
}

impl CodexThreadRefV1 {
    pub fn seal(mut self) -> Result<Self, CodexExecutionContractError> {
        if !bounded_token(&self.app_server_instance_id, 160)
            || !bounded_token(&self.protocol_version, 96)
            || !bounded_opaque(&self.thread_id, 512)
            || !document_ref(
                &self.capability_snapshot_ref,
                Some("star.capability-snapshot"),
            )
            || self
                .parent_thread_id
                .as_deref()
                .is_some_and(|value| !bounded_opaque(value, 512) || value == self.thread_id)
        {
            return Err(CodexExecutionContractError::Reference);
        }
        self.thread_fingerprint = canonical_sha256(&serde_json::json!({
            "domain":"star.codex-thread-ref",
            "version":CODEX_EXECUTION_CONTRACT_VERSION,
            "value":{
                "app_server_instance_id":self.app_server_instance_id,
                "protocol_version":self.protocol_version,
                "thread_id":self.thread_id,
                "parent_thread_id":self.parent_thread_id,
                "capability_snapshot_ref":self.capability_snapshot_ref,
            }
        }))
        .map_err(|_| CodexExecutionContractError::Fingerprint)?;
        Ok(self)
    }
}

impl CodexTurnRefV1 {
    pub fn seal(mut self) -> Result<Self, CodexExecutionContractError> {
        if !bounded_opaque(&self.thread_id, 512) || !bounded_opaque(&self.turn_id, 512) {
            return Err(CodexExecutionContractError::Reference);
        }
        self.turn_fingerprint = canonical_sha256(&serde_json::json!({
            "domain":"star.codex-turn-ref",
            "version":CODEX_EXECUTION_CONTRACT_VERSION,
            "value":{
                "thread_id":self.thread_id,
                "turn_id":self.turn_id,
            }
        }))
        .map_err(|_| CodexExecutionContractError::Fingerprint)?;
        Ok(self)
    }
}

impl CodexExecutionRecordV1 {
    pub fn seal(mut self) -> Result<Self, CodexExecutionContractError> {
        self.thread_ref = self.thread_ref.map(CodexThreadRefV1::seal).transpose()?;
        self.turn_ref = self.turn_ref.map(CodexTurnRefV1::seal).transpose()?;
        self.output_artifact_refs.sort();
        self.output_artifact_refs.dedup();
        self.validate_shape()?;
        self.execution_fingerprint = canonical_sha256(&serde_json::json!({
            "domain":CODEX_EXECUTION_RECORD_SCHEMA_ID,
            "version":CODEX_EXECUTION_CONTRACT_VERSION,
            "value":{
                "codex_execution_id":self.codex_execution_id,
                "revision":self.revision,
                "parent_execution_id":self.parent_execution_id,
                "goal_id":self.goal_id,
                "run_id":self.run_id,
                "stage_id":self.stage_id,
                "stage_revision":self.stage_revision,
                "route_decision_id":self.route_decision_id,
                "route_decision_ref":self.route_decision_ref,
                "context_pack_ref":self.context_pack_ref,
                "permission_plan_ref":self.permission_plan_ref,
                "gate_decision_ref":self.gate_decision_ref,
                "approval_id":self.approval_id,
                "controller_operation_id":self.controller_operation_id,
                "tool_id":self.tool_id,
                "descriptor_hash":self.descriptor_hash,
                "arguments_hash":self.arguments_hash,
                "executable_sha256":self.executable_sha256,
                "operation":self.operation,
                "state":self.state,
                "thread_ref":self.thread_ref,
                "turn_ref":self.turn_ref,
                "instruction_fingerprint":self.instruction_fingerprint,
                "last_event_sequence":self.last_event_sequence,
                "last_event_kind":self.last_event_kind,
                "started_at":self.started_at,
                "updated_at":self.updated_at,
                "finished_at":self.finished_at,
                "result_summary":self.result_summary,
                "output_artifact_refs":self.output_artifact_refs,
                "error_code":self.error_code,
                "redacted_error":self.redacted_error,
                "outcome_unknown":self.outcome_unknown,
                "recovery_action":self.recovery_action,
                "terminal_effect_receipt_ref":self.terminal_effect_receipt_ref,
            }
        }))
        .map_err(|_| CodexExecutionContractError::Fingerprint)?;
        Ok(self)
    }

    pub fn verify(&self) -> Result<(), CodexExecutionContractError> {
        let expected = self.clone().seal()?;
        if expected != *self {
            return Err(CodexExecutionContractError::Fingerprint);
        }
        Ok(())
    }

    pub fn reference(&self) -> DocumentRef {
        DocumentRef {
            schema_id: CODEX_EXECUTION_RECORD_SCHEMA_ID.to_owned(),
            document_id: self.codex_execution_id.to_string(),
            revision: self.revision,
            sha256: self.execution_fingerprint.clone(),
        }
    }

    fn validate_shape(&self) -> Result<(), CodexExecutionContractError> {
        let expected_tool_id = match self.operation {
            CodexExecutionOperationV1::Start => "codex.task.start",
            CodexExecutionOperationV1::Resume => "codex.task.resume",
            CodexExecutionOperationV1::Fork => "codex.task.fork",
            CodexExecutionOperationV1::Interrupt => "codex.task.interrupt",
            CodexExecutionOperationV1::Status => "codex.task.status",
        };
        if self.schema_id != CODEX_EXECUTION_RECORD_SCHEMA_ID
            || self.schema_version != CODEX_EXECUTION_CONTRACT_VERSION
            || self.revision == 0
            || self.stage_revision == 0
            || (self.operation == CodexExecutionOperationV1::Start
                && self.parent_execution_id.is_some())
            || (matches!(
                self.operation,
                CodexExecutionOperationV1::Resume | CodexExecutionOperationV1::Fork
            ) && self.parent_execution_id.is_none())
            || self
                .parent_execution_id
                .as_ref()
                .is_some_and(|parent| parent == &self.codex_execution_id)
            || !document_ref(&self.route_decision_ref, Some("star.route-decision"))
            || self.route_decision_ref.document_id != self.route_decision_id.as_str()
            || !document_ref(&self.context_pack_ref, Some("star.context-pack"))
            || !document_ref(&self.permission_plan_ref, Some("star.permission-plan"))
            || !document_ref(&self.gate_decision_ref, Some("star.gate-decision"))
            || self.tool_id != expected_tool_id
            || self.descriptor_hash == Sha256Hash::digest(b"")
            || self.arguments_hash == Sha256Hash::digest(b"")
            || self.executable_sha256 == Sha256Hash::digest(b"")
            || self.instruction_fingerprint == Sha256Hash::digest(b"")
            || self
                .output_artifact_refs
                .iter()
                .any(|reference| !document_ref(reference, None))
        {
            return Err(CodexExecutionContractError::Identity);
        }
        if self.updated_at < self.started_at
            || self
                .finished_at
                .is_some_and(|finished| finished < self.updated_at)
            || self.state.is_terminal() != self.finished_at.is_some()
            || (self.state.is_terminal() != self.terminal_effect_receipt_ref.is_some())
        {
            return Err(CodexExecutionContractError::Lifecycle);
        }
        if self
            .terminal_effect_receipt_ref
            .as_ref()
            .is_some_and(|receipt| !document_ref(receipt, Some("star.development-effect-receipt")))
        {
            return Err(CodexExecutionContractError::Reference);
        }
        if self
            .turn_ref
            .as_ref()
            .zip(self.thread_ref.as_ref())
            .is_some_and(|(turn, thread)| turn.thread_id != thread.thread_id)
            || self.turn_ref.is_some() && self.thread_ref.is_none()
        {
            return Err(CodexExecutionContractError::Reference);
        }
        if !bounded_token(&self.last_event_kind, 128)
            || self
                .result_summary
                .as_deref()
                .is_some_and(|value| !bounded_text(value, 16_384))
            || self
                .error_code
                .as_deref()
                .is_some_and(|value| !bounded_token(value, 128))
            || self
                .redacted_error
                .as_deref()
                .is_some_and(|value| !bounded_text(value, 4_096))
            || self
                .recovery_action
                .as_deref()
                .is_some_and(|value| !bounded_token(value, 128))
        {
            return Err(CodexExecutionContractError::Content);
        }
        match self.state {
            CodexExecutionStateV1::Initializing => {
                if self.thread_ref.is_some()
                    || self.turn_ref.is_some()
                    || self.last_event_sequence != 0
                {
                    return Err(CodexExecutionContractError::Lifecycle);
                }
            }
            CodexExecutionStateV1::ThreadReady => {
                if self.thread_ref.is_none()
                    || self.turn_ref.is_some()
                    || !matches!(
                        self.last_event_kind.as_str(),
                        "thread_ready" | "turn_start_requested"
                    )
                    || self.last_event_sequence == 0
                {
                    return Err(CodexExecutionContractError::Lifecycle);
                }
            }
            CodexExecutionStateV1::Running | CodexExecutionStateV1::InterruptRequested => {
                if self.thread_ref.is_none()
                    || self.turn_ref.is_none()
                    || self.last_event_sequence == 0
                {
                    return Err(CodexExecutionContractError::Lifecycle);
                }
            }
            CodexExecutionStateV1::Completed => {
                if self.thread_ref.is_none()
                    || self.turn_ref.is_none()
                    || self.result_summary.is_none()
                    || self.error_code.is_some()
                    || self.outcome_unknown
                    || self.last_event_kind != "turn_completed"
                {
                    return Err(CodexExecutionContractError::Lifecycle);
                }
            }
            CodexExecutionStateV1::Failed => {
                if self.error_code.is_none()
                    || self.redacted_error.is_none()
                    || self.outcome_unknown
                    || self.last_event_kind != "turn_failed"
                {
                    return Err(CodexExecutionContractError::Lifecycle);
                }
            }
            CodexExecutionStateV1::OutcomeUnknown => {
                if !self.outcome_unknown
                    || self.recovery_action.is_none()
                    || self.last_event_kind != "outcome_unknown"
                {
                    return Err(CodexExecutionContractError::Lifecycle);
                }
            }
            CodexExecutionStateV1::Interrupted => {
                if self.recovery_action.is_none() || self.last_event_kind != "turn_interrupted" {
                    return Err(CodexExecutionContractError::Lifecycle);
                }
            }
            CodexExecutionStateV1::RecoveryRequired => {
                if self.recovery_action.is_none() || self.last_event_kind != "outcome_unknown" {
                    return Err(CodexExecutionContractError::Lifecycle);
                }
            }
        }
        Ok(())
    }
}

fn document_ref(reference: &DocumentRef, schema_id: Option<&str>) -> bool {
    schema_id.is_none_or(|schema_id| reference.schema_id == schema_id)
        && bounded_token(&reference.schema_id, 192)
        && bounded_opaque(&reference.document_id, 512)
        && reference.revision > 0
        && reference.sha256 != Sha256Hash::digest(b"")
}

fn bounded_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.contains('\0')
}

fn bounded_token(value: &str, max: usize) -> bool {
    bounded_text(value, max)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/'))
}

fn bounded_opaque(value: &str, max: usize) -> bool {
    bounded_text(value, max) && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CapabilitySnapshotId, evidence::DocumentRef};

    fn hash(label: &str) -> Sha256Hash {
        Sha256Hash::digest(label.as_bytes())
    }

    fn document(schema: &str, id: &str) -> DocumentRef {
        DocumentRef {
            schema_id: schema.to_owned(),
            document_id: id.to_owned(),
            revision: 1,
            sha256: hash(id),
        }
    }

    fn thread() -> CodexThreadRefV1 {
        CodexThreadRefV1 {
            app_server_instance_id: "app-server-1".to_owned(),
            protocol_version: "app-server-v1".to_owned(),
            thread_id: "thread-1".to_owned(),
            parent_thread_id: None,
            capability_snapshot_ref: document(
                "star.capability-snapshot",
                CapabilitySnapshotId::new().as_str(),
            ),
            thread_fingerprint: hash("unsealed-thread"),
        }
        .seal()
        .unwrap()
    }

    fn turn() -> CodexTurnRefV1 {
        CodexTurnRefV1 {
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-1".to_owned(),
            turn_fingerprint: hash("unsealed-turn"),
        }
        .seal()
        .unwrap()
    }

    fn running() -> CodexExecutionRecordV1 {
        let now = Utc::now();
        let route_id = RouteDecisionId::new();
        CodexExecutionRecordV1 {
            schema_id: CODEX_EXECUTION_RECORD_SCHEMA_ID.to_owned(),
            schema_version: CODEX_EXECUTION_CONTRACT_VERSION,
            codex_execution_id: CodexExecutionId::new(),
            revision: 1,
            parent_execution_id: None,
            goal_id: GoalId::new(),
            run_id: RunId::new(),
            stage_id: StageId::new(),
            stage_revision: 1,
            route_decision_id: route_id.clone(),
            route_decision_ref: document("star.route-decision", route_id.as_str()),
            context_pack_ref: document("star.context-pack", "context-1"),
            permission_plan_ref: document("star.permission-plan", "permission-1"),
            gate_decision_ref: document("star.gate-decision", "gate-1"),
            approval_id: ApprovalId::new(),
            controller_operation_id: OperationId::new(),
            tool_id: "codex.task.start".to_owned(),
            descriptor_hash: hash("descriptor"),
            arguments_hash: hash("arguments"),
            executable_sha256: hash("executable"),
            operation: CodexExecutionOperationV1::Start,
            state: CodexExecutionStateV1::Running,
            thread_ref: Some(thread()),
            turn_ref: Some(turn()),
            instruction_fingerprint: hash("instruction"),
            last_event_sequence: 2,
            last_event_kind: "turn_started".to_owned(),
            started_at: now,
            updated_at: now,
            finished_at: None,
            result_summary: None,
            output_artifact_refs: Vec::new(),
            error_code: None,
            redacted_error: None,
            outcome_unknown: false,
            recovery_action: None,
            terminal_effect_receipt_ref: None,
            execution_fingerprint: hash("unsealed-record"),
        }
    }

    #[test]
    fn execution_positive_completed_record_is_sealed() {
        let mut record = running();
        record.state = CodexExecutionStateV1::Completed;
        record.revision = 2;
        record.last_event_sequence = 3;
        record.last_event_kind = "turn_completed".to_owned();
        record.result_summary = Some("bounded stage completed".to_owned());
        record.finished_at = Some(record.updated_at);
        record.terminal_effect_receipt_ref = Some(document(
            "star.development-effect-receipt",
            "effect-completed",
        ));
        let record = record.seal().unwrap();
        record.verify().unwrap();
    }

    #[test]
    fn execution_negative_completed_without_turn_is_rejected() {
        let mut record = running();
        record.state = CodexExecutionStateV1::Completed;
        record.turn_ref = None;
        record.result_summary = Some("invalid".to_owned());
        record.finished_at = Some(record.updated_at);
        assert_eq!(record.seal(), Err(CodexExecutionContractError::Lifecycle));
    }

    #[test]
    fn execution_failure_requires_redacted_error_evidence() {
        let mut record = running();
        record.state = CodexExecutionStateV1::Failed;
        record.last_event_kind = "turn_failed".to_owned();
        record.error_code = Some("CODEX_NOT_READY".to_owned());
        record.finished_at = Some(record.updated_at);
        assert_eq!(
            record.clone().seal(),
            Err(CodexExecutionContractError::Lifecycle)
        );
        record.redacted_error = Some("app server exited before completion".to_owned());
        record.terminal_effect_receipt_ref =
            Some(document("star.development-effect-receipt", "effect-failed"));
        assert!(record.seal().is_ok());
    }

    #[test]
    fn execution_recovery_preserves_outcome_unknown() {
        let mut record = running();
        record.state = CodexExecutionStateV1::OutcomeUnknown;
        record.last_event_kind = "outcome_unknown".to_owned();
        record.outcome_unknown = true;
        record.recovery_action = Some("thread_reconcile".to_owned());
        record.finished_at = Some(record.updated_at);
        record.terminal_effect_receipt_ref = Some(document(
            "star.development-effect-receipt",
            "effect-unknown",
        ));
        let record = record.seal().unwrap();
        assert!(record.outcome_unknown);
        assert_eq!(record.state, CodexExecutionStateV1::OutcomeUnknown);
    }

    #[test]
    fn execution_lineage_requires_an_external_parent_for_resume_and_fork() {
        let mut record = running();
        record.operation = CodexExecutionOperationV1::Resume;
        record.tool_id = "codex.task.resume".to_owned();
        assert_eq!(
            record.clone().seal(),
            Err(CodexExecutionContractError::Identity)
        );

        record.parent_execution_id = Some(CodexExecutionId::new());
        assert!(record.clone().seal().is_ok());

        record.parent_execution_id = Some(record.codex_execution_id.clone());
        assert_eq!(
            record.clone().seal(),
            Err(CodexExecutionContractError::Identity)
        );

        record.operation = CodexExecutionOperationV1::Start;
        record.tool_id = "codex.task.start".to_owned();
        record.parent_execution_id = Some(CodexExecutionId::new());
        assert_eq!(record.seal(), Err(CodexExecutionContractError::Identity));
    }

    #[test]
    fn execution_negative_rejects_unsealed_critical_reference() {
        let mut record = running();
        record.context_pack_ref.revision = 0;
        assert_eq!(record.seal(), Err(CodexExecutionContractError::Identity));
    }
}
