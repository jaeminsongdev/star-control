use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    Sha256Hash, ToolTrustId, manifest::UpdatePolicy, registry::RegistrySource,
    runtime::IsolationProfile,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrustMode {
    Exact,
    Compatible,
    ManagedPath,
}

/// Executable portion of one immutable code-trust grant.  Values that can
/// reveal a local path are represented by a locator hash, never by the path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrustedExecutable {
    pub executable_id: String,
    pub locator_hash: Sha256Hash,
    #[serde(default)]
    pub config_revision: Option<Sha256Hash>,
    #[serde(default)]
    pub fixed_working_directory_hash: Option<Sha256Hash>,
    pub update_policy: UpdatePolicy,
    pub exact_hash: Option<Sha256Hash>,
    pub publisher_subject: Option<String>,
    pub product_version_req: String,
    pub interface_version_req: String,
}

/// Frozen `star.tool-trust-record` v1 persisted by the Controller.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolTrustRecord {
    pub schema_id: String,
    pub schema_version: u32,
    pub trust_id: ToolTrustId,
    pub package_id: String,
    pub package_version: String,
    pub source_kind: RegistrySource,
    pub source_id_hash: Sha256Hash,
    pub manifest_hash: Sha256Hash,
    #[serde(default)]
    pub schema_hashes: BTreeMap<String, Sha256Hash>,
    pub trust_mode: TrustMode,
    #[serde(default)]
    pub executables: Vec<TrustedExecutable>,
    #[serde(default)]
    pub permission_actions: Vec<String>,
    #[serde(default)]
    pub isolation_profiles: Vec<IsolationProfile>,
    pub granted_by: serde_json::Value,
    pub granted_at: String,
    pub expires_at: Option<String>,
    pub revoked_at: Option<String>,
    pub revoke_reason: Option<String>,
}
