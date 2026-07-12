use crate::{
    Sha256Hash,
    ids::{OperationId, RequestId},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExternalProtocol {
    ArgvV1,
    StarJsonStdioV1,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IsolationProfile {
    TrustedDesktop,
    AppcontainerAdapter,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalToolRequest {
    pub frame: String,
    pub protocol_version: u32,
    pub schema_id: String,
    pub schema_version: u32,
    pub request_id: RequestId,
    pub tool_id: String,
    pub descriptor_hash: Sha256Hash,
    pub arguments: serde_json::Value,
    pub context: ExternalToolContext,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalToolContext {
    pub operation_id: OperationId,
    pub project_id: Option<String>,
    pub goal_id: Option<String>,
    pub run_id: Option<String>,
    pub stage_id: Option<String>,
    pub deadline_at: String,
    pub artifact_directory: String,
    pub temp_directory: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalToolProgress {
    pub frame: String,
    pub protocol_version: u32,
    pub request_id: RequestId,
    pub sequence: u64,
    pub progress: u64,
    pub total: Option<u64>,
    pub message: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalToolCancel {
    pub frame: String,
    pub protocol_version: u32,
    pub request_id: RequestId,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalToolCancelAck {
    pub frame: String,
    pub protocol_version: u32,
    pub request_id: RequestId,
    pub accepted: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalToolProbeRequest {
    pub frame: String,
    pub protocol_version: u32,
    pub request_id: RequestId,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalToolProbeResponse {
    pub frame: String,
    pub protocol_version: u32,
    pub request_id: RequestId,
    pub product_version: String,
    pub interface_version: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExternalToolResultStatus {
    Ok,
    Cancelled,
    Error,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalToolArtifact {
    pub path: String,
    pub media_type: String,
    pub role: String,
    pub sha256: Sha256Hash,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalToolError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    #[serde(default)]
    pub details: BTreeMap<String, serde_json::Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalToolResponse {
    pub frame: String,
    pub protocol_version: u32,
    pub schema_id: String,
    pub schema_version: u32,
    pub request_id: RequestId,
    pub status: ExternalToolResultStatus,
    pub summary: String,
    pub data: Option<serde_json::Value>,
    #[serde(default)]
    pub diagnostics: Vec<serde_json::Value>,
    #[serde(default)]
    pub artifacts: Vec<ExternalToolArtifact>,
    pub error: Option<ExternalToolError>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExecutableIdentity {
    pub volume_serial: String,
    pub file_id: String,
    pub size: u64,
    pub last_write: String,
    pub sha256: Sha256Hash,
    pub product_version: Option<String>,
    pub interface_version: Option<String>,
    pub architecture: String,
    pub signature_status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FileIdentityLease {
    pub final_path_hash: Sha256Hash,
    pub identity: ExecutableIdentity,
    pub acquired_at: String,
}
