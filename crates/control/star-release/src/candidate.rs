use std::collections::{BTreeMap, BTreeSet};

use star_contracts::{
    ApprovalId, GateId, ReleaseManifestId, Sha256Hash, TaskInvocationId, ValidationPlanId,
    ValidationRunId,
    release_v2::{
        EvidenceCompleteness, RELEASE_MANIFEST_V2_SCHEMA_ID, ReleaseArchitecture,
        ReleaseArtifactV2, ReleaseCompatibilityTarget, ReleaseIdentityBinding, ReleaseManifestV2,
        ReleaseRemoteAction, ReleaseSourceRevision, ReleaseStatus, ReleaseSupportTier,
        ReleaseVerificationLayer, RemoteActionKind, RemoteActionState, RuntimeVerificationState,
        SupplyChainDecision, SupplyChainKind, SupplyChainState, VerificationLayerKind,
    },
};
use star_domain::versioned_fingerprint;

use crate::ReleaseError;

#[derive(Clone, Debug)]
pub struct ArtifactBytes {
    pub logical_name: String,
    pub role: String,
    pub architecture: ReleaseArchitecture,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct ReleaseCandidateInput {
    pub product_id: String,
    pub version: String,
    pub channel: String,
    pub task_spec_ref: star_contracts::TaskSpecId,
    pub scope_revision_ref: star_contracts::ScopeRevisionId,
    pub source_revisions: Vec<ReleaseSourceRevision>,
    pub identity_binding: ReleaseIdentityBinding,
    pub build_invocation_refs: Vec<TaskInvocationId>,
    pub included_files_manifest_ref: String,
    pub metadata_refs: Vec<String>,
    pub supply_chain_applicability: Vec<SupplyChainDecision>,
    pub compatibility: Vec<ReleaseCompatibilityTarget>,
    pub validation_refs: Vec<String>,
    pub rollback_plan_ref: String,
    pub rollback_artifact_ref: Option<String>,
    pub user_data_policy: String,
    pub remaining_risks: Vec<String>,
    pub external_gates: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct VerificationObservation {
    pub completeness: EvidenceCompleteness,
    pub artifact_set_digest: Option<Sha256Hash>,
    pub validation_plan_ref: ValidationPlanId,
    pub validation_run_ref: Option<ValidationRunId>,
    pub gate_ref: Option<GateId>,
    pub limitations: Vec<String>,
}

pub trait CiAdapter {
    fn verify(
        &mut self,
        layer: VerificationLayerKind,
        artifact_set_digest: &Sha256Hash,
    ) -> VerificationObservation;
}

pub trait SignerAdapter {
    fn sign(
        &mut self,
        artifact: &ReleaseArtifactV2,
        unsigned_bytes: &[u8],
    ) -> Result<SignedBytes, ReleaseError>;
}

#[derive(Clone, Debug)]
pub struct SignedBytes {
    pub bytes: Vec<u8>,
    pub signature_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublishObservation {
    Verified {
        artifact_set_digest: Sha256Hash,
        snapshot_ref: String,
    },
    Partial {
        receipt_ref: String,
    },
    Timeout,
    Failed,
}

pub trait PublisherAdapter {
    fn publish(&mut self, manifest: &ReleaseManifestV2) -> PublishObservation;
    fn reconcile(&mut self, manifest: &ReleaseManifestV2) -> PublishObservation;
}

#[derive(Default)]
pub struct BuildOnceStore {
    candidates: BTreeMap<ReleaseManifestId, Sha256Hash>,
}

impl BuildOnceStore {
    pub fn insert(&mut self, manifest: &ReleaseManifestV2) -> Result<bool, ReleaseError> {
        let digest = manifest
            .artifact_set_digest
            .clone()
            .ok_or(ReleaseError::Invalid)?;
        match self.candidates.get(&manifest.release_manifest_id) {
            Some(existing) if existing == &digest => Ok(false),
            Some(_) => Err(ReleaseError::Conflict),
            None => {
                self.candidates
                    .insert(manifest.release_manifest_id.clone(), digest);
                Ok(true)
            }
        }
    }
}

pub fn seal_candidate(
    input: ReleaseCandidateInput,
    artifacts: &[ArtifactBytes],
) -> Result<ReleaseManifestV2, ReleaseError> {
    validate_candidate_input(&input)?;
    let artifact_entries = artifact_entries(artifacts)?;
    let artifact_set_digest = artifact_set_digest(&artifact_entries)?;
    let status = if input
        .supply_chain_applicability
        .iter()
        .any(|item| item.state == SupplyChainState::RequiredUnavailable)
        || !input.external_gates.is_empty()
    {
        ReleaseStatus::BlockedExternal
    } else if input
        .supply_chain_applicability
        .iter()
        .any(|item| item.state == SupplyChainState::RequiredIncomplete)
    {
        ReleaseStatus::Blocked
    } else {
        ReleaseStatus::Candidate
    };
    seal_manifest(ReleaseManifestV2 {
        schema_id: RELEASE_MANIFEST_V2_SCHEMA_ID.to_owned(),
        schema_version: 2,
        release_manifest_id: ReleaseManifestId::new(),
        revision: 1,
        supersedes: None,
        product_id: input.product_id,
        version: input.version,
        channel: input.channel,
        task_spec_ref: input.task_spec_ref,
        scope_revision_ref: input.scope_revision_ref,
        source_revisions: input.source_revisions,
        identity_binding: input.identity_binding,
        verification_layers: Vec::new(),
        build_invocation_refs: input.build_invocation_refs,
        artifacts: artifact_entries,
        artifact_set_digest: Some(artifact_set_digest),
        included_files_manifest_ref: Some(input.included_files_manifest_ref),
        metadata_refs: input.metadata_refs,
        supply_chain_applicability: input.supply_chain_applicability,
        sbom_ref: None,
        provenance_ref: None,
        signature_refs: Vec::new(),
        compatibility: input.compatibility,
        validation_refs: input.validation_refs,
        release_gate_refs: Vec::new(),
        remote_actions: Vec::new(),
        approval_request_refs: Vec::new(),
        remote_operation_refs: Vec::new(),
        before_remote_snapshot_refs: Vec::new(),
        after_remote_snapshot_refs: Vec::new(),
        rollback_plan_ref: input.rollback_plan_ref,
        rollback_artifact_ref: input.rollback_artifact_ref,
        user_data_policy: input.user_data_policy,
        remaining_risks: input.remaining_risks,
        external_gates: input.external_gates,
        status,
        manifest_fingerprint: placeholder(),
    })
}

pub fn verify_artifact_bytes(
    manifest: &ReleaseManifestV2,
    artifacts: &[ArtifactBytes],
) -> Result<(), ReleaseError> {
    let entries = artifact_entries(artifacts)?;
    if entries != manifest.artifacts
        || Some(artifact_set_digest(&entries)?) != manifest.artifact_set_digest
    {
        return Err(ReleaseError::Conflict);
    }
    Ok(())
}

pub fn run_ci_layers(
    mut manifest: ReleaseManifestV2,
    adapter: &mut impl CiAdapter,
) -> Result<ReleaseManifestV2, ReleaseError> {
    if !matches!(
        manifest.status,
        ReleaseStatus::Candidate | ReleaseStatus::Blocked
    ) {
        return Err(ReleaseError::Blocked);
    }
    let digest = manifest
        .artifact_set_digest
        .clone()
        .ok_or(ReleaseError::Invalid)?;
    let mut layers = Vec::new();
    let mut blocked = false;
    for layer in [
        VerificationLayerKind::LocalQuick,
        VerificationLayerKind::Target,
        VerificationLayerKind::Full,
        VerificationLayerKind::Release,
    ] {
        let observation = adapter.verify(layer, &digest);
        if observation.completeness != EvidenceCompleteness::Complete
            || observation.artifact_set_digest.as_ref() != Some(&digest)
            || observation.validation_run_ref.is_none()
            || observation.gate_ref.is_none()
        {
            blocked = true;
        }
        layers.push(ReleaseVerificationLayer {
            layer,
            validation_plan_ref: observation.validation_plan_ref,
            validation_run_ref: observation.validation_run_ref,
            gate_ref: observation.gate_ref,
            completeness: observation.completeness,
            artifact_set_digest: observation.artifact_set_digest,
            limitations: observation.limitations,
        });
    }
    manifest.verification_layers = layers;
    manifest.release_gate_refs = manifest
        .verification_layers
        .iter()
        .filter_map(|layer| layer.gate_ref.clone())
        .collect();
    manifest.status = if blocked {
        ReleaseStatus::Blocked
    } else {
        ReleaseStatus::Candidate
    };
    manifest.revision = manifest.revision.saturating_add(1);
    seal_manifest(manifest)
}

pub fn sign_candidate(
    manifest: &ReleaseManifestV2,
    unsigned_artifacts: &[ArtifactBytes],
    signer: &mut impl SignerAdapter,
) -> Result<(ReleaseManifestV2, Vec<ArtifactBytes>), ReleaseError> {
    verify_artifact_bytes(manifest, unsigned_artifacts)?;
    let by_key = unsigned_artifacts
        .iter()
        .map(|artifact| {
            (
                (artifact.logical_name.as_str(), artifact.architecture),
                artifact,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut signed_artifacts = Vec::with_capacity(manifest.artifacts.len());
    let mut signature_refs = Vec::new();
    for entry in &manifest.artifacts {
        let unsigned = by_key
            .get(&(entry.logical_name.as_str(), entry.architecture))
            .ok_or(ReleaseError::Conflict)?;
        let signed = signer.sign(entry, &unsigned.bytes)?;
        if signed.bytes == unsigned.bytes || signed.signature_ref.trim().is_empty() {
            return Err(ReleaseError::Adapter);
        }
        signed_artifacts.push(ArtifactBytes {
            logical_name: unsigned.logical_name.clone(),
            role: unsigned.role.clone(),
            architecture: unsigned.architecture,
            media_type: unsigned.media_type.clone(),
            bytes: signed.bytes,
        });
        signature_refs.push(signed.signature_ref);
    }
    signature_refs.sort();
    signature_refs.dedup();
    let signed_entries = artifact_entries(&signed_artifacts)?;
    let signed_digest = artifact_set_digest(&signed_entries)?;
    if Some(&signed_digest) == manifest.artifact_set_digest.as_ref() {
        return Err(ReleaseError::Conflict);
    }
    let mut signed = manifest.clone();
    signed.release_manifest_id = ReleaseManifestId::new();
    signed.revision = 1;
    signed.supersedes = Some(format!(
        "{}@{}",
        manifest.release_manifest_id, manifest.revision
    ));
    signed.artifacts = signed_entries;
    signed.artifact_set_digest = Some(signed_digest);
    signed.signature_refs = signature_refs;
    for decision in &mut signed.supply_chain_applicability {
        if decision.kind == SupplyChainKind::Signing {
            decision.state = SupplyChainState::Complete;
            decision.evidence_ref = Some("signer-receipt-set".to_owned());
            decision.reason = "final artifact bytes were signed".to_owned();
        }
    }
    signed.verification_layers.clear();
    signed.release_gate_refs.clear();
    signed.remote_actions.clear();
    signed.approval_request_refs.clear();
    signed.remote_operation_refs.clear();
    signed.before_remote_snapshot_refs.clear();
    signed.after_remote_snapshot_refs.clear();
    signed.status = ReleaseStatus::Candidate;
    signed
        .remaining_risks
        .push("signed_bytes_require_fresh_release_verification".to_owned());
    signed.remaining_risks.sort();
    signed.remaining_risks.dedup();
    Ok((seal_manifest(signed)?, signed_artifacts))
}

pub fn promote_ready(mut manifest: ReleaseManifestV2) -> Result<ReleaseManifestV2, ReleaseError> {
    let digest = manifest
        .artifact_set_digest
        .as_ref()
        .ok_or(ReleaseError::Invalid)?;
    let layers = manifest
        .verification_layers
        .iter()
        .map(|layer| layer.layer)
        .collect::<BTreeSet<_>>();
    let required_layers = BTreeSet::from([
        VerificationLayerKind::LocalQuick,
        VerificationLayerKind::Target,
        VerificationLayerKind::Full,
        VerificationLayerKind::Release,
    ]);
    let evidence_ready = layers == required_layers
        && manifest.verification_layers.iter().all(|layer| {
            layer.completeness == EvidenceCompleteness::Complete
                && layer.artifact_set_digest.as_ref() == Some(digest)
                && layer.validation_run_ref.is_some()
                && layer.gate_ref.is_some()
        });
    let supply_chain_ready = manifest.supply_chain_applicability.iter().all(|decision| {
        matches!(
            decision.state,
            SupplyChainState::NotRequired | SupplyChainState::Complete
        )
    });
    let x64_ready = manifest.compatibility.iter().any(|target| {
        target.architecture == ReleaseArchitecture::X64
            && target.support_tier == ReleaseSupportTier::Stable
            && target.runtime_verification == RuntimeVerificationState::NativeVerified
            && !target.evidence_refs.is_empty()
    });
    let arm64_ready = manifest.compatibility.iter().any(|target| {
        target.architecture == ReleaseArchitecture::Arm64
            && target.support_tier == ReleaseSupportTier::Preview
            && target.runtime_verification == RuntimeVerificationState::NativeUnverified
            && !target.evidence_refs.is_empty()
            && target
                .limitations
                .iter()
                .any(|item| item == "native_unverified")
    });
    if !manifest.external_gates.is_empty() {
        manifest.status = ReleaseStatus::BlockedExternal;
    } else if evidence_ready && supply_chain_ready && x64_ready && arm64_ready {
        manifest.status = ReleaseStatus::Ready;
        manifest
            .remaining_risks
            .retain(|risk| risk != "signed_bytes_require_fresh_release_verification");
    } else {
        manifest.status = ReleaseStatus::Blocked;
    }
    manifest.revision = manifest.revision.saturating_add(1);
    seal_manifest(manifest)
}

pub fn approve_publish(
    mut manifest: ReleaseManifestV2,
    approval: ApprovalId,
    expected_digest: &Sha256Hash,
    destination: &str,
) -> Result<ReleaseManifestV2, ReleaseError> {
    if manifest.status != ReleaseStatus::Ready
        || manifest.artifact_set_digest.as_ref() != Some(expected_digest)
        || destination != "github:jaeminsongdev/star-control:releases"
    {
        return Err(ReleaseError::Blocked);
    }
    manifest.approval_request_refs.push(approval.clone());
    manifest.remote_actions.push(ReleaseRemoteAction {
        action_id: "github-release-publish".to_owned(),
        kind: RemoteActionKind::Publish,
        provider: "github".to_owned(),
        destination: destination.to_owned(),
        immutable_subject_digest: expected_digest.clone(),
        state: RemoteActionState::Approved,
        approval_request_ref: Some(approval),
        before_snapshot_ref: Some("github-release-before-snapshot".to_owned()),
        after_snapshot_ref: None,
        receipt_ref: None,
    });
    manifest.status = ReleaseStatus::Approved;
    manifest.revision = manifest.revision.saturating_add(1);
    seal_manifest(manifest)
}

pub fn publish_with_reconcile(
    mut manifest: ReleaseManifestV2,
    publisher: &mut impl PublisherAdapter,
) -> Result<ReleaseManifestV2, ReleaseError> {
    if manifest.status != ReleaseStatus::Approved || manifest.remote_actions.len() != 1 {
        return Err(ReleaseError::Blocked);
    }
    manifest.status = ReleaseStatus::Publishing;
    manifest.remote_actions[0].state = RemoteActionState::Running;
    let observation = publisher.publish(&manifest);
    let observation = if observation == PublishObservation::Timeout {
        publisher.reconcile(&manifest)
    } else {
        observation
    };
    apply_publish_observation(&mut manifest, observation);
    manifest.revision = manifest.revision.saturating_add(1);
    seal_manifest(manifest)
}

fn apply_publish_observation(manifest: &mut ReleaseManifestV2, observation: PublishObservation) {
    let expected = manifest.artifact_set_digest.as_ref();
    let action = &mut manifest.remote_actions[0];
    match observation {
        PublishObservation::Verified {
            artifact_set_digest,
            snapshot_ref,
        } if Some(&artifact_set_digest) == expected => {
            action.state = RemoteActionState::Verified;
            action.after_snapshot_ref = Some(snapshot_ref.clone());
            manifest.after_remote_snapshot_refs.push(snapshot_ref);
            manifest.status = ReleaseStatus::Published;
        }
        PublishObservation::Verified { snapshot_ref, .. } => {
            action.state = RemoteActionState::RollbackRequired;
            action.after_snapshot_ref = Some(snapshot_ref.clone());
            manifest.after_remote_snapshot_refs.push(snapshot_ref);
            manifest.status = ReleaseStatus::RollbackRequired;
        }
        PublishObservation::Partial { receipt_ref } => {
            action.state = RemoteActionState::RollbackRequired;
            action.receipt_ref = Some(receipt_ref);
            manifest.status = ReleaseStatus::RollbackRequired;
        }
        PublishObservation::Timeout => {
            action.state = RemoteActionState::OutcomeUnknown;
            manifest.status = ReleaseStatus::PublishOutcomeUnknown;
        }
        PublishObservation::Failed => {
            action.state = RemoteActionState::Failed;
            manifest.status = ReleaseStatus::Ready;
        }
    }
}

fn validate_candidate_input(input: &ReleaseCandidateInput) -> Result<(), ReleaseError> {
    if !token(&input.product_id, 128)
        || input.version != "0.1.0"
        || input.channel != "github_releases"
        || input.source_revisions.is_empty()
        || input.build_invocation_refs.is_empty()
        || input.included_files_manifest_ref.trim().is_empty()
        || input.rollback_plan_ref.trim().is_empty()
        || input.user_data_policy.trim().is_empty()
    {
        return Err(ReleaseError::Invalid);
    }
    let unique_projects = input
        .source_revisions
        .iter()
        .map(|source| source.project_id.as_str())
        .collect::<BTreeSet<_>>();
    if unique_projects.len() != input.source_revisions.len()
        || input.source_revisions.iter().any(|source| {
            !matches!(source.revision.len(), 40 | 64)
                || source
                    .revision
                    .bytes()
                    .any(|byte| !byte.is_ascii_hexdigit())
        })
    {
        return Err(ReleaseError::Conflict);
    }
    let supply_kinds = input
        .supply_chain_applicability
        .iter()
        .map(|decision| decision.kind)
        .collect::<BTreeSet<_>>();
    if supply_kinds
        != BTreeSet::from([
            SupplyChainKind::Sbom,
            SupplyChainKind::Provenance,
            SupplyChainKind::Signing,
        ])
    {
        return Err(ReleaseError::Invalid);
    }
    Ok(())
}

fn artifact_entries(artifacts: &[ArtifactBytes]) -> Result<Vec<ReleaseArtifactV2>, ReleaseError> {
    if artifacts.is_empty() {
        return Err(ReleaseError::Invalid);
    }
    let mut entries = artifacts
        .iter()
        .map(|artifact| {
            if !token(&artifact.logical_name, 128)
                || !token(&artifact.role, 128)
                || artifact.media_type.trim().is_empty()
                || artifact.bytes.is_empty()
            {
                return Err(ReleaseError::Invalid);
            }
            Ok(ReleaseArtifactV2 {
                logical_name: artifact.logical_name.clone(),
                role: artifact.role.clone(),
                architecture: artifact.architecture,
                size: artifact.bytes.len() as u64,
                media_type: artifact.media_type.clone(),
                sha256: Sha256Hash::digest(&artifact.bytes),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    if entries.windows(2).any(|pair| {
        pair[0].logical_name == pair[1].logical_name && pair[0].architecture == pair[1].architecture
    }) {
        return Err(ReleaseError::Conflict);
    }
    Ok(entries)
}

fn artifact_set_digest(entries: &[ReleaseArtifactV2]) -> Result<Sha256Hash, ReleaseError> {
    versioned_fingerprint("star.release-artifact-set", 2, &entries)
        .map_err(|_| ReleaseError::Fingerprint)
}

fn seal_manifest(mut manifest: ReleaseManifestV2) -> Result<ReleaseManifestV2, ReleaseError> {
    manifest.manifest_fingerprint = versioned_fingerprint(
        RELEASE_MANIFEST_V2_SCHEMA_ID,
        2,
        &serde_json::json!({
            "release_manifest_id":manifest.release_manifest_id,
            "revision":manifest.revision,
            "supersedes":manifest.supersedes,
            "product_id":manifest.product_id,
            "version":manifest.version,
            "channel":manifest.channel,
            "task_spec_ref":manifest.task_spec_ref,
            "scope_revision_ref":manifest.scope_revision_ref,
            "source_revisions":manifest.source_revisions,
            "identity_binding":manifest.identity_binding,
            "verification_layers":manifest.verification_layers,
            "build_invocation_refs":manifest.build_invocation_refs,
            "artifacts":manifest.artifacts,
            "artifact_set_digest":manifest.artifact_set_digest,
            "included_files_manifest_ref":manifest.included_files_manifest_ref,
            "metadata_refs":manifest.metadata_refs,
            "supply_chain_applicability":manifest.supply_chain_applicability,
            "sbom_ref":manifest.sbom_ref,
            "provenance_ref":manifest.provenance_ref,
            "signature_refs":manifest.signature_refs,
            "compatibility":manifest.compatibility,
            "validation_refs":manifest.validation_refs,
            "release_gate_refs":manifest.release_gate_refs,
            "remote_actions":manifest.remote_actions,
            "approval_request_refs":manifest.approval_request_refs,
            "remote_operation_refs":manifest.remote_operation_refs,
            "before_remote_snapshot_refs":manifest.before_remote_snapshot_refs,
            "after_remote_snapshot_refs":manifest.after_remote_snapshot_refs,
            "rollback_plan_ref":manifest.rollback_plan_ref,
            "rollback_artifact_ref":manifest.rollback_artifact_ref,
            "user_data_policy":manifest.user_data_policy,
            "remaining_risks":manifest.remaining_risks,
            "external_gates":manifest.external_gates,
            "status":manifest.status,
        }),
    )
    .map_err(|_| ReleaseError::Fingerprint)?;
    Ok(manifest)
}

fn placeholder() -> Sha256Hash {
    Sha256Hash::digest(b"unsealed-release-manifest")
}

fn token(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::{
        ProjectId, ScopeRevisionId, TaskSpecId,
        release_v2::{ReleaseCompatibilityTarget, SupplyChainDecision},
    };

    fn supply(state: SupplyChainState) -> Vec<SupplyChainDecision> {
        [
            SupplyChainKind::Sbom,
            SupplyChainKind::Provenance,
            SupplyChainKind::Signing,
        ]
        .into_iter()
        .map(|kind| SupplyChainDecision {
            kind,
            state,
            policy_ref: "release-policy-v1".to_owned(),
            evidence_ref: (state == SupplyChainState::Complete)
                .then(|| format!("{kind:?}-evidence")),
            reason: "fixture".to_owned(),
        })
        .collect()
    }

    fn artifacts() -> Vec<ArtifactBytes> {
        vec![
            ArtifactBytes {
                logical_name: "star-control-x64-setup".to_owned(),
                role: "installer".to_owned(),
                architecture: ReleaseArchitecture::X64,
                media_type: "application/vnd.microsoft.portable-executable".to_owned(),
                bytes: b"MZ-x64-installer".to_vec(),
            },
            ArtifactBytes {
                logical_name: "star-control-arm64-setup".to_owned(),
                role: "installer".to_owned(),
                architecture: ReleaseArchitecture::Arm64,
                media_type: "application/vnd.microsoft.portable-executable".to_owned(),
                bytes: b"MZ-arm64-installer".to_vec(),
            },
        ]
    }

    fn input(supply_state: SupplyChainState) -> ReleaseCandidateInput {
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
            metadata_refs: vec!["Cargo.toml".to_owned(), "CHANGELOG.md".to_owned()],
            supply_chain_applicability: supply(supply_state),
            compatibility: vec![
                ReleaseCompatibilityTarget {
                    architecture: ReleaseArchitecture::X64,
                    support_tier: ReleaseSupportTier::Stable,
                    runtime_verification: RuntimeVerificationState::NativeVerified,
                    minimum_windows_build: 26_100,
                    evidence_refs: vec!["x64-native-lifecycle".to_owned()],
                    limitations: vec![],
                },
                ReleaseCompatibilityTarget {
                    architecture: ReleaseArchitecture::Arm64,
                    support_tier: ReleaseSupportTier::Preview,
                    runtime_verification: RuntimeVerificationState::NativeUnverified,
                    minimum_windows_build: 26_100,
                    evidence_refs: vec!["arm64-cross-build-simulation".to_owned()],
                    limitations: vec!["native_unverified".to_owned()],
                },
            ],
            validation_refs: vec!["x64-native".to_owned(), "arm64-simulation".to_owned()],
            rollback_plan_ref: "rollback-plan".to_owned(),
            rollback_artifact_ref: Some("previous-release".to_owned()),
            user_data_policy: "preserve".to_owned(),
            remaining_risks: vec![],
            external_gates: vec![],
        }
    }

    struct FakeCi {
        partial_at: Option<VerificationLayerKind>,
        mismatch_at: Option<VerificationLayerKind>,
    }

    impl CiAdapter for FakeCi {
        fn verify(
            &mut self,
            layer: VerificationLayerKind,
            artifact_set_digest: &Sha256Hash,
        ) -> VerificationObservation {
            VerificationObservation {
                completeness: if self.partial_at == Some(layer) {
                    EvidenceCompleteness::Partial
                } else {
                    EvidenceCompleteness::Complete
                },
                artifact_set_digest: Some(if self.mismatch_at == Some(layer) {
                    Sha256Hash::digest(b"wrong-candidate")
                } else {
                    artifact_set_digest.clone()
                }),
                validation_plan_ref: ValidationPlanId::new(),
                validation_run_ref: Some(ValidationRunId::new()),
                gate_ref: Some(GateId::new()),
                limitations: vec![],
            }
        }
    }

    struct FakeSigner;

    impl SignerAdapter for FakeSigner {
        fn sign(
            &mut self,
            artifact: &ReleaseArtifactV2,
            unsigned_bytes: &[u8],
        ) -> Result<SignedBytes, ReleaseError> {
            let mut bytes = unsigned_bytes.to_vec();
            bytes.extend_from_slice(b"-authenticode");
            Ok(SignedBytes {
                bytes,
                signature_ref: format!("signature:old={}", artifact.sha256),
            })
        }
    }

    struct FakePublisher {
        publish_result: PublishObservation,
        reconcile_result: PublishObservation,
        publishes: usize,
        reconciles: usize,
    }

    impl PublisherAdapter for FakePublisher {
        fn publish(&mut self, _manifest: &ReleaseManifestV2) -> PublishObservation {
            self.publishes += 1;
            self.publish_result.clone()
        }

        fn reconcile(&mut self, _manifest: &ReleaseManifestV2) -> PublishObservation {
            self.reconciles += 1;
            self.reconcile_result.clone()
        }
    }

    fn ready_manifest() -> ReleaseManifestV2 {
        let candidate = seal_candidate(input(SupplyChainState::Complete), &artifacts()).unwrap();
        let mut ci = FakeCi {
            partial_at: None,
            mismatch_at: None,
        };
        promote_ready(run_ci_layers(candidate, &mut ci).unwrap()).unwrap()
    }

    #[test]
    fn build_once_store_rejects_overwrite_and_verifies_exact_bytes() {
        let bytes = artifacts();
        let manifest = seal_candidate(input(SupplyChainState::Complete), &bytes).unwrap();
        let mut store = BuildOnceStore::default();
        assert!(store.insert(&manifest).unwrap());
        assert!(!store.insert(&manifest).unwrap());
        let mut conflicting = manifest.clone();
        conflicting.artifact_set_digest = Some(Sha256Hash::digest(b"different"));
        assert_eq!(store.insert(&conflicting), Err(ReleaseError::Conflict));
        let mut changed = bytes;
        changed[0].bytes.push(b'!');
        assert_eq!(
            verify_artifact_bytes(&manifest, &changed),
            Err(ReleaseError::Conflict)
        );
    }

    #[test]
    fn partial_or_digest_mismatched_ci_never_becomes_ready() {
        for mut ci in [
            FakeCi {
                partial_at: Some(VerificationLayerKind::Full),
                mismatch_at: None,
            },
            FakeCi {
                partial_at: None,
                mismatch_at: Some(VerificationLayerKind::Release),
            },
        ] {
            let manifest = seal_candidate(input(SupplyChainState::Complete), &artifacts()).unwrap();
            let verified = run_ci_layers(manifest, &mut ci).unwrap();
            assert_eq!(verified.status, ReleaseStatus::Blocked);
            assert_eq!(
                promote_ready(verified).unwrap().status,
                ReleaseStatus::Blocked
            );
        }
    }

    #[test]
    fn signing_changes_candidate_and_clears_unsigned_verification() {
        let unsigned_bytes = artifacts();
        let mut candidate_input = input(SupplyChainState::Complete);
        let signing = candidate_input
            .supply_chain_applicability
            .iter_mut()
            .find(|decision| decision.kind == SupplyChainKind::Signing)
            .unwrap();
        signing.state = SupplyChainState::RequiredIncomplete;
        signing.evidence_ref = None;
        let unsigned = seal_candidate(candidate_input, &unsigned_bytes).unwrap();
        let mut ci = FakeCi {
            partial_at: None,
            mismatch_at: None,
        };
        let unsigned = run_ci_layers(unsigned, &mut ci).unwrap();
        assert_eq!(unsigned.verification_layers.len(), 4);
        let (signed, signed_bytes) =
            sign_candidate(&unsigned, &unsigned_bytes, &mut FakeSigner).unwrap();
        assert_ne!(signed.release_manifest_id, unsigned.release_manifest_id);
        assert_ne!(signed.artifact_set_digest, unsigned.artifact_set_digest);
        assert!(signed.verification_layers.is_empty());
        assert!(signed.release_gate_refs.is_empty());
        verify_artifact_bytes(&signed, &signed_bytes).unwrap();
        let signed = run_ci_layers(signed, &mut ci).unwrap();
        assert_eq!(promote_ready(signed).unwrap().status, ReleaseStatus::Ready);
    }

    #[test]
    fn publish_timeout_reconciles_once_and_keeps_unknown() {
        let ready = ready_manifest();
        let digest = ready.artifact_set_digest.clone().unwrap();
        let approved = approve_publish(
            ready,
            ApprovalId::new(),
            &digest,
            "github:jaeminsongdev/star-control:releases",
        )
        .unwrap();
        let mut publisher = FakePublisher {
            publish_result: PublishObservation::Timeout,
            reconcile_result: PublishObservation::Timeout,
            publishes: 0,
            reconciles: 0,
        };
        let result = publish_with_reconcile(approved, &mut publisher).unwrap();
        assert_eq!(result.status, ReleaseStatus::PublishOutcomeUnknown);
        assert_eq!((publisher.publishes, publisher.reconciles), (1, 1));
    }

    #[test]
    fn partial_publish_and_remote_digest_mismatch_require_rollback() {
        for publish_result in [
            PublishObservation::Partial {
                receipt_ref: "partial-upload".to_owned(),
            },
            PublishObservation::Verified {
                artifact_set_digest: Sha256Hash::digest(b"wrong-remote"),
                snapshot_ref: "remote-snapshot".to_owned(),
            },
        ] {
            let ready = ready_manifest();
            let digest = ready.artifact_set_digest.clone().unwrap();
            let approved = approve_publish(
                ready,
                ApprovalId::new(),
                &digest,
                "github:jaeminsongdev/star-control:releases",
            )
            .unwrap();
            let mut publisher = FakePublisher {
                publish_result,
                reconcile_result: PublishObservation::Failed,
                publishes: 0,
                reconciles: 0,
            };
            let result = publish_with_reconcile(approved, &mut publisher).unwrap();
            assert_eq!(result.status, ReleaseStatus::RollbackRequired);
            assert_eq!((publisher.publishes, publisher.reconciles), (1, 0));
        }
    }
}
