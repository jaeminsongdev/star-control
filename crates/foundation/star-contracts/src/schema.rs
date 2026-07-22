use schemars::JsonSchema;
use serde_json::{Map, Value};

use crate::{
    development::{
        CHANGE_BUNDLE_HANDOFF_SCHEMA_ID, CHANGE_BUNDLE_SCHEMA_ID,
        CLEAN_ROOM_DOCTOR_REPORT_SCHEMA_ID, COMPATIBILITY_REPORT_SCHEMA_ID, ChangeBundle,
        ChangeBundleHandoff, CleanRoomDoctorReport, CompatibilityReport,
        MAINTENANCE_RADAR_SCHEMA_ID, MANAGED_REGISTRY_SNAPSHOT_SCHEMA_ID, MIGRATION_RUN_SCHEMA_ID,
        MaintenanceRadar, ManagedRegistrySnapshot, MigrationRun, PERFORMANCE_COMPARISON_SCHEMA_ID,
        PerformanceComparison, REPRODUCTION_PACK_SCHEMA_ID, ReproductionPack,
    },
    evidence::{
        ArtifactRef, DIAGNOSTIC_SCHEMA_ID, Diagnostic, EVIDENCE_BUNDLE_SCHEMA_ID, EvidenceBundle,
        GATE_DECISION_SCHEMA_ID, GateDecision, VALIDATION_PLAN_SCHEMA_ID, VALIDATION_RUN_SCHEMA_ID,
        ValidationPlan, ValidationRun,
    },
    evidence_v2::{
        DIAGNOSTIC_V2_SCHEMA_ID, DiagnosticV2, EVIDENCE_BUNDLE_V2_SCHEMA_ID, EvidenceBundleV2,
        GATE_DECISION_V2_SCHEMA_ID, GateDecisionV2, TASK_INVOCATION_V2_SCHEMA_ID, TaskInvocationV2,
        VALIDATION_RUN_V2_SCHEMA_ID, ValidationRunV2,
    },
    fixed_mcp::{CallInput, McpToolResult, fixed_input_schema, fixed_result_schema},
    index::{CodeIndexSnapshot, ProjectCatalogSnapshot},
    installation::{
        CODEX_INTEGRATION_RECORD_SCHEMA_ID, CodexIntegrationRecord, INSTALLATION_RECORD_SCHEMA_ID,
        INTEGRATION_CANDIDATE_REVIEW_SCHEMA_ID, InstallationRecord, IntegrationCandidateReview,
        RELEASE_FILE_MANIFEST_SCHEMA_ID, RUNTIME_ACTIVATION_RECORD_SCHEMA_ID,
        RUNTIME_CANDIDATE_REVIEW_SCHEMA_ID, RUNTIME_GENERATION_MANIFEST_SCHEMA_ID,
        ReleaseFileManifest, RuntimeActivationRecord, RuntimeCandidateReview,
        RuntimeGenerationManifest,
    },
    ipc::{IpcChallenge, IpcHandshakeError, IpcHello, IpcRequest, IpcResponse, IpcWelcome},
    management::{
        Baseline, CanonicalSource, ChangePlan, ChangeRecipe, CoordinatedOperation, Disposition,
        Finding, ManagementStoreStatus, Occurrence, PatchSet, Project, ProjectCheckout,
        ProjectRevision, ProjectV1, ProjectV1ToV2MigrationPlan, ProjectV1ToV2MigrationResult, Rule,
        ScanRun, Suppression, Symbol, SymbolReference, ValidationResult, WorkspaceSnapshot,
    },
    manifest::ToolPackageManifest,
    orchestration::{GOAL_RECORD_SCHEMA_ID, GoalRecord},
    planning::{
        CHANGE_SET_SCHEMA_ID, ChangeSet, FULL_VALIDATION_PLAN_SCHEMA_ID, FullValidationPlan,
        IMPACT_ANALYSIS_SCHEMA_ID, ImpactAnalysis, PlanningBundle, RISK_PATH_DESCRIPTOR_SCHEMA_ID,
        RiskPathDescriptor, SCOPE_REVISION_SCHEMA_ID, ScopeRevision, TASK_SPEC_SCHEMA_ID, TaskSpec,
    },
    registry::{RegistrySnapshot, ToolRegistryCache},
    release_v2::{
        EVALUATION_CATALOG_ITEM_SCHEMA_ID, EVALUATION_RUN_V2_SCHEMA_ID, EvaluationCatalogItem,
        EvaluationRunV2, RELEASE_MANIFEST_V2_SCHEMA_ID, ReleaseManifestV2,
    },
    runtime::{
        ExecutableIdentity, ExternalToolCancel, ExternalToolCancelAck, ExternalToolProbeRequest,
        ExternalToolProbeResponse, ExternalToolProgress, ExternalToolRequest, ExternalToolResponse,
    },
    rust_style::{
        RUST_STYLE_COVERAGE_MATRIX_SCHEMA_ID, RUST_STYLE_POLICY_SNAPSHOT_SCHEMA_ID,
        RUST_STYLE_STEP_EXECUTION_SCHEMA_ID, RUST_TOOLCHAIN_BINDING_SCHEMA_ID,
        RustStyleCoverageMatrix, RustStylePolicySnapshot, RustStyleStepExecution,
        RustToolchainBinding,
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
    management_schema_document_version::<T>(schema_id, 1)
}

fn management_schema_document_version<T: JsonSchema>(schema_id: &str, version: u32) -> Value {
    let mut value = schema_document::<T>(schema_id);
    value
        .as_object_mut()
        .expect("schema root is an object")
        .insert(
            "$id".to_owned(),
            Value::String(format!("urn:star-control:schema:{schema_id}:v{version}")),
        );
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
        serde_json::json!({"type":"integer","const":version}),
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
    name.contains("project_catalog_snapshot")
        || name.contains("code_index_snapshot")
        || name.contains("project_revision")
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
    if name.contains("task_spec_id") {
        Some("tsk_")
    } else if name.contains("scope_revision_id") {
        Some("scp_")
    } else if name.contains("impact_analysis_id") {
        Some("imp_")
    } else if name.contains("change_set_id") {
        Some("chg_")
    } else if name.contains("validation_plan_id") {
        Some("vpl_")
    } else if name == "goal_id" {
        Some("gol_")
    } else if name == "run_id" {
        Some("run_")
    } else if name.contains("project_catalog_snapshot") {
        Some("pcs_")
    } else if name.contains("code_index_snapshot") {
        Some("cix_")
    } else if name == "project_id" {
        Some("prj_")
    } else if name == "checkout_id" || name.ends_with("checkout_ids") {
        Some("cko_")
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
    } else if name.contains("release_manifest_id") {
        Some("rel_")
    } else if name.contains("evaluation_run_id") {
        Some("evr_")
    } else {
        None
    }
}

pub fn generated_documents() -> Vec<(&'static str, Value)> {
    let mut documents = vec![
        (
            "validation-plan.schema.json",
            schema_document::<ValidationPlan>(VALIDATION_PLAN_SCHEMA_ID),
        ),
        (
            "validation-plan-v2.schema.json",
            management_schema_document_version::<FullValidationPlan>(
                FULL_VALIDATION_PLAN_SCHEMA_ID,
                2,
            ),
        ),
        (
            "task-spec.schema.json",
            management_schema_document::<TaskSpec>(TASK_SPEC_SCHEMA_ID),
        ),
        (
            "scope-revision.schema.json",
            management_schema_document::<ScopeRevision>(SCOPE_REVISION_SCHEMA_ID),
        ),
        (
            "change-set.schema.json",
            management_schema_document::<ChangeSet>(CHANGE_SET_SCHEMA_ID),
        ),
        (
            "impact-analysis.schema.json",
            management_schema_document::<ImpactAnalysis>(IMPACT_ANALYSIS_SCHEMA_ID),
        ),
        (
            "risk-path-descriptor.schema.json",
            management_schema_document::<RiskPathDescriptor>(RISK_PATH_DESCRIPTOR_SCHEMA_ID),
        ),
        (
            "planning-bundle.schema.json",
            management_schema_document::<PlanningBundle>("star.planning-bundle"),
        ),
        (
            "goal-record.schema.json",
            management_schema_document::<GoalRecord>(GOAL_RECORD_SCHEMA_ID),
        ),
        (
            "managed-registry-snapshot.schema.json",
            management_schema_document::<ManagedRegistrySnapshot>(
                MANAGED_REGISTRY_SNAPSHOT_SCHEMA_ID,
            ),
        ),
        (
            "compatibility-report.schema.json",
            management_schema_document::<CompatibilityReport>(COMPATIBILITY_REPORT_SCHEMA_ID),
        ),
        (
            "clean-room-doctor-report.schema.json",
            management_schema_document::<CleanRoomDoctorReport>(CLEAN_ROOM_DOCTOR_REPORT_SCHEMA_ID),
        ),
        (
            "reproduction-pack.schema.json",
            management_schema_document::<ReproductionPack>(REPRODUCTION_PACK_SCHEMA_ID),
        ),
        (
            "maintenance-radar.schema.json",
            management_schema_document::<MaintenanceRadar>(MAINTENANCE_RADAR_SCHEMA_ID),
        ),
        (
            "migration-run.schema.json",
            management_schema_document::<MigrationRun>(MIGRATION_RUN_SCHEMA_ID),
        ),
        (
            "performance-comparison.schema.json",
            management_schema_document::<PerformanceComparison>(PERFORMANCE_COMPARISON_SCHEMA_ID),
        ),
        (
            "change-bundle.schema.json",
            management_schema_document::<ChangeBundle>(CHANGE_BUNDLE_SCHEMA_ID),
        ),
        (
            "change-bundle-handoff.schema.json",
            management_schema_document::<ChangeBundleHandoff>(CHANGE_BUNDLE_HANDOFF_SCHEMA_ID),
        ),
        (
            "release-manifest-v2.schema.json",
            management_schema_document_version::<ReleaseManifestV2>(
                RELEASE_MANIFEST_V2_SCHEMA_ID,
                2,
            ),
        ),
        (
            "evaluation-run-v2.schema.json",
            management_schema_document_version::<EvaluationRunV2>(EVALUATION_RUN_V2_SCHEMA_ID, 2),
        ),
        (
            "evaluation-catalog-item.schema.json",
            management_schema_document::<EvaluationCatalogItem>(EVALUATION_CATALOG_ITEM_SCHEMA_ID),
        ),
        (
            "rust-toolchain-binding.schema.json",
            management_schema_document::<RustToolchainBinding>(RUST_TOOLCHAIN_BINDING_SCHEMA_ID),
        ),
        (
            "rust-style-policy-snapshot.schema.json",
            management_schema_document::<RustStylePolicySnapshot>(
                RUST_STYLE_POLICY_SNAPSHOT_SCHEMA_ID,
            ),
        ),
        (
            "rust-style-coverage-matrix.schema.json",
            management_schema_document::<RustStyleCoverageMatrix>(
                RUST_STYLE_COVERAGE_MATRIX_SCHEMA_ID,
            ),
        ),
        (
            "rust-style-step-execution.schema.json",
            management_schema_document::<RustStyleStepExecution>(
                RUST_STYLE_STEP_EXECUTION_SCHEMA_ID,
            ),
        ),
        (
            "validation-run.schema.json",
            schema_document::<ValidationRun>(VALIDATION_RUN_SCHEMA_ID),
        ),
        (
            "task-invocation-v2.schema.json",
            management_schema_document_version::<TaskInvocationV2>(TASK_INVOCATION_V2_SCHEMA_ID, 2),
        ),
        (
            "validation-run-v2.schema.json",
            management_schema_document_version::<ValidationRunV2>(VALIDATION_RUN_V2_SCHEMA_ID, 2),
        ),
        (
            "gate-decision-v2.schema.json",
            management_schema_document_version::<GateDecisionV2>(GATE_DECISION_V2_SCHEMA_ID, 2),
        ),
        (
            "evidence-bundle-v2.schema.json",
            management_schema_document_version::<EvidenceBundleV2>(EVIDENCE_BUNDLE_V2_SCHEMA_ID, 2),
        ),
        (
            "diagnostic-v2.schema.json",
            management_schema_document_version::<DiagnosticV2>(DIAGNOSTIC_V2_SCHEMA_ID, 2),
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
            "runtime-generation-manifest.schema.json",
            schema_document::<RuntimeGenerationManifest>(RUNTIME_GENERATION_MANIFEST_SCHEMA_ID),
        ),
        (
            "runtime-activation-record.schema.json",
            schema_document::<RuntimeActivationRecord>(RUNTIME_ACTIVATION_RECORD_SCHEMA_ID),
        ),
        (
            "runtime-candidate-review.schema.json",
            schema_document::<RuntimeCandidateReview>(RUNTIME_CANDIDATE_REVIEW_SCHEMA_ID),
        ),
        (
            "integration-candidate-review.schema.json",
            schema_document::<IntegrationCandidateReview>(INTEGRATION_CANDIDATE_REVIEW_SCHEMA_ID),
        ),
        (
            "project.schema.json",
            management_schema_document_version::<Project>("star.project", 2),
        ),
        (
            "project-v1.schema.json",
            management_schema_document::<ProjectV1>("star.project"),
        ),
        (
            "project-checkout.schema.json",
            management_schema_document::<ProjectCheckout>("star.project-checkout"),
        ),
        (
            "project-catalog-snapshot.schema.json",
            management_schema_document::<ProjectCatalogSnapshot>("star.project-catalog-snapshot"),
        ),
        (
            "code-index-snapshot.schema.json",
            management_schema_document::<CodeIndexSnapshot>("star.code-index-snapshot"),
        ),
        (
            "project-v1-to-v2-migration-plan.schema.json",
            management_schema_document::<ProjectV1ToV2MigrationPlan>(
                "star.management.project-v1-to-v2-migration-plan",
            ),
        ),
        (
            "project-v1-to-v2-migration-result.schema.json",
            management_schema_document::<ProjectV1ToV2MigrationResult>(
                "star.management.project-v1-to-v2-migration-result",
            ),
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
