use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    process::Command,
};

use star_contracts::{
    ProjectId, Sha256Hash, canonical_sha256,
    development_v2::{
        CLEAN_ROOM_SPECIFICATION_SCHEMA_ID, COMPATIBILITY_REPORT_V2_SCHEMA_ID,
        CONFIG_KEY_TRACE_SCHEMA_ID, CONTRACT_SURFACE_SNAPSHOT_SCHEMA_ID, CleanRoomReadiness,
        CleanRoomSpecification, CompatibilityClass, CompatibilityReportV2, ConfigKeyTrace,
        ConfigOverrideObservation, ConfigReaderObservation, ConstraintEvaluation,
        ConsumerImpactRecord, ContractChangeRecord, ContractSurfaceDescriptor, ContractSurfaceKind,
        ContractSurfaceSnapshot, CoverageState, DEPENDENCY_SECURITY_INPUT_MANIFEST_SCHEMA_ID,
        DOCUMENTATION_SNAPSHOT_SCHEMA_ID, DependencySecurityInputManifest,
        DocumentationObservation, DocumentationObservationKind, DocumentationSnapshot,
        DocumentationTarget, ENVIRONMENT_SNAPSHOT_SCHEMA_ID, EnvironmentConstraint,
        EnvironmentSnapshot, EvaluationState, ManifestObservation, ObservationState,
        PROJECT_CONTRACT_MANIFEST_SCHEMA_ID, PROJECT_DOCTOR_REPORT_SCHEMA_ID,
        ProjectContractManifest, ProjectDoctorReport, SurfaceChangeKind, SurfaceObservation,
        SurfaceSnapshotRole, ToolchainObservation,
    },
};

use crate::{DevelopmentError, fingerprint, placeholder, safe_relative_path, token};

