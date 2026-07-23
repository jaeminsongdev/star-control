use std::{collections::BTreeSet, path::Path};

use star_contracts::{
    ProjectId, Sha256Hash,
    release_v2::{
        RELEASE_ASSET_BINDING_V1_SCHEMA_ID, ReleaseArtifactV2, ReleaseAssetBindingV1,
        ReleaseAssetSourceV1, ReleaseManifestV2,
    },
};
use star_domain::versioned_fingerprint;

use crate::ReleaseError;

pub fn seal_release_asset_binding(
    manifest: &ReleaseManifestV2,
    project_id: ProjectId,
    mut assets: Vec<ReleaseAssetSourceV1>,
    target_commitish: String,
    notes_relative_path: String,
) -> Result<ReleaseAssetBindingV1, ReleaseError> {
    let artifact_set_digest = manifest
        .artifact_set_digest
        .clone()
        .ok_or(ReleaseError::Invalid)?;
    if assets.is_empty()
        || assets.len() != manifest.artifacts.len()
        || assets.len() > 1_024
        || target_commitish.len() != 40 && target_commitish.len() != 64
        || !target_commitish
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || !safe_relative_path(&notes_relative_path)
    {
        return Err(ReleaseError::Invalid);
    }
    assets.sort();
    let mut remote_names = BTreeSet::new();
    let mut relative_paths = BTreeSet::new();
    for asset in &assets {
        if !safe_name(&asset.logical_name)
            || !safe_name(&asset.remote_name)
            || !safe_relative_path(&asset.relative_path)
            || asset.size == 0
            || !remote_names.insert(asset.remote_name.clone())
            || !relative_paths.insert(asset.relative_path.clone())
        {
            return Err(ReleaseError::Invalid);
        }
    }
    let mut expected = manifest.artifacts.clone();
    expected.sort();
    let observed = assets
        .iter()
        .map(|asset| ReleaseArtifactV2 {
            logical_name: asset.logical_name.clone(),
            role: asset.role.clone(),
            architecture: asset.architecture,
            size: asset.size,
            media_type: asset.media_type.clone(),
            sha256: asset.sha256.clone(),
        })
        .collect::<Vec<_>>();
    if expected != observed {
        return Err(ReleaseError::Conflict);
    }
    let mut binding = ReleaseAssetBindingV1 {
        schema_id: RELEASE_ASSET_BINDING_V1_SCHEMA_ID.to_owned(),
        schema_version: 1,
        release_manifest_id: manifest.release_manifest_id.clone(),
        project_id,
        artifact_set_digest,
        assets,
        repository: "jaeminsongdev/star-control".to_owned(),
        tag: format!("v{}", manifest.version),
        target_commitish,
        title: format!("Star-Control v{}", manifest.version),
        notes_relative_path,
        prerelease: false,
        binding_fingerprint: Sha256Hash::digest(b"pending"),
    };
    binding.binding_fingerprint = versioned_fingerprint(
        RELEASE_ASSET_BINDING_V1_SCHEMA_ID,
        1,
        &serde_json::json!({
            "release_manifest_id":binding.release_manifest_id,
            "project_id":binding.project_id,
            "artifact_set_digest":binding.artifact_set_digest,
            "assets":binding.assets,
            "repository":binding.repository,
            "tag":binding.tag,
            "target_commitish":binding.target_commitish,
            "title":binding.title,
            "notes_relative_path":binding.notes_relative_path,
            "prerelease":binding.prerelease,
        }),
    )
    .map_err(|_| ReleaseError::Fingerprint)?;
    Ok(binding)
}

pub fn verify_release_asset_binding(
    manifest: &ReleaseManifestV2,
    binding: &ReleaseAssetBindingV1,
) -> Result<(), ReleaseError> {
    let sealed = seal_release_asset_binding(
        manifest,
        binding.project_id.clone(),
        binding.assets.clone(),
        binding.target_commitish.clone(),
        binding.notes_relative_path.clone(),
    )?;
    if &sealed == binding {
        Ok(())
    } else {
        Err(ReleaseError::Conflict)
    }
}

fn safe_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && !matches!(value, "." | "..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && !matches!(byte, b'/' | b'\\' | b':' | 0))
}

fn safe_relative_path(value: &str) -> bool {
    if value.is_empty()
        || value.len() > 1_024
        || value.contains('\\')
        || value.contains('\0')
        || Path::new(value).is_absolute()
    {
        return false;
    }
    let mut count = 0_usize;
    for component in value.split('/') {
        if component.is_empty() || matches!(component, "." | "..") {
            return false;
        }
        count += 1;
    }
    count > 0
}

