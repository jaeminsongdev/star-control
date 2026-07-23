//! Deterministic per-source M1 index construction and incremental reuse.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use star_contracts::{
    Sha256Hash,
    ids::{
        CanonicalSourceId, CodeIndexSnapshotId, GenerationId, ScanRunId, SymbolId,
        SymbolReferenceId,
    },
    index::{
        CodeIndexCounts, CodeIndexSnapshot, DiscoveryProvenance, FreshnessProof, GuidanceKind,
        GuidanceRecord, HardcodingAssessment, HardcodingCandidate, HardcodingCategory,
        HardcodingRedactionState, IndexCoverage, IndexEdge, IndexEntity, IndexEntityKind,
        IndexFreshnessState, IndexLimitation, IndexPartition, IndexPartitionKind,
        IndexPartitionState, IndexRelation, IndexScanMode, IndexTier, ProjectCatalogSnapshot,
        SourceClass, SourceEntry, ToolchainCommandDeclaration, ToolchainCommandKind,
        ToolchainRecord,
    },
    management::{
        Project, ProjectCheckout, ProjectPathRef, SourceRange, Symbol, SymbolReference,
        SymbolResolution,
    },
};
use star_domain::versioned_fingerprint;

use crate::{FileObservation, ProjectError, ProjectObservation};

#[derive(Clone, Debug, Serialize)]
pub struct IndexPolicy {
    pub required_tier: IndexTier,
    pub max_tier: IndexTier,
    pub max_text_tokens_per_file: usize,
    pub classification_contract_version: u32,
    pub text_adapter_version: u32,
}

