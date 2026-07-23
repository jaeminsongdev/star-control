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
        DiagnosticConfidence, DiagnosticRef, DiagnosticSeverity, DiagnosticStatus, DocumentRef,
        EnvironmentValueRef, GateDecisionKind, GateDecisionRef, GateScope, ObservedTool,
        OutputLimits, RiskRef, TerminationReason, ValidationOutcome, ValidationRunRef,
    },
    ids::{
        BaselineId, CheckoutId, DiagnosticId, DispositionId, EvidenceBundleId, GateId, ProjectId,
        ProjectRevisionId, ReviewPackId, ReworkDirectiveId, SuppressionId, TaskInvocationId,
        ValidationResultId, ValidationRunId, WorkspaceSnapshotId,
    },
    management::{DispositionDecision, ProjectPathRef, SuppressionStatus},
    planning::{ObservedChangeKind, ValidationPlanV2Readiness},
};

pub const TASK_INVOCATION_V2_SCHEMA_ID: &str = "star.task-invocation";
pub const VALIDATION_RUN_V2_SCHEMA_ID: &str = "star.validation-run";
pub const DIAGNOSTIC_V2_SCHEMA_ID: &str = "star.diagnostic";
pub const GATE_DECISION_V2_SCHEMA_ID: &str = "star.gate-decision";
pub const EVIDENCE_BUNDLE_V2_SCHEMA_ID: &str = "star.evidence-bundle";
pub const VALIDATION_RESULT_V2_SCHEMA_ID: &str = "star.validation-result";
pub const BASELINE_V2_SCHEMA_ID: &str = "star.baseline";
pub const SUPPRESSION_V2_SCHEMA_ID: &str = "star.suppression";
pub const DISPOSITION_V2_SCHEMA_ID: &str = "star.disposition";
pub const REVIEW_PACK_SCHEMA_ID: &str = "star.review-pack";
pub const REWORK_DIRECTIVE_SCHEMA_ID: &str = "star.rework-directive";
pub const EVIDENCE_V2_SCHEMA_VERSION: u32 = 2;
pub const REVIEW_PACK_SCHEMA_VERSION: u32 = 1;
pub const REWORK_DIRECTIVE_SCHEMA_VERSION: u32 = 1;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceFreshnessV2 {
    Current,
    StaleSource,
    StalePlan,
    StaleConfig,
    StaleCatalog,
    StaleTool,
    StaleEnvironment,
    Unverified,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum GatePhaseV2 {
    DuringStage,
    GoalExit,
    PatchPreApply,
    PatchPostApply,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSubjectBinding {
    pub project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub project_revision_id: ProjectRevisionId,
    pub workspace_snapshot_id: WorkspaceSnapshotId,
    pub workspace_content_fingerprint: Sha256Hash,
    pub task_spec_ref: DocumentRef,
    pub scope_revision_ref: DocumentRef,
    pub impact_analysis_ref: DocumentRef,
    pub change_set_refs: Vec<DocumentRef>,
    pub change_plan_refs: Vec<DocumentRef>,
    pub patch_set_ref: Option<DocumentRef>,
    pub validation_plan_ref: DocumentRef,
    pub gate_phase: GatePhaseV2,
    pub profile_resolution_fingerprint: Sha256Hash,
    pub effective_config_fingerprint: Sha256Hash,
    pub gate_policy_fingerprint: Sha256Hash,
    pub catalog_snapshot_ref: DocumentRef,
    pub validator_registry_fingerprint: Sha256Hash,
    pub check_descriptor_ref: Option<DocumentRef>,
    pub rule_refs: Vec<CatalogRef>,
    pub tool_registry_snapshot_ref: Option<DocumentRef>,
    pub tool_descriptor_ref: Option<CatalogRef>,
    pub observed_tool_fingerprint: Option<Sha256Hash>,
    pub invocation_fingerprint: Option<Sha256Hash>,
    pub execution_environment_fingerprint: Sha256Hash,
    pub normalizer_fingerprint: Sha256Hash,
    pub freshness: EvidenceFreshnessV2,
    pub stale_reasons: Vec<String>,
    pub binding_fingerprint: Sha256Hash,
    pub probed_at: DateTime<Utc>,
}

impl EvidenceSubjectBinding {
    pub fn seal(mut self) -> Result<Self, EvidenceV2Error> {
        self.change_set_refs.sort_by(document_ref_order);
        self.change_set_refs.dedup();
        self.change_plan_refs.sort_by(document_ref_order);
        self.change_plan_refs.dedup();
        self.rule_refs.sort_by(catalog_ref_order);
        self.rule_refs.dedup();
        self.stale_reasons.sort();
        self.stale_reasons.dedup();
        if self.change_set_refs.is_empty()
            || (self.freshness == EvidenceFreshnessV2::Current && !self.stale_reasons.is_empty())
            || (self.freshness != EvidenceFreshnessV2::Current && self.stale_reasons.is_empty())
            || self.task_spec_ref.revision == 0
            || self.scope_revision_ref.revision == 0
            || self.impact_analysis_ref.revision == 0
            || self.validation_plan_ref.revision == 0
            || self.catalog_snapshot_ref.revision == 0
        {
            return Err(EvidenceV2Error::SubjectBinding);
        }
        self.binding_fingerprint = fingerprint(
            "star.evidence-subject-binding",
            EVIDENCE_V2_SCHEMA_VERSION,
            &serde_json::json!({
                "project_id":self.project_id,
                "checkout_id":self.checkout_id,
                "project_revision_id":self.project_revision_id,
                "workspace_snapshot_id":self.workspace_snapshot_id,
                "workspace_content_fingerprint":self.workspace_content_fingerprint,
                "task_spec_ref":self.task_spec_ref,
                "scope_revision_ref":self.scope_revision_ref,
                "impact_analysis_ref":self.impact_analysis_ref,
                "change_set_refs":self.change_set_refs,
                "change_plan_refs":self.change_plan_refs,
                "patch_set_ref":self.patch_set_ref,
                "validation_plan_ref":self.validation_plan_ref,
                "gate_phase":self.gate_phase,
                "profile_resolution_fingerprint":self.profile_resolution_fingerprint,
                "effective_config_fingerprint":self.effective_config_fingerprint,
                "gate_policy_fingerprint":self.gate_policy_fingerprint,
                "catalog_snapshot_ref":self.catalog_snapshot_ref,
                "validator_registry_fingerprint":self.validator_registry_fingerprint,
                "check_descriptor_ref":self.check_descriptor_ref,
                "rule_refs":self.rule_refs,
                "tool_registry_snapshot_ref":self.tool_registry_snapshot_ref,
                "tool_descriptor_ref":self.tool_descriptor_ref,
                "observed_tool_fingerprint":self.observed_tool_fingerprint,
                "invocation_fingerprint":self.invocation_fingerprint,
                "execution_environment_fingerprint":self.execution_environment_fingerprint,
                "normalizer_fingerprint":self.normalizer_fingerprint,
                "freshness":self.freshness,
                "stale_reasons":self.stale_reasons,
            }),
        )?;
        Ok(self)
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CompletionClaimKindV2 {
    Change,
    CheckExecuted,
    BugFixed,
    Compatibility,
    GeneratedCurrent,
    DocsCurrent,
    RegistryCurrent,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CompletionClaimSubjectV2 {
    Project {
        project_id: ProjectId,
    },
    Path {
        project_id: ProjectId,
        path: ProjectPathRef,
    },
    CheckPlan {
        project_id: ProjectId,
        plan_item_id: String,
        descriptor_ref: DocumentRef,
    },
    Document {
        project_id: ProjectId,
        document_ref: DocumentRef,
    },
}

impl CompletionClaimSubjectV2 {
    pub fn project_id(&self) -> &ProjectId {
        match self {
            Self::Project { project_id }
            | Self::Path { project_id, .. }
            | Self::CheckPlan { project_id, .. }
            | Self::Document { project_id, .. } => project_id,
        }
    }

    fn valid(&self) -> bool {
        match self {
            Self::Project { .. } | Self::Path { .. } => true,
            Self::CheckPlan {
                plan_item_id,
                descriptor_ref,
                ..
            } => !plan_item_id.trim().is_empty() && descriptor_ref.revision > 0,
            Self::Document { document_ref, .. } => document_ref.revision > 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CompletionAssertionV2 {
    Change {
        operation: ObservedChangeKind,
        after_sha256: Option<Sha256Hash>,
    },
    Pass,
    Fixed,
    Compatible,
    Current,
    Other {
        assertion_code: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClaimEvidenceRefV2 {
    Document {
        document_ref: DocumentRef,
    },
    ValidationRun {
        validation_run_ref: ValidationRunRef,
    },
    Artifact {
        artifact_ref: ArtifactRef,
    },
}

impl ClaimEvidenceRefV2 {
    fn valid(&self) -> bool {
        match self {
            Self::Document { document_ref } => document_ref.revision > 0,
            Self::ValidationRun { validation_run_ref } => validation_run_ref.revision > 0,
            Self::Artifact { artifact_ref } => artifact_ref.validate().is_ok(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompletionClaimRefV2 {
    pub claim_id: String,
    pub required: bool,
    pub claim_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompletionClaimV2 {
    pub claim_id: String,
    pub kind: CompletionClaimKindV2,
    pub subject: CompletionClaimSubjectV2,
    pub assertion: CompletionAssertionV2,
    pub required: bool,
    pub reported_evidence_refs: Vec<ClaimEvidenceRefV2>,
    pub reported_subject_binding: Option<EvidenceSubjectBinding>,
    pub source_actor: ActorRef,
    pub created_at: DateTime<Utc>,
    pub claim_fingerprint: Sha256Hash,
}

impl CompletionClaimV2 {
    pub fn seal(mut self) -> Result<Self, EvidenceV2Error> {
        self.reported_evidence_refs = sort_claim_evidence_refs(self.reported_evidence_refs)?;
        self.reported_evidence_refs.dedup();
        self.reported_subject_binding = self
            .reported_subject_binding
            .map(EvidenceSubjectBinding::seal)
            .transpose()?;
        let assertion_matches_kind = matches!(
            (&self.kind, &self.assertion),
            (
                CompletionClaimKindV2::Change,
                CompletionAssertionV2::Change { .. }
            ) | (
                CompletionClaimKindV2::CheckExecuted,
                CompletionAssertionV2::Pass
            ) | (
                CompletionClaimKindV2::BugFixed,
                CompletionAssertionV2::Fixed
            ) | (
                CompletionClaimKindV2::Compatibility,
                CompletionAssertionV2::Compatible
            ) | (
                CompletionClaimKindV2::GeneratedCurrent
                    | CompletionClaimKindV2::DocsCurrent
                    | CompletionClaimKindV2::RegistryCurrent,
                CompletionAssertionV2::Current
            ) | (
                CompletionClaimKindV2::Other,
                CompletionAssertionV2::Other { .. }
            )
        );
        let change_assertion_is_exact = match &self.assertion {
            CompletionAssertionV2::Change {
                operation: ObservedChangeKind::Delete,
                after_sha256,
            } => after_sha256.is_none(),
            CompletionAssertionV2::Change { after_sha256, .. } => after_sha256.is_some(),
            _ => true,
        };
        if self.claim_id.is_empty()
            || self.claim_id.len() > 128
            || !self
                .claim_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            || !self.subject.valid()
            || !assertion_matches_kind
            || !change_assertion_is_exact
            || matches!(
                &self.assertion,
                CompletionAssertionV2::Other { assertion_code }
                    if assertion_code.trim().is_empty()
            )
            || self
                .reported_evidence_refs
                .iter()
                .any(|reference| !reference.valid())
            || self.source_actor.actor_id.trim().is_empty()
            || self.source_actor.auth_source.trim().is_empty()
        {
            return Err(EvidenceV2Error::CompletionClaim);
        }
        self.claim_fingerprint = fingerprint(
            "star.completion-claim",
            EVIDENCE_V2_SCHEMA_VERSION,
            &serde_json::json!({
                "claim_id":self.claim_id,
                "kind":self.kind,
                "subject":self.subject,
                "assertion":self.assertion,
                "required":self.required,
                "reported_evidence_refs":self.reported_evidence_refs,
                "reported_subject_binding":self.reported_subject_binding,
                "source_actor":self.source_actor,
            }),
        )?;
        Ok(self)
    }

    pub fn reference(&self) -> CompletionClaimRefV2 {
        CompletionClaimRefV2 {
            claim_id: self.claim_id.clone(),
            required: self.required,
            claim_fingerprint: self.claim_fingerprint.clone(),
        }
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ClaimEvaluationStatusV2 {
    Verified,
    Contradicted,
    Unverified,
    Stale,
    NotApplicable,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ClaimGateEffectV2 {
    None,
    HumanReview,
    Block,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClaimEvaluationV2 {
    pub claim_ref: CompletionClaimRefV2,
    pub current_subject_binding: EvidenceSubjectBinding,
    pub actual_evidence_refs: Vec<ClaimEvidenceRefV2>,
    pub status: ClaimEvaluationStatusV2,
    pub diagnostic_refs: Vec<DiagnosticRef>,
    pub gate_effect: ClaimGateEffectV2,
    pub reason_codes: Vec<String>,
    pub evaluation_fingerprint: Sha256Hash,
}

impl ClaimEvaluationV2 {
    pub fn seal(mut self) -> Result<Self, EvidenceV2Error> {
        self.current_subject_binding = self.current_subject_binding.seal()?;
        self.actual_evidence_refs = sort_claim_evidence_refs(self.actual_evidence_refs)?;
        self.actual_evidence_refs.dedup();
        self.diagnostic_refs.sort();
        self.diagnostic_refs.dedup();
        self.reason_codes.sort();
        self.reason_codes.dedup();
        let expected_effect = match (self.claim_ref.required, self.status) {
            (_, ClaimEvaluationStatusV2::Verified | ClaimEvaluationStatusV2::NotApplicable) => {
                ClaimGateEffectV2::None
            }
            (true, ClaimEvaluationStatusV2::Contradicted | ClaimEvaluationStatusV2::Stale) => {
                ClaimGateEffectV2::Block
            }
            (true, ClaimEvaluationStatusV2::Unverified) => ClaimGateEffectV2::HumanReview,
            (false, _) => ClaimGateEffectV2::None,
        };
        if self.claim_ref.claim_id.trim().is_empty()
            || self.reason_codes.is_empty()
            || self
                .reason_codes
                .iter()
                .any(|reason| reason.trim().is_empty())
            || self.gate_effect != expected_effect
            || (!matches!(
                self.status,
                ClaimEvaluationStatusV2::Verified | ClaimEvaluationStatusV2::NotApplicable
            ) && self.diagnostic_refs.is_empty())
            || self
                .actual_evidence_refs
                .iter()
                .any(|reference| !reference.valid())
        {
            return Err(EvidenceV2Error::ClaimEvaluation);
        }
        self.evaluation_fingerprint = fingerprint(
            "star.claim-evaluation",
            EVIDENCE_V2_SCHEMA_VERSION,
            &serde_json::json!({
                "claim_ref":self.claim_ref,
                "current_subject_binding":self.current_subject_binding,
                "actual_evidence_refs":self.actual_evidence_refs,
                "status":self.status,
                "diagnostic_refs":self.diagnostic_refs,
                "gate_effect":self.gate_effect,
                "reason_codes":self.reason_codes,
            }),
        )?;
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BaselineEntryV2 {
    pub diagnostic_fingerprint: Sha256Hash,
    pub rule_ref: CatalogRef,
    pub severity: DiagnosticSeverity,
    pub scope_fingerprint: Sha256Hash,
    pub entry_fingerprint: Sha256Hash,
}

impl BaselineEntryV2 {
    pub fn seal(mut self) -> Result<Self, EvidenceV2Error> {
        self.entry_fingerprint = fingerprint(
            "star.baseline-entry",
            EVIDENCE_V2_SCHEMA_VERSION,
            &serde_json::json!({
                "diagnostic_fingerprint":self.diagnostic_fingerprint,
                "rule_ref":self.rule_ref,
                "severity":self.severity,
                "scope_fingerprint":self.scope_fingerprint,
            }),
        )?;
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BaselineV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub baseline_id: BaselineId,
    pub revision: u64,
    pub project_id: ProjectId,
    pub subject_binding_fingerprint: Sha256Hash,
    pub rule_set_fingerprint: Sha256Hash,
    pub entries: Vec<BaselineEntryV2>,
    pub created_at: DateTime<Utc>,
    pub reason: String,
    pub reviewed: bool,
    pub active: bool,
    pub set_fingerprint: Sha256Hash,
}

impl BaselineV2 {
    pub fn seal(mut self) -> Result<Self, EvidenceV2Error> {
        self.entries = self
            .entries
            .into_iter()
            .map(BaselineEntryV2::seal)
            .collect::<Result<Vec<_>, _>>()?;
        self.entries.sort_by(|left, right| {
            left.diagnostic_fingerprint
                .cmp(&right.diagnostic_fingerprint)
        });
        if self.schema_id != BASELINE_V2_SCHEMA_ID
            || self.schema_version != EVIDENCE_V2_SCHEMA_VERSION
            || self.revision == 0
            || self.reason.trim().is_empty()
            || self
                .entries
                .windows(2)
                .any(|pair| pair[0].diagnostic_fingerprint == pair[1].diagnostic_fingerprint)
        {
            return Err(EvidenceV2Error::Baseline);
        }
        self.set_fingerprint = fingerprint(
            "star.baseline",
            EVIDENCE_V2_SCHEMA_VERSION,
            &serde_json::json!({
                "baseline_id":self.baseline_id,
                "revision":self.revision,
                "project_id":self.project_id,
                "subject_binding_fingerprint":self.subject_binding_fingerprint,
                "rule_set_fingerprint":self.rule_set_fingerprint,
                "entries":self.entries,
                "created_at":self.created_at,
                "reason":self.reason,
                "reviewed":self.reviewed,
                "active":self.active,
            }),
        )?;
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SuppressionV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub suppression_id: SuppressionId,
    pub revision: u64,
    pub project_id: ProjectId,
    pub diagnostic_fingerprint: Option<Sha256Hash>,
    pub rule_ref: Option<CatalogRef>,
    pub scope_fingerprint: Sha256Hash,
    pub subject_binding_fingerprint: Sha256Hash,
    pub reason_code: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub permanent: bool,
    pub status: SuppressionStatus,
    pub content_fingerprint: Sha256Hash,
}

impl SuppressionV2 {
    pub fn seal(mut self) -> Result<Self, EvidenceV2Error> {
        if self.schema_id != SUPPRESSION_V2_SCHEMA_ID
            || self.schema_version != EVIDENCE_V2_SCHEMA_VERSION
            || self.revision == 0
            || (self.diagnostic_fingerprint.is_none() && self.rule_ref.is_none())
            || self.reason_code.trim().is_empty()
            || self.reason.trim().is_empty()
            || (self.permanent && self.expires_at.is_some())
            || self
                .expires_at
                .is_some_and(|expires| expires <= self.created_at)
        {
            return Err(EvidenceV2Error::Suppression);
        }
        self.content_fingerprint = fingerprint(
            "star.suppression",
            EVIDENCE_V2_SCHEMA_VERSION,
            &serde_json::json!({
                "suppression_id":self.suppression_id,
                "revision":self.revision,
                "project_id":self.project_id,
                "diagnostic_fingerprint":self.diagnostic_fingerprint,
                "rule_ref":self.rule_ref,
                "scope_fingerprint":self.scope_fingerprint,
                "subject_binding_fingerprint":self.subject_binding_fingerprint,
                "reason_code":self.reason_code,
                "reason":self.reason,
                "created_at":self.created_at,
                "expires_at":self.expires_at,
                "permanent":self.permanent,
                "status":self.status,
            }),
        )?;
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DispositionV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub disposition_id: DispositionId,
    pub revision: u64,
    pub project_id: ProjectId,
    pub diagnostic_fingerprint: Sha256Hash,
    pub subject_binding_fingerprint: Sha256Hash,
    pub decision: DispositionDecision,
    pub reason_code: String,
    pub reason: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub active: bool,
    pub decided_at: DateTime<Utc>,
    pub content_fingerprint: Sha256Hash,
}

impl DispositionV2 {
    pub fn seal(mut self) -> Result<Self, EvidenceV2Error> {
        if self.schema_id != DISPOSITION_V2_SCHEMA_ID
            || self.schema_version != EVIDENCE_V2_SCHEMA_VERSION
            || self.revision == 0
            || self.reason_code.trim().is_empty()
            || self.reason.trim().is_empty()
            || self
                .expires_at
                .is_some_and(|expires| expires <= self.decided_at)
        {
            return Err(EvidenceV2Error::Disposition);
        }
        self.content_fingerprint = fingerprint(
            "star.disposition",
            EVIDENCE_V2_SCHEMA_VERSION,
            &serde_json::json!({
                "disposition_id":self.disposition_id,
                "revision":self.revision,
                "project_id":self.project_id,
                "diagnostic_fingerprint":self.diagnostic_fingerprint,
                "subject_binding_fingerprint":self.subject_binding_fingerprint,
                "decision":self.decision,
                "reason_code":self.reason_code,
                "reason":self.reason,
                "expires_at":self.expires_at,
                "active":self.active,
                "decided_at":self.decided_at,
            }),
        )?;
        Ok(self)
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BaselineRelationV2 {
    New,
    ExistingUnchanged,
    Worsened,
    Improved,
    NotObserved,
    Incompatible,
    Unbaselined,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SuppressionStateV2 {
    None,
    Active,
    Expired,
    Stale,
    Revoked,
    Invalid,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticGateEffectV2 {
    None,
    RemainingRisk,
    RequiresReview,
    Blocks,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DiagnosticEvaluationSubjectV2 {
    CurrentDiagnostic {
        diagnostic_ref: DiagnosticRef,
    },
    BaselineEntry {
        baseline_id: BaselineId,
        revision: u64,
        entry_fingerprint: Sha256Hash,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionDocumentRefV2 {
    pub document_id: String,
    pub revision: u64,
    pub fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticEvaluationV2 {
    pub evaluation_subject: DiagnosticEvaluationSubjectV2,
    pub subject_binding_fingerprint: Sha256Hash,
    pub baseline_relation: BaselineRelationV2,
    pub baseline_ref: Option<DecisionDocumentRefV2>,
    pub suppression_state: SuppressionStateV2,
    pub suppression_ref: Option<DecisionDocumentRefV2>,
    pub disposition_ref: Option<DecisionDocumentRefV2>,
    pub gate_effect: DiagnosticGateEffectV2,
    pub reason_codes: Vec<String>,
    pub evaluation_fingerprint: Sha256Hash,
}

impl DiagnosticEvaluationV2 {
    pub fn seal(mut self) -> Result<Self, EvidenceV2Error> {
        self.reason_codes.sort();
        self.reason_codes.dedup();
        if self.reason_codes.is_empty()
            || self
                .reason_codes
                .iter()
                .any(|reason| reason.trim().is_empty())
            || (self.suppression_state == SuppressionStateV2::None
                && self.suppression_ref.is_some())
            || (self.suppression_state != SuppressionStateV2::None
                && self.suppression_ref.is_none())
        {
            return Err(EvidenceV2Error::DiagnosticEvaluation);
        }
        self.evaluation_fingerprint = fingerprint(
            "star.diagnostic-evaluation",
            EVIDENCE_V2_SCHEMA_VERSION,
            &serde_json::json!({
                "evaluation_subject":self.evaluation_subject,
                "subject_binding_fingerprint":self.subject_binding_fingerprint,
                "baseline_relation":self.baseline_relation,
                "baseline_ref":self.baseline_ref,
                "suppression_state":self.suppression_state,
                "suppression_ref":self.suppression_ref,
                "disposition_ref":self.disposition_ref,
                "gate_effect":self.gate_effect,
                "reason_codes":self.reason_codes,
            }),
        )?;
        Ok(self)
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CheckRequirementV2 {
    Required,
    Optional,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RunSatisfactionStateV2 {
    CleanPass,
    RatchetSatisfied,
    Unsatisfied,
    WaivedForReview,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RunGateEffectV2 {
    None,
    HumanReview,
    Block,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunSatisfactionV2 {
    pub plan_item_id: String,
    pub requirement: CheckRequirementV2,
    pub validation_run_refs: Vec<ValidationRunRef>,
    pub raw_outcomes: Vec<ValidationOutcome>,
    pub satisfaction: RunSatisfactionStateV2,
    pub gate_effect: RunGateEffectV2,
    pub reason_code: String,
    pub diagnostic_evaluation_fingerprints: Vec<Sha256Hash>,
    pub policy_reason: String,
    pub content_fingerprint: Sha256Hash,
}

impl RunSatisfactionV2 {
    pub fn seal(mut self) -> Result<Self, EvidenceV2Error> {
        self.validation_run_refs.sort();
        self.validation_run_refs.dedup();
        self.diagnostic_evaluation_fingerprints.sort();
        self.diagnostic_evaluation_fingerprints.dedup();
        if self.plan_item_id.trim().is_empty()
            || self.validation_run_refs.is_empty()
            || self.raw_outcomes.is_empty()
            || self.reason_code.trim().is_empty()
            || self.policy_reason.trim().is_empty()
            || (matches!(
                self.satisfaction,
                RunSatisfactionStateV2::CleanPass | RunSatisfactionStateV2::RatchetSatisfied
            ) && self.gate_effect != RunGateEffectV2::None)
            || (matches!(
                self.satisfaction,
                RunSatisfactionStateV2::Unsatisfied | RunSatisfactionStateV2::WaivedForReview
            ) && self.requirement == CheckRequirementV2::Required
                && self.gate_effect == RunGateEffectV2::None)
        {
            return Err(EvidenceV2Error::RunSatisfaction);
        }
        self.content_fingerprint = fingerprint(
            "star.run-satisfaction",
            EVIDENCE_V2_SCHEMA_VERSION,
            &serde_json::json!({
                "plan_item_id":self.plan_item_id,
                "requirement":self.requirement,
                "validation_run_refs":self.validation_run_refs,
                "raw_outcomes":self.raw_outcomes,
                "satisfaction":self.satisfaction,
                "gate_effect":self.gate_effect,
                "reason_code":self.reason_code,
                "diagnostic_evaluation_fingerprints":self.diagnostic_evaluation_fingerprints,
                "policy_reason":self.policy_reason,
            }),
        )?;
        Ok(self)
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStabilityV2 {
    Stable,
    Flaky,
    NotEvaluated,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ProcessStartStateV2 {
    NotStarted,
    Started,
    Unknown,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum NotRunReasonV2 {
    DependencyUnsatisfied,
    PermissionBlocked,
    ToolUnavailable,
    PreflightInvalidated,
    CancelledBeforeStart,
    LaunchError,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InvocationWorkingDirectoryV2 {
    ProjectRoot,
    ProjectPath { path: ProjectPathRef },
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
    pub cwd: InvocationWorkingDirectoryV2,
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
    pub validation_plan_ref: DocumentRef,
    pub check_ref: DocumentRef,
    pub subject_binding: EvidenceSubjectBinding,
    pub plan_item_id: String,
    pub project_id: ProjectId,
    pub phase: String,
    pub attempt: u32,
    pub invocation: TaskInvocationV2,
    pub process_start_state: ProcessStartStateV2,
    pub not_run_reason: Option<NotRunReasonV2>,
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
        self.subject_binding.invocation_fingerprint =
            Some(self.invocation.input_fingerprint.clone());
        self.subject_binding.observed_tool_fingerprint = self
            .observed_tool
            .as_ref()
            .map(|tool| fingerprint("star.observed-tool", EVIDENCE_V2_SCHEMA_VERSION, tool))
            .transpose()?;
        self.subject_binding = self.subject_binding.seal()?;
        self.diagnostic_ids.sort();
        self.diagnostic_ids.dedup();
        self.artifact_refs
            .sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
        if self.schema_id != VALIDATION_RUN_V2_SCHEMA_ID
            || self.schema_version != EVIDENCE_V2_SCHEMA_VERSION
            || self.revision == 0
            || self.validation_plan_ref != self.subject_binding.validation_plan_ref
            || self.subject_binding.project_id != self.project_id
            || self.subject_binding.check_descriptor_ref.as_ref() != Some(&self.check_ref)
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
                || self.process_start_state != ProcessStartStateV2::NotStarted
                || self.not_run_reason.is_none()
                || self.stability != ValidationStabilityV2::NotEvaluated)
        {
            return Err(EvidenceV2Error::Run);
        }
        if self.outcome != ValidationOutcome::NotRun && self.not_run_reason.is_some() {
            return Err(EvidenceV2Error::Run);
        }
        if self.outcome != ValidationOutcome::NotRun
            && self.process_start_state == ProcessStartStateV2::NotStarted
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
                "check_ref":self.check_ref,
                "subject_binding":self.subject_binding,
                "plan_item_id":self.plan_item_id,
                "project_id":self.project_id,
                "phase":self.phase,
                "attempt":self.attempt,
                "invocation":self.invocation,
                "process_start_state":self.process_start_state,
                "not_run_reason":self.not_run_reason,
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
            && self.process_start_state == ProcessStartStateV2::Started
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
pub struct ValidationResultV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub validation_result_id: ValidationResultId,
    pub revision: u64,
    pub validation_plan_ref: DocumentRef,
    pub project_id: ProjectId,
    pub subject_binding: EvidenceSubjectBinding,
    pub validation_run_refs: Vec<ValidationRunRef>,
    pub outcome: ValidationOutcome,
    pub completeness: Completeness,
    pub freshness: EvidenceFreshnessV2,
    pub stale_reasons: Vec<String>,
    pub stability: ValidationStabilityV2,
    pub run_satisfactions: Vec<RunSatisfactionV2>,
    pub normalizer_fingerprint: Sha256Hash,
    pub created_at: DateTime<Utc>,
    pub result_fingerprint: Sha256Hash,
}

impl ValidationResultV2 {
    pub fn seal(mut self, runs: &[ValidationRunV2]) -> Result<Self, EvidenceV2Error> {
        self.subject_binding = self.subject_binding.seal()?;
        self.validation_run_refs.sort();
        self.validation_run_refs.dedup();
        self.stale_reasons.sort();
        self.stale_reasons.dedup();
        self.run_satisfactions = self
            .run_satisfactions
            .into_iter()
            .map(RunSatisfactionV2::seal)
            .collect::<Result<Vec<_>, _>>()?;
        self.run_satisfactions
            .sort_by(|left, right| left.plan_item_id.cmp(&right.plan_item_id));
        let actual_refs = runs
            .iter()
            .filter(|run| run.project_id == self.project_id)
            .map(ValidationRunV2::reference)
            .collect::<Result<BTreeSet<_>, _>>()?;
        let selected_refs = self
            .validation_run_refs
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let selected_items = runs
            .iter()
            .filter(|run| run.project_id == self.project_id)
            .map(|run| run.plan_item_id.as_str())
            .collect::<BTreeSet<_>>();
        let satisfaction_items = self
            .run_satisfactions
            .iter()
            .map(|item| item.plan_item_id.as_str())
            .collect::<BTreeSet<_>>();
        if self.schema_id != VALIDATION_RESULT_V2_SCHEMA_ID
            || self.schema_version != EVIDENCE_V2_SCHEMA_VERSION
            || self.revision == 0
            || self.validation_plan_ref != self.subject_binding.validation_plan_ref
            || self.project_id != self.subject_binding.project_id
            || selected_refs != actual_refs
            || satisfaction_items != selected_items
            || self.run_satisfactions.len() != selected_items.len()
            || (self.freshness == EvidenceFreshnessV2::Current && !self.stale_reasons.is_empty())
            || (self.freshness != EvidenceFreshnessV2::Current && self.stale_reasons.is_empty())
        {
            return Err(EvidenceV2Error::ValidationResult);
        }
        let positive = self.run_satisfactions.iter().all(|item| {
            matches!(
                item.satisfaction,
                RunSatisfactionStateV2::CleanPass | RunSatisfactionStateV2::RatchetSatisfied
            ) && item.gate_effect == RunGateEffectV2::None
        });
        if self.outcome == ValidationOutcome::Pass
            && (!positive
                || self.completeness != Completeness::Complete
                || self.freshness != EvidenceFreshnessV2::Current
                || self.stability != ValidationStabilityV2::Stable)
        {
            return Err(EvidenceV2Error::FalsePass);
        }
        self.result_fingerprint = fingerprint(
            "star.validation-result",
            EVIDENCE_V2_SCHEMA_VERSION,
            &serde_json::json!({
                "validation_result_id":self.validation_result_id,
                "revision":self.revision,
                "validation_plan_ref":self.validation_plan_ref,
                "project_id":self.project_id,
                "subject_binding":self.subject_binding,
                "validation_run_refs":self.validation_run_refs,
                "outcome":self.outcome,
                "completeness":self.completeness,
                "freshness":self.freshness,
                "stale_reasons":self.stale_reasons,
                "stability":self.stability,
                "run_satisfactions":self.run_satisfactions,
                "normalizer_fingerprint":self.normalizer_fingerprint,
                "created_at":self.created_at,
            }),
        )?;
        Ok(self)
    }

    pub fn reference(&self) -> Result<DocumentRef, EvidenceV2Error> {
        Ok(DocumentRef {
            schema_id: VALIDATION_RESULT_V2_SCHEMA_ID.to_owned(),
            document_id: self.validation_result_id.to_string(),
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
                "severity":self.severity,
                "project_id":self.project_id,
                "plan_item_id":self.plan_item_id,
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
    pub subject_binding_set_fingerprint: Sha256Hash,
    pub scope: GateScope,
    pub decision: GateDecisionKind,
    pub validation_result_refs: Vec<DocumentRef>,
    pub required_run_refs: Vec<ValidationRunRef>,
    pub satisfied_run_refs: Vec<ValidationRunRef>,
    pub diagnostic_evaluations: Vec<DiagnosticEvaluationV2>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claim_evaluations: Vec<ClaimEvaluationV2>,
    pub run_satisfactions: Vec<RunSatisfactionV2>,
    pub blocking_diagnostic_refs: Vec<crate::evidence::DiagnosticRef>,
    pub reason_codes: Vec<String>,
    pub remaining_risks: Vec<RiskRef>,
    pub policy_fingerprint: Sha256Hash,
    pub decided_by: ActorRef,
    pub decided_at: DateTime<Utc>,
    pub valid_until: Option<DateTime<Utc>>,
    pub decision_fingerprint: Sha256Hash,
}

impl GateDecisionV2 {
    pub fn seal(
        mut self,
        runs: &[ValidationRunV2],
        diagnostics: &[DiagnosticV2],
        results: &[ValidationResultV2],
    ) -> Result<Self, EvidenceV2Error> {
        self.validation_result_refs.sort_by(document_ref_order);
        self.validation_result_refs.dedup();
        self.required_run_refs.sort();
        self.required_run_refs.dedup();
        self.satisfied_run_refs.sort();
        self.satisfied_run_refs.dedup();
        self.blocking_diagnostic_refs.sort();
        self.blocking_diagnostic_refs.dedup();
        self.reason_codes.sort();
        self.reason_codes.dedup();
        self.diagnostic_evaluations = self
            .diagnostic_evaluations
            .into_iter()
            .map(DiagnosticEvaluationV2::seal)
            .collect::<Result<Vec<_>, _>>()?;
        self.diagnostic_evaluations.sort_by(|left, right| {
            left.evaluation_fingerprint
                .cmp(&right.evaluation_fingerprint)
        });
        self.claim_evaluations = self
            .claim_evaluations
            .into_iter()
            .map(ClaimEvaluationV2::seal)
            .collect::<Result<Vec<_>, _>>()?;
        self.claim_evaluations
            .sort_by(|left, right| left.claim_ref.claim_id.cmp(&right.claim_ref.claim_id));
        self.run_satisfactions = self
            .run_satisfactions
            .into_iter()
            .map(RunSatisfactionV2::seal)
            .collect::<Result<Vec<_>, _>>()?;
        self.run_satisfactions
            .sort_by(|left, right| left.plan_item_id.cmp(&right.plan_item_id));
        if self.schema_id != GATE_DECISION_V2_SCHEMA_ID
            || self.schema_version != EVIDENCE_V2_SCHEMA_VERSION
            || self.revision == 0
            || self.reason_codes.is_empty()
            || self
                .reason_codes
                .iter()
                .any(|reason| reason.trim().is_empty())
            || self
                .valid_until
                .is_some_and(|until| until <= self.decided_at)
            || self
                .claim_evaluations
                .windows(2)
                .any(|pair| pair[0].claim_ref.claim_id == pair[1].claim_ref.claim_id)
        {
            return Err(EvidenceV2Error::Gate);
        }
        let actual_result_refs = results
            .iter()
            .map(ValidationResultV2::reference)
            .collect::<Result<BTreeSet<_>, _>>()?;
        if self
            .validation_result_refs
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            != actual_result_refs
        {
            return Err(EvidenceV2Error::Gate);
        }
        let binding_set = results
            .iter()
            .map(|result| result.subject_binding.binding_fingerprint.clone())
            .collect::<BTreeSet<_>>();
        if self.subject_binding_set_fingerprint
            != fingerprint(
                "star.evidence-subject-binding-set",
                EVIDENCE_V2_SCHEMA_VERSION,
                &binding_set,
            )?
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
        if self.claim_evaluations.iter().any(|evaluation| {
            evaluation
                .diagnostic_refs
                .iter()
                .any(|reference| !actual_diagnostics.contains(reference))
        }) {
            return Err(EvidenceV2Error::Gate);
        }
        let run_items = runs
            .iter()
            .map(|run| run.plan_item_id.as_str())
            .collect::<BTreeSet<_>>();
        let satisfaction_items = self
            .run_satisfactions
            .iter()
            .map(|item| item.plan_item_id.as_str())
            .collect::<BTreeSet<_>>();
        if run_items != satisfaction_items || self.run_satisfactions.len() != run_items.len() {
            return Err(EvidenceV2Error::Gate);
        }
        let effects = self
            .run_satisfactions
            .iter()
            .map(|item| item.gate_effect)
            .collect::<BTreeSet<_>>();
        let claim_effects = self
            .claim_evaluations
            .iter()
            .map(|evaluation| evaluation.gate_effect)
            .collect::<BTreeSet<_>>();
        match self.decision {
            GateDecisionKind::AutoPass
                if !self.blocking_diagnostic_refs.is_empty()
                    || effects
                        .iter()
                        .any(|effect| *effect != RunGateEffectV2::None)
                    || claim_effects
                        .iter()
                        .any(|effect| *effect != ClaimGateEffectV2::None)
                    || results.iter().any(|result| {
                        result.completeness != Completeness::Complete
                            || result.freshness != EvidenceFreshnessV2::Current
                            || result.stability != ValidationStabilityV2::Stable
                            || result.run_satisfactions.iter().any(|item| {
                                !matches!(
                                    item.satisfaction,
                                    RunSatisfactionStateV2::CleanPass
                                        | RunSatisfactionStateV2::RatchetSatisfied
                                )
                            })
                    }) =>
            {
                return Err(EvidenceV2Error::FalsePass);
            }
            GateDecisionKind::HumanReview
                if effects.contains(&RunGateEffectV2::Block)
                    || claim_effects.contains(&ClaimGateEffectV2::Block)
                    || (!effects.contains(&RunGateEffectV2::HumanReview)
                        && !claim_effects.contains(&ClaimGateEffectV2::HumanReview)
                        && !self
                            .reason_codes
                            .iter()
                            .any(|reason| reason == "INDEPENDENT_REVIEW_REQUIRED")) =>
            {
                return Err(EvidenceV2Error::FalsePass);
            }
            GateDecisionKind::Block
                if required == satisfied
                    && self.blocking_diagnostic_refs.is_empty()
                    && !effects.contains(&RunGateEffectV2::Block)
                    && !claim_effects.contains(&ClaimGateEffectV2::Block) =>
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
                "subject_binding_set_fingerprint":self.subject_binding_set_fingerprint,
                "scope":self.scope,
                "decision":self.decision,
                "validation_result_refs":self.validation_result_refs,
                "required_run_refs":self.required_run_refs,
                "satisfied_run_refs":self.satisfied_run_refs,
                "diagnostic_evaluations":self.diagnostic_evaluations,
                "claim_evaluations":self.claim_evaluations,
                "run_satisfactions":self.run_satisfactions,
                "blocking_diagnostic_refs":self.blocking_diagnostic_refs,
                "reason_codes":self.reason_codes,
                "remaining_risks":self.remaining_risks,
                "policy_fingerprint":self.policy_fingerprint,
                "decided_by":self.decided_by,
                "decided_at":self.decided_at,
                "valid_until":self.valid_until,
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
    pub subject_binding_set_fingerprint: Sha256Hash,
    pub validation_run_refs: Vec<ValidationRunRef>,
    pub validation_result_refs: Vec<DocumentRef>,
    pub diagnostic_refs: Vec<crate::evidence::DiagnosticRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub completion_claims: Vec<CompletionClaimV2>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claim_evaluations: Vec<ClaimEvaluationV2>,
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
        results: &[ValidationResultV2],
        diagnostics: &[DiagnosticV2],
        gate: &GateDecisionV2,
    ) -> Result<Self, EvidenceV2Error> {
        self.completion_claims = self
            .completion_claims
            .into_iter()
            .map(CompletionClaimV2::seal)
            .collect::<Result<Vec<_>, _>>()?;
        self.completion_claims
            .sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
        self.claim_evaluations = self
            .claim_evaluations
            .into_iter()
            .map(ClaimEvaluationV2::seal)
            .collect::<Result<Vec<_>, _>>()?;
        self.claim_evaluations
            .sort_by(|left, right| left.claim_ref.claim_id.cmp(&right.claim_ref.claim_id));
        self.validation_run_refs.sort();
        self.validation_run_refs.dedup();
        self.validation_result_refs.sort_by(document_ref_order);
        self.validation_result_refs.dedup();
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
            || self.subject_binding_set_fingerprint != gate.subject_binding_set_fingerprint
            || self.claim_evaluations != gate.claim_evaluations
            || self
                .completion_claims
                .windows(2)
                .any(|pair| pair[0].claim_id == pair[1].claim_id)
        {
            return Err(EvidenceV2Error::Bundle);
        }
        let claim_refs = self
            .completion_claims
            .iter()
            .map(CompletionClaimV2::reference)
            .collect::<BTreeSet<_>>();
        let evaluated_claim_refs = self
            .claim_evaluations
            .iter()
            .map(|evaluation| evaluation.claim_ref.clone())
            .collect::<BTreeSet<_>>();
        if claim_refs != evaluated_claim_refs
            || self.claim_evaluations.len() != evaluated_claim_refs.len()
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
        let result_refs = results
            .iter()
            .map(ValidationResultV2::reference)
            .collect::<Result<BTreeSet<_>, _>>()?;
        if self
            .validation_run_refs
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            != run_refs
            || self
                .validation_result_refs
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                != result_refs
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
                "subject_binding_set_fingerprint":self.subject_binding_set_fingerprint,
                "validation_run_refs":self.validation_run_refs,
                "validation_result_refs":self.validation_result_refs,
                "diagnostic_refs":self.diagnostic_refs,
                "completion_claims":self.completion_claims,
                "claim_evaluations":self.claim_evaluations,
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

    pub fn reference(&self) -> Result<DocumentRef, EvidenceV2Error> {
        Ok(DocumentRef {
            schema_id: EVIDENCE_BUNDLE_V2_SCHEMA_ID.to_owned(),
            document_id: self.evidence_bundle_id.to_string(),
            revision: self.revision,
            sha256: document_hash(self)?,
        })
    }
}

pub const REVIEW_PACK_SECTION_ORDER: [&str; 9] = [
    "request_and_completion_criteria",
    "planned_vs_actual_changes",
    "completion_claims",
    "check_results",
    "diagnostic_relations",
    "quality_security_highlights",
    "gate_decision",
    "remaining_risks_and_questions",
    "evidence_identity",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewPackItemV1 {
    pub item_kind: String,
    pub status: String,
    pub summary: String,
    pub evidence_refs: Vec<DocumentRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewPackSectionV1 {
    pub key: String,
    pub items: Vec<ReviewPackItemV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewQuestionV1 {
    pub question_id: String,
    pub prompt: String,
    pub options: Vec<String>,
    pub impact: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewPackV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub review_pack_id: ReviewPackId,
    pub revision: u64,
    pub evidence_bundle_ref: DocumentRef,
    pub authoritative_gate_decision_ref: GateDecisionRef,
    pub section_order: Vec<String>,
    pub sections: Vec<ReviewPackSectionV1>,
    pub questions: Vec<ReviewQuestionV1>,
    pub required_action_refs: Vec<DocumentRef>,
    pub rendered_artifact_refs: Vec<ArtifactRef>,
    pub completeness: Completeness,
    pub missing_reasons: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub review_pack_fingerprint: Sha256Hash,
}

impl ReviewPackV1 {
    pub fn seal(
        mut self,
        bundle: &EvidenceBundleV2,
        gate: &GateDecisionV2,
    ) -> Result<Self, EvidenceV2Error> {
        self.required_action_refs.sort_by(document_ref_order);
        self.required_action_refs.dedup();
        self.rendered_artifact_refs
            .sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
        self.missing_reasons.sort();
        self.missing_reasons.dedup();
        let expected_order = REVIEW_PACK_SECTION_ORDER
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>();
        let section_keys = self
            .sections
            .iter()
            .map(|section| section.key.clone())
            .collect::<Vec<_>>();
        if self.schema_id != REVIEW_PACK_SCHEMA_ID
            || self.schema_version != REVIEW_PACK_SCHEMA_VERSION
            || self.revision == 0
            || self.evidence_bundle_ref != bundle.reference()?
            || self.authoritative_gate_decision_ref != gate.reference()?
            || self.section_order != expected_order
            || section_keys != expected_order
            || self
                .sections
                .iter()
                .flat_map(|section| section.items.iter())
                .any(|item| {
                    item.item_kind.trim().is_empty()
                        || item.status.trim().is_empty()
                        || item.summary.trim().is_empty()
                })
            || self.questions.iter().any(|question| {
                question.question_id.trim().is_empty()
                    || question.prompt.trim().is_empty()
                    || question.impact.trim().is_empty()
            })
            || self
                .rendered_artifact_refs
                .iter()
                .any(|artifact| artifact.validate().is_err())
            || completeness_rank(self.completeness) > completeness_rank(bundle.completeness)
            || (self.completeness == Completeness::Complete && !self.missing_reasons.is_empty())
            || (self.completeness != Completeness::Complete && self.missing_reasons.is_empty())
        {
            return Err(EvidenceV2Error::ReviewPack);
        }
        self.review_pack_fingerprint = fingerprint(
            "star.review-pack",
            REVIEW_PACK_SCHEMA_VERSION,
            &serde_json::json!({
                "review_pack_id":self.review_pack_id,
                "revision":self.revision,
                "evidence_bundle_ref":self.evidence_bundle_ref,
                "authoritative_gate_decision_ref":self.authoritative_gate_decision_ref,
                "section_order":self.section_order,
                "sections":self.sections,
                "questions":self.questions,
                "required_action_refs":self.required_action_refs,
                "rendered_artifact_refs":self.rendered_artifact_refs,
                "completeness":self.completeness,
                "missing_reasons":self.missing_reasons,
                "created_at":self.created_at,
            }),
        )?;
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReworkDirectiveV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub rework_directive_id: ReworkDirectiveId,
    pub revision: u64,
    pub gate_decision_ref: GateDecisionRef,
    pub blocking_diagnostic_refs: Vec<DiagnosticRef>,
    pub failed_or_missing_plan_item_ids: Vec<String>,
    pub expected_actual_differences: Vec<String>,
    pub safe_remediations: Vec<String>,
    pub required_rechecks: Vec<String>,
    pub replan_required: bool,
    pub rerunnable_same_plan: bool,
    pub created_at: DateTime<Utc>,
    pub directive_fingerprint: Sha256Hash,
}

impl ReworkDirectiveV1 {
    pub fn seal(mut self, gate: &GateDecisionV2) -> Result<Self, EvidenceV2Error> {
        self.blocking_diagnostic_refs.sort();
        self.blocking_diagnostic_refs.dedup();
        self.failed_or_missing_plan_item_ids.sort();
        self.failed_or_missing_plan_item_ids.dedup();
        self.expected_actual_differences.sort();
        self.expected_actual_differences.dedup();
        self.safe_remediations.sort();
        self.safe_remediations.dedup();
        self.required_rechecks.sort();
        self.required_rechecks.dedup();
        if self.schema_id != REWORK_DIRECTIVE_SCHEMA_ID
            || self.schema_version != REWORK_DIRECTIVE_SCHEMA_VERSION
            || self.revision == 0
            || gate.decision != GateDecisionKind::Block
            || self.gate_decision_ref != gate.reference()?
            || self.blocking_diagnostic_refs != gate.blocking_diagnostic_refs
            || self.failed_or_missing_plan_item_ids.is_empty()
            || self.safe_remediations.is_empty()
            || self.required_rechecks.is_empty()
            || (self.replan_required && self.rerunnable_same_plan)
        {
            return Err(EvidenceV2Error::ReworkDirective);
        }
        self.directive_fingerprint = fingerprint(
            "star.rework-directive",
            REWORK_DIRECTIVE_SCHEMA_VERSION,
            &serde_json::json!({
                "rework_directive_id":self.rework_directive_id,
                "revision":self.revision,
                "gate_decision_ref":self.gate_decision_ref,
                "blocking_diagnostic_refs":self.blocking_diagnostic_refs,
                "failed_or_missing_plan_item_ids":self.failed_or_missing_plan_item_ids,
                "expected_actual_differences":self.expected_actual_differences,
                "safe_remediations":self.safe_remediations,
                "required_rechecks":self.required_rechecks,
                "replan_required":self.replan_required,
                "rerunnable_same_plan":self.rerunnable_same_plan,
                "created_at":self.created_at,
            }),
        )?;
        Ok(self)
    }

    pub fn reference(&self) -> Result<DocumentRef, EvidenceV2Error> {
        Ok(DocumentRef {
            schema_id: REWORK_DIRECTIVE_SCHEMA_ID.to_owned(),
            document_id: self.rework_directive_id.to_string(),
            revision: self.revision,
            sha256: document_hash(self)?,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceV2Error {
    #[error("evidence subject binding is invalid")]
    SubjectBinding,
    #[error("completion claim is invalid")]
    CompletionClaim,
    #[error("completion claim evaluation is invalid")]
    ClaimEvaluation,
    #[error("baseline v2 is invalid")]
    Baseline,
    #[error("suppression v2 is invalid")]
    Suppression,
    #[error("disposition v2 is invalid")]
    Disposition,
    #[error("task invocation v2 is invalid")]
    Invocation,
    #[error("validation run v2 is invalid")]
    Run,
    #[error("diagnostic v2 is invalid")]
    Diagnostic,
    #[error("diagnostic evaluation is invalid")]
    DiagnosticEvaluation,
    #[error("run satisfaction is invalid")]
    RunSatisfaction,
    #[error("validation result v2 is invalid")]
    ValidationResult,
    #[error("gate decision v2 is invalid")]
    Gate,
    #[error("evidence bundle v2 is invalid")]
    Bundle,
    #[error("review pack is invalid")]
    ReviewPack,
    #[error("rework directive is invalid")]
    ReworkDirective,
    #[error("a pass claim is not supported by complete stable evidence")]
    FalsePass,
    #[error("artifact reference is invalid")]
    Artifact,
    #[error("canonical fingerprint could not be calculated")]
    Fingerprint,
}

fn sort_claim_evidence_refs(
    references: Vec<ClaimEvidenceRefV2>,
) -> Result<Vec<ClaimEvidenceRefV2>, EvidenceV2Error> {
    let mut keyed = references
        .into_iter()
        .map(|reference| {
            fingerprint(
                "star.claim-evidence-ref",
                EVIDENCE_V2_SCHEMA_VERSION,
                &reference,
            )
            .map(|key| (key, reference))
        })
        .collect::<Result<Vec<_>, _>>()?;
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(keyed.into_iter().map(|(_, reference)| reference).collect())
}

fn document_ref_order(left: &DocumentRef, right: &DocumentRef) -> std::cmp::Ordering {
    (
        left.schema_id.as_str(),
        left.document_id.as_str(),
        left.revision,
        left.sha256.as_str(),
    )
        .cmp(&(
            right.schema_id.as_str(),
            right.document_id.as_str(),
            right.revision,
            right.sha256.as_str(),
        ))
}

fn catalog_ref_order(left: &CatalogRef, right: &CatalogRef) -> std::cmp::Ordering {
    (
        left.catalog_id.as_str(),
        left.format_version,
        left.item_version.as_str(),
        left.sha256.as_str(),
    )
        .cmp(&(
            right.catalog_id.as_str(),
            right.format_version,
            right.item_version.as_str(),
            right.sha256.as_str(),
        ))
}

const fn completeness_rank(value: Completeness) -> u8 {
    match value {
        Completeness::Unverified => 0,
        Completeness::Partial => 1,
        Completeness::Complete => 2,
    }
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

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    fn fixture(name: &str) -> serde_json::Value {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../specs/fixtures/management/v1")
            .join(name)
            .join("minimal.json");
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn claim_fields_are_an_additive_reader_extension_for_existing_v2_documents() {
        let mut gate_value = fixture("gate-decision-v2");
        gate_value
            .as_object_mut()
            .unwrap()
            .remove("claim_evaluations");
        let gate: GateDecisionV2 = serde_json::from_value(gate_value.clone()).unwrap();
        assert!(gate.claim_evaluations.is_empty());
        let gate_round_trip = serde_json::to_value(&gate).unwrap();
        assert!(gate_round_trip.get("claim_evaluations").is_none());

        let mut bundle_value = fixture("evidence-bundle-v2");
        bundle_value
            .as_object_mut()
            .unwrap()
            .remove("completion_claims");
        bundle_value
            .as_object_mut()
            .unwrap()
            .remove("claim_evaluations");
        // The generic fixture generator cannot infer the domain-specific `evb_`
        // prefix from this nested contract yet. Keep this compatibility test
        // focused on the additive fields by supplying a valid typed identifier.
        bundle_value["evidence_bundle_id"] =
            serde_json::Value::String(EvidenceBundleId::new().to_string());
        let bundle: EvidenceBundleV2 = serde_json::from_value(bundle_value.clone()).unwrap();
        assert!(bundle.completion_claims.is_empty());
        assert!(bundle.claim_evaluations.is_empty());
        let bundle_round_trip = serde_json::to_value(&bundle).unwrap();
        assert!(bundle_round_trip.get("completion_claims").is_none());
        assert!(bundle_round_trip.get("claim_evaluations").is_none());
    }
}
