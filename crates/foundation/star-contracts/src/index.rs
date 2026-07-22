//! Persisted M1 project catalog and code-index contracts.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    Sha256Hash,
    evidence::ArtifactRef,
    ids::{
        CanonicalSourceId, CheckoutId, CodeIndexSnapshotId, GenerationId, ProjectCatalogSnapshotId,
        ProjectId, ProjectRevisionId, ScanRunId, SymbolId, WorkspaceSnapshotId,
    },
    management::{Completeness, ProjectPathRef, SourceRange, SymbolResolution},
};

macro_rules! string_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    };
}

string_enum!(IndexTier {
    Text,
    Syntax,
    Semantic
});
string_enum!(IndexPartitionKind {
    Inventory,
    Classification,
    Text,
    Syntax,
    Semantic,
    Graph,
    Finding
});
string_enum!(IndexPartitionState {
    NotPlanned,
    Queued,
    Running,
    Succeeded,
    Incomplete,
    Failed,
    Cancelled,
    Reused
});
string_enum!(IndexFreshnessState {
    Current,
    StaleCatalog,
    StaleSource,
    StaleConfig,
    StaleAdapter,
    Partial,
    Unverified,
    Unavailable
});
string_enum!(SourceClass {
    Source,
    Test,
    Docs,
    Config,
    Schema,
    Migration,
    Vendor,
    Generated,
    Cache,
    Output,
    Unknown
});
string_enum!(WorkspaceKind { Cargo, Generic });
string_enum!(ProjectCatalogEdgeKind {
    Nested,
    Submodule,
    WorkspaceMember,
    SameRepository
});
string_enum!(IndexEntityKind {
    Project,
    Checkout,
    Workspace,
    Source,
    Package,
    Module,
    Symbol,
    Contract,
    ConfigKey,
    SchemaId,
    ErrorCode,
    Constant,
    PublicSurface,
    ExternalDependency,
    TextToken
});
string_enum!(IndexRelation {
    Contains,
    Defines,
    References,
    Imports,
    DependsOn,
    Tests,
    Documents,
    Generates,
    GeneratedFrom,
    WorkspaceMember,
    Nested,
    SameRepository,
    TextOccurrence
});

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IndexLimitation {
    pub code: String,
    pub scope: Option<String>,
    pub parameters: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectCatalogProjectRef {
    pub project_id: ProjectId,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectCatalogCheckoutRef {
    pub checkout_id: CheckoutId,
    pub project_id: ProjectId,
    pub observation_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceNode {
    pub workspace_key: String,
    pub kind: WorkspaceKind,
    pub project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub marker_source: ProjectPathRef,
    pub member_refs: Vec<ProjectPathRef>,
    pub evidence_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectCatalogEdge {
    pub from_project_id: ProjectId,
    pub from_checkout_id: CheckoutId,
    pub to_project_id: ProjectId,
    pub to_checkout_id: CheckoutId,
    pub relation: ProjectCatalogEdgeKind,
    pub evidence_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectCatalogCounts {
    pub roots: u64,
    pub projects: u64,
    pub checkouts: u64,
    pub workspaces: u64,
    pub excluded: u64,
    pub errors: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectCatalogSnapshot {
    pub schema_id: String,
    pub schema_version: u32,
    pub project_catalog_snapshot_id: ProjectCatalogSnapshotId,
    pub discovery_scope_fingerprint: Sha256Hash,
    pub discovery_config_fingerprint: Sha256Hash,
    pub project_refs: Vec<ProjectCatalogProjectRef>,
    pub checkout_refs: Vec<ProjectCatalogCheckoutRef>,
    pub workspace_nodes: Vec<WorkspaceNode>,
    pub project_edges: Vec<ProjectCatalogEdge>,
    pub counts: ProjectCatalogCounts,
    pub completeness: Completeness,
    pub limitations: Vec<IndexLimitation>,
    pub captured_at: DateTime<Utc>,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IndexPartition {
    pub partition_key: String,
    pub kind: IndexPartitionKind,
    pub required: bool,
    pub requested_tier: IndexTier,
    pub used_tier: Option<IndexTier>,
    pub state: IndexPartitionState,
    pub input_fingerprint: Sha256Hash,
    pub output_fingerprint: Option<Sha256Hash>,
    pub target_count: u64,
    pub indexed_count: u64,
    pub failed_count: u64,
    pub excluded_count: u64,
    pub cache_hit: bool,
    pub limitations: Vec<IndexLimitation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IndexCoverage {
    pub source_class: SourceClass,
    pub language_id: String,
    pub tier: IndexTier,
    pub target_count: u64,
    pub succeeded_count: u64,
    pub failed_count: u64,
    pub excluded_count: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CodeIndexCounts {
    pub sources: u64,
    pub packages: u64,
    pub modules: u64,
    pub symbols: u64,
    pub definitions: u64,
    pub references: u64,
    pub graph_edges: u64,
    pub findings: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FreshnessProof {
    pub partition_key: String,
    pub state: IndexFreshnessState,
    pub indexed_catalog_fingerprint: Sha256Hash,
    pub indexed_source_fingerprint: Sha256Hash,
    pub indexed_config_fingerprint: Sha256Hash,
    pub indexed_adapter_fingerprint: Sha256Hash,
    pub observed_source_fingerprint: Option<Sha256Hash>,
    pub probe_method: String,
    pub probed_at: DateTime<Utc>,
    pub stale_reason_codes: Vec<String>,
    pub unverified_scope_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceEntry {
    pub canonical_source_id: CanonicalSourceId,
    pub path: ProjectPathRef,
    pub content_sha256: Sha256Hash,
    pub size_bytes: u64,
    pub source_class: SourceClass,
    pub facets: Vec<String>,
    pub language_id: String,
    pub encoding: String,
    pub owner_project_id: ProjectId,
    pub owner_checkout_id: CheckoutId,
    pub analysis_eligible: bool,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IndexEntity {
    pub entity_key: String,
    pub kind: IndexEntityKind,
    pub canonical_source_id: Option<CanonicalSourceId>,
    pub symbol_id: Option<SymbolId>,
    pub qualified_name: String,
    pub source_range: Option<SourceRange>,
    pub tier: IndexTier,
    pub confidence: String,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IndexEdge {
    pub edge_key: String,
    pub from_entity_key: String,
    pub to_entity_key: Option<String>,
    pub unresolved_target: Option<String>,
    pub relation: IndexRelation,
    pub evidence_source_id: CanonicalSourceId,
    pub evidence_range: Option<SourceRange>,
    pub tier: IndexTier,
    pub resolution: SymbolResolution,
    pub confidence: String,
    pub content_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CodeIndexSnapshot {
    pub schema_id: String,
    pub schema_version: u32,
    pub code_index_snapshot_id: CodeIndexSnapshotId,
    pub project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub project_catalog_snapshot_id: ProjectCatalogSnapshotId,
    pub checkout_observation_fingerprint: Sha256Hash,
    pub project_revision_id: ProjectRevisionId,
    pub workspace_snapshot_id: WorkspaceSnapshotId,
    pub scan_run_id: ScanRunId,
    pub generation_id: GenerationId,
    pub analysis_input_fingerprint: Sha256Hash,
    pub scan_config_fingerprint: Sha256Hash,
    pub index_config_fingerprint: Sha256Hash,
    pub required_tier: IndexTier,
    pub max_tier: IndexTier,
    pub adapter_set_fingerprint: Sha256Hash,
    pub classification_fingerprint: Sha256Hash,
    pub partitions: Vec<IndexPartition>,
    pub coverage: Vec<IndexCoverage>,
    pub counts: CodeIndexCounts,
    pub freshness: Vec<FreshnessProof>,
    pub limitations: Vec<IndexLimitation>,
    pub artifact_refs: Vec<ArtifactRef>,
    pub content_fingerprint: Sha256Hash,
}
