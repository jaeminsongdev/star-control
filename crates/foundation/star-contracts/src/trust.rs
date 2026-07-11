use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Sha256Hash, ToolTrustId};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolTrustRecord {
    pub trust_id: ToolTrustId,
    pub package_id: String,
    pub manifest_hash: Sha256Hash,
    pub granted_at: String,
    pub expires_at: Option<String>,
}
