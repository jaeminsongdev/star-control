//! Controller-owned Codex App Server execution lifecycle.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Deserialize;
use star_adapter_codex::app_server::{
    CodexAppServerError, CodexAppServerProcess, probe_codex_version_with_environment,
};
use star_application::{ApplicationError, ManagementApplicationService};
use star_contracts::{
    ApprovalId, CapabilitySnapshotId, CodexExecutionId, OperationId, ProjectId, RequestId,
    RouteDecisionId, Sha256Hash, canonical_sha256,
    codex_execution::{
        CODEX_EXECUTION_CONTRACT_VERSION, CODEX_EXECUTION_RECORD_SCHEMA_ID,
        CodexExecutionOperationV1, CodexExecutionRecordV1, CodexExecutionStateV1, CodexThreadRefV1,
        CodexTurnRefV1,
    },
    context_pack::{CONTEXT_PACK_SCHEMA_ID, ContextPackV1},
    development_effect::{
        DEVELOPMENT_EFFECT_RECEIPT_V1_SCHEMA_ID, DevelopmentEffectKind, DevelopmentEffectReceiptV1,
        DevelopmentEffectState,
    },
    evidence::{AuthoritativeGateState, Completeness, DocumentRef, GateScope, ValidationOutcome},
    evidence_v2::{
        EVIDENCE_V2_SCHEMA_VERSION, EvidenceFreshnessV2, GATE_DECISION_V2_SCHEMA_ID,
        GateDecisionV2, GatePhaseV2,
    },
    fixed_mcp::ApprovalDecision,
    management::ProjectPathRef,
    permission_plan::{
        PERMISSION_PLAN_SCHEMA_ID, PathPermissionKindV1, PermissionDecisionV1, PermissionPlanV1,
    },
    routing::{
        CAPABILITY_SNAPSHOT_SCHEMA_ID, CapabilitySnapshotV1, CodexPermissionCapabilitiesV1,
        ExecutionModeV1, ROUTE_DECISION_SCHEMA_ID, ROUTING_CONTRACT_VERSION, RouteDecisionV1,
    },
    stage::StageSpecV1,
};
use star_ports::DevelopmentRecord;

use star_controller::approval_store::{ApprovalRecord, ApprovalScope, ApprovalStore};
use star_controller::goal_store::with_default_goal_store;
use star_controller::operation_store::{OperationCreate, OperationStore};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const STATUS_POLL: Duration = Duration::from_millis(25);
const MAX_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CodexTaskLaunchInput {
    project_id: ProjectId,
    route_decision_id: RouteDecisionId,
    capability_snapshot_id: CapabilitySnapshotId,
    stage: StageSpecV1,
    context_pack_ref: DocumentRef,
    gate_decision_ref: DocumentRef,
    codex_executable: String,
    cwd: String,
    instruction: String,
    #[serde(default)]
    parent_execution_id: Option<CodexExecutionId>,
    #[serde(default)]
    approval_id: Option<ApprovalId>,
}

struct ActiveCodexExecution {
    process: CodexAppServerProcess,
    record: CodexExecutionRecordV1,
}

type ActiveCodexExecutionHandle = Arc<Mutex<ActiveCodexExecution>>;

static ACTIVE_EXECUTIONS: OnceLock<Mutex<BTreeMap<String, ActiveCodexExecutionHandle>>> =
    OnceLock::new();

fn active_executions() -> &'static Mutex<BTreeMap<String, ActiveCodexExecutionHandle>> {
    ACTIVE_EXECUTIONS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn active_execution(
    execution_id: &CodexExecutionId,
) -> Result<Option<ActiveCodexExecutionHandle>, ApplicationError> {
    Ok(active_executions()
        .lock()
        .map_err(|_| apply("CODEX_OPERATION_LOST"))?
        .get(execution_id.as_str())
        .cloned())
}

fn remove_active_execution(
    execution_id: &CodexExecutionId,
    expected: &ActiveCodexExecutionHandle,
) -> Result<(), ApplicationError> {
    let mut executions = active_executions()
        .lock()
        .map_err(|_| apply("CODEX_OPERATION_LOST"))?;
    if executions
        .get(execution_id.as_str())
        .is_some_and(|active| Arc::ptr_eq(active, expected))
    {
        executions.remove(execution_id.as_str());
    }
    Ok(())
}

fn apply(code: &str) -> ApplicationError {
    ApplicationError::Apply(code.to_owned())
}

fn map_app_server(error: CodexAppServerError) -> ApplicationError {
    match error {
        CodexAppServerError::Path
        | CodexAppServerError::Protocol
        | CodexAppServerError::UnsupportedServerRequest
        | CodexAppServerError::UnsupportedReasoningEffort
        | CodexAppServerError::Evidence => apply("CODEX_PROTOCOL_MISMATCH"),
        CodexAppServerError::UnsupportedExecutionMode => apply("ROUTE_MODE_UNAVAILABLE"),
        CodexAppServerError::Io
        | CodexAppServerError::Timeout
        | CodexAppServerError::Remote(_)
        | CodexAppServerError::Exited => apply("CODEX_NOT_READY"),
    }
}

pub(crate) fn read_executable_hash(path: &Path) -> Result<Sha256Hash, ApplicationError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| apply("CODEX_NOT_READY"))?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_EXECUTABLE_BYTES
    {
        return Err(apply("CODEX_PROTOCOL_MISMATCH"));
    }
    let bytes = std::fs::read(path).map_err(|_| apply("CODEX_NOT_READY"))?;
    Ok(Sha256Hash::digest(&bytes))
}

fn capability_from_record(
    record: &DevelopmentRecord,
    project_id: &ProjectId,
) -> Result<CapabilitySnapshotV1, ApplicationError> {
    if record.schema_version != 1
        || record.record_kind != "capability_snapshot"
        || record.revision != 1
        || record.project_id.as_ref() != Some(project_id)
        || record.state != "current"
        || record.document_schema_id != CAPABILITY_SNAPSHOT_SCHEMA_ID
        || record.document_schema_version != ROUTING_CONTRACT_VERSION
        || canonical_sha256(&record.document).map_err(|_| apply("CODEX_PROTOCOL_MISMATCH"))?
            != record.document_fingerprint
        || DateTime::parse_from_rfc3339(&record.created_at).is_err()
    {
        return Err(apply("CODEX_PROTOCOL_MISMATCH"));
    }
    let snapshot: CapabilitySnapshotV1 = serde_json::from_value(record.document.clone())
        .map_err(|_| apply("CODEX_PROTOCOL_MISMATCH"))?;
    if snapshot.capability_snapshot_id.as_str() != record.record_id || snapshot.verify().is_err() {
        return Err(apply("CODEX_PROTOCOL_MISMATCH"));
    }
    Ok(snapshot)
}

fn latest_capability_snapshot_id(
    candidates: impl IntoIterator<Item = (CapabilitySnapshotId, DateTime<Utc>)>,
) -> Result<Option<CapabilitySnapshotId>, ApplicationError> {
    let mut latest: Option<(CapabilitySnapshotId, DateTime<Utc>)> = None;
    for (candidate_id, captured_at) in candidates {
        match &latest {
            None => latest = Some((candidate_id, captured_at)),
            Some((_, latest_captured_at)) if captured_at > *latest_captured_at => {
                latest = Some((candidate_id, captured_at));
            }
            Some((latest_id, latest_captured_at))
                if captured_at == *latest_captured_at && candidate_id != *latest_id =>
            {
                // Two independently identified snapshots cannot both be the unique latest
                // observation. Fail closed instead of selecting by a random identifier.
                return Err(apply("CODEX_PROTOCOL_MISMATCH"));
            }
            _ => {}
        }
    }
    Ok(latest.map(|(id, _)| id))
}

pub(crate) fn load_capability(
    service: &ManagementApplicationService,
    project_id: &ProjectId,
    id: &CapabilitySnapshotId,
) -> Result<CapabilitySnapshotV1, ApplicationError> {
    let record = service
        .get_development_record("capability_snapshot", id.as_str(), None)?
        .ok_or(ApplicationError::NotFound)?;
    if record.project_id.as_ref() != Some(project_id) {
        return Err(ApplicationError::NotFound);
    }
    let snapshot = capability_from_record(&record, project_id)?;
    if snapshot.capability_snapshot_id != *id {
        return Err(apply("CODEX_PROTOCOL_MISMATCH"));
    }
    Ok(snapshot)
}

pub(crate) fn require_latest_capability(
    service: &ManagementApplicationService,
    project_id: &ProjectId,
    expected: &CapabilitySnapshotV1,
) -> Result<(), ApplicationError> {
    let snapshots = service
        .list_development_records("capability_snapshot", Some(project_id))?
        .iter()
        .map(|record| capability_from_record(record, project_id))
        .collect::<Result<Vec<_>, _>>()?;
    let latest = latest_capability_snapshot_id(snapshots.iter().map(|snapshot| {
        (
            snapshot.capability_snapshot_id.clone(),
            snapshot.captured_at,
        )
    }))?
    .ok_or_else(|| apply("CODEX_NOT_READY"))?;
    if latest != expected.capability_snapshot_id {
        return Err(apply("CODEX_NOT_READY"));
    }
    Ok(())
}

fn load_route(
    service: &ManagementApplicationService,
    project_id: &ProjectId,
    id: &RouteDecisionId,
) -> Result<RouteDecisionV1, ApplicationError> {
    let record = service
        .get_development_record("route_decision", id.as_str(), None)?
        .ok_or(ApplicationError::NotFound)?;
    if record.project_id.as_ref() != Some(project_id) {
        return Err(ApplicationError::NotFound);
    }
    let route: RouteDecisionV1 =
        serde_json::from_value(record.document).map_err(|_| ApplicationError::Invalid)?;
    if route.route_decision_id != *id {
        return Err(ApplicationError::Invalid);
    }
    Ok(route)
}

pub(crate) fn load_execution(
    service: &ManagementApplicationService,
    project_id: &ProjectId,
    id: &CodexExecutionId,
) -> Result<CodexExecutionRecordV1, ApplicationError> {
    let record = service
        .get_development_record("codex_execution", id.as_str(), None)?
        .ok_or(ApplicationError::NotFound)?;
    if record.project_id.as_ref() != Some(project_id) {
        return Err(ApplicationError::NotFound);
    }
    let execution: CodexExecutionRecordV1 =
        serde_json::from_value(record.document).map_err(|_| ApplicationError::Invalid)?;
    if execution.codex_execution_id != *id || execution.verify().is_err() {
        return Err(apply("CODEX_PROTOCOL_MISMATCH"));
    }
    Ok(execution)
}

