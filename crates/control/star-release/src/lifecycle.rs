use star_contracts::Sha256Hash;
pub use star_contracts::release_v2::{
    LifecycleEvent, LifecycleExecutionMode, LifecyclePhase, RELEASE_LIFECYCLE_EVIDENCE_SCHEMA_ID,
    ReleaseArchitecture, ReleaseLifecycleEvidence, RuntimeVerificationState,
};

use crate::ReleaseError;

pub trait ReleaseLifecycleEvidenceExt: Sized {
    fn new(
        architecture: ReleaseArchitecture,
        execution_mode: LifecycleExecutionMode,
        candidate_artifact_set_digest: Sha256Hash,
        user_data_digest: Sha256Hash,
        evidence_ref: impl Into<String>,
    ) -> Result<Self, ReleaseError>;
    fn install(&mut self, evidence_ref: impl Into<String>) -> Result<(), ReleaseError>;
    fn verify_first_run(&mut self, evidence_ref: impl Into<String>) -> Result<(), ReleaseError>;
    fn stage_update(
        &mut self,
        update_artifact_set_digest: Sha256Hash,
        evidence_ref: impl Into<String>,
    ) -> Result<(), ReleaseError>;
    fn record_update_failure(
        &mut self,
        evidence_ref: impl Into<String>,
    ) -> Result<(), ReleaseError>;
    fn rollback(&mut self, evidence_ref: impl Into<String>) -> Result<(), ReleaseError>;
    fn repair(&mut self, evidence_ref: impl Into<String>) -> Result<(), ReleaseError>;
    fn uninstall_preserving_user_data(
        &mut self,
        observed_user_data_digest: Sha256Hash,
        evidence_ref: impl Into<String>,
    ) -> Result<(), ReleaseError>;
    fn validate_complete(&self) -> Result<(), ReleaseError>;
}

impl ReleaseLifecycleEvidenceExt for ReleaseLifecycleEvidence {
    fn new(
        architecture: ReleaseArchitecture,
        execution_mode: LifecycleExecutionMode,
        candidate_artifact_set_digest: Sha256Hash,
        user_data_digest: Sha256Hash,
        evidence_ref: impl Into<String>,
    ) -> Result<Self, ReleaseError> {
        let evidence_ref = checked_evidence_ref(evidence_ref.into())?;
        let runtime_verification = match execution_mode {
            LifecycleExecutionMode::NativeIsolated => RuntimeVerificationState::NativeVerified,
            LifecycleExecutionMode::FakeModel => RuntimeVerificationState::NativeUnverified,
        };
        let limitations = match execution_mode {
            LifecycleExecutionMode::NativeIsolated => Vec::new(),
            LifecycleExecutionMode::FakeModel => vec!["native_unverified".to_owned()],
        };
        Ok(Self {
            schema_id: RELEASE_LIFECYCLE_EVIDENCE_SCHEMA_ID.to_owned(),
            schema_version: 1,
            architecture,
            execution_mode,
            runtime_verification,
            phase: LifecyclePhase::NotInstalled,
            candidate_artifact_set_digest,
            active_artifact_set_digest: None,
            previous_artifact_set_digest: None,
            user_data_digest_before: user_data_digest.clone(),
            user_data_digest_after: user_data_digest,
            events: vec![LifecycleEvent {
                sequence: 1,
                phase: LifecyclePhase::NotInstalled,
                active_artifact_set_digest: None,
                evidence_ref,
            }],
            limitations,
        })
    }

    fn install(&mut self, evidence_ref: impl Into<String>) -> Result<(), ReleaseError> {
        transition(
            self,
            LifecyclePhase::NotInstalled,
            LifecyclePhase::Installed,
            evidence_ref,
            |state| {
                state.active_artifact_set_digest =
                    Some(state.candidate_artifact_set_digest.clone());
            },
        )
    }

    fn verify_first_run(&mut self, evidence_ref: impl Into<String>) -> Result<(), ReleaseError> {
        transition(
            self,
            LifecyclePhase::Installed,
            LifecyclePhase::FirstRunVerified,
            evidence_ref,
            |_| {},
        )
    }

    fn stage_update(
        &mut self,
        update_artifact_set_digest: Sha256Hash,
        evidence_ref: impl Into<String>,
    ) -> Result<(), ReleaseError> {
        if self.phase != LifecyclePhase::FirstRunVerified
            || self.active_artifact_set_digest.as_ref() == Some(&update_artifact_set_digest)
        {
            return Err(ReleaseError::Conflict);
        }
        let evidence_ref = checked_evidence_ref(evidence_ref.into())?;
        self.previous_artifact_set_digest = self.active_artifact_set_digest.clone();
        self.candidate_artifact_set_digest = update_artifact_set_digest;
        self.phase = LifecyclePhase::UpdateStaged;
        push_event(self, evidence_ref);
        Ok(())
    }

