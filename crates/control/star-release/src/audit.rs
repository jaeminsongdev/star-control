use std::collections::BTreeSet;

use star_contracts::{
    Sha256Hash,
    profile::BUILTIN_DEVELOPMENT_PROFILE_IDS,
    release_v2::{
        FINAL_PRODUCT_AUDIT_V1_SCHEMA_ID, FinalProductAuditV1, ProductAuditStatusV1,
        ProductFeatureOwnershipStatusV1, ProductLifecycleEvidenceStatusV1,
        ProductProfileConformanceStatusV1, ReleaseArchitecture, ReleaseManifestV2, ReleaseStatus,
        ReleaseSupportTier, RuntimeVerificationState, SupplyChainKind, SupplyChainState,
    },
};
use star_domain::versioned_fingerprint;

use crate::{ReleaseError, candidate::verify_release_manifest};

pub const PRODUCT_FEATURE_IDS: [&str; 23] = [
    "A01", "A02", "A03", "A04", "A05", "A06", "A07", "A08", "A09", "A10", "B01", "B02", "B03",
    "B04", "B05", "B06", "B07", "B08", "B09", "C01", "D01", "D02", "D03",
];

pub fn build_final_product_audit(
    manifest: &ReleaseManifestV2,
    profile_catalog_fingerprint: Sha256Hash,
    feature_statuses: Vec<ProductFeatureOwnershipStatusV1>,
    profile_statuses: Vec<ProductProfileConformanceStatusV1>,
    m11_profile_conformant: bool,
    lifecycle_statuses: Vec<ProductLifecycleEvidenceStatusV1>,
) -> Result<FinalProductAuditV1, ReleaseError> {
    verify_release_manifest(manifest)?;
    let artifact_set_digest = manifest
        .artifact_set_digest
        .clone()
        .ok_or(ReleaseError::Invalid)?;
    seal_final_product_audit(FinalProductAuditV1 {
        schema_id: FINAL_PRODUCT_AUDIT_V1_SCHEMA_ID.to_owned(),
        schema_version: 1,
        release_manifest_id: manifest.release_manifest_id.clone(),
        release_manifest_fingerprint: manifest.manifest_fingerprint.clone(),
        artifact_set_digest,
        profile_catalog_fingerprint,
        feature_statuses,
        profile_statuses,
        m11_profile_conformant,
        lifecycle_statuses,
        internal_conformance: false,
        release_status: manifest.status,
        external_gate_reasons: release_external_gate_reasons(manifest),
        status: ProductAuditStatusV1::Blocked,
        audit_fingerprint: placeholder(),
    })
}

pub fn verify_final_product_audit(audit: &FinalProductAuditV1) -> Result<(), ReleaseError> {
    let sealed = seal_final_product_audit(audit.clone())?;
    if &sealed != audit {
        return Err(ReleaseError::Conflict);
    }
    Ok(())
}

