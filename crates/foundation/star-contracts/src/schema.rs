use schemars::JsonSchema;
use serde_json::{Map, Value};

use crate::{
    evidence::{
        ArtifactRef, DIAGNOSTIC_SCHEMA_ID, Diagnostic, EVIDENCE_BUNDLE_SCHEMA_ID, EvidenceBundle,
        GATE_DECISION_SCHEMA_ID, GateDecision, VALIDATION_RUN_SCHEMA_ID, ValidationRun,
    },
    fixed_mcp::{CallInput, McpToolResult, fixed_input_schema, fixed_result_schema},
    installation::{
        CODEX_INTEGRATION_RECORD_SCHEMA_ID, CodexIntegrationRecord, INSTALLATION_RECORD_SCHEMA_ID,
        InstallationRecord, RELEASE_FILE_MANIFEST_SCHEMA_ID, ReleaseFileManifest,
    },
    ipc::{IpcChallenge, IpcHandshakeError, IpcHello, IpcRequest, IpcResponse, IpcWelcome},
    management::{
        Baseline, CanonicalSource, ChangePlan, ChangeRecipe, CoordinatedOperation, Disposition,
        Finding, ManagementStoreStatus, Occurrence, PatchSet, Project, ProjectRevision, Rule,
        ScanRun, Suppression, Symbol, SymbolReference, ValidationResult, WorkspaceSnapshot,
    },
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

fn management_schema_document<T: JsonSchema>(schema_id: &str) -> Value {
    let mut value = schema_document::<T>(schema_id);
    let properties = value
        .as_object_mut()
        .and_then(|root| root.get_mut("properties"))
        .and_then(Value::as_object_mut)
        .expect("management contract schema has object properties");
    properties.insert(
        "schema_id".to_owned(),
        serde_json::json!({"type":"string","const":schema_id}),
    );
    properties.insert(
        "schema_version".to_owned(),
        serde_json::json!({"type":"integer","const":1}),
    );
    strengthen_management_scalars(&mut value, None);
    value
}

fn strengthen_management_scalars(value: &mut Value, property_name: Option<&str>) {
    match value {
        Value::Array(values) => {
            for value in values {
                strengthen_management_scalars(value, property_name);
            }
        }
        Value::Object(object) => {
            if property_name == Some("Sha256Hash") {
                object.insert(
                    "pattern".to_owned(),
                    Value::String("^sha256:[0-9a-f]{64}$".to_owned()),
                );
            }
            if let Some(name) = property_name
                && let Some(prefix) = management_id_prefix(name)
            {
                let pattern = if management_id_is_source_derived(name) {
                    format!("^{prefix}[a-z2-7]{{52}}$")
                } else {
                    format!("^{prefix}[0-9A-HJKMNP-TV-Z]{{26}}$")
                };
                if let Some(items) = object.get_mut("items") {
                    if let Some(items) = items.as_object_mut() {
                        items.insert("pattern".to_owned(), Value::String(pattern));
                    }
                } else {
                    object.insert("pattern".to_owned(), Value::String(pattern));
                }
            }
            if let Some(properties) = object.get_mut("properties").and_then(Value::as_object_mut) {
                for (name, property) in properties {
                    strengthen_management_scalars(property, Some(name));
                }
            }
            if let Some(definitions) = object.get_mut("$defs").and_then(Value::as_object_mut) {
                for (name, definition) in definitions {
                    strengthen_management_scalars(definition, Some(name));
                }
            }
            for keyword in ["items", "anyOf", "oneOf", "allOf"] {
                if let Some(nested) = object.get_mut(keyword) {
                    strengthen_management_scalars(nested, property_name);
                }
            }
        }
        _ => {}
    }
}

fn management_id_is_source_derived(name: &str) -> bool {
    name.contains("project_revision")
        || name.contains("source_revision")
        || name == "scope_revision"
        || name == "latest_revision_id"
        || name.contains("workspace_snapshot")
        || (name.contains("finding") && !name.contains("fingerprint"))
        || (name.contains("occurrence") && !name.contains("fingerprint"))
        || name.ends_with("symbol_id")
        || name.contains("symbol_reference")
        || name.contains("canonical_source")
        || name.ends_with("source_id")
        || name.contains("generated_from")
}

fn management_id_prefix(name: &str) -> Option<&'static str> {
    if name == "project_id" {
        Some("prj_")
    } else if name.contains("project_revision")
        || name.contains("source_revision")
        || name == "scope_revision"
        || name == "latest_revision_id"
    {
        Some("prv_")
    } else if name.contains("workspace_snapshot") {
        Some("wsp_")
    } else if name.contains("scan_run")
        || name.ends_with("scan_id")
        || name.contains("observed_scan_id")
    {
        Some("scn_")
    } else if name.contains("finding") && !name.contains("fingerprint") {
        Some("fnd_")
    } else if name.contains("occurrence") && !name.contains("fingerprint") {
        Some("occ_")
    } else if name.contains("symbol_reference") {
        Some("srf_")
    } else if name.ends_with("symbol_id") {
        Some("sym_")
    } else if name.contains("canonical_source")
        || name.ends_with("source_id")
        || name.contains("generated_from")
    {
        Some("src_")
    } else if name.contains("suppression_id") {
        Some("sup_")
    } else if name.contains("baseline_id") {
        Some("bas_")
    } else if name.contains("disposition_id") {
        Some("dsp_")
    } else if name.contains("change_plan_id") {
        Some("cpl_")
    } else if name.contains("patch_set_id") {
        Some("pat_")
    } else if name.contains("validation_result_id") {
        Some("vrs_")
    } else if name.contains("gate_decision_id") {
        Some("gtd_")
    } else if name.contains("artifact_id") {
        Some("art_")
    } else if name.contains("root_binding_id") {
        Some("rtb_")
    } else if name.contains("generation_id") {
        Some("gen_")
    } else if name.contains("coordinated_operation_id") || name == "operation_id" {
        Some("cop_")
    } else if name.contains("store_id") {
        Some("mst_")
    } else {
        None
    }
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
            "artifact-ref.schema.json",
            schema_document::<ArtifactRef>("star.artifact-ref"),
        ),
        (
            "release-file-manifest.schema.json",
            schema_document::<ReleaseFileManifest>(RELEASE_FILE_MANIFEST_SCHEMA_ID),
        ),
        (
            "installation-record.schema.json",
            schema_document::<InstallationRecord>(INSTALLATION_RECORD_SCHEMA_ID),
        ),
        (
            "codex-integration-record.schema.json",
            schema_document::<CodexIntegrationRecord>(CODEX_INTEGRATION_RECORD_SCHEMA_ID),
        ),
        (
            "project.schema.json",
            management_schema_document::<Project>("star.project"),
        ),
        (
            "project-revision.schema.json",
            management_schema_document::<ProjectRevision>("star.project-revision"),
        ),
        (
            "workspace-snapshot.schema.json",
            management_schema_document::<WorkspaceSnapshot>("star.workspace-snapshot"),
        ),
        (
            "scan-run.schema.json",
            management_schema_document::<ScanRun>("star.scan-run"),
        ),
        (
            "rule.schema.json",
            management_schema_document::<Rule>("star.rule"),
        ),
        (
            "finding.schema.json",
            management_schema_document::<Finding>("star.finding"),
        ),
        (
            "occurrence.schema.json",
            management_schema_document::<Occurrence>("star.occurrence"),
        ),
        (
            "symbol.schema.json",
            management_schema_document::<Symbol>("star.symbol"),
        ),
        (
            "symbol-reference.schema.json",
            management_schema_document::<SymbolReference>("star.symbol-reference"),
        ),
        (
            "canonical-source.schema.json",
            management_schema_document::<CanonicalSource>("star.canonical-source"),
        ),
        (
            "suppression.schema.json",
            management_schema_document::<Suppression>("star.suppression"),
        ),
        (
            "baseline.schema.json",
            management_schema_document::<Baseline>("star.baseline"),
        ),
        (
            "disposition.schema.json",
            management_schema_document::<Disposition>("star.disposition"),
        ),
        (
            "change-plan.schema.json",
            management_schema_document::<ChangePlan>("star.change-plan"),
        ),
        (
            "patch-set.schema.json",
            management_schema_document::<PatchSet>("star.patch-set"),
        ),
        (
            "change-recipe.schema.json",
            management_schema_document::<ChangeRecipe>("star.change-recipe"),
        ),
        (
            "validation-result.schema.json",
            management_schema_document::<ValidationResult>("star.validation-result"),
        ),
        (
            "management-store-status.schema.json",
            management_schema_document::<ManagementStoreStatus>("star.management-store-status"),
        ),
        (
            "coordinated-operation.schema.json",
            management_schema_document::<CoordinatedOperation>("star.coordinated-operation"),
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
