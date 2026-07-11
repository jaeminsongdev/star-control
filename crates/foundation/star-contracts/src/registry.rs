use crate::{Sha256Hash, fixed_mcp::RiskLane};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    pub display_name: String,
    pub summary: String,
    pub source: RegistrySource,
    pub readiness: ToolReadiness,
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
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackageSnapshot {
    pub package_id: String,
    pub manifest_hash: Sha256Hash,
    pub descriptors: Vec<ToolDescriptor>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistrySnapshot {
    pub registry_revision: u64,
    pub snapshot_hash: Sha256Hash,
    pub packages: BTreeMap<String, PackageSnapshot>,
}