fn seal_final_product_audit(
    mut audit: FinalProductAuditV1,
) -> Result<FinalProductAuditV1, ReleaseError> {
    if audit.schema_id != FINAL_PRODUCT_AUDIT_V1_SCHEMA_ID || audit.schema_version != 1 {
        return Err(ReleaseError::Invalid);
    }
    for feature in &mut audit.feature_statuses {
        normalize_strings(&mut feature.command_surfaces);
        normalize_strings(&mut feature.missing_command_surfaces);
        if !token(&feature.feature_id, 8)
            || !reference(&feature.semantic_owner_ref, 256)
            || !reference(&feature.physical_owner, 256)
            || feature.command_surfaces.is_empty()
            || feature
                .missing_command_surfaces
                .iter()
                .any(|command| !feature.command_surfaces.contains(command))
        {
            return Err(ReleaseError::Invalid);
        }
    }
    audit
        .feature_statuses
        .sort_by(|left, right| left.feature_id.cmp(&right.feature_id));
    for profile in &mut audit.profile_statuses {
        normalize_strings(&mut profile.limitations);
        if !reference(&profile.profile_id, 96) || !reference(&profile.profile_version, 96) {
            return Err(ReleaseError::Invalid);
        }
    }
    audit
        .profile_statuses
        .sort_by(|left, right| left.profile_id.cmp(&right.profile_id));
    audit
        .lifecycle_statuses
        .sort_by_key(|status| status.architecture);
    if audit.lifecycle_statuses.iter().any(|status| {
        !reference(&status.evidence_record_id, 192)
            || status.candidate_artifact_set_digest != audit.artifact_set_digest
    }) {
        return Err(ReleaseError::Invalid);
    }

    let feature_ids = audit
        .feature_statuses
        .iter()
        .map(|feature| feature.feature_id.as_str())
        .collect::<Vec<_>>();
    let profile_ids = audit
        .profile_statuses
        .iter()
        .map(|profile| profile.profile_id.as_str())
        .collect::<Vec<_>>();
    let lifecycle_architectures = audit
        .lifecycle_statuses
        .iter()
        .map(|status| status.architecture)
        .collect::<BTreeSet<_>>();
    if feature_ids != PRODUCT_FEATURE_IDS
        || profile_ids != BUILTIN_DEVELOPMENT_PROFILE_IDS
        || lifecycle_architectures.len() != audit.lifecycle_statuses.len()
    {
        return Err(ReleaseError::Invalid);
    }

    audit.internal_conformance = audit
        .feature_statuses
        .iter()
        .all(|feature| feature.missing_command_surfaces.is_empty())
        && audit
            .profile_statuses
            .iter()
            .all(|profile| profile.conformant && profile.limitations.is_empty())
        && audit.m11_profile_conformant;
    normalize_strings(&mut audit.external_gate_reasons);
    for architecture in [ReleaseArchitecture::X64, ReleaseArchitecture::Arm64] {
        let Some(lifecycle) = audit
            .lifecycle_statuses
            .iter()
            .find(|status| status.architecture == architecture)
        else {
            audit
                .external_gate_reasons
                .push(format!("lifecycle:{architecture:?}:missing").to_ascii_lowercase());
            continue;
        };
        if lifecycle.support_tier == ReleaseSupportTier::Stable
            && lifecycle.runtime_verification != RuntimeVerificationState::NativeVerified
        {
            audit
                .external_gate_reasons
                .push(format!("lifecycle:{architecture:?}:native_unverified").to_ascii_lowercase());
        }
    }
    normalize_strings(&mut audit.external_gate_reasons);
    audit.status = if !audit.internal_conformance {
        ProductAuditStatusV1::Blocked
    } else if !audit.external_gate_reasons.is_empty() {
        ProductAuditStatusV1::BlockedExternal
    } else if matches!(
        audit.release_status,
        ReleaseStatus::Ready
            | ReleaseStatus::Approved
            | ReleaseStatus::Published
            | ReleaseStatus::Withdrawn
    ) {
        ProductAuditStatusV1::Conformant
    } else {
        ProductAuditStatusV1::Blocked
    };
    audit.audit_fingerprint = versioned_fingerprint(
        FINAL_PRODUCT_AUDIT_V1_SCHEMA_ID,
        1,
        &serde_json::json!({
            "release_manifest_id": audit.release_manifest_id,
            "release_manifest_fingerprint": audit.release_manifest_fingerprint,
            "artifact_set_digest": audit.artifact_set_digest,
            "profile_catalog_fingerprint": audit.profile_catalog_fingerprint,
            "feature_statuses": audit.feature_statuses,
            "profile_statuses": audit.profile_statuses,
            "m11_profile_conformant": audit.m11_profile_conformant,
            "lifecycle_statuses": audit.lifecycle_statuses,
            "internal_conformance": audit.internal_conformance,
            "release_status": audit.release_status,
            "external_gate_reasons": audit.external_gate_reasons,
            "status": audit.status,
        }),
    )
    .map_err(|_| ReleaseError::Fingerprint)?;
    Ok(audit)
}