#[cfg(test)]
mod tests {
    use star_contracts::{
        ReleaseManifestId, ScopeRevisionId, TaskSpecId,
        release_v2::{ReleaseArchitecture, ReleaseArtifactV2, ReleaseStatus},
    };

    use super::*;

    fn manifest() -> ReleaseManifestV2 {
        ReleaseManifestV2 {
            schema_id: star_contracts::release_v2::RELEASE_MANIFEST_V2_SCHEMA_ID.to_owned(),
            schema_version: 2,
            release_manifest_id: ReleaseManifestId::new(),
            revision: 1,
            supersedes: None,
            product_id: "star-control".to_owned(),
            version: "0.1.0".to_owned(),
            channel: "github_releases".to_owned(),
            task_spec_ref: TaskSpecId::new(),
            scope_revision_ref: ScopeRevisionId::new(),
            source_revisions: vec![],
            identity_binding: star_contracts::release_v2::ReleaseIdentityBinding {
                config_fingerprint: Sha256Hash::digest(b"config"),
                catalog_fingerprint: Sha256Hash::digest(b"catalog"),
                tool_descriptor_fingerprints: vec![],
                profile_fingerprint: Sha256Hash::digest(b"profile"),
                environment_fingerprints: vec![],
            },
            verification_layers: vec![],
            build_invocation_refs: vec![],
            artifacts: vec![ReleaseArtifactV2 {
                logical_name: "installer-x64".to_owned(),
                role: "installer".to_owned(),
                architecture: ReleaseArchitecture::X64,
                size: 3,
                media_type: "application/vnd.microsoft.portable-executable".to_owned(),
                sha256: Sha256Hash::digest(b"abc"),
            }],
            artifact_set_digest: Some(Sha256Hash::digest(b"set")),
            included_files_manifest_ref: None,
            metadata_refs: vec![],
            supply_chain_applicability: vec![],
            sbom_ref: None,
            provenance_ref: None,
            signature_refs: vec![],
            compatibility: vec![],
            validation_refs: vec![],
            release_gate_refs: vec![],
            remote_actions: vec![],
            approval_request_refs: vec![],
            remote_operation_refs: vec![],
            before_remote_snapshot_refs: vec![],
            after_remote_snapshot_refs: vec![],
            rollback_plan_ref: "rollback".to_owned(),
            rollback_artifact_ref: None,
            user_data_policy: "preserve".to_owned(),
            remaining_risks: vec![],
            external_gates: vec![],
            status: ReleaseStatus::Candidate,
            manifest_fingerprint: Sha256Hash::digest(b"manifest"),
        }
    }

    fn asset() -> ReleaseAssetSourceV1 {
        ReleaseAssetSourceV1 {
            logical_name: "installer-x64".to_owned(),
            remote_name: "star-control-windows-x64-0.1.0-setup.exe".to_owned(),
            role: "installer".to_owned(),
            architecture: ReleaseArchitecture::X64,
            media_type: "application/vnd.microsoft.portable-executable".to_owned(),
            relative_path: "dist/star-control-windows-x64-0.1.0-setup.exe".to_owned(),
            size: 3,
            sha256: Sha256Hash::digest(b"abc"),
        }
    }

    #[test]
    fn binding_is_exact_and_rejects_path_or_digest_drift() {
        let manifest = manifest();
        let binding = seal_release_asset_binding(
            &manifest,
            ProjectId::new(),
            vec![asset()],
            "a".repeat(40),
            "CHANGELOG.md".to_owned(),
        )
        .unwrap();
        verify_release_asset_binding(&manifest, &binding).unwrap();

        let mut drift = binding.clone();
        drift.assets[0].sha256 = Sha256Hash::digest(b"different");
        assert_eq!(
            verify_release_asset_binding(&manifest, &drift),
            Err(ReleaseError::Conflict)
        );

        let mut unsafe_asset = asset();
        unsafe_asset.relative_path = "../outside".to_owned();
        assert_eq!(
            seal_release_asset_binding(
                &manifest,
                ProjectId::new(),
                vec![unsafe_asset],
                "a".repeat(40),
                "CHANGELOG.md".to_owned(),
            ),
            Err(ReleaseError::Invalid)
        );
    }
}