    fn record_update_failure(
        &mut self,
        evidence_ref: impl Into<String>,
    ) -> Result<(), ReleaseError> {
        transition(
            self,
            LifecyclePhase::UpdateStaged,
            LifecyclePhase::RollbackRequired,
            evidence_ref,
            |state| {
                state.active_artifact_set_digest =
                    Some(state.candidate_artifact_set_digest.clone());
            },
        )
    }

    fn rollback(&mut self, evidence_ref: impl Into<String>) -> Result<(), ReleaseError> {
        if self.phase != LifecyclePhase::RollbackRequired {
            return Err(ReleaseError::Conflict);
        }
        let previous = self
            .previous_artifact_set_digest
            .clone()
            .ok_or(ReleaseError::Conflict)?;
        let evidence_ref = checked_evidence_ref(evidence_ref.into())?;
        self.active_artifact_set_digest = Some(previous);
        self.phase = LifecyclePhase::RolledBack;
        push_event(self, evidence_ref);
        Ok(())
    }

    fn repair(&mut self, evidence_ref: impl Into<String>) -> Result<(), ReleaseError> {
        transition(
            self,
            LifecyclePhase::RolledBack,
            LifecyclePhase::Repaired,
            evidence_ref,
            |_| {},
        )
    }

    fn uninstall_preserving_user_data(
        &mut self,
        observed_user_data_digest: Sha256Hash,
        evidence_ref: impl Into<String>,
    ) -> Result<(), ReleaseError> {
        if self.phase != LifecyclePhase::Repaired
            || observed_user_data_digest != self.user_data_digest_before
        {
            return Err(ReleaseError::Blocked);
        }
        let evidence_ref = checked_evidence_ref(evidence_ref.into())?;
        self.user_data_digest_after = observed_user_data_digest;
        self.active_artifact_set_digest = None;
        self.phase = LifecyclePhase::Uninstalled;
        push_event(self, evidence_ref);
        Ok(())
    }

    fn validate_complete(&self) -> Result<(), ReleaseError> {
        let expected_phases = [
            LifecyclePhase::NotInstalled,
            LifecyclePhase::Installed,
            LifecyclePhase::FirstRunVerified,
            LifecyclePhase::UpdateStaged,
            LifecyclePhase::RollbackRequired,
            LifecyclePhase::RolledBack,
            LifecyclePhase::Repaired,
            LifecyclePhase::Uninstalled,
        ];
        let Some(previous) = self.previous_artifact_set_digest.as_ref() else {
            return Err(ReleaseError::Blocked);
        };
        let expected_active = [
            None,
            Some(previous),
            Some(previous),
            Some(previous),
            Some(&self.candidate_artifact_set_digest),
            Some(previous),
            Some(previous),
            None,
        ];
        let execution_binding_valid = match self.execution_mode {
            LifecycleExecutionMode::NativeIsolated => {
                self.runtime_verification == RuntimeVerificationState::NativeVerified
                    && !self
                        .limitations
                        .iter()
                        .any(|item| item == "native_unverified")
            }
            LifecycleExecutionMode::FakeModel => {
                self.runtime_verification == RuntimeVerificationState::NativeUnverified
                    && self
                        .limitations
                        .iter()
                        .any(|item| item == "native_unverified")
            }
        };
        if self.schema_id != RELEASE_LIFECYCLE_EVIDENCE_SCHEMA_ID
            || self.schema_version != 1
            || self.phase != LifecyclePhase::Uninstalled
            || self.active_artifact_set_digest.is_some()
            || self.user_data_digest_before != self.user_data_digest_after
            || self.events.len() != expected_phases.len()
            || self.events.iter().enumerate().any(|(index, event)| {
                event.sequence != index as u32 + 1
                    || event.phase != expected_phases[index]
                    || event.active_artifact_set_digest.as_ref() != expected_active[index]
                    || checked_evidence_ref(event.evidence_ref.clone()).is_err()
            })
            || !execution_binding_valid
        {
            return Err(ReleaseError::Blocked);
        }
        Ok(())
    }
}

fn transition(
    evidence: &mut ReleaseLifecycleEvidence,
    expected: LifecyclePhase,
    next: LifecyclePhase,
    evidence_ref: impl Into<String>,
    change: impl FnOnce(&mut ReleaseLifecycleEvidence),
) -> Result<(), ReleaseError> {
    if evidence.phase != expected {
        return Err(ReleaseError::Conflict);
    }
    let evidence_ref = checked_evidence_ref(evidence_ref.into())?;
    change(evidence);
    evidence.phase = next;
    push_event(evidence, evidence_ref);
    Ok(())
}

