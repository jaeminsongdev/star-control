use std::collections::BTreeSet;

use star_contracts::{
    Sha256Hash, canonical_sha256,
    profile::BUILTIN_DEVELOPMENT_PROFILE_IDS,
    release_v2::{
        FINAL_PRODUCT_AUDIT_V1_SCHEMA_ID, FINAL_PRODUCT_AUDIT_V2_SCHEMA_ID, FinalProductAuditV1,
        FinalProductAuditV2, PRODUCT_SOURCE_EVIDENCE_V1_SCHEMA_ID, ProductAuditStatusV1,
        ProductFeatureOwnershipStatusV1, ProductFeatureSourceEvidenceV1,
        ProductLifecycleEvidenceStatusV1, ProductProfileConformanceStatusV1,
        ProductSourceEvidenceV1, ProductSourceFileEvidenceV1, ReleaseArchitecture,
        ReleaseManifestV2, ReleaseStatus, RuntimeVerificationState, SupplyChainKind,
        SupplyChainState,
    },
};
use star_domain::versioned_fingerprint;

use crate::{ReleaseError, candidate::verify_release_manifest};

pub const PRODUCT_FEATURE_IDS: [&str; 23] = [
    "A01", "A02", "A03", "A04", "A05", "A06", "A07", "A08", "A09", "A10", "B01", "B02", "B03",
    "B04", "B05", "B06", "B07", "B08", "B09", "C01", "D01", "D02", "D03",
];

pub const PRODUCT_RUNTIME_EXECUTABLES: [&str; 4] =
    ["star", "star-controller", "star-mcp", "star-updater"];