fn release_external_gate_reasons(manifest: &ReleaseManifestV2) -> Vec<String> {
    let mut reasons = manifest.external_gates.clone();
    for kind in [
        SupplyChainKind::Sbom,
        SupplyChainKind::Provenance,
        SupplyChainKind::Signing,
    ] {
        match manifest
            .supply_chain_applicability
            .iter()
            .find(|decision| decision.kind == kind)
        {
            None => reasons.push(format!("supply_chain:{kind:?}:missing").to_ascii_lowercase()),
            Some(decision)
                if matches!(
                    decision.state,
                    SupplyChainState::RequiredUnavailable | SupplyChainState::RequiredIncomplete
                ) =>
            {
                reasons.push(format!("supply_chain:{kind:?}:incomplete").to_ascii_lowercase());
            }
            Some(decision)
                if kind == SupplyChainKind::Signing
                    && manifest.channel.eq_ignore_ascii_case("stable")
                    && decision.state == SupplyChainState::NotRequired =>
            {
                reasons.push("supply_chain:signing:not_required_for_stable".to_owned());
            }
            Some(_) => {}
        }
    }
    for architecture in [ReleaseArchitecture::X64, ReleaseArchitecture::Arm64] {
        match manifest
            .compatibility
            .iter()
            .find(|target| target.architecture == architecture)
        {
            None => {
                reasons.push(format!("compatibility:{architecture:?}:missing").to_ascii_lowercase())
            }
            Some(target)
                if target.support_tier == ReleaseSupportTier::Stable
                    && target.runtime_verification != RuntimeVerificationState::NativeVerified =>
            {
                reasons.push(
                    format!("compatibility:{architecture:?}:native_unverified")
                        .to_ascii_lowercase(),
                );
            }
            Some(_) => {}
        }
    }
    normalize_strings(&mut reasons);
    reasons
}

