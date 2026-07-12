use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ipc::ErrorEnvelope;
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
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadClosed => "read_closed",
            Self::ReadOpen => "read_open",
            Self::WriteClosed => "write_closed",
            Self::DestructiveClosed => "destructive_closed",
            Self::WriteOpen => "write_open",
            Self::DestructiveOpen => "destructive_open",
        }
    }

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
    pub error: Option<ErrorEnvelope>,
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

const DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";
const GLOBAL_ID_PATTERN: &str = r"^[a-z][a-z0-9]*(?:[._-][a-z0-9]+){1,7}$";
const HASH_PATTERN: &str = r"^sha256:[0-9a-f]{64}$";
const REQUEST_ID_PATTERN: &str = r"^req_[0-9A-HJKMNP-TV-Z]{26}$";
const OPERATION_ID_PATTERN: &str = r"^opn_[0-9A-HJKMNP-TV-Z]{26}$";
const APPROVAL_ID_PATTERN: &str = r"^apr_[0-9A-HJKMNP-TV-Z]{26}$";
const GOAL_ID_PATTERN: &str = r"^gol_[0-9A-HJKMNP-TV-Z]{26}$";

fn nullable(schema: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"anyOf":[schema,{"type":"null"}]})
}

fn schema_root(
    schema_id: &str,
    properties: serde_json::Value,
    required: &[&str],
) -> serde_json::Value {
    serde_json::json!({
        "$schema": DRAFT_2020_12,
        "$id": format!("urn:star-control:schema:{schema_id}:v1"),
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

fn string_array(
    item: serde_json::Value,
    maximum: usize,
    default: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "type":"array",
        "items":item,
        "maxItems":maximum,
        "uniqueItems":true,
        "default":default
    })
}

/// Fully resolved fixed MCP input Schema. These objects are used both by the
/// generated schema set and directly in `tools/list`; no remote `$ref` is
/// permitted on the wire.
pub fn fixed_input_schema(tool_name: &str) -> Option<serde_json::Value> {
    let id = format!("star.mcp.{tool_name}.input");
    let value = match tool_name {
        "star_tool_search" => schema_root(
            &id,
            serde_json::json!({
                "query":{"type":"string","minLength":1,"maxLength":256,"pattern":r"\S"},
                "namespaces":string_array(serde_json::json!({"type":"string","minLength":1,"maxLength":128,"pattern":r"^[a-z][a-z0-9]*(?:[._-][a-z0-9]+){0,7}$"}),16,serde_json::json!([])),
                "tags":string_array(serde_json::json!({"type":"string","pattern":r"^[a-z][a-z0-9_-]{0,31}$"}),32,serde_json::json!([])),
                "task_kinds":string_array(serde_json::json!({"type":"string","pattern":r"^[a-z][a-z0-9_-]{0,31}$"}),16,serde_json::json!([])),
                "sources":string_array(serde_json::json!({"type":"string","enum":["release","user","project"]}),3,serde_json::json!(["release","user","project"])),
                "readiness":string_array(serde_json::json!({"type":"string","enum":["ready","unavailable","untrusted","incompatible","degraded"]}),5,serde_json::json!(["ready"])),
                "risk_lanes":string_array(serde_json::json!({"type":"string","enum":["read_closed","read_open","write_closed","destructive_closed","write_open","destructive_open"]}),6,serde_json::json!(["read_closed","read_open","write_closed","destructive_closed","write_open","destructive_open"])),
                "limit":{"type":"integer","minimum":1,"maximum":50,"default":10},
                "cursor":nullable(serde_json::json!({"type":"string","maxLength":1024,"pattern":r"^[^\u0000]*$"})),
            }),
            &["query"],
        ),
        "star_tool_describe" => schema_root(
            &id,
            serde_json::json!({
                "tool_id":{"type":"string","minLength":3,"maxLength":128,"pattern":GLOBAL_ID_PATTERN}
            }),
            &["tool_id"],
        ),
        "star_tool_registry_status" => schema_root(
            &id,
            serde_json::json!({
                "package_id":nullable(serde_json::json!({"type":"string","minLength":3,"maxLength":128,"pattern":GLOBAL_ID_PATTERN})),
                "sources":string_array(serde_json::json!({"type":"string","enum":["release","user","project"]}),3,serde_json::json!(["release","user","project"])),
                "include_diagnostics":{"type":"boolean","default":true},
                "limit":{"type":"integer","minimum":1,"maximum":200,"default":50},
                "cursor":nullable(serde_json::json!({"type":"string","maxLength":1024,"pattern":r"^[^\u0000]*$"})),
            }),
            &[],
        ),
        "star_tool_call_read_closed"
        | "star_tool_call_read_open"
        | "star_tool_call_write_closed"
        | "star_tool_call_destructive_closed"
        | "star_tool_call_write_open"
        | "star_tool_call_destructive_open" => schema_root(
            &id,
            serde_json::json!({
                "tool_id":{"type":"string","minLength":3,"maxLength":128,"pattern":GLOBAL_ID_PATTERN},
                "descriptor_hash":{"type":"string","pattern":HASH_PATTERN},
                "arguments":{"type":"object"},
                "client_request_id":nullable(serde_json::json!({"type":"string","pattern":REQUEST_ID_PATTERN})),
                "idempotency_key":nullable(serde_json::json!({"type":"string","minLength":1,"maxLength":128,"pattern":r"^[^\u0000]+$"})),
                "goal_id":nullable(serde_json::json!({"type":"string","pattern":GOAL_ID_PATTERN})),
                "expected_revision":nullable(serde_json::json!({"type":"integer","minimum":0,"maximum":JSON_SAFE_INTEGER_MAX})),
                "wait_mode":{"type":"string","enum":["auto","sync","accepted"],"default":"auto"},
                "requested_timeout_ms":nullable(serde_json::json!({"type":"integer","minimum":100,"maximum":86400000})),
            }),
            &["tool_id", "descriptor_hash", "arguments"],
        ),
        "star_tool_operation_get" => schema_root(
            &id,
            serde_json::json!({
                "operation_id":{"type":"string","pattern":OPERATION_ID_PATTERN},
                "after_sequence":{"type":"integer","minimum":0,"maximum":JSON_SAFE_INTEGER_MAX,"default":0},
                "wait_ms":{"type":"integer","minimum":0,"maximum":30000,"default":0},
            }),
            &["operation_id"],
        ),
        "star_tool_operation_cancel" => schema_root(
            &id,
            serde_json::json!({
                "operation_id":{"type":"string","pattern":OPERATION_ID_PATTERN},
                "reason":nullable(serde_json::json!({"type":"string","maxLength":512,"pattern":r"^[^\u0000]*$"})),
                "force_after_ms":nullable(serde_json::json!({"type":"integer","minimum":0,"maximum":30000})),
            }),
            &["operation_id"],
        ),
        "star_approval_resolve" => schema_root(
            &id,
            serde_json::json!({
                "approval_id":{"type":"string","pattern":APPROVAL_ID_PATTERN},
                "decision":{"type":"string","enum":["approve","deny"]},
                "scope_hash":{"type":"string","pattern":HASH_PATTERN},
                "reason":nullable(serde_json::json!({"type":"string","maxLength":1000,"pattern":r"^[^\u0000]*$"})),
                "conditions":nullable(serde_json::json!({"type":"object"})),
            }),
            &["approval_id", "decision", "scope_hash"],
        ),
        _ => return None,
    };
    Some(value)
}

fn error_schema() -> serde_json::Value {
    serde_json::json!({
        "type":"object",
        "properties":{
            "schema_id":{"const":"star.error"},
            "schema_version":{"const":1},
            "code":{"type":"string","pattern":r"^[A-Z][A-Z0-9]*(?:_[A-Z0-9]+)+$"},
            "category":{"type":"string","enum":["config","contract","state","policy","route","tool","codex","validation","vcs","ipc","release","internal"]},
            "message":{"type":"string","minLength":1,"maxLength":1000,"pattern":r"^[^\u0000]+$"},
            "retryable":{"type":"boolean"},
            "retry_after_ms":nullable(serde_json::json!({"type":"integer","minimum":0,"maximum":86400000})),
            "user_action":nullable(serde_json::json!({"type":"object"})),
            "context":{"type":"object"},
            "correlation_id":{"type":"string","minLength":1,"maxLength":128,"pattern":r"^[^\u0000]+$"},
            "caused_by":nullable(serde_json::json!({"type":"object","properties":{"code":{"type":"string"},"summary":{"type":"string","minLength":1,"maxLength":1000}},"required":["code","summary"],"additionalProperties":false})),
            "artifact_refs":{"type":"array","maxItems":256,"items":{"type":"object"}},
            "occurred_at":{"type":"string","pattern":r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$"},
            "component":{"type":"string","minLength":1,"maxLength":128}
        },
        "required":["schema_id","schema_version","code","category","message","retryable","retry_after_ms","user_action","context","correlation_id","caused_by","artifact_refs","occurred_at","component"],
        "additionalProperties":false
    })
}

fn result_data_schema(tool_name: &str) -> serde_json::Value {
    let safe_integer =
        serde_json::json!({"type":"integer","minimum":0,"maximum":JSON_SAFE_INTEGER_MAX});
    let hash = serde_json::json!({"type":"string","pattern":HASH_PATTERN});
    let object = match tool_name {
        "star_tool_search" => serde_json::json!({
            "type":"object",
            "properties":{
                "registry_revision":safe_integer,
                "snapshot_hash":hash,
                "items":{"type":"array","maxItems":50,"items":{"type":"object","properties":{
                    "tool_id":{"type":"string","pattern":GLOBAL_ID_PATTERN},
                    "display_name":{"type":"string","minLength":1,"maxLength":80},
                    "summary":{"type":"string","minLength":1,"maxLength":240},
                    "source":{"type":"string","enum":["release","user","project"]},
                    "readiness":{"type":"string","enum":["ready","unavailable","untrusted","incompatible","degraded"]},
                    "risk_lane":{"type":"string","enum":["read_closed","read_open","write_closed","destructive_closed","write_open","destructive_open"]},
                    "descriptor_hash":{"type":"string","pattern":HASH_PATTERN},
                    "matched_fields":{"type":"array","uniqueItems":true,"items":{"type":"string","enum":["tool_id","alias","tag","task_kind","summary","description"]}}
                },"required":["tool_id","display_name","summary","source","readiness","risk_lane","descriptor_hash","matched_fields"],"additionalProperties":false}},
                "next_cursor":nullable(serde_json::json!({"type":"string","maxLength":1024}))
            },
            "required":["registry_revision","snapshot_hash","items","next_cursor"],
            "additionalProperties":false
        }),
        "star_tool_describe" => serde_json::json!({
            "type":"object",
            "properties":{
                "registry_revision":safe_integer,"snapshot_hash":hash,"descriptor_hash":hash,
                "required_call_tool":{"type":"string","enum":["star_tool_call_read_closed","star_tool_call_read_open","star_tool_call_write_closed","star_tool_call_destructive_closed","star_tool_call_write_open","star_tool_call_destructive_open"]},
                "tool_id":{"type":"string","pattern":GLOBAL_ID_PATTERN},"package_id":{"type":"string","pattern":GLOBAL_ID_PATTERN},
                "source":{"type":"string","enum":["release","user","project"]},"trust_state":{"type":"string"},"trust_basis":{"type":"string"},
                "readiness":{"type":"string","enum":["ready","unavailable","untrusted","incompatible","degraded"]},
                "display_name":{"type":"string"},"summary":{"type":"string"},"description":{"type":"string"},
                "aliases":{"type":"array","items":{"type":"string"}},"tags":{"type":"array","items":{"type":"string"}},"task_kinds":{"type":"array","items":{"type":"string"}},
                "when_to_use":{"type":"array","items":{"type":"string"}},"when_not_to_use":{"type":"array","items":{"type":"string"}},
                "input_schema":{"type":"object"},"output_schema":nullable(serde_json::json!({"type":"object"})),
                "permission_actions":{"type":"array","items":{"type":"string"}},"paid_action":{"type":"string"},
                "risk_lane":{"type":"string"},"isolation":{"type":"object"},"idempotency":{"type":"string"},"concurrency":{"type":"object"},
                "backend_kind":{"type":"string"},"protocol":nullable(serde_json::json!({"type":"string"})),"executable_identity":nullable(serde_json::json!({"type":"object"})),
                "timeout":{"type":"object"},"output":{"type":"object"},"progress":{"type":"object"},"cancel":{"type":"object"},
                "valid_examples":{"type":"array","maxItems":3},"invalid_examples":{"type":"array","maxItems":3}
            },
            "required":["registry_revision","snapshot_hash","descriptor_hash","required_call_tool","tool_id","package_id","source","trust_state","trust_basis","readiness","display_name","summary","description","aliases","tags","task_kinds","when_to_use","when_not_to_use","input_schema","output_schema","permission_actions","paid_action","risk_lane","isolation","idempotency","concurrency","backend_kind","protocol","executable_identity","timeout","output","progress","cancel","valid_examples","invalid_examples"],
            "additionalProperties":false
        }),
        "star_tool_registry_status" => serde_json::json!({
            "type":"object",
            "properties":{
                "registry_revision":safe_integer,"diagnostic_revision":safe_integer,"snapshot_hash":hash,
                "controller":{"type":"object"},"items":{"type":"array"},"diagnostics":{"type":"array"},
                "watcher":{"type":"object"},"last_demand_scan_at":{"type":"string"},
                "next_cursor":nullable(serde_json::json!({"type":"string","maxLength":1024}))
            },
            "required":["registry_revision","diagnostic_revision","snapshot_hash","controller","items","diagnostics","watcher","last_demand_scan_at","next_cursor"],
            "additionalProperties":false
        }),
        "star_tool_call_read_closed"
        | "star_tool_call_read_open"
        | "star_tool_call_write_closed"
        | "star_tool_call_destructive_closed"
        | "star_tool_call_write_open"
        | "star_tool_call_destructive_open" => serde_json::json!({
            "type":"object",
            "properties":{
                "tool_id":{"type":"string","pattern":GLOBAL_ID_PATTERN},"descriptor_hash":hash,
                "registry_revision":safe_integer,"arguments_hash":hash,"output_provenance":{"type":"object"},
                "result":{"type":"object"},"operation":{"type":"object"},"approval_request":{"type":"object"}
            },
            "required":["tool_id","descriptor_hash","registry_revision","arguments_hash"],
            "additionalProperties":false
        }),
        "star_tool_operation_get" => serde_json::json!({
            "type":"object","properties":{"operation":{"type":"object"},"progress":{"type":"array","maxItems":256},"next_after_sequence":safe_integer,"has_more":{"type":"boolean"},"wait_timed_out":{"type":"boolean"}},
            "required":["operation","progress","next_after_sequence","has_more","wait_timed_out"],"additionalProperties":false
        }),
        "star_tool_operation_cancel" => serde_json::json!({
            "type":"object","properties":{"operation":{"type":"object"},"cancel_requested":{"type":"boolean"},"cancel_effective":{"type":"boolean"}},
            "required":["operation","cancel_requested","cancel_effective"],"additionalProperties":false
        }),
        "star_approval_resolve" => serde_json::json!({
            "type":"object","properties":{"approval_id":{"type":"string","pattern":APPROVAL_ID_PATTERN},"decision":{"type":"string","enum":["approve","deny"]},"resolved_at":{"type":"string"},"operation":nullable(serde_json::json!({"type":"object"}))},
            "required":["approval_id","decision","resolved_at","operation"],"additionalProperties":false
        }),
        _ => serde_json::json!({"type":"object"}),
    };
    nullable(object)
}

/// Fully resolved per-tool result Schema advertised by `tools/list`.
pub fn fixed_result_schema(tool_name: &str) -> Option<serde_json::Value> {
    fixed_tool(tool_name)?;
    let schema_id = format!("star.mcp.{tool_name}.result");
    let operation_id =
        nullable(serde_json::json!({"type":"string","pattern":OPERATION_ID_PATTERN}));
    let error = nullable(error_schema());
    Some(serde_json::json!({
        "$schema":DRAFT_2020_12,
        "$id":format!("urn:star-control:schema:{schema_id}:v1"),
        "type":"object",
        "properties":{
            "schema_id":{"const":schema_id},
            "schema_version":{"const":1},
            "status":{"type":"string","enum":["ok","accepted","question_required","approval_required","blocked","error"]},
            "summary":{"type":"string","minLength":1,"maxLength":1000,"pattern":r"^[^\u0000]+$"},
            "data":result_data_schema(tool_name),
            "operation_id":operation_id,
            "next_actions":{"type":"array","maxItems":16,"items":{"type":"object","properties":{"tool_name":{"type":"string","enum":FIXED_TOOLS.map(|tool| tool.name)},"reason":{"type":"string","minLength":1,"maxLength":240},"arguments":{"type":"object"}},"required":["tool_name","reason","arguments"],"additionalProperties":false}},
            "artifact_refs":{"type":"array","items":{"type":"object"}},
            "diagnostic_refs":{"type":"array","items":{}},
            "error":error,
            "correlation_id":{"type":"string","pattern":REQUEST_ID_PATTERN}
        },
        "required":["schema_id","schema_version","status","summary","artifact_refs","diagnostic_refs","correlation_id"],
        "additionalProperties":false,
        "allOf":[
            {"if":{"properties":{"status":{"const":"ok"}},"required":["status"]},"then":{"properties":{"error":{"type":"null"},"operation_id":{"type":"null"}}}},
            {"if":{"properties":{"status":{"const":"accepted"}},"required":["status"]},"then":{
                "required":["operation_id","data"],
                "properties":{
                    "operation_id":{"type":"string","pattern":OPERATION_ID_PATTERN},
                    "data":{"type":"object","required":["operation"],"properties":{"operation":{"type":"object"}}},
                    "error":{"type":"null"}
                }
            }},
            {"if":{"properties":{"status":{"const":"approval_required"}},"required":["status"]},"then":{
                "required":["operation_id","data","next_actions"],
                "properties":{
                    "operation_id":{"type":"string","pattern":OPERATION_ID_PATTERN},
                    "data":{"type":"object","required":["operation","approval_request"],"properties":{
                        "operation":{"type":"object"},
                        "approval_request":{"type":"object","required":["approval_id","scope_hash"],"properties":{
                            "approval_id":{"type":"string","pattern":APPROVAL_ID_PATTERN},
                            "scope_hash":{"type":"string","pattern":HASH_PATTERN}
                        }}
                    }},
                    "next_actions":{"minItems":1,"contains":{"type":"object","properties":{"tool_name":{"const":"star_approval_resolve"}},"required":["tool_name"]}},
                    "error":{"type":"null"}
                }
            }},
            {"if":{"properties":{"status":{"const":"question_required"}},"required":["status"]},"then":{
                "required":["data","next_actions"],
                "properties":{
                    "data":{"type":"object","required":["question"],"properties":{"question":{"type":"object"}}},
                    "next_actions":{"minItems":1,"contains":{"type":"object","properties":{
                        "tool_name":{"const":"star_tool_call_write_closed"},
                        "arguments":{"type":"object","properties":{"tool_id":{"const":"star.core.goal.answer"}},"required":["tool_id"]}
                    },"required":["tool_name","arguments"]}},
                    "error":{"type":"null"}
                }
            }},
            {"if":{"properties":{"status":{"const":"blocked"}},"required":["status"]},"then":{
                "properties":{"operation_id":{"type":"null"}},
                "anyOf":[
                    {"properties":{"diagnostic_refs":{"minItems":1}},"required":["diagnostic_refs"]},
                    {"properties":{"error":error_schema()},"required":["error"]},
                    {"properties":{"data":{"type":"object","anyOf":[{"required":["policy_basis"]},{"required":["diagnostic"]}]}},"required":["data"]}
                ]
            }},
            {"if":{"properties":{"status":{"const":"error"}},"required":["status"]},"then":{
                "required":["error"],
                "properties":{"error":error_schema(),"data":{"type":"null"},"operation_id":{"type":"null"}}
            }}
        ]
    }))
}

const JSON_SAFE_INTEGER_MAX: u64 = 9_007_199_254_740_991;

fn bounded_text(value: &str, minimum: usize, maximum: usize) -> bool {
    !value.contains('\0') && (minimum..=maximum).contains(&value.chars().count())
}

fn all_unique<T: PartialEq>(values: &[T]) -> bool {
    values
        .iter()
        .enumerate()
        .all(|(index, value)| !values[..index].contains(value))
}

fn global_id(value: &str) -> bool {
    (3..=128).contains(&value.len())
        && regex::Regex::new(r"^[a-z][a-z0-9]*(?:[._-][a-z0-9]+){1,7}$")
            .expect("static global ID regex")
            .is_match(value)
}

fn namespace(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && regex::Regex::new(r"^[a-z][a-z0-9]*(?:[._-][a-z0-9]+){0,7}$")
            .expect("static namespace regex")
            .is_match(value)
}

fn tag(value: &str) -> bool {
    regex::Regex::new(r"^[a-z][a-z0-9_-]{0,31}$")
        .expect("static tag regex")
        .is_match(value)
}

fn prefixed_ulid(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|raw| {
        raw.len() == 26
            && !raw.bytes().any(|byte| byte.is_ascii_lowercase())
            && ulid::Ulid::from_string(raw).is_ok()
    })
}

/// Exact validation used by the bounded STDIO supervisor before rmcp can
/// dispatch a fixed tool call. Schemars describes the same surface, while
/// this function enforces cross-field and canonical-byte bounds.
pub fn fixed_input_valid(name: &str, arguments: serde_json::Value) -> bool {
    match name {
        "star_tool_search" => serde_json::from_value::<SearchInput>(arguments)
            .ok()
            .is_some_and(|input| {
                bounded_text(&input.query, 1, 256)
                    && !input.query.trim().is_empty()
                    && input.namespaces.as_ref().is_none_or(|values| {
                        values.len() <= 16
                            && all_unique(values)
                            && values.iter().all(|value| namespace(value))
                    })
                    && input.tags.as_ref().is_none_or(|values| {
                        values.len() <= 32
                            && all_unique(values)
                            && values.iter().all(|value| tag(value))
                    })
                    && input.task_kinds.as_ref().is_none_or(|values| {
                        values.len() <= 16
                            && all_unique(values)
                            && values.iter().all(|value| tag(value))
                    })
                    && input
                        .sources
                        .as_ref()
                        .is_none_or(|values| values.len() <= 3 && all_unique(values))
                    && input
                        .readiness
                        .as_ref()
                        .is_none_or(|values| values.len() <= 5 && all_unique(values))
                    && input
                        .risk_lanes
                        .as_ref()
                        .is_none_or(|values| values.len() <= 6 && all_unique(values))
                    && input.limit.is_none_or(|limit| (1..=50).contains(&limit))
                    && input.cursor.as_ref().is_none_or(|cursor| {
                        !cursor.contains('\0') && cursor.chars().count() <= 1_024
                    })
            }),
        "star_tool_describe" => serde_json::from_value::<DescribeInput>(arguments)
            .ok()
            .is_some_and(|input| global_id(&input.tool_id)),
        "star_tool_registry_status" => serde_json::from_value::<RegistryStatusInput>(arguments)
            .ok()
            .is_some_and(|input| {
                input.package_id.as_ref().is_none_or(|id| global_id(id))
                    && input
                        .sources
                        .as_ref()
                        .is_none_or(|values| values.len() <= 3 && all_unique(values))
                    && input.limit.is_none_or(|limit| (1..=200).contains(&limit))
                    && input.cursor.as_ref().is_none_or(|cursor| {
                        !cursor.contains('\0') && cursor.chars().count() <= 1_024
                    })
            }),
        "star_tool_call_read_closed"
        | "star_tool_call_read_open"
        | "star_tool_call_write_closed"
        | "star_tool_call_destructive_closed"
        | "star_tool_call_write_open"
        | "star_tool_call_destructive_open" => {
            let canonical_size = arguments
                .get("arguments")
                .ok_or(crate::canonical::CanonicalError::Jcs(
                    "missing action arguments".to_owned(),
                ))
                .and_then(crate::canonical::jcs_bytes)
                .map(|bytes| bytes.len());
            serde_json::from_value::<CallInput>(arguments)
                .ok()
                .is_some_and(|input| {
                    global_id(&input.tool_id)
                        && canonical_size.is_ok_and(|size| size <= 4 * 1024 * 1024)
                        && input
                            .idempotency_key
                            .as_ref()
                            .is_none_or(|key| bounded_text(key, 1, 128))
                        && input
                            .goal_id
                            .as_ref()
                            .is_none_or(|id| prefixed_ulid(id, "gol_"))
                        && input
                            .expected_revision
                            .is_none_or(|revision| revision <= JSON_SAFE_INTEGER_MAX)
                        && input
                            .requested_timeout_ms
                            .is_none_or(|timeout| (100..=86_400_000).contains(&timeout))
                })
        }
        "star_tool_operation_get" => serde_json::from_value::<OperationGetInput>(arguments)
            .ok()
            .is_some_and(|input| {
                input
                    .after_sequence
                    .is_none_or(|value| value <= JSON_SAFE_INTEGER_MAX)
                    && input.wait_ms.is_none_or(|value| value <= 30_000)
            }),
        "star_tool_operation_cancel" => serde_json::from_value::<OperationCancelInput>(arguments)
            .ok()
            .is_some_and(|input| {
                input
                    .reason
                    .as_ref()
                    .is_none_or(|reason| bounded_text(reason, 0, 512))
                    && input.force_after_ms.is_none_or(|value| value <= 30_000)
            }),
        "star_approval_resolve" => serde_json::from_value::<ApprovalResolveInput>(arguments)
            .ok()
            .is_some_and(|input| {
                input
                    .reason
                    .as_ref()
                    .is_none_or(|reason| bounded_text(reason, 0, 1_000))
            }),
        _ => false,
    }
}
