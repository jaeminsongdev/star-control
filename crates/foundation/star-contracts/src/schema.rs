use schemars::JsonSchema;
use serde_json::{Map, Value};

use crate::{
    evidence::{
        DIAGNOSTIC_SCHEMA_ID, Diagnostic, EVIDENCE_BUNDLE_SCHEMA_ID, EvidenceBundle,
        GATE_DECISION_SCHEMA_ID, GateDecision, VALIDATION_RUN_SCHEMA_ID, ValidationRun,
    },
    fixed_mcp::{CallInput, McpToolResult, fixed_input_schema, fixed_result_schema},
    ipc::{IpcChallenge, IpcHandshakeError, IpcHello, IpcRequest, IpcResponse, IpcWelcome},
    manifest::ToolPackageManifest,
    registry::{RegistrySnapshot, ToolRegistryCache},
    runtime::{
        ExecutableIdentity, ExternalToolCancel, ExternalToolCancelAck, ExternalToolProbeRequest,
        ExternalToolProbeResponse, ExternalToolProgress, ExternalToolRequest, ExternalToolResponse,
    },
    trust::ToolTrustRecord,
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
    let mut documents = vec![
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
            "tool-registry-cache.schema.json",
            schema_document::<ToolRegistryCache>("tool-registry-cache"),
        ),
        (
            "tool-trust-record.schema.json",
            schema_document::<ToolTrustRecord>("tool-trust-record"),
        ),
        (
            "external-tool-request.schema.json",
            schema_document::<ExternalToolRequest>("external-tool-request"),
        ),
        (
            "external-tool-response.schema.json",
            schema_document::<ExternalToolResponse>("external-tool-response"),
        ),
        (
            "external-tool-progress.schema.json",
            schema_document::<ExternalToolProgress>("external-tool-progress"),
        ),
        (
            "external-tool-cancel.schema.json",
            schema_document::<ExternalToolCancel>("external-tool-cancel"),
        ),
        (
            "external-tool-cancel-ack.schema.json",
            schema_document::<ExternalToolCancelAck>("external-tool-cancel-ack"),
        ),
        (
            "external-tool-probe-request.schema.json",
            schema_document::<ExternalToolProbeRequest>("external-tool-probe-request"),
        ),
        (
            "external-tool-probe-response.schema.json",
            schema_document::<ExternalToolProbeResponse>("external-tool-probe-response"),
        ),
        (
            "executable-identity.schema.json",
            schema_document::<ExecutableIdentity>("executable-identity"),
        ),
        ("ipc.schema.json", ipc_schema()),
        (
            "mcp-star-tool-search.input.schema.json",
            fixed_input_schema("star_tool_search").expect("fixed search input schema"),
        ),
        (
            "mcp-star-tool-describe.input.schema.json",
            fixed_input_schema("star_tool_describe").expect("fixed describe input schema"),
        ),
        (
            "mcp-star-tool-registry-status.input.schema.json",
            fixed_input_schema("star_tool_registry_status").expect("fixed status input schema"),
        ),
        (
            "mcp-star-tool-call.input.schema.json",
            schema_document::<CallInput>("star.mcp.star_tool_call.input"),
        ),
        (
            "mcp-star-tool-operation-get.input.schema.json",
            fixed_input_schema("star_tool_operation_get").expect("fixed operation input schema"),
        ),
        (
            "mcp-star-tool-operation-cancel.input.schema.json",
            fixed_input_schema("star_tool_operation_cancel")
                .expect("fixed cancellation input schema"),
        ),
        (
            "mcp-star-approval-resolve.input.schema.json",
            fixed_input_schema("star_approval_resolve").expect("fixed approval input schema"),
        ),
        (
            "mcp-tool-result.schema.json",
            schema_document::<McpToolResult>("star.mcp.tool.result"),
        ),
    ];
    for (file, tool_name) in [
        (
            "mcp-star-tool-call-read-closed.input.schema.json",
            "star_tool_call_read_closed",
        ),
        (
            "mcp-star-tool-call-read-open.input.schema.json",
            "star_tool_call_read_open",
        ),
        (
            "mcp-star-tool-call-write-closed.input.schema.json",
            "star_tool_call_write_closed",
        ),
        (
            "mcp-star-tool-call-destructive-closed.input.schema.json",
            "star_tool_call_destructive_closed",
        ),
        (
            "mcp-star-tool-call-write-open.input.schema.json",
            "star_tool_call_write_open",
        ),
        (
            "mcp-star-tool-call-destructive-open.input.schema.json",
            "star_tool_call_destructive_open",
        ),
    ] {
        documents.push((
            file,
            fixed_input_schema(tool_name).expect("fixed lane input schema"),
        ));
    }
    for (file, schema_id) in [
        (
            "mcp-star-tool-search.result.schema.json",
            "star.mcp.star_tool_search.result",
        ),
        (
            "mcp-star-tool-describe.result.schema.json",
            "star.mcp.star_tool_describe.result",
        ),
        (
            "mcp-star-tool-registry-status.result.schema.json",
            "star.mcp.star_tool_registry_status.result",
        ),
        (
            "mcp-star-tool-call-read-closed.result.schema.json",
            "star.mcp.star_tool_call_read_closed.result",
        ),
        (
            "mcp-star-tool-call-read-open.result.schema.json",
            "star.mcp.star_tool_call_read_open.result",
        ),
        (
            "mcp-star-tool-call-write-closed.result.schema.json",
            "star.mcp.star_tool_call_write_closed.result",
        ),
        (
            "mcp-star-tool-call-destructive-closed.result.schema.json",
            "star.mcp.star_tool_call_destructive_closed.result",
        ),
        (
            "mcp-star-tool-call-write-open.result.schema.json",
            "star.mcp.star_tool_call_write_open.result",
        ),
        (
            "mcp-star-tool-call-destructive-open.result.schema.json",
            "star.mcp.star_tool_call_destructive_open.result",
        ),
        (
            "mcp-star-tool-operation-get.result.schema.json",
            "star.mcp.star_tool_operation_get.result",
        ),
        (
            "mcp-star-tool-operation-cancel.result.schema.json",
            "star.mcp.star_tool_operation_cancel.result",
        ),
        (
            "mcp-star-approval-resolve.result.schema.json",
            "star.mcp.star_approval_resolve.result",
        ),
    ] {
        let tool_name = schema_id
            .strip_prefix("star.mcp.")
            .and_then(|value| value.strip_suffix(".result"))
            .expect("fixed result schema ID");
        documents.push((
            file,
            fixed_result_schema(tool_name).expect("fixed result schema"),
        ));
    }
    documents
}

fn ipc_schema() -> Value {
    let mut definitions = Map::new();
    definitions.insert(
        "challenge".to_owned(),
        schema_document::<IpcChallenge>("ipc.challenge"),
    );
    definitions.insert("hello".to_owned(), schema_document::<IpcHello>("ipc.hello"));
    definitions.insert(
        "welcome".to_owned(),
        schema_document::<IpcWelcome>("ipc.welcome"),
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