fn normalize_strings(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

fn token(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn reference(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && !value.chars().any(char::is_control)
        && value.trim() == value
}

fn placeholder() -> Sha256Hash {
    Sha256Hash::digest(b"unsealed-final-product-audit")
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::{
        ProjectId, ScopeRevisionId, TaskInvocationId, TaskSpecId,
        release_v2::{
            ReleaseCompatibilityTarget, ReleaseIdentityBinding, ReleaseSourceRevision,
            SupplyChainDecision,
        },
    };

    use crate::candidate::{ArtifactBytes, ReleaseCandidateInput, seal_candidate};

    fn manifest() -> ReleaseManifestV2 {
        seal_candidate(
            ReleaseCandidateInput {
                product_id: "star-control".to_owned(),
                version: "0.1.0".to_owned(),
                channel: "github_releases".to_owned(),
                task_spec_ref: TaskSpecId::new(),
                scope_revision_ref: ScopeRevisionId::new(),
                source_revisions: vec![ReleaseSourceRevision {
                    project_id: ProjectId::new(),
                    revision: "a".repeat(40),
                }],
                identity_binding: ReleaseIdentityBinding {
                    config_fingerprint: Sha256Hash::digest(b"config"),
                    catalog_fingerprint: Sha256Hash::digest(b"catalog"),
                    tool_descriptor_fingerprints: vec![Sha256Hash::digest(b"tool")],
                    profile_fingerprint: Sha256Hash::digest(b"profile"),
                    environment_fingerprints: vec![Sha256Hash::digest(b"environment")],
                },
                build_invocation_refs: vec![TaskInvocationId::new()],
                included_files_manifest_ref: "included-files.json".to_owned(),
                metadata_refs: vec!["Cargo.toml".to_owned()],
                supply_chain_applicability: [
                    SupplyChainKind::Sbom,
                    SupplyChainKind::Provenance,
                    SupplyChainKind::Signing,
                ]
                .into_iter()
                .map(|kind| SupplyChainDecision {
                    kind,
                    state: SupplyChainState::Complete,
                    policy_ref: "release-policy-v1".to_owned(),
                    evidence_ref: Some(format!("{kind:?}-evidence")),
                    reason: "fixture".to_owned(),
                })
                .collect(),
                compatibility: vec![
                    ReleaseCompatibilityTarget {
                        architecture: ReleaseArchitecture::X64,
                        support_tier: ReleaseSupportTier::Stable,
                        runtime_verification: RuntimeVerificationState::NativeVerified,
                        minimum_windows_build: 26_100,
                        evidence_refs: vec!["x64-native".to_owned()],
                        limitations: Vec::new(),
                    },
                    ReleaseCompatibilityTarget {
                        architecture: ReleaseArchitecture::Arm64,
                        support_tier: ReleaseSupportTier::Preview,
                        runtime_verification: RuntimeVerificationState::NativeUnverified,
                        minimum_windows_build: 26_100,
                        evidence_refs: vec!["arm64-simulation".to_owned()],
                        limitations: vec!["native_unverified".to_owned()],
                    },
                ],
                validation_refs: vec!["target".to_owned()],
                rollback_plan_ref: "rollback-plan".to_owned(),
                rollback_artifact_ref: None,
                user_data_policy: "preserve".to_owned(),
                remaining_risks: vec!["arm64_native_unverified".to_owned()],
                external_gates: Vec::new(),
            },
            &[
                ArtifactBytes {
                    logical_name: "star-control-x64".to_owned(),
                    role: "archive".to_owned(),
                    architecture: ReleaseArchitecture::X64,
                    media_type: "application/zip".to_owned(),
                    bytes: b"x64".to_vec(),
                },
                ArtifactBytes {
                    logical_name: "star-control-arm64".to_owned(),
                    role: "archive".to_owned(),
                    architecture: ReleaseArchitecture::Arm64,
                    media_type: "application/zip".to_owned(),
                    bytes: b"arm64".to_vec(),
                },
            ],
        )
        .unwrap()
    }

    fn features() -> Vec<ProductFeatureOwnershipStatusV1> {
        PRODUCT_FEATURE_IDS
            .iter()
            .map(|feature_id| ProductFeatureOwnershipStatusV1 {
                feature_id: (*feature_id).to_owned(),
                semantic_owner_ref: format!("docs/{feature_id}.md"),
                physical_owner: format!("owner/{feature_id}"),
                command_surfaces: vec![format!("feature.{}", feature_id.to_ascii_lowercase())],
                missing_command_surfaces: Vec::new(),
            })
            .collect()
    }

    fn profiles() -> Vec<ProductProfileConformanceStatusV1> {
        BUILTIN_DEVELOPMENT_PROFILE_IDS
            .iter()
            .map(|profile_id| ProductProfileConformanceStatusV1 {
                profile_id: (*profile_id).to_owned(),
                profile_version: "1.0.0".to_owned(),
                definition_hash: Sha256Hash::digest(profile_id.as_bytes()),
                resolution_fingerprint: Sha256Hash::digest(
                    format!("resolution:{profile_id}").as_bytes(),
                ),
                conformant: true,
                limitations: Vec::new(),
            })
            .collect()
    }

    #[test]
    fn audit_is_derived_from_exact_feature_profile_and_lifecycle_evidence() {
        let manifest = manifest();
        let artifact_set_digest = manifest.artifact_set_digest.clone().unwrap();
        let audit = build_final_product_audit(
            &manifest,
            Sha256Hash::digest(b"profile-catalog"),
            features(),
            profiles(),
            true,
            vec![
                ProductLifecycleEvidenceStatusV1 {
                    architecture: ReleaseArchitecture::X64,
                    support_tier: ReleaseSupportTier::Stable,
                    runtime_verification: RuntimeVerificationState::NativeVerified,
                    evidence_record_id: "lifecycle-x64".to_owned(),
                    evidence_fingerprint: Sha256Hash::digest(b"lifecycle-x64"),
                    candidate_artifact_set_digest: artifact_set_digest.clone(),
                },
                ProductLifecycleEvidenceStatusV1 {
                    architecture: ReleaseArchitecture::Arm64,
                    support_tier: ReleaseSupportTier::Preview,
                    runtime_verification: RuntimeVerificationState::NativeUnverified,
                    evidence_record_id: "lifecycle-arm64-simulated".to_owned(),
                    evidence_fingerprint: Sha256Hash::digest(b"lifecycle-arm64"),
                    candidate_artifact_set_digest: artifact_set_digest,
                },
            ],
        )
        .unwrap();
        assert!(audit.internal_conformance);
        assert!(audit.external_gate_reasons.is_empty());
        assert_eq!(audit.status, ProductAuditStatusV1::Blocked);
        verify_final_product_audit(&audit).unwrap();

        let mut tampered = audit;
        let command = tampered.feature_statuses[0].command_surfaces[0].clone();
        tampered.feature_statuses[0]
            .missing_command_surfaces
            .push(command);
        assert_eq!(
            verify_final_product_audit(&tampered),
            Err(ReleaseError::Conflict)
        );
    }

    #[test]
    fn stored_release_manifest_tampering_is_rejected() {
        let mut manifest = manifest();
        manifest.artifacts[0].size += 1;
        assert_eq!(
            verify_release_manifest(&manifest),
            Err(ReleaseError::Conflict)
        );
    }
}