fn load_context_pack(
    service: &ManagementApplicationService,
    reference: &DocumentRef,
) -> Result<ContextPackV1, ApplicationError> {
    if reference.schema_id != CONTEXT_PACK_SCHEMA_ID {
        return Err(ApplicationError::Invalid);
    }
    let id = star_contracts::ContextPackId::parse(reference.document_id.clone())
        .map_err(|_| ApplicationError::Invalid)?;
    let record = service
        .get_development_record("context_pack", id.as_str(), Some(reference.revision))?
        .ok_or(ApplicationError::NotFound)?;
    let pack: ContextPackV1 =
        serde_json::from_value(record.document).map_err(|_| ApplicationError::Invalid)?;
    if pack.context_pack_id != id
        || pack.revision != reference.revision
        || pack.context_fingerprint != reference.sha256
        || pack.verify().is_err()
    {
        return Err(apply("PLANNING_OUTPUT_COHERENCE"));
    }
    service.verify_context_pack_current(&pack)?;
    Ok(pack)
}

fn load_permission_plan(
    service: &ManagementApplicationService,
    reference: &DocumentRef,
) -> Result<PermissionPlanV1, ApplicationError> {
    if reference.schema_id != PERMISSION_PLAN_SCHEMA_ID {
        return Err(ApplicationError::Invalid);
    }
    let id = star_contracts::PermissionPlanId::parse(reference.document_id.clone())
        .map_err(|_| ApplicationError::Invalid)?;
    let record = service
        .get_development_record("permission_plan", id.as_str(), Some(reference.revision))?
        .ok_or(ApplicationError::NotFound)?;
    let plan: PermissionPlanV1 =
        serde_json::from_value(record.document).map_err(|_| ApplicationError::Invalid)?;
    if plan.permission_plan_id != id
        || plan.revision != reference.revision
        || plan.plan_fingerprint != reference.sha256
        || plan.verify().is_err()
    {
        return Err(apply("PLANNING_OUTPUT_COHERENCE"));
    }
    Ok(plan)
}