pub const PRODUCT_PROFILE_AUDIT_ORDER: [&str; 16] = [
    "project_understanding",
    "docs_config_environment",
    "change_planning",
    "test_correctness",
    "architecture_quality",
    "ai_development_validation",
    "refactor_codemod",
    "api_contract_change",
    "rust_style_auto_fix",
    "debug_recovery",
    "security_supply_chain",
    "dependency_upgrade",
    "data_config_db_migration",
    "performance_build",
    "language_platform_migration",
    "ci_release_deploy",
];
pub const PRODUCT_GENERATED_SCHEMA_COUNT: u32 = 215;
pub const PRODUCT_STABLE_ERROR_COUNT: u32 = 528;
pub const PRODUCT_MCP_MATRIX_COUNT: u32 = 170;
const PRODUCT_PROFILE_CONFORMANCE_REFS: [(&str, &str); 10] = [
    (
        "crates/foundation/star-contracts/src/profile.rs",
        "pub fn resolve_development_profiles",
    ),
    (
        "crates/foundation/star-contracts/src/profile.rs",
        "exact_builtin_set_resolves_deterministically",
    ),
    (
        "crates/control/star-planning/src/lib.rs",
        "fn select_validation_plan",
    ),
    (
        "crates/control/star-planning/src/lib.rs",
        "missing_required_check_is_blocked_not_not_applicable",
    ),
    (
        "apps/star-controller/src/main.rs",
        "fn create_permission_plan",
    ),
    (
        "apps/star-controller/src/main.rs",
        "effective_permission_policy_prompts_or_denies_before_dispatch",
    ),
    ("apps/star-controller/src/main.rs", "fn record_stage_result"),
    (
        "crates/foundation/star-contracts/src/stage.rs",
        "stage_result_recovery_requires_effect_boundary_and_action",
    ),
    (
        "crates/control/star-execution/src/lib.rs",
        "pub fn rollback_applied",
    ),
    (
        "crates/control/star-execution/src/lib.rs",
        "exact_hash_apply_preserves_unrelated_dirty_file_and_safe_rollback_restores_target",
    ),
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
        if lifecycle.runtime_verification != RuntimeVerificationState::NativeVerified {
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

pub fn embedded_product_source_evidence() -> Result<ProductSourceEvidenceV1, ReleaseError> {
    let evidence = serde_json::from_str(include_str!(
        "../../../../catalog/product-source-evidence.json"
    ))
    .map_err(|_| ReleaseError::Invalid)?;
    verify_product_source_evidence(&evidence)?;
    Ok(evidence)
}

pub fn verify_product_source_evidence(
    evidence: &ProductSourceEvidenceV1,
) -> Result<(), ReleaseError> {
    if evidence.schema_id != PRODUCT_SOURCE_EVIDENCE_V1_SCHEMA_ID
        || evidence.schema_version != 1
        || evidence.feature_count != PRODUCT_FEATURE_IDS.len() as u32
        || evidence.profile_count != BUILTIN_DEVELOPMENT_PROFILE_IDS.len() as u32
        || evidence.generated_schema_count != PRODUCT_GENERATED_SCHEMA_COUNT
        || evidence.stable_error_count != PRODUCT_STABLE_ERROR_COUNT
        || evidence.mcp_matrix_count != PRODUCT_MCP_MATRIX_COUNT
        || evidence.runtime_executables != PRODUCT_RUNTIME_EXECUTABLES
    {
        return Err(ReleaseError::Invalid);
    }

    let feature_ids = evidence
        .features
        .iter()
        .map(|feature| feature.feature_id.as_str())
        .collect::<Vec<_>>();
    let profile_ids = evidence
        .profiles
        .iter()
        .map(|profile| profile.profile_id.as_str())
        .collect::<Vec<_>>();
    if feature_ids != PRODUCT_FEATURE_IDS || profile_ids != PRODUCT_PROFILE_AUDIT_ORDER {
        return Err(ReleaseError::Invalid);
    }

    for feature in &evidence.features {
        verify_feature_source_evidence(feature)?;
    }
    for profile in &evidence.profiles {
        verify_profile_source_evidence(profile)?;
    }

    let mut value = serde_json::to_value(evidence).map_err(|_| ReleaseError::Invalid)?;
    value
        .as_object_mut()
        .ok_or(ReleaseError::Invalid)?
        .remove("evidence_fingerprint");
    let fingerprint = canonical_sha256(&value).map_err(|_| ReleaseError::Fingerprint)?;
    if fingerprint != evidence.evidence_fingerprint {
        return Err(ReleaseError::Conflict);
    }
    Ok(())
}

fn verify_profile_source_evidence(
    profile: &star_contracts::release_v2::ProductProfileSourceEvidenceV1,
) -> Result<(), ReleaseError> {
    let unique = |values: &[String]| {
        values.iter().all(|value| reference(value, 256))
            && values.iter().collect::<BTreeSet<_>>().len() == values.len()
    };
    let distinct_refs = profile
        .conformance_refs
        .iter()
        .map(|source| (source.path.as_str(), source.marker.as_deref()))
        .collect::<BTreeSet<_>>();
    let observed_refs = profile
        .conformance_refs
        .iter()
        .map(|source| {
            (
                source.path.as_str(),
                source.marker.as_deref().unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    let expected_definition_path = format!("catalog/profiles/{}.toml", profile.profile_id);
    let expected_policy_fingerprint = canonical_sha256(&serde_json::json!({
        "required_rule_families":profile.required_rule_families,
        "required_check_families":profile.required_check_families,
        "gate_phases":profile.gate_phases,
        "permission_actions":profile.permission_actions,
        "approval_checkpoints":profile.approval_checkpoints,
        "allowed_effect_classes":profile.allowed_effect_classes,
        "permission_floor":profile.permission_floor,
        "unknown_outcome_policy":profile.unknown_outcome_policy,
        "rollback_policy":profile.rollback_policy,
    }))
    .map_err(|_| ReleaseError::Fingerprint)?;
    if !reference(&profile.profile_id, 96)
        || !reference(&profile.profile_version, 96)
        || profile.definition_source.marker.is_some()
        || !verify_source_file(&profile.definition_source, false)
        || profile.definition_source.path != expected_definition_path
        || profile.definition_source.source_sha256 != profile.definition_fingerprint
        || profile.descriptor_definition_hash == Sha256Hash::digest(b"")
        || profile.activation_inputs_fingerprint == Sha256Hash::digest(b"")
        || profile.conformance_policy_fingerprint != expected_policy_fingerprint
        || profile.required_rule_families.is_empty()
        || !unique(&profile.required_rule_families)
        || profile.required_check_families.is_empty()
        || !unique(&profile.required_check_families)
        || profile.gate_phases.is_empty()
        || profile.gate_phases.iter().collect::<BTreeSet<_>>().len() != profile.gate_phases.len()
        || !unique(&profile.permission_actions)
        || profile
            .approval_checkpoints
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != profile.approval_checkpoints.len()
        || profile.allowed_effect_classes.is_empty()
        || profile
            .allowed_effect_classes
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != profile.allowed_effect_classes.len()
        || profile.conformance_refs.len() != 10
        || distinct_refs.len() != profile.conformance_refs.len()
        || observed_refs != PRODUCT_PROFILE_CONFORMANCE_REFS
        || profile.conformance_refs.iter().any(|source| {
            !verify_source_file(source, true) || source.source_sha256 == Sha256Hash::digest(b"")
        })
    {
        return Err(ReleaseError::Invalid);
    }
    Ok(())
}

fn verify_feature_source_evidence(
    feature: &ProductFeatureSourceEvidenceV1,
) -> Result<(), ReleaseError> {
    if !token(&feature.feature_id, 8)
        || feature.generated_schemas.is_empty()
        || feature.handler_refs.is_empty()
        || feature.cli_commands.is_empty()
        || feature.mcp_required == feature.mcp_actions.is_empty()
        || feature.codex_required == feature.codex_refs.is_empty()
        || feature.product_surface_fingerprints.len()
            < 2 + usize::from(feature.mcp_required) + usize::from(feature.codex_required)
        || !verify_source_file(&feature.owner_document, false)
        || feature
            .generated_schemas
            .iter()
            .any(|source| !verify_source_file(source, false))
        || feature
            .handler_refs
            .iter()
            .any(|source| !verify_source_file(source, true))
        || feature
            .cli_commands
            .iter()
            .any(|command| !token(command, 96))
        || feature.mcp_actions.iter().any(|action| !token(action, 96))
        || feature
            .codex_refs
            .iter()
            .any(|source| !verify_source_file(source, true))
    {
        return Err(ReleaseError::Invalid);
    }

    let unique_surfaces = feature
        .product_surface_fingerprints
        .iter()
        .collect::<BTreeSet<_>>();
    if unique_surfaces.len() != feature.product_surface_fingerprints.len() {
        return Err(ReleaseError::Invalid);
    }

    let tests = [
        &feature.test_refs.positive,
        &feature.test_refs.negative,
        &feature.test_refs.failure,
        &feature.test_refs.recovery,
    ];
    if tests.iter().any(|source| !verify_source_file(source, true)) {
        return Err(ReleaseError::Invalid);
    }
    let distinct_tests = tests
        .iter()
        .map(|source| (source.path.as_str(), source.marker.as_deref()))
        .collect::<BTreeSet<_>>();
    if distinct_tests.len() != tests.len() {
        return Err(ReleaseError::Invalid);
    }

    let mut value = serde_json::to_value(feature).map_err(|_| ReleaseError::Invalid)?;
    value
        .as_object_mut()
        .ok_or(ReleaseError::Invalid)?
        .remove("feature_fingerprint");
    let fingerprint = canonical_sha256(&value).map_err(|_| ReleaseError::Fingerprint)?;
    if fingerprint != feature.feature_fingerprint {
        return Err(ReleaseError::Conflict);
    }
    Ok(())
}

fn verify_source_file(source: &ProductSourceFileEvidenceV1, marker_required: bool) -> bool {
    let path = source.path.as_str();
    let safe_path = reference(path, 320)
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains(':')
        && !path.split('/').any(|component| component == "..");
    let marker_valid = match (&source.marker, marker_required) {
        (Some(marker), _) => reference(marker, 320),
        (None, false) => true,
        (None, true) => false,
    };
    safe_path && marker_valid
}

pub fn build_final_product_audit_v2(
    manifest: &ReleaseManifestV2,
    source_evidence: ProductSourceEvidenceV1,
    profile_catalog_fingerprint: Sha256Hash,
    profile_statuses: Vec<ProductProfileConformanceStatusV1>,
    m11_profile_conformant: bool,
    lifecycle_statuses: Vec<ProductLifecycleEvidenceStatusV1>,
) -> Result<FinalProductAuditV2, ReleaseError> {
    verify_release_manifest(manifest)?;
    verify_product_source_evidence(&source_evidence)?;
    let artifact_set_digest = manifest
        .artifact_set_digest
        .clone()
        .ok_or(ReleaseError::Invalid)?;
    seal_final_product_audit_v2(FinalProductAuditV2 {
        schema_id: FINAL_PRODUCT_AUDIT_V2_SCHEMA_ID.to_owned(),
        schema_version: 2,
        release_manifest_id: manifest.release_manifest_id.clone(),
        release_manifest_fingerprint: manifest.manifest_fingerprint.clone(),
        artifact_set_digest,
        source_evidence,
        profile_catalog_fingerprint,
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

pub fn verify_final_product_audit_v2(audit: &FinalProductAuditV2) -> Result<(), ReleaseError> {
    let sealed = seal_final_product_audit_v2(audit.clone())?;
    if &sealed != audit {
        return Err(ReleaseError::Conflict);
    }
    Ok(())
}

fn seal_final_product_audit_v2(
    mut audit: FinalProductAuditV2,
) -> Result<FinalProductAuditV2, ReleaseError> {
    if audit.schema_id != FINAL_PRODUCT_AUDIT_V2_SCHEMA_ID || audit.schema_version != 2 {
        return Err(ReleaseError::Invalid);
    }
    verify_product_source_evidence(&audit.source_evidence)?;

    for profile in &mut audit.profile_statuses {
        normalize_strings(&mut profile.required_check_families);
        normalize_strings(&mut profile.covered_check_families);
        normalize_strings(&mut profile.limitations);
        if !reference(&profile.profile_id, 96)
            || !reference(&profile.profile_version, 96)
            || profile.required_check_families.is_empty()
            || profile
                .required_check_families
                .iter()
                .any(|family| !reference(family, 128))
            || profile
                .covered_check_families
                .iter()
                .any(|family| !reference(family, 128))
        {
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
    if profile_ids != BUILTIN_DEVELOPMENT_PROFILE_IDS
        || lifecycle_architectures.len() != audit.lifecycle_statuses.len()
        || audit.profile_statuses.iter().any(|runtime| {
            !audit.source_evidence.profiles.iter().any(|source| {
                runtime.profile_id == source.profile_id
                    && runtime.profile_version == source.profile_version
            })
        })
    {
        return Err(ReleaseError::Invalid);
    }

    for runtime in &mut audit.profile_statuses {
        let source = audit
            .source_evidence
            .profiles
            .iter()
            .find(|source| source.profile_id == runtime.profile_id)
            .ok_or(ReleaseError::Invalid)?;
        let source_checks = source
            .required_check_families
            .iter()
            .collect::<BTreeSet<_>>();
        let runtime_checks = runtime
            .required_check_families
            .iter()
            .collect::<BTreeSet<_>>();
        let binding_exact = runtime.definition_hash == source.descriptor_definition_hash
            && runtime.source_definition_fingerprint == source.definition_fingerprint
            && runtime.activation_inputs_fingerprint == source.activation_inputs_fingerprint
            && runtime.conformance_policy_fingerprint == source.conformance_policy_fingerprint
            && runtime.project_context_fingerprint != Sha256Hash::digest(b"")
            && runtime.effective_config_fingerprint != Sha256Hash::digest(b"")
            && runtime.toolchain_fingerprint != Sha256Hash::digest(b"")
            && source_checks.is_subset(&runtime_checks)
            && runtime.required_check_families == runtime.covered_check_families
            && runtime.approval_path_verified
            && runtime.unknown_outcome_path_verified
            && runtime.rollback_path_verified;
        if !binding_exact {
            runtime
                .limitations
                .push("profile_runtime_binding_incomplete".to_owned());
        }
        normalize_strings(&mut runtime.limitations);
        runtime.conformant = runtime.conformant && binding_exact && runtime.limitations.is_empty();
    }

    audit.internal_conformance = audit
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
        if lifecycle.runtime_verification != RuntimeVerificationState::NativeVerified {
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
        FINAL_PRODUCT_AUDIT_V2_SCHEMA_ID,
        2,
        &serde_json::json!({
            "release_manifest_id": audit.release_manifest_id,
            "release_manifest_fingerprint": audit.release_manifest_fingerprint,
            "artifact_set_digest": audit.artifact_set_digest,
            "source_evidence": audit.source_evidence,
            "profile_catalog_fingerprint": audit.profile_catalog_fingerprint,
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
                if target.runtime_verification != RuntimeVerificationState::NativeVerified =>
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
            ReleaseSupportTier, SupplyChainDecision,
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
        let source_evidence = embedded_product_source_evidence().unwrap();
        BUILTIN_DEVELOPMENT_PROFILE_IDS
            .iter()
            .map(|profile_id| {
                let source = source_evidence
                    .profiles
                    .iter()
                    .find(|source| source.profile_id == *profile_id)
                    .unwrap();
                ProductProfileConformanceStatusV1 {
                    profile_id: (*profile_id).to_owned(),
                    profile_version: source.profile_version.clone(),
                    definition_hash: source.descriptor_definition_hash.clone(),
                    source_definition_fingerprint: source.definition_fingerprint.clone(),
                    resolution_fingerprint: Sha256Hash::digest(
                        format!("resolution:{profile_id}").as_bytes(),
                    ),
                    activation_inputs_fingerprint: source.activation_inputs_fingerprint.clone(),
                    conformance_policy_fingerprint: source.conformance_policy_fingerprint.clone(),
                    project_context_fingerprint: Sha256Hash::digest(b"project-context"),
                    effective_config_fingerprint: Sha256Hash::digest(b"effective-config"),
                    toolchain_fingerprint: Sha256Hash::digest(b"toolchain"),
                    required_check_families: source.required_check_families.clone(),
                    covered_check_families: source.required_check_families.clone(),
                    approval_path_verified: true,
                    unknown_outcome_path_verified: true,
                    rollback_path_verified: true,
                    conformant: true,
                    limitations: Vec::new(),
                }
            })
            .collect()
    }

    fn lifecycle_statuses(manifest: &ReleaseManifestV2) -> Vec<ProductLifecycleEvidenceStatusV1> {
        let artifact_set_digest = manifest.artifact_set_digest.clone().unwrap();
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
        ]
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
        assert!(
            audit
                .external_gate_reasons
                .iter()
                .any(|reason| reason == "lifecycle:arm64:native_unverified")
        );
        assert_eq!(audit.status, ProductAuditStatusV1::BlockedExternal);
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

    #[test]
    fn v2_audit_binds_current_source_inventory_profiles_and_lifecycle() {
        let manifest = manifest();
        let source_evidence = embedded_product_source_evidence().unwrap();
        let audit = build_final_product_audit_v2(
            &manifest,
            source_evidence,
            Sha256Hash::digest(b"profile-catalog-v2"),
            profiles(),
            true,
            lifecycle_statuses(&manifest),
        )
        .unwrap();

        assert_eq!(audit.source_evidence.feature_count, 23);
        assert_eq!(audit.source_evidence.profile_count, 16);
        assert_eq!(audit.source_evidence.generated_schema_count, 215);
        assert!(audit.internal_conformance);
        assert_eq!(audit.status, ProductAuditStatusV1::BlockedExternal);
        assert!(
            audit
                .external_gate_reasons
                .iter()
                .any(|reason| reason == "lifecycle:arm64:native_unverified")
        );
        verify_final_product_audit_v2(&audit).unwrap();
    }

    #[test]
    fn v2_source_evidence_rejects_tampering_and_collapsed_test_classes() {
        let mut tampered = embedded_product_source_evidence().unwrap();
        tampered.features[0].handler_refs[0].source_sha256 = Sha256Hash::digest(b"tampered");
        assert_eq!(
            verify_product_source_evidence(&tampered),
            Err(ReleaseError::Conflict)
        );

        let mut collapsed = embedded_product_source_evidence().unwrap();
        collapsed.features[0].test_refs.recovery = collapsed.features[0].test_refs.positive.clone();
        assert_eq!(
            verify_product_source_evidence(&collapsed),
            Err(ReleaseError::Invalid)
        );

        let mut profile_path_swapped = embedded_product_source_evidence().unwrap();
        profile_path_swapped.profiles[0].conformance_refs.swap(0, 1);
        assert_eq!(
            verify_product_source_evidence(&profile_path_swapped),
            Err(ReleaseError::Invalid)
        );
    }

    #[test]
    fn v2_audit_downgrades_incomplete_profile_check_coverage() {
        let manifest = manifest();
        let mut profile_statuses = profiles();
        profile_statuses[0].covered_check_families.clear();
        let audit = build_final_product_audit_v2(
            &manifest,
            embedded_product_source_evidence().unwrap(),
            Sha256Hash::digest(b"profile-catalog-v2"),
            profile_statuses,
            true,
            lifecycle_statuses(&manifest),
        )
        .unwrap();

        assert!(!audit.internal_conformance);
        assert!(!audit.profile_statuses[0].conformant);
        assert!(
            audit.profile_statuses[0]
                .limitations
                .iter()
                .any(|limitation| limitation == "profile_runtime_binding_incomplete")
        );
        verify_final_product_audit_v2(&audit).unwrap();
    }

    #[test]
    fn v2_audit_downgrades_stale_profile_descriptor_binding() {
        let manifest = manifest();
        let mut profile_statuses = profiles();
        profile_statuses[0].definition_hash = Sha256Hash::digest(b"stale-profile-definition");
        let audit = build_final_product_audit_v2(
            &manifest,
            embedded_product_source_evidence().unwrap(),
            Sha256Hash::digest(b"profile-catalog-v2"),
            profile_statuses,
            true,
            lifecycle_statuses(&manifest),
        )
        .unwrap();

        assert!(!audit.internal_conformance);
        assert!(!audit.profile_statuses[0].conformant);
        assert!(
            audit.profile_statuses[0]
                .limitations
                .iter()
                .any(|limitation| limitation == "profile_runtime_binding_incomplete")
        );
        verify_final_product_audit_v2(&audit).unwrap();
    }
}
