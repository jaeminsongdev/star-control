use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    Sha256Hash, ToolTrustId,
    fixed_mcp::RiskLane,
    ids::ToolCacheId,
    runtime::{ExecutableIdentity, ExternalProtocol, IsolationProfile},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RegistrySource {
    Release,
    User,
    Project,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToolReadiness {
    Ready,
    Unavailable,
    Untrusted,
    Incompatible,
    Degraded,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolDescriptor {
    pub tool_id: String,
    pub package_id: String,
    pub package_version: String,
    pub display_name: String,
    pub summary: String,
    pub description: String,
    pub source: RegistrySource,
    pub readiness: ToolReadiness,
    pub trust_id: Option<ToolTrustId>,
    pub risk_lane: RiskLane,
    pub descriptor_hash: Sha256Hash,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub task_kinds: Vec<String>,
    #[serde(default)]
    pub when_to_use: Vec<String>,
    #[serde(default)]
    pub when_not_to_use: Vec<String>,
    #[serde(default)]
    pub permission_actions: Vec<String>,
    pub paid_action: String,
    pub idempotency: String,
    pub isolation: IsolationProfile,
    pub backend_kind: String,
    pub backend_ref: String,
    pub protocol: Option<ExternalProtocol>,
    pub executable_identity_ref: Option<Sha256Hash>,
    pub execution_contract: serde_json::Value,
    #[serde(default)]
    pub valid_examples: Vec<serde_json::Value>,
    #[serde(default)]
    pub invalid_examples: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExecutableSnapshot {
    pub executable_id: String,
    pub locator_hash: Sha256Hash,
    pub identity: ExecutableIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackageSnapshot {
    pub package_id: String,
    pub package_version: String,
    pub source_kind: RegistrySource,
    pub manifest_hash: Sha256Hash,
    #[serde(default)]
    pub schema_hashes: BTreeMap<String, Sha256Hash>,
    #[serde(default)]
    pub executable_identities: BTreeMap<String, Sha256Hash>,
    #[serde(default)]
    pub tool_descriptor_hashes: BTreeMap<String, Sha256Hash>,
    pub trust_id: Option<ToolTrustId>,
    pub package_hash: Sha256Hash,
}

/// Immutable live Registry snapshot. Diagnostics and creation time are
/// carried for status but are excluded from `snapshot_hash` by its producer.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistrySnapshot {
    pub schema_id: String,
    pub schema_version: u32,
    pub registry_revision: u64,
    pub snapshot_hash: Sha256Hash,
    #[serde(default)]
    pub package_snapshots: BTreeMap<String, PackageSnapshot>,
    #[serde(default)]
    pub tool_descriptors: BTreeMap<String, ToolDescriptor>,
    #[serde(default)]
    pub executable_snapshots: BTreeMap<String, ExecutableSnapshot>,
    pub search_index_hash: Sha256Hash,
    #[serde(default)]
    pub diagnostics: Vec<serde_json::Value>,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceFileIdentity {
    pub volume_serial: String,
    pub file_id: String,
    pub size: u64,
    pub last_write: String,
}

/// One durable last-known-good cache entry. The outer cache document owns its
/// own JCS sidecar hash and therefore is intentionally a separate envelope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolRegistryCache {
    pub schema_id: String,
    pub schema_version: u32,
    pub cache_id: ToolCacheId,
    pub package_id: String,
    pub package_version: String,
    pub source_kind: RegistrySource,
    pub source_id_hash: Sha256Hash,
    pub source_file_identity: SourceFileIdentity,
    pub source_content_hash: Sha256Hash,
    pub manifest_hash: Sha256Hash,
    pub package_snapshot: PackageSnapshot,
    pub trust_id: Option<ToolTrustId>,
    pub mcp_contract_version: u32,
    pub product_version: String,
    pub validated_at: String,
}
