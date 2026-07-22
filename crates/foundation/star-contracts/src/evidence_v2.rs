//! M3 validation execution, diagnostic, gate, and evidence contracts.
//!
//! Version 1 evidence remains readable for the P0 precursor. These version 2
//! documents are the only writer contracts for the full CheckGraph runner.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    Sha256Hash, canonical_sha256,
    evidence::{
        ActorRef, ArtifactManifest, ArtifactRef, AuthoritativeGateState, CatalogRef, Completeness,
        DiagnosticConfidence, DiagnosticSeverity, DiagnosticStatus, EnvironmentValueRef,
        GateDecisionKind, GateScope, ObservedTool, OutputLimits, RiskRef, TerminationReason,
        ValidationOutcome, ValidationRunRef,
    },
    ids::{DiagnosticId, EvidenceBundleId, GateId, ProjectId, TaskInvocationId, ValidationRunId},
    management::ProjectPathRef,
    planning::ValidationPlanV2Readiness,
};

pub const TASK_INVOCATION_V2_SCHEMA_ID: &str = "star.task-invocation";
pub const VALIDATION_RUN_V2_SCHEMA_ID: &str = "star.validation-run";
pub const DIAGNOSTIC_V2_SCHEMA_ID: &str = "star.diagnostic";
pub const GATE_DECISION_V2_SCHEMA_ID: &str = "star.gate-decision";
pub const EVIDENCE_BUNDLE_V2_SCHEMA_ID: &str = "star.evidence-bundle";
pub const EVIDENCE_V2_SCHEMA_VERSION: u32 = 2;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStabilityV2 {
    Stable,
    Flaky,
    NotEvaluated,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskInvocationV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub invocation_id: TaskInvocationId,
    pub tool_ref: CatalogRef,
    pub executable: String,
    pub executable_binding_fingerprint: Sha256Hash,
    pub args: Vec<String>,
    pub cwd: ProjectPathRef,
    pub env_refs: BTreeMap<String, EnvironmentValueRef>,
    pub stdin_ref: Option<ArtifactRef>,
    pub timeout_ms: u64,
    pub permission_action: String,
    pub idempotency_key: String,
    pub expected_exit_codes: BTreeSet<i32>,
    pub output_limits: OutputLimits,
    pub input_fingerprint: Sha256Hash,
}

