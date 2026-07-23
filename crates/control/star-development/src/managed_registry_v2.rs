use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use star_contracts::{
    CheckoutId, ManagedRegistrySnapshotId, ProjectId, RegistryConsistencyRecordId, Sha256Hash,
    evidence::DocumentRef,
    ids::{CodeIndexSnapshotId, ProjectRevisionId, WorkspaceSnapshotId},
    index::SourceEntry,
    managed_registry::{
        AliasRecord, BindingObservation, ConsumerMigrationPlan, ConsumerMigrationState,
        ConsumerRewrite, EvidenceCompleteness, MANAGED_DECLARATION_CHANGE_INTENT_SCHEMA_ID,
        MANAGED_REGISTRY_FRAGMENT_SCHEMA_ID, MANAGED_REGISTRY_MANIFEST_SCHEMA_ID,
        MANAGED_REGISTRY_SNAPSHOT_SCHEMA_ID, ManagedCandidate, ManagedConsumer,
        ManagedConsumerRequirement, ManagedConsumerState, ManagedDeclaration,
        ManagedDeclarationChangeIntent, ManagedDeclarationChangeKind,
        ManagedDeclarationClassification, ManagedDeclarationId, ManagedDeclarationKind,
        ManagedDeclarationRef, ManagedDeclarationSource, ManagedDesiredFields, ManagedLifecycle,
        ManagedRegistryFragment, ManagedRegistryManifest, ManagedRegistrySnapshot,
        REGISTRY_CONSISTENCY_RECORD_SCHEMA_ID, RegistryConsistencyRecord,
        RegistryConsistencyStatus, RegistryFreshness, RegistryResolutionState, RegistrySourceRef,
        RegistryTombstone,
    },
    management::ProjectPathRef,
};
use star_domain::versioned_fingerprint;

use crate::DevelopmentError;

const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_FRAGMENT_BYTES: u64 = 8 * 1024 * 1024;
const REQUIRED_REGISTRY_CHECKS: [&str; 4] = [
    "managed_registry_contract",
    "consumer_compatibility",
    "generated_consistency",
    "docs_contract_drift",
];

#[derive(Clone, Debug)]
pub struct RegistryResolutionInput {
    pub owner_project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub project_revision_id: ProjectRevisionId,
    pub workspace_snapshot_id: WorkspaceSnapshotId,
    pub code_index_snapshot_id: Option<CodeIndexSnapshotId>,
    pub index_current: bool,
    pub coverage_complete: bool,
    pub consumers: Vec<ManagedConsumer>,
    pub candidates: Vec<ManagedCandidate>,
    pub local_constants: Vec<ManagedCandidate>,
}

#[derive(Clone, Debug)]
pub struct RegistryResolution {
    pub root_manifest: ManagedRegistryManifest,
    pub fragments: Vec<ManagedRegistryFragment>,
    pub snapshot: ManagedRegistrySnapshot,
    pub consistency_records: Vec<RegistryConsistencyRecord>,
}

#[derive(Clone, Debug, Default)]
pub struct CandidateInventory {
    pub candidates: Vec<ManagedCandidate>,
    pub local_constants: Vec<ManagedCandidate>,
}

#[derive(Clone, Debug)]
pub struct ConsumerProjectInput {
    pub project_id: ProjectId,
    pub project_root: PathBuf,
    pub source_entries: Vec<SourceEntry>,
    pub index_current: bool,
    pub coverage_complete: bool,
}

#[derive(Clone, Debug)]
pub struct ConsumerDiscovery {
    pub consumers: Vec<ManagedConsumer>,
    pub coverage_complete: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistrySourceRewrite {
    pub path: ProjectPathRef,
    pub before_bytes: Vec<u8>,
    pub after_bytes: Vec<u8>,
    pub before_sha256: Sha256Hash,
    pub after_sha256: Sha256Hash,
}

#[derive(Clone, Debug)]
struct LoadedSource {
    path: ProjectPathRef,
    bytes: Vec<u8>,
}

pub fn load_git_registry_from_project(
    project_root: &Path,
    root_manifest_path: &ProjectPathRef,
    mut input: RegistryResolutionInput,
) -> Result<RegistryResolution, DevelopmentError> {
    let canonical_root = fs::canonicalize(project_root).map_err(|_| DevelopmentError::Adapter)?;
    if !canonical_root.is_dir() {
        return Err(DevelopmentError::Invalid);
    }
    let git_root = git_text(&canonical_root, &["rev-parse", "--show-toplevel"])?;
    let canonical_git_root = fs::canonicalize(git_root).map_err(|_| DevelopmentError::Adapter)?;
    if canonical_root != canonical_git_root {
        return Err(DevelopmentError::Blocked);
    }
    let git_revision = git_text(&canonical_root, &["rev-parse", "HEAD"])?;
    if git_revision.len() != 40 || !git_revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(DevelopmentError::Invalid);
    }

    require_git_tracked(&canonical_root, root_manifest_path)?;
    let root_source = read_bounded_source(&canonical_root, root_manifest_path, MAX_MANIFEST_BYTES)?;
    let root_text =
        std::str::from_utf8(&root_source.bytes).map_err(|_| DevelopmentError::Invalid)?;
    let mut manifest: ManagedRegistryManifest =
        toml::from_str(root_text).map_err(|_| DevelopmentError::Invalid)?;
    validate_root_manifest(&manifest, &input.owner_project_id)?;
    canonicalize_root(&mut manifest)?;

    let mut fragments = Vec::with_capacity(manifest.declaration_files.len());
    let mut fragment_sources = Vec::with_capacity(manifest.declaration_files.len());
    for path in &manifest.declaration_files {
        require_git_tracked(&canonical_root, path)?;
        let source = read_bounded_source(&canonical_root, path, MAX_FRAGMENT_BYTES)?;
        let text = std::str::from_utf8(&source.bytes).map_err(|_| DevelopmentError::Invalid)?;
        let mut fragment: ManagedRegistryFragment =
            toml::from_str(text).map_err(|_| DevelopmentError::Invalid)?;
        if fragment.schema_id != MANAGED_REGISTRY_FRAGMENT_SCHEMA_ID
            || fragment.schema_version != 1
            || fragment.registry_id != manifest.registry_id
            || !valid_namespace(&fragment.namespace)
        {
            return Err(DevelopmentError::Invalid);
        }
        fragment.declarations.sort_by(|left, right| {
            left.managed_declaration_id
                .cmp(&right.managed_declaration_id)
        });
        fragments.push(fragment);
        fragment_sources.push(source);
    }

    let claims = resolve_namespace_claims(&manifest)?;
    let mut declarations = Vec::new();
    let mut ids = BTreeSet::new();
    let mut semantic_keys = BTreeSet::new();
    let mut owned_values = BTreeMap::<String, ManagedDeclarationId>::new();
    for (fragment, source) in fragments.iter().zip(&fragment_sources) {
        let claim = claims
            .get(fragment.namespace.as_str())
            .ok_or(DevelopmentError::Conflict)?;
        for declaration in &fragment.declarations {
            if declaration.namespace != fragment.namespace
                || declaration.owner.project_id != claim.owner_project_id
                || !claim.allowed_kinds.contains(&declaration.kind)
                || !ids.insert(declaration.managed_declaration_id.clone())
                || !semantic_keys.insert((
                    declaration.namespace.clone(),
                    declaration.semantic_key.clone(),
                ))
            {
                return Err(DevelopmentError::Conflict);
            }
            validate_declaration_source(declaration, &manifest.registry_version)?;
            for value in declaration
                .primary_value
                .iter()
                .chain(declaration.aliases.iter().map(|alias| &alias.value))
            {
                let key = format!(
                    "{}:{:?}:{}:{}",
                    declaration.uniqueness_scope, declaration.kind, declaration.namespace, value
                );
                if owned_values
                    .insert(key, declaration.managed_declaration_id.clone())
                    .is_some()
                {
                    return Err(DevelopmentError::Conflict);
                }
            }
            let definition_fingerprint = declaration_fingerprint(declaration)?;
            declarations.push(ManagedDeclaration {
                managed_declaration_id: declaration.managed_declaration_id.clone(),
                item_version: declaration.item_version.clone(),
                namespace: declaration.namespace.clone(),
                semantic_key: declaration.semantic_key.clone(),
                kind: declaration.kind,
                owner: declaration.owner.clone(),
                value_type: declaration.value_type.clone(),
                value_role: declaration.value_role,
                primary_value: declaration.primary_value.clone(),
                description: declaration.description.clone(),
                status: declaration.status,
                lifecycle: declaration.lifecycle.clone(),
                aliases: declaration.aliases.clone(),
                binding_specs: declaration.binding_specs.clone(),
                consumer_contracts: declaration.consumer_contracts.clone(),
                uniqueness_scope: declaration.uniqueness_scope.clone(),
                source_path: source.path.clone(),
                source_sha256: Sha256Hash::digest(&source.bytes),
                definition_fingerprint,
            });
        }
    }
    declarations.sort_by(|left, right| {
        left.managed_declaration_id
            .cmp(&right.managed_declaration_id)
    });

    input.consumers.sort_by(|left, right| {
        (&left.declaration_id, &left.project_id, &left.path).cmp(&(
            &right.declaration_id,
            &right.project_id,
            &right.path,
        ))
    });
    if input.consumers.windows(2).any(|pair| {
        pair[0].declaration_id == pair[1].declaration_id
            && pair[0].project_id == pair[1].project_id
            && pair[0].path == pair[1].path
    }) {
        return Err(DevelopmentError::Conflict);
    }
    validate_consumers(&declarations, &input.consumers)?;
    canonicalize_candidates(&mut input.candidates)?;
    canonicalize_candidates(&mut input.local_constants)?;
    let classification_seeds = apply_source_candidate_classifications(
        &manifest,
        &mut input.candidates,
        &mut input.local_constants,
    )?;

    let (binding_observations, mut consistency_seeds) =
        observe_bindings(&canonical_root, &declarations)?;
    consistency_seeds.extend(classification_seeds);
    consistency_seeds.extend(consumer_consistency(&declarations, &input.consumers));