fn push_event(evidence: &mut ReleaseLifecycleEvidence, evidence_ref: String) {
    evidence.events.push(LifecycleEvent {
        sequence: evidence.events.len() as u32 + 1,
        phase: evidence.phase,
        active_artifact_set_digest: evidence.active_artifact_set_digest.clone(),
        evidence_ref,
    });
}

fn checked_evidence_ref(value: String) -> Result<String, ReleaseError> {
    let value = value.trim().to_owned();
    if value.is_empty() || value.len() > 512 {
        return Err(ReleaseError::Invalid);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_lifecycle(
        architecture: ReleaseArchitecture,
        mode: LifecycleExecutionMode,
    ) -> ReleaseLifecycleEvidence {
        let user_data = Sha256Hash::digest(b"preserved-user-data");
        let mut evidence = ReleaseLifecycleEvidence::new(
            architecture,
            mode,
            Sha256Hash::digest(b"candidate-v1"),
            user_data.clone(),
            "clean-room-before-snapshot",
        )
        .unwrap();
        evidence.install("manifest-verified-install").unwrap();
        evidence.verify_first_run("first-run-status").unwrap();
        evidence
            .stage_update(Sha256Hash::digest(b"candidate-v2"), "update-stage")
            .unwrap();
        evidence
            .record_update_failure("injected-postcheck-failure")
            .unwrap();
        evidence.rollback("rollback-digest-restored").unwrap();
        evidence.repair("repair-status-verified").unwrap();
        evidence
            .uninstall_preserving_user_data(user_data, "uninstall-after-snapshot")
            .unwrap();
        evidence.validate_complete().unwrap();
        evidence
    }

    #[test]
    fn x64_native_isolated_lifecycle_preserves_data_and_restores_failed_update() {
        let evidence = complete_lifecycle(
            ReleaseArchitecture::X64,
            LifecycleExecutionMode::NativeIsolated,
        );
        assert_eq!(
            evidence.runtime_verification,
            RuntimeVerificationState::NativeVerified
        );
        assert!(evidence.limitations.is_empty());
        assert_eq!(
            evidence.events[5].active_artifact_set_digest,
            evidence.previous_artifact_set_digest
        );
    }

    #[test]
    fn arm64_fake_lifecycle_stays_unverified_and_native_receipt_path_remains_available() {
        let evidence = complete_lifecycle(
            ReleaseArchitecture::Arm64,
            LifecycleExecutionMode::FakeModel,
        );
        assert_eq!(
            evidence.runtime_verification,
            RuntimeVerificationState::NativeUnverified
        );
        assert_eq!(evidence.limitations, vec!["native_unverified"]);
        let native = complete_lifecycle(
            ReleaseArchitecture::Arm64,
            LifecycleExecutionMode::NativeIsolated,
        );
        assert_eq!(
            native.runtime_verification,
            RuntimeVerificationState::NativeVerified
        );
        assert!(native.limitations.is_empty());
    }

    #[test]
    fn complete_lifecycle_rejects_forged_phase_or_digest_history() {
        let evidence = complete_lifecycle(
            ReleaseArchitecture::X64,
            LifecycleExecutionMode::NativeIsolated,
        );
        let mut phase_tampered = evidence.clone();
        phase_tampered.events[3].phase = LifecyclePhase::Repaired;
        assert_eq!(
            phase_tampered.validate_complete(),
            Err(ReleaseError::Blocked)
        );

        let mut digest_tampered = evidence;
        digest_tampered.events[4].active_artifact_set_digest =
            digest_tampered.previous_artifact_set_digest.clone();
        assert_eq!(
            digest_tampered.validate_complete(),
            Err(ReleaseError::Blocked)
        );
    }

    #[test]
    fn lifecycle_rejects_illegal_order_same_byte_update_and_user_data_loss() {
        let user_data = Sha256Hash::digest(b"user");
        let candidate = Sha256Hash::digest(b"candidate");
        let mut evidence = ReleaseLifecycleEvidence::new(
            ReleaseArchitecture::Arm64,
            LifecycleExecutionMode::FakeModel,
            candidate.clone(),
            user_data.clone(),
            "before",
        )
        .unwrap();
        assert_eq!(
            evidence.verify_first_run("too-early"),
            Err(ReleaseError::Conflict)
        );
        evidence.install("install").unwrap();
        evidence.verify_first_run("first-run").unwrap();
        assert_eq!(
            evidence.stage_update(candidate, "same-byte"),
            Err(ReleaseError::Conflict)
        );
        evidence
            .stage_update(Sha256Hash::digest(b"update"), "stage")
            .unwrap();
        evidence.record_update_failure("failure").unwrap();
        evidence.rollback("rollback").unwrap();
        evidence.repair("repair").unwrap();
        assert_eq!(
            evidence
                .uninstall_preserving_user_data(Sha256Hash::digest(b"lost-user-data"), "uninstall"),
            Err(ReleaseError::Blocked)
        );
    }
}