impl Default for IndexPolicy {
    fn default() -> Self {
        Self {
            required_tier: IndexTier::Text,
            max_tier: IndexTier::Semantic,
            max_text_tokens_per_file: 250_000,
            classification_contract_version: 1,
            text_adapter_version: 1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SyntaxDefinition {
    pub qualified_name: String,
    pub symbol_kind: String,
    pub range: SourceRange,
    pub visibility: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SyntaxReference {
    pub target_name: String,
    pub range: SourceRange,
    pub reference_kind: String,
    pub resolution: SymbolResolution,
}

#[derive(Clone, Debug, Default)]
pub struct SyntaxAnalysis {
    pub definitions: Vec<SyntaxDefinition>,
    pub references: Vec<SyntaxReference>,
    pub limitations: Vec<IndexLimitation>,
}

#[derive(Clone, Copy, Debug)]
pub enum AdapterFailure {
    ParseFailed,
    ResourceLimit,
    Unavailable,
}

pub trait SyntaxAdapter: Send + Sync {
    fn language_id(&self) -> &'static str;
    fn fingerprint(&self) -> Sha256Hash;
    fn analyze(&self, source: &FileObservation) -> Result<SyntaxAnalysis, AdapterFailure>;
}

#[derive(Clone, Debug, Default)]
pub struct SemanticAnalysis {
    pub definitions: Vec<SyntaxDefinition>,
    pub references: Vec<SyntaxReference>,
    pub limitations: Vec<IndexLimitation>,
}

pub trait SemanticAdapter: Send + Sync {
    fn language_id(&self) -> &'static str;
    fn fingerprint(&self) -> Sha256Hash;
    fn prepare(
        &self,
        _project_root: &Path,
        _observation: &ProjectObservation,
    ) -> Result<(), AdapterFailure> {
        Ok(())
    }
    fn analyze(&self, source: &FileObservation) -> Result<SemanticAnalysis, AdapterFailure>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CodeIndexProjection {
    pub snapshot: CodeIndexSnapshot,
    pub source_entries: Vec<SourceEntry>,
    pub entities: Vec<IndexEntity>,
    pub edges: Vec<IndexEdge>,
    pub symbols: Vec<Symbol>,
    pub references: Vec<SymbolReference>,
}

pub struct CodeIndexBuildRequest<'a> {
    pub project_root: Option<&'a Path>,
    pub project: &'a Project,
    pub checkout: &'a ProjectCheckout,
    pub catalog_snapshot: &'a ProjectCatalogSnapshot,
    pub observation: &'a ProjectObservation,
    pub scan_run_id: &'a ScanRunId,
    pub generation_id: &'a GenerationId,
    pub policy: &'a IndexPolicy,
    pub syntax_adapters: &'a [&'a dyn SyntaxAdapter],
    pub semantic_adapters: &'a [&'a dyn SemanticAdapter],
    pub scan_mode: IndexScanMode,
    pub previous: Option<&'a CodeIndexProjection>,
}

pub fn build_code_index(
    request: &CodeIndexBuildRequest<'_>,
) -> Result<CodeIndexProjection, ProjectError> {
    validate_request(request)?;
    let semantic_prepare_failures = request
        .semantic_adapters
        .iter()
        .filter_map(|adapter| {
            let result = request
                .project_root
                .ok_or(AdapterFailure::Unavailable)
                .and_then(|root| adapter.prepare(root, request.observation));
            result.err().map(|failure| (adapter.language_id(), failure))
        })
        .collect::<BTreeMap<_, _>>();
    let classification_fingerprint = versioned_fingerprint(
        "star.source-classification",
        request.policy.classification_contract_version,
        &serde_json::json!({
            "precedence":["vendor","generated","cache","output","test","documentation","source","other"],
            "nested_ownership":"catalog_snapshot",
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    let index_config_fingerprint = versioned_fingerprint("star.index-config", 1, request.policy)
        .map_err(|_| ProjectError::Fingerprint)?;
    let adapter_set_fingerprint = adapter_set_fingerprint(request)?;
    let checkout_ref = request
        .catalog_snapshot
        .checkout_refs
        .iter()
        .find(|item| item.checkout_id == request.checkout.checkout_id)
        .ok_or(ProjectError::InvalidManifest)?;
    let workspace_snapshot_id = request
        .observation
        .workspace_snapshot_id(&request.project.project_id)?;

    let can_reuse = request.scan_mode == IndexScanMode::Incremental
        && request.previous.is_some_and(|previous| {
            previous.snapshot.project_id == request.project.project_id
                && previous.snapshot.checkout_id == request.checkout.checkout_id
                && previous.snapshot.checkout_observation_fingerprint
                    == checkout_ref.observation_fingerprint
                && previous.snapshot.scan_config_fingerprint
                    == request.observation.scan_config_fingerprint
                && previous.snapshot.index_config_fingerprint == index_config_fingerprint
                && previous.snapshot.adapter_set_fingerprint == adapter_set_fingerprint
                && previous.snapshot.classification_fingerprint == classification_fingerprint
        });
    let previous_index = request
        .previous
        .filter(|_| can_reuse)
        .map(PreviousProjectionIndex::new);

    let mut projection = ProjectionAccumulator::default();
    for file in &request.observation.files {
        let source = source_entry(request, file)?;
        projection
            .entities
            .push(source_index_entity(file, &source)?);
        let reusable = previous_index
            .as_ref()
            .and_then(|index| index.source_entries.get(source.path.as_str()))
            .is_some_and(|previous| {
                previous.content_sha256 == source.content_sha256
                    && previous.source_class == source.source_class
                    && previous.facets == source.facets
                    && previous.language_id == source.language_id
                    && previous.analysis_eligible == source.analysis_eligible
            });
        let reused_partitions = if reusable {
            if let Some(previous) = &previous_index {
                reuse_source_projection(
                    previous,
                    &source,
                    request.scan_run_id,
                    &workspace_snapshot_id,
                    &mut projection,
                )
            } else {
                BTreeSet::new()
            }
        } else {
            BTreeSet::new()
        };
        if !reused_partitions.contains(&IndexPartitionKind::Classification) {
            index_classification_source(request, &source, &mut projection)?;
        }
        if !reused_partitions.contains(&IndexPartitionKind::Text) {
            index_text_source(request, file, &source, &mut projection)?;
        }
        if request.policy.max_tier >= IndexTier::Syntax
            && !reused_partitions.contains(&IndexPartitionKind::Syntax)
        {
            if syntax_eligible(&source, file) {
                let adapter = request
                    .syntax_adapters
                    .iter()
                    .find(|adapter| adapter.language_id() == source.language_id)
                    .copied();
                index_syntax_partition(request, file, &source, adapter, &mut projection)?;
            } else {
                index_classification_exclusion(
                    request,
                    &source,
                    IndexPartitionKind::Syntax,
                    IndexTier::Syntax,
                    &mut projection,
                )?;
            }
        }
        if request.policy.max_tier >= IndexTier::Semantic
            && !reused_partitions.contains(&IndexPartitionKind::Semantic)
        {
            if semantic_eligible(&source, file) {
                let adapter = request
                    .semantic_adapters
                    .iter()
                    .find(|adapter| adapter.language_id() == source.language_id)
                    .copied();
                let prepare_failure = semantic_prepare_failures
                    .get(source.language_id.as_str())
                    .copied();
                index_semantic_partition(
                    request,
                    file,
                    &source,
                    adapter,
                    prepare_failure,
                    &mut projection,
                )?;
            } else {
                index_classification_exclusion(
                    request,
                    &source,
                    IndexPartitionKind::Semantic,
                    IndexTier::Semantic,
                    &mut projection,
                )?;
            }
        }
        projection.source_entries.push(source);
    }
    projection
        .source_entries
        .sort_by(|left, right| left.path.cmp(&right.path));
    projection
        .entities
        .sort_by(|left, right| left.entity_key.cmp(&right.entity_key));
    projection
        .entities
        .dedup_by(|left, right| left.entity_key == right.entity_key);
    projection
        .edges
        .sort_by(|left, right| left.edge_key.cmp(&right.edge_key));
    projection
        .edges
        .dedup_by(|left, right| left.edge_key == right.edge_key);
    projection
        .symbols
        .sort_by(|left, right| left.symbol_id.cmp(&right.symbol_id));
    projection
        .symbols
        .dedup_by(|left, right| left.symbol_id == right.symbol_id);
    projection
        .references
        .sort_by(|left, right| left.symbol_reference_id.cmp(&right.symbol_reference_id));
    projection
        .references
        .dedup_by(|left, right| left.symbol_reference_id == right.symbol_reference_id);
    projection
        .partitions
        .sort_by(|left, right| left.partition_key.cmp(&right.partition_key));
    projection.limitations.sort_by(|left, right| {
        (&left.code, &left.scope, &left.parameters).cmp(&(
            &right.code,
            &right.scope,
            &right.parameters,
        ))
    });
    projection.limitations.dedup();

    let inventory_input = versioned_fingerprint(
        "star.index-partition-input.inventory",
        1,
        &request.observation.entries_fingerprint,
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    let inventory_output = versioned_fingerprint(
        "star.index-partition-output.inventory",
        1,
        &projection.source_entries,
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    projection.partitions.push(IndexPartition {
        partition_key: "inventory".to_owned(),
        kind: IndexPartitionKind::Inventory,
        required: true,
        requested_tier: IndexTier::Text,
        used_tier: Some(IndexTier::Text),
        state: if can_reuse
            && request.previous.is_some_and(|previous| {
                previous.snapshot.workspace_snapshot_id == workspace_snapshot_id
            }) {
            IndexPartitionState::Reused
        } else if request.observation.completeness
            == star_contracts::management::Completeness::Complete
        {
            IndexPartitionState::Succeeded
        } else {
            IndexPartitionState::Incomplete
        },
        input_fingerprint: inventory_input,
        output_fingerprint: Some(inventory_output),
        target_count: projection.source_entries.len() as u64,
        indexed_count: projection.source_entries.len() as u64,
        failed_count: 0,
        excluded_count: projection
            .source_entries
            .iter()
            .filter(|source| !source.analysis_eligible)
            .count() as u64,
        cache_hit: can_reuse,
        limitations: Vec::new(),
    });
    projection
        .partitions
        .sort_by(|left, right| left.partition_key.cmp(&right.partition_key));

    let hardcoding_candidates =
        detect_hardcoding_candidates(request, &projection.source_entries, &projection.entities)?;
    let finding_input = versioned_fingerprint(
        "star.index-partition-input.hardcoding",
        1,
        &serde_json::json!({
            "entries_fingerprint":request.observation.entries_fingerprint,
            "classification_fingerprint":classification_fingerprint,
            "rule_version":"1.0.0",
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    let finding_output = versioned_fingerprint(
        "star.index-partition-output.hardcoding",
        1,
        &hardcoding_candidates,
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    projection.partitions.push(IndexPartition {
        partition_key: "finding:hardcoding".to_owned(),
        kind: IndexPartitionKind::Finding,
        required: false,
        requested_tier: IndexTier::Text,
        used_tier: Some(IndexTier::Text),
        state: IndexPartitionState::Succeeded,
        input_fingerprint: finding_input,
        output_fingerprint: Some(finding_output),
        target_count: projection.source_entries.len() as u64,
        indexed_count: projection.source_entries.len() as u64,
        failed_count: 0,
        excluded_count: projection
            .source_entries
            .iter()
            .filter(|source| !hardcoding_source_eligible(source))
            .count() as u64,
        cache_hit: false,
        limitations: Vec::new(),
    });
    projection
        .partitions
        .sort_by(|left, right| left.partition_key.cmp(&right.partition_key));
    let coverage = coverage(&projection.source_entries, &projection.partitions);
    let toolchains = discover_toolchains(request)?;
    let guidance = discover_guidance(request)?;
    let counts = CodeIndexCounts {
        sources: projection.source_entries.len() as u64,
        packages: projection
            .entities
            .iter()
            .filter(|item| item.kind == IndexEntityKind::Package)
            .count() as u64,
        modules: projection
            .entities
            .iter()
            .filter(|item| item.kind == IndexEntityKind::Module)
            .count() as u64,
        symbols: projection.symbols.len() as u64,
        definitions: projection.symbols.len() as u64,
        references: projection.references.len() as u64,
        graph_edges: projection.edges.len() as u64,
        findings: hardcoding_candidates.len() as u64,
    };
    let analysis_input_fingerprint = versioned_fingerprint(
        "star.code-index-analysis-input",
        1,
        &serde_json::json!({
            "project_id":request.project.project_id,
            "checkout_id":request.checkout.checkout_id,
            "checkout_observation_fingerprint":checkout_ref.observation_fingerprint,
            "project_revision_id":request.observation.revision.project_revision_id,
            "workspace_snapshot_id":workspace_snapshot_id,
            "scan_config_fingerprint":request.observation.scan_config_fingerprint,
            "index_config_fingerprint":index_config_fingerprint,
            "classification_fingerprint":classification_fingerprint,
            "adapter_set_fingerprint":adapter_set_fingerprint,
            "scan_mode":request.scan_mode,
            "partition_inputs":projection.partitions.iter().map(|item| (&item.partition_key,&item.input_fingerprint)).collect::<Vec<_>>(),
            "toolchains":toolchains.iter().map(|item| (&item.record_key,&item.content_fingerprint)).collect::<Vec<_>>(),
            "guidance":guidance.iter().map(|item| (&item.record_key,&item.content_fingerprint)).collect::<Vec<_>>(),
            "hardcoding_candidates":hardcoding_candidates.iter().map(|item| (&item.candidate_key,&item.content_fingerprint)).collect::<Vec<_>>(),
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    let stable_partition_outputs = projection
        .partitions
        .iter()
        .map(|item| {
            serde_json::json!({
                "partition_key":item.partition_key,
                "kind":item.kind,
                "used_tier":item.used_tier,
                "output_fingerprint":item.output_fingerprint,
                "target_count":item.target_count,
                "indexed_count":item.indexed_count,
                "failed_count":item.failed_count,
                "excluded_count":item.excluded_count,
                "limitations":item.limitations,
            })
        })
        .collect::<Vec<_>>();
    let content_fingerprint = versioned_fingerprint(
        "star.code-index-snapshot-content",
        1,
        &serde_json::json!({
            "partition_outputs":stable_partition_outputs,
            "source_entries":projection.source_entries,
            "entities":projection.entities,
            "edges":projection.edges,
            "symbols":projection.symbols.iter().map(|item| (&item.symbol_id,&item.content_fingerprint)).collect::<Vec<_>>(),
            "references":projection.references.iter().map(|item| &item.symbol_reference_id).collect::<Vec<_>>(),
            "coverage":coverage,
            "counts":counts,
            "toolchains":toolchains,
            "guidance":guidance,
            "hardcoding_candidates":hardcoding_candidates,
            "limitations":projection.limitations,
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    let id_fingerprint = versioned_fingerprint(
        "star.code-index-snapshot-id",
        1,
        &serde_json::json!({
            "analysis_input_fingerprint":analysis_input_fingerprint,
            "partition_outputs":stable_partition_outputs,
            "content_fingerprint":content_fingerprint,
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    let freshness = projection
        .partitions
        .iter()
        .map(|partition| FreshnessProof {
            partition_key: partition.partition_key.clone(),
            state: if matches!(
                partition.state,
                IndexPartitionState::Succeeded | IndexPartitionState::Reused
            ) {
                IndexFreshnessState::Current
            } else if partition_unavailable(partition) {
                IndexFreshnessState::Unavailable
            } else {
                IndexFreshnessState::Partial
            },
            indexed_catalog_fingerprint: request.catalog_snapshot.content_fingerprint.clone(),
            indexed_source_fingerprint: partition.input_fingerprint.clone(),
            indexed_config_fingerprint: index_config_fingerprint.clone(),
            indexed_adapter_fingerprint: adapter_set_fingerprint.clone(),
            observed_source_fingerprint: Some(partition.input_fingerprint.clone()),
            probe_method: "scan_content_sha256".to_owned(),
            probed_at: Utc::now(),
            stale_reason_codes: Vec::new(),
            unverified_scope_count: if matches!(
                partition.state,
                IndexPartitionState::Succeeded | IndexPartitionState::Reused
            ) {
                0
            } else {
                partition.failed_count.max(1)
            },
        })
        .collect();
    let snapshot = CodeIndexSnapshot {
        schema_id: "star.code-index-snapshot".to_owned(),
        schema_version: 1,
        code_index_snapshot_id: CodeIndexSnapshotId::from_fingerprint(&id_fingerprint),
        project_id: request.project.project_id.clone(),
        checkout_id: request.checkout.checkout_id.clone(),
        project_catalog_snapshot_id: request.catalog_snapshot.project_catalog_snapshot_id.clone(),
        checkout_observation_fingerprint: checkout_ref.observation_fingerprint.clone(),
        project_revision_id: request.observation.revision.project_revision_id.clone(),
        workspace_snapshot_id,
        scan_run_id: request.scan_run_id.clone(),
        generation_id: request.generation_id.clone(),
        analysis_input_fingerprint,
        scan_config_fingerprint: request.observation.scan_config_fingerprint.clone(),
        index_config_fingerprint,
        scan_mode: request.scan_mode,
        required_tier: request.policy.required_tier,
        max_tier: request.policy.max_tier,
        adapter_set_fingerprint,
        classification_fingerprint,
        partitions: projection.partitions,
        coverage,
        counts,
        freshness,
        toolchains,
        guidance,
        hardcoding_candidates,
        limitations: projection.limitations,
        artifact_refs: Vec::new(),
        content_fingerprint,
    };
    Ok(CodeIndexProjection {
        snapshot,
        source_entries: projection.source_entries,
        entities: projection.entities,
        edges: projection.edges,
        symbols: projection.symbols,
        references: projection.references,
    })
}

fn discover_toolchains(
    request: &CodeIndexBuildRequest<'_>,
) -> Result<Vec<ToolchainRecord>, ProjectError> {
    let mut records = Vec::new();
    for manifest in &request.observation.files {
        let path = manifest.path.as_str();
        let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
        let scope = path.rsplit_once('/').map(|(scope, _)| scope);
        let descriptor = match name.as_str() {
            "cargo.toml" => Some((
                vec!["rust".to_owned()],
                Some("cargo".to_owned()),
                Some("cargo".to_owned()),
                vec![
                    ("check", vec!["check"]),
                    ("test", vec!["test"]),
                    ("clippy", vec!["clippy"]),
                    ("fmt-check", vec!["fmt", "--check"]),
                ],
                &["Cargo.lock"][..],
                &["rust-toolchain.toml", "rust-toolchain"][..],
            )),
            "package.json" => Some((
                vec!["javascript".to_owned(), "typescript".to_owned()],
                Some("node".to_owned()),
                package_manager_from_package_json(manifest.text.as_deref()),
                Vec::new(),
                &["pnpm-lock.yaml", "yarn.lock", "package-lock.json"][..],
                &[".nvmrc", ".node-version"][..],
            )),
            "pyproject.toml" => Some((
                vec!["python".to_owned()],
                Some("pyproject".to_owned()),
                Some("python".to_owned()),
                Vec::new(),
                &["poetry.lock", "uv.lock", "Pipfile.lock"][..],
                &[".python-version"][..],
            )),
            "go.mod" => Some((
                vec!["go".to_owned()],
                Some("go_modules".to_owned()),
                Some("go".to_owned()),
                vec![("test", vec!["test", "./..."])],
                &["go.sum"][..],
                &["go.work"][..],
            )),
            "pom.xml" => Some((
                vec!["java".to_owned()],
                Some("maven".to_owned()),
                Some("maven".to_owned()),
                vec![("test", vec!["test"])],
                &["mvnw", ".mvn/wrapper/maven-wrapper.properties"][..],
                &[".mvn/jvm.config"][..],
            )),
            "build.gradle" | "build.gradle.kts" => Some((
                vec!["java".to_owned(), "kotlin".to_owned()],
                Some("gradle".to_owned()),
                Some("gradle".to_owned()),
                vec![("test", vec!["test"])],
                &["gradle.lockfile", "gradlew"][..],
                &["gradle.properties"][..],
            )),
            _ if name.ends_with(".csproj") || name.ends_with(".sln") => Some((
                vec!["csharp".to_owned()],
                Some("dotnet".to_owned()),
                Some("nuget".to_owned()),
                vec![("test", vec!["test"])],
                &["packages.lock.json"][..],
                &["global.json"][..],
            )),
            _ => None,
        };
        let Some((
            mut language_ids,
            build_system,
            mut package_manager,
            suggested,
            lock_names,
            toolchain_names,
        )) = descriptor
        else {
            continue;
        };
        language_ids.sort();
        language_ids.dedup();

        let lockfile = first_observed_relative(request.observation, scope, lock_names);
        let toolchain = first_observed_relative(request.observation, scope, toolchain_names)
            .or_else(|| first_observed_relative(request.observation, None, toolchain_names));
        let mut commands = if name == "package.json" {
            package_json_commands(manifest, package_manager.as_deref().unwrap_or("npm"))
        } else {
            suggested
                .into_iter()
                .map(|(command_id, args)| ToolchainCommandDeclaration {
                    command_id: command_id.to_owned(),
                    executable_hint: executable_for(build_system.as_deref()),
                    args: args.into_iter().map(str::to_owned).collect(),
                    cwd_scope: scope.and_then(project_path),
                    source_ref: manifest.path.clone(),
                    declaration_kind: ToolchainCommandKind::Suggested,
                    confidence: "medium".to_owned(),
                })
                .collect::<Vec<_>>()
        };
        commands.sort_by(|left, right| left.command_id.cmp(&right.command_id));
        commands.dedup_by(|left, right| left.command_id == right.command_id);

        if package_manager.is_none() {
            package_manager = build_system.clone();
        }
        let toolchain_constraint = toolchain
            .and_then(|file| extract_toolchain_constraint(&name, file.text.as_deref()))
            .or_else(|| extract_manifest_constraint(&name, manifest.text.as_deref()));
        let mut evidence_refs = vec![manifest.path.clone()];
        if let Some(file) = lockfile {
            evidence_refs.push(file.path.clone());
        }
        if let Some(file) = toolchain {
            evidence_refs.push(file.path.clone());
        }
        evidence_refs.sort();
        evidence_refs.dedup();
        let record_key = format!("toolchain:{}", manifest.path.as_str());
        let content_fingerprint = versioned_fingerprint(
            "star.toolchain-record",
            1,
            &serde_json::json!({
                "record_key":record_key,
                "project_id":request.project.project_id,
                "checkout_id":request.checkout.checkout_id,
                "language_ids":language_ids,
                "build_system":build_system,
                "package_manager":package_manager,
                "manifest_ref":manifest.path,
                "lockfile_ref":lockfile.map(|file| &file.path),
                "lockfile_sha256":lockfile.map(|file| &file.content_sha256),
                "toolchain_file_ref":toolchain.map(|file| &file.path),
                "toolchain_constraint":toolchain_constraint,
                "provenance":DiscoveryProvenance::Declared,
                "commands":commands,
                "evidence_refs":evidence_refs,
            }),
        )
        .map_err(|_| ProjectError::Fingerprint)?;
        records.push(ToolchainRecord {
            record_key,
            project_id: request.project.project_id.clone(),
            checkout_id: request.checkout.checkout_id.clone(),
            language_ids,
            build_system,
            package_manager,
            manifest_ref: Some(manifest.path.clone()),
            lockfile_ref: lockfile.map(|file| file.path.clone()),
            lockfile_sha256: lockfile.map(|file| file.content_sha256.clone()),
            toolchain_file_ref: toolchain.map(|file| file.path.clone()),
            toolchain_constraint,
            provenance: DiscoveryProvenance::Declared,
            commands,
            evidence_refs,
            limitations: Vec::new(),
            content_fingerprint,
        });
    }
    records.sort_by(|left, right| left.record_key.cmp(&right.record_key));
    Ok(records)
}

fn first_observed_relative<'a>(
    observation: &'a ProjectObservation,
    scope: Option<&str>,
    names: &[&str],
) -> Option<&'a FileObservation> {
    names.iter().find_map(|name| {
        let candidate = match scope {
            Some(scope) => format!("{scope}/{name}"),
            None => (*name).to_owned(),
        };
        observation
            .files
            .iter()
            .find(|file| file.path.as_str().eq_ignore_ascii_case(&candidate))
    })
}

fn project_path(value: &str) -> Option<ProjectPathRef> {
    ProjectPathRef::parse(value.to_owned()).ok()
}

fn executable_for(build_system: Option<&str>) -> String {
    match build_system {
        Some("maven") => "mvn",
        Some("gradle") => "gradle",
        Some("dotnet") => "dotnet",
        Some("go_modules") => "go",
        Some(value) => value,
        None => "unknown",
    }
    .to_owned()
}

fn package_manager_from_package_json(text: Option<&str>) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text?).ok()?;
    if let Some(declared) = value
        .get("packageManager")
        .and_then(serde_json::Value::as_str)
    {
        let manager = declared.split('@').next().unwrap_or(declared);
        if matches!(manager, "npm" | "pnpm" | "yarn" | "bun") {
            return Some(manager.to_owned());
        }
    }
    Some("npm".to_owned())
}

fn package_json_commands(
    file: &FileObservation,
    package_manager: &str,
) -> Vec<ToolchainCommandDeclaration> {
    let value = file
        .text
        .as_deref()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok());
    let mut script_names = value
        .as_ref()
        .and_then(|value| value.get("scripts"))
        .and_then(serde_json::Value::as_object)
        .map(|scripts| scripts.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    script_names.sort();
    script_names
        .into_iter()
        .filter(|name| {
            !name.is_empty()
                && name.len() <= 80
                && name.chars().all(|character| {
                    character.is_ascii_alphanumeric() || "-_:./".contains(character)
                })
        })
        .map(|name| ToolchainCommandDeclaration {
            command_id: format!("script:{name}"),
            executable_hint: package_manager.to_owned(),
            args: vec!["run".to_owned(), name],
            cwd_scope: file
                .path
                .as_str()
                .rsplit_once('/')
                .and_then(|(scope, _)| project_path(scope)),
            source_ref: file.path.clone(),
            declaration_kind: ToolchainCommandKind::Declared,
            confidence: "high".to_owned(),
        })
        .collect()
}

fn extract_toolchain_constraint(manifest_name: &str, text: Option<&str>) -> Option<String> {
    let text = text?;
    if manifest_name == "cargo.toml"
        && let Ok(value) = toml::from_str::<toml::Value>(text)
        && let Some(channel) = value
            .get("toolchain")
            .and_then(|value| value.get("channel"))
            .and_then(toml::Value::as_str)
    {
        return safe_constraint(channel);
    }
    safe_constraint(text.lines().find(|line| !line.trim().is_empty())?.trim())
}

fn extract_manifest_constraint(manifest_name: &str, text: Option<&str>) -> Option<String> {
    let text = text?;
    match manifest_name {
        "package.json" => {
            let value: serde_json::Value = serde_json::from_str(text).ok()?;
            value
                .get("engines")
                .and_then(|value| value.get("node"))
                .and_then(serde_json::Value::as_str)
                .and_then(safe_constraint)
        }
        "pyproject.toml" => {
            let value: toml::Value = toml::from_str(text).ok()?;
            value
                .get("project")
                .and_then(|value| value.get("requires-python"))
                .and_then(toml::Value::as_str)
                .and_then(safe_constraint)
        }
        "go.mod" => text
            .lines()
            .find_map(|line| line.trim().strip_prefix("go "))
            .and_then(safe_constraint),
        _ => None,
    }
}

fn safe_constraint(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 128
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || ".,_+<>=^~* -".contains(character)
        })
    {
        return None;
    }
    Some(value.to_owned())
}

fn discover_guidance(
    request: &CodeIndexBuildRequest<'_>,
) -> Result<Vec<GuidanceRecord>, ProjectError> {
    let mut records = request
        .observation
        .files
        .iter()
        .filter_map(|file| {
            let (kind, priority, priority_reason) = guidance_descriptor(file.path.as_str())?;
            let applicable_scope = file
                .path
                .as_str()
                .rsplit_once('/')
                .and_then(|(scope, _)| project_path(scope));
            let heading_anchors = file
                .text
                .as_deref()
                .map(guidance_heading_anchors)
                .unwrap_or_default();
            let limitations = if file.text.is_some() {
                Vec::new()
            } else {
                vec![IndexLimitation {
                    code: "GUIDANCE_CONTENT_UNAVAILABLE".to_owned(),
                    scope: Some(file.path.as_str().to_owned()),
                    parameters: BTreeMap::new(),
                }]
            };
            let record_key = format!("guidance:{}", file.path.as_str());
            let content_fingerprint = versioned_fingerprint(
                "star.guidance-record",
                1,
                &serde_json::json!({
                    "record_key":record_key,
                    "project_id":request.project.project_id,
                    "checkout_id":request.checkout.checkout_id,
                    "kind":kind,
                    "source_ref":file.path,
                    "source_sha256":file.content_sha256,
                    "applicable_scope":applicable_scope,
                    "priority":priority,
                    "priority_reason":priority_reason,
                    "heading_anchors":heading_anchors,
                    "freshness":IndexFreshnessState::Current,
                    "limitations":limitations,
                }),
            )
            .ok()?;
            Some(GuidanceRecord {
                record_key,
                project_id: request.project.project_id.clone(),
                checkout_id: request.checkout.checkout_id.clone(),
                kind,
                source_ref: file.path.clone(),
                source_sha256: file.content_sha256.clone(),
                applicable_scope,
                priority,
                priority_reason: priority_reason.to_owned(),
                supersedes: Vec::new(),
                heading_anchors,
                redacted_summary: None,
                freshness: IndexFreshnessState::Current,
                conflict: false,
                limitations,
                content_fingerprint,
            })
        })
        .collect::<Vec<_>>();
    records.sort_by(|left, right| {
        (left.priority, &left.record_key).cmp(&(right.priority, &right.record_key))
    });
    for index in 0..records.len() {
        let scope = records[index].applicable_scope.clone();
        let priority = records[index].priority;
        let same_precedence = records
            .iter()
            .enumerate()
            .filter(|(other, record)| {
                *other != index && record.priority == priority && record.applicable_scope == scope
            })
            .count();
        records[index].conflict = same_precedence > 0;
        records[index].supersedes = records
            .iter()
            .filter(|record| record.priority > priority && record.applicable_scope == scope)
            .map(|record| record.record_key.clone())
            .collect();
    }
    for record in &mut records {
        record.content_fingerprint = versioned_fingerprint(
            "star.guidance-record-final",
            1,
            &serde_json::json!({
                "base":record.content_fingerprint,
                "supersedes":record.supersedes,
                "conflict":record.conflict,
            }),
        )
        .map_err(|_| ProjectError::Fingerprint)?;
    }
    Ok(records)
}

fn guidance_descriptor(path: &str) -> Option<(GuidanceKind, u32, &'static str)> {
    let lower = path.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    if lower.starts_with(".star-control/") {
        return Some((
            GuidanceKind::ProjectManifest,
            10,
            "explicit_project_declaration",
        ));
    }
    if name == "agents.md" {
        return Some((GuidanceKind::Agents, 20, "agents_scope_chain"));
    }
    if lower == "docs/readme.md" {
        return Some((
            GuidanceKind::ReadingOrder,
            30,
            "canonical_docs_reading_order",
        ));
    }
    if !lower.contains('/') && (name == "readme.md" || name == "readme") {
        return Some((GuidanceKind::Readme, 40, "root_readme"));
    }
    if name.starts_with("contributing") {
        return Some((GuidanceKind::Contribution, 45, "contribution_guidance"));
    }
    if lower.starts_with("docs/architecture/") {
        return Some((GuidanceKind::Architecture, 50, "architecture_document"));
    }
    if lower.starts_with("docs/contracts/") {
        return Some((GuidanceKind::Contract, 55, "contract_document"));
    }
    if matches!(
        name,
        "cargo.toml"
            | "package.json"
            | "pyproject.toml"
            | "go.mod"
            | "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
    ) {
        return Some((GuidanceKind::BuildManifest, 60, "build_manifest"));
    }
    if name.contains("guidance") || name.contains("instruction") {
        return Some((GuidanceKind::Heuristic, 70, "filename_heuristic"));
    }
    None
}

fn guidance_heading_anchors(text: &str) -> Vec<String> {
    let mut anchors = text
        .lines()
        .filter_map(|line| {
            let heading = line
                .trim_start()
                .strip_prefix('#')?
                .trim_start_matches('#')
                .trim();
            if heading.is_empty() || heading.len() > 160 {
                return None;
            }
            let mut anchor = String::new();
            let mut dash = false;
            for character in heading.chars() {
                if character.is_alphanumeric() {
                    anchor.extend(character.to_lowercase());
                    dash = false;
                } else if !dash && !anchor.is_empty() {
                    anchor.push('-');
                    dash = true;
                }
            }
            let anchor = anchor.trim_end_matches('-');
            (!anchor.is_empty()).then(|| anchor.chars().take(128).collect::<String>())
        })
        .take(64)
        .collect::<Vec<_>>();
    anchors.sort();
    anchors.dedup();
    anchors
}

#[derive(Clone)]
struct HardcodingSeed<'a> {
    file: &'a FileObservation,
    source: &'a SourceEntry,
    category: HardcodingCategory,
    start_byte: usize,
    end_byte: usize,
    line_index: usize,
    matched_predicate: &'static str,
    literal_shape: &'static str,
    confidence: &'static str,
    assessment: HardcodingAssessment,
    guards: Vec<String>,
}

fn detect_hardcoding_candidates(
    request: &CodeIndexBuildRequest<'_>,
    sources: &[SourceEntry],
    entities: &[IndexEntity],
) -> Result<Vec<HardcodingCandidate>, ProjectError> {
    let source_by_path = sources
        .iter()
        .map(|source| (source.path.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let parameter_fingerprint = versioned_fingerprint(
        "star.rule.hardcoding.parameters",
        1,
        &serde_json::json!({
            "categories":["absolute_path","endpoint","timeout_retry_limit","raw_command","duplicate_error","config_duplicate"],
            "production_classes":["source","migration"],
            "literal_persistence":"shape_only",
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    let mut seeds = Vec::new();
    let mut duplicate_errors = BTreeMap::<String, Vec<HardcodingSeed<'_>>>::new();
    let mut duplicate_configs = BTreeMap::<String, Vec<HardcodingSeed<'_>>>::new();
    for file in &request.observation.files {
        let Some(source) = source_by_path.get(file.path.as_str()).copied() else {
            return Err(ProjectError::InvalidManifest);
        };
        if !hardcoding_source_eligible(source) {
            continue;
        }
        let Some(text) = file.text.as_deref() else {
            continue;
        };
        for (line_index, line) in text.lines().enumerate() {
            if line.len() > 16 * 1024 {
                continue;
            }
            for (start, end, predicate, shape) in endpoint_matches(line) {
                seeds.push(HardcodingSeed {
                    file,
                    source,
                    category: HardcodingCategory::Endpoint,
                    start_byte: start,
                    end_byte: end,
                    line_index,
                    matched_predicate: predicate,
                    literal_shape: shape,
                    confidence: "medium",
                    assessment: HardcodingAssessment::Candidate,
                    guards: default_hardcoding_guards("endpoint_grammar"),
                });
            }
            for (start, end, predicate, shape) in absolute_path_matches(line) {
                seeds.push(HardcodingSeed {
                    file,
                    source,
                    category: HardcodingCategory::AbsolutePath,
                    start_byte: start,
                    end_byte: end,
                    line_index,
                    matched_predicate: predicate,
                    literal_shape: shape,
                    confidence: "medium",
                    assessment: HardcodingAssessment::Candidate,
                    guards: default_hardcoding_guards("absolute_path_grammar"),
                });
            }
            if let Some((start, end)) = timeout_retry_limit_match(line) {
                seeds.push(HardcodingSeed {
                    file,
                    source,
                    category: HardcodingCategory::TimeoutRetryLimit,
                    start_byte: start,
                    end_byte: end,
                    line_index,
                    matched_predicate: "numeric_literal_with_control_identifier",
                    literal_shape: "numeric_control_value",
                    confidence: "medium",
                    assessment: HardcodingAssessment::Candidate,
                    guards: default_hardcoding_guards("identifier_and_unit_context"),
                });
            }
            if let Some((start, end)) = raw_command_match(line) {
                seeds.push(HardcodingSeed {
                    file,
                    source,
                    category: HardcodingCategory::RawCommand,
                    start_byte: start,
                    end_byte: end,
                    line_index,
                    matched_predicate: "process_or_shell_sink",
                    literal_shape: "command_sink_expression",
                    confidence: "medium",
                    assessment: HardcodingAssessment::Review,
                    guards: default_hardcoding_guards("typed_command_declaration_not_matched"),
                });
            }
            if let Some((normalized, start, end)) = error_literal_match(line) {
                duplicate_errors
                    .entry(normalized)
                    .or_default()
                    .push(HardcodingSeed {
                        file,
                        source,
                        category: HardcodingCategory::DuplicateError,
                        start_byte: start,
                        end_byte: end,
                        line_index,
                        matched_predicate: "duplicate_production_error_literal",
                        literal_shape: "error_message_literal",
                        confidence: "high",
                        assessment: HardcodingAssessment::Warning,
                        guards: default_hardcoding_guards("safe_literal_and_distinct_location"),
                    });
            }
            if let Some((normalized, start, end, shape)) = config_literal_match(line) {
                duplicate_configs
                    .entry(normalized)
                    .or_default()
                    .push(HardcodingSeed {
                        file,
                        source,
                        category: HardcodingCategory::ConfigDuplicate,
                        start_byte: start,
                        end_byte: end,
                        line_index,
                        matched_predicate: "duplicate_config_literal",
                        literal_shape: shape,
                        confidence: "medium",
                        assessment: HardcodingAssessment::Review,
                        guards: default_hardcoding_guards(
                            "config_identifier_and_distinct_location",
                        ),
                    });
            }
        }
    }
    for group in duplicate_errors.into_values() {
        let distinct_locations = group
            .iter()
            .map(|seed| (seed.source.canonical_source_id.as_str(), seed.line_index))
            .collect::<BTreeSet<_>>();
        if distinct_locations.len() > 1 {
            seeds.extend(group);
        }
    }
    for group in duplicate_configs.into_values() {
        let distinct_locations = group
            .iter()
            .map(|seed| (seed.source.canonical_source_id.as_str(), seed.line_index))
            .collect::<BTreeSet<_>>();
        if distinct_locations.len() > 1 {
            seeds.extend(group);
        }
    }
    seeds.sort_by(|left, right| {
        (
            left.source.path.as_str(),
            left.line_index,
            left.start_byte,
            left.category,
        )
            .cmp(&(
                right.source.path.as_str(),
                right.line_index,
                right.start_byte,
                right.category,
            ))
    });
    seeds.dedup_by(|left, right| {
        left.source.canonical_source_id == right.source.canonical_source_id
            && left.line_index == right.line_index
            && left.start_byte == right.start_byte
            && left.category == right.category
    });
    seeds
        .into_iter()
        .map(|seed| hardcoding_candidate(seed, &parameter_fingerprint, entities))
        .collect()
}

fn hardcoding_source_eligible(source: &SourceEntry) -> bool {
    matches!(
        source.source_class,
        SourceClass::Source | SourceClass::Migration
    ) && !source
        .facets
        .iter()
        .any(|facet| matches!(facet.as_str(), "fixture" | "docs_example" | "generated"))
}

fn hardcoding_candidate(
    seed: HardcodingSeed<'_>,
    parameter_fingerprint: &Sha256Hash,
    entities: &[IndexEntity],
) -> Result<HardcodingCandidate, ProjectError> {
    let line = seed
        .file
        .text
        .as_deref()
        .and_then(|text| text.lines().nth(seed.line_index))
        .ok_or(ProjectError::InvalidManifest)?;
    let start_column = line[..seed.start_byte].chars().count() + 1;
    let end_column = line[..seed.end_byte].chars().count() + 1;
    let source_range = SourceRange {
        start_line: u32::try_from(seed.line_index + 1).unwrap_or(u32::MAX),
        start_column: u32::try_from(start_column).unwrap_or(u32::MAX),
        end_line: u32::try_from(seed.line_index + 1).unwrap_or(u32::MAX),
        end_column: u32::try_from(end_column.max(start_column + 1)).unwrap_or(u32::MAX),
    };
    let related_entity_key =
        hardcoding_owner_entity(&seed.source.canonical_source_id, &source_range, entities);
    let identity = versioned_fingerprint(
        "star.identity.hardcoding-candidate",
        1,
        &serde_json::json!({
            "rule_id":"star.rule.hardcoding-candidate",
            "rule_version":"1.0.0",
            "parameter_fingerprint":parameter_fingerprint,
            "canonical_source_id":seed.source.canonical_source_id,
            "source_content_sha256":seed.file.content_sha256,
            "source_range":source_range,
            "category":seed.category,
            "matched_predicate":seed.matched_predicate,
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    let candidate_key = format!("hardcoding:{}", identity.as_str());
    let length_bucket = length_bucket(seed.end_byte.saturating_sub(seed.start_byte));
    let content_fingerprint = versioned_fingerprint(
        "star.hardcoding-candidate",
        1,
        &serde_json::json!({
            "candidate_key":candidate_key,
            "source_class":seed.source.source_class,
            "source_facets":seed.source.facets,
            "used_tier":IndexTier::Text,
            "false_positive_guards":seed.guards,
            "confidence":seed.confidence,
            "redaction_state":HardcodingRedactionState::ShapeOnly,
            "literal_shape":seed.literal_shape,
            "length_bucket":length_bucket,
            "assessment":seed.assessment,
            "related_entity_key":related_entity_key,
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    Ok(HardcodingCandidate {
        candidate_key,
        rule_id: "star.rule.hardcoding-candidate".to_owned(),
        rule_version: "1.0.0".to_owned(),
        parameter_fingerprint: parameter_fingerprint.clone(),
        category: seed.category,
        canonical_source_id: seed.source.canonical_source_id.clone(),
        source_ref: seed.source.path.clone(),
        source_content_sha256: seed.file.content_sha256.clone(),
        source_range,
        source_class: seed.source.source_class,
        source_facets: seed.source.facets.clone(),
        used_tier: IndexTier::Text,
        matched_predicate: seed.matched_predicate.to_owned(),
        related_entity_key,
        false_positive_guards: seed.guards,
        confidence: seed.confidence.to_owned(),
        redaction_state: HardcodingRedactionState::ShapeOnly,
        literal_shape: seed.literal_shape.to_owned(),
        length_bucket,
        assessment: seed.assessment,
        limitations: Vec::new(),
        content_fingerprint,
    })
}

fn hardcoding_owner_entity(
    source_id: &CanonicalSourceId,
    candidate: &SourceRange,
    entities: &[IndexEntity],
) -> Option<String> {
    entities
        .iter()
        .filter(|entity| entity.canonical_source_id.as_ref() == Some(source_id))
        .filter_map(|entity| {
            let range = entity.source_range.as_ref()?;
            if !source_range_contains(range, candidate) {
                return None;
            }
            let line_span = range.end_line.saturating_sub(range.start_line);
            let column_span = if line_span == 0 {
                range.end_column.saturating_sub(range.start_column)
            } else {
                u32::MAX
            };
            let kind_rank = match entity.kind {
                IndexEntityKind::Symbol => 0_u8,
                IndexEntityKind::Contract
                | IndexEntityKind::ConfigKey
                | IndexEntityKind::SchemaId => 1,
                IndexEntityKind::Module | IndexEntityKind::Package => 2,
                _ => 3,
            };
            Some((
                (line_span, column_span, kind_rank, &entity.entity_key),
                entity,
            ))
        })
        .min_by(|left, right| left.0.cmp(&right.0))
        .map(|(_, entity)| entity.entity_key.clone())
}

fn source_range_contains(owner: &SourceRange, candidate: &SourceRange) -> bool {
    let starts_before =
        (owner.start_line, owner.start_column) <= (candidate.start_line, candidate.start_column);
    let ends_after =
        (owner.end_line, owner.end_column) >= (candidate.end_line, candidate.end_column);
    starts_before && ends_after
}

fn default_hardcoding_guards(specific: &str) -> Vec<String> {
    vec![
        "production_source_only".to_owned(),
        "docs_test_generated_vendor_excluded".to_owned(),
        "raw_literal_not_persisted_or_hashed".to_owned(),
        specific.to_owned(),
    ]
}

fn endpoint_matches(line: &str) -> Vec<(usize, usize, &'static str, &'static str)> {
    let lower = line.to_ascii_lowercase();
    let mut matches = Vec::new();
    for (needle, predicate, shape) in [
        ("https://", "url_with_https_scheme", "https_endpoint"),
        ("http://", "url_with_http_scheme", "http_endpoint"),
    ] {
        let mut offset = 0;
        while let Some(relative) = lower[offset..].find(needle) {
            let start = offset + relative;
            let end = token_end(line, start, 512);
            matches.push((start, end, predicate, shape));
            offset = end.max(start + needle.len());
            if offset >= line.len() {
                break;
            }
        }
    }
    matches
}

fn absolute_path_matches(line: &str) -> Vec<(usize, usize, &'static str, &'static str)> {
    let bytes = line.as_bytes();
    let mut matches = Vec::new();
    for index in 0..bytes.len().saturating_sub(2) {
        if bytes[index].is_ascii_alphabetic()
            && bytes[index + 1] == b':'
            && matches!(bytes[index + 2], b'\\' | b'/')
        {
            matches.push((
                index,
                token_end(line, index, 512),
                "windows_drive_absolute_path",
                "windows_absolute_path",
            ));
        }
    }
    for prefix in [
        "\"/Users/",
        "'/Users/",
        "\"/home/",
        "'/home/",
        "\"/var/",
        "'/var/",
        "\"/etc/",
        "'/etc/",
        "\"/tmp/",
        "'/tmp/",
    ] {
        let mut offset = 0;
        while let Some(relative) = line[offset..].find(prefix) {
            let start = offset + relative + 1;
            let end = token_end(line, start, 512);
            matches.push((start, end, "posix_absolute_path", "posix_absolute_path"));
            offset = end.max(start + prefix.len() - 1);
            if offset >= line.len() {
                break;
            }
        }
    }
    matches
}

fn timeout_retry_limit_match(line: &str) -> Option<(usize, usize)> {
    let lower = line.to_ascii_lowercase();
    if ![
        "timeout", "retry", "retries", "limit", "max_", "max-", "ttl",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        return None;
    }
    let start = line.as_bytes().iter().position(u8::is_ascii_digit)?;
    let end = line.as_bytes()[start..]
        .iter()
        .position(|byte| !byte.is_ascii_digit() && *byte != b'_')
        .map(|relative| start + relative)
        .unwrap_or(line.len());
    Some((start, end))
}

fn raw_command_match(line: &str) -> Option<(usize, usize)> {
    let lower = line.to_ascii_lowercase();
    [
        "command::new(",
        "process::command",
        "std::process::command",
        "shell_execute",
        "cmd.exe /c",
        "powershell -command",
    ]
    .iter()
    .find_map(|needle| {
        lower
            .find(needle)
            .map(|start| (start, start + needle.len()))
    })
}

fn error_literal_match(line: &str) -> Option<(String, usize, usize)> {
    let start = ["Err(\"", "panic!(\"", "anyhow!(\"", "bail!(\""]
        .iter()
        .filter_map(|needle| line.find(needle).map(|index| (index, needle.len())))
        .min_by_key(|(index, _)| *index)?;
    let literal_start = start.0 + start.1;
    let bytes = line.as_bytes();
    let mut index = literal_start;
    let mut escaped = false;
    while index < bytes.len() {
        if bytes[index] == b'"' && !escaped {
            break;
        }
        escaped = bytes[index] == b'\\' && !escaped;
        if bytes[index] != b'\\' {
            escaped = false;
        }
        index += 1;
    }
    if index >= bytes.len() || index == literal_start || index - literal_start > 512 {
        return None;
    }
    let raw = &line[literal_start..index];
    let lower = raw.to_ascii_lowercase();
    if raw.chars().count() < 8
        || [
            "password",
            "secret",
            "token",
            "credential",
            "http://",
            "https://",
            "/users/",
            "/home/",
        ]
        .iter()
        .any(|needle| lower.contains(needle))
        || lower.contains(":\\")
    {
        return None;
    }
    let normalized = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    Some((normalized, literal_start, index))
}

fn config_literal_match(line: &str) -> Option<(String, usize, usize, &'static str)> {
    let lower = line.to_ascii_lowercase();
    if !["config", "setting", "option", "default"]
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return None;
    }
    let assignment = line.find('=').or_else(|| line.find(':'))?;
    let tail = &line[assignment + 1..];
    if let Some(relative_quote) = tail.find(['\"', '\'']) {
        let quote_start = assignment + 1 + relative_quote;
        let quote = line.as_bytes()[quote_start];
        let rest = &line[quote_start + 1..];
        let relative_end = rest.as_bytes().iter().position(|byte| *byte == quote)?;
        if !(3..=128).contains(&relative_end) {
            return None;
        }
        let end = quote_start + 1 + relative_end;
        let normalized = rest[..relative_end]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase();
        if normalized.is_empty() {
            return None;
        }
        return Some((normalized, quote_start + 1, end, "config_string_literal"));
    }
    let digit_start = tail.find(|character: char| character.is_ascii_digit())?;
    let start = assignment + 1 + digit_start;
    let length = line[start..]
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '_')
        .map(char::len_utf8)
        .sum::<usize>();
    if length == 0 || length > 32 {
        return None;
    }
    let end = start + length;
    Some((
        line[start..end].replace('_', ""),
        start,
        end,
        "config_numeric_literal",
    ))
}

fn token_end(line: &str, start: usize, max_len: usize) -> usize {
    let mut limit = line.len().min(start.saturating_add(max_len));
    while limit > start && !line.is_char_boundary(limit) {
        limit -= 1;
    }
    line.as_bytes()[start..limit]
        .iter()
        .position(|byte| {
            byte.is_ascii_whitespace()
                || matches!(*byte, b'"' | b'\'' | b')' | b']' | b'}' | b',' | b';')
        })
        .map(|relative| start + relative)
        .unwrap_or(limit)
        .max(start + 1)
}

fn length_bucket(length: usize) -> String {
    match length {
        0..=8 => "1-8",
        9..=32 => "9-32",
        33..=128 => "33-128",
        _ => "129+",
    }
    .to_owned()
}

#[derive(Default)]
struct ProjectionAccumulator {
    source_entries: Vec<SourceEntry>,
    entities: Vec<IndexEntity>,
    edges: Vec<IndexEdge>,
    symbols: Vec<Symbol>,
    references: Vec<SymbolReference>,
    partitions: Vec<IndexPartition>,
    limitations: Vec<IndexLimitation>,
}

struct PreviousProjectionIndex<'a> {
    source_entries: BTreeMap<String, &'a SourceEntry>,
    entities: BTreeMap<String, Vec<&'a IndexEntity>>,
    edges: BTreeMap<String, Vec<&'a IndexEdge>>,
    symbols: BTreeMap<String, Vec<&'a Symbol>>,
    references: BTreeMap<String, Vec<&'a SymbolReference>>,
    partitions: BTreeMap<String, Vec<&'a IndexPartition>>,
}

impl<'a> PreviousProjectionIndex<'a> {
    fn new(previous: &'a CodeIndexProjection) -> Self {
        let mut index = Self {
            source_entries: BTreeMap::new(),
            entities: BTreeMap::new(),
            edges: BTreeMap::new(),
            symbols: BTreeMap::new(),
            references: BTreeMap::new(),
            partitions: BTreeMap::new(),
        };
        for source in &previous.source_entries {
            index
                .source_entries
                .insert(source.path.as_str().to_owned(), source);
        }
        for entity in &previous.entities {
            if let Some(source_id) = &entity.canonical_source_id {
                index
                    .entities
                    .entry(source_id.as_str().to_owned())
                    .or_default()
                    .push(entity);
            }
        }
        for edge in &previous.edges {
            index
                .edges
                .entry(edge.evidence_source_id.as_str().to_owned())
                .or_default()
                .push(edge);
        }
        for symbol in &previous.symbols {
            index
                .symbols
                .entry(symbol.canonical_source_id.as_str().to_owned())
                .or_default()
                .push(symbol);
        }
        for reference in &previous.references {
            index
                .references
                .entry(reference.from_source_id.as_str().to_owned())
                .or_default()
                .push(reference);
        }
        for partition in &previous.snapshot.partitions {
            if let Some((path, _)) = partition.partition_key.rsplit_once(':') {
                index
                    .partitions
                    .entry(path.to_owned())
                    .or_default()
                    .push(partition);
            }
        }
        index
    }
}

fn validate_request(request: &CodeIndexBuildRequest<'_>) -> Result<(), ProjectError> {
    if request.checkout.project_id != request.project.project_id
        || !request
            .catalog_snapshot
            .project_refs
            .iter()
            .any(|item| item.project_id == request.project.project_id)
        || !request.catalog_snapshot.checkout_refs.iter().any(|item| {
            item.checkout_id == request.checkout.checkout_id
                && item.project_id == request.project.project_id
        })
        || request.policy.required_tier > request.policy.max_tier
        || request.policy.max_text_tokens_per_file == 0
    {
        return Err(ProjectError::InvalidManifest);
    }
    Ok(())
}

fn adapter_set_fingerprint(
    request: &CodeIndexBuildRequest<'_>,
) -> Result<Sha256Hash, ProjectError> {
    let mut adapters = vec![serde_json::json!({
        "language_id":"text",
        "tier":"text",
        "fingerprint":versioned_fingerprint(
            "star.text-index-adapter",
            request.policy.text_adapter_version,
            &serde_json::json!({"tokenization":"unicode_alphanumeric_underscore"}),
        ).map_err(|_| ProjectError::Fingerprint)?,
    })];
    adapters.extend(request.syntax_adapters.iter().map(|adapter| {
        serde_json::json!({"language_id":adapter.language_id(),"tier":"syntax","fingerprint":adapter.fingerprint()})
    }));
    adapters.extend(request.semantic_adapters.iter().map(|adapter| {
        serde_json::json!({"language_id":adapter.language_id(),"tier":"semantic","fingerprint":adapter.fingerprint()})
    }));
    adapters.sort_by_key(|value| value.to_string());
    versioned_fingerprint("star.index-adapter-set", 1, &adapters)
        .map_err(|_| ProjectError::Fingerprint)
}

fn source_entry(
    request: &CodeIndexBuildRequest<'_>,
    file: &FileObservation,
) -> Result<SourceEntry, ProjectError> {
    let identity = versioned_fingerprint(
        "star.identity.canonical-source",
        1,
        &serde_json::json!({
            "project_id":request.project.project_id,
            "source_kind":"file",
            "path":file.path,
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    let source_class = classify_source(file.path.as_str());
    let facets = source_facets(file.path.as_str(), source_class);
    let language_id = file
        .language_id
        .clone()
        .unwrap_or_else(|| "text".to_owned());
    let analysis_eligible = class_text_eligible(source_class);
    let content_fingerprint = versioned_fingerprint(
        "star.source-entry",
        1,
        &serde_json::json!({
            "canonical_source_id":CanonicalSourceId::from_fingerprint(&identity),
            "path":file.path,
            "content_sha256":file.content_sha256,
            "size_bytes":file.size_bytes,
            "source_class":source_class,
            "facets":facets,
            "language_id":language_id,
            "encoding":if file.text.is_some() {"utf-8"} else {"binary_or_non_utf8"},
            "owner_project_id":request.project.project_id,
            "owner_checkout_id":request.checkout.checkout_id,
            "analysis_eligible":analysis_eligible,
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    Ok(SourceEntry {
        canonical_source_id: CanonicalSourceId::from_fingerprint(&identity),
        path: file.path.clone(),
        content_sha256: file.content_sha256.clone(),
        size_bytes: file.size_bytes,
        source_class,
        facets,
        language_id,
        encoding: if file.text.is_some() {
            "utf-8".to_owned()
        } else {
            "binary_or_non_utf8".to_owned()
        },
        owner_project_id: request.project.project_id.clone(),
        owner_checkout_id: request.checkout.checkout_id.clone(),
        analysis_eligible,
        content_fingerprint,
    })
}

fn source_index_entity(
    file: &FileObservation,
    source: &SourceEntry,
) -> Result<IndexEntity, ProjectError> {
    let source_range = file.text.as_deref().map(|text| {
        let mut lines = text.lines();
        let mut line_count = 0_u32;
        let mut last_column = 1_u32;
        for line in &mut lines {
            line_count = line_count.saturating_add(1);
            last_column = u32::try_from(line.chars().count().saturating_add(1)).unwrap_or(u32::MAX);
        }
        SourceRange {
            start_line: 1,
            start_column: 1,
            end_line: line_count.max(1),
            end_column: last_column,
        }
    });
    let entity_key = format!("source:{}", source.canonical_source_id.as_str());
    let content_fingerprint = versioned_fingerprint(
        "star.index-source-entity",
        1,
        &serde_json::json!({
            "entity_key":entity_key,
            "canonical_source_id":source.canonical_source_id,
            "path":source.path,
            "source_content_sha256":source.content_sha256,
            "source_range":source_range,
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    Ok(IndexEntity {
        entity_key,
        kind: IndexEntityKind::Source,
        canonical_source_id: Some(source.canonical_source_id.clone()),
        symbol_id: None,
        qualified_name: source.path.as_str().to_owned(),
        source_range,
        tier: IndexTier::Text,
        confidence: "high".to_owned(),
        content_fingerprint,
    })
}

fn classify_source(path: &str) -> SourceClass {
    let normalized = path.to_ascii_lowercase();
    let segments: Vec<_> = normalized.split('/').collect();
    if segments
        .iter()
        .any(|part| matches!(*part, "vendor" | "third_party" | "node_modules" | ".cargo"))
    {
        SourceClass::Vendor
    } else if segments
        .iter()
        .any(|part| matches!(*part, "generated" | "gen"))
        || normalized.ends_with(".generated.rs")
    {
        SourceClass::Generated
    } else if segments
        .iter()
        .any(|part| matches!(*part, ".cache" | "cache"))
    {
        SourceClass::Cache
    } else if segments
        .iter()
        .any(|part| matches!(*part, "target" | "dist" | "build" | "out"))
    {
        SourceClass::Output
    } else if segments
        .iter()
        .any(|part| matches!(*part, "test" | "tests"))
        || normalized.ends_with("_test.rs")
    {
        SourceClass::Test
    } else if segments
        .iter()
        .any(|part| matches!(*part, "migration" | "migrations"))
    {
        SourceClass::Migration
    } else if segments.contains(&"schemas")
        || normalized.ends_with(".schema.json")
        || normalized.ends_with(".proto")
        || normalized.ends_with(".graphql")
    {
        SourceClass::Schema
    } else if segments
        .iter()
        .any(|part| matches!(*part, "doc" | "docs" | "examples"))
        || normalized.ends_with("readme.md")
    {
        SourceClass::Docs
    } else if segments
        .iter()
        .any(|part| matches!(*part, "config" | ".config" | ".github"))
        || normalized.ends_with("cargo.toml")
        || normalized.ends_with("rust-toolchain.toml")
        || normalized.ends_with(".toml")
        || normalized.ends_with(".yaml")
        || normalized.ends_with(".yml")
    {
        SourceClass::Config
    } else if matches!(
        normalized.rsplit_once('.').map(|(_, extension)| extension),
        Some("rs" | "ps1" | "ts" | "tsx" | "js" | "jsx" | "py")
    ) {
        SourceClass::Source
    } else {
        SourceClass::Unknown
    }
}

fn source_facets(path: &str, class: SourceClass) -> Vec<String> {
    let normalized = path.to_ascii_lowercase();
    let mut facets = BTreeSet::new();
    facets.insert(format!("class:{class:?}").to_ascii_lowercase());
    if normalized
        .split('/')
        .any(|part| matches!(part, "fixture" | "fixtures"))
    {
        facets.insert("fixture".to_owned());
    }
    if normalized.split('/').any(|part| part == "examples") {
        facets.insert("docs_example".to_owned());
    }
    facets.into_iter().collect()
}

fn class_text_eligible(class: SourceClass) -> bool {
    !matches!(
        class,
        SourceClass::Vendor | SourceClass::Generated | SourceClass::Cache | SourceClass::Output
    )
}

fn class_syntax_eligible(class: SourceClass) -> bool {
    matches!(
        class,
        SourceClass::Source
            | SourceClass::Test
            | SourceClass::Config
            | SourceClass::Schema
            | SourceClass::Migration
    )
}

fn class_semantic_eligible(class: SourceClass) -> bool {
    matches!(
        class,
        SourceClass::Source | SourceClass::Test | SourceClass::Migration
    )
}

fn syntax_eligible(source: &SourceEntry, file: &FileObservation) -> bool {
    source.analysis_eligible
        && file.text.is_some()
        && !source.facets.iter().any(|facet| facet == "fixture")
        && class_syntax_eligible(source.source_class)
}

fn semantic_eligible(source: &SourceEntry, file: &FileObservation) -> bool {
    source.analysis_eligible
        && file.text.is_some()
        && !source.facets.iter().any(|facet| facet == "fixture")
        && class_semantic_eligible(source.source_class)
}

fn reuse_source_projection(
    previous: &PreviousProjectionIndex<'_>,
    source: &SourceEntry,
    scan_run_id: &ScanRunId,
    workspace_snapshot_id: &star_contracts::ids::WorkspaceSnapshotId,
    output: &mut ProjectionAccumulator,
) -> BTreeSet<IndexPartitionKind> {
    let source_partitions = previous
        .partitions
        .get(source.path.as_str())
        .map(Vec::as_slice)
        .unwrap_or_default();
    let optional_is_partial = source_partitions.iter().any(|partition| {
        matches!(
            partition.kind,
            IndexPartitionKind::Syntax | IndexPartitionKind::Semantic
        ) && partition.state == IndexPartitionState::Incomplete
    });
    let reusable_kinds = source_partitions
        .iter()
        .filter(|partition| {
            matches!(
                partition.state,
                IndexPartitionState::Succeeded | IndexPartitionState::Reused
            ) && (!optional_is_partial
                || !matches!(
                    partition.kind,
                    IndexPartitionKind::Syntax | IndexPartitionKind::Semantic
                ))
        })
        .map(|partition| partition.kind)
        .collect::<BTreeSet<_>>();
    let reusable_tiers = reusable_kinds
        .iter()
        .filter_map(|kind| match kind {
            IndexPartitionKind::Classification => None,
            IndexPartitionKind::Text => Some(IndexTier::Text),
            IndexPartitionKind::Syntax => Some(IndexTier::Syntax),
            IndexPartitionKind::Semantic => Some(IndexTier::Semantic),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let source_key = source.canonical_source_id.as_str();
    let source_entities = previous
        .entities
        .get(source_key)
        .map(Vec::as_slice)
        .unwrap_or_default();
    output.entities.extend(
        source_entities
            .iter()
            .filter(|item| reusable_tiers.contains(&item.tier))
            .map(|item| (*item).clone()),
    );
    output.edges.extend(
        previous
            .edges
            .get(source_key)
            .into_iter()
            .flatten()
            .filter(|item| reusable_tiers.contains(&item.tier))
            .map(|item| (*item).clone()),
    );
    let reusable_symbol_ids = source_entities
        .iter()
        .filter(|item| reusable_tiers.contains(&item.tier))
        .filter_map(|item| item.symbol_id.clone())
        .collect::<BTreeSet<_>>();
    output.symbols.extend(
        previous
            .symbols
            .get(source_key)
            .into_iter()
            .flatten()
            .filter_map(|item| {
                if !reusable_symbol_ids.contains(&item.symbol_id) {
                    return None;
                }
                let mut rebound = (*item).clone();
                rebound.scan_run_id = scan_run_id.clone();
                rebound.workspace_snapshot_id = workspace_snapshot_id.clone();
                Some(rebound)
            }),
    );
    if reusable_kinds.contains(&IndexPartitionKind::Syntax)
        || reusable_kinds.contains(&IndexPartitionKind::Semantic)
    {
        output.references.extend(
            previous
                .references
                .get(source_key)
                .into_iter()
                .flatten()
                .map(|item| {
                    let mut rebound = (*item).clone();
                    rebound.scan_run_id = scan_run_id.clone();
                    rebound.workspace_snapshot_id = workspace_snapshot_id.clone();
                    rebound
                }),
        );
    }
    for mut partition in source_partitions
        .iter()
        .filter(|partition| reusable_kinds.contains(&partition.kind))
        .map(|partition| (**partition).clone())
    {
        output.limitations.extend(partition.limitations.clone());
        partition.state = IndexPartitionState::Reused;
        partition.cache_hit = true;
        output.partitions.push(partition);
    }
    reusable_kinds
}

fn index_classification_source(
    request: &CodeIndexBuildRequest<'_>,
    source: &SourceEntry,
    output: &mut ProjectionAccumulator,
) -> Result<(), ProjectError> {
    let input_fingerprint = versioned_fingerprint(
        "star.index-partition-input.classification",
        1,
        &serde_json::json!({
            "source_content_fingerprint":source.content_fingerprint,
            "classification_contract_version":request.policy.classification_contract_version,
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    let output_fingerprint = versioned_fingerprint(
        "star.index-partition-output.classification",
        1,
        &serde_json::json!({
            "source_class":source.source_class,
            "facets":source.facets,
            "language_id":source.language_id,
            "analysis_eligible":source.analysis_eligible,
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    output.partitions.push(IndexPartition {
        partition_key: format!("{}:classification", source.path.as_str()),
        kind: IndexPartitionKind::Classification,
        required: true,
        requested_tier: IndexTier::Text,
        used_tier: None,
        state: IndexPartitionState::Succeeded,
        input_fingerprint,
        output_fingerprint: Some(output_fingerprint),
        target_count: 1,
        indexed_count: 1,
        failed_count: 0,
        excluded_count: 0,
        cache_hit: false,
        limitations: Vec::new(),
    });
    Ok(())
}

fn index_text_source(
    request: &CodeIndexBuildRequest<'_>,
    file: &FileObservation,
    source: &SourceEntry,
    output: &mut ProjectionAccumulator,
) -> Result<(), ProjectError> {
    let partition_key = format!("{}:text", source.path.as_str());
    let input_fingerprint = versioned_fingerprint(
        "star.index-partition-input.text",
        1,
        &serde_json::json!({
            "source_content_sha256":source.content_sha256,
            "analysis_eligible":source.analysis_eligible,
            "adapter_version":request.policy.text_adapter_version,
            "max_tokens":request.policy.max_text_tokens_per_file,
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    if !source.analysis_eligible || file.text.is_none() {
        let output_fingerprint = versioned_fingerprint(
            "star.index-partition-output.text",
            1,
            &serde_json::json!({"excluded":true}),
        )
        .map_err(|_| ProjectError::Fingerprint)?;
        output.partitions.push(IndexPartition {
            partition_key,
            kind: IndexPartitionKind::Text,
            required: true,
            requested_tier: IndexTier::Text,
            used_tier: Some(IndexTier::Text),
            state: IndexPartitionState::Succeeded,
            input_fingerprint,
            output_fingerprint: Some(output_fingerprint),
            target_count: 0,
            indexed_count: 0,
            failed_count: 0,
            excluded_count: 1,
            cache_hit: false,
            limitations: Vec::new(),
        });
        return Ok(());
    }
    let tokens = text_tokens(
        file.text.as_deref().unwrap_or_default(),
        request.policy.max_text_tokens_per_file,
    );
    let truncated = tokens.truncated;
    let first_entity = output.entities.len();
    let first_edge = output.edges.len();
    let mut entity_keys = BTreeMap::new();
    for occurrence in &tokens.items {
        let token_identity = versioned_fingerprint(
            "star.index-text-token",
            1,
            &serde_json::json!({
                "canonical_source_id":source.canonical_source_id,
                "token":occurrence.token,
            }),
        )
        .map_err(|_| ProjectError::Fingerprint)?;
        let entity_key = format!("text:{}", token_identity.as_str());
        if !entity_keys.contains_key(&occurrence.token) {
            output.entities.push(IndexEntity {
                entity_key: entity_key.clone(),
                kind: IndexEntityKind::TextToken,
                canonical_source_id: Some(source.canonical_source_id.clone()),
                symbol_id: None,
                qualified_name: occurrence.token.clone(),
                source_range: None,
                tier: IndexTier::Text,
                confidence: "literal".to_owned(),
                content_fingerprint: token_identity,
            });
            entity_keys.insert(occurrence.token.clone(), entity_key.clone());
        }
        let edge_fingerprint = versioned_fingerprint(
            "star.index-text-occurrence",
            1,
            &serde_json::json!({
                "canonical_source_id":source.canonical_source_id,
                "entity_key":entity_key,
                "range":occurrence.range,
            }),
        )
        .map_err(|_| ProjectError::Fingerprint)?;
        output.edges.push(IndexEdge {
            edge_key: format!("edge:{}", edge_fingerprint.as_str()),
            from_entity_key: format!("source:{}", source.canonical_source_id.as_str()),
            to_entity_key: Some(entity_key),
            unresolved_target: None,
            relation: IndexRelation::TextOccurrence,
            evidence_source_id: source.canonical_source_id.clone(),
            evidence_range: Some(occurrence.range.clone()),
            tier: IndexTier::Text,
            resolution: SymbolResolution::Unresolved,
            confidence: "literal".to_owned(),
            content_fingerprint: edge_fingerprint,
        });
    }
    let mut limitations = Vec::new();
    if truncated {
        let limitation = limitation("INDEX_RESOURCE_LIMIT", Some(source.path.as_str()));
        output.limitations.push(limitation.clone());
        limitations.push(limitation);
    }
    let output_fingerprint = versioned_fingerprint(
        "star.index-partition-output.text",
        1,
        &serde_json::json!({
            "entities":&output.entities[first_entity..],
            "edges":&output.edges[first_edge..],
            "limitations":limitations,
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    output.partitions.push(IndexPartition {
        partition_key,
        kind: IndexPartitionKind::Text,
        required: true,
        requested_tier: IndexTier::Text,
        used_tier: Some(IndexTier::Text),
        state: if truncated {
            IndexPartitionState::Incomplete
        } else {
            IndexPartitionState::Succeeded
        },
        input_fingerprint,
        output_fingerprint: Some(output_fingerprint),
        target_count: tokens.items.len() as u64,
        indexed_count: tokens.items.len() as u64,
        failed_count: u64::from(truncated),
        excluded_count: 0,
        cache_hit: false,
        limitations,
    });
    Ok(())
}

fn index_classification_exclusion(
    request: &CodeIndexBuildRequest<'_>,
    source: &SourceEntry,
    kind: IndexPartitionKind,
    tier: IndexTier,
    output: &mut ProjectionAccumulator,
) -> Result<(), ProjectError> {
    let suffix = match kind {
        IndexPartitionKind::Syntax => "syntax",
        IndexPartitionKind::Semantic => "semantic",
        _ => return Err(ProjectError::InvalidManifest),
    };
    let input_fingerprint = versioned_fingerprint(
        "star.index-partition-input.classification-exclusion",
        1,
        &serde_json::json!({
            "source":source.content_fingerprint,
            "kind":kind,
            "tier":tier,
            "classification_contract_version":request.policy.classification_contract_version,
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    let limitation = limitation(
        "INDEX_TIER_EXCLUDED_BY_CLASSIFICATION",
        Some(source.path.as_str()),
    );
    let output_fingerprint = versioned_fingerprint(
        "star.index-partition-output.classification-exclusion",
        1,
        &serde_json::json!({"excluded":true,"limitation":limitation}),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    output.limitations.push(limitation.clone());
    output.partitions.push(IndexPartition {
        partition_key: format!("{}:{suffix}", source.path.as_str()),
        kind,
        required: request.policy.required_tier >= tier,
        requested_tier: tier,
        used_tier: None,
        state: IndexPartitionState::Succeeded,
        input_fingerprint,
        output_fingerprint: Some(output_fingerprint),
        target_count: 1,
        indexed_count: 0,
        failed_count: 0,
        excluded_count: 1,
        cache_hit: false,
        limitations: vec![limitation],
    });
    Ok(())
}

fn index_syntax_partition(
    request: &CodeIndexBuildRequest<'_>,
    file: &FileObservation,
    source: &SourceEntry,
    adapter: Option<&dyn SyntaxAdapter>,
    output: &mut ProjectionAccumulator,
) -> Result<(), ProjectError> {
    let key = format!("{}:syntax", source.path.as_str());
    let adapter_fingerprint = adapter
        .map(SyntaxAdapter::fingerprint)
        .unwrap_or_else(|| Sha256Hash::digest(b"syntax-adapter-unavailable"));
    let input = versioned_fingerprint(
        "star.index-partition-input.syntax",
        1,
        &serde_json::json!({"source":source.content_sha256,"adapter":adapter_fingerprint}),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    let Some(adapter) = adapter else {
        let item = limitation("INDEX_LANGUAGE_UNSUPPORTED", Some(source.path.as_str()));
        output.limitations.push(item.clone());
        output.partitions.push(unavailable_partition(
            key,
            IndexPartitionKind::Syntax,
            IndexTier::Syntax,
            request.policy.required_tier >= IndexTier::Syntax,
            input,
            item,
        ));
        return Ok(());
    };
    match adapter.analyze(file) {
        Ok(analysis) => {
            let before_symbols = output.symbols.len();
            let before_references = output.references.len();
            append_adapter_analysis(
                request,
                source,
                IndexTier::Syntax,
                analysis.definitions,
                analysis.references,
                output,
            )?;
            output.limitations.extend(analysis.limitations.clone());
            let fingerprint = versioned_fingerprint(
                "star.index-partition-output.syntax",
                1,
                &serde_json::json!({
                    "symbols":output.symbols[before_symbols..].iter().map(|item| (&item.symbol_id,&item.content_fingerprint)).collect::<Vec<_>>(),
                    "references":output.references[before_references..].iter().map(|item| &item.symbol_reference_id).collect::<Vec<_>>(),
                    "limitations":analysis.limitations,
                }),
            )
            .map_err(|_| ProjectError::Fingerprint)?;
            output.partitions.push(IndexPartition {
                partition_key: key,
                kind: IndexPartitionKind::Syntax,
                required: request.policy.required_tier >= IndexTier::Syntax,
                requested_tier: IndexTier::Syntax,
                used_tier: Some(IndexTier::Syntax),
                state: if analysis.limitations.is_empty() {
                    IndexPartitionState::Succeeded
                } else {
                    IndexPartitionState::Incomplete
                },
                input_fingerprint: input,
                output_fingerprint: Some(fingerprint),
                target_count: 1,
                indexed_count: 1,
                failed_count: 0,
                excluded_count: 0,
                cache_hit: false,
                limitations: analysis.limitations,
            });
        }
        Err(failure) => {
            let code = match failure {
                AdapterFailure::ParseFailed => "INDEX_PARSE_FAILED",
                AdapterFailure::ResourceLimit => "INDEX_RESOURCE_LIMIT",
                AdapterFailure::Unavailable => "INDEX_LANGUAGE_UNSUPPORTED",
            };
            let item = limitation(code, Some(source.path.as_str()));
            output.limitations.push(item.clone());
            output.partitions.push(unavailable_partition(
                key,
                IndexPartitionKind::Syntax,
                IndexTier::Syntax,
                request.policy.required_tier >= IndexTier::Syntax,
                input,
                item,
            ));
        }
    }
    Ok(())
}

fn index_semantic_partition(
    request: &CodeIndexBuildRequest<'_>,
    file: &FileObservation,
    source: &SourceEntry,
    adapter: Option<&dyn SemanticAdapter>,
    prepare_failure: Option<AdapterFailure>,
    output: &mut ProjectionAccumulator,
) -> Result<(), ProjectError> {
    let key = format!("{}:semantic", source.path.as_str());
    let adapter_fingerprint = adapter
        .map(SemanticAdapter::fingerprint)
        .unwrap_or_else(|| Sha256Hash::digest(b"semantic-adapter-unavailable"));
    let input = versioned_fingerprint(
        "star.index-partition-input.semantic",
        1,
        &serde_json::json!({"source":source.content_sha256,"adapter":adapter_fingerprint}),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    let Some(adapter) = adapter else {
        let item = limitation("INDEX_SEMANTIC_UNAVAILABLE", Some(source.path.as_str()));
        output.limitations.push(item.clone());
        output.partitions.push(unavailable_partition(
            key,
            IndexPartitionKind::Semantic,
            IndexTier::Semantic,
            request.policy.required_tier >= IndexTier::Semantic,
            input,
            item,
        ));
        return Ok(());
    };
    if let Some(failure) = prepare_failure {
        let code = match failure {
            AdapterFailure::ParseFailed => "INDEX_SEMANTIC_ANALYSIS_FAILED",
            AdapterFailure::ResourceLimit => "INDEX_RESOURCE_LIMIT",
            AdapterFailure::Unavailable => "INDEX_SEMANTIC_UNAVAILABLE",
        };
        let item = limitation(code, Some(source.path.as_str()));
        output.limitations.push(item.clone());
        output.partitions.push(unavailable_partition(
            key,
            IndexPartitionKind::Semantic,
            IndexTier::Semantic,
            request.policy.required_tier >= IndexTier::Semantic,
            input,
            item,
        ));
        return Ok(());
    }
    match adapter.analyze(file) {
        Ok(analysis) => {
            let before_symbols = output.symbols.len();
            let before_references = output.references.len();
            append_adapter_analysis(
                request,
                source,
                IndexTier::Semantic,
                analysis.definitions,
                analysis.references,
                output,
            )?;
            output.limitations.extend(analysis.limitations.clone());
            let fingerprint = versioned_fingerprint(
                "star.index-partition-output.semantic",
                1,
                &serde_json::json!({
                    "symbols":output.symbols[before_symbols..].iter().map(|item| (&item.symbol_id,&item.content_fingerprint)).collect::<Vec<_>>(),
                    "references":output.references[before_references..].iter().map(|item| &item.symbol_reference_id).collect::<Vec<_>>(),
                    "limitations":analysis.limitations,
                }),
            )
            .map_err(|_| ProjectError::Fingerprint)?;
            output.partitions.push(IndexPartition {
                partition_key: key,
                kind: IndexPartitionKind::Semantic,
                required: request.policy.required_tier >= IndexTier::Semantic,
                requested_tier: IndexTier::Semantic,
                used_tier: Some(IndexTier::Semantic),
                state: if analysis.limitations.is_empty() {
                    IndexPartitionState::Succeeded
                } else {
                    IndexPartitionState::Incomplete
                },
                input_fingerprint: input,
                output_fingerprint: Some(fingerprint),
                target_count: 1,
                indexed_count: 1,
                failed_count: 0,
                excluded_count: 0,
                cache_hit: false,
                limitations: analysis.limitations,
            });
        }
        Err(failure) => {
            let code = match failure {
                AdapterFailure::ParseFailed => "INDEX_SEMANTIC_ANALYSIS_FAILED",
                AdapterFailure::ResourceLimit => "INDEX_RESOURCE_LIMIT",
                AdapterFailure::Unavailable => "INDEX_SEMANTIC_UNAVAILABLE",
            };
            let item = limitation(code, Some(source.path.as_str()));
            output.limitations.push(item.clone());
            output.partitions.push(unavailable_partition(
                key,
                IndexPartitionKind::Semantic,
                IndexTier::Semantic,
                request.policy.required_tier >= IndexTier::Semantic,
                input,
                item,
            ));
        }
    }
    Ok(())
}

fn unavailable_partition(
    partition_key: String,
    kind: IndexPartitionKind,
    tier: IndexTier,
    required: bool,
    input_fingerprint: Sha256Hash,
    limitation: IndexLimitation,
) -> IndexPartition {
    IndexPartition {
        partition_key,
        kind,
        required,
        requested_tier: tier,
        used_tier: Some(IndexTier::Text),
        state: IndexPartitionState::Incomplete,
        input_fingerprint,
        output_fingerprint: None,
        target_count: 1,
        indexed_count: 0,
        failed_count: 1,
        excluded_count: 0,
        cache_hit: false,
        limitations: vec![limitation],
    }
}

fn append_adapter_analysis(
    request: &CodeIndexBuildRequest<'_>,
    source: &SourceEntry,
    tier: IndexTier,
    mut definitions: Vec<SyntaxDefinition>,
    mut references: Vec<SyntaxReference>,
    output: &mut ProjectionAccumulator,
) -> Result<(), ProjectError> {
    definitions.sort_by(|left, right| {
        (
            &left.qualified_name,
            left.range.start_line,
            left.range.start_column,
        )
            .cmp(&(
                &right.qualified_name,
                right.range.start_line,
                right.range.start_column,
            ))
    });
    references.sort_by(|left, right| {
        (
            &left.target_name,
            left.range.start_line,
            left.range.start_column,
        )
            .cmp(&(
                &right.target_name,
                right.range.start_line,
                right.range.start_column,
            ))
    });
    let mut symbols_by_name = BTreeMap::<String, Vec<SymbolId>>::new();
    for definition in definitions {
        let fingerprint = versioned_fingerprint(
            "star.identity.symbol",
            1,
            &serde_json::json!({
                "project_id":request.project.project_id,
                "language_id":source.language_id,
                "symbol_kind":definition.symbol_kind,
                "qualified_name":definition.qualified_name,
                "canonical_source_id":source.canonical_source_id,
                "tier":tier,
            }),
        )
        .map_err(|_| ProjectError::Fingerprint)?;
        let symbol_id = SymbolId::from_fingerprint(&fingerprint);
        symbols_by_name
            .entry(definition.qualified_name.clone())
            .or_default()
            .push(symbol_id.clone());
        output.symbols.push(Symbol {
            schema_id: "star.symbol".to_owned(),
            schema_version: 1,
            symbol_id: symbol_id.clone(),
            project_id: request.project.project_id.clone(),
            canonical_source_id: source.canonical_source_id.clone(),
            language_id: source.language_id.clone(),
            symbol_kind: definition.symbol_kind,
            qualified_name: definition.qualified_name.clone(),
            signature_fingerprint: None,
            declaration_range: definition.range.clone(),
            visibility: definition.visibility,
            workspace_snapshot_id: request
                .observation
                .workspace_snapshot_id(&request.project.project_id)?,
            scan_run_id: request.scan_run_id.clone(),
            content_fingerprint: fingerprint.clone(),
        });
        output.entities.push(IndexEntity {
            entity_key: format!("symbol:{}", symbol_id.as_str()),
            kind: IndexEntityKind::Symbol,
            canonical_source_id: Some(source.canonical_source_id.clone()),
            symbol_id: Some(symbol_id),
            qualified_name: definition.qualified_name,
            source_range: Some(definition.range),
            tier,
            confidence: "confirmed_definition".to_owned(),
            content_fingerprint: fingerprint,
        });
    }
    for reference in references {
        let candidates = symbols_by_name.get(&reference.target_name);
        let (to_symbol_id, resolution) = match (reference.resolution, candidates) {
            (SymbolResolution::Resolved, Some(values)) if values.len() == 1 => {
                (Some(values[0].clone()), SymbolResolution::Resolved)
            }
            (SymbolResolution::Resolved, Some(_)) => (None, SymbolResolution::Ambiguous),
            (SymbolResolution::Resolved, None) => (None, SymbolResolution::Unresolved),
            (other, _) => (None, other),
        };
        let fingerprint = versioned_fingerprint(
            "star.identity.symbol-reference",
            1,
            &serde_json::json!({
                "project_id":request.project.project_id,
                "from_source_id":source.canonical_source_id,
                "from_range":reference.range,
                "to_symbol_id":to_symbol_id,
                "unresolved_target":if to_symbol_id.is_none() {Some(&reference.target_name)} else {None},
                "reference_kind":reference.reference_kind,
                "resolution":resolution,
                "tier":tier,
            }),
        )
        .map_err(|_| ProjectError::Fingerprint)?;
        let unresolved_target = to_symbol_id.is_none().then_some(reference.target_name);
        output.references.push(SymbolReference {
            schema_id: "star.symbol-reference".to_owned(),
            schema_version: 1,
            symbol_reference_id: SymbolReferenceId::from_fingerprint(&fingerprint),
            project_id: request.project.project_id.clone(),
            from_symbol_id: None,
            from_source_id: source.canonical_source_id.clone(),
            from_range: reference.range,
            to_symbol_id,
            unresolved_target,
            reference_kind: reference.reference_kind,
            resolution,
            workspace_snapshot_id: request
                .observation
                .workspace_snapshot_id(&request.project.project_id)?,
            scan_run_id: request.scan_run_id.clone(),
        });
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct TextOccurrence {
    token: String,
    range: SourceRange,
}

struct TextTokenResult {
    items: Vec<TextOccurrence>,
    truncated: bool,
}

fn text_tokens(text: &str, limit: usize) -> TextTokenResult {
    let mut items = Vec::new();
    let mut truncated = false;
    for (line_index, line) in text.lines().enumerate() {
        let mut start = None;
        let chars: Vec<_> = line.char_indices().collect();
        for index in 0..=chars.len() {
            let is_token = chars
                .get(index)
                .is_some_and(|(_, character)| character.is_alphanumeric() || *character == '_');
            if is_token && start.is_none() {
                start = Some(index);
            }
            if !is_token && let Some(start_index) = start.take() {
                if items.len() == limit {
                    truncated = true;
                    return TextTokenResult { items, truncated };
                }
                let start_byte = chars[start_index].0;
                let end_byte = chars.get(index).map_or(line.len(), |item| item.0);
                let token = line[start_byte..end_byte].to_owned();
                let start_column = line[..start_byte].chars().count() as u32 + 1;
                let end_column = start_column + token.chars().count() as u32;
                items.push(TextOccurrence {
                    token,
                    range: SourceRange {
                        start_line: line_index as u32 + 1,
                        start_column,
                        end_line: line_index as u32 + 1,
                        end_column,
                    },
                });
            }
        }
    }
    TextTokenResult { items, truncated }
}

fn coverage(sources: &[SourceEntry], partitions: &[IndexPartition]) -> Vec<IndexCoverage> {
    let by_path: BTreeMap<_, _> = sources
        .iter()
        .map(|source| (source.path.as_str(), source))
        .collect();
    let mut grouped = BTreeMap::<(SourceClass, String, IndexTier), IndexCoverage>::new();
    for partition in partitions {
        let Some((path, _)) = partition.partition_key.rsplit_once(':') else {
            continue;
        };
        let Some(source) = by_path.get(path) else {
            continue;
        };
        let tier = partition.requested_tier;
        let entry = grouped
            .entry((source.source_class, source.language_id.clone(), tier))
            .or_insert(IndexCoverage {
                source_class: source.source_class,
                language_id: source.language_id.clone(),
                tier,
                target_count: 0,
                succeeded_count: 0,
                failed_count: 0,
                excluded_count: 0,
            });
        entry.target_count += partition.target_count;
        entry.succeeded_count += partition.indexed_count;
        entry.failed_count += partition.failed_count;
        entry.excluded_count += partition.excluded_count;
    }
    grouped.into_values().collect()
}

fn partition_unavailable(partition: &IndexPartition) -> bool {
    partition.limitations.iter().any(|limitation| {
        matches!(
            limitation.code.as_str(),
            "INDEX_LANGUAGE_UNSUPPORTED" | "INDEX_SEMANTIC_UNAVAILABLE"
        )
    })
}

fn limitation(code: &str, scope: Option<&str>) -> IndexLimitation {
    IndexLimitation {
        code: code.to_owned(),
        scope: scope.map(str::to_owned),
        parameters: BTreeMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::{
        ScanPolicy,
        catalog_snapshot::{CatalogSnapshotInput, DiscoveryConfig, build_project_catalog_snapshot},
        observe_project,
    };
    use star_contracts::{
        ids::{CheckoutId, ProjectId, RootBindingId},
        management::{
            CheckoutAttachmentState, CheckoutHeadState, CheckoutKind, IdentityScope,
            RegistrationState, RepositoryKind,
        },
    };

    fn fixture() -> (std::path::PathBuf, Project, ProjectCheckout) {
        let root = std::env::temp_dir().join(format!("star-index-{}", CheckoutId::new()));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn alpha() { alpha(); }\n").unwrap();
        fs::write(root.join("README.md"), "alpha documentation\n").unwrap();
        let project_id = ProjectId::new();
        let checkout_id = CheckoutId::new();
        let declaration_fingerprint = Sha256Hash::digest(project_id.as_str().as_bytes());
        let project = Project {
            schema_id: "star.project".to_owned(),
            schema_version: 2,
            project_id: project_id.clone(),
            identity_scope: IdentityScope::Local,
            display_name: "fixture".to_owned(),
            repository_kind: RepositoryKind::None,
            source_of_truth: vec!["source".to_owned()],
            declaration_fingerprint: declaration_fingerprint.clone(),
            registration_state: RegistrationState::Attached,
            attached_checkout_ids: vec![checkout_id.clone()],
            latest_revision_id: None,
            latest_workspace_snapshot_id: None,
        };
        let checkout = ProjectCheckout {
            schema_id: "star.project-checkout".to_owned(),
            schema_version: 1,
            checkout_id,
            project_id,
            root_binding_id: Some(RootBindingId::new()),
            repository_kind: RepositoryKind::None,
            checkout_kind: CheckoutKind::FilesystemRoot,
            repository_binding_id: None,
            worktree_binding_id: None,
            object_format: None,
            head_state: CheckoutHeadState::Unavailable,
            head_ref: None,
            head_commit_id: None,
            head_tree_id: None,
            upstream_ref: None,
            default_branch_hint: None,
            remote_identity: None,
            attachment_state: CheckoutAttachmentState::Attached,
            last_observed_at: Utc::now(),
            limitations: vec![],
            content_fingerprint: declaration_fingerprint,
        };
        (root, project, checkout)
    }

    fn build(
        root: &std::path::Path,
        project: &Project,
        checkout: &ProjectCheckout,
        previous: Option<&CodeIndexProjection>,
    ) -> CodeIndexProjection {
        let catalog = build_project_catalog_snapshot(
            &[CatalogSnapshotInput {
                project,
                checkout,
                root,
            }],
            &DiscoveryConfig::default(),
        )
        .unwrap();
        let observation = observe_project(project, root, &ScanPolicy::default()).unwrap();
        build_code_index(&CodeIndexBuildRequest {
            project_root: Some(root),
            project,
            checkout,
            catalog_snapshot: &catalog,
            observation: &observation,
            scan_run_id: &ScanRunId::new(),
            generation_id: &GenerationId::new(),
            policy: &IndexPolicy::default(),
            syntax_adapters: &[],
            semantic_adapters: &[],
            scan_mode: IndexScanMode::Incremental,
            previous,
        })
        .unwrap()
    }

    fn build_with_mode(
        root: &std::path::Path,
        project: &Project,
        checkout: &ProjectCheckout,
        previous: Option<&CodeIndexProjection>,
        scan_mode: IndexScanMode,
    ) -> CodeIndexProjection {
        let catalog = build_project_catalog_snapshot(
            &[CatalogSnapshotInput {
                project,
                checkout,
                root,
            }],
            &DiscoveryConfig::default(),
        )
        .unwrap();
        let observation = observe_project(project, root, &ScanPolicy::default()).unwrap();
        build_code_index(&CodeIndexBuildRequest {
            project_root: Some(root),
            project,
            checkout,
            catalog_snapshot: &catalog,
            observation: &observation,
            scan_run_id: &ScanRunId::new(),
            generation_id: &GenerationId::new(),
            policy: &IndexPolicy::default(),
            syntax_adapters: &[],
            semantic_adapters: &[],
            scan_mode,
            previous,
        })
        .unwrap()
    }

    #[test]
    fn unchanged_files_reuse_only_eligible_partitions_and_semantic_is_explicitly_unavailable() {
        let (root, project, checkout) = fixture();
        let first = build(&root, &project, &checkout, None);
        assert!(
            first
                .snapshot
                .limitations
                .iter()
                .any(|item| item.code == "INDEX_SEMANTIC_UNAVAILABLE")
        );
        assert_eq!(first.snapshot.counts.definitions, 0);
        assert_eq!(first.snapshot.counts.references, 0);
        let second = build(&root, &project, &checkout, Some(&first));
        assert_eq!(
            first.snapshot.code_index_snapshot_id,
            second.snapshot.code_index_snapshot_id
        );
        assert!(
            second
                .snapshot
                .partitions
                .iter()
                .filter(|item| item.kind == IndexPartitionKind::Text)
                .all(|item| item.state == IndexPartitionState::Reused)
        );

        fs::write(root.join("src/lib.rs"), "pub fn beta() { beta(); }\n").unwrap();
        let third = build(&root, &project, &checkout, Some(&second));
        assert_ne!(
            second.snapshot.code_index_snapshot_id,
            third.snapshot.code_index_snapshot_id
        );
        assert_eq!(
            third
                .snapshot
                .partitions
                .iter()
                .filter(|item| item.partition_key == "README.md:text"
                    && item.state == IndexPartitionState::Reused)
                .count(),
            1
        );
        assert_eq!(
            third
                .snapshot
                .partitions
                .iter()
                .filter(|item| item.partition_key == "src/lib.rs:text"
                    && item.state != IndexPartitionState::Reused)
                .count(),
            1
        );
    }

    #[test]
    fn text_tokens_preserve_unicode_columns_and_resource_limits() {
        let result = text_tokens("가나다 alpha", 1);
        assert!(result.truncated);
        assert_eq!(result.items[0].token, "가나다");
        assert_eq!(result.items[0].range.end_column, 4);
    }

    #[test]
    fn hardcoding_owner_keeps_same_literal_occurrences_separate_by_symbol() {
        let source_id = CanonicalSourceId::new();
        let entity = |key: &str, start_line: u32, end_line: u32| IndexEntity {
            entity_key: key.to_owned(),
            kind: IndexEntityKind::Symbol,
            canonical_source_id: Some(source_id.clone()),
            symbol_id: None,
            qualified_name: key.to_owned(),
            source_range: Some(SourceRange {
                start_line,
                start_column: 1,
                end_line,
                end_column: 120,
            }),
            tier: IndexTier::Syntax,
            confidence: "high".to_owned(),
            content_fingerprint: Sha256Hash::digest(key.as_bytes()),
        };
        let entities = vec![entity("symbol:first", 1, 3), entity("symbol:second", 5, 7)];
        let first = hardcoding_owner_entity(
            &source_id,
            &SourceRange {
                start_line: 2,
                start_column: 10,
                end_line: 2,
                end_column: 20,
            },
            &entities,
        );
        let second = hardcoding_owner_entity(
            &source_id,
            &SourceRange {
                start_line: 6,
                start_column: 10,
                end_line: 6,
                end_column: 20,
            },
            &entities,
        );
        assert_eq!(first.as_deref(), Some("symbol:first"));
        assert_eq!(second.as_deref(), Some("symbol:second"));
    }

    #[test]
    fn source_classification_matches_default_analysis_boundaries() {
        let cases = [
            ("src/lib.rs", SourceClass::Source),
            ("tests/api.rs", SourceClass::Test),
            ("docs/example.rs", SourceClass::Docs),
            ("Cargo.toml", SourceClass::Config),
            ("specs/schemas/api.schema.json", SourceClass::Schema),
            ("migrations/001.rs", SourceClass::Migration),
            ("generated/bindings.rs", SourceClass::Generated),
            ("vendor/library.rs", SourceClass::Vendor),
            ("cache/index.bin", SourceClass::Cache),
            ("target/debug/app.exe", SourceClass::Output),
            ("assets/blob.bin", SourceClass::Unknown),
        ];
        for (path, expected) in cases {
            assert_eq!(classify_source(path), expected, "{path}");
        }
        assert!(class_text_eligible(SourceClass::Docs));
        assert!(!class_syntax_eligible(SourceClass::Docs));
        assert!(class_syntax_eligible(SourceClass::Schema));
        assert!(!class_semantic_eligible(SourceClass::Schema));
        assert!(class_semantic_eligible(SourceClass::Migration));
        assert!(!class_text_eligible(SourceClass::Generated));
        assert!(!class_text_eligible(SourceClass::Vendor));
    }

    #[test]
    fn toolchain_guidance_hardcoding_and_full_scan_are_persisted_without_raw_literals() {
        let (root, project, checkout) = fixture();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(root.join("Cargo.lock"), "version = 4\n").unwrap();
        fs::write(
            root.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.96.0\"\n",
        )
        .unwrap();
        fs::write(root.join("AGENTS.md"), "# Build rules\n## Validation\n").unwrap();
        fs::write(
            root.join("src/hardcoded.rs"),
            concat!(
                "const ENDPOINT: &str = \"https://example.invalid/api\";\n",
                "const RETRY_LIMIT: u32 = 7;\n",
                "fn run() { std::process::Command::new(\"tool\"); }\n",
                "fn first() { panic!(\"stable duplicate failure\"); }\n",
                "fn second() { panic!(\"stable duplicate failure\"); }\n",
                "fn config_first() { let config_mode = \"private-stage\"; }\n",
                "fn config_second() { let config_mode = \"private-stage\"; }\n",
            ),
        )
        .unwrap();

        let incremental =
            build_with_mode(&root, &project, &checkout, None, IndexScanMode::Incremental);
        assert_eq!(incremental.snapshot.toolchains.len(), 1);
        let toolchain = &incremental.snapshot.toolchains[0];
        assert_eq!(toolchain.build_system.as_deref(), Some("cargo"));
        assert_eq!(
            toolchain.lockfile_ref.as_ref().unwrap().as_str(),
            "Cargo.lock"
        );
        assert_eq!(toolchain.toolchain_constraint.as_deref(), Some("1.96.0"));
        assert!(
            incremental
                .snapshot
                .guidance
                .iter()
                .any(|record| record.source_ref.as_str() == "AGENTS.md")
        );
        let categories = incremental
            .snapshot
            .hardcoding_candidates
            .iter()
            .map(|candidate| candidate.category)
            .collect::<BTreeSet<_>>();
        assert!(
            categories.contains(&HardcodingCategory::Endpoint),
            "detected categories: {categories:?}; sources: {:?}; limitations: {:?}",
            incremental
                .source_entries
                .iter()
                .map(|source| (
                    source.path.as_str(),
                    source.source_class,
                    source.facets.clone()
                ))
                .collect::<Vec<_>>(),
            incremental.snapshot.limitations,
        );
        assert!(categories.contains(&HardcodingCategory::TimeoutRetryLimit));
        assert!(categories.contains(&HardcodingCategory::RawCommand));
        assert!(categories.contains(&HardcodingCategory::DuplicateError));
        assert!(categories.contains(&HardcodingCategory::ConfigDuplicate));
        let encoded = serde_json::to_string(&incremental.snapshot.hardcoding_candidates).unwrap();
        assert!(!encoded.contains("example.invalid"));
        assert!(!encoded.contains("stable duplicate failure"));
        assert!(!encoded.contains("private-stage"));

        let full = build_with_mode(
            &root,
            &project,
            &checkout,
            Some(&incremental),
            IndexScanMode::Full,
        );
        assert_eq!(full.snapshot.scan_mode, IndexScanMode::Full);
        assert!(full.snapshot.partitions.iter().all(|partition| {
            partition.kind == IndexPartitionKind::Finding
                || partition.state != IndexPartitionState::Reused
        }));
    }

    struct FixtureSyntaxAdapter;

    impl SyntaxAdapter for FixtureSyntaxAdapter {
        fn language_id(&self) -> &'static str {
            "rust"
        }

        fn fingerprint(&self) -> Sha256Hash {
            Sha256Hash::digest(b"fixture-syntax-adapter-v1")
        }

        fn analyze(&self, _source: &FileObservation) -> Result<SyntaxAnalysis, AdapterFailure> {
            let range = SourceRange {
                start_line: 1,
                start_column: 8,
                end_line: 1,
                end_column: 13,
            };
            Ok(SyntaxAnalysis {
                definitions: vec![SyntaxDefinition {
                    qualified_name: "alpha".to_owned(),
                    symbol_kind: "function".to_owned(),
                    range: range.clone(),
                    visibility: Some("public".to_owned()),
                }],
                references: vec![SyntaxReference {
                    target_name: "alpha".to_owned(),
                    range,
                    reference_kind: "call".to_owned(),
                    resolution: SymbolResolution::Resolved,
                }],
                limitations: Vec::new(),
            })
        }
    }

    #[test]
    fn reused_syntax_projection_rebinds_run_evidence_without_changing_identity() {
        let (root, project, checkout) = fixture();
        let catalog = build_project_catalog_snapshot(
            &[CatalogSnapshotInput {
                project: &project,
                checkout: &checkout,
                root: &root,
            }],
            &DiscoveryConfig::default(),
        )
        .unwrap();
        let observation = observe_project(&project, &root, &ScanPolicy::default()).unwrap();
        let adapter = FixtureSyntaxAdapter;
        let syntax_adapters: [&dyn SyntaxAdapter; 1] = [&adapter];
        let first_run = ScanRunId::new();
        let first_generation = GenerationId::new();
        let first = build_code_index(&CodeIndexBuildRequest {
            project_root: Some(&root),
            project: &project,
            checkout: &checkout,
            catalog_snapshot: &catalog,
            observation: &observation,
            scan_run_id: &first_run,
            generation_id: &first_generation,
            policy: &IndexPolicy::default(),
            syntax_adapters: &syntax_adapters,
            semantic_adapters: &[],
            scan_mode: IndexScanMode::Incremental,
            previous: None,
        })
        .unwrap();
        let second_run = ScanRunId::new();
        let second_generation = GenerationId::new();
        let second = build_code_index(&CodeIndexBuildRequest {
            project_root: Some(&root),
            project: &project,
            checkout: &checkout,
            catalog_snapshot: &catalog,
            observation: &observation,
            scan_run_id: &second_run,
            generation_id: &second_generation,
            policy: &IndexPolicy::default(),
            syntax_adapters: &syntax_adapters,
            semantic_adapters: &[],
            scan_mode: IndexScanMode::Incremental,
            previous: Some(&first),
        })
        .unwrap();

        assert_eq!(
            first.snapshot.code_index_snapshot_id,
            second.snapshot.code_index_snapshot_id
        );
        assert!(
            second
                .symbols
                .iter()
                .all(|symbol| symbol.scan_run_id == second_run)
        );
        assert!(
            second
                .references
                .iter()
                .all(|reference| reference.scan_run_id == second_run)
        );
        assert!(
            second
                .references
                .iter()
                .all(|reference| reference.unresolved_target.is_none())
        );
    }
}
