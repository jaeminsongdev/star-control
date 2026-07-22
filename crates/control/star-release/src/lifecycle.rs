use serde::{Deserialize, Serialize};
use star_contracts::{
    Sha256Hash,
    release_v2::{ReleaseArchitecture, RuntimeVerificationState},
};

use crate::ReleaseError;

pub const RELEASE_LIFECYCLE_EVIDENCE_SCHEMA_ID: &str = "star.release-lifecycle-evidence";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleExecutionMode {
    NativeIsolated,
    FakeModel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecyclePhase {
    NotInstalled,
    Installed,
    FirstRunVerified,
    UpdateStaged,
    RollbackRequired,
    RolledBack,
    Repaired,
    Uninstalled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleEvent {
    pub sequence: u32,
    pub phase: LifecyclePhase,
    pub active_artifact_set_digest: Option<Sha256Hash>,
    pub evidence_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseLifecycleEvidence {
    pub schema_id: String,
    pub schema_version: u32,
    pub architecture: ReleaseArchitecture,
    pub execution_mode: LifecycleExecutionMode,
    pub runtime_verification: RuntimeVerificationState,
    pub phase: LifecyclePhase,
    pub candidate_artifact_set_digest: Sha256Hash,
    pub active_artifact_set_digest: Option<Sha256Hash>,
    pub previous_artifact_set_digest: Option<Sha256Hash>,
    pub user_data_digest_before: Sha256Hash,
    pub user_data_digest_after: Sha256Hash,
    pub events: Vec<LifecycleEvent>,
    pub limitations: Vec<String>,
}

impl ReleaseLifecycleEvidence {
    pub fn new(
        architecture: ReleaseArchitecture,
        execution_mode: LifecycleExecutionMode,
        candidate_artifact_set_digest: Sha256Hash,
        user_data_digest: Sha256Hash,
        evidence_ref: impl Into<String>,
    ) -> Result<Self, ReleaseError> {
        if architecture == ReleaseArchitecture::Arm64
            && execution_mode != LifecycleExecutionMode::FakeModel
        {
            return Err(ReleaseError::Blocked);
        }
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

    pub fn install(&mut self, evidence_ref: impl Into<String>) -> Result<(), ReleaseError> {
        self.transition(
            LifecyclePhase::NotInstalled,
            LifecyclePhase::Installed,
            evidence_ref,
            |state| {
                state.active_artifact_set_digest =
                    Some(state.candidate_artifact_set_digest.clone());
            },
        )
    }

    pub fn verify_first_run(
        &mut self,
        evidence_ref: impl Into<String>,
    ) -> Result<(), ReleaseError> {
        self.transition(
            LifecyclePhase::Installed,
            LifecyclePhase::FirstRunVerified,
            evidence_ref,
            |_| {},
        )
    }

    pub fn stage_update(
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
        self.push_event(evidence_ref);
        Ok(())
    }

    pub fn record_update_failure(
        &mut self,
        evidence_ref: impl Into<String>,
    ) -> Result<(), ReleaseError> {
        self.transition(
            LifecyclePhase::UpdateStaged,
            LifecyclePhase::RollbackRequired,
            evidence_ref,
            |state| {
                state.active_artifact_set_digest =
                    Some(state.candidate_artifact_set_digest.clone());
            },
        )
    }

    pub fn rollback(&mut self, evidence_ref: impl Into<String>) -> Result<(), ReleaseError> {
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
        self.push_event(evidence_ref);
        Ok(())
    }

    pub fn repair(&mut self, evidence_ref: impl Into<String>) -> Result<(), ReleaseError> {
        self.transition(
            LifecyclePhase::RolledBack,
            LifecyclePhase::Repaired,
            evidence_ref,
            |_| {},
        )
    }

    pub fn uninstall_preserving_user_data(
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
        self.push_event(evidence_ref);
        Ok(())
    }

    pub fn validate_complete(&self) -> Result<(), ReleaseError> {
        if self.schema_id != RELEASE_LIFECYCLE_EVIDENCE_SCHEMA_ID
            || self.schema_version != 1
            || self.phase != LifecyclePhase::Uninstalled
            || self.active_artifact_set_digest.is_some()
            || self.previous_artifact_set_digest.is_none()
            || self.user_data_digest_before != self.user_data_digest_after
            || self.events.len() != 8
            || self
                .events
                .iter()
                .enumerate()
                .any(|(index, event)| event.sequence != index as u32 + 1)
            || (self.architecture == ReleaseArchitecture::Arm64
                && (self.execution_mode != LifecycleExecutionMode::FakeModel
                    || self.runtime_verification != RuntimeVerificationState::NativeUnverified
                    || !self
                        .limitations
                        .iter()
                        .any(|item| item == "native_unverified")))
        {
            return Err(ReleaseError::Blocked);
        }
        Ok(())
    }

    fn transition(
        &mut self,
        expected: LifecyclePhase,
        next: LifecyclePhase,
        evidence_ref: impl Into<String>,
        change: impl FnOnce(&mut Self),
    ) -> Result<(), ReleaseError> {
        if self.phase != expected {
            return Err(ReleaseError::Conflict);
        }
        let evidence_ref = checked_evidence_ref(evidence_ref.into())?;
        change(self);
        self.phase = next;
        self.push_event(evidence_ref);
        Ok(())
    }

    fn push_event(&mut self, evidence_ref: String) {
        self.events.push(LifecycleEvent {
            sequence: self.events.len() as u32 + 1,
            phase: self.phase,
            active_artifact_set_digest: self.active_artifact_set_digest.clone(),
            evidence_ref,
        });
    }
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
    fn arm64_fake_lifecycle_cannot_be_promoted_to_native_success() {
        let evidence = complete_lifecycle(
            ReleaseArchitecture::Arm64,
            LifecycleExecutionMode::FakeModel,
        );
        assert_eq!(
            evidence.runtime_verification,
            RuntimeVerificationState::NativeUnverified
        );
        assert_eq!(evidence.limitations, vec!["native_unverified"]);
        assert!(matches!(
            ReleaseLifecycleEvidence::new(
                ReleaseArchitecture::Arm64,
                LifecycleExecutionMode::NativeIsolated,
                Sha256Hash::digest(b"candidate"),
                Sha256Hash::digest(b"user"),
                "unsupported-native-claim",
            ),
            Err(ReleaseError::Blocked)
        ));
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
