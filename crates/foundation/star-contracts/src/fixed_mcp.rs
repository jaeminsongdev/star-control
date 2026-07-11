use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::registry::{RegistrySource, ToolReadiness};
use crate::{ApprovalId, OperationId, RequestId, Sha256Hash};

pub const SERVER_NAME: &str = "star-control";
pub const SERVER_TITLE: &str = "Star-Control";
pub const SERVER_DESCRIPTION: &str = "Fixed MCP gateway for the Star-Control live tool registry.";
pub const SERVER_INSTRUCTIONS: &str = "개발 작업과 외부 도구 사용은 먼저 `star_tool_search`로 action을 찾고 `star_tool_describe`로 현재 Schema, 위험 lane과 `descriptor_hash`를 확인한다. 반환된 `required_call_tool`에 `tool_id`, hash와 `arguments`를 전달한다. `TOOL_DESCRIPTOR_STALE`이면 다시 describe한다. `approval_required`와 `question_required`를 완료로 간주하지 않는다. 장기 실행은 Operation ID로 조회·취소한다.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RiskLane {
    ReadClosed,
    ReadOpen,
    WriteClosed,
    DestructiveClosed,
    WriteOpen,
    DestructiveOpen,
}

impl RiskLane {
    pub const fn call_tool(self) -> &'static str {
        match self {
            Self::ReadClosed => "star_tool_call_read_closed",
            Self::ReadOpen => "star_tool_call_read_open",
            Self::WriteClosed => "star_tool_call_write_closed",
            Self::DestructiveClosed => "star_tool_call_destructive_closed",
            Self::WriteOpen => "star_tool_call_write_open",
            Self::DestructiveOpen => "star_tool_call_destructive_open",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedTool {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub read_only: bool,
    pub destructive: bool,
    pub idempotent: bool,
    pub open_world: bool,
}

pub const FIXED_TOOLS: [FixedTool; 12] = [
    FixedTool {
        name: "star_tool_search",
        title: "Search Star-Control Tools",
        description: "Search the current Star-Control live registry for an action. Call describe before invoking a result.",
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: false,
    },
    FixedTool {
        name: "star_tool_describe",
        title: "Describe a Star-Control Tool",
        description: "Return the current Schema, risk lane, executable readiness, and descriptor hash for one action.",
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: false,
    },
    FixedTool {
        name: "star_tool_registry_status",
        title: "Inspect the Tool Registry",
        description: "Inspect live registry revisions, packages, watchers, last-known-good state, and diagnostics.",
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: false,
    },
    FixedTool {
        name: "star_tool_call_read_closed",
        title: "Run a Local Read Action",
        description: "Invoke the described local read-only action. The descriptor must require this exact lane.",
        read_only: true,
        destructive: false,
        idempotent: false,
        open_world: false,
    },
    FixedTool {
        name: "star_tool_call_read_open",
        title: "Run an External Read Action",
        description: "Invoke the described read-only action that may access external systems.",
        read_only: true,
        destructive: false,
        idempotent: false,
        open_world: true,
    },
    FixedTool {
        name: "star_tool_call_write_closed",
        title: "Run a Local Write Action",
        description: "Invoke the described non-destructive local mutation.",
        read_only: false,
        destructive: false,
        idempotent: false,
        open_world: false,
    },
    FixedTool {
        name: "star_tool_call_destructive_closed",
        title: "Run a Destructive Local Action",
        description: "Invoke the described destructive local action after policy checks.",
        read_only: false,
        destructive: true,
        idempotent: false,
        open_world: false,
    },
    FixedTool {
        name: "star_tool_call_write_open",
        title: "Run an External Write Action",
        description: "Invoke the described non-destructive action that changes or uses an external system.",
        read_only: false,
        destructive: false,
        idempotent: false,
        open_world: true,
    },
    FixedTool {
        name: "star_tool_call_destructive_open",
        title: "Run a Destructive External Action",
        description: "Invoke the described destructive external action after policy checks.",
        read_only: false,
        destructive: true,
        idempotent: false,
        open_world: true,
    },
    FixedTool {
        name: "star_tool_operation_get",
        title: "Get an Operation",
        description: "Read durable status, progress, and result for a Star-Control operation.",
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: false,
    },
    FixedTool {
        name: "star_tool_operation_cancel",
        title: "Cancel an Operation",
        description: "Request cancellation of a durable operation and return its current state.",
        read_only: false,
        destructive: true,
        idempotent: true,
        open_world: true,
    },
    FixedTool {
        name: "star_approval_resolve",
        title: "Resolve an Approval",
        description: "Record the user's approve or deny decision for the exact approval scope.",
        read_only: false,
        destructive: true,
        idempotent: true,
        open_world: true,
    },
];

pub fn fixed_tool(name: &str) -> Option<&'static FixedTool> {
    FIXED_TOOLS.iter().find(|tool| tool.name == name)
}

pub fn ipc_command(name: &str) -> Option<&'static str> {
    match name {
        "star_tool_search" => Some("tool.search"),
        "star_tool_describe" => Some("tool.describe"),
        "star_tool_registry_status" => Some("tool.registry.status"),
        "star_tool_call_read_closed"
        | "star_tool_call_read_open"
        | "star_tool_call_write_closed"
        | "star_tool_call_destructive_closed"
        | "star_tool_call_write_open"
        | "star_tool_call_destructive_open" => Some("tool.invoke"),
        "star_tool_operation_get" => Some("operation.get"),
        "star_tool_operation_cancel" => Some("operation.cancel"),
        "star_approval_resolve" => Some("approval.resolve"),
        _ => None,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpResultStatus {
    Ok,
    Accepted,
    QuestionRequired,
    ApprovalRequired,
    Blocked,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct McpToolResult {
    pub schema_id: String,
    pub schema_version: u32,
    pub status: McpResultStatus,
    pub summary: String,
    pub data: Option<serde_json::Value>,
    pub operation_id: Option<OperationId>,
    #[serde(default)]
    pub next_actions: Vec<serde_json::Value>,
    #[serde(default)]
    pub artifact_refs: Vec<serde_json::Value>,
    #[serde(default)]
    pub diagnostic_refs: Vec<serde_json::Value>,
    pub error: Option<serde_json::Value>,
    pub correlation_id: RequestId,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchInput {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespaces: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_kinds: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<RegistrySource>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readiness: Option<Vec<ToolReadiness>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_lanes: Option<Vec<RiskLane>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DescribeInput {
    pub tool_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryStatusInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<RegistrySource>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_diagnostics: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WaitMode {
    Auto,
    Sync,
    Accepted,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CallInput {
    pub tool_id: String,
    pub descriptor_hash: Sha256Hash,
    pub arguments: serde_json::Map<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_request_id: Option<RequestId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_mode: Option<WaitMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_timeout_ms: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OperationGetInput {
    pub operation_id: OperationId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_sequence: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_ms: Option<u16>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OperationCancelInput {
    pub operation_id: OperationId,
    pub reason: Option<String>,
    pub force_after_ms: Option<u16>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approve,
    Deny,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApprovalResolveInput {
    pub approval_id: ApprovalId,
    pub decision: ApprovalDecision,
    pub scope_hash: Sha256Hash,
    pub reason: Option<String>,
    pub conditions: Option<serde_json::Map<String, serde_json::Value>>,
}