    let tombstones = declarations
        .iter()
        .filter(|declaration| {
            matches!(
                declaration.status,
                ManagedLifecycle::Reserved | ManagedLifecycle::Removed
            )
        })
        .map(|declaration| {
            let removed_in_registry_version = declaration
                .lifecycle
                .removed_in_registry_version
                .clone()
                .unwrap_or_else(|| manifest.registry_version.clone());
            let tombstone_fingerprint = fingerprint(
                "star.registry-tombstone",
                &serde_json::json!({
                    "declaration_id":declaration.managed_declaration_id,
                    "reserved_value":declaration.primary_value,
                    "removed_in_registry_version":removed_in_registry_version,
                }),
            )?;
            Ok(RegistryTombstone {
                declaration_id: declaration.managed_declaration_id.clone(),
                reserved_value: declaration.primary_value.clone(),
                removed_in_registry_version,
                tombstone_fingerprint,
            })
        })
        .collect::<Result<Vec<_>, DevelopmentError>>()?;
    let tombstone_set_fingerprint = fingerprint("star.registry-tombstone-set", &tombstones)?;

    let manifest_sha256 = manifest_set_fingerprint(&manifest, &root_source, &fragment_sources)?;
    let mut manifest_source_refs = vec![RegistrySourceRef {
        path: root_source.path,
        source_sha256: Sha256Hash::digest(&root_source.bytes),
    }];
    manifest_source_refs.extend(fragment_sources.iter().map(|source| RegistrySourceRef {
        path: source.path.clone(),
        source_sha256: Sha256Hash::digest(&source.bytes),
    }));
    manifest_source_refs.sort_by(|left, right| left.path.cmp(&right.path));

    let consistency_current = consistency_seeds
        .iter()
        .all(|seed| seed.status == RegistryConsistencyStatus::Current);
    let completeness = if input.index_current && input.coverage_complete && consistency_current {
        EvidenceCompleteness::Complete
    } else {
        EvidenceCompleteness::Partial
    };
    let mut limitations = Vec::new();
    if !input.index_current {
        limitations.push("REGISTRY_INDEX_NOT_CURRENT".to_owned());
    }
    if !input.coverage_complete {
        limitations.push("REGISTRY_COVERAGE_PARTIAL".to_owned());
    }
    for seed in &consistency_seeds {
        if seed.status != RegistryConsistencyStatus::Current {
            limitations.push(format!("REGISTRY_DRIFT:{:?}:{}", seed.status, seed.subject));
        }
    }
    limitations.sort();
    limitations.dedup();
    let freshness = if !input.index_current {
        RegistryFreshness::StaleSource
    } else if !input.coverage_complete {
        RegistryFreshness::Partial
    } else {
        RegistryFreshness::Current
    };
    let mut snapshot = ManagedRegistrySnapshot {
        schema_id: MANAGED_REGISTRY_SNAPSHOT_SCHEMA_ID.to_owned(),
        schema_version: 2,
        managed_registry_snapshot_id: ManagedRegistrySnapshotId::new(),
        registry_id: manifest.registry_id.clone(),
        registry_version: manifest.registry_version.clone(),
        owner_project_id: input.owner_project_id,
        checkout_id: input.checkout_id,
        project_revision_id: input.project_revision_id,
        workspace_snapshot_id: input.workspace_snapshot_id,
        git_revision,
        manifest_sha256,
        manifest_source_refs,
        namespace_claims: manifest.namespace_claims.clone(),
        declarations,
        binding_observations,
        consumers: input.consumers,
        candidates: input.candidates,
        local_constants: input.local_constants,
        code_index_snapshot_id: input.code_index_snapshot_id,
        tombstones,
        tombstone_set_fingerprint,
        resolution_state: RegistryResolutionState::Valid,
        freshness,
        completeness,
        limitations,
        diagnostic_refs: vec![],
        content_fingerprint: Sha256Hash::digest(b"unsealed-registry-snapshot"),
    };
    snapshot = snapshot.seal().map_err(|_| DevelopmentError::Conflict)?;
    let snapshot_ref = registry_snapshot_ref(&snapshot)?;
    let mut consistency_records = consistency_seeds
        .into_iter()
        .map(|seed| seal_consistency_record(&snapshot_ref, seed))
        .collect::<Result<Vec<_>, _>>()?;
    consistency_records.sort_by(|left, right| {
        left.registry_consistency_record_id
            .cmp(&right.registry_consistency_record_id)
    });

    Ok(RegistryResolution {
        root_manifest: manifest,
        fragments,
        snapshot,
        consistency_records,
    })
}

#[derive(Clone, Debug)]
struct ConsistencySeed {
    declaration_ref: Option<ManagedDeclarationRef>,
    status: RegistryConsistencyStatus,
    subject: String,
    expected_value: Option<String>,
    observed_value: Option<String>,
    remediation: String,
}

pub fn build_change_intent(
    snapshot: &ManagedRegistrySnapshot,
    declaration_id: Option<&ManagedDeclarationId>,
    change_kind: ManagedDeclarationChangeKind,
    desired_fields: ManagedDesiredFields,
    reason: String,
    mut requested_consumer_scope: Vec<ProjectId>,
) -> Result<ManagedDeclarationChangeIntent, DevelopmentError> {
    if snapshot.schema_id != MANAGED_REGISTRY_SNAPSHOT_SCHEMA_ID
        || snapshot.completeness != EvidenceCompleteness::Complete
        || snapshot.freshness != RegistryFreshness::Current
        || snapshot.resolution_state != RegistryResolutionState::Valid
        || reason.trim().is_empty()
        || reason.len() > 2_048
        || !change_kind_matches_fields(change_kind, &desired_fields)
    {
        return Err(DevelopmentError::Unverified);
    }
    requested_consumer_scope.sort();
    requested_consumer_scope.dedup();
    let declaration_ref = match declaration_id {
        Some(id) => {
            let declaration = snapshot
                .declarations
                .iter()
                .find(|declaration| &declaration.managed_declaration_id == id)
                .ok_or(DevelopmentError::Invalid)?;
            Some(ManagedDeclarationRef {
                managed_declaration_id: declaration.managed_declaration_id.clone(),
                item_version: declaration.item_version.clone(),
                definition_fingerprint: declaration.definition_fingerprint.clone(),
            })
        }
        None if matches!(
            change_kind,
            ManagedDeclarationChangeKind::Create | ManagedDeclarationChangeKind::ClassifyCandidate
        ) =>
        {
            None
        }
        None => return Err(DevelopmentError::Invalid),
    };
    validate_desired_transition(snapshot, declaration_ref.as_ref(), &desired_fields)?;
    let mut intent = ManagedDeclarationChangeIntent {
        schema_id: MANAGED_DECLARATION_CHANGE_INTENT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        registry_snapshot_ref: registry_snapshot_ref(snapshot)?,
        declaration_ref,
        change_kind,
        desired_fields,
        reason,
        requested_consumer_scope,
        expected_manifest_fingerprint: snapshot.manifest_sha256.clone(),
        intent_fingerprint: Sha256Hash::digest(b"unsealed-registry-intent"),
    };
    intent = intent.seal().map_err(|_| DevelopmentError::Conflict)?;
    Ok(intent)
}

pub fn prepare_registry_change_rewrite(
    project_root: &Path,
    snapshot: &ManagedRegistrySnapshot,
    intent: &ManagedDeclarationChangeIntent,
) -> Result<Vec<RegistrySourceRewrite>, DevelopmentError> {
    if snapshot.clone().seal().as_ref() != Ok(snapshot)
        || intent.clone().seal().as_ref() != Ok(intent)
        || intent.registry_snapshot_ref != registry_snapshot_ref(snapshot)?
        || intent.expected_manifest_fingerprint != snapshot.manifest_sha256
        || snapshot.freshness != RegistryFreshness::Current
        || snapshot.completeness != EvidenceCompleteness::Complete
        || snapshot.resolution_state != RegistryResolutionState::Valid
    {
        return Err(DevelopmentError::Unverified);
    }
    let canonical_root = fs::canonicalize(project_root).map_err(|_| DevelopmentError::Adapter)?;
    let source_path = match &intent.desired_fields {
        ManagedDesiredFields::ClassifyCandidate { candidate_id, .. } => {
            if !snapshot
                .candidates
                .iter()
                .chain(&snapshot.local_constants)
                .any(|candidate| &candidate.candidate_id == candidate_id)
            {
                return Err(DevelopmentError::Invalid);
            }
            snapshot
                .manifest_source_refs
                .iter()
                .find_map(|source| {
                    let loaded =
                        read_bounded_source(&canonical_root, &source.path, MAX_MANIFEST_BYTES)
                            .ok()?;
                    let text = std::str::from_utf8(&loaded.bytes).ok()?;
                    toml::from_str::<ManagedRegistryManifest>(text)
                        .ok()
                        .map(|_| source.path.clone())
                })
                .ok_or(DevelopmentError::Invalid)?
        }
        ManagedDesiredFields::Create { declaration } => declaration.source_path.clone(),
        _ => intent
            .declaration_ref
            .as_ref()
            .and_then(|reference| {
                snapshot.declarations.iter().find(|declaration| {
                    declaration.managed_declaration_id == reference.managed_declaration_id
                })
            })
            .map(|declaration| declaration.source_path.clone())
            .ok_or(DevelopmentError::Invalid)?,
    };
    require_git_tracked(&canonical_root, &source_path)?;
    let source_ref = snapshot
        .manifest_source_refs
        .iter()
        .find(|source| source.path == source_path)
        .ok_or(DevelopmentError::Conflict)?;
    let source = read_bounded_source(&canonical_root, &source_path, MAX_FRAGMENT_BYTES)?;
    if Sha256Hash::digest(&source.bytes) != source_ref.source_sha256 {
        return Err(DevelopmentError::Unverified);
    }
    let text = std::str::from_utf8(&source.bytes).map_err(|_| DevelopmentError::Invalid)?;
    let after_bytes = if matches!(
        intent.desired_fields,
        ManagedDesiredFields::ClassifyCandidate { .. }
    ) {
        let mut manifest: ManagedRegistryManifest =
            toml::from_str(text).map_err(|_| DevelopmentError::Invalid)?;
        if !apply_candidate_classification(&mut manifest, intent)? {
            return Err(DevelopmentError::Unverified);
        }
        validate_root_manifest(&manifest, &snapshot.owner_project_id)?;
        let rendered = render_toml(&manifest)?;
        let mut replay: ManagedRegistryManifest =
            toml::from_str(&rendered).map_err(|_| DevelopmentError::Invalid)?;
        if apply_candidate_classification(&mut replay, intent)? {
            return Err(DevelopmentError::Conflict);
        }
        rendered.into_bytes()
    } else {
        let mut fragment: ManagedRegistryFragment =
            toml::from_str(text).map_err(|_| DevelopmentError::Invalid)?;
        if !apply_declaration_intent(&mut fragment, intent)? {
            return Err(DevelopmentError::Unverified);
        }
        fragment.declarations.sort_by(|left, right| {
            left.managed_declaration_id
                .cmp(&right.managed_declaration_id)
        });
        for declaration in &fragment.declarations {
            validate_declaration_source(declaration, &snapshot.registry_version)?;
        }
        let rendered = render_toml(&fragment)?;
        let mut replay: ManagedRegistryFragment =
            toml::from_str(&rendered).map_err(|_| DevelopmentError::Invalid)?;
        if apply_declaration_intent(&mut replay, intent)? {
            return Err(DevelopmentError::Conflict);
        }
        rendered.into_bytes()
    };
    let after_sha256 = Sha256Hash::digest(&after_bytes);
    if after_sha256 == source_ref.source_sha256 {
        return Err(DevelopmentError::Unverified);
    }
    Ok(vec![RegistrySourceRewrite {
        path: source_path,
        before_sha256: source_ref.source_sha256.clone(),
        after_sha256,
        before_bytes: source.bytes,
        after_bytes,
    }])
}