impl TaskInvocationV2 {
    pub fn seal(mut self) -> Result<Self, EvidenceV2Error> {
        if self.schema_id != TASK_INVOCATION_V2_SCHEMA_ID
            || self.schema_version != EVIDENCE_V2_SCHEMA_VERSION
            || self.executable.trim().is_empty()
            || self.executable.contains(['/', '\\', ':', '\0'])
            || self.args.iter().any(|argument| argument.contains('\0'))
            || self.args.iter().map(String::len).sum::<usize>() > 256 * 1024
            || self.timeout_ms == 0
            || self.permission_action.trim().is_empty()
            || self.idempotency_key.trim().is_empty()
            || self.idempotency_key.len() > 128
            || self.expected_exit_codes.is_empty()
            || self.output_limits.stdout_bytes == 0
            || self.output_limits.stderr_bytes == 0
            || self.output_limits.artifact_bytes == 0
            || self.env_refs.keys().any(|name| {
                name.is_empty()
                    || name.len() > 128
                    || !name
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            })
        {
            return Err(EvidenceV2Error::Invocation);
        }
        if let Some(stdin) = &self.stdin_ref {
            stdin.validate().map_err(|_| EvidenceV2Error::Artifact)?;
        }
        self.input_fingerprint = fingerprint(
            "star.task-invocation",
            EVIDENCE_V2_SCHEMA_VERSION,
            &serde_json::json!({
                "tool_ref":self.tool_ref,
                "executable":self.executable,
                "executable_binding_fingerprint":self.executable_binding_fingerprint,
                "args":self.args,
                "cwd":self.cwd,
                "env_refs":self.env_refs,
                "stdin_ref":self.stdin_ref,
                "timeout_ms":self.timeout_ms,
                "permission_action":self.permission_action,
                "idempotency_key":self.idempotency_key,
                "expected_exit_codes":self.expected_exit_codes,
                "output_limits":self.output_limits,
            }),
        )?;
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidationRunV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub validation_run_id: ValidationRunId,
    pub revision: u64,
    pub validation_plan_ref: crate::evidence::DocumentRef,
    pub plan_item_id: String,
    pub project_id: ProjectId,
    pub phase: String,
    pub attempt: u32,
    pub invocation: TaskInvocationV2,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub outcome: ValidationOutcome,
    pub completeness: Completeness,
    pub stability: ValidationStabilityV2,
    pub exit_code: Option<i32>,
    pub termination_reason: Option<TerminationReason>,
    pub diagnostic_ids: Vec<DiagnosticId>,
    pub artifact_refs: Vec<ArtifactRef>,
    pub observed_tool: Option<ObservedTool>,
    pub result_fingerprint: Sha256Hash,
}

impl ValidationRunV2 {
    pub fn seal(mut self) -> Result<Self, EvidenceV2Error> {
        self.invocation = self.invocation.seal()?;
        self.diagnostic_ids.sort();
        self.diagnostic_ids.dedup();
        self.artifact_refs
            .sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
        if self.schema_id != VALIDATION_RUN_V2_SCHEMA_ID
            || self.schema_version != EVIDENCE_V2_SCHEMA_VERSION
            || self.revision == 0
            || self.plan_item_id.trim().is_empty()
            || self.phase.trim().is_empty()
            || self.attempt == 0
            || self
                .started_at
                .zip(self.finished_at)
                .is_some_and(|(started, finished)| finished < started)
            || self
                .artifact_refs
                .iter()
                .any(|artifact| artifact.validate().is_err())
        {
            return Err(EvidenceV2Error::Run);
        }
        if self.outcome == ValidationOutcome::NotRun
            && (self.started_at.is_some()
                || self.finished_at.is_some()
                || self.exit_code.is_some()
                || self.termination_reason.is_some()
                || self.observed_tool.is_some()
                || self.stability != ValidationStabilityV2::NotEvaluated)
        {
            return Err(EvidenceV2Error::Run);
        }
        if self.outcome == ValidationOutcome::Pass && !self.satisfies_required_check() {
            return Err(EvidenceV2Error::FalsePass);
        }
        self.result_fingerprint = fingerprint(
            "star.validation-run",
            EVIDENCE_V2_SCHEMA_VERSION,
            &serde_json::json!({
                "validation_run_id":self.validation_run_id,
                "revision":self.revision,
                "validation_plan_ref":self.validation_plan_ref,
                "plan_item_id":self.plan_item_id,
                "project_id":self.project_id,
                "phase":self.phase,
                "attempt":self.attempt,
                "invocation":self.invocation,
                "started_at":self.started_at,
                "finished_at":self.finished_at,
                "outcome":self.outcome,
                "completeness":self.completeness,
                "stability":self.stability,
                "exit_code":self.exit_code,
                "termination_reason":self.termination_reason,
                "diagnostic_ids":self.diagnostic_ids,
                "artifact_refs":self.artifact_refs,
                "observed_tool":self.observed_tool,
            }),
        )?;
        Ok(self)
    }

    pub fn satisfies_required_check(&self) -> bool {
        self.outcome == ValidationOutcome::Pass
            && self.completeness == Completeness::Complete
            && self.stability == ValidationStabilityV2::Stable
            && self.started_at.is_some()
            && self.finished_at.is_some()
            && self.termination_reason == Some(TerminationReason::Exited)
            && self
                .exit_code
                .is_some_and(|code| self.invocation.expected_exit_codes.contains(&code))
            && self.observed_tool.is_some()
    }

