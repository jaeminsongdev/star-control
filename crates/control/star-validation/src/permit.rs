//! Process-memory pre/post Patch Gate protocol.

use chrono::{DateTime, Utc};
use star_contracts::{
    Sha256Hash, canonical_sha256,
    evidence::{ActorRef, GateDecisionKind, GateDecisionRef},
    evidence_v2::{GATE_DECISION_V2_SCHEMA_ID, GateDecisionV2, GatePhaseV2, empty_fingerprint},
};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatchPermitKindV2 {
    Automatic,
    ManualApproved,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManualPatchApprovalV2 {
    approval_id: String,
    approved_by: ActorRef,
    gate_decision_ref: GateDecisionRef,
    patch_fingerprint: Sha256Hash,
    before_binding_set_fingerprint: Sha256Hash,
    permission_fingerprint: Sha256Hash,
    approved_at: DateTime<Utc>,
    approval_fingerprint: Sha256Hash,
}

impl ManualPatchApprovalV2 {
    #[allow(clippy::too_many_arguments)]
    pub fn seal(
        approval_id: String,
        approved_by: ActorRef,
        gate_decision_ref: GateDecisionRef,
        patch_fingerprint: Sha256Hash,
        before_binding_set_fingerprint: Sha256Hash,
        permission_fingerprint: Sha256Hash,
        approved_at: DateTime<Utc>,
    ) -> Result<Self, PatchPermitError> {
        if approval_id.trim().is_empty() || approval_id.contains('\0') {
            return Err(PatchPermitError::Approval);
        }
        let approval_fingerprint = canonical_sha256(&serde_json::json!({
            "domain":"star.patch-manual-approval",
            "version":2,
            "approval_id":approval_id,
            "approved_by":approved_by,
            "gate_decision_ref":gate_decision_ref,
            "patch_fingerprint":patch_fingerprint,
            "before_binding_set_fingerprint":before_binding_set_fingerprint,
            "permission_fingerprint":permission_fingerprint,
            "approved_at":approved_at,
        }))
        .map_err(|_| PatchPermitError::Fingerprint)?;
        Ok(Self {
            approval_id,
            approved_by,
            gate_decision_ref,
            patch_fingerprint,
            before_binding_set_fingerprint,
            permission_fingerprint,
            approved_at,
            approval_fingerprint,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedPatchGateV2 {
    gate_decision_ref: GateDecisionRef,
    decision: GateDecisionKind,
    phase: GatePhaseV2,
    subject_binding_set_fingerprint: Sha256Hash,
    valid_until: Option<DateTime<Utc>>,
}

impl VerifiedPatchGateV2 {
    pub fn from_persisted_gate(
        gate: &GateDecisionV2,
        phase: GatePhaseV2,
    ) -> Result<Self, PatchPermitError> {
        if gate.schema_id != GATE_DECISION_V2_SCHEMA_ID
            || gate.schema_version != 2
            || gate.decision_fingerprint == empty_fingerprint()
        {
            return Err(PatchPermitError::Gate);
        }
        Ok(Self {
            gate_decision_ref: gate.reference().map_err(|_| PatchPermitError::Gate)?,
            decision: gate.decision,
            phase,
            subject_binding_set_fingerprint: gate.subject_binding_set_fingerprint.clone(),
            valid_until: gate.valid_until,
        })
    }
}

/// Deliberately neither `Clone` nor `Serialize`. The source-write port must
/// receive and consume this exact process-memory value.
pub struct PatchApplyPermitV2 {
    kind: PatchPermitKindV2,
    gate_decision_ref: GateDecisionRef,
    patch_fingerprint: Sha256Hash,
    before_binding_set_fingerprint: Sha256Hash,
    permission_fingerprint: Sha256Hash,
    approval_fingerprint: Option<Sha256Hash>,
    nonce: Sha256Hash,
    consumed: bool,
}

impl PatchApplyPermitV2 {
    pub fn kind(&self) -> PatchPermitKindV2 {
        self.kind
    }

    pub fn gate_decision_ref(&self) -> &GateDecisionRef {
        &self.gate_decision_ref
    }

    pub fn consume(
        &mut self,
        patch_fingerprint: &Sha256Hash,
        before_binding_set_fingerprint: &Sha256Hash,
        permission_fingerprint: &Sha256Hash,
    ) -> Result<PatchPermitUseV2, PatchPermitError> {
        if self.consumed {
            return Err(PatchPermitError::Consumed);
        }
        if patch_fingerprint != &self.patch_fingerprint
            || before_binding_set_fingerprint != &self.before_binding_set_fingerprint
            || permission_fingerprint != &self.permission_fingerprint
        {
            return Err(PatchPermitError::Binding);
        }
        self.consumed = true;
        Ok(PatchPermitUseV2 {
            kind: self.kind,
            gate_decision_ref: self.gate_decision_ref.clone(),
            patch_fingerprint: self.patch_fingerprint.clone(),
            before_binding_set_fingerprint: self.before_binding_set_fingerprint.clone(),
            permission_fingerprint: self.permission_fingerprint.clone(),
            approval_fingerprint: self.approval_fingerprint.clone(),
            nonce: self.nonce.clone(),
        })
    }
}

/// Non-serializable proof handed directly to one source-write call.
pub struct PatchPermitUseV2 {
    pub kind: PatchPermitKindV2,
    pub gate_decision_ref: GateDecisionRef,
    pub patch_fingerprint: Sha256Hash,
    pub before_binding_set_fingerprint: Sha256Hash,
    pub permission_fingerprint: Sha256Hash,
    pub approval_fingerprint: Option<Sha256Hash>,
    nonce: Sha256Hash,
}

impl PatchPermitUseV2 {
    pub fn nonce(&self) -> &Sha256Hash {
        &self.nonce
    }
}

pub fn issue_patch_apply_permit(
    gate: &VerifiedPatchGateV2,
    patch_fingerprint: Sha256Hash,
    before_binding_set_fingerprint: Sha256Hash,
    permission_fingerprint: Sha256Hash,
    manual_approval: Option<&ManualPatchApprovalV2>,
    evaluation_time: DateTime<Utc>,
) -> Result<PatchApplyPermitV2, PatchPermitError> {
    if gate.phase != GatePhaseV2::PatchPreApply
        || gate.subject_binding_set_fingerprint != before_binding_set_fingerprint
        || gate
            .valid_until
            .is_some_and(|valid_until| valid_until <= evaluation_time)
    {
        return Err(PatchPermitError::Gate);
    }
    let (kind, approval_fingerprint) = match gate.decision {
        GateDecisionKind::AutoPass if manual_approval.is_none() => {
            (PatchPermitKindV2::Automatic, None)
        }
        GateDecisionKind::HumanReview => {
            let approval = manual_approval.ok_or(PatchPermitError::Approval)?;
            if approval.gate_decision_ref != gate.gate_decision_ref
                || approval.patch_fingerprint != patch_fingerprint
                || approval.before_binding_set_fingerprint != before_binding_set_fingerprint
                || approval.permission_fingerprint != permission_fingerprint
            {
                return Err(PatchPermitError::Approval);
            }
            (
                PatchPermitKindV2::ManualApproved,
                Some(approval.approval_fingerprint.clone()),
            )
        }
        GateDecisionKind::AutoPass | GateDecisionKind::Block => {
            return Err(PatchPermitError::Gate);
        }
    };
    let nonce = canonical_sha256(&serde_json::json!({
        "domain":"star.patch-apply-permit-nonce",
        "version":2,
        "gate":gate.gate_decision_ref,
        "patch":patch_fingerprint,
        "before":before_binding_set_fingerprint,
        "permission":permission_fingerprint,
        "approval":approval_fingerprint,
        "issued_at":evaluation_time,
        "entropy":star_contracts::ids::RequestId::new(),
    }))
    .map_err(|_| PatchPermitError::Fingerprint)?;
    Ok(PatchApplyPermitV2 {
        kind,
        gate_decision_ref: gate.gate_decision_ref.clone(),
        patch_fingerprint,
        before_binding_set_fingerprint,
        permission_fingerprint,
        approval_fingerprint,
        nonce,
        consumed: false,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatchApplicationStateV2 {
    AppliedExact,
    PartiallyApplied,
    OutcomeUnknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatchPostApplyDispositionV2 {
    Complete,
    AwaitingHumanReview,
    RecoveryRequired,
}

#[allow(clippy::too_many_arguments)]
pub fn evaluate_patch_post_apply(
    state: PatchApplicationStateV2,
    expected_operation_fingerprint: &Sha256Hash,
    actual_operation_fingerprint: &Sha256Hash,
    before_binding_set_fingerprint: &Sha256Hash,
    after_binding_set_fingerprint: &Sha256Hash,
    post_gate: &VerifiedPatchGateV2,
    evaluation_time: DateTime<Utc>,
) -> PatchPostApplyDispositionV2 {
    if state != PatchApplicationStateV2::AppliedExact
        || expected_operation_fingerprint != actual_operation_fingerprint
        || before_binding_set_fingerprint == after_binding_set_fingerprint
        || post_gate.phase != GatePhaseV2::PatchPostApply
        || &post_gate.subject_binding_set_fingerprint != after_binding_set_fingerprint
        || post_gate
            .valid_until
            .is_some_and(|valid_until| valid_until <= evaluation_time)
    {
        return PatchPostApplyDispositionV2::RecoveryRequired;
    }
    match post_gate.decision {
        GateDecisionKind::AutoPass => PatchPostApplyDispositionV2::Complete,
        GateDecisionKind::HumanReview => PatchPostApplyDispositionV2::AwaitingHumanReview,
        GateDecisionKind::Block => PatchPostApplyDispositionV2::RecoveryRequired,
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PatchPermitError {
    #[error("pre-apply Gate is invalid, stale, or does not authorize this operation")]
    Gate,
    #[error("manual approval is absent or not exact")]
    Approval,
    #[error("permit binding does not match the source-write request")]
    Binding,
    #[error("permit has already been consumed")]
    Consumed,
    #[error("permit fingerprint could not be calculated")]
    Fingerprint,
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::ids::GateId;

    fn gate(decision: GateDecisionKind, phase: GatePhaseV2) -> VerifiedPatchGateV2 {
        VerifiedPatchGateV2 {
            gate_decision_ref: GateDecisionRef {
                gate_id: GateId::new(),
                revision: 1,
                sha256: Sha256Hash::digest(b"gate"),
            },
            decision,
            phase,
            subject_binding_set_fingerprint: Sha256Hash::digest(b"before"),
            valid_until: None,
        }
    }

    #[test]
    fn automatic_permit_is_exact_and_single_use() {
        let patch = Sha256Hash::digest(b"patch");
        let before = Sha256Hash::digest(b"before");
        let permission = Sha256Hash::digest(b"permission");
        let mut permit = issue_patch_apply_permit(
            &gate(GateDecisionKind::AutoPass, GatePhaseV2::PatchPreApply),
            patch.clone(),
            before.clone(),
            permission.clone(),
            None,
            Utc::now(),
        )
        .unwrap();
        assert_eq!(permit.kind(), PatchPermitKindV2::Automatic);
        assert!(permit.consume(&patch, &before, &permission).is_ok());
        assert!(matches!(
            permit.consume(&patch, &before, &permission),
            Err(PatchPermitError::Consumed)
        ));
    }

    #[test]
    fn human_review_never_issues_without_exact_manual_approval() {
        let result = issue_patch_apply_permit(
            &gate(GateDecisionKind::HumanReview, GatePhaseV2::PatchPreApply),
            Sha256Hash::digest(b"patch"),
            Sha256Hash::digest(b"before"),
            Sha256Hash::digest(b"permission"),
            None,
            Utc::now(),
        );
        assert!(matches!(result, Err(PatchPermitError::Approval)));
    }
}