fn apply_declaration_intent(
    fragment: &mut ManagedRegistryFragment,
    intent: &ManagedDeclarationChangeIntent,
) -> Result<bool, DevelopmentError> {
    if let ManagedDesiredFields::Create { declaration } = &intent.desired_fields {
        let source = declaration_source(declaration);
        match fragment
            .declarations
            .iter()
            .find(|candidate| candidate.managed_declaration_id == source.managed_declaration_id)
        {
            Some(existing) if existing == &source => return Ok(false),
            Some(_) => return Err(DevelopmentError::Conflict),
            None => {
                if source.namespace != fragment.namespace {
                    return Err(DevelopmentError::Conflict);
                }
                fragment.declarations.push(source);
                return Ok(true);
            }
        }
    }
    let reference = intent
        .declaration_ref
        .as_ref()
        .ok_or(DevelopmentError::Invalid)?;
    let declaration = fragment
        .declarations
        .iter_mut()
        .find(|candidate| candidate.managed_declaration_id == reference.managed_declaration_id)
        .ok_or(DevelopmentError::Invalid)?;
    let changed = match (&intent.change_kind, &intent.desired_fields) {
        (
            ManagedDeclarationChangeKind::UpdateDescription,
            ManagedDesiredFields::UpdateDescription { description },
        ) => replace_value(&mut declaration.description, description.clone()),
        (
            ManagedDeclarationChangeKind::ChangePrimaryValue,
            ManagedDesiredFields::ChangePrimaryValue {
                primary_value,
                new_item_version,
            },
        ) => {
            let changed = declaration.primary_value.as_ref() != Some(primary_value)
                || &declaration.item_version != new_item_version;
            declaration.primary_value = Some(primary_value.clone());
            declaration.item_version = new_item_version.clone();
            changed
        }
        (
            ManagedDeclarationChangeKind::Deprecate,
            ManagedDesiredFields::Deprecate {
                deprecated_in_registry_version,
                replacement_id,
            },
        ) => {
            let changed = declaration.status != ManagedLifecycle::Deprecated
                || declaration
                    .lifecycle
                    .deprecated_in_registry_version
                    .as_ref()
                    != Some(deprecated_in_registry_version)
                || declaration.lifecycle.replacement_id != *replacement_id;
            declaration.status = ManagedLifecycle::Deprecated;
            declaration.lifecycle.deprecated_in_registry_version =
                Some(deprecated_in_registry_version.clone());
            declaration.lifecycle.replacement_id = replacement_id.clone();
            changed
        }
        (ManagedDeclarationChangeKind::AddAlias, ManagedDesiredFields::AddAlias { alias }) => {
            if declaration.aliases.iter().any(|existing| existing == alias) {
                false
            } else if declaration
                .aliases
                .iter()
                .any(|existing| existing.value == alias.value)
            {
                return Err(DevelopmentError::Conflict);
            } else {
                declaration.aliases.push(alias.clone());
                declaration
                    .aliases
                    .sort_by(|left, right| left.value.cmp(&right.value));
                true
            }
        }
        (
            ManagedDeclarationChangeKind::Remove,
            ManagedDesiredFields::Remove {
                removed_in_registry_version,
            },
        ) => {
            let changed = declaration.status != ManagedLifecycle::Removed
                || declaration.lifecycle.removed_in_registry_version.as_ref()
                    != Some(removed_in_registry_version);
            declaration.status = ManagedLifecycle::Removed;
            declaration.lifecycle.removed_in_registry_version =
                Some(removed_in_registry_version.clone());
            changed
        }
        (
            ManagedDeclarationChangeKind::AddBinding,
            ManagedDesiredFields::AddBinding { binding },
        ) => {
            if declaration
                .binding_specs
                .iter()
                .any(|existing| existing == binding)
            {
                false
            } else if declaration
                .binding_specs
                .iter()
                .any(|existing| existing.binding_id == binding.binding_id)
            {
                return Err(DevelopmentError::Conflict);
            } else {
                declaration.binding_specs.push(binding.clone());
                declaration
                    .binding_specs
                    .sort_by(|left, right| left.binding_id.cmp(&right.binding_id));
                true
            }
        }
        (
            ManagedDeclarationChangeKind::ChangeConsumerFloor,
            ManagedDesiredFields::ChangeConsumerFloor {
                consumer_surface_id,
                minimum_item_version,
            },
        ) => {
            let consumer = declaration
                .consumer_contracts
                .iter_mut()
                .find(|consumer| &consumer.consumer_surface_id == consumer_surface_id)
                .ok_or(DevelopmentError::Invalid)?;
            replace_value(
                &mut consumer.minimum_item_version,
                minimum_item_version.clone(),
            )
        }
        _ => return Err(DevelopmentError::Invalid),
    };
    Ok(changed)
}

fn declaration_source(declaration: &ManagedDeclaration) -> ManagedDeclarationSource {
    ManagedDeclarationSource {
        managed_declaration_id: declaration.managed_declaration_id.clone(),
        item_version: declaration.item_version.clone(),
        namespace: declaration.namespace.clone(),
        semantic_key: declaration.semantic_key.clone(),
        kind: declaration.kind,
        owner: declaration.owner.clone(),
        value_type: declaration.value_type.clone(),
        value_role: declaration.value_role,
        primary_value: declaration.primary_value.clone(),
        description: declaration.description.clone(),
        status: declaration.status,
        lifecycle: declaration.lifecycle.clone(),
        aliases: declaration.aliases.clone(),
        binding_specs: declaration.binding_specs.clone(),
        consumer_contracts: declaration.consumer_contracts.clone(),
        uniqueness_scope: declaration.uniqueness_scope.clone(),
    }
}

fn apply_candidate_classification(
    manifest: &mut ManagedRegistryManifest,
    intent: &ManagedDeclarationChangeIntent,
) -> Result<bool, DevelopmentError> {
    let ManagedDesiredFields::ClassifyCandidate {
        candidate_id,
        classification,
    } = &intent.desired_fields
    else {
        return Err(DevelopmentError::Invalid);
    };
    let key = "star.registry.candidate_classifications".to_owned();
    let mut records = manifest
        .extensions
        .get(&key)
        .cloned()
        .map(serde_json::from_value::<Vec<serde_json::Value>>)
        .transpose()
        .map_err(|_| DevelopmentError::Invalid)?
        .unwrap_or_default();
    let desired = serde_json::json!({
        "candidate_id":candidate_id,
        "classification":classification,
        "reason":intent.reason,
    });
    if let Some(existing) = records.iter_mut().find(|record| {
        record
            .get("candidate_id")
            .and_then(serde_json::Value::as_str)
            == Some(candidate_id.as_str())
    }) {
        if *existing == desired {
            return Ok(false);
        }
        *existing = desired;
    } else {
        records.push(desired);
    }
    records.sort_by(|left, right| {
        left.get("candidate_id")
            .and_then(serde_json::Value::as_str)
            .cmp(
                &right
                    .get("candidate_id")
                    .and_then(serde_json::Value::as_str),
            )
    });
    manifest
        .extensions
        .insert(key, serde_json::Value::Array(records));
    Ok(true)
}

fn replace_value<T: PartialEq>(target: &mut T, value: T) -> bool {
    if *target == value {
        false
    } else {
        *target = value;
        true
    }
}

fn render_toml(value: &impl serde::Serialize) -> Result<String, DevelopmentError> {
    let mut rendered = toml::to_string_pretty(value).map_err(|_| DevelopmentError::Invalid)?;
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    Ok(rendered)
}