#[derive(Clone, Debug)]
pub struct SurfaceSourceInput {
    pub descriptor: ContractSurfaceDescriptor,
    pub bytes: Vec<u8>,
    pub coverage: CoverageState,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct DocumentationSourceInput {
    pub target: DocumentationTarget,
    pub bytes: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct ConfigReaderInput {
    pub reader_ref: String,
    pub source_path: String,
    pub bytes: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct EnvironmentProbeInput {
    pub snapshot_id: String,
    pub project_id: ProjectId,
    pub subject_revision: String,
    pub os_family: String,
    pub os_release: String,
    pub architecture: String,
    pub filesystem_kind: String,
    pub case_behavior: String,
    pub symlink_capability: String,
    pub long_path_capability: String,
    pub path_kind: String,
    pub path_depth: u32,
    pub path_length_bucket: String,
    pub text_encoding_policy: String,
    pub line_ending_policy: String,
    pub toolchains: Vec<ToolchainObservation>,
    pub manifests: Vec<ManifestObservation>,
    pub task_descriptor_refs: Vec<String>,
    pub environment_contract_presence:
        Vec<star_contracts::development_v2::EnvironmentContractPresence>,
    pub completeness: CoverageState,
    pub limitations: Vec<String>,
}

pub fn read_worktree_surface_sources(
    project_root: &Path,
    manifest: &ProjectContractManifest,
) -> Result<Vec<SurfaceSourceInput>, DevelopmentError> {
    let canonical_root = project_root
        .canonicalize()
        .map_err(|_| DevelopmentError::Adapter)?;
    manifest
        .surfaces
        .iter()
        .map(|descriptor| {
            let path = confined_source_path(&canonical_root, &descriptor.source_path)?;
            let bytes = std::fs::read(&path).map_err(|_| {
                if descriptor.required {
                    DevelopmentError::Adapter
                } else {
                    DevelopmentError::Unverified
                }
            })?;
            if bytes.len() > 16 * 1024 * 1024 {
                return Err(DevelopmentError::Blocked);
            }
            Ok(SurfaceSourceInput {
                descriptor: descriptor.clone(),
                bytes,
                coverage: CoverageState::Complete,
                limitations: Vec::new(),
            })
        })
        .collect()
}

pub fn read_git_surface_sources(
    project_root: &Path,
    revision: &str,
    manifest: &ProjectContractManifest,
) -> Result<(String, Vec<SurfaceSourceInput>), DevelopmentError> {
    let canonical_root = project_root
        .canonicalize()
        .map_err(|_| DevelopmentError::Adapter)?;
    let revision = resolve_git_commit(&canonical_root, revision)?;
    let sources = manifest
        .surfaces
        .iter()
        .map(|descriptor| {
            if !safe_relative_path(&descriptor.source_path) {
                return Err(DevelopmentError::Invalid);
            }
            let object = format!("{revision}:{}", descriptor.source_path);
            let output = Command::new("git")
                .args(["-C"])
                .arg(&canonical_root)
                .args(["show", "--no-textconv", &object])
                .output()
                .map_err(|_| DevelopmentError::Adapter)?;
            if !output.status.success() {
                return Err(if descriptor.required {
                    DevelopmentError::Conflict
                } else {
                    DevelopmentError::Unverified
                });
            }
            if output.stdout.len() > 16 * 1024 * 1024 {
                return Err(DevelopmentError::Blocked);
            }
            Ok(SurfaceSourceInput {
                descriptor: descriptor.clone(),
                bytes: output.stdout,
                coverage: CoverageState::Complete,
                limitations: Vec::new(),
            })
        })
        .collect::<Result<Vec<_>, DevelopmentError>>()?;
    Ok((revision, sources))
}

pub fn read_documentation_sources(
    project_root: &Path,
    targets: &[DocumentationTarget],
) -> Result<Vec<DocumentationSourceInput>, DevelopmentError> {
    let canonical_root = project_root
        .canonicalize()
        .map_err(|_| DevelopmentError::Adapter)?;
    targets
        .iter()
        .map(|target| {
            let path = confined_source_path(&canonical_root, &target.source_path)?;
            let bytes = match std::fs::read(path) {
                Ok(bytes) if bytes.len() <= 16 * 1024 * 1024 => Some(bytes),
                Ok(_) => return Err(DevelopmentError::Blocked),
                Err(_) => None,
            };
            Ok(DocumentationSourceInput {
                target: target.clone(),
                bytes,
            })
        })
        .collect()
}

pub fn parse_project_contract_manifest(
    bytes: &[u8],
) -> Result<ProjectContractManifest, DevelopmentError> {
    let text = std::str::from_utf8(bytes).map_err(|_| DevelopmentError::Invalid)?;
    let mut manifest: ProjectContractManifest =
        toml::from_str(text).map_err(|_| DevelopmentError::Invalid)?;
    validate_project_contract_manifest(&manifest)?;
    manifest.source_fingerprint = Sha256Hash::digest(bytes);
    Ok(manifest)
}

pub fn validate_project_contract_manifest(
    manifest: &ProjectContractManifest,
) -> Result<(), DevelopmentError> {
    if manifest.schema_id != PROJECT_CONTRACT_MANIFEST_SCHEMA_ID
        || manifest.schema_version != 1
        || !token(&manifest.manifest_id, 128)
        || manifest.manifest_version.trim().is_empty()
        || manifest.baseline_policy.baseline_ref.trim().is_empty()
        || manifest.baseline_policy.approval_ref.trim().is_empty()
        || manifest.surfaces.is_empty()
    {
        return Err(DevelopmentError::Invalid);
    }
    let mut surface_ids = BTreeSet::new();
    for surface in &manifest.surfaces {
        if !token(&surface.surface_id, 128)
            || !safe_relative_path(&surface.source_path)
            || surface.owner.trim().is_empty()
            || surface.source_selector.trim().is_empty()
            || surface.compatibility_policy_ref.trim().is_empty()
            || surface.visibility_policy.trim().is_empty()
            || !surface_ids.insert(surface.surface_id.as_str())
        {
            return Err(DevelopmentError::Invalid);
        }
    }
    let mut documentation_ids = BTreeSet::new();
    if manifest.documentation.iter().any(|target| {
        !token(&target.target_id, 128)
            || !safe_relative_path(&target.source_path)
            || !documentation_ids.insert(target.target_id.as_str())
    }) {
        return Err(DevelopmentError::Invalid);
    }
    let mut assumption_ids = BTreeSet::new();
    if manifest.assumptions.iter().any(|assumption| {
        !token(&assumption.assumption_id, 128)
            || assumption.subject.trim().is_empty()
            || assumption.expected.trim().is_empty()
            || !assumption_ids.insert(assumption.assumption_id.as_str())
    }) {
        return Err(DevelopmentError::Invalid);
    }
    let mut constraint_ids = BTreeSet::new();
    if manifest.environment_constraints.iter().any(|constraint| {
        !token(&constraint.constraint_id, 128)
            || constraint.kind.trim().is_empty()
            || constraint.subject.trim().is_empty()
            || constraint.accepted.is_empty()
            || !constraint_ids.insert(constraint.constraint_id.as_str())
    }) {
        return Err(DevelopmentError::Invalid);
    }
    Ok(())
}

pub fn snapshot_contract_surfaces(
    manifest: &ProjectContractManifest,
    snapshot_id: String,
    role: SurfaceSnapshotRole,
    subject_revision: String,
    registry_snapshot_ref: Option<String>,
    mut sources: Vec<SurfaceSourceInput>,
) -> Result<ContractSurfaceSnapshot, DevelopmentError> {
    validate_project_contract_manifest(manifest)?;
    if !token(&snapshot_id, 192) || subject_revision.trim().is_empty() {
        return Err(DevelopmentError::Invalid);
    }
    sources.sort_by(|left, right| left.descriptor.surface_id.cmp(&right.descriptor.surface_id));
    if sources.len() != manifest.surfaces.len()
        || sources
            .windows(2)
            .any(|pair| pair[0].descriptor.surface_id == pair[1].descriptor.surface_id)
    {
        return Err(DevelopmentError::Invalid);
    }
    let expected = manifest
        .surfaces
        .iter()
        .map(|surface| (&surface.surface_id, surface))
        .collect::<BTreeMap<_, _>>();
    let mut observations = Vec::with_capacity(sources.len());
    let mut limitations = Vec::new();
    for source in sources {
        let Some(descriptor) = expected.get(&source.descriptor.surface_id) else {
            return Err(DevelopmentError::Invalid);
        };
        if *descriptor != &source.descriptor {
            return Err(DevelopmentError::Conflict);
        }
        let normalized_shape = normalize_surface(source.descriptor.kind, &source.bytes)?;
        let source_sha256 = Sha256Hash::digest(&source.bytes);
        let binding_refs = surface_binding_refs(&source.descriptor);
        let mut observation = SurfaceObservation {
            surface_id: source.descriptor.surface_id,
            kind: source.descriptor.kind,
            normalized_shape,
            visibility: source.descriptor.visibility_policy,
            source_path: source.descriptor.source_path,
            source_sha256,
            binding_refs,
            evidence_refs: Vec::new(),
            coverage: source.coverage,
            limitations: source.limitations,
            observation_fingerprint: placeholder(),
        };
        observation.observation_fingerprint = fingerprint(
            "star.surface-observation",
            &serde_json::json!({
                "surface_id": observation.surface_id,
                "kind": observation.kind,
                "normalized_shape": observation.normalized_shape,
                "visibility": observation.visibility,
                "source_path": observation.source_path,
                "source_sha256": observation.source_sha256,
                "binding_refs": observation.binding_refs,
                "coverage": observation.coverage,
                "limitations": observation.limitations,
            }),
        )?;
        limitations.extend(observation.limitations.iter().cloned());
        observations.push(observation);
    }
    limitations.sort();
    limitations.dedup();
    let coverage = observations.iter().map(|item| item.coverage).fold(
        CoverageState::Complete,
        |state, item| match (state, item) {
            (CoverageState::Unverified, _) | (_, CoverageState::Unverified) => {
                CoverageState::Unverified
            }
            (CoverageState::Partial, _) | (_, CoverageState::Partial) => CoverageState::Partial,
            _ => CoverageState::Complete,
        },
    );
    let mut snapshot = ContractSurfaceSnapshot {
        schema_id: CONTRACT_SURFACE_SNAPSHOT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        snapshot_id,
        snapshot_role: role,
        project_id: manifest.project_id.clone(),
        subject_revision,
        manifest_fingerprint: manifest.source_fingerprint.clone(),
        registry_snapshot_ref,
        surfaces: observations,
        coverage,
        limitations,
        content_fingerprint: placeholder(),
    };
    snapshot.content_fingerprint = fingerprint(
        CONTRACT_SURFACE_SNAPSHOT_SCHEMA_ID,
        &serde_json::json!({
            "snapshot_id": snapshot.snapshot_id,
            "snapshot_role": snapshot.snapshot_role,
            "project_id": snapshot.project_id,
            "subject_revision": snapshot.subject_revision,
            "manifest_fingerprint": snapshot.manifest_fingerprint,
            "registry_snapshot_ref": snapshot.registry_snapshot_ref,
            "surfaces": snapshot.surfaces,
            "coverage": snapshot.coverage,
            "limitations": snapshot.limitations,
        }),
    )?;
    Ok(snapshot)
}

pub fn compare_surface_snapshots(
    manifest: &ProjectContractManifest,
    report_id: String,
    baseline: &ContractSurfaceSnapshot,
    current: &ContractSurfaceSnapshot,
) -> Result<CompatibilityReportV2, DevelopmentError> {
    validate_project_contract_manifest(manifest)?;
    if !token(&report_id, 192)
        || baseline.snapshot_role != SurfaceSnapshotRole::Baseline
        || current.snapshot_role != SurfaceSnapshotRole::Current
        || baseline.project_id != manifest.project_id
        || current.project_id != manifest.project_id
        || baseline.manifest_fingerprint != manifest.source_fingerprint
        || current.manifest_fingerprint != manifest.source_fingerprint
    {
        return Err(DevelopmentError::Invalid);
    }
    let before = baseline
        .surfaces
        .iter()
        .map(|surface| (surface.surface_id.as_str(), surface))
        .collect::<BTreeMap<_, _>>();
    let after = current
        .surfaces
        .iter()
        .map(|surface| (surface.surface_id.as_str(), surface))
        .collect::<BTreeMap<_, _>>();
    let mut ids = before
        .keys()
        .chain(after.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    ids.extend(
        manifest
            .surfaces
            .iter()
            .map(|surface| surface.surface_id.as_str()),
    );
    let mut changes = Vec::new();
    let mut consumer_impacts = Vec::new();
    for surface_id in ids {
        let descriptor = manifest
            .surfaces
            .iter()
            .find(|surface| surface.surface_id == surface_id);
        let old = before.get(surface_id).copied();
        let new = after.get(surface_id).copied();
        let kind = new
            .map(|item| item.kind)
            .or_else(|| old.map(|item| item.kind))
            .or_else(|| descriptor.map(|item| item.kind))
            .ok_or(DevelopmentError::Invalid)?;
        let (change_kind, classification, rule_id, summary) = classify_surface_change(old, new);
        changes.push(ContractChangeRecord {
            surface_id: surface_id.to_owned(),
            kind,
            change_kind,
            classification,
            before_fingerprint: old.map(|item| item.observation_fingerprint.clone()),
            after_fingerprint: new.map(|item| item.observation_fingerprint.clone()),
            rule_id: rule_id.to_owned(),
            summary: summary.to_owned(),
            evidence_refs: Vec::new(),
        });
        if let Some(descriptor) = descriptor {
            for consumer_ref in &descriptor.consumer_contract_refs {
                consumer_impacts.push(ConsumerImpactRecord {
                    consumer_ref: consumer_ref.clone(),
                    surface_id: surface_id.to_owned(),
                    observed_revision: None,
                    minimum_version: manifest.baseline_policy.minimum_consumer_version.clone(),
                    classification,
                    migration_required: classification == CompatibilityClass::Breaking,
                    state: match classification {
                        CompatibilityClass::Breaking => EvaluationState::Block,
                        CompatibilityClass::Unknown => EvaluationState::Unknown,
                        CompatibilityClass::Additive => EvaluationState::HumanReview,
                        _ => EvaluationState::Pass,
                    },
                    limitations: if classification == CompatibilityClass::Breaking {
                        vec!["consumer revision was not independently observed".to_owned()]
                    } else {
                        Vec::new()
                    },
                });
            }
        }
    }
    changes.sort_by(|left, right| left.surface_id.cmp(&right.surface_id));
    consumer_impacts.sort_by(|left, right| {
        (&left.surface_id, &left.consumer_ref).cmp(&(&right.surface_id, &right.consumer_ref))
    });
    let outcome = aggregate_compatibility(changes.iter().map(|change| change.classification));
    let completeness = match (baseline.coverage, current.coverage) {
        (CoverageState::Unverified, _) | (_, CoverageState::Unverified) => {
            CoverageState::Unverified
        }
        (CoverageState::Partial, _) | (_, CoverageState::Partial) => CoverageState::Partial,
        _ if outcome == CompatibilityClass::Unknown => CoverageState::Partial,
        _ => CoverageState::Complete,
    };
    let mut limitations = baseline.limitations.clone();
    limitations.extend(current.limitations.iter().cloned());
    limitations.sort();
    limitations.dedup();
    let mut report = CompatibilityReportV2 {
        schema_id: COMPATIBILITY_REPORT_V2_SCHEMA_ID.to_owned(),
        schema_version: 2,
        report_id,
        project_id: manifest.project_id.clone(),
        manifest_ref: manifest.manifest_id.clone(),
        manifest_fingerprint: manifest.source_fingerprint.clone(),
        baseline_snapshot_ref: baseline.snapshot_id.clone(),
        baseline_snapshot_fingerprint: baseline.content_fingerprint.clone(),
        current_snapshot_ref: current.snapshot_id.clone(),
        current_snapshot_fingerprint: current.content_fingerprint.clone(),
        changes,
        consumer_impacts,
        outcome,
        completeness,
        limitations,
        report_fingerprint: placeholder(),
    };
    report.report_fingerprint = fingerprint(
        COMPATIBILITY_REPORT_V2_SCHEMA_ID,
        &serde_json::json!({
            "report_id": report.report_id,
            "project_id": report.project_id,
            "manifest_ref": report.manifest_ref,
            "manifest_fingerprint": report.manifest_fingerprint,
            "baseline_snapshot_ref": report.baseline_snapshot_ref,
            "baseline_snapshot_fingerprint": report.baseline_snapshot_fingerprint,
            "current_snapshot_ref": report.current_snapshot_ref,
            "current_snapshot_fingerprint": report.current_snapshot_fingerprint,
            "changes": report.changes,
            "consumer_impacts": report.consumer_impacts,
            "outcome": report.outcome,
            "completeness": report.completeness,
            "limitations": report.limitations,
        }),
    )?;
    Ok(report)
}

pub fn build_documentation_snapshot(
    snapshot_id: String,
    project_id: ProjectId,
    subject_revision: String,
    mut sources: Vec<DocumentationSourceInput>,
    registered_commands: &BTreeSet<String>,
    registered_config_keys: &BTreeSet<String>,
) -> Result<DocumentationSnapshot, DevelopmentError> {
    if !token(&snapshot_id, 192) || subject_revision.trim().is_empty() {
        return Err(DevelopmentError::Invalid);
    }
    sources.sort_by(|left, right| left.target.target_id.cmp(&right.target.target_id));
    if sources
        .windows(2)
        .any(|pair| pair[0].target.target_id == pair[1].target.target_id)
    {
        return Err(DevelopmentError::Invalid);
    }
    let mut observations = Vec::new();
    let mut limitations = Vec::new();
    let mut completeness = CoverageState::Complete;
    for source in sources {
        if !token(&source.target.target_id, 128) || !safe_relative_path(&source.target.source_path)
        {
            return Err(DevelopmentError::Invalid);
        }
        let Some(bytes) = source.bytes else {
            completeness = CoverageState::Partial;
            limitations.push(format!(
                "missing documentation source: {}",
                source.target.source_path
            ));
            for subject in source
                .target
                .required_commands
                .iter()
                .chain(source.target.required_config_keys.iter())
                .chain(source.target.required_references.iter())
            {
                observations.push(documentation_observation(
                    &source.target,
                    DocumentationObservationKind::Reference,
                    subject,
                    EvaluationState::Block,
                    &[],
                    "required documentation source is missing",
                ));
            }
            continue;
        };
        let text = std::str::from_utf8(&bytes).map_err(|_| DevelopmentError::Invalid)?;
        let tokens = documentation_tokens(text);
        for command in &source.target.required_commands {
            let documented = tokens.contains(&format!("code:{command}")) || text.contains(command);
            let registered = registered_commands.contains(command);
            observations.push(documentation_observation(
                &source.target,
                DocumentationObservationKind::Command,
                command,
                if documented && registered {
                    EvaluationState::Pass
                } else {
                    EvaluationState::Block
                },
                &bytes,
                if !documented {
                    "required command is not documented"
                } else if !registered {
                    "documented command is not registered"
                } else {
                    "documented command is registered"
                },
            ));
        }
        for key in &source.target.required_config_keys {
            let documented = tokens.contains(&format!("code:{key}")) || text.contains(key);
            let registered = registered_config_keys.contains(key);
            observations.push(documentation_observation(
                &source.target,
                DocumentationObservationKind::ConfigKey,
                key,
                if documented && registered {
                    EvaluationState::Pass
                } else {
                    EvaluationState::Block
                },
                &bytes,
                if !documented {
                    "required config key is not documented"
                } else if !registered {
                    "documented config key is not registered"
                } else {
                    "documented config key is registered"
                },
            ));
        }
        for reference in &source.target.required_references {
            let documented =
                tokens.iter().any(|token| token.ends_with(reference)) || text.contains(reference);
            observations.push(documentation_observation(
                &source.target,
                DocumentationObservationKind::Reference,
                reference,
                if documented {
                    EvaluationState::Pass
                } else {
                    EvaluationState::Block
                },
                &bytes,
                if documented {
                    "required reference is documented"
                } else {
                    "required reference is missing"
                },
            ));
        }
        for token in tokens {
            let (kind, subject) = if let Some(value) = token.strip_prefix("anchor:") {
                (DocumentationObservationKind::Anchor, value)
            } else if let Some(value) = token.strip_prefix("link:") {
                (DocumentationObservationKind::Link, value)
            } else if let Some(value) = token.strip_prefix("code:") {
                (DocumentationObservationKind::Snippet, value)
            } else {
                continue;
            };
            observations.push(documentation_observation(
                &source.target,
                kind,
                subject,
                if kind == DocumentationObservationKind::Link
                    && (subject.starts_with("http://") || subject.starts_with("https://"))
                {
                    EvaluationState::HumanReview
                } else {
                    EvaluationState::Pass
                },
                &bytes,
                "documentation token observed",
            ));
        }
    }
    observations.sort_by(|left, right| {
        (&left.target_id, left.kind, &left.subject).cmp(&(
            &right.target_id,
            right.kind,
            &right.subject,
        ))
    });
    observations.dedup_by(|left, right| {
        left.target_id == right.target_id
            && left.kind == right.kind
            && left.subject == right.subject
    });
    limitations.sort();
    limitations.dedup();
    let mut snapshot = DocumentationSnapshot {
        schema_id: DOCUMENTATION_SNAPSHOT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        snapshot_id,
        project_id,
        subject_revision,
        observations,
        completeness,
        limitations,
        content_fingerprint: placeholder(),
    };
    snapshot.content_fingerprint = fingerprint(
        DOCUMENTATION_SNAPSHOT_SCHEMA_ID,
        &serde_json::json!({
            "snapshot_id": snapshot.snapshot_id,
            "project_id": snapshot.project_id,
            "subject_revision": snapshot.subject_revision,
            "observations": snapshot.observations,
            "completeness": snapshot.completeness,
            "limitations": snapshot.limitations,
        }),
    )?;
    Ok(snapshot)
}

#[allow(clippy::too_many_arguments)]
pub fn trace_config_key(
    trace_id: String,
    project_id: ProjectId,
    key_ref: String,
    lifecycle: String,
    declaration_ref: Option<String>,
    mut readers: Vec<ConfigReaderInput>,
    mut overrides: Vec<ConfigOverrideObservation>,
) -> Result<ConfigKeyTrace, DevelopmentError> {
    if !token(&trace_id, 192)
        || key_ref.trim().is_empty()
        || lifecycle.trim().is_empty()
        || overrides
            .windows(2)
            .any(|pair| pair[0].precedence == pair[1].precedence)
    {
        return Err(DevelopmentError::Invalid);
    }
    readers.sort_by(|left, right| {
        (&left.reader_ref, &left.source_path).cmp(&(&right.reader_ref, &right.source_path))
    });
    overrides.sort_by_key(|item| item.precedence);
    if overrides
        .windows(2)
        .any(|pair| pair[0].precedence == pair[1].precedence)
    {
        return Err(DevelopmentError::Invalid);
    }
    let mut limitations = Vec::new();
    let reader_observations = readers
        .into_iter()
        .map(|reader| {
            if !safe_relative_path(&reader.source_path) || reader.reader_ref.trim().is_empty() {
                return Err(DevelopmentError::Invalid);
            }
            let (source_sha256, state) = match reader.bytes {
                Some(bytes) => {
                    let contains = std::str::from_utf8(&bytes)
                        .map_err(|_| DevelopmentError::Invalid)?
                        .contains(&key_ref);
                    (
                        Sha256Hash::digest(&bytes),
                        if contains {
                            EvaluationState::Pass
                        } else {
                            EvaluationState::Block
                        },
                    )
                }
                None => {
                    limitations.push(format!("reader source missing: {}", reader.source_path));
                    (Sha256Hash::digest(b"missing"), EvaluationState::Unknown)
                }
            };
            Ok(ConfigReaderObservation {
                reader_ref: reader.reader_ref,
                source_path: reader.source_path,
                source_sha256,
                state,
            })
        })
        .collect::<Result<Vec<_>, DevelopmentError>>()?;
    let effective_provenance = overrides
        .iter()
        .filter(|item| item.present)
        .max_by_key(|item| item.precedence)
        .map(|item| item.provenance.clone());
    let state = if reader_observations
        .iter()
        .any(|reader| reader.state == EvaluationState::Unknown)
    {
        EvaluationState::Unknown
    } else if reader_observations.is_empty()
        || reader_observations
            .iter()
            .any(|reader| reader.state == EvaluationState::Block)
    {
        EvaluationState::Block
    } else {
        EvaluationState::Pass
    };
    let mut trace = ConfigKeyTrace {
        schema_id: CONFIG_KEY_TRACE_SCHEMA_ID.to_owned(),
        schema_version: 1,
        trace_id,
        project_id,
        key_ref,
        lifecycle,
        declaration_ref,
        readers: reader_observations,
        overrides,
        effective_provenance,
        value_redacted: true,
        state,
        limitations,
        trace_fingerprint: placeholder(),
    };
    trace.trace_fingerprint = fingerprint(
        CONFIG_KEY_TRACE_SCHEMA_ID,
        &serde_json::json!({
            "trace_id": trace.trace_id,
            "project_id": trace.project_id,
            "key_ref": trace.key_ref,
            "lifecycle": trace.lifecycle,
            "declaration_ref": trace.declaration_ref,
            "readers": trace.readers,
            "overrides": trace.overrides,
            "effective_provenance": trace.effective_provenance,
            "value_redacted": trace.value_redacted,
            "state": trace.state,
            "limitations": trace.limitations,
        }),
    )?;
    Ok(trace)
}

pub fn build_environment_snapshot(
    mut input: EnvironmentProbeInput,
) -> Result<EnvironmentSnapshot, DevelopmentError> {
    if !token(&input.snapshot_id, 192)
        || input.subject_revision.trim().is_empty()
        || input.os_family.trim().is_empty()
        || input.architecture.trim().is_empty()
        || input.path_kind.trim().is_empty()
    {
        return Err(DevelopmentError::Invalid);
    }
    input
        .toolchains
        .sort_by(|left, right| left.toolchain_id.cmp(&right.toolchain_id));
    input
        .manifests
        .sort_by(|left, right| left.logical_path.cmp(&right.logical_path));
    input.task_descriptor_refs.sort();
    input.task_descriptor_refs.dedup();
    input
        .environment_contract_presence
        .sort_by(|left, right| left.declaration_ref.cmp(&right.declaration_ref));
    input.limitations.sort();
    input.limitations.dedup();
    let fingerprint_value = serde_json::json!({
        "project_id": input.project_id,
        "subject_revision": input.subject_revision,
        "os_family": input.os_family,
        "os_release": input.os_release,
        "architecture": input.architecture,
        "filesystem_kind": input.filesystem_kind,
        "case_behavior": input.case_behavior,
        "symlink_capability": input.symlink_capability,
        "long_path_capability": input.long_path_capability,
        "path_kind": input.path_kind,
        "path_depth": input.path_depth,
        "path_length_bucket": input.path_length_bucket,
        "text_encoding_policy": input.text_encoding_policy,
        "line_ending_policy": input.line_ending_policy,
        "toolchains": input.toolchains,
        "manifests": input.manifests,
        "task_descriptor_refs": input.task_descriptor_refs,
        "environment_contract_presence": input.environment_contract_presence,
        "completeness": input.completeness,
        "limitations": input.limitations,
    });
    let environment_fingerprint =
        canonical_sha256(&fingerprint_value).map_err(|_| DevelopmentError::Fingerprint)?;
    Ok(EnvironmentSnapshot {
        schema_id: ENVIRONMENT_SNAPSHOT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        snapshot_id: input.snapshot_id,
        project_id: input.project_id,
        subject_revision: input.subject_revision,
        os_family: input.os_family,
        os_release: input.os_release,
        architecture: input.architecture,
        filesystem_kind: input.filesystem_kind,
        case_behavior: input.case_behavior,
        symlink_capability: input.symlink_capability,
        long_path_capability: input.long_path_capability,
        path_kind: input.path_kind,
        path_depth: input.path_depth,
        path_length_bucket: input.path_length_bucket,
        text_encoding_policy: input.text_encoding_policy,
        line_ending_policy: input.line_ending_policy,
        toolchains: input.toolchains,
        manifests: input.manifests,
        task_descriptor_refs: input.task_descriptor_refs,
        environment_contract_presence: input.environment_contract_presence,
        completeness: input.completeness,
        limitations: input.limitations,
        environment_fingerprint,
    })
}

pub fn evaluate_project_doctor(
    report_id: String,
    manifest: &ProjectContractManifest,
    environment: &EnvironmentSnapshot,
    clean_room_specification: Option<&CleanRoomSpecification>,
    registered_tasks: &BTreeSet<String>,
) -> Result<ProjectDoctorReport, DevelopmentError> {
    if !token(&report_id, 192)
        || environment.project_id != manifest.project_id
        || environment.schema_id != ENVIRONMENT_SNAPSHOT_SCHEMA_ID
    {
        return Err(DevelopmentError::Invalid);
    }
    let mut evaluations = manifest
        .environment_constraints
        .iter()
        .map(|constraint| evaluate_constraint(constraint, environment))
        .collect::<Vec<_>>();
    evaluations.sort_by(|left, right| left.constraint_id.cmp(&right.constraint_id));
    let clean_room_readiness =
        evaluate_clean_room_readiness(clean_room_specification, environment, registered_tasks)?;
    let mut diagnostics = evaluations
        .iter()
        .filter_map(|item| item.diagnostic_code.clone())
        .collect::<Vec<_>>();
    if clean_room_readiness == CleanRoomReadiness::NotReady {
        diagnostics.push("CLEAN_ROOM_NOT_READY".to_owned());
    } else if clean_room_readiness == CleanRoomReadiness::Unknown {
        diagnostics.push("CLEAN_ROOM_READINESS_UNKNOWN".to_owned());
    }
    diagnostics.sort();
    diagnostics.dedup();
    let state = if evaluations
        .iter()
        .any(|item| item.state == EvaluationState::Block)
        || clean_room_readiness == CleanRoomReadiness::NotReady
    {
        EvaluationState::Block
    } else if evaluations
        .iter()
        .any(|item| item.state == EvaluationState::Unknown)
        || clean_room_readiness == CleanRoomReadiness::Unknown
    {
        EvaluationState::Unknown
    } else {
        EvaluationState::Pass
    };
    let completeness = if environment.completeness == CoverageState::Complete
        && state != EvaluationState::Unknown
    {
        CoverageState::Complete
    } else if environment.completeness == CoverageState::Unverified {
        CoverageState::Unverified
    } else {
        CoverageState::Partial
    };
    let mut report = ProjectDoctorReport {
        schema_id: PROJECT_DOCTOR_REPORT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        report_id,
        project_id: manifest.project_id.clone(),
        subject_revision: environment.subject_revision.clone(),
        environment_snapshot_ref: environment.snapshot_id.clone(),
        environment_snapshot_fingerprint: environment.environment_fingerprint.clone(),
        constraint_evaluations: evaluations,
        toolchain_observations: environment.toolchains.clone(),
        manifest_observations: environment.manifests.clone(),
        command_availability: Vec::new(),
        windows_compatibility: Vec::new(),
        clean_room_readiness,
        diagnostics,
        forbidden_actions_observed: Vec::new(),
        completeness,
        limitations: environment.limitations.clone(),
        state,
        report_fingerprint: placeholder(),
    };
    report.report_fingerprint = fingerprint(
        PROJECT_DOCTOR_REPORT_SCHEMA_ID,
        &serde_json::json!({
            "report_id": report.report_id,
            "project_id": report.project_id,
            "subject_revision": report.subject_revision,
            "environment_snapshot_ref": report.environment_snapshot_ref,
            "environment_snapshot_fingerprint": report.environment_snapshot_fingerprint,
            "constraint_evaluations": report.constraint_evaluations,
            "toolchain_observations": report.toolchain_observations,
            "manifest_observations": report.manifest_observations,
            "command_availability": report.command_availability,
            "windows_compatibility": report.windows_compatibility,
            "clean_room_readiness": report.clean_room_readiness,
            "diagnostics": report.diagnostics,
            "forbidden_actions_observed": report.forbidden_actions_observed,
            "completeness": report.completeness,
            "limitations": report.limitations,
            "state": report.state,
        }),
    )?;
    Ok(report)
}

pub fn validate_clean_room_specification(
    specification: &CleanRoomSpecification,
) -> Result<(), DevelopmentError> {
    if specification.schema_id != CLEAN_ROOM_SPECIFICATION_SCHEMA_ID
        || specification.schema_version != 1
        || !token(&specification.specification_id, 192)
        || specification.source_revision.trim().is_empty()
        || specification.target_os.is_empty()
        || specification.architectures.is_empty()
        || specification.tasks.is_empty()
        || specification.dependency_download != "deny"
        || specification.package_install != "deny"
        || specification.system_mutation != "deny"
        || specification
            .tasks
            .iter()
            .any(|task| task.timeout_ms == 0 || task.task_id.trim().is_empty())
        || !specification
            .forbidden_actions
            .iter()
            .any(|item| item == "download")
        || !specification
            .forbidden_actions
            .iter()
            .any(|item| item == "install")
        || !specification
            .forbidden_actions
            .iter()
            .any(|item| item == "system_change")
    {
        return Err(DevelopmentError::Invalid);
    }
    let mut copy = specification.clone();
    copy.specification_fingerprint = placeholder();
    let expected = fingerprint(
        CLEAN_ROOM_SPECIFICATION_SCHEMA_ID,
        &serde_json::json!({
            "specification_id": copy.specification_id,
            "project_id": copy.project_id,
            "source_revision": copy.source_revision,
            "source_sha256": copy.source_sha256,
            "target_os": copy.target_os,
            "architectures": copy.architectures,
            "required_toolchains": copy.required_toolchains,
            "manifest_refs": copy.manifest_refs,
            "tasks": copy.tasks,
            "required_environment_contracts": copy.required_environment_contracts,
            "test_network_policy": copy.test_network_policy,
            "dependency_download": copy.dependency_download,
            "package_install": copy.package_install,
            "system_mutation": copy.system_mutation,
            "cache_state": copy.cache_state,
            "writable_output_roots": copy.writable_output_roots,
            "forbidden_actions": copy.forbidden_actions,
        }),
    )?;
    if specification.specification_fingerprint != expected {
        return Err(DevelopmentError::Conflict);
    }
    Ok(())
}

pub fn seal_clean_room_specification(
    mut specification: CleanRoomSpecification,
) -> Result<CleanRoomSpecification, DevelopmentError> {
    specification.specification_fingerprint = fingerprint(
        CLEAN_ROOM_SPECIFICATION_SCHEMA_ID,
        &serde_json::json!({
            "specification_id": specification.specification_id,
            "project_id": specification.project_id,
            "source_revision": specification.source_revision,
            "source_sha256": specification.source_sha256,
            "target_os": specification.target_os,
            "architectures": specification.architectures,
            "required_toolchains": specification.required_toolchains,
            "manifest_refs": specification.manifest_refs,
            "tasks": specification.tasks,
            "required_environment_contracts": specification.required_environment_contracts,
            "test_network_policy": specification.test_network_policy,
            "dependency_download": specification.dependency_download,
            "package_install": specification.package_install,
            "system_mutation": specification.system_mutation,
            "cache_state": specification.cache_state,
            "writable_output_roots": specification.writable_output_roots,
            "forbidden_actions": specification.forbidden_actions,
        }),
    )?;
    validate_clean_room_specification(&specification)?;
    Ok(specification)
}

pub fn dependency_security_input_manifest(
    manifest_id: String,
    environment: &EnvironmentSnapshot,
) -> Result<DependencySecurityInputManifest, DevelopmentError> {
    if !token(&manifest_id, 192) {
        return Err(DevelopmentError::Invalid);
    }
    let mut output = DependencySecurityInputManifest {
        schema_id: DEPENDENCY_SECURITY_INPUT_MANIFEST_SCHEMA_ID.to_owned(),
        schema_version: 1,
        manifest_id,
        project_id: environment.project_id.clone(),
        subject_revision: environment.subject_revision.clone(),
        environment_snapshot_ref: environment.snapshot_id.clone(),
        manifest_observations: environment.manifests.clone(),
        toolchain_observations: environment.toolchains.clone(),
        completeness: environment.completeness,
        limitations: environment.limitations.clone(),
        content_fingerprint: placeholder(),
    };
    output.content_fingerprint = fingerprint(
        DEPENDENCY_SECURITY_INPUT_MANIFEST_SCHEMA_ID,
        &serde_json::json!({
            "manifest_id": output.manifest_id,
            "project_id": output.project_id,
            "subject_revision": output.subject_revision,
            "environment_snapshot_ref": output.environment_snapshot_ref,
            "manifest_observations": output.manifest_observations,
            "toolchain_observations": output.toolchain_observations,
            "completeness": output.completeness,
            "limitations": output.limitations,
        }),
    )?;
    Ok(output)
}

fn surface_binding_refs(descriptor: &ContractSurfaceDescriptor) -> Vec<String> {
    let mut refs = Vec::new();
    refs.extend(descriptor.declaration_ref.iter().cloned());
    refs.extend(descriptor.schema_ref.iter().cloned());
    refs.extend(descriptor.generated_refs.iter().cloned());
    refs.extend(descriptor.documentation_refs.iter().cloned());
    refs.extend(descriptor.consumer_contract_refs.iter().cloned());
    refs.sort();
    refs.dedup();
    refs
}

fn normalize_surface(kind: ContractSurfaceKind, bytes: &[u8]) -> Result<String, DevelopmentError> {
    let text = std::str::from_utf8(bytes).map_err(|_| DevelopmentError::Invalid)?;
    match kind {
        ContractSurfaceKind::Schema => {
            let value: serde_json::Value =
                serde_json::from_str(text).map_err(|_| DevelopmentError::Invalid)?;
            let hash = canonical_sha256(&value).map_err(|_| DevelopmentError::Fingerprint)?;
            Ok(format!("canonical-json:{hash}"))
        }
        ContractSurfaceKind::Config => {
            let value: toml::Value = toml::from_str(text).map_err(|_| DevelopmentError::Invalid)?;
            let mut output = BTreeMap::new();
            flatten_toml("", &value, &mut output);
            serde_json::to_string(&output).map_err(|_| DevelopmentError::Invalid)
        }
        ContractSurfaceKind::Api => normalize_lines(text, |line| {
            line.starts_with("pub ") || line.starts_with("export ")
        }),
        ContractSurfaceKind::Cli => {
            normalize_lines(text, |line| !line.starts_with('#') && !line.is_empty())
        }
        ContractSurfaceKind::ErrorCode => normalize_lines(text, |line| {
            line.bytes().any(|byte| byte.is_ascii_uppercase()) && line.contains('_')
        }),
        ContractSurfaceKind::FileFormat => normalize_lines(text, |line| !line.is_empty()),
    }
}

fn normalize_lines(text: &str, include: impl Fn(&str) -> bool) -> Result<String, DevelopmentError> {
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|line| include(line))
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>();
    lines.sort();
    lines.dedup();
    serde_json::to_string(&lines).map_err(|_| DevelopmentError::Invalid)
}

fn flatten_toml(prefix: &str, value: &toml::Value, output: &mut BTreeMap<String, String>) {
    if let toml::Value::Table(table) = value {
        for (key, value) in table {
            let next = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            flatten_toml(&next, value, output);
        }
    } else {
        output.insert(prefix.to_owned(), toml_kind(value).to_owned());
    }
}

fn toml_kind(value: &toml::Value) -> &'static str {
    match value {
        toml::Value::String(_) => "string",
        toml::Value::Integer(_) => "integer",
        toml::Value::Float(_) => "float",
        toml::Value::Boolean(_) => "boolean",
        toml::Value::Datetime(_) => "datetime",
        toml::Value::Array(_) => "array",
        toml::Value::Table(_) => "table",
    }
}

fn classify_surface_change(
    old: Option<&SurfaceObservation>,
    new: Option<&SurfaceObservation>,
) -> (
    SurfaceChangeKind,
    CompatibilityClass,
    &'static str,
    &'static str,
) {
    match (old, new) {
        (None, Some(new)) if new.coverage == CoverageState::Complete => (
            SurfaceChangeKind::Added,
            CompatibilityClass::Additive,
            "surface.added.v1",
            "new declared public surface was added",
        ),
        (Some(old), None) if old.coverage == CoverageState::Complete => (
            SurfaceChangeKind::Removed,
            CompatibilityClass::Breaking,
            "surface.removed.v1",
            "declared public surface was removed",
        ),
        (Some(old), Some(new))
            if old.coverage != CoverageState::Complete
                || new.coverage != CoverageState::Complete =>
        {
            (
                SurfaceChangeKind::CoverageChanged,
                CompatibilityClass::Unknown,
                "surface.coverage-incomplete.v1",
                "surface coverage is partial or unverified",
            )
        }
        (Some(old), Some(new)) if old.observation_fingerprint == new.observation_fingerprint => (
            SurfaceChangeKind::Unchanged,
            CompatibilityClass::Unchanged,
            "surface.identical.v1",
            "canonical surface is unchanged",
        ),
        (Some(_), Some(_)) => (
            SurfaceChangeKind::Modified,
            CompatibilityClass::Breaking,
            "surface.modified-conservative.v1",
            "canonical public surface changed and no kind-specific compatibility proof was supplied",
        ),
        _ => (
            SurfaceChangeKind::CoverageChanged,
            CompatibilityClass::Unknown,
            "surface.missing.v1",
            "required surface observation is missing",
        ),
    }
}

fn aggregate_compatibility(values: impl Iterator<Item = CompatibilityClass>) -> CompatibilityClass {
    values.fold(CompatibilityClass::Unchanged, |state, item| {
        let rank = |value| match value {
            CompatibilityClass::Unchanged => 0,
            CompatibilityClass::Compatible => 1,
            CompatibilityClass::Additive => 2,
            CompatibilityClass::Unknown => 3,
            CompatibilityClass::Breaking => 4,
        };
        if rank(item) > rank(state) {
            item
        } else {
            state
        }
    })
}

fn documentation_tokens(text: &str) -> BTreeSet<String> {
    let mut output = BTreeSet::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            output.insert(format!(
                "anchor:{}",
                trimmed
                    .trim_start_matches('#')
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        for part in trimmed.split('`').skip(1).step_by(2) {
            if !part.trim().is_empty() {
                output.insert(format!("code:{}", part.trim()));
            }
        }
        for part in trimmed.split("](").skip(1) {
            if let Some(target) = part.split(')').next() {
                output.insert(format!("link:{target}"));
            }
        }
    }
    output
}

fn documentation_observation(
    target: &DocumentationTarget,
    kind: DocumentationObservationKind,
    subject: &str,
    state: EvaluationState,
    bytes: &[u8],
    summary: &str,
) -> DocumentationObservation {
    DocumentationObservation {
        target_id: target.target_id.clone(),
        kind,
        subject: subject.to_owned(),
        state,
        source_path: target.source_path.clone(),
        source_sha256: Sha256Hash::digest(bytes),
        summary: summary.to_owned(),
    }
}

fn evaluate_constraint(
    constraint: &EnvironmentConstraint,
    environment: &EnvironmentSnapshot,
) -> ConstraintEvaluation {
    let observed = match constraint.kind.as_str() {
        "os_family" => environment.os_family.clone(),
        "os_release" => environment.os_release.clone(),
        "architecture" => environment.architecture.clone(),
        "filesystem_kind" => environment.filesystem_kind.clone(),
        "case_behavior" => environment.case_behavior.clone(),
        "long_path_capability" => environment.long_path_capability.clone(),
        "text_encoding" => environment.text_encoding_policy.clone(),
        "line_ending" => environment.line_ending_policy.clone(),
        "toolchain" => environment
            .toolchains
            .iter()
            .find(|tool| tool.toolchain_id == constraint.subject)
            .and_then(|tool| tool.observed_version.clone())
            .unwrap_or_else(|| "not_observed".to_owned()),
        _ => "unknown_constraint_kind".to_owned(),
    };
    let unknown = observed == "not_observed" || observed == "unknown_constraint_kind";
    let accepted = constraint.accepted.iter().any(|value| value == &observed);
    let state = if unknown {
        EvaluationState::Unknown
    } else if accepted {
        EvaluationState::Pass
    } else if constraint.required {
        EvaluationState::Block
    } else {
        EvaluationState::HumanReview
    };
    ConstraintEvaluation {
        constraint_id: constraint.constraint_id.clone(),
        observed,
        required: constraint.accepted.join("|"),
        state,
        diagnostic_code: match state {
            EvaluationState::Block => Some("PROJECT_DOCTOR_CONSTRAINT_MISMATCH".to_owned()),
            EvaluationState::Unknown => Some("PROJECT_DOCTOR_CONSTRAINT_UNVERIFIED".to_owned()),
            _ => None,
        },
    }
}

fn evaluate_clean_room_readiness(
    specification: Option<&CleanRoomSpecification>,
    environment: &EnvironmentSnapshot,
    registered_tasks: &BTreeSet<String>,
) -> Result<CleanRoomReadiness, DevelopmentError> {
    let Some(specification) = specification else {
        return Ok(CleanRoomReadiness::NotRequired);
    };
    validate_clean_room_specification(specification)?;
    if specification.project_id != environment.project_id
        || !specification
            .target_os
            .iter()
            .any(|value| value == &environment.os_family)
        || !specification
            .architectures
            .iter()
            .any(|value| value == &environment.architecture)
        || specification
            .tasks
            .iter()
            .any(|task| !registered_tasks.contains(&task.task_id))
        || specification.required_toolchains.iter().any(|required| {
            !environment.toolchains.iter().any(|observed| {
                observed.toolchain_id == required.toolchain_id
                    && observed.state == ObservationState::Present
                    && required
                        .observed_version
                        .as_ref()
                        .is_none_or(|version| observed.observed_version.as_ref() == Some(version))
            })
        })
    {
        return Ok(CleanRoomReadiness::NotReady);
    }
    if environment.completeness != CoverageState::Complete {
        return Ok(CleanRoomReadiness::Unknown);
    }
    Ok(CleanRoomReadiness::Ready)
}

fn confined_source_path(
    canonical_root: &Path,
    logical_path: &str,
) -> Result<PathBuf, DevelopmentError> {
    if !safe_relative_path(logical_path) {
        return Err(DevelopmentError::Invalid);
    }
    let candidate = canonical_root.join(logical_path);
    if candidate.exists() {
        let canonical = candidate
            .canonicalize()
            .map_err(|_| DevelopmentError::Adapter)?;
        if !canonical.starts_with(canonical_root) {
            return Err(DevelopmentError::Blocked);
        }
        return Ok(canonical);
    }
    let mut ancestor = candidate.parent();
    while let Some(path) = ancestor {
        if path.exists() {
            let canonical = path.canonicalize().map_err(|_| DevelopmentError::Adapter)?;
            if !canonical.starts_with(canonical_root) {
                return Err(DevelopmentError::Blocked);
            }
            return Ok(candidate);
        }
        ancestor = path.parent();
    }
    Err(DevelopmentError::Blocked)
}

fn resolve_git_commit(project_root: &Path, revision: &str) -> Result<String, DevelopmentError> {
    if revision.trim().is_empty()
        || revision.len() > 256
        || revision.contains('\0')
        || revision.starts_with('-')
    {
        return Err(DevelopmentError::Invalid);
    }
    let verify = format!("{revision}^{{commit}}");
    let output = Command::new("git")
        .args(["-C"])
        .arg(project_root)
        .args(["rev-parse", "--verify", &verify])
        .output()
        .map_err(|_| DevelopmentError::Adapter)?;
    if !output.status.success() {
        return Err(DevelopmentError::Conflict);
    }
    let resolved = std::str::from_utf8(&output.stdout)
        .map_err(|_| DevelopmentError::Adapter)?
        .trim();
    if resolved.len() < 40 || !resolved.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(DevelopmentError::Adapter);
    }
    Ok(resolved.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::development_v2::{BaselinePolicy, ContractSurfaceDescriptor};

    fn manifest(project_id: ProjectId) -> ProjectContractManifest {
        ProjectContractManifest {
            schema_id: PROJECT_CONTRACT_MANIFEST_SCHEMA_ID.to_owned(),
            schema_version: 1,
            manifest_id: "contract-main".to_owned(),
            manifest_version: "1.0.0".to_owned(),
            project_id,
            baseline_policy: BaselinePolicy {
                baseline_ref: "git:0123456789abcdef".to_owned(),
                baseline_sha256: Sha256Hash::digest(b"baseline"),
                approval_ref: "approval:one".to_owned(),
                activated_at: "2026-07-23T00:00:00Z".to_owned(),
                supported_until: None,
                minimum_consumer_version: None,
            },
            surfaces: vec![ContractSurfaceDescriptor {
                surface_id: "api-main".to_owned(),
                kind: ContractSurfaceKind::Api,
                owner: "crate".to_owned(),
                source_path: "src/lib.rs".to_owned(),
                source_selector: "public_items".to_owned(),
                declaration_ref: None,
                schema_ref: None,
                generated_refs: Vec::new(),
                documentation_refs: Vec::new(),
                consumer_contract_refs: vec!["consumer:one".to_owned()],
                compatibility_policy_ref: "policy:semver".to_owned(),
                visibility_policy: "public".to_owned(),
                required: true,
            }],
            documentation: Vec::new(),
            assumptions: Vec::new(),
            environment_constraints: Vec::new(),
            clean_room_spec_ref: None,
            source_fingerprint: Sha256Hash::digest(b"manifest"),
        }
    }

    #[test]
    fn snapshot_compare_marks_removed_or_modified_public_surface_breaking() {
        let manifest = manifest(ProjectId::new());
        let baseline = snapshot_contract_surfaces(
            &manifest,
            "baseline-one".to_owned(),
            SurfaceSnapshotRole::Baseline,
            "a".repeat(40),
            None,
            vec![SurfaceSourceInput {
                descriptor: manifest.surfaces[0].clone(),
                bytes: b"pub fn old();\n".to_vec(),
                coverage: CoverageState::Complete,
                limitations: Vec::new(),
            }],
        )
        .unwrap();
        let current = snapshot_contract_surfaces(
            &manifest,
            "current-one".to_owned(),
            SurfaceSnapshotRole::Current,
            "b".repeat(40),
            None,
            vec![SurfaceSourceInput {
                descriptor: manifest.surfaces[0].clone(),
                bytes: b"pub fn new();\n".to_vec(),
                coverage: CoverageState::Complete,
                limitations: Vec::new(),
            }],
        )
        .unwrap();
        let report =
            compare_surface_snapshots(&manifest, "report-one".to_owned(), &baseline, &current)
                .unwrap();
        assert_eq!(report.outcome, CompatibilityClass::Breaking);
        assert!(report.consumer_impacts[0].migration_required);
    }

    #[test]
    fn environment_fingerprint_excludes_absolute_project_path_and_doctor_is_read_only() {
        let project_id = ProjectId::new();
        let first = build_environment_snapshot(EnvironmentProbeInput {
            snapshot_id: "environment-one".to_owned(),
            project_id: project_id.clone(),
            subject_revision: "a".repeat(40),
            os_family: "windows".to_owned(),
            os_release: "11".to_owned(),
            architecture: "x86_64".to_owned(),
            filesystem_kind: "ntfs".to_owned(),
            case_behavior: "insensitive_preserving".to_owned(),
            symlink_capability: "unknown".to_owned(),
            long_path_capability: "unknown".to_owned(),
            path_kind: "drive".to_owned(),
            path_depth: 5,
            path_length_bucket: "64_127".to_owned(),
            text_encoding_policy: "utf8".to_owned(),
            line_ending_policy: "mixed_observed".to_owned(),
            toolchains: Vec::new(),
            manifests: Vec::new(),
            task_descriptor_refs: Vec::new(),
            environment_contract_presence: Vec::new(),
            completeness: CoverageState::Partial,
            limitations: vec!["read-only probe did not execute toolchains".to_owned()],
        })
        .unwrap();
        assert_eq!(first.schema_id, ENVIRONMENT_SNAPSHOT_SCHEMA_ID);
        assert!(!serde_json::to_string(&first).unwrap().contains("C:\\\\"));
    }
}