fn load_current_gate(
    service: &ManagementApplicationService,
    project_id: &ProjectId,
    reference: &DocumentRef,
    route: &RouteDecisionV1,
    stage: &StageSpecV1,
    context_pack: &ContextPackV1,
    permission_plan: &PermissionPlanV1,
) -> Result<GateDecisionV2, ApplicationError> {
    if reference.schema_id != GATE_DECISION_V2_SCHEMA_ID {
        return Err(ApplicationError::Invalid);
    }
    let gate_id = star_contracts::GateId::parse(reference.document_id.clone())
        .map_err(|_| ApplicationError::Invalid)?;
    let gate = service.get_gate_decision_v2(project_id, &gate_id)?;
    let gate_reference = gate
        .reference()
        .map_err(|_| apply("VALIDATION_GATE_INPUT_INVALID"))?;
    let expected_scope = GateScope::Stage {
        goal_id: route.goal_id.clone(),
        run_id: route.run_id.clone(),
        stage_id: route.stage_id.clone(),
        revision: stage.revision,
    };
    let now = Utc::now();
    if reference.revision != gate_reference.revision
        || reference.sha256 != gate_reference.sha256
        || gate.schema_version != EVIDENCE_V2_SCHEMA_VERSION
        || gate.scope != expected_scope
        || gate.authoritative_state() != AuthoritativeGateState::Passed
        || gate.decided_at > now
        || gate
            .valid_until
            .is_some_and(|valid_until| valid_until <= now)
        || stage.validation_plan_ref.as_ref() != Some(&gate.validation_plan_ref)
    {
        return Err(apply("VALIDATION_GATE_INPUT_INVALID"));
    }
    let expected_projects = context_pack
        .project_inputs
        .iter()
        .map(|input| input.project_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut observed_projects = std::collections::BTreeSet::new();
    for result_ref in &gate.validation_result_refs {
        let validation_result_id =
            star_contracts::ids::ValidationResultId::parse(result_ref.document_id.clone())
                .map_err(|_| apply("VALIDATION_GATE_INPUT_INVALID"))?;
        let mut found = None;
        for candidate_project in &expected_projects {
            match service.get_validation_result_v2(candidate_project, &validation_result_id) {
                Ok(result) => {
                    if found.is_some() {
                        return Err(apply("VALIDATION_GATE_INPUT_INVALID"));
                    }
                    found = Some(result);
                }
                Err(ApplicationError::NotFound) => {}
                Err(error) => return Err(error),
            }
        }
        let result = found.ok_or_else(|| apply("VALIDATION_GATE_INPUT_INVALID"))?;
        if result
            .reference()
            .map_err(|_| apply("VALIDATION_GATE_INPUT_INVALID"))?
            != *result_ref
        {
            return Err(apply("VALIDATION_GATE_INPUT_INVALID"));
        }
        let project_input = context_pack
            .project_inputs
            .iter()
            .find(|input| input.project_id == result.project_id)
            .ok_or_else(|| apply("VALIDATION_GATE_INPUT_INVALID"))?;
        let binding = &result.subject_binding;
        if result.outcome != ValidationOutcome::Pass
            || result.completeness != Completeness::Complete
            || result.freshness != EvidenceFreshnessV2::Current
            || binding.freshness != EvidenceFreshnessV2::Current
            || binding.gate_phase != GatePhaseV2::DuringStage
            || binding.checkout_id != project_input.checkout_id
            || binding.project_revision_id != project_input.project_revision_id
            || binding.workspace_snapshot_id != project_input.workspace_snapshot_id
            || binding.workspace_content_fingerprint != project_input.workspace_entries_fingerprint
            || binding.task_spec_ref != context_pack.task_spec_ref
            || binding.scope_revision_ref != context_pack.scope_revision_ref
            || binding.validation_plan_ref != gate.validation_plan_ref
            || binding.effective_config_fingerprint != permission_plan.effective_config_ref.sha256
            || binding.catalog_snapshot_ref != context_pack.project_catalog_snapshot_ref
        {
            return Err(apply("VALIDATION_GATE_INPUT_INVALID"));
        }
        observed_projects.insert(result.project_id);
    }
    if observed_projects != expected_projects {
        return Err(apply("VALIDATION_GATE_INPUT_INVALID"));
    }
    Ok(gate)
}

fn publish_execution(
    service: &ManagementApplicationService,
    project_id: &ProjectId,
    record: &CodexExecutionRecordV1,
) -> Result<star_ports::DevelopmentRecord, ApplicationError> {
    record
        .verify()
        .map_err(|_| apply("CODEX_PROTOCOL_MISMATCH"))?;
    service.publish_development_document(
        "codex_execution",
        record.codex_execution_id.as_str(),
        record.revision,
        Some(project_id.clone()),
        match record.state {
            CodexExecutionStateV1::Initializing => "initializing",
            CodexExecutionStateV1::ThreadReady => "thread_ready",
            CodexExecutionStateV1::Running => "running",
            CodexExecutionStateV1::InterruptRequested => "interrupt_requested",
            CodexExecutionStateV1::Interrupted => "interrupted",
            CodexExecutionStateV1::Completed => "completed",
            CodexExecutionStateV1::Failed => "failed",
            CodexExecutionStateV1::OutcomeUnknown => "outcome_unknown",
            CodexExecutionStateV1::RecoveryRequired => "recovery_required",
        },
        CODEX_EXECUTION_RECORD_SCHEMA_ID,
        CODEX_EXECUTION_CONTRACT_VERSION,
        record,
    )
}

fn canonical_launch_scope(
    command: &str,
    input: &CodexTaskLaunchInput,
    executable: &Path,
    executable_sha256: &Sha256Hash,
    cwd: &Path,
    turn_instruction_fingerprint: &Sha256Hash,
    permission_actions: &[String],
) -> Result<serde_json::Value, ApplicationError> {
    Ok(serde_json::json!({
        "command":command,
        "project_id":input.project_id,
        "route_decision_id":input.route_decision_id,
        "capability_snapshot_id":input.capability_snapshot_id,
        "stage_id":input.stage.stage_id,
        "stage_revision":input.stage.revision,
        "stage_fingerprint":input.stage.stage_fingerprint,
        "context_pack_ref":input.context_pack_ref,
        "permission_plan_ref":input.stage.permission_plan_ref,
        "gate_decision_ref":input.gate_decision_ref,
        "codex_executable":executable.to_string_lossy(),
        "executable_sha256":executable_sha256,
        "cwd":cwd.to_string_lossy(),
        "instruction_fingerprint":turn_instruction_fingerprint,
        "parent_execution_id":input.parent_execution_id,
        "permission_actions":permission_actions,
    }))
}

fn render_context_bound_instruction(
    service: &ManagementApplicationService,
    pack: &ContextPackV1,
    instruction: &str,
) -> Result<(String, BTreeMap<ProjectId, PathBuf>), ApplicationError> {
    let mut project_roots = BTreeMap::new();
    let mut root_bindings = BTreeMap::new();
    for input in &pack.project_inputs {
        let root = service
            .development_project_root(&input.project_id)?
            .canonicalize()
            .map_err(|_| apply("PROJECT_ROOT_UNAVAILABLE"))?;
        project_roots.insert(
            input.project_id.to_string(),
            root.to_string_lossy().to_string(),
        );
        root_bindings.insert(input.project_id.clone(), root);
    }
    let rendered = serde_json::to_string(&serde_json::json!({
        "schema_id":"star.codex-context-bound-instruction",
        "schema_version":1,
        "context_pack":pack,
        "project_roots":project_roots,
        "instruction":instruction,
    }))
    .map_err(|_| ApplicationError::Invalid)?;
    // The rendered envelope, rather than only the caller suffix, is sent as the
    // App Server turn input. Keep this bound aligned with the adapter.
    if rendered.len() > 256 * 1024 {
        return Err(apply("INDEX_RESOURCE_LIMIT"));
    }
    Ok((rendered, root_bindings))
}

fn approval_current(record: &ApprovalRecord) -> bool {
    record
        .expires_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .is_some_and(|expires_at| expires_at > Utc::now())
}

fn operation_matches_approval(
    operations: &Arc<Mutex<OperationStore>>,
    approval: &ApprovalRecord,
    allowed_statuses: &[&str],
) -> Result<(), ApplicationError> {
    let operation = operations
        .lock()
        .map_err(|_| apply("OPERATION_STORE_UNAVAILABLE"))?
        .get(approval.operation_id.as_str())
        .ok_or_else(|| apply("OPERATION_STORE_UNAVAILABLE"))?;
    if operation.tool_id != approval.tool_id
        || operation.descriptor_hash != approval.descriptor_hash.to_string()
        || operation.arguments_hash != approval.arguments_hash.to_string()
        || operation.permission_actions != approval.permission_actions
        || !allowed_statuses.contains(&operation.status.as_str())
    {
        return Err(apply("POLICY_APPROVAL_STALE"));
    }
    Ok(())
}

fn operation_error(code: &str, message: &str) -> serde_json::Value {
    serde_json::json!({
        "code":code,
        "message":message,
        "retryable":false,
    })
}

fn fail_operation(
    operations: &Arc<Mutex<OperationStore>>,
    operation_id: &OperationId,
    code: &str,
    message: &str,
) {
    if let Ok(mut store) = operations.lock() {
        let _ = store.complete(operation_id.as_str(), Err(operation_error(code, message)));
    }
}

fn transition_operation(
    operations: &Arc<Mutex<OperationStore>>,
    operation_id: &OperationId,
    next: &str,
    detail: &str,
) -> Result<(), ApplicationError> {
    operations
        .lock()
        .map_err(|_| apply("OPERATION_STORE_UNAVAILABLE"))?
        .transition(operation_id.as_str(), next, detail)
        .map(|_| ())
        .map_err(|_| apply("OPERATION_STORE_UNAVAILABLE"))
}

fn mark_operation_outcome_unknown(
    operations: &Arc<Mutex<OperationStore>>,
    operation_id: &OperationId,
    detail: &str,
) {
    if let Ok(mut store) = operations.lock() {
        let status = store
            .get(operation_id.as_str())
            .map(|operation| operation.status);
        match status.as_deref() {
            Some("running" | "cancelling") => {
                let _ = store.transition(operation_id.as_str(), "outcome_unknown", detail);
            }
            Some(
                "succeeded" | "failed" | "cancelled" | "outcome_unknown" | "denied" | "expired",
            ) => {}
            Some(_) => {
                let _ = store.complete(
                    operation_id.as_str(),
                    Err(operation_error(
                        "TOOL_OUTCOME_UNKNOWN",
                        "The Codex operation outcome could not be reconciled.",
                    )),
                );
            }
            None => {}
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "approval identity fields stay explicit at the durable effect boundary"
)]
fn resolve_launch_approval(
    approvals: &Arc<Mutex<ApprovalStore>>,
    operations: &Arc<Mutex<OperationStore>>,
    command: &str,
    input: &CodexTaskLaunchInput,
    route: &RouteDecisionV1,
    actor: &serde_json::Value,
    correlation_id: &str,
    arguments: &serde_json::Value,
    arguments_hash: &Sha256Hash,
    descriptor_hash: &Sha256Hash,
    permission_actions: &[String],
) -> Result<Result<ApprovalRecord, serde_json::Value>, ApplicationError> {
    let mut store = approvals
        .lock()
        .map_err(|_| apply("DEVELOPMENT_EFFECT_APPROVAL_STORE_UNAVAILABLE"))?;
    if let Some(approval_id) = input.approval_id.as_ref() {
        let approval = store
            .get(approval_id)
            .ok_or_else(|| apply("POLICY_APPROVAL_STALE"))?;
        if approval.tool_id != command
            || approval.arguments_hash != *arguments_hash
            || approval.descriptor_hash != *descriptor_hash
            || approval.expected_revision != Some(route.revision)
            || approval.permission_actions != permission_actions
            || approval.decision != Some(ApprovalDecision::Approve)
            || !approval_current(&approval)
        {
            return Err(apply("POLICY_APPROVAL_STALE"));
        }
        operation_matches_approval(
            operations,
            &approval,
            &[
                "approval_wait",
                "queued",
                "starting",
                "running",
                "cancelling",
                "succeeded",
                "failed",
                "cancelled",
                "outcome_unknown",
            ],
        )?;
        return Ok(Ok(approval));
    }
    let approval = if let Some(existing) =
        store.find_unresolved_exact(command, arguments_hash, Some(route.revision))
    {
        operation_matches_approval(operations, &existing, &["approval_wait"])?;
        existing
    } else {
        let invocation_hash = canonical_sha256(&serde_json::json!({
            "command":command,
            "descriptor_hash":descriptor_hash,
            "arguments_hash":arguments_hash,
            "route_revision":route.revision,
        }))
        .map_err(|_| ApplicationError::Invalid)?;
        let operation = {
            let mut operation_store = operations
                .lock()
                .map_err(|_| apply("OPERATION_STORE_UNAVAILABLE"))?;
            let operation = operation_store
                .create(OperationCreate {
                    command: command.to_owned(),
                    correlation_id: correlation_id.to_owned(),
                    tool_id: command.to_owned(),
                    descriptor_hash: descriptor_hash.to_string(),
                    arguments_hash: arguments_hash.to_string(),
                    permission_actions: permission_actions.to_vec(),
                    goal_id: Some(route.goal_id.to_string()),
                    run_id: Some(route.run_id.to_string()),
                    stage_id: Some(route.stage_id.to_string()),
                    output_provenance: Some(serde_json::json!({
                        "kind":"codex_app_server_turn",
                        "capability_snapshot_id":input.capability_snapshot_id,
                        "route_decision_id":input.route_decision_id,
                    })),
                    cancellable: true,
                    idempotency_key: None,
                    invocation_hash: invocation_hash.to_string(),
                })
                .map_err(|_| apply("OPERATION_STORE_UNAVAILABLE"))?;
            operation_store
                .transition(operation.operation_id.as_str(), "resolving", "policy_check")
                .and_then(|_| {
                    operation_store.transition(
                        operation.operation_id.as_str(),
                        "approval_wait",
                        "codex_execution_approval",
                    )
                })
                .map_err(|_| apply("OPERATION_STORE_UNAVAILABLE"))?
        };
        match store.create(ApprovalScope {
            operation_id: operation.operation_id.clone(),
            tool_id: command.to_owned(),
            descriptor_hash: descriptor_hash.clone(),
            arguments_hash: arguments_hash.clone(),
            permission_actions: permission_actions.to_vec(),
            paid_limit: serde_json::json!({
                "state":"unknown",
                "approval_required":true,
            }),
            target_refs: vec![serde_json::json!({
                "kind":"codex_turn",
                "project_id":input.project_id,
                "route_decision_id":input.route_decision_id,
                "stage_id":input.stage.stage_id,
                "instruction_fingerprint":arguments["instruction_fingerprint"],
            })],
            expected_revision: Some(route.revision),
            arguments: arguments.clone(),
            actor: actor.clone(),
            runtime_scope: serde_json::json!({
                "kind":"codex_app_server_turn",
                "command":command,
            }),
        }) {
            Ok(approval) => approval,
            Err(_) => {
                fail_operation(
                    operations,
                    &operation.operation_id,
                    "DEVELOPMENT_EFFECT_APPROVAL_STORE_UNAVAILABLE",
                    "The Codex approval scope could not be persisted.",
                );
                return Err(apply("DEVELOPMENT_EFFECT_APPROVAL_STORE_UNAVAILABLE"));
            }
        }
    };
    Ok(Err(serde_json::json!({
        "status":"approval_required",
        "approval_id":approval.approval_id,
        "operation_id":approval.operation_id,
        "scope_hash":approval.scope_hash,
        "expires_at":approval.expires_at,
        "next_action":"approval.resolve",
    })))
}

fn launch_operation(command: &str) -> Result<CodexExecutionOperationV1, ApplicationError> {
    match command {
        "codex.task.start" => Ok(CodexExecutionOperationV1::Start),
        "codex.task.resume" => Ok(CodexExecutionOperationV1::Resume),
        "codex.task.fork" => Ok(CodexExecutionOperationV1::Fork),
        _ => Err(ApplicationError::Invalid),
    }
}

fn capability_supports_launch(
    snapshot: &CapabilitySnapshotV1,
    operation: CodexExecutionOperationV1,
) -> bool {
    launch_capability_is_supported(
        &snapshot.operations,
        &snapshot.permission_capabilities,
        operation,
    )
}

fn launch_capability_is_supported(
    operations: &BTreeMap<String, bool>,
    permissions: &CodexPermissionCapabilitiesV1,
    operation: CodexExecutionOperationV1,
) -> bool {
    let operation_supported = match operation {
        CodexExecutionOperationV1::Start => operations.get("thread_start") == Some(&true),
        CodexExecutionOperationV1::Resume => operations.get("thread_resume") == Some(&true),
        CodexExecutionOperationV1::Fork => operations.get("thread_fork") == Some(&true),
        CodexExecutionOperationV1::Interrupt | CodexExecutionOperationV1::Status => false,
    };
    operation_supported
        && operations.get("turn_start") == Some(&true)
        && permissions.approval_policy_configurable
        && permissions.sandbox_mode_configurable
}

pub fn launch(
    service: &ManagementApplicationService,
    approvals: Option<&Arc<Mutex<ApprovalStore>>>,
    operations: Option<&Arc<Mutex<OperationStore>>>,
    command: &str,
    payload: &serde_json::Value,
    actor: &serde_json::Value,
    correlation_id: &str,
) -> Result<serde_json::Value, ApplicationError> {
    let input: CodexTaskLaunchInput =
        serde_json::from_value(payload.clone()).map_err(|_| ApplicationError::Invalid)?;
    let operation = launch_operation(command)?;
    if (operation == CodexExecutionOperationV1::Start && input.parent_execution_id.is_some())
        || (operation != CodexExecutionOperationV1::Start && input.parent_execution_id.is_none())
        || input.context_pack_ref.schema_id != CONTEXT_PACK_SCHEMA_ID
        || input.gate_decision_ref.schema_id != GATE_DECISION_V2_SCHEMA_ID
        || input.instruction.trim().is_empty()
        || input.instruction.len() > 256 * 1024
    {
        return Err(ApplicationError::Invalid);
    }
    input
        .stage
        .verify()
        .map_err(|_| apply("PLANNING_OUTPUT_COHERENCE"))?;
    service.verify_stage_profile_current(&input.stage)?;
    let snapshot = load_capability(service, &input.project_id, &input.capability_snapshot_id)?;
    require_latest_capability(service, &input.project_id, &snapshot)?;
    if !capability_supports_launch(&snapshot, operation) {
        return Err(apply("ROUTE_MODE_UNAVAILABLE"));
    }
    let route = load_route(service, &input.project_id, &input.route_decision_id)?;
    let stage_graph_id =
        star_contracts::StageGraphId::parse(route.stage_graph_ref.document_id.clone())
            .map_err(|_| apply("PLANNING_OUTPUT_COHERENCE"))?;
    let stage_graph = with_default_goal_store(|store| store.stage_graph(stage_graph_id.as_str()))
        .map_err(|_| apply("PLANNING_OUTPUT_COHERENCE"))?;
    if stage_graph.verify().is_err()
        || route.stage_graph_ref.schema_id != "star.stage-graph"
        || route.stage_graph_ref.revision != stage_graph.plan_revision
        || route.stage_graph_ref.sha256 != stage_graph.graph_fingerprint
        || stage_graph
            .stages
            .iter()
            .find(|stage| stage.stage_id == input.stage.stage_id)
            != Some(&input.stage)
        || input.stage.state != star_contracts::stage::StageStateV1::Ready
    {
        return Err(apply("PLANNING_OUTPUT_COHERENCE"));
    }
    route
        .verify_against(&input.stage, &snapshot)
        .map_err(|_| apply("ROUTE_MODE_UNAVAILABLE"))?;
    if !snapshot.is_current_at(Utc::now()) || route.execution_mode != ExecutionModeV1::Single {
        return Err(apply("ROUTE_MODE_UNAVAILABLE"));
    }
    let context_pack = load_context_pack(service, &input.context_pack_ref)?;
    if context_pack.stage_id != input.stage.stage_id
        || context_pack.stage_revision != input.stage.revision
        || context_pack.stage_graph_ref != route.stage_graph_ref
        || !context_pack
            .project_inputs
            .iter()
            .any(|project| project.project_id == input.project_id)
        || input.stage.task_spec_ref.as_ref() != Some(&context_pack.task_spec_ref)
        || input.stage.scope_revision_ref.as_ref() != Some(&context_pack.scope_revision_ref)
    {
        return Err(apply("PLANNING_OUTPUT_COHERENCE"));
    }
    let (turn_instruction, root_bindings) =
        render_context_bound_instruction(service, &context_pack, &input.instruction)?;
    let turn_instruction_fingerprint = Sha256Hash::digest(turn_instruction.as_bytes());
    let project_root = service.development_project_root(&input.project_id)?;
    let canonical_project = project_root
        .canonicalize()
        .map_err(|_| apply("PROJECT_ROOT_UNAVAILABLE"))?;
    let cwd = PathBuf::from(&input.cwd)
        .canonicalize()
        .map_err(|_| ApplicationError::Invalid)?;
    if !cwd.starts_with(&canonical_project) {
        return Err(apply("PROJECT_ROOT_NOT_ALLOWLISTED"));
    }
    let executable = PathBuf::from(&input.codex_executable)
        .canonicalize()
        .map_err(|_| apply("CODEX_NOT_READY"))?;
    let executable_sha256 = read_executable_hash(&executable)?;
    let permission_plan = load_permission_plan(service, &route.permission_plan_ref)?;
    let effective_config = service.effective_config()?;
    let relative_cwd = cwd
        .strip_prefix(&canonical_project)
        .map_err(|_| ApplicationError::Invalid)?;
    let relative_cwd = if relative_cwd.as_os_str().is_empty() {
        None
    } else {
        let value = relative_cwd
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        Some(ProjectPathRef::parse(value).map_err(|_| ApplicationError::Invalid)?)
    };
    let stage_scope_hash = input
        .stage
        .permission_scope_hash()
        .map_err(|_| apply("PLANNING_OUTPUT_COHERENCE"))?;
    if !permission_plan.is_current_at(Utc::now())
        || route.config_fingerprint != effective_config.config_fingerprint
        || permission_plan.goal_id != route.goal_id
        || permission_plan.run_id != route.run_id
        || permission_plan.stage_id != input.stage.stage_id
        || permission_plan.stage_revision != input.stage.revision
        || permission_plan.stage_scope_hash != stage_scope_hash
        || permission_plan.effective_config_ref.sha256 != effective_config.config_fingerprint
        || permission_plan.policy_profile_ref.catalog_id != effective_config.policy_profile_id
        || permission_plan.decision("external.ai.execute") != PermissionDecisionV1::Prompt
        || permission_plan.decision("paid_action") != PermissionDecisionV1::Prompt
        || !permission_plan.external_constraints.codex_approval_required
        || permission_plan.external_constraints.codex_approval_policy != "never"
        || permission_plan.external_constraints.administrator_required
        || (input.stage.stage_mode == star_contracts::stage::StageModeV1::Execute
            && permission_plan.external_constraints.codex_sandbox_mode != "workspace-write")
        || (input.stage.stage_mode != star_contracts::stage::StageModeV1::Execute
            && permission_plan.external_constraints.codex_sandbox_mode != "read-only")
        || !permission_plan.allows_process(
            &executable_sha256,
            &input.project_id,
            relative_cwd.as_ref(),
            &["app-server", "--listen", "stdio://"],
            true,
        )
        || !permission_plan.allows_process(
            &executable_sha256,
            &input.project_id,
            relative_cwd.as_ref(),
            &["--version"],
            false,
        )
        || !permission_plan.network_rules.iter().any(|rule| {
            rule.target == "codex-provider"
                && rule.operation == "execute"
                && rule.decision != PermissionDecisionV1::Deny
        })
    {
        return Err(apply("POLICY_DENIED"));
    }
    let mut permission_actions = vec![
        "external.ai.execute".to_owned(),
        "local_read".to_owned(),
        "network_read".to_owned(),
        "paid_action".to_owned(),
        "plan_execute".to_owned(),
        "process_run".to_owned(),
        // App Server receives a whole project root. Even if the current Context
        // Pack contains no secret item, fine-grained secret exclusion is not an
        // enforceable sandbox boundary and must be represented in the approval.
        "secret_access".to_owned(),
    ];
    if input.stage.stage_mode == star_contracts::stage::StageModeV1::Execute {
        permission_actions.push("local_write".to_owned());
    }
    permission_actions.sort();
    permission_actions.dedup();
    if permission_actions
        .iter()
        .any(|action| permission_plan.decision(action) == PermissionDecisionV1::Deny)
    {
        return Err(apply("POLICY_DENIED"));
    }
    // App Server sandbox roots are project-wide read grants. A path-scoped read
    // allow or nested deny cannot be represented faithfully, so fail closed.
    for project_id in root_bindings.keys() {
        let project_wide_read = permission_plan.path_rules.iter().any(|rule| {
            &rule.project_id == project_id
                && rule.path_prefix.is_none()
                && rule.kind == PathPermissionKindV1::AllowRead
        });
        let unenforceable_deny = permission_plan.path_rules.iter().any(|rule| {
            &rule.project_id == project_id
                && matches!(
                    rule.kind,
                    PathPermissionKindV1::DenyRead | PathPermissionKindV1::DenyExternalTransfer
                )
        });
        let unenforceable_write_deny = input.stage.stage_mode
            == star_contracts::stage::StageModeV1::Execute
            && permission_plan.path_rules.iter().any(|rule| {
                &rule.project_id == project_id && rule.kind == PathPermissionKindV1::DenyWrite
            });
        let project_write = permission_plan.path_rules.iter().any(|rule| {
            &rule.project_id == project_id && rule.kind == PathPermissionKindV1::AllowWrite
        });
        if !project_wide_read
            || unenforceable_deny
            || unenforceable_write_deny
            || (input.stage.stage_mode == star_contracts::stage::StageModeV1::Execute
                && !project_write)
        {
            return Err(apply("POLICY_DENIED"));
        }
    }
    let mut runtime_workspace_roots = root_bindings.values().cloned().collect::<Vec<_>>();
    runtime_workspace_roots.sort();
    runtime_workspace_roots.dedup();
    let mut writable_roots = Vec::new();
    if input.stage.stage_mode == star_contracts::stage::StageModeV1::Execute {
        for rule in permission_plan
            .path_rules
            .iter()
            .filter(|rule| rule.kind == PathPermissionKindV1::AllowWrite)
        {
            let root = root_bindings
                .get(&rule.project_id)
                .ok_or_else(|| apply("POLICY_DENIED"))?;
            let writable_candidate = rule
                .path_prefix
                .as_ref()
                .map(|prefix| {
                    prefix
                        .as_str()
                        .split('/')
                        .fold(root.clone(), |path, segment| path.join(segment))
                })
                .unwrap_or_else(|| root.clone());
            let writable = writable_candidate
                .canonicalize()
                .map_err(|_| apply("POLICY_DENIED"))?;
            if !writable.starts_with(root)
                || !writable
                    .metadata()
                    .map(|metadata| metadata.is_dir())
                    .unwrap_or(false)
            {
                return Err(apply("POLICY_DENIED"));
            }
            writable_roots.push(writable);
        }
        writable_roots.sort();
        writable_roots.dedup();
        if writable_roots.is_empty() {
            return Err(apply("POLICY_DENIED"));
        }
    }
    let allowed_environment_names = permission_plan.allowed_environment_names();
    if allowed_environment_names.is_empty() {
        return Err(apply("POLICY_DENIED"));
    }
    let _gate = load_current_gate(
        service,
        &input.project_id,
        &input.gate_decision_ref,
        &route,
        &input.stage,
        &context_pack,
        &permission_plan,
    )?;
    let observed_version =
        probe_codex_version_with_environment(&executable, &allowed_environment_names)
            .map_err(map_app_server)?;
    if snapshot.codex_version.as_deref() != Some(observed_version.as_str()) {
        return Err(apply("CODEX_PROTOCOL_MISMATCH"));
    }
    let scope = canonical_launch_scope(
        command,
        &input,
        &executable,
        &executable_sha256,
        &cwd,
        &turn_instruction_fingerprint,
        &permission_actions,
    )?;
    let arguments_hash = canonical_sha256(&scope).map_err(|_| ApplicationError::Invalid)?;
    let descriptor_hash = Sha256Hash::digest(format!("{command}|app-server-v2|1").as_bytes());
    let approvals =
        approvals.ok_or_else(|| apply("DEVELOPMENT_EFFECT_APPROVAL_STORE_UNAVAILABLE"))?;
    let operations = operations.ok_or_else(|| apply("OPERATION_STORE_UNAVAILABLE"))?;
    let parent = input
        .parent_execution_id
        .as_ref()
        .map(|id| load_execution(service, &input.project_id, id))
        .transpose()?;
    let parent_thread_id = parent
        .as_ref()
        .and_then(|record| record.thread_ref.as_ref())
        .map(|thread| thread.thread_id.clone());
    if parent.as_ref().is_some_and(|parent| {
        parent.goal_id != route.goal_id
            || parent.run_id != route.run_id
            || parent.stage_id != route.stage_id
            || parent.stage_revision != route.stage_revision
            || !parent.state.is_terminal()
    }) || (operation != CodexExecutionOperationV1::Start && parent_thread_id.is_none())
    {
        return Err(apply("CODEX_OPERATION_LOST"));
    }
    let approval = match resolve_launch_approval(
        approvals,
        operations,
        command,
        &input,
        &route,
        actor,
        correlation_id,
        &scope,
        &arguments_hash,
        &descriptor_hash,
        &permission_actions,
    )? {
        Ok(approval) => approval,
        Err(required) => return Ok(required),
    };

    let execution_id = CodexExecutionId::from_stable_bytes(
        format!("{}:codex-execution", approval.operation_id).as_bytes(),
    );
    if service
        .get_development_record("codex_execution", execution_id.as_str(), None)?
        .is_some()
    {
        let existing = load_execution(service, &input.project_id, &execution_id)?;
        if existing.parent_execution_id != input.parent_execution_id
            || existing.route_decision_ref.document_id != route.route_decision_id.as_str()
            || existing.route_decision_ref.sha256 != route.decision_fingerprint
            || existing.context_pack_ref != input.context_pack_ref
            || existing.permission_plan_ref != route.permission_plan_ref
            || existing.gate_decision_ref != input.gate_decision_ref
            || existing.approval_id != approval.approval_id
            || existing.controller_operation_id != approval.operation_id
            || existing.tool_id != command
            || existing.descriptor_hash != descriptor_hash
            || existing.arguments_hash != arguments_hash
            || existing.executable_sha256 != executable_sha256
            || existing.operation != operation
            || existing.instruction_fingerprint != turn_instruction_fingerprint
        {
            return Err(apply("STATE_MISMATCH"));
        }
        if existing.state.is_terminal() {
            reconcile_terminal_operation(operations, &existing)?;
            return Ok(serde_json::json!({"status":"terminal","record":existing}));
        }
        if active_execution(&execution_id)?.is_some() {
            return Ok(serde_json::json!({"status":"running","record":existing}));
        }
        return finalize_with_effect_started(
            service,
            operations,
            &input.project_id,
            existing.clone(),
            CodexExecutionStateV1::OutcomeUnknown,
            "launch_retry_requires_reconciliation",
            source_effect_may_have_started(&existing),
        );
    }

    let started_at = Utc::now();
    let mut record = CodexExecutionRecordV1 {
        schema_id: CODEX_EXECUTION_RECORD_SCHEMA_ID.to_owned(),
        schema_version: CODEX_EXECUTION_CONTRACT_VERSION,
        codex_execution_id: execution_id,
        revision: 1,
        parent_execution_id: input.parent_execution_id.clone(),
        goal_id: route.goal_id.clone(),
        run_id: route.run_id.clone(),
        stage_id: route.stage_id.clone(),
        stage_revision: route.stage_revision,
        route_decision_id: route.route_decision_id.clone(),
        route_decision_ref: DocumentRef {
            schema_id: ROUTE_DECISION_SCHEMA_ID.to_owned(),
            document_id: route.route_decision_id.to_string(),
            revision: route.revision,
            sha256: route.decision_fingerprint.clone(),
        },
        context_pack_ref: input.context_pack_ref.clone(),
        permission_plan_ref: route.permission_plan_ref.clone(),
        gate_decision_ref: input.gate_decision_ref.clone(),
        approval_id: approval.approval_id.clone(),
        controller_operation_id: approval.operation_id.clone(),
        tool_id: command.to_owned(),
        descriptor_hash: descriptor_hash.clone(),
        arguments_hash: arguments_hash.clone(),
        executable_sha256: executable_sha256.clone(),
        operation,
        state: CodexExecutionStateV1::Initializing,
        thread_ref: None,
        turn_ref: None,
        instruction_fingerprint: turn_instruction_fingerprint.clone(),
        last_event_sequence: 0,
        last_event_kind: "initializing".to_owned(),
        started_at,
        updated_at: started_at,
        finished_at: None,
        result_summary: None,
        output_artifact_refs: Vec::new(),
        error_code: None,
        redacted_error: None,
        outcome_unknown: false,
        recovery_action: None,
        terminal_effect_receipt_ref: None,
        execution_fingerprint: Sha256Hash::digest(b"unsealed"),
    }
    .seal()
    .map_err(|_| {
        mark_operation_outcome_unknown(
            operations,
            &approval.operation_id,
            "codex_record_seal_outcome_unknown",
        );
        apply("CODEX_PROTOCOL_MISMATCH")
    })?;
    publish_execution(service, &input.project_id, &record)?;

    if let Err(error) = transition_operation(
        operations,
        &approval.operation_id,
        "queued",
        "codex_execution_approved",
    ) {
        finalize_with_effect_started(
            service,
            operations,
            &input.project_id,
            record,
            CodexExecutionStateV1::Failed,
            "operation_queue_failed",
            false,
        )?;
        return Err(error);
    }
    if let Err(error) = transition_operation(
        operations,
        &approval.operation_id,
        "starting",
        "codex_app_server_starting",
    ) {
        finalize_with_effect_started(
            service,
            operations,
            &input.project_id,
            record,
            CodexExecutionStateV1::Failed,
            "operation_start_failed",
            false,
        )?;
        return Err(error);
    }
    let mut process = match CodexAppServerProcess::spawn_with_environment(
        &executable,
        &allowed_environment_names,
    ) {
        Ok(process) => process,
        Err(error) => {
            let mapped = map_app_server(error);
            finalize_with_effect_started(
                service,
                operations,
                &input.project_id,
                record,
                CodexExecutionStateV1::Failed,
                "app_server_spawn_failed",
                false,
            )?;
            return Err(mapped);
        }
    };
    if let Err(error) = process.initialize(env!("CARGO_PKG_VERSION"), REQUEST_TIMEOUT) {
        let known_failure = matches!(
            error,
            CodexAppServerError::Remote(_)
                | CodexAppServerError::Protocol
                | CodexAppServerError::UnsupportedServerRequest
        );
        let mapped = map_app_server(error);
        finalize_with_effect_started(
            service,
            operations,
            &input.project_id,
            record,
            if known_failure {
                CodexExecutionStateV1::Failed
            } else {
                CodexExecutionStateV1::OutcomeUnknown
            },
            "app_server_initialize_failed",
            false,
        )?;
        return Err(mapped);
    }
    let thread_result = match operation {
        CodexExecutionOperationV1::Start => process.thread_start_with_policy(
            &route.resolved_model,
            Some(&cwd),
            Some(&permission_plan.external_constraints.codex_approval_policy),
            Some(&permission_plan.external_constraints.codex_sandbox_mode),
            &runtime_workspace_roots,
            REQUEST_TIMEOUT,
        ),
        CodexExecutionOperationV1::Resume => process.thread_resume_with_policy(
            parent_thread_id
                .as_deref()
                .ok_or_else(|| apply("CODEX_OPERATION_LOST"))?,
            Some(&permission_plan.external_constraints.codex_approval_policy),
            Some(&permission_plan.external_constraints.codex_sandbox_mode),
            &runtime_workspace_roots,
            REQUEST_TIMEOUT,
        ),
        CodexExecutionOperationV1::Fork => process.thread_fork_with_policy(
            parent_thread_id
                .as_deref()
                .ok_or_else(|| apply("CODEX_OPERATION_LOST"))?,
            Some(&permission_plan.external_constraints.codex_approval_policy),
            Some(&permission_plan.external_constraints.codex_sandbox_mode),
            &runtime_workspace_roots,
            REQUEST_TIMEOUT,
        ),
        _ => unreachable!("launch operation is start, resume, or fork"),
    };
    let thread_id = match thread_result {
        Ok(thread_id) => thread_id,
        Err(error) => {
            let known_failure = matches!(error, CodexAppServerError::Remote(_));
            let mapped = map_app_server(error);
            finalize_with_effect_started(
                service,
                operations,
                &input.project_id,
                record,
                if known_failure {
                    CodexExecutionStateV1::Failed
                } else {
                    CodexExecutionStateV1::OutcomeUnknown
                },
                "thread_request_failed",
                false,
            )?;
            return Err(mapped);
        }
    };
    let thread_ref = match (CodexThreadRefV1 {
        app_server_instance_id: RequestId::new().to_string(),
        protocol_version: snapshot.protocol_version.clone(),
        thread_id: thread_id.clone(),
        parent_thread_id: (operation == CodexExecutionOperationV1::Fork)
            .then(|| parent_thread_id.clone())
            .flatten(),
        capability_snapshot_ref: route.capability_snapshot_ref.clone(),
        thread_fingerprint: Sha256Hash::digest(b"unsealed"),
    })
    .seal()
    {
        Ok(thread_ref) => thread_ref,
        Err(_) => {
            finalize_with_effect_started(
                service,
                operations,
                &input.project_id,
                record,
                CodexExecutionStateV1::OutcomeUnknown,
                "thread_ref_invalid",
                false,
            )?;
            return Err(apply("CODEX_PROTOCOL_MISMATCH"));
        }
    };
    record.revision = record.revision.saturating_add(1);
    record.state = CodexExecutionStateV1::ThreadReady;
    record.thread_ref = Some(thread_ref);
    record.last_event_sequence = record.last_event_sequence.saturating_add(1);
    record.last_event_kind = "thread_ready".to_owned();
    record.updated_at = Utc::now();
    record = record
        .seal()
        .map_err(|_| apply("CODEX_PROTOCOL_MISMATCH"))?;
    if let Err(error) = publish_execution(service, &input.project_id, &record) {
        mark_operation_outcome_unknown(
            operations,
            &approval.operation_id,
            "thread_ready_publication_outcome_unknown",
        );
        return Err(error);
    }
    if let Err(error) = transition_operation(
        operations,
        &approval.operation_id,
        "running",
        "codex_turn_start_requested",
    ) {
        finalize_with_effect_started(
            service,
            operations,
            &input.project_id,
            record,
            CodexExecutionStateV1::Failed,
            "operation_running_transition_failed",
            false,
        )?;
        return Err(error);
    }
    record.revision = record.revision.saturating_add(1);
    record.last_event_sequence = record.last_event_sequence.saturating_add(1);
    record.last_event_kind = "turn_start_requested".to_owned();
    record.updated_at = Utc::now();
    record = record
        .seal()
        .map_err(|_| apply("CODEX_PROTOCOL_MISMATCH"))?;
    if let Err(error) = publish_execution(service, &input.project_id, &record) {
        mark_operation_outcome_unknown(
            operations,
            &approval.operation_id,
            "turn_request_publication_outcome_unknown",
        );
        return Err(error);
    }
    let turn_id = match process.turn_start_with_policy(
        &thread_id,
        &turn_instruction,
        Some(&route.resolved_model),
        Some(route.reasoning_effort),
        Some(&permission_plan.external_constraints.codex_approval_policy),
        Some(&permission_plan.external_constraints.codex_sandbox_mode),
        &runtime_workspace_roots,
        &writable_roots,
        REQUEST_TIMEOUT,
    ) {
        Ok(turn_id) => turn_id,
        Err(error) => {
            let known_failure = matches!(error, CodexAppServerError::Remote(_));
            let mapped = map_app_server(error);
            finalize_with_effect_started(
                service,
                operations,
                &input.project_id,
                record,
                if known_failure {
                    CodexExecutionStateV1::Failed
                } else {
                    CodexExecutionStateV1::OutcomeUnknown
                },
                "turn_start_failed",
                !known_failure,
            )?;
            return Err(mapped);
        }
    };
    let turn_ref = match (CodexTurnRefV1 {
        thread_id,
        turn_id,
        turn_fingerprint: Sha256Hash::digest(b"unsealed"),
    })
    .seal()
    {
        Ok(turn_ref) => turn_ref,
        Err(_) => {
            finalize_with_effect_started(
                service,
                operations,
                &input.project_id,
                record,
                CodexExecutionStateV1::OutcomeUnknown,
                "turn_ref_invalid",
                true,
            )?;
            return Err(apply("CODEX_PROTOCOL_MISMATCH"));
        }
    };
    record.revision = record.revision.saturating_add(1);
    record.state = CodexExecutionStateV1::Running;
    record.turn_ref = Some(turn_ref);
    record.last_event_sequence = record.last_event_sequence.saturating_add(1);
    record.last_event_kind = "turn_started".to_owned();
    record.updated_at = Utc::now();
    record = record.seal().map_err(|_| {
        mark_operation_outcome_unknown(
            operations,
            &approval.operation_id,
            "codex_record_seal_outcome_unknown",
        );
        apply("CODEX_PROTOCOL_MISMATCH")
    })?;
    let persisted = publish_execution(service, &input.project_id, &record).inspect_err(|_| {
        mark_operation_outcome_unknown(
            operations,
            &approval.operation_id,
            "codex_record_publish_outcome_unknown",
        );
    })?;
    let mut executions = match active_executions().lock() {
        Ok(executions) => executions,
        Err(_) => {
            return finalize(
                service,
                operations,
                &input.project_id,
                record,
                CodexExecutionStateV1::OutcomeUnknown,
                "active_registry_unavailable",
            );
        }
    };
    executions.insert(
        record.codex_execution_id.to_string(),
        Arc::new(Mutex::new(ActiveCodexExecution { process, record })),
    );
    Ok(serde_json::json!({"status":"running","record":persisted}))
}

fn terminal_receipt(
    record: &CodexExecutionRecordV1,
    project_id: &ProjectId,
    state: DevelopmentEffectState,
    event_kind: &str,
    limitation_codes: Vec<String>,
    observed_at: DateTime<Utc>,
    source_effect_started: bool,
) -> Result<DevelopmentEffectReceiptV1, ApplicationError> {
    DevelopmentEffectReceiptV1 {
        schema_id: DEVELOPMENT_EFFECT_RECEIPT_V1_SCHEMA_ID.to_owned(),
        schema_version: 1,
        receipt_id: format!("codex-effect-{}", record.codex_execution_id),
        revision: 1,
        project_id: project_id.clone(),
        effect_kind: DevelopmentEffectKind::CodexTurn,
        exact_subject_ref: format!("codex_execution:{}", record.codex_execution_id),
        exact_subject_fingerprint: record.execution_fingerprint.clone(),
        operation_id: record.controller_operation_id.clone(),
        tool_id: record.tool_id.clone(),
        descriptor_hash: record.descriptor_hash.clone(),
        arguments_hash: record.arguments_hash.clone(),
        executable_sha256: record.executable_sha256.clone(),
        approval_ref: Some(record.approval_id.to_string()),
        permission_decision_ref: Some(record.permission_plan_ref.document_id.clone()),
        gate_decision_ref: Some(record.gate_decision_ref.document_id.clone()),
        started_at: Some(
            record
                .started_at
                .to_rfc3339_opts(SecondsFormat::Millis, true),
        ),
        observed_at: observed_at.to_rfc3339_opts(SecondsFormat::Millis, true),
        state,
        source_effect_started,
        output_artifact_refs: record
            .output_artifact_refs
            .iter()
            .map(|reference| reference.sha256.clone())
            .collect(),
        result_fingerprint: Some(
            canonical_sha256(&serde_json::json!({
                "execution_id":record.codex_execution_id,
                "event_kind":event_kind,
                "observed_at":observed_at,
            }))
            .map_err(|_| ApplicationError::Invalid)?,
        ),
        limitation_codes,
        receipt_fingerprint: Sha256Hash::digest(b"unsealed"),
    }
    .seal()
    .map_err(|_| apply("DEVELOPMENT_EFFECT_RECEIPT_INVALID"))
}

fn terminal_execution_state(state: DevelopmentEffectState) -> CodexExecutionStateV1 {
    match state {
        DevelopmentEffectState::Succeeded => CodexExecutionStateV1::Completed,
        DevelopmentEffectState::Failed => CodexExecutionStateV1::Failed,
        DevelopmentEffectState::Partial => CodexExecutionStateV1::Interrupted,
        DevelopmentEffectState::OutcomeUnknown => CodexExecutionStateV1::OutcomeUnknown,
    }
}

fn canonical_terminal_event(state: CodexExecutionStateV1) -> &'static str {
    match state {
        CodexExecutionStateV1::Completed => "turn_completed",
        CodexExecutionStateV1::Failed => "turn_failed",
        CodexExecutionStateV1::Interrupted => "turn_interrupted",
        CodexExecutionStateV1::OutcomeUnknown | CodexExecutionStateV1::RecoveryRequired => {
            "outcome_unknown"
        }
        _ => "invalid_terminal_state",
    }
}

fn expected_terminal_limitations(state: DevelopmentEffectState) -> Vec<String> {
    match state {
        DevelopmentEffectState::Succeeded => Vec::new(),
        DevelopmentEffectState::Failed => vec!["codex_turn_failed".to_owned()],
        DevelopmentEffectState::Partial => vec!["codex_turn_interrupted".to_owned()],
        DevelopmentEffectState::OutcomeUnknown => {
            vec!["codex_turn_outcome_unknown".to_owned()]
        }
    }
}

fn verify_terminal_receipt(
    receipt: DevelopmentEffectReceiptV1,
    record: &CodexExecutionRecordV1,
    project_id: &ProjectId,
) -> Result<DevelopmentEffectReceiptV1, ApplicationError> {
    let expected_fingerprint = receipt.receipt_fingerprint.clone();
    let receipt = receipt
        .seal()
        .map_err(|_| apply("DEVELOPMENT_EFFECT_RECEIPT_INVALID"))?;
    let mut output_artifact_refs: Vec<_> = record
        .output_artifact_refs
        .iter()
        .map(|reference| reference.sha256.clone())
        .collect();
    output_artifact_refs.sort();
    output_artifact_refs.dedup();
    let expected_started_at = record
        .started_at
        .to_rfc3339_opts(SecondsFormat::Millis, true);
    if receipt.receipt_fingerprint != expected_fingerprint
        || receipt.receipt_id != format!("codex-effect-{}", record.codex_execution_id)
        || receipt.revision != 1
        || &receipt.project_id != project_id
        || receipt.effect_kind != DevelopmentEffectKind::CodexTurn
        || receipt.exact_subject_ref != format!("codex_execution:{}", record.codex_execution_id)
        || receipt.exact_subject_fingerprint != record.execution_fingerprint
        || receipt.operation_id != record.controller_operation_id
        || receipt.tool_id != record.tool_id
        || receipt.descriptor_hash != record.descriptor_hash
        || receipt.arguments_hash != record.arguments_hash
        || receipt.executable_sha256 != record.executable_sha256
        || receipt.approval_ref.as_deref() != Some(record.approval_id.as_str())
        || receipt.permission_decision_ref.as_deref()
            != Some(record.permission_plan_ref.document_id.as_str())
        || receipt.gate_decision_ref.as_deref()
            != Some(record.gate_decision_ref.document_id.as_str())
        || receipt.started_at.as_deref() != Some(expected_started_at.as_str())
        || receipt.output_artifact_refs != output_artifact_refs
        || receipt.result_fingerprint.is_none()
        || receipt.limitation_codes != expected_terminal_limitations(receipt.state)
    {
        return Err(apply("DEVELOPMENT_EFFECT_RECEIPT_MISMATCH"));
    }
    Ok(receipt)
}

fn load_or_publish_terminal_receipt(
    service: &ManagementApplicationService,
    record: &CodexExecutionRecordV1,
    project_id: &ProjectId,
    requested_state: DevelopmentEffectState,
    event_kind: &str,
    source_effect_started: bool,
) -> Result<(DevelopmentEffectReceiptV1, star_ports::DevelopmentRecord), ApplicationError> {
    let receipt_id = format!("codex-effect-{}", record.codex_execution_id);
    if let Some(stored) =
        service.get_development_record("development_effect_receipt", &receipt_id, Some(1))?
    {
        if stored.project_id.as_ref() != Some(project_id)
            || stored.record_kind != "development_effect_receipt"
            || stored.record_id != receipt_id
            || stored.revision != 1
            || stored.document_schema_id != DEVELOPMENT_EFFECT_RECEIPT_V1_SCHEMA_ID
            || stored.document_schema_version != 1
        {
            return Err(apply("DEVELOPMENT_EFFECT_RECEIPT_MISMATCH"));
        }
        let receipt: DevelopmentEffectReceiptV1 =
            serde_json::from_value(stored.document.clone())
                .map_err(|_| apply("DEVELOPMENT_EFFECT_RECEIPT_INVALID"))?;
        let receipt = verify_terminal_receipt(receipt, record, project_id)?;
        return Ok((receipt, stored));
    }

    let receipt = terminal_receipt(
        record,
        project_id,
        requested_state,
        event_kind,
        expected_terminal_limitations(requested_state),
        Utc::now(),
        source_effect_started,
    )?;
    let receipt = verify_terminal_receipt(receipt, record, project_id)?;
    let stored = service.publish_development_document(
        "development_effect_receipt",
        &receipt.receipt_id,
        receipt.revision,
        Some(project_id.clone()),
        match receipt.state {
            DevelopmentEffectState::Succeeded => "succeeded",
            DevelopmentEffectState::Failed => "failed",
            DevelopmentEffectState::Partial => "partial",
            DevelopmentEffectState::OutcomeUnknown => "outcome_unknown",
        },
        DEVELOPMENT_EFFECT_RECEIPT_V1_SCHEMA_ID,
        1,
        &receipt,
    )?;
    Ok((receipt, stored))
}

fn reconcile_terminal_operation(
    operations: &Arc<Mutex<OperationStore>>,
    record: &CodexExecutionRecordV1,
) -> Result<(), ApplicationError> {
    let mut store = operations
        .lock()
        .map_err(|_| apply("OPERATION_STORE_UNAVAILABLE"))?;
    if store.get(record.controller_operation_id.as_str()).is_none() {
        return Err(apply("OPERATION_STORE_UNAVAILABLE"));
    }
    let result = match record.state {
        CodexExecutionStateV1::Completed => store.complete(
            record.controller_operation_id.as_str(),
            Ok(serde_json::json!({
                "codex_execution_id":record.codex_execution_id,
                "execution_fingerprint":record.execution_fingerprint,
                "terminal_effect_receipt_ref":record.terminal_effect_receipt_ref,
            })),
        ),
        CodexExecutionStateV1::Failed => store.complete(
            record.controller_operation_id.as_str(),
            Err(operation_error(
                record.error_code.as_deref().unwrap_or("CODEX_NOT_READY"),
                "The Codex turn reached a verified failed terminal state.",
            )),
        ),
        CodexExecutionStateV1::Interrupted => store.complete(
            record.controller_operation_id.as_str(),
            Err(operation_error(
                "TOOL_CANCELLED",
                "The Codex turn was interrupted and its partial effect was retained.",
            )),
        ),
        CodexExecutionStateV1::OutcomeUnknown | CodexExecutionStateV1::RecoveryRequired => {
            let status = store
                .get(record.controller_operation_id.as_str())
                .map(|operation| operation.status)
                .ok_or_else(|| apply("OPERATION_STORE_UNAVAILABLE"))?;
            if matches!(status.as_str(), "starting" | "running" | "cancelling") {
                store.transition(
                    record.controller_operation_id.as_str(),
                    "outcome_unknown",
                    "codex_execution_outcome_unknown",
                )
            } else if matches!(
                status.as_str(),
                "succeeded" | "failed" | "cancelled" | "outcome_unknown"
            ) {
                return Ok(());
            } else {
                return Err(apply("STATE_MISMATCH"));
            }
        }
        _ => return Err(ApplicationError::Invalid),
    };
    result
        .map(|_| ())
        .map_err(|_| apply("OPERATION_STORE_UNAVAILABLE"))
}

fn finalize(
    service: &ManagementApplicationService,
    operations: &Arc<Mutex<OperationStore>>,
    project_id: &ProjectId,
    record: CodexExecutionRecordV1,
    state: CodexExecutionStateV1,
    event_kind: &str,
) -> Result<serde_json::Value, ApplicationError> {
    finalize_with_effect_started(
        service, operations, project_id, record, state, event_kind, true,
    )
}

pub(crate) fn source_effect_may_have_started(record: &CodexExecutionRecordV1) -> bool {
    record.turn_ref.is_some() || record.last_event_kind == "turn_start_requested"
}

fn finalize_with_effect_started(
    service: &ManagementApplicationService,
    operations: &Arc<Mutex<OperationStore>>,
    project_id: &ProjectId,
    mut record: CodexExecutionRecordV1,
    state: CodexExecutionStateV1,
    event_kind: &str,
    source_effect_started: bool,
) -> Result<serde_json::Value, ApplicationError> {
    let requested_effect_state = match state {
        CodexExecutionStateV1::Completed => DevelopmentEffectState::Succeeded,
        CodexExecutionStateV1::Failed => DevelopmentEffectState::Failed,
        CodexExecutionStateV1::Interrupted => DevelopmentEffectState::Partial,
        CodexExecutionStateV1::OutcomeUnknown | CodexExecutionStateV1::RecoveryRequired => {
            DevelopmentEffectState::OutcomeUnknown
        }
        _ => return Err(ApplicationError::Invalid),
    };
    // A receipt may already exist if its publication succeeded but the following
    // execution publication failed. The immutable receipt is then authoritative:
    // reusing its observation time makes the terminal execution retry identical.
    let (receipt, receipt_record) = load_or_publish_terminal_receipt(
        service,
        &record,
        project_id,
        requested_effect_state,
        event_kind,
        source_effect_started,
    )?;
    let state = terminal_execution_state(receipt.state);
    let observed_at = DateTime::parse_from_rfc3339(&receipt.observed_at)
        .map_err(|_| apply("DEVELOPMENT_EFFECT_RECEIPT_INVALID"))?
        .with_timezone(&Utc);
    record.revision = record.revision.saturating_add(1);
    record.state = state;
    record.last_event_sequence = record.last_event_sequence.saturating_add(1);
    record.last_event_kind = canonical_terminal_event(state).to_owned();
    record.updated_at = observed_at;
    record.finished_at = Some(observed_at);
    record.terminal_effect_receipt_ref = Some(DocumentRef {
        schema_id: DEVELOPMENT_EFFECT_RECEIPT_V1_SCHEMA_ID.to_owned(),
        document_id: receipt.receipt_id.clone(),
        revision: receipt.revision,
        sha256: receipt.receipt_fingerprint.clone(),
    });
    match state {
        CodexExecutionStateV1::Completed => {
            record.result_summary = Some("Codex turn completed.".to_owned());
            record.error_code = None;
            record.redacted_error = None;
            record.outcome_unknown = false;
            record.recovery_action = None;
        }
        CodexExecutionStateV1::Failed => {
            record.result_summary = None;
            record.error_code = Some("CODEX_NOT_READY".to_owned());
            record.redacted_error = Some("Codex App Server reported a failed turn.".to_owned());
            record.outcome_unknown = false;
            record.recovery_action = Some("inspect_failure_evidence".to_owned());
        }
        CodexExecutionStateV1::Interrupted => {
            record.result_summary = None;
            record.error_code = None;
            record.redacted_error = None;
            record.outcome_unknown = false;
            record.recovery_action = Some("resume_or_reconcile".to_owned());
        }
        CodexExecutionStateV1::OutcomeUnknown | CodexExecutionStateV1::RecoveryRequired => {
            record.result_summary = None;
            record.error_code = None;
            record.redacted_error = None;
            record.outcome_unknown = true;
            record.recovery_action = Some("reconcile_before_retry".to_owned());
        }
        _ => unreachable!("terminal state checked above"),
    }
    record = record
        .seal()
        .map_err(|_| apply("CODEX_PROTOCOL_MISMATCH"))?;
    let execution_record = publish_execution(service, project_id, &record)?;
    reconcile_terminal_operation(operations, &record)?;
    Ok(serde_json::json!({
        "status":match state {
            CodexExecutionStateV1::Completed => "completed",
            CodexExecutionStateV1::Failed => "failed",
            CodexExecutionStateV1::Interrupted => "interrupted",
            _ => "outcome_unknown",
        },
        "record":execution_record,
        "terminal_effect_receipt":receipt_record,
    }))
}

fn payload_ids(
    payload: &serde_json::Value,
) -> Result<(ProjectId, CodexExecutionId), ApplicationError> {
    let project_id = payload
        .get("project_id")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| ProjectId::parse(value.to_owned()).ok())
        .ok_or(ApplicationError::Invalid)?;
    let execution_id = payload
        .get("codex_execution_id")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| CodexExecutionId::parse(value.to_owned()).ok())
        .ok_or(ApplicationError::Invalid)?;
    Ok((project_id, execution_id))
}

