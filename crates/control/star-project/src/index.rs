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
        CodeIndexCounts, CodeIndexSnapshot, FreshnessProof, IndexCoverage, IndexEdge, IndexEntity,
        IndexEntityKind, IndexFreshnessState, IndexLimitation, IndexPartition, IndexPartitionKind,
        IndexPartitionState, IndexRelation, IndexTier, ProjectCatalogSnapshot, SourceClass,
        SourceEntry,
    },
    management::{
        Project, ProjectCheckout, SourceRange, Symbol, SymbolReference, SymbolResolution,
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

    let can_reuse = request.previous.is_some_and(|previous| {
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

    let coverage = coverage(&projection.source_entries, &projection.partitions);
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
        findings: 0,
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
            "partition_inputs":projection.partitions.iter().map(|item| (&item.partition_key,&item.input_fingerprint)).collect::<Vec<_>>(),
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
        required_tier: request.policy.required_tier,
        max_tier: request.policy.max_tier,
        adapter_set_fingerprint,
        classification_fingerprint,
        partitions: projection.partitions,
        coverage,
        counts,
        freshness,
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