    pub fn reference(&self) -> Result<ValidationRunRef, EvidenceV2Error> {
        Ok(ValidationRunRef {
            validation_run_id: self.validation_run_id.clone(),
            revision: self.revision,
            sha256: document_hash(self)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub diagnostic_id: DiagnosticId,
    pub sequence: u64,
    pub code: String,
    pub rule_ref: CatalogRef,
    pub title: String,
    pub message: String,
    pub severity: DiagnosticSeverity,
    pub confidence: DiagnosticConfidence,
    pub status: DiagnosticStatus,
    pub blocking: bool,
    pub project_id: ProjectId,
    pub plan_item_id: String,
    pub validation_run_id: ValidationRunId,
    pub evidence_refs: Vec<ArtifactRef>,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub fingerprint: Sha256Hash,
}

impl DiagnosticV2 {
    pub fn seal(mut self) -> Result<Self, EvidenceV2Error> {
        self.evidence_refs
            .sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
        if self.schema_id != DIAGNOSTIC_V2_SCHEMA_ID
            || self.schema_version != EVIDENCE_V2_SCHEMA_VERSION
            || self.sequence == 0
            || self.code.trim().is_empty()
            || self.title.trim().is_empty()
            || self.message.trim().is_empty()
            || self.plan_item_id.trim().is_empty()
            || self.last_seen_at < self.first_seen_at
            || self
                .evidence_refs
                .iter()
                .any(|artifact| artifact.validate().is_err())
            || (self.blocking
                && matches!(
                    self.status,
                    DiagnosticStatus::Suppressed | DiagnosticStatus::Resolved
                ))
        {
            return Err(EvidenceV2Error::Diagnostic);
        }
        self.fingerprint = fingerprint(
            "star.diagnostic",
            EVIDENCE_V2_SCHEMA_VERSION,
            &serde_json::json!({
                "code":self.code,
                "rule_ref":self.rule_ref,
                "title":self.title,
                "message":self.message,
                "severity":self.severity,
                "confidence":self.confidence,
                "status":self.status,
                "blocking":self.blocking,
                "project_id":self.project_id,
                "plan_item_id":self.plan_item_id,
                "validation_run_id":self.validation_run_id,
                "evidence_refs":self.evidence_refs,
            }),
        )?;
        Ok(self)
    }

    pub fn reference(&self) -> Result<crate::evidence::DiagnosticRef, EvidenceV2Error> {
        Ok(crate::evidence::DiagnosticRef {
            diagnostic_id: self.diagnostic_id.clone(),
            sequence: self.sequence,
            sha256: document_hash(self)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GateDecisionV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub gate_id: GateId,
    pub revision: u64,
    pub validation_plan_ref: crate::evidence::DocumentRef,
    pub scope: GateScope,
    pub decision: GateDecisionKind,
    pub required_run_refs: Vec<ValidationRunRef>,
    pub satisfied_run_refs: Vec<ValidationRunRef>,
    pub blocking_diagnostic_refs: Vec<crate::evidence::DiagnosticRef>,
    pub reason_codes: Vec<String>,
    pub remaining_risks: Vec<RiskRef>,
    pub policy_fingerprint: Sha256Hash,
    pub decided_by: ActorRef,
    pub decided_at: DateTime<Utc>,
    pub decision_fingerprint: Sha256Hash,
}

impl GateDecisionV2 {
    pub fn seal(
        mut self,
        runs: &[ValidationRunV2],
        diagnostics: &[DiagnosticV2],
    ) -> Result<Self, EvidenceV2Error> {
        self.required_run_refs.sort();
        self.required_run_refs.dedup();
        self.satisfied_run_refs.sort();
        self.satisfied_run_refs.dedup();
        self.blocking_diagnostic_refs.sort();
        self.blocking_diagnostic_refs.dedup();
        self.reason_codes.sort();
        self.reason_codes.dedup();
        if self.schema_id != GATE_DECISION_V2_SCHEMA_ID
            || self.schema_version != EVIDENCE_V2_SCHEMA_VERSION
            || self.revision == 0
            || self.reason_codes.is_empty()
            || self
                .reason_codes
                .iter()
                .any(|reason| reason.trim().is_empty())
        {
            return Err(EvidenceV2Error::Gate);
        }
        let actual_run_refs = runs
            .iter()
            .map(ValidationRunV2::reference)
            .collect::<Result<BTreeSet<_>, _>>()?;
        let required = self
            .required_run_refs
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let satisfied = self
            .satisfied_run_refs
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if !required.is_subset(&actual_run_refs) || !satisfied.is_subset(&required) {
            return Err(EvidenceV2Error::Gate);
        }
        for reference in &satisfied {
            let run = runs
                .iter()
                .find(|run| {
                    run.validation_run_id == reference.validation_run_id
                        && run.revision == reference.revision
                })
                .ok_or(EvidenceV2Error::Gate)?;
            if !run.satisfies_required_check() {
                return Err(EvidenceV2Error::FalsePass);
            }
        }
        let actual_diagnostics = diagnostics
            .iter()
            .map(DiagnosticV2::reference)
            .collect::<Result<BTreeSet<_>, _>>()?;
        if !self
            .blocking_diagnostic_refs
            .iter()
            .all(|reference| actual_diagnostics.contains(reference))
        {
            return Err(EvidenceV2Error::Gate);
        }
        match self.decision {
            GateDecisionKind::AutoPass
                if required != satisfied || !self.blocking_diagnostic_refs.is_empty() =>
            {
                return Err(EvidenceV2Error::FalsePass);
            }
            GateDecisionKind::HumanReview if required != satisfied => {
                return Err(EvidenceV2Error::FalsePass);
            }
            GateDecisionKind::Block
                if required == satisfied && self.blocking_diagnostic_refs.is_empty() =>
            {
                return Err(EvidenceV2Error::Gate);
            }
            _ => {}
        }
        self.decision_fingerprint = fingerprint(
            "star.gate-decision",
            EVIDENCE_V2_SCHEMA_VERSION,
            &serde_json::json!({
                "gate_id":self.gate_id,
                "revision":self.revision,
                "validation_plan_ref":self.validation_plan_ref,
                "scope":self.scope,
                "decision":self.decision,
                "required_run_refs":self.required_run_refs,
                "satisfied_run_refs":self.satisfied_run_refs,
                "blocking_diagnostic_refs":self.blocking_diagnostic_refs,
                "reason_codes":self.reason_codes,
                "remaining_risks":self.remaining_risks,
                "policy_fingerprint":self.policy_fingerprint,
                "decided_by":self.decided_by,
                "decided_at":self.decided_at,
            }),
        )?;
        Ok(self)
    }

    pub fn authoritative_state(&self) -> AuthoritativeGateState {
        match self.decision {
            GateDecisionKind::AutoPass => AuthoritativeGateState::Passed,
            GateDecisionKind::HumanReview => AuthoritativeGateState::AwaitingHumanReview,
            GateDecisionKind::Block => AuthoritativeGateState::Blocked,
        }
    }

    pub fn reference(&self) -> Result<crate::evidence::GateDecisionRef, EvidenceV2Error> {
        Ok(crate::evidence::GateDecisionRef {
            gate_id: self.gate_id.clone(),
            revision: self.revision,
            sha256: document_hash(self)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvidenceBundleV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub evidence_bundle_id: EvidenceBundleId,
    pub revision: u64,
    pub task_spec_ref: crate::evidence::DocumentRef,
    pub scope_revision_ref: crate::evidence::DocumentRef,
    pub impact_analysis_ref: crate::evidence::DocumentRef,
    pub validation_plan_ref: crate::evidence::DocumentRef,
    pub validation_run_refs: Vec<ValidationRunRef>,
    pub diagnostic_refs: Vec<crate::evidence::DiagnosticRef>,
    pub gate_decision_ref: crate::evidence::GateDecisionRef,
    pub authoritative_gate_state: AuthoritativeGateState,
    pub remaining_risks: Vec<RiskRef>,
    pub artifact_manifest: ArtifactManifest,
    pub completeness: Completeness,
    pub missing_reasons: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub bundle_fingerprint: Sha256Hash,
}

impl EvidenceBundleV2 {
    pub fn seal(
        mut self,
        runs: &[ValidationRunV2],
        diagnostics: &[DiagnosticV2],
        gate: &GateDecisionV2,
    ) -> Result<Self, EvidenceV2Error> {
        self.validation_run_refs.sort();
        self.validation_run_refs.dedup();
        self.diagnostic_refs.sort();
        self.diagnostic_refs.dedup();
        self.missing_reasons.sort();
        self.missing_reasons.dedup();
        if self.schema_id != EVIDENCE_BUNDLE_V2_SCHEMA_ID
            || self.schema_version != EVIDENCE_V2_SCHEMA_VERSION
            || self.revision == 0
            || (self.completeness == Completeness::Complete && !self.missing_reasons.is_empty())
            || (self.completeness != Completeness::Complete && self.missing_reasons.is_empty())
            || self.gate_decision_ref != gate.reference()?
            || self.authoritative_gate_state != gate.authoritative_state()
        {
            return Err(EvidenceV2Error::Bundle);
        }
        let run_refs = runs
            .iter()
            .map(ValidationRunV2::reference)
            .collect::<Result<BTreeSet<_>, _>>()?;
        let diagnostic_refs = diagnostics
            .iter()
            .map(DiagnosticV2::reference)
            .collect::<Result<BTreeSet<_>, _>>()?;
        if self
            .validation_run_refs
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            != run_refs
            || self
                .diagnostic_refs
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                != diagnostic_refs
            || (self.authoritative_gate_state == AuthoritativeGateState::Passed
                && self.completeness != Completeness::Complete)
        {
            return Err(EvidenceV2Error::Bundle);
        }
        self.artifact_manifest
            .manifest_ref
            .validate()
            .map_err(|_| EvidenceV2Error::Artifact)?;
        let artifact_ids = self
            .artifact_manifest
            .artifacts
            .iter()
            .map(|artifact| &artifact.artifact_id)
            .collect::<BTreeSet<_>>();
        if artifact_ids.len() != self.artifact_manifest.artifacts.len() {
            return Err(EvidenceV2Error::Artifact);
        }
        let referenced_artifacts = runs.iter().flat_map(|run| run.artifact_refs.iter()).chain(
            diagnostics
                .iter()
                .flat_map(|diagnostic| diagnostic.evidence_refs.iter()),
        );
        for artifact in referenced_artifacts {
            if !self.artifact_manifest.artifacts.iter().any(|entry| {
                entry.artifact_id == artifact.artifact_id
                    && entry.sha256 == artifact.sha256
                    && entry.size_bytes == artifact.size_bytes
                    && entry.redaction_status == artifact.redaction_status
            }) {
                return Err(EvidenceV2Error::Artifact);
            }
        }
        self.bundle_fingerprint = fingerprint(
            "star.evidence-bundle",
            EVIDENCE_V2_SCHEMA_VERSION,
            &serde_json::json!({
                "evidence_bundle_id":self.evidence_bundle_id,
                "revision":self.revision,
                "task_spec_ref":self.task_spec_ref,
                "scope_revision_ref":self.scope_revision_ref,
                "impact_analysis_ref":self.impact_analysis_ref,
                "validation_plan_ref":self.validation_plan_ref,
                "validation_run_refs":self.validation_run_refs,
                "diagnostic_refs":self.diagnostic_refs,
                "gate_decision_ref":self.gate_decision_ref,
                "authoritative_gate_state":self.authoritative_gate_state,
                "remaining_risks":self.remaining_risks,
                "artifact_manifest":self.artifact_manifest,
                "completeness":self.completeness,
                "missing_reasons":self.missing_reasons,
                "created_at":self.created_at,
            }),
        )?;
        Ok(self)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceV2Error {
    #[error("task invocation v2 is invalid")]
    Invocation,
    #[error("validation run v2 is invalid")]
    Run,
    #[error("diagnostic v2 is invalid")]
    Diagnostic,
    #[error("gate decision v2 is invalid")]
    Gate,
    #[error("evidence bundle v2 is invalid")]
    Bundle,
    #[error("a pass claim is not supported by complete stable evidence")]
    FalsePass,
    #[error("artifact reference is invalid")]
    Artifact,
    #[error("canonical fingerprint could not be calculated")]
    Fingerprint,
}

pub fn empty_fingerprint() -> Sha256Hash {
    Sha256Hash::digest(b"")
}

fn fingerprint<T: Serialize>(
    domain: &str,
    version: u32,
    value: &T,
) -> Result<Sha256Hash, EvidenceV2Error> {
    canonical_sha256(&serde_json::json!({
        "domain":domain,
        "version":version,
        "value":value,
    }))
    .map_err(|_| EvidenceV2Error::Fingerprint)
}

fn document_hash<T: Serialize>(value: &T) -> Result<Sha256Hash, EvidenceV2Error> {
    let value = serde_json::to_value(value).map_err(|_| EvidenceV2Error::Fingerprint)?;
    canonical_sha256(&value).map_err(|_| EvidenceV2Error::Fingerprint)
}

pub fn plan_is_executable(readiness: ValidationPlanV2Readiness) -> bool {
    readiness == ValidationPlanV2Readiness::Ready
}