pub fn status(
    service: &ManagementApplicationService,
    operations: Option<&Arc<Mutex<OperationStore>>>,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, ApplicationError> {
    let operations = operations.ok_or_else(|| apply("OPERATION_STORE_UNAVAILABLE"))?;
    let (project_id, execution_id) = payload_ids(payload)?;
    let persisted = load_execution(service, &project_id, &execution_id)?;
    if persisted.state.is_terminal() {
        if let Some(active) = active_execution(&execution_id)? {
            remove_active_execution(&execution_id, &active)?;
        }
        reconcile_terminal_operation(operations, &persisted)?;
        return Ok(serde_json::json!({"status":"terminal","record":persisted}));
    }
    let Some(active_handle) = active_execution(&execution_id)? else {
        let source_effect_started = source_effect_may_have_started(&persisted);
        return finalize_with_effect_started(
            service,
            operations,
            &project_id,
            persisted,
            CodexExecutionStateV1::OutcomeUnknown,
            "controller_restarted_outcome_unknown",
            source_effect_started,
        );
    };
    let mut active = match active_handle.lock() {
        Ok(active) => active,
        Err(_) => {
            remove_active_execution(&execution_id, &active_handle)?;
            return finalize(
                service,
                operations,
                &project_id,
                persisted,
                CodexExecutionStateV1::OutcomeUnknown,
                "active_execution_lock_outcome_unknown",
            );
        }
    };
    // A concurrent observer may have waited on this per-execution lock while
    // another observer persisted the terminal revision. Re-read after acquiring
    // the lock so stale pre-lock state is never reported as running.
    let persisted = load_execution(service, &project_id, &execution_id)?;
    if persisted.state.is_terminal() {
        remove_active_execution(&execution_id, &active_handle)?;
        reconcile_terminal_operation(operations, &persisted)?;
        return Ok(serde_json::json!({"status":"terminal","record":persisted}));
    }
    if active.record.execution_fingerprint != persisted.execution_fingerprint {
        remove_active_execution(&execution_id, &active_handle)?;
        return finalize(
            service,
            operations,
            &project_id,
            persisted,
            CodexExecutionStateV1::OutcomeUnknown,
            "active_registry_state_mismatch",
        );
    }
    let notification = match active.process.next_notification(STATUS_POLL) {
        Ok(notification) => notification,
        Err(CodexAppServerError::Timeout) => {
            return Ok(serde_json::json!({"status":"running","record":persisted}));
        }
        Err(_) => {
            let record = active.record.clone();
            remove_active_execution(&execution_id, &active_handle)?;
            return finalize(
                service,
                operations,
                &project_id,
                record,
                CodexExecutionStateV1::OutcomeUnknown,
                "app_server_lost_outcome_unknown",
            );
        }
    };
    let Some(method) = notification
        .get("method")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
    else {
        let record = active.record.clone();
        remove_active_execution(&execution_id, &active_handle)?;
        return finalize(
            service,
            operations,
            &project_id,
            record,
            CodexExecutionStateV1::OutcomeUnknown,
            "protocol_notification_outcome_unknown",
        );
    };
    if method == "turn/completed" {
        let status = notification
            .pointer("/params/turn/status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown");
        let state = match status {
            "completed" => CodexExecutionStateV1::Completed,
            "interrupted" | "cancelled" => CodexExecutionStateV1::Interrupted,
            "failed" => CodexExecutionStateV1::Failed,
            _ => CodexExecutionStateV1::OutcomeUnknown,
        };
        let record = active.record.clone();
        remove_active_execution(&execution_id, &active_handle)?;
        return finalize(
            service,
            operations,
            &project_id,
            record,
            state,
            "turn_completed",
        );
    }
    let mut next_record = active.record.clone();
    next_record.revision = next_record.revision.saturating_add(1);
    next_record.last_event_sequence = next_record.last_event_sequence.saturating_add(1);
    next_record.last_event_kind = method.replace('/', "_");
    next_record.updated_at = Utc::now();
    let next_record = match next_record.seal() {
        Ok(record) => record,
        Err(_) => {
            let record = active.record.clone();
            remove_active_execution(&execution_id, &active_handle)?;
            return finalize(
                service,
                operations,
                &project_id,
                record,
                CodexExecutionStateV1::OutcomeUnknown,
                "protocol_event_outcome_unknown",
            );
        }
    };
    let record = match publish_execution(service, &project_id, &next_record) {
        Ok(record) => record,
        Err(_) => {
            let record = active.record.clone();
            remove_active_execution(&execution_id, &active_handle)?;
            return finalize(
                service,
                operations,
                &project_id,
                record,
                CodexExecutionStateV1::OutcomeUnknown,
                "event_publication_outcome_unknown",
            );
        }
    };
    active.record = next_record;
    Ok(serde_json::json!({"status":"running","record":record}))
}

pub fn interrupt(
    service: &ManagementApplicationService,
    operations: Option<&Arc<Mutex<OperationStore>>>,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, ApplicationError> {
    let operations = operations.ok_or_else(|| apply("OPERATION_STORE_UNAVAILABLE"))?;
    let (project_id, execution_id) = payload_ids(payload)?;
    let persisted = load_execution(service, &project_id, &execution_id)?;
    if persisted.state.is_terminal() {
        reconcile_terminal_operation(operations, &persisted)?;
        return Ok(serde_json::json!({"status":"terminal","record":persisted}));
    }
    let Some(active_handle) = active_execution(&execution_id)? else {
        let source_effect_started = source_effect_may_have_started(&persisted);
        return finalize_with_effect_started(
            service,
            operations,
            &project_id,
            persisted,
            CodexExecutionStateV1::OutcomeUnknown,
            "interrupt_after_controller_restart",
            source_effect_started,
        );
    };
    let mut active = match active_handle.lock() {
        Ok(active) => active,
        Err(_) => {
            remove_active_execution(&execution_id, &active_handle)?;
            return finalize(
                service,
                operations,
                &project_id,
                persisted,
                CodexExecutionStateV1::OutcomeUnknown,
                "interrupt_lock_outcome_unknown",
            );
        }
    };
    let persisted = load_execution(service, &project_id, &execution_id)?;
    if persisted.state.is_terminal() {
        remove_active_execution(&execution_id, &active_handle)?;
        reconcile_terminal_operation(operations, &persisted)?;
        return Ok(serde_json::json!({"status":"terminal","record":persisted}));
    }
    if active.record.execution_fingerprint != persisted.execution_fingerprint {
        remove_active_execution(&execution_id, &active_handle)?;
        return finalize(
            service,
            operations,
            &project_id,
            persisted,
            CodexExecutionStateV1::OutcomeUnknown,
            "interrupt_state_mismatch",
        );
    }
    let thread_id = active
        .record
        .thread_ref
        .as_ref()
        .map(|thread| thread.thread_id.clone())
        .ok_or_else(|| apply("CODEX_OPERATION_LOST"))?;
    let turn_id = active
        .record
        .turn_ref
        .as_ref()
        .map(|turn| turn.turn_id.clone())
        .ok_or_else(|| apply("CODEX_OPERATION_LOST"))?;
    transition_operation(
        operations,
        &active.record.controller_operation_id,
        "cancelling",
        "codex_interrupt_requested",
    )?;
    if active
        .process
        .turn_interrupt(&thread_id, &turn_id, REQUEST_TIMEOUT)
        .is_err()
    {
        mark_operation_outcome_unknown(
            operations,
            &active.record.controller_operation_id,
            "codex_interrupt_outcome_unknown",
        );
        return Err(apply("CODEX_OPERATION_LOST"));
    }
    let mut next_record = active.record.clone();
    next_record.revision = next_record.revision.saturating_add(1);
    next_record.state = CodexExecutionStateV1::InterruptRequested;
    next_record.last_event_sequence = next_record.last_event_sequence.saturating_add(1);
    next_record.last_event_kind = "interrupt_requested".to_owned();
    next_record.updated_at = Utc::now();
    let next_record = next_record
        .seal()
        .map_err(|_| apply("CODEX_PROTOCOL_MISMATCH"))?;
    let record = publish_execution(service, &project_id, &next_record).inspect_err(|_| {
        mark_operation_outcome_unknown(
            operations,
            &active.record.controller_operation_id,
            "codex_interrupt_record_outcome_unknown",
        );
    })?;
    active.record = next_record;
    Ok(serde_json::json!({"status":"interrupt_requested","record":record}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_selection_rejects_stale_and_ambiguous_latest_snapshots() {
        let older_id = CapabilitySnapshotId::new();
        let newer_id = CapabilitySnapshotId::new();
        let now = Utc::now();
        let selected = latest_capability_snapshot_id([
            (older_id.clone(), now - chrono::Duration::seconds(1)),
            (newer_id.clone(), now),
        ])
        .unwrap();
        assert_eq!(selected, Some(newer_id.clone()));
        assert_ne!(selected, Some(older_id));

        let ambiguous =
            latest_capability_snapshot_id([(newer_id, now), (CapabilitySnapshotId::new(), now)]);
        assert!(matches!(
            ambiguous,
            Err(ApplicationError::Apply(code)) if code == "CODEX_PROTOCOL_MISMATCH"
        ));
    }

    #[test]
    fn launch_capability_rejects_missing_operation_or_permission_override() {
        let mut operations = BTreeMap::from([
            ("thread_start".to_owned(), true),
            ("turn_start".to_owned(), true),
        ]);
        let mut permissions = CodexPermissionCapabilitiesV1 {
            approval_policy_configurable: true,
            sandbox_mode_configurable: true,
            network_policy_observable: true,
            paid_action_observable: false,
        };
        assert!(launch_capability_is_supported(
            &operations,
            &permissions,
            CodexExecutionOperationV1::Start,
        ));
        assert!(!launch_capability_is_supported(
            &operations,
            &permissions,
            CodexExecutionOperationV1::Resume,
        ));
        operations.insert("thread_resume".to_owned(), true);
        assert!(launch_capability_is_supported(
            &operations,
            &permissions,
            CodexExecutionOperationV1::Resume,
        ));
        permissions.sandbox_mode_configurable = false;
        assert!(!launch_capability_is_supported(
            &operations,
            &permissions,
            CodexExecutionOperationV1::Start,
        ));
    }

    #[test]
    fn codex_approval_positive_creates_and_reuses_one_exact_durable_operation() {
        let root = std::env::temp_dir().join(format!(
            "star-codex-approval-{}-{}",
            std::process::id(),
            star_ipc::nonce()
        ));
        let approvals = Arc::new(Mutex::new(
            ApprovalStore::load(root.join("approvals.json")).unwrap(),
        ));
        let operations = Arc::new(Mutex::new(
            OperationStore::load(root.join("operations.json")).unwrap(),
        ));
        let project_id = ProjectId::new();
        let mut stage_value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../specs/fixtures/management/v1/stage-spec/full.json"
        ))
        .unwrap();
        stage_value["revision"] = serde_json::json!(1);
        stage_value["project_ids"] = serde_json::json!([project_id]);
        stage_value["dependencies"] = serde_json::json!([]);
        let stage: StageSpecV1 = serde_json::from_value(stage_value).unwrap();
        let capability_snapshot_id = CapabilitySnapshotId::new();
        let mut route_value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../specs/fixtures/management/v1/route-decision/full.json"
        ))
        .unwrap();
        route_value["route_decision_id"] = serde_json::json!(RouteDecisionId::new());
        route_value["revision"] = serde_json::json!(1);
        route_value["stage_revision"] = serde_json::json!(1);
        route_value["capability_snapshot_ref"]["document_id"] =
            serde_json::json!(capability_snapshot_id);
        let route: RouteDecisionV1 = serde_json::from_value(route_value).unwrap();
        let mut input = CodexTaskLaunchInput {
            project_id: stage.project_ids[0].clone(),
            route_decision_id: route.route_decision_id.clone(),
            capability_snapshot_id,
            stage,
            context_pack_ref: DocumentRef {
                schema_id: "star.context-pack".to_owned(),
                document_id: "ctx_test".to_owned(),
                revision: 1,
                sha256: Sha256Hash::digest(b"context"),
            },
            gate_decision_ref: DocumentRef {
                schema_id: "star.gate-decision".to_owned(),
                document_id: "gate_test".to_owned(),
                revision: 1,
                sha256: Sha256Hash::digest(b"gate"),
            },
            codex_executable: "C:\\Codex\\codex.exe".to_owned(),
            cwd: "C:\\project".to_owned(),
            instruction: "bounded test instruction".to_owned(),
            parent_execution_id: None,
            approval_id: None,
        };
        let arguments = serde_json::json!({
            "instruction_fingerprint":Sha256Hash::digest(input.instruction.as_bytes()),
        });
        let arguments_hash = canonical_sha256(&arguments).unwrap();
        let descriptor_hash = Sha256Hash::digest(b"codex.task.start|app-server-v2|1");
        let permission_actions = vec![
            "external.ai.execute".to_owned(),
            "local_read".to_owned(),
            "paid_action".to_owned(),
            "process_run".to_owned(),
        ];

        let required = resolve_launch_approval(
            &approvals,
            &operations,
            "codex.task.start",
            &input,
            &route,
            &serde_json::json!({"kind":"test"}),
            "req_codex_test",
            &arguments,
            &arguments_hash,
            &descriptor_hash,
            &permission_actions,
        )
        .unwrap()
        .unwrap_err();
        let approval_id: ApprovalId =
            serde_json::from_value(required["approval_id"].clone()).unwrap();
        let operation_id: OperationId =
            serde_json::from_value(required["operation_id"].clone()).unwrap();
        let operation = operations
            .lock()
            .unwrap()
            .get(operation_id.as_str())
            .unwrap();
        assert_eq!(operation.status, "approval_wait");
        assert_eq!(operation.tool_id, "codex.task.start");
        assert_eq!(operation.arguments_hash, arguments_hash.to_string());
        assert_eq!(operation.permission_actions, permission_actions);

        let approval = approvals.lock().unwrap().get(&approval_id).unwrap();
        approvals
            .lock()
            .unwrap()
            .resolve(
                &approval_id,
                &approval.scope_hash,
                ApprovalDecision::Approve,
                Some("approved for exact test scope".to_owned()),
                None,
                serde_json::json!({"kind":"test"}),
            )
            .unwrap();
        input.approval_id = Some(approval_id);
        let resolved = resolve_launch_approval(
            &approvals,
            &operations,
            "codex.task.start",
            &input,
            &route,
            &serde_json::json!({"kind":"test"}),
            "req_codex_test",
            &arguments,
            &arguments_hash,
            &descriptor_hash,
            &permission_actions,
        )
        .unwrap()
        .unwrap();
        assert_eq!(resolved.operation_id, operation_id);
        assert_eq!(resolved.decision, Some(ApprovalDecision::Approve));
        assert_eq!(
            operations
                .lock()
                .unwrap()
                .get(operation_id.as_str())
                .unwrap()
                .status,
            "approval_wait"
        );
    }
}
