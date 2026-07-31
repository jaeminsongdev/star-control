//! Application-owned normalization boundary for external Code Health evidence.
//!
//! Gateway forwards registered artifacts and descriptors only.  Tool-specific
//! logs are never parsed here; callers must provide a stable machine protocol
//! and one of the provider-neutral typed observations below.

use star_contracts::{
    development_v2::CompatibilityReportV2,
    external_analysis::{
        CompatibilityProviderObservationV1, CoverageObservationV1, ExternalAnalysisContractError,
        ExternalAnalysisEvidenceV1, FlakyTestObservationV1, NearCloneObservationV1,
        ReproducibilityVerificationReportV1, RuntimeSafetyObservationV1,
        SupplyChainProviderObservationV1,
    },
    maintenance_v2::SupplyChainSnapshot,
};

#[derive(Debug, thiserror::Error)]
pub enum ExternalAnalysisNormalizationError {
    #[error("external analysis contract is invalid")]
    Contract(#[from] ExternalAnalysisContractError),
    #[error("provider protocol is not stable machine-readable structured data")]
    Protocol,
    #[error("existing report and provider evidence conflict")]
    Conflict,
}

pub fn require_application_normalizable(
    evidence: &ExternalAnalysisEvidenceV1,
) -> Result<(), ExternalAnalysisNormalizationError> {
    evidence.validate()?;
    if !evidence.protocol.application_normalization_eligible() {
        return Err(ExternalAnalysisNormalizationError::Protocol);
    }
    Ok(())
}

pub fn normalize_coverage_observation(
    observation: CoverageObservationV1,
) -> Result<CoverageObservationV1, ExternalAnalysisNormalizationError> {
    require_application_normalizable(&observation.evidence)?;
    observation.validate()?;
    Ok(observation)
}

pub fn normalize_flaky_test_observation(
    observation: FlakyTestObservationV1,
) -> Result<FlakyTestObservationV1, ExternalAnalysisNormalizationError> {
    require_application_normalizable(&observation.evidence)?;
    observation.validate()?;
    Ok(observation)
}

pub fn attach_compatibility_providers(
    report: CompatibilityReportV2,
    observations: Vec<CompatibilityProviderObservationV1>,
) -> Result<CompatibilityReportV2, ExternalAnalysisNormalizationError> {
    star_development::compatibility_v2::attach_compatibility_provider_observations(
        report,
        observations,
    )
    .map_err(|_| ExternalAnalysisNormalizationError::Conflict)
}

pub fn attach_supply_chain_providers(
    snapshot: SupplyChainSnapshot,
    observations: Vec<SupplyChainProviderObservationV1>,
) -> Result<SupplyChainSnapshot, ExternalAnalysisNormalizationError> {
    star_development::maintenance_v2::attach_supply_chain_provider_observations(
        snapshot,
        observations,
    )
    .map_err(|_| ExternalAnalysisNormalizationError::Conflict)
}

pub fn verify_reproducibility_report(
    report: ReproducibilityVerificationReportV1,
) -> Result<ReproducibilityVerificationReportV1, ExternalAnalysisNormalizationError> {
    report.validate()?;
    Ok(report)
}

pub fn normalize_runtime_safety_observation(
    observation: RuntimeSafetyObservationV1,
) -> Result<RuntimeSafetyObservationV1, ExternalAnalysisNormalizationError> {
    require_application_normalizable(&observation.evidence)?;
    observation.validate()?;
    Ok(observation)
}

pub fn accept_near_clone_advisory(
    observation: NearCloneObservationV1,
) -> Result<NearCloneObservationV1, ExternalAnalysisNormalizationError> {
    observation.validate()?;
    Ok(observation)
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use star_contracts::{
        ProjectId, Sha256Hash,
        external_analysis::{
            EXTERNAL_ANALYSIS_EVIDENCE_V1_SCHEMA_ID, ExternalAnalysisCompleteness,
            ExternalAnalysisProtocolV1, ProtocolDetailLevel, ProtocolStability,
        },
    };

    use super::*;

    fn unavailable_evidence(
        stability: ProtocolStability,
        detail_level: ProtocolDetailLevel,
        machine_readable: bool,
    ) -> ExternalAnalysisEvidenceV1 {
        let observed_at = Utc
            .with_ymd_and_hms(2026, 7, 31, 0, 0, 0)
            .single()
            .expect("valid timestamp");
        ExternalAnalysisEvidenceV1 {
            schema_id: EXTERNAL_ANALYSIS_EVIDENCE_V1_SCHEMA_ID.to_owned(),
            schema_version: 1,
            evidence_id: "provider-unavailable".to_owned(),
            project_id: ProjectId::new(),
            provider_id: "provider".to_owned(),
            provider_version: None,
            executable_sha256: None,
            protocol: ExternalAnalysisProtocolV1 {
                protocol_id: "provider-protocol".to_owned(),
                protocol_version: "1".to_owned(),
                media_type: "application/json".to_owned(),
                stability,
                detail_level,
                machine_readable,
                schema_uri: None,
            },
            config_fingerprint: Sha256Hash::digest(b"config"),
            input_fingerprint: Sha256Hash::digest(b"input"),
            source_fingerprint: Sha256Hash::digest(b"source"),
            environment_fingerprint: Sha256Hash::digest(b"environment"),
            raw_artifact_ref: None,
            normalized_artifact_ref: None,
            exit_code: None,
            started_at: observed_at,
            completed_at: observed_at,
            completeness: ExternalAnalysisCompleteness::Unavailable,
            limitations: vec!["provider is not installed".to_owned()],
            content_fingerprint: Sha256Hash::digest(b"placeholder"),
        }
        .seal()
        .expect("valid unavailable evidence")
    }

    #[test]
    fn application_normalizes_only_stable_structured_machine_protocols() {
        let structured = unavailable_evidence(
            ProtocolStability::Stable,
            ProtocolDetailLevel::Structured,
            true,
        );
        require_application_normalizable(&structured).expect("structured protocol");

        for evidence in [
            unavailable_evidence(
                ProtocolStability::Stable,
                ProtocolDetailLevel::ExitClassification,
                true,
            ),
            unavailable_evidence(
                ProtocolStability::HumanText,
                ProtocolDetailLevel::RawOnly,
                false,
            ),
        ] {
            assert!(matches!(
                require_application_normalizable(&evidence),
                Err(ExternalAnalysisNormalizationError::Protocol)
            ));
        }
    }
}