pub fn scan_git_registry_candidates(
    project_root: &Path,
    snapshot: &ManagedRegistrySnapshot,
) -> Result<CandidateInventory, DevelopmentError> {
    let canonical_root = fs::canonicalize(project_root).map_err(|_| DevelopmentError::Adapter)?;
    let git_root = git_text(&canonical_root, &["rev-parse", "--show-toplevel"])?;
    if fs::canonicalize(git_root).map_err(|_| DevelopmentError::Adapter)? != canonical_root {
        return Err(DevelopmentError::Blocked);
    }
    let output = Command::new("git")
        .current_dir(&canonical_root)
        .args(["ls-files", "-z", "--cached"])
        .output()
        .map_err(|_| DevelopmentError::Adapter)?;
    if !output.status.success() || output.stdout.len() > 8 * 1024 * 1024 {
        return Err(DevelopmentError::Adapter);
    }
    let managed_values = snapshot
        .declarations
        .iter()
        .flat_map(|declaration| {
            declaration
                .primary_value
                .iter()
                .chain(declaration.aliases.iter().map(|alias| &alias.value))
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let manifest_paths = snapshot
        .manifest_source_refs
        .iter()
        .map(|source| &source.path)
        .collect::<BTreeSet<_>>();
    let mut inventory = CandidateInventory::default();
    for raw in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|raw| !raw.is_empty())
        .take(20_000)
    {
        let relative = std::str::from_utf8(raw).map_err(|_| DevelopmentError::Invalid)?;
        let path =
            ProjectPathRef::parse(relative.to_owned()).map_err(|_| DevelopmentError::Invalid)?;
        if manifest_paths.contains(&path) || !candidate_source_path(relative) {
            continue;
        }
        let source = match read_bounded_source(&canonical_root, &path, MAX_MANIFEST_BYTES) {
            Ok(source) => source,
            Err(_) => continue,
        };
        let Some(text) = std::str::from_utf8(&source.bytes).ok() else {
            continue;
        };
        for (line_index, line) in text.lines().enumerate() {
            for value in quoted_values(line) {
                let Some(kind) = candidate_kind(value) else {
                    continue;
                };
                if managed_values.contains(value) {
                    continue;
                }
                let classification = if line.contains("const ") && !line.contains("pub ") {
                    ManagedDeclarationClassification::LocalImplementationConstant
                } else {
                    ManagedDeclarationClassification::Candidate
                };
                let candidate_fingerprint = fingerprint(
                    "star.managed-registry-candidate",
                    &serde_json::json!({
                        "path":path,
                        "line":line_index + 1,
                        "kind":kind,
                        "value":value,
                        "classification":classification,
                        "source_sha256":Sha256Hash::digest(&source.bytes),
                    }),
                )?;
                let candidate = ManagedCandidate {
                    candidate_id: format!(
                        "mrc_{}",
                        candidate_fingerprint.as_str().trim_start_matches("sha256:")
                    ),
                    classification,
                    kind,
                    observed_value: value.to_owned(),
                    path: path.clone(),
                    source_sha256: Sha256Hash::digest(&source.bytes),
                    reason_codes: vec![
                        match kind {
                            ManagedDeclarationKind::ErrorCode => "ERROR_CODE_SHAPE_OBSERVED",
                            ManagedDeclarationKind::DiagnosticId => "DIAGNOSTIC_ID_SHAPE_OBSERVED",
                            ManagedDeclarationKind::SchemaId => "SCHEMA_ID_SHAPE_OBSERVED",
                            ManagedDeclarationKind::ConfigKey => "CONFIG_KEY_SHAPE_OBSERVED",
                            _ => "CONTRACT_LITERAL_SHAPE_OBSERVED",
                        }
                        .to_owned(),
                    ],
                };
                if classification == ManagedDeclarationClassification::LocalImplementationConstant {
                    inventory.local_constants.push(candidate);
                } else {
                    inventory.candidates.push(candidate);
                }
            }
        }
    }
    canonicalize_candidates(&mut inventory.candidates)?;
    canonicalize_candidates(&mut inventory.local_constants)?;
    Ok(inventory)
}

pub fn discover_registry_consumers(
    snapshot: &ManagedRegistrySnapshot,
    projects: &[ConsumerProjectInput],
) -> Result<ConsumerDiscovery, DevelopmentError> {
    let mut projects_by_id = BTreeMap::new();
    for project in projects {
        if projects_by_id
            .insert(&project.project_id, project)
            .is_some()
        {
            return Err(DevelopmentError::Conflict);
        }
    }
    let mut coverage_complete = true;
    let mut consumers = BTreeMap::new();
    for declaration in &snapshot.declarations {
        for contract in &declaration.consumer_contracts {
            let Some(project) = projects_by_id.get(&contract.project_id) else {
                coverage_complete = false;
                continue;
            };
            coverage_complete &= project.index_current && project.coverage_complete;
            let canonical_root = match fs::canonicalize(&project.project_root) {
                Ok(root) if root.is_dir() => root,
                _ => {
                    coverage_complete = false;
                    continue;
                }
            };
            if project.source_entries.len() > 50_000 {
                coverage_complete = false;
            }
            for source_entry in project.source_entries.iter().take(50_000) {
                if source_entry.owner_project_id != project.project_id
                    || !source_entry.analysis_eligible
                    || source_entry
                        .path
                        .as_str()
                        .starts_with(".star-control/registry/")
                {
                    continue;
                }
                let source = match read_bounded_source(
                    &canonical_root,
                    &source_entry.path,
                    MAX_FRAGMENT_BYTES,
                ) {
                    Ok(source) => source,
                    Err(_) => {
                        coverage_complete = false;
                        continue;
                    }
                };
                if Sha256Hash::digest(&source.bytes) != source_entry.content_sha256 {
                    coverage_complete = false;
                    continue;
                }
                let Some((observed_value, observed_item_version, mut state)) =
                    match_consumer_value(declaration, contract, &source.bytes)
                else {
                    continue;
                };
                if semver_greater(&contract.minimum_item_version, &observed_item_version)? {
                    state = ManagedConsumerState::BelowMinimum;
                }
                let consumer = ManagedConsumer {
                    declaration_id: declaration.managed_declaration_id.clone(),
                    project_id: project.project_id.clone(),
                    path: source_entry.path.clone(),
                    observed_value,
                    observed_item_version,
                    state,
                    source_sha256: source_entry.content_sha256.clone(),
                };
                consumers.insert(
                    (
                        consumer.declaration_id.clone(),
                        consumer.project_id.clone(),
                        consumer.path.clone(),
                    ),
                    consumer,
                );
            }
        }
    }
    Ok(ConsumerDiscovery {
        consumers: consumers.into_values().collect(),
        coverage_complete,
    })
}

fn match_consumer_value(
    declaration: &ManagedDeclaration,
    contract: &star_contracts::managed_registry::ConsumerContract,
    bytes: &[u8],
) -> Option<(String, String, ManagedConsumerState)> {
    if let Some(primary) = declaration.primary_value.as_ref()
        && contains_bytes(bytes, primary.as_bytes())
    {
        return Some((
            primary.clone(),
            declaration.item_version.clone(),
            ManagedConsumerState::Bound,
        ));
    }
    for alias in &declaration.aliases {
        if contains_bytes(bytes, alias.value.as_bytes()) {
            return Some((
                alias.value.clone(),
                alias.introduced_in_registry_version.clone(),
                ManagedConsumerState::Alias,
            ));
        }
    }
    for accepted in &contract.accepted_values {
        if contains_bytes(bytes, accepted.as_bytes()) {
            return Some((
                accepted.clone(),
                "0.0.0".to_owned(),
                ManagedConsumerState::Unresolved,
            ));
        }
    }
    None
}

pub fn plan_consumer_migration(
    before: &ManagedRegistrySnapshot,
    after: &ManagedRegistrySnapshot,
    declaration_id: &ManagedDeclarationId,
) -> Result<ConsumerMigrationPlan, DevelopmentError> {
    if before.registry_id != after.registry_id
        || before.completeness != EvidenceCompleteness::Complete
        || after.completeness != EvidenceCompleteness::Complete
        || before.freshness != RegistryFreshness::Current
        || after.freshness != RegistryFreshness::Current
    {
        return Err(DevelopmentError::Unverified);
    }
    let old = before
        .declarations
        .iter()
        .find(|entry| &entry.managed_declaration_id == declaration_id)
        .ok_or(DevelopmentError::Invalid)?;
    let new = after
        .declarations
        .iter()
        .find(|entry| &entry.managed_declaration_id == declaration_id)
        .ok_or(DevelopmentError::Invalid)?;
    let mut blockers = Vec::new();
    if old.kind != new.kind
        || old.namespace != new.namespace
        || old.owner.project_id != new.owner.project_id
    {
        blockers.push("REGISTRY_DECLARATION_IDENTITY_CHANGED".to_owned());
    }
    if !matches!(
        new.status,
        ManagedLifecycle::Active | ManagedLifecycle::Deprecated
    ) {
        blockers.push("REGISTRY_TARGET_NOT_CONSUMABLE".to_owned());
    }
    if old.primary_value != new.primary_value
        && old
            .primary_value
            .as_ref()
            .is_some_and(|old_value| !new.aliases.iter().any(|alias| &alias.value == old_value))
    {
        blockers.push("REGISTRY_OLD_VALUE_ALIAS_MISSING".to_owned());
    }
    let new_value = new
        .primary_value
        .as_ref()
        .ok_or(DevelopmentError::Invalid)?;
    let mut rewrites = Vec::new();
    for consumer in before
        .consumers
        .iter()
        .filter(|consumer| &consumer.declaration_id == declaration_id)
    {
        if matches!(
            consumer.state,
            ManagedConsumerState::Stale
                | ManagedConsumerState::Unresolved
                | ManagedConsumerState::Unverified
        ) {
            blockers.push(format!("REGISTRY_CONSUMER_NOT_CURRENT:{}", consumer.path));
            continue;
        }
        if consumer.observed_value != *new_value {
            rewrites.push(ConsumerRewrite {
                project_id: consumer.project_id.clone(),
                path: consumer.path.clone(),
                expected_source_sha256: consumer.source_sha256.clone(),
                before_value: consumer.observed_value.clone(),
                after_value: new_value.clone(),
            });
        }
    }
    blockers.sort();
    blockers.dedup();
    rewrites.sort_by(|left, right| {
        (&left.project_id, &left.path).cmp(&(&right.project_id, &right.path))
    });
    let state = if !blockers.is_empty() {
        ConsumerMigrationState::Blocked
    } else if rewrites.is_empty() {
        ConsumerMigrationState::NoChange
    } else {
        ConsumerMigrationState::Ready
    };
    let mut plan = ConsumerMigrationPlan {
        declaration_id: declaration_id.clone(),
        from_snapshot: before.content_fingerprint.clone(),
        to_snapshot: after.content_fingerprint.clone(),
        state,
        rewrites,
        blockers,
        plan_fingerprint: Sha256Hash::digest(b"unsealed-consumer-migration"),
    };
    plan.plan_fingerprint = fingerprint(
        "star.consumer-migration-plan",
        &serde_json::json!({
            "declaration_id":plan.declaration_id,
            "from_snapshot":plan.from_snapshot,
            "to_snapshot":plan.to_snapshot,
            "state":plan.state,
            "rewrites":plan.rewrites,
            "blockers":plan.blockers,
        }),
    )?;
    Ok(plan)
}

fn validate_root_manifest(
    manifest: &ManagedRegistryManifest,
    expected_owner: &ProjectId,
) -> Result<(), DevelopmentError> {
    let required = manifest
        .required_check_families
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if manifest.schema_id != MANAGED_REGISTRY_MANIFEST_SCHEMA_ID
        || manifest.schema_version != 1
        || manifest.owner_project_id != *expected_owner
        || !valid_token(&manifest.registry_id, 128)
        || !valid_semver(&manifest.registry_version)
        || manifest.namespace_claims.is_empty()
        || manifest.declaration_files.is_empty()
        || REQUIRED_REGISTRY_CHECKS
            .iter()
            .any(|family| !required.contains(family))
        || manifest
            .compatibility_policy_ref
            .catalog_id
            .trim()
            .is_empty()
        || manifest
            .compatibility_policy_ref
            .item_version
            .trim()
            .is_empty()
    {
        return Err(DevelopmentError::Invalid);
    }
    Ok(())
}

fn canonicalize_root(manifest: &mut ManagedRegistryManifest) -> Result<(), DevelopmentError> {
    manifest.declaration_files.sort();
    if manifest
        .declaration_files
        .windows(2)
        .any(|pair| pair[0] == pair[1])
    {
        return Err(DevelopmentError::Conflict);
    }
    manifest
        .namespace_claims
        .sort_by(|left, right| left.namespace.cmp(&right.namespace));
    if manifest
        .namespace_claims
        .windows(2)
        .any(|pair| pair[0].namespace == pair[1].namespace)
    {
        return Err(DevelopmentError::Conflict);
    }
    manifest.required_check_families.sort();
    manifest.required_check_families.dedup();
    Ok(())
}

fn resolve_namespace_claims(
    manifest: &ManagedRegistryManifest,
) -> Result<BTreeMap<&str, &star_contracts::managed_registry::NamespaceClaim>, DevelopmentError> {
    let mut claims = BTreeMap::new();
    let mut delegated = BTreeSet::new();
    for claim in &manifest.namespace_claims {
        if !valid_namespace(&claim.namespace)
            || claim.allowed_kinds.is_empty()
            || !valid_semver(&claim.introduced_in_registry_version)
            || claims.insert(claim.namespace.as_str(), claim).is_some()
        {
            return Err(DevelopmentError::Conflict);
        }
        for child in &claim.delegated_child_namespaces {
            if !valid_namespace(&child.namespace)
                || !child
                    .namespace
                    .starts_with(&format!("{}.", claim.namespace))
                || child.allowed_kinds.is_empty()
                || !delegated.insert(child.namespace.clone())
            {
                return Err(DevelopmentError::Conflict);
            }
        }
    }
    Ok(claims)
}

fn validate_declaration_source(
    declaration: &star_contracts::managed_registry::ManagedDeclarationSource,
    registry_version: &str,
) -> Result<(), DevelopmentError> {
    let primary_required = !matches!(declaration.status, ManagedLifecycle::Reserved);
    if !valid_semver(&declaration.item_version)
        || !valid_namespace(&declaration.namespace)
        || !valid_token(&declaration.semantic_key, 160)
        || declaration.value_type.trim().is_empty()
        || declaration.description.trim().is_empty()
        || declaration.description.len() > 4_096
        || declaration.uniqueness_scope.trim().is_empty()
        || (primary_required
            && declaration
                .primary_value
                .as_deref()
                .is_none_or(str::is_empty))
        || !valid_semver(&declaration.lifecycle.introduced_in_registry_version)
        || semver_greater(
            &declaration.lifecycle.introduced_in_registry_version,
            registry_version,
        )?
    {
        return Err(DevelopmentError::Invalid);
    }
    match declaration.status {
        ManagedLifecycle::Active
            if declaration
                .lifecycle
                .deprecated_in_registry_version
                .is_some()
                || declaration.lifecycle.removed_in_registry_version.is_some() =>
        {
            return Err(DevelopmentError::Conflict);
        }
        ManagedLifecycle::Deprecated
            if declaration
                .lifecycle
                .deprecated_in_registry_version
                .is_none()
                || declaration.lifecycle.removed_in_registry_version.is_some() =>
        {
            return Err(DevelopmentError::Conflict);
        }
        ManagedLifecycle::Removed
            if declaration.lifecycle.removed_in_registry_version.is_none() =>
        {
            return Err(DevelopmentError::Conflict);
        }
        _ => {}
    }
    let mut aliases = BTreeSet::new();
    for alias in &declaration.aliases {
        validate_alias(alias, declaration.primary_value.as_deref())?;
        if !aliases.insert(alias.value.as_str()) {
            return Err(DevelopmentError::Conflict);
        }
    }
    let mut bindings = BTreeSet::new();
    for binding in &declaration.binding_specs {
        if !valid_token(&binding.binding_id, 160)
            || binding.expected_value.is_empty()
            || !bindings.insert(binding.binding_id.as_str())
        {
            return Err(DevelopmentError::Conflict);
        }
    }
    let mut consumers = BTreeSet::new();
    for consumer in &declaration.consumer_contracts {
        if !valid_token(&consumer.consumer_surface_id, 160)
            || !valid_semver(&consumer.minimum_item_version)
            || !consumers.insert(consumer.consumer_surface_id.as_str())
        {
            return Err(DevelopmentError::Conflict);
        }
    }
    Ok(())
}

fn validate_alias(alias: &AliasRecord, primary: Option<&str>) -> Result<(), DevelopmentError> {
    if alias.value.trim().is_empty()
        || alias.value.len() > 512
        || primary == Some(alias.value.as_str())
        || !valid_semver(&alias.introduced_in_registry_version)
        || (alias.expires_in_registry_version.is_none() && alias.expires_at.is_none())
        || alias
            .expires_in_registry_version
            .as_ref()
            .is_some_and(|version| !valid_semver(version))
    {
        return Err(DevelopmentError::Conflict);
    }
    Ok(())
}

fn validate_consumers(
    declarations: &[ManagedDeclaration],
    consumers: &[ManagedConsumer],
) -> Result<(), DevelopmentError> {
    let declarations = declarations
        .iter()
        .map(|declaration| (&declaration.managed_declaration_id, declaration))
        .collect::<BTreeMap<_, _>>();
    for consumer in consumers {
        let declaration = declarations
            .get(&consumer.declaration_id)
            .ok_or(DevelopmentError::Conflict)?;
        if consumer.observed_item_version.trim().is_empty() {
            return Err(DevelopmentError::Invalid);
        }
        let expected =
            if declaration.primary_value.as_deref() == Some(consumer.observed_value.as_str()) {
                ManagedConsumerState::Bound
            } else if declaration
                .aliases
                .iter()
                .any(|alias| alias.value == consumer.observed_value)
            {
                ManagedConsumerState::Alias
            } else {
                consumer.state
            };
        if matches!(
            consumer.state,
            ManagedConsumerState::Bound | ManagedConsumerState::Alias
        ) && expected != consumer.state
        {
            return Err(DevelopmentError::Conflict);
        }
    }
    Ok(())
}

fn canonicalize_candidates(candidates: &mut [ManagedCandidate]) -> Result<(), DevelopmentError> {
    candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    if candidates
        .windows(2)
        .any(|pair| pair[0].candidate_id == pair[1].candidate_id)
        || candidates.iter().any(|candidate| {
            candidate.candidate_id.trim().is_empty()
                || candidate.observed_value.is_empty()
                || candidate.reason_codes.is_empty()
        })
    {
        return Err(DevelopmentError::Conflict);
    }
    Ok(())
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CandidateClassificationRecord {
    candidate_id: String,
    classification: ManagedDeclarationClassification,
    reason: String,
}

fn apply_source_candidate_classifications(
    manifest: &ManagedRegistryManifest,
    candidates: &mut Vec<ManagedCandidate>,
    local_constants: &mut Vec<ManagedCandidate>,
) -> Result<Vec<ConsistencySeed>, DevelopmentError> {
    let mut inventory = BTreeMap::new();
    for candidate in candidates.drain(..).chain(local_constants.drain(..)) {
        if inventory
            .insert(candidate.candidate_id.clone(), candidate)
            .is_some()
        {
            return Err(DevelopmentError::Conflict);
        }
    }
    let records = manifest
        .extensions
        .get("star.registry.candidate_classifications")
        .cloned()
        .map(serde_json::from_value::<Vec<CandidateClassificationRecord>>)
        .transpose()
        .map_err(|_| DevelopmentError::Invalid)?
        .unwrap_or_default();
    let mut seen = BTreeSet::new();
    let mut consistency = Vec::new();
    for record in records {
        if !valid_token(&record.candidate_id, 256)
            || record.reason.trim().is_empty()
            || record.reason.chars().count() > 2_048
            || !seen.insert(record.candidate_id.clone())
        {
            return Err(DevelopmentError::Conflict);
        }
        let Some(candidate) = inventory.get_mut(&record.candidate_id) else {
            consistency.push(ConsistencySeed {
                declaration_ref: None,
                status: RegistryConsistencyStatus::BindingDrift,
                subject: format!("candidate:{}", record.candidate_id),
                expected_value: Some(
                    candidate_classification_name(record.classification).to_owned(),
                ),
                observed_value: None,
                remediation:
                    "remove the stale source classification or restore the tracked candidate"
                        .to_owned(),
            });
            continue;
        };
        candidate.classification = record.classification;
        candidate.reason_codes.push(format!(
            "SOURCE_CLASSIFICATION:{}",
            candidate_classification_name(record.classification)
        ));
        candidate.reason_codes.sort();
        candidate.reason_codes.dedup();
    }
    for (_, candidate) in inventory {
        if candidate.classification == ManagedDeclarationClassification::LocalImplementationConstant
        {
            local_constants.push(candidate);
        } else {
            candidates.push(candidate);
        }
    }
    canonicalize_candidates(candidates)?;
    canonicalize_candidates(local_constants)?;
    Ok(consistency)
}

fn candidate_classification_name(classification: ManagedDeclarationClassification) -> &'static str {
    match classification {
        ManagedDeclarationClassification::ManagedDeclaration => "managed_declaration",
        ManagedDeclarationClassification::Candidate => "candidate",
        ManagedDeclarationClassification::LocalImplementationConstant => {
            "local_implementation_constant"
        }
    }
}

fn observe_bindings(
    root: &Path,
    declarations: &[ManagedDeclaration],
) -> Result<(Vec<BindingObservation>, Vec<ConsistencySeed>), DevelopmentError> {
    let mut observations = Vec::new();
    let mut records = Vec::new();
    for declaration in declarations {
        for binding in &declaration.binding_specs {
            let source = read_bounded_source(root, &binding.path, MAX_FRAGMENT_BYTES);
            let (observed_value, current, source_sha256) = match source {
                Ok(source) => {
                    let current = contains_bytes(&source.bytes, binding.expected_value.as_bytes());
                    (
                        current.then(|| binding.expected_value.clone()),
                        current,
                        Sha256Hash::digest(&source.bytes),
                    )
                }
                Err(_) => (None, false, Sha256Hash::digest(b"missing-binding-source")),
            };
            let declaration_ref = ManagedDeclarationRef {
                managed_declaration_id: declaration.managed_declaration_id.clone(),
                item_version: declaration.item_version.clone(),
                definition_fingerprint: declaration.definition_fingerprint.clone(),
            };
            records.push(ConsistencySeed {
                declaration_ref: Some(declaration_ref),
                status: if current {
                    RegistryConsistencyStatus::Current
                } else {
                    RegistryConsistencyStatus::BindingDrift
                },
                subject: binding.binding_id.clone(),
                expected_value: Some(binding.expected_value.clone()),
                observed_value: observed_value.clone(),
                remediation: if current {
                    "none".to_owned()
                } else {
                    "rebuild the binding from the Git registry source and rerun M3".to_owned()
                },
            });
            observations.push(BindingObservation {
                declaration_id: declaration.managed_declaration_id.clone(),
                binding_id: binding.binding_id.clone(),
                path: binding.path.clone(),
                expected_value: binding.expected_value.clone(),
                observed_value,
                current,
                source_sha256,
            });
        }
    }
    observations.sort_by(|left, right| {
        (&left.declaration_id, &left.binding_id).cmp(&(&right.declaration_id, &right.binding_id))
    });
    Ok((observations, records))
}

fn consumer_consistency(
    declarations: &[ManagedDeclaration],
    consumers: &[ManagedConsumer],
) -> Vec<ConsistencySeed> {
    let by_id = declarations
        .iter()
        .map(|declaration| (&declaration.managed_declaration_id, declaration))
        .collect::<BTreeMap<_, _>>();
    let mut records = consumers
        .iter()
        .filter_map(|consumer| {
            let declaration = by_id.get(&consumer.declaration_id)?;
            let status = match consumer.state {
                ManagedConsumerState::Bound | ManagedConsumerState::Alias => {
                    RegistryConsistencyStatus::Current
                }
                _ if declaration.status == ManagedLifecycle::Removed => {
                    RegistryConsistencyStatus::RemovedReference
                }
                _ => RegistryConsistencyStatus::ConsumerDrift,
            };
            Some(ConsistencySeed {
                declaration_ref: Some(ManagedDeclarationRef {
                    managed_declaration_id: declaration.managed_declaration_id.clone(),
                    item_version: declaration.item_version.clone(),
                    definition_fingerprint: declaration.definition_fingerprint.clone(),
                }),
                status,
                subject: format!("{}:{}", consumer.project_id, consumer.path),
                expected_value: declaration.primary_value.clone(),
                observed_value: Some(consumer.observed_value.clone()),
                remediation: if status == RegistryConsistencyStatus::Current {
                    "none".to_owned()
                } else {
                    "plan an exact consumer migration before removal".to_owned()
                },
            })
        })
        .collect::<Vec<_>>();
    let observed = consumers
        .iter()
        .map(|consumer| (&consumer.declaration_id, &consumer.project_id))
        .collect::<BTreeSet<_>>();
    for declaration in declarations {
        for contract in &declaration.consumer_contracts {
            if contract.requirement == ManagedConsumerRequirement::Required
                && !observed.contains(&(&declaration.managed_declaration_id, &contract.project_id))
            {
                records.push(ConsistencySeed {
                    declaration_ref: Some(ManagedDeclarationRef {
                        managed_declaration_id: declaration.managed_declaration_id.clone(),
                        item_version: declaration.item_version.clone(),
                        definition_fingerprint: declaration.definition_fingerprint.clone(),
                    }),
                    status: RegistryConsistencyStatus::ConsumerDrift,
                    subject: format!(
                        "consumer-surface:{}:{}",
                        contract.consumer_surface_id, contract.project_id
                    ),
                    expected_value: Some(format!(
                        "minimum_item_version={}",
                        contract.minimum_item_version
                    )),
                    observed_value: None,
                    remediation:
                        "restore the required tracked consumer or revise its source contract"
                            .to_owned(),
                });
            }
        }
    }
    records
}

fn seal_consistency_record(
    snapshot_ref: &DocumentRef,
    seed: ConsistencySeed,
) -> Result<RegistryConsistencyRecord, DevelopmentError> {
    let fingerprint = fingerprint(
        REGISTRY_CONSISTENCY_RECORD_SCHEMA_ID,
        &serde_json::json!({
            "registry_snapshot_ref":snapshot_ref,
            "declaration_ref":seed.declaration_ref,
            "status":seed.status,
            "subject":seed.subject,
            "expected_value":seed.expected_value,
            "observed_value":seed.observed_value,
            "completeness":if seed.status == RegistryConsistencyStatus::Unverified {
                EvidenceCompleteness::Unverified
            } else {
                EvidenceCompleteness::Complete
            },
            "remediation":seed.remediation,
        }),
    )?;
    RegistryConsistencyRecord {
        schema_id: REGISTRY_CONSISTENCY_RECORD_SCHEMA_ID.to_owned(),
        schema_version: 1,
        registry_consistency_record_id: RegistryConsistencyRecordId::from_fingerprint(&fingerprint),
        registry_snapshot_ref: snapshot_ref.clone(),
        declaration_ref: seed.declaration_ref,
        status: seed.status,
        subject: seed.subject,
        expected_value: seed.expected_value,
        observed_value: seed.observed_value,
        completeness: if seed.status == RegistryConsistencyStatus::Unverified {
            EvidenceCompleteness::Unverified
        } else {
            EvidenceCompleteness::Complete
        },
        evidence_refs: vec![],
        remediation: seed.remediation,
        record_fingerprint: fingerprint,
    }
    .seal()
    .map_err(|_| DevelopmentError::Conflict)
}

fn registry_snapshot_ref(
    snapshot: &ManagedRegistrySnapshot,
) -> Result<DocumentRef, DevelopmentError> {
    snapshot.reference().map_err(|_| DevelopmentError::Conflict)
}

fn declaration_fingerprint(
    declaration: &star_contracts::managed_registry::ManagedDeclarationSource,
) -> Result<Sha256Hash, DevelopmentError> {
    fingerprint(
        "star.managed-declaration",
        &serde_json::json!({
            "managed_declaration_id":declaration.managed_declaration_id,
            "item_version":declaration.item_version,
            "namespace":declaration.namespace,
            "semantic_key":declaration.semantic_key,
            "kind":declaration.kind,
            "owner":{
                "project_id":declaration.owner.project_id,
                "contract_id":declaration.owner.contract_id,
                "module_key":declaration.owner.module_key,
                "approval_policy_ref":declaration.owner.approval_policy_ref,
            },
            "value_type":declaration.value_type,
            "value_role":declaration.value_role,
            "primary_value":declaration.primary_value,
            "status":declaration.status,
            "lifecycle":declaration.lifecycle,
            "aliases":declaration.aliases,
            "binding_specs":declaration.binding_specs,
            "consumer_contracts":declaration.consumer_contracts,
            "uniqueness_scope":declaration.uniqueness_scope,
        }),
    )
}

fn manifest_set_fingerprint(
    manifest: &ManagedRegistryManifest,
    root: &LoadedSource,
    fragments: &[LoadedSource],
) -> Result<Sha256Hash, DevelopmentError> {
    fingerprint(
        "star.managed-registry-manifest-set",
        &serde_json::json!({
            "manifest":manifest,
            "root_source_sha256":Sha256Hash::digest(&root.bytes),
            "fragment_sources":fragments.iter().map(|source| serde_json::json!({
                "path":source.path,
                "sha256":Sha256Hash::digest(&source.bytes),
            })).collect::<Vec<_>>(),
        }),
    )
}

fn change_kind_matches_fields(
    kind: ManagedDeclarationChangeKind,
    fields: &ManagedDesiredFields,
) -> bool {
    matches!(
        (kind, fields),
        (
            ManagedDeclarationChangeKind::Create,
            ManagedDesiredFields::Create { .. }
        ) | (
            ManagedDeclarationChangeKind::UpdateDescription,
            ManagedDesiredFields::UpdateDescription { .. }
        ) | (
            ManagedDeclarationChangeKind::ChangePrimaryValue,
            ManagedDesiredFields::ChangePrimaryValue { .. }
        ) | (
            ManagedDeclarationChangeKind::Deprecate,
            ManagedDesiredFields::Deprecate { .. }
        ) | (
            ManagedDeclarationChangeKind::AddAlias,
            ManagedDesiredFields::AddAlias { .. }
        ) | (
            ManagedDeclarationChangeKind::Remove,
            ManagedDesiredFields::Remove { .. }
        ) | (
            ManagedDeclarationChangeKind::AddBinding,
            ManagedDesiredFields::AddBinding { .. }
        ) | (
            ManagedDeclarationChangeKind::ChangeConsumerFloor,
            ManagedDesiredFields::ChangeConsumerFloor { .. }
        ) | (
            ManagedDeclarationChangeKind::ClassifyCandidate,
            ManagedDesiredFields::ClassifyCandidate { .. }
        )
    )
}

fn validate_desired_transition(
    snapshot: &ManagedRegistrySnapshot,
    declaration_ref: Option<&ManagedDeclarationRef>,
    desired: &ManagedDesiredFields,
) -> Result<(), DevelopmentError> {
    if let ManagedDesiredFields::Create { declaration } = desired {
        if snapshot.declarations.iter().any(|existing| {
            existing.managed_declaration_id == declaration.managed_declaration_id
                || existing.primary_value == declaration.primary_value
        }) {
            return Err(DevelopmentError::Conflict);
        }
        return Ok(());
    }
    if let ManagedDesiredFields::ClassifyCandidate { candidate_id, .. } = desired {
        if snapshot
            .candidates
            .iter()
            .chain(&snapshot.local_constants)
            .any(|candidate| candidate.candidate_id == *candidate_id)
        {
            return Ok(());
        }
        return Err(DevelopmentError::Invalid);
    }
    let declaration_ref = declaration_ref.ok_or(DevelopmentError::Invalid)?;
    let declaration = snapshot
        .declarations
        .iter()
        .find(|entry| entry.managed_declaration_id == declaration_ref.managed_declaration_id)
        .ok_or(DevelopmentError::Invalid)?;
    match desired {
        ManagedDesiredFields::UpdateDescription { description }
            if description.trim().is_empty() || description.len() > 4_096 =>
        {
            Err(DevelopmentError::Invalid)
        }
        ManagedDesiredFields::ChangePrimaryValue {
            primary_value,
            new_item_version,
        } if primary_value.trim().is_empty()
            || !valid_semver(new_item_version)
            || new_item_version == &declaration.item_version =>
        {
            Err(DevelopmentError::Conflict)
        }
        ManagedDesiredFields::Deprecate { .. }
            if declaration.status != ManagedLifecycle::Active =>
        {
            Err(DevelopmentError::Conflict)
        }
        ManagedDesiredFields::Remove { .. }
            if declaration.status != ManagedLifecycle::Deprecated =>
        {
            Err(DevelopmentError::Conflict)
        }
        ManagedDesiredFields::AddAlias { alias }
            if declaration
                .aliases
                .iter()
                .any(|item| item.value == alias.value) =>
        {
            Err(DevelopmentError::Conflict)
        }
        _ => Ok(()),
    }
}

fn read_bounded_source(
    root: &Path,
    path: &ProjectPathRef,
    max_bytes: u64,
) -> Result<LoadedSource, DevelopmentError> {
    let candidate = root.join(path.as_str());
    let metadata = fs::symlink_metadata(&candidate).map_err(|_| DevelopmentError::Adapter)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > max_bytes {
        return Err(DevelopmentError::Blocked);
    }
    let canonical = fs::canonicalize(&candidate).map_err(|_| DevelopmentError::Adapter)?;
    if !canonical.starts_with(root) {
        return Err(DevelopmentError::Blocked);
    }
    let bytes = fs::read(canonical).map_err(|_| DevelopmentError::Adapter)?;
    Ok(LoadedSource {
        path: path.clone(),
        bytes,
    })
}

fn require_git_tracked(root: &Path, path: &ProjectPathRef) -> Result<(), DevelopmentError> {
    let status = Command::new("git")
        .current_dir(root)
        .args(["--literal-pathspecs", "ls-files", "--error-unmatch", "--"])
        .arg(path.as_str())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| DevelopmentError::Adapter)?;
    status
        .success()
        .then_some(())
        .ok_or(DevelopmentError::Blocked)
}

