use schemars::JsonSchema;
use serde_json::{Map, Value};

use crate::{
    evidence::{
        DIAGNOSTIC_SCHEMA_ID, Diagnostic, EVIDENCE_BUNDLE_SCHEMA_ID, EvidenceBundle,
        GATE_DECISION_SCHEMA_ID, GateDecision, VALIDATION_RUN_SCHEMA_ID, ValidationRun,
    },
    fixed_mcp::{
        ApprovalResolveInput, CallInput, DescribeInput, McpToolResult, OperationCancelInput,
        OperationGetInput, RegistryStatusInput, SearchInput,
    },
    ipc::{IpcChallenge, IpcHandshakeError, IpcRequest, IpcResponse},
    manifest::ToolPackageManifest,
    registry::RegistrySnapshot,
    runtime::{ExternalToolRequest, ExternalToolResponse},
};

pub fn schema_document<T: JsonSchema>(schema_id: &str) -> Value {
    let mut value =
        serde_json::to_value(schemars::schema_for!(T)).expect("schemars JSON serialization");
    let object = value.as_object_mut().expect("schema root is an object");
    object.insert(
        "$schema".to_owned(),
        Value::String("https://json-schema.org/draft/2020-12/schema".to_owned()),
    );
    object.insert(
        "$id".to_owned(),
        Value::String(format!("urn:star-control:schema:{schema_id}:v1")),
    );
    value
}

pub fn generated_documents() -> Vec<(&'static str, Value)> {
    vec![
        (
            "validation-run.schema.json",
            schema_document::<ValidationRun>(VALIDATION_RUN_SCHEMA_ID),
        ),
        (
            "gate-decision.schema.json",
            schema_document::<GateDecision>(GATE_DECISION_SCHEMA_ID),
        ),
        (
            "evidence-bundle.schema.json",
            schema_document::<EvidenceBundle>(EVIDENCE_BUNDLE_SCHEMA_ID),
        ),
        (
            "diagnostic.schema.json",
            schema_document::<Diagnostic>(DIAGNOSTIC_SCHEMA_ID),
        ),
        (
            "tool-package-manifest.schema.json",
            schema_document::<ToolPackageManifest>("tool-package-manifest"),
        ),
        (
            "tool-registry-snapshot.schema.json",
            schema_document::<RegistrySnapshot>("tool-registry-snapshot"),
        ),
        (
            "external-tool-request.schema.json",
            schema_document::<ExternalToolRequest>("external-tool-request"),
        ),
        (
            "external-tool-response.schema.json",
            schema_document::<ExternalToolResponse>("external-tool-response"),
        ),
        ("ipc.schema.json", ipc_schema()),
        (
            "mcp-star-tool-search.input.schema.json",
            schema_document::<SearchInput>("star.mcp.star_tool_search.input"),
        ),
        (
            "mcp-star-tool-describe.input.schema.json",
            schema_document::<DescribeInput>("star.mcp.star_tool_describe.input"),
        ),
        (
            "mcp-star-tool-registry-status.input.schema.json",
            schema_document::<RegistryStatusInput>("star.mcp.star_tool_registry_status.input"),
        ),
        (
            "mcp-star-tool-call.input.schema.json",
            schema_document::<CallInput>("star.mcp.star_tool_call.input"),
        ),
        (
            "mcp-star-tool-operation-get.input.schema.json",
            schema_document::<OperationGetInput>("star.mcp.star_tool_operation_get.input"),
        ),
        (
            "mcp-star-tool-operation-cancel.input.schema.json",
            schema_document::<OperationCancelInput>("star.mcp.star_tool_operation_cancel.input"),
        ),
        (
            "mcp-star-approval-resolve.input.schema.json",
            schema_document::<ApprovalResolveInput>("star.mcp.star_approval_resolve.input"),
        ),
        (
            "mcp-tool-result.schema.json",
            schema_document::<McpToolResult>("star.mcp.tool.result"),
        ),
    ]
}

fn ipc_schema() -> Value {
    let mut definitions = Map::new();
    definitions.insert(
        "challenge".to_owned(),
        schema_document::<IpcChallenge>("ipc.challenge"),
    );
    definitions.insert(
        "handshake_error".to_owned(),
        schema_document::<IpcHandshakeError>("ipc.handshake-error"),
    );
    definitions.insert(
        "request".to_owned(),
        schema_document::<IpcRequest>("ipc.request"),
    );
    definitions.insert(
        "response".to_owned(),
        schema_document::<IpcResponse>("ipc.response"),
    );
    let mut root = Map::new();
    root.insert(
        "$schema".to_owned(),
        Value::String("https://json-schema.org/draft/2020-12/schema".to_owned()),
    );
    root.insert(
        "$id".to_owned(),
        Value::String("urn:star-control:schema:ipc:v1".to_owned()),
    );
    root.insert("$defs".to_owned(), Value::Object(definitions));
    Value::Object(root)
}
