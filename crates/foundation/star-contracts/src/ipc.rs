use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    MCP_CONTRACT_VERSION,
    ids::{OperationId, RequestId},
};

pub const IPC_PROTOCOL_MAJOR: u16 = 1;
pub const IPC_MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IpcChallenge {
    pub schema_id: String,
    pub schema_version: u32,
    pub protocol_major: u16,
    pub controller_instance_id: String,
    pub server_pid: u32,
    pub server_nonce: String,
    pub issued_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IpcClientKind {
    Cli,
    Mcp,
    Hook,
    InternalTest,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IpcHello {
    pub schema_id: String,
    pub schema_version: u32,
    pub protocol_versions: Vec<String>,
    pub client_kind: IpcClientKind,
    pub client_version: String,
    pub client_instance_id: String,
    pub client_pid: u32,
    pub client_nonce: String,
    pub server_nonce: String,
    pub auth_tag: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ControllerReadiness {
    Ready,
    Degraded,
    Recovering,
    Blocked,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IpcWelcome {
    pub schema_id: String,
    pub schema_version: u32,
    pub protocol_version: String,
    pub controller_version: String,
    pub controller_instance_id: String,
    pub session_id: String,
    pub server_nonce: String,
    pub auth_tag: String,
    pub readiness: ControllerReadiness,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub registry_revision: u64,
    pub server_time: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IpcHandshakeError {
    pub schema_id: String,
    pub schema_version: u32,
    pub code: String,
    pub supported_versions: Vec<String>,
    pub correlation_id: String,
    pub auth_tag: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IpcStatus {
    Ok,
    Accepted,
    QuestionRequired,
    ApprovalRequired,
    Blocked,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IpcRequest {
    pub schema_id: String,
    pub schema_version: u32,
    pub request_id: RequestId,
    pub command: String,
    pub payload: serde_json::Value,
    pub client_request_id: String,
    pub idempotency_key: Option<String>,
    pub deadline: Option<String>,
    pub actor: serde_json::Value,
    pub trace_context: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IpcResponse {
    pub schema_id: String,
    pub schema_version: u32,
    pub request_id: RequestId,
    pub status: IpcStatus,
    pub data: Option<serde_json::Value>,
    pub operation_id: Option<OperationId>,
    #[serde(default)]
    pub diagnostics: Vec<serde_json::Value>,
    pub error: Option<ErrorEnvelope>,
    pub registry_revision: Option<u64>,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ErrorEnvelope {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl IpcChallenge {
    pub fn v1(
        controller_instance_id: String,
        server_pid: u32,
        server_nonce: String,
        issued_at: String,
    ) -> Self {
        Self {
            schema_id: "star.ipc.challenge".to_owned(),
            schema_version: MCP_CONTRACT_VERSION,
            protocol_major: IPC_PROTOCOL_MAJOR,
            controller_instance_id,
            server_pid,
            server_nonce,
            issued_at,
        }
    }
}
