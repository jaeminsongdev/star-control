//! Shared M7-M9 receipt for externally executed development effects.
//!
//! The external tool remains the owner of scanner, debugger, package-manager,
//! migration, profiler, updater, or Git behavior.  Star-Control owns only the
//! exact invocation binding and the durable observation of its outcome.

use chrono::DateTime;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{OperationId, ProjectId, Sha256Hash, canonical_sha256};

pub const DEVELOPMENT_EFFECT_RECEIPT_V1_SCHEMA_ID: &str = "star.development-effect-receipt";

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DevelopmentEffectKind {
    SecurityRefresh,
    DebuggerCapture,
    LicenseScan,
    DependencyPrepare,
    DependencyApply,
    UpdaterApply,
    MigrationExecute,
    PerformanceRun,
    LanguageCutover,
    RemoteRecovery,
}

impl DevelopmentEffectKind {
    pub const fn permission_action(self) -> &'static str {
        match self {
            Self::SecurityRefresh => "external.security.read",
            Self::DebuggerCapture => "process.debug.attach",
            Self::LicenseScan => "external.license.read",
            Self::DependencyPrepare => "dependency.package_manager.prepare",
            Self::DependencyApply => "dependency.package_manager.apply",
            Self::UpdaterApply => "installation.update",
            Self::MigrationExecute => "migration.execute",
            Self::PerformanceRun => "performance.execute",
            Self::LanguageCutover => "migration.language.cutover",
            Self::RemoteRecovery => "git.remote.recovery",
        }
    }

    pub const fn requires_approval(self) -> bool {
        matches!(
            self,
            Self::DebuggerCapture
                | Self::DependencyApply
                | Self::UpdaterApply
                | Self::MigrationExecute
                | Self::LanguageCutover
                | Self::RemoteRecovery
        )
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DevelopmentEffectState {
    Succeeded,
    Failed,
    Partial,
    OutcomeUnknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentEffectReceiptV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub receipt_id: String,
    pub revision: u64,
    pub project_id: ProjectId,
    pub effect_kind: DevelopmentEffectKind,
    pub exact_subject_ref: String,
    pub exact_subject_fingerprint: Sha256Hash,
    pub operation_id: OperationId,
    pub tool_id: String,
    pub descriptor_hash: Sha256Hash,
    pub arguments_hash: Sha256Hash,
    pub executable_sha256: Sha256Hash,
    pub approval_ref: Option<String>,
    pub permission_decision_ref: Option<String>,
    pub gate_decision_ref: Option<String>,
    pub started_at: Option<String>,
    pub observed_at: String,
    pub state: DevelopmentEffectState,
    pub source_effect_started: bool,
    #[serde(default)]
    pub output_artifact_refs: Vec<Sha256Hash>,
    pub result_fingerprint: Option<Sha256Hash>,
    #[serde(default)]
    pub limitation_codes: Vec<String>,
    pub receipt_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DevelopmentEffectContractError {
    #[error("development effect receipt is invalid")]
    Invalid,
    #[error("development effect receipt fingerprint failed")]
    Fingerprint,
}

impl DevelopmentEffectReceiptV1 {
    pub fn seal(mut self) -> Result<Self, DevelopmentEffectContractError> {
        let observed_at = DateTime::parse_from_rfc3339(&self.observed_at)
            .map_err(|_| DevelopmentEffectContractError::Invalid)?;
        let started_at = self
            .started_at
            .as_deref()
            .map(DateTime::parse_from_rfc3339)
            .transpose()
            .map_err(|_| DevelopmentEffectContractError::Invalid)?;
        if self.schema_id != DEVELOPMENT_EFFECT_RECEIPT_V1_SCHEMA_ID
            || self.schema_version != 1
            || !token(&self.receipt_id, 192)
            || self.revision == 0
            || self.exact_subject_ref.trim().is_empty()
            || self.exact_subject_ref.chars().count() > 512
            || self.exact_subject_ref.contains('\0')
            || !token(&self.tool_id, 192)
            || started_at.is_some_and(|started_at| started_at > observed_at)
            || self.effect_kind.requires_approval()
                && [
                    self.approval_ref.as_deref(),
                    self.permission_decision_ref.as_deref(),
                    self.gate_decision_ref.as_deref(),
                ]
                .into_iter()
                .any(|value| value.is_none_or(|value| value.trim().is_empty()))
            || self.state == DevelopmentEffectState::Succeeded
                && (!self.source_effect_started
                    || self.started_at.is_none()
                    || self.result_fingerprint.is_none())
        {
            return Err(DevelopmentEffectContractError::Invalid);
        }
        for value in [
            self.approval_ref.as_deref(),
            self.permission_decision_ref.as_deref(),
            self.gate_decision_ref.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if value.trim().is_empty() || value.chars().count() > 512 || value.contains('\0') {
                return Err(DevelopmentEffectContractError::Invalid);
            }
        }
        self.output_artifact_refs.sort();
        self.output_artifact_refs.dedup();
        self.limitation_codes.sort();
        self.limitation_codes.dedup();
        if self.limitation_codes.iter().any(|value| !token(value, 128)) {
            return Err(DevelopmentEffectContractError::Invalid);
        }
        self.receipt_fingerprint = canonical_sha256(&serde_json::json!({
            "schema_id":self.schema_id,
            "schema_version":self.schema_version,
            "receipt_id":self.receipt_id,
            "revision":self.revision,
            "project_id":self.project_id,
            "effect_kind":self.effect_kind,
            "exact_subject_ref":self.exact_subject_ref,
            "exact_subject_fingerprint":self.exact_subject_fingerprint,
            "operation_id":self.operation_id,
            "tool_id":self.tool_id,
            "descriptor_hash":self.descriptor_hash,
            "arguments_hash":self.arguments_hash,
            "executable_sha256":self.executable_sha256,
            "approval_ref":self.approval_ref,
            "permission_decision_ref":self.permission_decision_ref,
            "gate_decision_ref":self.gate_decision_ref,
            "started_at":self.started_at,
            "observed_at":self.observed_at,
            "state":self.state,
            "source_effect_started":self.source_effect_started,
            "output_artifact_refs":self.output_artifact_refs,
            "result_fingerprint":self.result_fingerprint,
            "limitation_codes":self.limitation_codes,
        }))
        .map_err(|_| DevelopmentEffectContractError::Fingerprint)?;
        Ok(self)
    }
}

fn token(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.chars().count() <= maximum
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(kind: DevelopmentEffectKind) -> DevelopmentEffectReceiptV1 {
        DevelopmentEffectReceiptV1 {
            schema_id: DEVELOPMENT_EFFECT_RECEIPT_V1_SCHEMA_ID.to_owned(),
            schema_version: 1,
            receipt_id: "effect-01".to_owned(),
            revision: 1,
            project_id: ProjectId::new(),
            effect_kind: kind,
            exact_subject_ref: "migration-plan:one".to_owned(),
            exact_subject_fingerprint: Sha256Hash::digest(b"subject"),
            operation_id: OperationId::new(),
            tool_id: "fixture.effect".to_owned(),
            descriptor_hash: Sha256Hash::digest(b"descriptor"),
            arguments_hash: Sha256Hash::digest(b"arguments"),
            executable_sha256: Sha256Hash::digest(b"executable"),
            approval_ref: kind.requires_approval().then(|| "approval-01".to_owned()),
            permission_decision_ref: Some("permission-01".to_owned()),
            gate_decision_ref: Some("gate-01".to_owned()),
            started_at: Some("2026-07-23T00:00:00Z".to_owned()),
            observed_at: "2026-07-23T00:00:01Z".to_owned(),
            state: DevelopmentEffectState::Succeeded,
            source_effect_started: true,
            output_artifact_refs: vec![Sha256Hash::digest(b"artifact")],
            result_fingerprint: Some(Sha256Hash::digest(b"result")),
            limitation_codes: Vec::new(),
            receipt_fingerprint: Sha256Hash::digest(b"placeholder"),
        }
    }

    #[test]
    fn effect_receipt_is_deterministic_and_requires_approval_when_risky() {
        let candidate = receipt(DevelopmentEffectKind::LanguageCutover);
        let first = candidate.clone().seal().unwrap();
        let second = candidate.seal().unwrap();
        assert_eq!(first.receipt_fingerprint, second.receipt_fingerprint);

        let mut missing = receipt(DevelopmentEffectKind::RemoteRecovery);
        missing.approval_ref = None;
        assert_eq!(
            missing.seal().unwrap_err(),
            DevelopmentEffectContractError::Invalid
        );
    }
}