fn git_text(root: &Path, args: &[&str]) -> Result<String, DevelopmentError> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|_| DevelopmentError::Adapter)?;
    if !output.status.success() || output.stdout.len() > 64 * 1024 {
        return Err(DevelopmentError::Adapter);
    }
    String::from_utf8(output.stdout)
        .map(|text| text.trim().to_owned())
        .map_err(|_| DevelopmentError::Invalid)
}

fn fingerprint(
    domain: &str,
    value: &impl serde::Serialize,
) -> Result<Sha256Hash, DevelopmentError> {
    versioned_fingerprint(domain, 1, value).map_err(|_| DevelopmentError::Fingerprint)
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn candidate_source_path(path: &str) -> bool {
    matches!(
        Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("rs" | "json" | "toml" | "md" | "yaml" | "yml")
    )
}

fn quoted_values(line: &str) -> Vec<&str> {
    let bytes = line.as_bytes();
    let mut values = Vec::new();
    let mut start = None;
    let mut quote = 0_u8;
    let mut escaped = false;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if let Some(value_start) = start {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == quote {
                if let Some(value) = line.get(value_start..index) {
                    values.push(value);
                }
                start = None;
            }
        } else if matches!(byte, b'\'' | b'"') {
            quote = byte;
            start = Some(index + 1);
        }
    }
    values
}

fn candidate_kind(value: &str) -> Option<ManagedDeclarationKind> {
    if value.len() < 4 || value.len() > 160 || value.chars().any(char::is_whitespace) {
        return None;
    }
    let upper = value.to_ascii_uppercase();
    if upper.contains("DIAGNOSTIC") || upper.starts_with("DIA_") {
        Some(ManagedDeclarationKind::DiagnosticId)
    } else if upper.contains("ERROR")
        || (value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
            && value.contains('_'))
    {
        Some(ManagedDeclarationKind::ErrorCode)
    } else if value.starts_with("star.") && value.contains("schema") {
        Some(ManagedDeclarationKind::SchemaId)
    } else if value.contains('.')
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
    {
        Some(ManagedDeclarationKind::ConfigKey)
    } else {
        None
    }
}

fn valid_namespace(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'-' | b'_')
                })
        })
}

fn valid_token(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn valid_semver(value: &str) -> bool {
    let core = value.split_once('-').map_or(value, |(core, _)| core);
    let parts = core.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn semver_greater(left: &str, right: &str) -> Result<bool, DevelopmentError> {
    fn parse(value: &str) -> Result<[u64; 3], DevelopmentError> {
        let core = value.split_once('-').map_or(value, |(core, _)| core);
        let values = core
            .split('.')
            .map(|part| part.parse::<u64>().map_err(|_| DevelopmentError::Invalid))
            .collect::<Result<Vec<_>, _>>()?;
        values.try_into().map_err(|_| DevelopmentError::Invalid)
    }
    Ok(parse(left)? > parse(right)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::{
        evidence::CatalogRef,
        ids::CanonicalSourceId,
        index::SourceClass,
        managed_registry::{
            BindingSpec, ConsumerContract, ManagedBindingKind, ManagedDeclarationKind,
            ManagedDeclarationSource, ManagedOwnerRef, ManagedValueRole, NamespaceClaim,
            NamespaceClaimStatus,
        },
    };

    #[test]
    fn semantic_fingerprint_ignores_description_and_source_provenance() {
        let declaration = ManagedDeclarationSource {
            managed_declaration_id: ManagedDeclarationId::parse("star.error.invalid-input")
                .unwrap(),
            item_version: "1.0.0".to_owned(),
            namespace: "star.error".to_owned(),
            semantic_key: "invalid-input".to_owned(),
            kind: ManagedDeclarationKind::ErrorCode,
            owner: ManagedOwnerRef {
                project_id: ProjectId::parse("prj_00000000000000000000000000").unwrap(),
                contract_id: Some("star.errors".to_owned()),
                module_key: Some("errors".to_owned()),
                approval_policy_ref: CatalogRef {
                    catalog_id: "star.policy.registry".to_owned(),
                    format_version: 1,
                    item_version: "1.0.0".to_owned(),
                    sha256: Sha256Hash::digest(b"policy"),
                },
                display_owner: Some("Errors".to_owned()),
            },
            value_type: "string".to_owned(),
            value_role: ManagedValueRole::StableIdentifier,
            primary_value: Some("INVALID_INPUT".to_owned()),
            description: "first".to_owned(),
            status: ManagedLifecycle::Active,
            lifecycle: star_contracts::managed_registry::ManagedLifecycleRecord {
                introduced_in_registry_version: "1.0.0".to_owned(),
                deprecated_in_registry_version: None,
                removed_in_registry_version: None,
                replacement_id: None,
                migration_record_ref: None,
            },
            aliases: vec![],
            binding_specs: vec![BindingSpec {
                binding_id: "rust.error.invalid-input".to_owned(),
                kind: ManagedBindingKind::Definition,
                path: ProjectPathRef::parse("src/errors.rs").unwrap(),
                symbol_key: Some("INVALID_INPUT".to_owned()),
                expected_value: "INVALID_INPUT".to_owned(),
                required: true,
                generator_id: None,
            }],
            consumer_contracts: vec![],
            uniqueness_scope: "star.error".to_owned(),
        };
        let mut display_only = declaration.clone();
        display_only.description = "second".to_owned();
        display_only.owner.display_owner = Some("Different".to_owned());
        assert_eq!(
            declaration_fingerprint(&declaration).unwrap(),
            declaration_fingerprint(&display_only).unwrap()
        );
    }

    #[test]
    fn root_requires_explicit_registry_checks_and_namespace_claim() {
        let owner = ProjectId::parse("prj_00000000000000000000000000").unwrap();
        let mut manifest = ManagedRegistryManifest {
            schema_id: MANAGED_REGISTRY_MANIFEST_SCHEMA_ID.to_owned(),
            schema_version: 1,
            registry_id: "star.shared".to_owned(),
            registry_version: "1.0.0".to_owned(),
            owner_project_id: owner.clone(),
            namespace_claims: vec![NamespaceClaim {
                namespace: "star.error".to_owned(),
                owner_project_id: owner.clone(),
                allowed_kinds: vec![ManagedDeclarationKind::ErrorCode],
                delegated_child_namespaces: vec![],
                status: NamespaceClaimStatus::Active,
                introduced_in_registry_version: "1.0.0".to_owned(),
                transfer_ref: None,
            }],
            declaration_files: vec![ProjectPathRef::parse("registry/errors.toml").unwrap()],
            compatibility_policy_ref: CatalogRef {
                catalog_id: "star.policy.registry".to_owned(),
                format_version: 1,
                item_version: "1.0.0".to_owned(),
                sha256: Sha256Hash::digest(b"policy"),
            },
            required_check_families: REQUIRED_REGISTRY_CHECKS
                .iter()
                .map(|item| (*item).to_owned())
                .collect(),
            extensions: BTreeMap::new(),
        };
        validate_root_manifest(&manifest, &owner).unwrap();
        manifest.required_check_families.pop();
        assert_eq!(
            validate_root_manifest(&manifest, &owner),
            Err(DevelopmentError::Invalid)
        );
    }

    #[test]
    fn git_source_only_loader_rebuilds_snapshot_and_detects_binding_drift() {
        let owner = ProjectId::parse("prj_00000000000000000000000000").unwrap();
        let checkout = CheckoutId::parse("cko_00000000000000000000000000").unwrap();
        let root = std::env::temp_dir().join(format!(
            "star-managed-registry-{}-{}",
            std::process::id(),
            ManagedRegistrySnapshotId::new()
        ));
        fs::create_dir_all(root.join(".star-control/registry")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        let policy = CatalogRef {
            catalog_id: "star.policy.registry".to_owned(),
            format_version: 1,
            item_version: "1.0.0".to_owned(),
            sha256: Sha256Hash::digest(b"policy"),
        };
        let manifest = ManagedRegistryManifest {
            schema_id: MANAGED_REGISTRY_MANIFEST_SCHEMA_ID.to_owned(),
            schema_version: 1,
            registry_id: "star.shared".to_owned(),
            registry_version: "1.0.0".to_owned(),
            owner_project_id: owner.clone(),
            namespace_claims: vec![NamespaceClaim {
                namespace: "star.error".to_owned(),
                owner_project_id: owner.clone(),
                allowed_kinds: vec![ManagedDeclarationKind::ErrorCode],
                delegated_child_namespaces: vec![],
                status: NamespaceClaimStatus::Active,
                introduced_in_registry_version: "1.0.0".to_owned(),
                transfer_ref: None,
            }],
            declaration_files: vec![
                ProjectPathRef::parse(".star-control/registry/errors.toml").unwrap(),
            ],
            compatibility_policy_ref: policy.clone(),
            required_check_families: REQUIRED_REGISTRY_CHECKS
                .iter()
                .map(|item| (*item).to_owned())
                .collect(),
            extensions: BTreeMap::new(),
        };
        let declaration = ManagedDeclarationSource {
            managed_declaration_id: ManagedDeclarationId::parse(
                "star.error.management-store-unavailable",
            )
            .unwrap(),
            item_version: "1.0.0".to_owned(),
            namespace: "star.error".to_owned(),
            semantic_key: "management-store-unavailable".to_owned(),
            kind: ManagedDeclarationKind::ErrorCode,
            owner: ManagedOwnerRef {
                project_id: owner.clone(),
                contract_id: Some("star.errors".to_owned()),
                module_key: Some("errors".to_owned()),
                approval_policy_ref: policy,
                display_owner: Some("management".to_owned()),
            },
            value_type: "string".to_owned(),
            value_role: ManagedValueRole::StableIdentifier,
            primary_value: Some("MANAGEMENT_STORE_UNAVAILABLE".to_owned()),
            description: "Stable management storage error code".to_owned(),
            status: ManagedLifecycle::Active,
            lifecycle: star_contracts::managed_registry::ManagedLifecycleRecord {
                introduced_in_registry_version: "1.0.0".to_owned(),
                deprecated_in_registry_version: None,
                removed_in_registry_version: None,
                replacement_id: None,
                migration_record_ref: None,
            },
            aliases: vec![],
            binding_specs: vec![BindingSpec {
                binding_id: "rust.management-store-unavailable".to_owned(),
                kind: ManagedBindingKind::Definition,
                path: ProjectPathRef::parse("src/errors.rs").unwrap(),
                symbol_key: Some("MANAGEMENT_STORE_UNAVAILABLE".to_owned()),
                expected_value: "MANAGEMENT_STORE_UNAVAILABLE".to_owned(),
                required: true,
                generator_id: None,
            }],
            consumer_contracts: vec![ConsumerContract {
                consumer_surface_id: "star-controller.error-display".to_owned(),
                project_id: owner.clone(),
                requirement: ManagedConsumerRequirement::Optional,
                minimum_item_version: "1.0.0".to_owned(),
                accepted_values: vec![],
                migration_window_end: None,
            }],
            uniqueness_scope: "star.error".to_owned(),
        };
        let fragment = ManagedRegistryFragment {
            schema_id: MANAGED_REGISTRY_FRAGMENT_SCHEMA_ID.to_owned(),
            schema_version: 1,
            registry_id: "star.shared".to_owned(),
            namespace: "star.error".to_owned(),
            declarations: vec![declaration],
            source_description: None,
        };
        fs::write(
            root.join(".star-control/registry/manifest.toml"),
            toml::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        fs::write(
            root.join(".star-control/registry/errors.toml"),
            toml::to_string_pretty(&fragment).unwrap(),
        )
        .unwrap();
        fs::write(
            root.join("src/errors.rs"),
            "pub const CODE: &str = \"MANAGEMENT_STORE_UNAVAILABLE\";\npub const OTHER: &str = \"OTHER_ERROR\";\nconst LOCAL: &str = \"LOCAL_ONLY_ERROR\";\n",
        )
        .unwrap();
        git_ok(&root, &["init", "-b", "main"]);
        git_ok(&root, &["config", "user.email", "fixture@example.invalid"]);
        git_ok(&root, &["config", "user.name", "Fixture"]);
        git_ok(&root, &["add", "."]);
        git_ok(&root, &["commit", "-m", "registry fixture"]);

        let input = RegistryResolutionInput {
            owner_project_id: owner.clone(),
            checkout_id: checkout.clone(),
            project_revision_id: ProjectRevisionId::from_stable_bytes(b"revision"),
            workspace_snapshot_id: WorkspaceSnapshotId::from_stable_bytes(b"workspace"),
            code_index_snapshot_id: Some(CodeIndexSnapshotId::from_stable_bytes(b"index")),
            index_current: true,
            coverage_complete: true,
            consumers: vec![],
            candidates: vec![],
            local_constants: vec![],
        };
        let path = ProjectPathRef::parse(".star-control/registry/manifest.toml").unwrap();
        let resolved = load_git_registry_from_project(&root, &path, input.clone()).unwrap();
        assert_eq!(
            resolved.snapshot.completeness,
            EvidenceCompleteness::Complete
        );
        assert_eq!(resolved.snapshot.freshness, RegistryFreshness::Current);
        assert_eq!(resolved.snapshot.declarations.len(), 1);
        assert!(
            resolved
                .consistency_records
                .iter()
                .all(|record| record.status == RegistryConsistencyStatus::Current)
        );
        let candidates = scan_git_registry_candidates(&root, &resolved.snapshot).unwrap();
        assert!(
            candidates
                .candidates
                .iter()
                .any(|candidate| candidate.observed_value == "OTHER_ERROR")
        );
        assert!(
            candidates
                .local_constants
                .iter()
                .any(|candidate| candidate.observed_value == "LOCAL_ONLY_ERROR")
        );

        let source_bytes = fs::read(root.join("src/errors.rs")).unwrap();
        let consumer_discovery = discover_registry_consumers(
            &resolved.snapshot,
            &[ConsumerProjectInput {
                project_id: owner.clone(),
                project_root: root.clone(),
                source_entries: vec![SourceEntry {
                    canonical_source_id: CanonicalSourceId::from_stable_bytes(b"errors-source"),
                    path: ProjectPathRef::parse("src/errors.rs").unwrap(),
                    content_sha256: Sha256Hash::digest(&source_bytes),
                    size_bytes: source_bytes.len() as u64,
                    source_class: SourceClass::Source,
                    facets: vec![],
                    language_id: "rust".to_owned(),
                    encoding: "utf-8".to_owned(),
                    owner_project_id: owner.clone(),
                    owner_checkout_id: checkout.clone(),
                    analysis_eligible: true,
                    content_fingerprint: Sha256Hash::digest(b"errors-source-entry"),
                }],
                index_current: true,
                coverage_complete: true,
            }],
        )
        .unwrap();
        assert!(consumer_discovery.coverage_complete);
        assert_eq!(consumer_discovery.consumers.len(), 1);
        assert_eq!(
            consumer_discovery.consumers[0].state,
            ManagedConsumerState::Bound
        );

        let classified_input = RegistryResolutionInput {
            consumers: consumer_discovery.consumers,
            candidates: candidates.candidates.clone(),
            local_constants: candidates.local_constants.clone(),
            ..input.clone()
        };
        let classified_base =
            load_git_registry_from_project(&root, &path, classified_input.clone()).unwrap();
        let candidate = classified_base
            .snapshot
            .candidates
            .iter()
            .find(|candidate| candidate.observed_value == "OTHER_ERROR")
            .unwrap();
        let classify_intent = build_change_intent(
            &classified_base.snapshot,
            None,
            ManagedDeclarationChangeKind::ClassifyCandidate,
            ManagedDesiredFields::ClassifyCandidate {
                candidate_id: candidate.candidate_id.clone(),
                classification: ManagedDeclarationClassification::LocalImplementationConstant,
            },
            "confirmed implementation-local error constant".to_owned(),
            vec![],
        )
        .unwrap();
        let classification_rewrite =
            prepare_registry_change_rewrite(&root, &classified_base.snapshot, &classify_intent)
                .unwrap();
        assert_eq!(classification_rewrite.len(), 1);
        fs::write(
            root.join(classification_rewrite[0].path.as_str()),
            &classification_rewrite[0].after_bytes,
        )
        .unwrap();
        let classified =
            load_git_registry_from_project(&root, &path, classified_input.clone()).unwrap();
        assert!(classified.snapshot.local_constants.iter().any(|candidate| {
            candidate.observed_value == "OTHER_ERROR"
                && candidate.classification
                    == ManagedDeclarationClassification::LocalImplementationConstant
        }));

        let declaration_id =
            ManagedDeclarationId::parse("star.error.management-store-unavailable").unwrap();
        let description_intent = build_change_intent(
            &classified.snapshot,
            Some(&declaration_id),
            ManagedDeclarationChangeKind::UpdateDescription,
            ManagedDesiredFields::UpdateDescription {
                description: "Stable management storage availability error code".to_owned(),
            },
            "separate display text from the stable error code".to_owned(),
            vec![],
        )
        .unwrap();
        let description_rewrite =
            prepare_registry_change_rewrite(&root, &classified.snapshot, &description_intent)
                .unwrap();
        fs::write(
            root.join(description_rewrite[0].path.as_str()),
            &description_rewrite[0].after_bytes,
        )
        .unwrap();
        let updated = load_git_registry_from_project(&root, &path, classified_input).unwrap();
        assert_eq!(
            updated.snapshot.declarations[0].description,
            "Stable management storage availability error code"
        );

        fs::write(
            root.join("src/errors.rs"),
            "pub const CODE: &str = \"DRIFTED_VALUE\";\n",
        )
        .unwrap();
        let drifted = load_git_registry_from_project(&root, &path, input).unwrap();
        assert_eq!(drifted.snapshot.completeness, EvidenceCompleteness::Partial);
        assert!(
            drifted
                .consistency_records
                .iter()
                .any(|record| { record.status == RegistryConsistencyStatus::BindingDrift })
        );
    }

    fn git_ok(root: &Path, args: &[&str]) {
        assert!(
            Command::new("git")
                .current_dir(root)
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    }
}
