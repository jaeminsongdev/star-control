use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use star_contracts::{
    ArtifactId, ProjectId, Sha256Hash,
    evidence::{ArtifactKind, ArtifactRef, ProducerRef, RedactionStatus, RetentionClass},
    fixed_mcp::ApprovalDecision,
    ids::{ChangePlanId, PatchSetId, WorkspaceSnapshotId},
    management::{FileOperationKind, PatchFileOperation, PatchSet, PatchSetStatus, ProjectPathRef},
    rust_style::{
        ClippySuggestion, RUST_STYLE_PIPELINE_ID, RUST_STYLE_PIPELINE_VERSION,
        RUST_STYLE_POLICY_APPROVAL_DECISION_SCHEMA_ID,
        RUST_STYLE_POLICY_APPROVAL_REQUEST_SCHEMA_ID, RUST_STYLE_STEP_EXECUTION_SCHEMA_ID,
        RustAutoPolicy, RustSideEffectResult, RustStepResult, RustStyleCoverageMatrix,
        RustStylePolicyApprovalDecision, RustStylePolicyApprovalRequest, RustStylePolicySnapshot,
        RustStyleStepExecution, RustToolchainBinding,
    },
};
use star_domain::versioned_fingerprint;
use star_execution::rust_style::{
    RustStyleAdapter, RustStyleAdapterError, RustStylePatchScope, RustToolOutput,
};
use star_validation::rust_style::{
    RustFileChange, RustFileSnapshot, RustStyleValidationError, parse_clippy_json_lines,
    select_allowlisted_suggestions, snapshot_fingerprint, validate_binding_policy_and_coverage,
    validate_clippy_fix_result, validate_side_effects,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RustCandidateState {
    Prepared,
    SucceededNoChange,
}

#[derive(Clone, Debug)]
pub struct RustStyleCandidate {
    pub project_id: ProjectId,
    pub base_workspace_snapshot_id: WorkspaceSnapshotId,
    pub before_fingerprint: Sha256Hash,
    pub expected_after_fingerprint: Sha256Hash,
    pub toolchain_fingerprint: Sha256Hash,
    pub policy_fingerprint: Sha256Hash,
    pub coverage_fingerprint: Sha256Hash,
    pub fixed_adapter_fingerprint: Sha256Hash,
    pub scope: RustStylePatchScope,
    pub auto_policy: RustAutoPolicy,
    pub selected_suggestions: Vec<ClippySuggestion>,
    pub changes: Vec<RustFileChange>,
    pub steps: Vec<RustStyleStepExecution>,
    pub idempotence_proved: bool,
    pub patch_set: Option<PatchSet>,
    pub forward_artifact: Option<serde_json::Value>,
    pub reverse_artifact: Option<serde_json::Value>,
    pub state: RustCandidateState,
    pub candidate_fingerprint: Sha256Hash,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RustStyleWorkflowError {
    #[error("Rust validation failed: {0}")]
    Validation(#[from] RustStyleValidationError),
    #[error("Rust tool adapter failed: {0}")]
    Adapter(#[from] RustStyleAdapterError),
    #[error("fixed Rust tool returned a failure")]
    ToolFailed,
    #[error("Rust candidate fingerprint failed")]
    Fingerprint,
    #[error("Rust apply is not authorized for the exact candidate")]
    ApprovalRequired,
    #[error("Rust apply permit was stale or already consumed")]
    PermitInvalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreApplyGateVerdict {
    AutoPass,
    HumanReview,
    Block,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RustAutoApplyGrant {
    pub project_id: ProjectId,
    pub profile_ref: String,
    pub pipeline_ref: String,
    pub toolchain_fingerprint: Sha256Hash,
    pub style_policy_fingerprint: Sha256Hash,
    pub coverage_fingerprint: Sha256Hash,
    pub scope_paths: Vec<ProjectPathRef>,
    pub max_files: u32,
    pub max_changed_bytes: u64,
    pub expires_at: String,
    pub grant_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug)]
pub struct RustApplyPermit {
    pub permit_id: String,
    pub candidate_fingerprint: Sha256Hash,
    pub approval_fingerprint: Sha256Hash,
    pub automatic: bool,
    consumed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceMutationObservation {
    Applied {
        post_gate_auto_pass: bool,
        evidence_complete: bool,
    },
    Partial,
    OutcomeUnknown,
    Stale,
}

pub trait RustSourceMutationPort {
    fn apply_exact(&mut self, candidate: &RustStyleCandidate) -> SourceMutationObservation;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RustApplyState {
    Applied,
    RecoveryRequired,
    FailedStale,
}

pub struct RustStyleCandidateInput<'a> {
    pub project_id: ProjectId,
    pub base_workspace_snapshot_id: WorkspaceSnapshotId,
    pub scope: RustStylePatchScope,
    pub binding: &'a RustToolchainBinding,
    pub policy: &'a RustStylePolicySnapshot,
    pub coverage: &'a RustStyleCoverageMatrix,
}

pub fn prepare_rust_style_candidate(
    input: RustStyleCandidateInput<'_>,
    preview: &mut impl RustStyleAdapter,
    replay: &mut impl RustStyleAdapter,
) -> Result<RustStyleCandidate, RustStyleWorkflowError> {
    let RustStyleCandidateInput {
        project_id,
        base_workspace_snapshot_id,
        scope,
        binding,
        policy,
        coverage,
    } = input;
    validate_binding_policy_and_coverage(binding, policy, coverage)?;
    if policy.scope_project_id != project_id {
        return Err(RustStyleValidationError::AutoScopeMismatch.into());
    }
    let before = preview.snapshot()?;
    let before_fingerprint = snapshot_fingerprint(&before)?;
    let mut steps = Vec::new();

    let rustfmt_first = require_success(preview.run_rustfmt(false)?)?;
    let after_rustfmt = preview.snapshot()?;
    validate_side_effects(&before, &after_rustfmt, policy)?;
    steps.push(step_record(
        5,
        "rustfmt_first",
        &before,
        &after_rustfmt,
        policy,
        Some(&rustfmt_first),
        coverage,
    )?);

    let clippy_check = require_success(preview.run_clippy_check()?)?;
    let coverage_cell = coverage
        .required_cell_ids
        .first()
        .ok_or(RustStyleValidationError::CoverageIncomplete)?;
    let suggestions = parse_clippy_json_lines(&clippy_check.stdout, coverage_cell, &after_rustfmt)?;
    let selected = select_allowlisted_suggestions(&suggestions, policy, &after_rustfmt)?;
    let after_clippy = if selected.is_empty() {
        after_rustfmt.clone()
    } else {
        let lint_ids = selected
            .iter()
            .map(|suggestion| suggestion.lint_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let clippy_fix = require_success(preview.run_clippy_fix(&lint_ids)?)?;
        let after = preview.snapshot()?;
        validate_side_effects(&after_rustfmt, &after, policy)?;
        validate_clippy_fix_result(&after_rustfmt, &after, &selected)?;
        steps.push(step_record(
            6,
            "clippy_allowlisted_fix",
            &after_rustfmt,
            &after,
            policy,
            Some(&clippy_fix),
            coverage,
        )?);
        after
    };

    let rustfmt_final = require_success(preview.run_rustfmt(false)?)?;
    let final_files = preview.snapshot()?;
    validate_side_effects(&after_clippy, &final_files, policy)?;
    let final_summary = validate_side_effects(&before, &final_files, policy)?;
    steps.push(step_record(
        7,
        "rustfmt_final",
        &after_clippy,
        &final_files,
        policy,
        Some(&rustfmt_final),
        coverage,
    )?);

    prove_idempotence(replay, &final_files, policy, coverage)?;
    steps.push(step_record(
        11,
        "idempotence_replay",
        &final_files,
        &final_files,
        policy,
        None,
        coverage,
    )?);

    let fmt_check = require_success(preview.run_rustfmt(true)?)?;
    let final_clippy = require_success(preview.run_clippy_check()?)?;
    if preview.snapshot()? != final_files {
        return Err(RustStyleValidationError::SideEffectViolation.into());
    }
    let combined_check = RustToolOutput {
        success: true,
        exit_code: Some(0),
        stdout: format!("{}\n{}", fmt_check.stdout, final_clippy.stdout),
        stderr: format!("{}\n{}", fmt_check.stderr, final_clippy.stderr),
        command_fingerprint: versioned_fingerprint(
            "star.rust-style-candidate-checks",
            1,
            &[
                fmt_check.command_fingerprint,
                final_clippy.command_fingerprint,
            ],
        )
        .map_err(|_| RustStyleWorkflowError::Fingerprint)?,
    };
    steps.push(step_record(
        12,
        "candidate_validate",
        &final_files,
        &final_files,
        policy,
        Some(&combined_check),
        coverage,
    )?);

    let expected_after_fingerprint = snapshot_fingerprint(&final_files)?;
    let (patch_set, forward_artifact, reverse_artifact) = if final_summary.changes.is_empty() {
        (None, None, None)
    } else {
        let (patch, forward, reverse) = finalize_patch_set(
            &project_id,
            &base_workspace_snapshot_id,
            &scope,
            &final_summary.changes,
            &expected_after_fingerprint,
            binding,
            policy,
            coverage,
            &steps,
        )?;
        (Some(patch), Some(forward), Some(reverse))
    };
    let state = if patch_set.is_some() {
        RustCandidateState::Prepared
    } else {
        RustCandidateState::SucceededNoChange
    };
    let candidate_fingerprint = versioned_fingerprint(
        "star.rust-style-candidate",
        1,
        &serde_json::json!({
            "project_id":project_id,
            "base_workspace_snapshot_id":base_workspace_snapshot_id,
            "before_fingerprint":before_fingerprint,
            "expected_after_fingerprint":expected_after_fingerprint,
            "toolchain_fingerprint":binding.binding_fingerprint,
            "policy_fingerprint":policy.policy_fingerprint,
            "coverage_fingerprint":coverage.coverage_fingerprint,
            "fixed_adapter_fingerprint":policy.fixed_adapter_definition_fingerprint,
            "scope":scope,
            "auto_policy":policy.auto_policy,
            "selected_suggestions":selected,
            "changes":final_summary.changes.iter().map(|change| serde_json::json!({
                "path":change.path,
                "before":change.before_sha256,
                "after":change.after_sha256,
            })).collect::<Vec<_>>(),
            "steps":steps.iter().map(|step| &step.step_execution_fingerprint).collect::<Vec<_>>(),
            "patch_set":patch_set,
            "idempotence_proved":true,
            "state":format!("{state:?}"),
        }),
    )
    .map_err(|_| RustStyleWorkflowError::Fingerprint)?;
    Ok(RustStyleCandidate {
        project_id,
        base_workspace_snapshot_id,
        before_fingerprint,
        expected_after_fingerprint,
        toolchain_fingerprint: binding.binding_fingerprint.clone(),
        policy_fingerprint: policy.policy_fingerprint.clone(),
        coverage_fingerprint: coverage.coverage_fingerprint.clone(),
        fixed_adapter_fingerprint: policy.fixed_adapter_definition_fingerprint.clone(),
        scope,
        auto_policy: policy.auto_policy,
        selected_suggestions: selected,
        changes: final_summary.changes,
        steps,
        idempotence_proved: true,
        patch_set,
        forward_artifact,
        reverse_artifact,
        state,
        candidate_fingerprint,
    })
}

pub fn authorize_exact_human(
    candidate: &RustStyleCandidate,
    approved_candidate_fingerprint: &Sha256Hash,
    pre_gate: PreApplyGateVerdict,
) -> Result<RustApplyPermit, RustStyleWorkflowError> {
    if candidate.state != RustCandidateState::Prepared
        || &candidate.candidate_fingerprint != approved_candidate_fingerprint
        || pre_gate != PreApplyGateVerdict::AutoPass
    {
        return Err(RustStyleWorkflowError::ApprovalRequired);
    }
    permit(candidate, approved_candidate_fingerprint.clone(), false)
}

pub struct PersonalAutoAuthorization<'a> {
    pub candidate: &'a RustStyleCandidate,
    pub policy: &'a RustStylePolicySnapshot,
    pub grant: &'a RustAutoApplyGrant,
    pub approved_patch_set_id: &'a PatchSetId,
    pub approved_patch_fingerprint: &'a Sha256Hash,
    pub approval_request: &'a RustStylePolicyApprovalRequest,
    pub approval_decision: &'a RustStylePolicyApprovalDecision,
    pub pre_gate: PreApplyGateVerdict,
    pub now: DateTime<Utc>,
}

pub fn authorize_personal_auto(
    authorization: PersonalAutoAuthorization<'_>,
) -> Result<RustApplyPermit, RustStyleWorkflowError> {
    let PersonalAutoAuthorization {
        candidate,
        policy,
        grant,
        approved_patch_set_id,
        approved_patch_fingerprint,
        approval_request,
        approval_decision,
        pre_gate,
        now,
    } = authorization;
    let expected_request = prepare_personal_auto_approval_request_for_patch(
        candidate,
        policy,
        grant,
        approved_patch_set_id,
        approved_patch_fingerprint,
        pre_gate,
        approval_request.pre_gate_id.clone(),
        approval_request.pre_gate_revision,
        approval_request.pre_gate_fingerprint.clone(),
        now,
    )?;
    let sealed_decision = seal_rust_style_policy_approval_decision(approval_decision.clone())?;
    if &expected_request != approval_request
        || &sealed_decision != approval_decision
        || approval_decision.request_fingerprint != approval_request.request_fingerprint
        || approval_decision.decision != ApprovalDecision::Approve
    {
        return Err(RustStyleWorkflowError::ApprovalRequired);
    }
    permit(
        candidate,
        approval_decision.decision_fingerprint.clone(),
        true,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_personal_auto_approval_request(
    candidate: &RustStyleCandidate,
    policy: &RustStylePolicySnapshot,
    grant: &RustAutoApplyGrant,
    pre_gate: PreApplyGateVerdict,
    pre_gate_id: star_contracts::ids::GateId,
    pre_gate_revision: u64,
    pre_gate_fingerprint: Sha256Hash,
    now: DateTime<Utc>,
) -> Result<RustStylePolicyApprovalRequest, RustStyleWorkflowError> {
    let patch_set = candidate
        .patch_set
        .as_ref()
        .ok_or(RustStyleWorkflowError::ApprovalRequired)?;
    prepare_personal_auto_approval_request_for_patch(
        candidate,
        policy,
        grant,
        &patch_set.patch_set_id,
        &patch_set.patch_fingerprint,
        pre_gate,
        pre_gate_id,
        pre_gate_revision,
        pre_gate_fingerprint,
        now,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_personal_auto_approval_request_for_patch(
    candidate: &RustStyleCandidate,
    policy: &RustStylePolicySnapshot,
    grant: &RustAutoApplyGrant,
    approved_patch_set_id: &PatchSetId,
    approved_patch_fingerprint: &Sha256Hash,
    pre_gate: PreApplyGateVerdict,
    pre_gate_id: star_contracts::ids::GateId,
    pre_gate_revision: u64,
    pre_gate_fingerprint: Sha256Hash,
    now: DateTime<Utc>,
) -> Result<RustStylePolicyApprovalRequest, RustStyleWorkflowError> {
    let expires_at = DateTime::parse_from_rfc3339(&grant.expires_at)
        .map_err(|_| RustStyleWorkflowError::ApprovalRequired)?
        .with_timezone(&Utc);
    let scope_ok = candidate
        .changes
        .iter()
        .all(|change| path_in_any_scope(&change.path, &grant.scope_paths));
    let changed_bytes = candidate
        .changes
        .iter()
        .map(|change| change.before_bytes.len() as u64 + change.after_bytes.len() as u64)
        .sum::<u64>();
    if policy.auto_policy != RustAutoPolicy::PersonalAuto
        || candidate.state != RustCandidateState::Prepared
        || !candidate.idempotence_proved
        || pre_gate != PreApplyGateVerdict::AutoPass
        || grant.project_id != candidate.project_id
        || grant.profile_ref != policy.profile_ref
        || grant.pipeline_ref != policy.pipeline_ref
        || grant.toolchain_fingerprint != candidate.toolchain_fingerprint
        || grant.style_policy_fingerprint != candidate.policy_fingerprint
        || grant.coverage_fingerprint != candidate.coverage_fingerprint
        || candidate.changes.len() > grant.max_files as usize
        || changed_bytes > grant.max_changed_bytes
        || !scope_ok
        || now >= expires_at
    {
        return Err(RustStyleWorkflowError::ApprovalRequired);
    }
    let expected_grant = versioned_fingerprint(
        "star.rust-style-auto-grant",
        1,
        &serde_json::json!({
            "project_id":grant.project_id,
            "profile_ref":grant.profile_ref,
            "pipeline_ref":grant.pipeline_ref,
            "toolchain_fingerprint":grant.toolchain_fingerprint,
            "style_policy_fingerprint":grant.style_policy_fingerprint,
            "coverage_fingerprint":grant.coverage_fingerprint,
            "scope_paths":grant.scope_paths,
            "max_files":grant.max_files,
            "max_changed_bytes":grant.max_changed_bytes,
            "expires_at":grant.expires_at,
        }),
    )
    .map_err(|_| RustStyleWorkflowError::Fingerprint)?;
    if expected_grant != grant.grant_fingerprint {
        return Err(RustStyleWorkflowError::ApprovalRequired);
    }
    let mut scope_paths = grant.scope_paths.clone();
    scope_paths.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    scope_paths.dedup();
    let mut changed_paths = candidate
        .changes
        .iter()
        .map(|change| change.path.clone())
        .collect::<Vec<_>>();
    changed_paths.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    changed_paths.dedup();
    let request = RustStylePolicyApprovalRequest {
        schema_id: RUST_STYLE_POLICY_APPROVAL_REQUEST_SCHEMA_ID.to_owned(),
        schema_version: 1,
        contract_version: 1,
        project_id: candidate.project_id.clone(),
        profile_ref: policy.profile_ref.clone(),
        pipeline_ref: policy.pipeline_ref.clone(),
        patch_set_id: approved_patch_set_id.clone(),
        patch_fingerprint: approved_patch_fingerprint.clone(),
        candidate_fingerprint: candidate.candidate_fingerprint.clone(),
        before_fingerprint: candidate.before_fingerprint.clone(),
        expected_after_fingerprint: candidate.expected_after_fingerprint.clone(),
        toolchain_fingerprint: candidate.toolchain_fingerprint.clone(),
        policy_fingerprint: candidate.policy_fingerprint.clone(),
        coverage_fingerprint: candidate.coverage_fingerprint.clone(),
        fixed_adapter_fingerprint: candidate.fixed_adapter_fingerprint.clone(),
        standing_grant_fingerprint: grant.grant_fingerprint.clone(),
        pre_gate_id,
        pre_gate_revision,
        pre_gate_fingerprint,
        scope_paths,
        changed_paths,
        request_fingerprint: Sha256Hash::digest(b"pending-rust-style-policy-approval"),
    };
    seal_rust_style_policy_approval_request(request)
}

pub fn seal_rust_style_policy_approval_request(
    mut request: RustStylePolicyApprovalRequest,
) -> Result<RustStylePolicyApprovalRequest, RustStyleWorkflowError> {
    if request.schema_id != RUST_STYLE_POLICY_APPROVAL_REQUEST_SCHEMA_ID
        || request.schema_version != 1
        || request.contract_version != 1
        || request.profile_ref.trim().is_empty()
        || request.pipeline_ref.trim().is_empty()
        || request.pre_gate_revision == 0
        || request.scope_paths.is_empty()
        || request.changed_paths.is_empty()
    {
        return Err(RustStyleWorkflowError::ApprovalRequired);
    }
    request
        .scope_paths
        .sort_by(|left, right| left.as_str().cmp(right.as_str()));
    request.scope_paths.dedup();
    request
        .changed_paths
        .sort_by(|left, right| left.as_str().cmp(right.as_str()));
    request.changed_paths.dedup();
    request.request_fingerprint = rust_style_policy_approval_request_fingerprint(&request)?;
    Ok(request)
}

pub fn seal_rust_style_policy_approval_decision(
    mut decision: RustStylePolicyApprovalDecision,
) -> Result<RustStylePolicyApprovalDecision, RustStyleWorkflowError> {
    if decision.schema_id != RUST_STYLE_POLICY_APPROVAL_DECISION_SCHEMA_ID
        || decision.schema_version != 1
        || decision.contract_version != 1
        || decision.resolved_at.trim().is_empty()
        || DateTime::parse_from_rfc3339(&decision.resolved_at).is_err()
    {
        return Err(RustStyleWorkflowError::ApprovalRequired);
    }
    decision.decision_fingerprint = versioned_fingerprint(
        "star.rust-style-policy-approval-decision",
        1,
        &serde_json::json!({
            "approval_id":decision.approval_id,
            "scope_hash":decision.scope_hash,
            "request_fingerprint":decision.request_fingerprint,
            "decision":decision.decision,
            "resolved_at":decision.resolved_at,
        }),
    )
    .map_err(|_| RustStyleWorkflowError::Fingerprint)?;
    Ok(decision)
}

fn rust_style_policy_approval_request_fingerprint(
    request: &RustStylePolicyApprovalRequest,
) -> Result<Sha256Hash, RustStyleWorkflowError> {
    versioned_fingerprint(
        "star.rust-style-policy-approval-request",
        1,
        &serde_json::json!({
            "schema_id":request.schema_id,
            "schema_version":request.schema_version,
            "contract_version":request.contract_version,
            "project_id":request.project_id,
            "profile_ref":request.profile_ref,
            "pipeline_ref":request.pipeline_ref,
            "patch_set_id":request.patch_set_id,
            "patch_fingerprint":request.patch_fingerprint,
            "candidate_fingerprint":request.candidate_fingerprint,
            "before_fingerprint":request.before_fingerprint,
            "expected_after_fingerprint":request.expected_after_fingerprint,
            "toolchain_fingerprint":request.toolchain_fingerprint,
            "policy_fingerprint":request.policy_fingerprint,
            "coverage_fingerprint":request.coverage_fingerprint,
            "fixed_adapter_fingerprint":request.fixed_adapter_fingerprint,
            "standing_grant_fingerprint":request.standing_grant_fingerprint,
            "pre_gate_id":request.pre_gate_id,
            "pre_gate_revision":request.pre_gate_revision,
            "pre_gate_fingerprint":request.pre_gate_fingerprint,
            "scope_paths":request.scope_paths,
            "changed_paths":request.changed_paths,
        }),
    )
    .map_err(|_| RustStyleWorkflowError::Fingerprint)
}

pub fn apply_with_permit(
    candidate: &RustStyleCandidate,
    permit: &mut RustApplyPermit,
    port: &mut impl RustSourceMutationPort,
) -> Result<RustApplyState, RustStyleWorkflowError> {
    if permit.consumed
        || permit.candidate_fingerprint != candidate.candidate_fingerprint
        || candidate.state != RustCandidateState::Prepared
        || candidate.patch_set.is_none()
    {
        return Err(RustStyleWorkflowError::PermitInvalid);
    }
    permit.consumed = true;
    Ok(match port.apply_exact(candidate) {
        SourceMutationObservation::Applied {
            post_gate_auto_pass: true,
            evidence_complete: true,
        } => RustApplyState::Applied,
        SourceMutationObservation::Applied { .. }
        | SourceMutationObservation::Partial
        | SourceMutationObservation::OutcomeUnknown => RustApplyState::RecoveryRequired,
        SourceMutationObservation::Stale => RustApplyState::FailedStale,
    })
}

fn prove_idempotence(
    replay: &mut impl RustStyleAdapter,
    final_files: &[RustFileSnapshot],
    policy: &RustStylePolicySnapshot,
    coverage: &RustStyleCoverageMatrix,
) -> Result<(), RustStyleWorkflowError> {
    replay.materialize_exact(final_files)?;
    if replay.snapshot()? != final_files {
        return Err(RustStyleValidationError::NonIdempotent.into());
    }
    require_success(replay.run_rustfmt(false)?)?;
    let after_first_fmt = replay.snapshot()?;
    if validate_side_effects(final_files, &after_first_fmt, policy)?
        .changes
        .is_empty()
        .not()
    {
        return Err(RustStyleValidationError::NonIdempotent.into());
    }
    let clippy = require_success(replay.run_clippy_check()?)?;
    let coverage_cell = coverage
        .required_cell_ids
        .first()
        .ok_or(RustStyleValidationError::CoverageIncomplete)?;
    let suggestions = parse_clippy_json_lines(&clippy.stdout, coverage_cell, &after_first_fmt)?;
    if !select_allowlisted_suggestions(&suggestions, policy, &after_first_fmt)?.is_empty() {
        return Err(RustStyleValidationError::NonIdempotent.into());
    }
    require_success(replay.run_rustfmt(false)?)?;
    if replay.snapshot()? != final_files {
        return Err(RustStyleValidationError::NonIdempotent.into());
    }
    Ok(())
}

trait BoolNot {
    fn not(self) -> bool;
}

impl BoolNot for bool {
    fn not(self) -> bool {
        !self
    }
}

fn require_success(output: RustToolOutput) -> Result<RustToolOutput, RustStyleWorkflowError> {
    if output.success && output.exit_code == Some(0) {
        Ok(output)
    } else {
        Err(RustStyleWorkflowError::ToolFailed)
    }
}

fn step_record(
    ordinal: u32,
    step_id: &str,
    before: &[RustFileSnapshot],
    after: &[RustFileSnapshot],
    policy: &RustStylePolicySnapshot,
    tool: Option<&RustToolOutput>,
    coverage: &RustStyleCoverageMatrix,
) -> Result<RustStyleStepExecution, RustStyleWorkflowError> {
    let subject_before = snapshot_fingerprint(before)?;
    let subject_after = snapshot_fingerprint(after)?;
    let now = Utc::now().to_rfc3339();
    let step_execution_id = format!("rust-style-{ordinal}-{}", &subject_before.as_str()[7..19]);
    let mut step = RustStyleStepExecution {
        schema_id: RUST_STYLE_STEP_EXECUTION_SCHEMA_ID.to_owned(),
        schema_version: 1,
        contract_version: 1,
        step_execution_id,
        ordinal,
        step_id: step_id.to_owned(),
        pipeline_ref: format!("{RUST_STYLE_PIPELINE_ID}@{RUST_STYLE_PIPELINE_VERSION}"),
        adapter_fingerprint: policy.fixed_adapter_definition_fingerprint.clone(),
        subject_before,
        subject_after,
        tool_descriptor_ref: tool.map(|_| format!("star.rust.style.{step_id}")),
        task_invocation_ref: tool.map(|output| output.command_fingerprint.to_string()),
        execution_result_ref: tool.map(|output| format!("exit:{:?}", output.exit_code)),
        coverage_cell_refs: coverage.required_cell_ids.clone(),
        diagnostic_set_ref: (step_id.contains("clippy") || step_id == "candidate_validate")
            .then(|| format!("diagnostics:{step_id}")),
        suggestion_manifest_ref: step_id
            .contains("clippy")
            .then(|| format!("suggestions:{step_id}")),
        diff_artifact_ref: (before != after).then(|| format!("diff:{step_id}")),
        filesystem_manifest_ref: format!("manifest:{step_id}"),
        side_effect_result: RustSideEffectResult::Pass,
        result: RustStepResult::Succeeded,
        started_at: now.clone(),
        finished_at: now,
        step_execution_fingerprint: Sha256Hash::digest(b"unsealed-rust-step"),
    };
    step.step_execution_fingerprint = versioned_fingerprint(
        RUST_STYLE_STEP_EXECUTION_SCHEMA_ID,
        1,
        &serde_json::json!({
            "step_execution_id":step.step_execution_id,
            "ordinal":step.ordinal,
            "step_id":step.step_id,
            "pipeline_ref":step.pipeline_ref,
            "adapter_fingerprint":step.adapter_fingerprint,
            "subject_before":step.subject_before,
            "subject_after":step.subject_after,
            "tool_descriptor_ref":step.tool_descriptor_ref,
            "task_invocation_ref":step.task_invocation_ref,
            "execution_result_ref":step.execution_result_ref,
            "coverage_cell_refs":step.coverage_cell_refs,
            "diagnostic_set_ref":step.diagnostic_set_ref,
            "suggestion_manifest_ref":step.suggestion_manifest_ref,
            "diff_artifact_ref":step.diff_artifact_ref,
            "filesystem_manifest_ref":step.filesystem_manifest_ref,
            "side_effect_result":step.side_effect_result,
            "result":step.result,
        }),
    )
    .map_err(|_| RustStyleWorkflowError::Fingerprint)?;
    Ok(step)
}

#[allow(clippy::too_many_arguments)]
fn finalize_patch_set(
    project_id: &ProjectId,
    base_workspace_snapshot_id: &WorkspaceSnapshotId,
    scope: &RustStylePatchScope,
    changes: &[RustFileChange],
    expected_after_fingerprint: &Sha256Hash,
    binding: &RustToolchainBinding,
    policy: &RustStylePolicySnapshot,
    coverage: &RustStyleCoverageMatrix,
    steps: &[RustStyleStepExecution],
) -> Result<(PatchSet, serde_json::Value, serde_json::Value), RustStyleWorkflowError> {
    let forward_files = changes
        .iter()
        .map(|change| {
            let after_utf8 = String::from_utf8(change.after_bytes.clone())
                .map_err(|_| RustStyleValidationError::SideEffectViolation)?;
            Ok(serde_json::json!({
                "path":change.path,
                "before_sha256":change.before_sha256,
                "after_sha256":change.after_sha256,
                "after_utf8":after_utf8,
            }))
        })
        .collect::<Result<Vec<_>, RustStyleWorkflowError>>()?;
    let reverse_files = changes
        .iter()
        .rev()
        .map(|change| {
            let after_utf8 = String::from_utf8(change.before_bytes.clone())
                .map_err(|_| RustStyleValidationError::SideEffectViolation)?;
            Ok(serde_json::json!({
                "path":change.path,
                "before_sha256":change.after_sha256,
                "after_sha256":change.before_sha256,
                "after_utf8":after_utf8,
            }))
        })
        .collect::<Result<Vec<_>, RustStyleWorkflowError>>()?;
    let forward_artifact = serde_json::json!({
        "schema_id":"star.rust-style-forward-patch",
        "schema_version":1,
        "pipeline_ref":policy.pipeline_ref,
        "toolchain_fingerprint":binding.binding_fingerprint,
        "policy_fingerprint":policy.policy_fingerprint,
        "coverage_fingerprint":coverage.coverage_fingerprint,
        "fixed_adapter_fingerprint":policy.fixed_adapter_definition_fingerprint,
        "scope":scope,
        "auto_policy":policy.auto_policy,
        "steps":steps.iter().map(|step| &step.step_execution_fingerprint).collect::<Vec<_>>(),
        "files":forward_files,
        "idempotence":"proved",
    });
    let reverse_artifact = serde_json::json!({
        "schema_id":"star.rust-style-reverse-patch",
        "schema_version":1,
        "files":reverse_files,
    });
    let forward_bytes = serde_json::to_vec_pretty(&forward_artifact)
        .map_err(|_| RustStyleWorkflowError::Fingerprint)?;
    let reverse_bytes = serde_json::to_vec_pretty(&reverse_artifact)
        .map_err(|_| RustStyleWorkflowError::Fingerprint)?;
    let now = Utc::now();
    let producer = ProducerRef {
        component: "star-application/rust-style".to_owned(),
        product_version: "0.1.0".to_owned(),
        build_id: env!("CARGO_PKG_VERSION").to_owned(),
        platform: "windows".to_owned(),
    };
    let forward_ref = ArtifactRef {
        artifact_id: ArtifactId::new(),
        kind: ArtifactKind::Diff,
        project_id: Some(project_id.clone()),
        relative_path: "rust-style/forward-patch.json".to_owned(),
        media_type: "application/json".to_owned(),
        size_bytes: forward_bytes.len() as u64,
        sha256: Sha256Hash::digest(&forward_bytes),
        created_at: now,
        producer: producer.clone(),
        redaction_status: RedactionStatus::NotNeeded,
        retention_class: RetentionClass::Evidence,
        source_artifact_ref: None,
    };
    let reverse_ref = ArtifactRef {
        artifact_id: ArtifactId::new(),
        kind: ArtifactKind::Diff,
        project_id: Some(project_id.clone()),
        relative_path: "rust-style/reverse-patch.json".to_owned(),
        media_type: "application/json".to_owned(),
        size_bytes: reverse_bytes.len() as u64,
        sha256: Sha256Hash::digest(&reverse_bytes),
        created_at: now,
        producer,
        redaction_status: RedactionStatus::NotNeeded,
        retention_class: RetentionClass::Hold,
        source_artifact_ref: None,
    };
    let operations = changes
        .iter()
        .map(|change| {
            let operation_fingerprint = versioned_fingerprint(
                "star.rust-style-patch-operation",
                1,
                &serde_json::json!({
                    "kind":"modify",
                    "path":change.path,
                    "before_sha256":change.before_sha256,
                    "after_sha256":change.after_sha256,
                }),
            )
            .map_err(|_| RustStyleWorkflowError::Fingerprint)?;
            Ok(PatchFileOperation {
                kind: FileOperationKind::Modify,
                path: change.path.clone(),
                rename_from: None,
                before_sha256: Some(change.before_sha256.clone()),
                after_sha256: Some(change.after_sha256.clone()),
                before_mode: None,
                after_mode: None,
                operation_fingerprint,
            })
        })
        .collect::<Result<Vec<_>, RustStyleWorkflowError>>()?;
    let change_plan_id = ChangePlanId::new();
    let patch_fingerprint = versioned_fingerprint(
        "star.rust-style-patch-set",
        1,
        &serde_json::json!({
            "project_id":project_id,
            "base_workspace_snapshot_id":base_workspace_snapshot_id,
            "change_plan_id":change_plan_id,
            "operations":operations,
            "forward_artifact_sha256":forward_ref.sha256,
            "reverse_artifact_sha256":reverse_ref.sha256,
            "expected_after_fingerprint":expected_after_fingerprint,
        }),
    )
    .map_err(|_| RustStyleWorkflowError::Fingerprint)?;
    Ok((
        PatchSet {
            schema_id: "star.patch-set".to_owned(),
            schema_version: 1,
            patch_set_id: PatchSetId::new(),
            change_plan_id,
            change_plan_revision: 1,
            project_id: project_id.clone(),
            base_workspace_snapshot_id: base_workspace_snapshot_id.clone(),
            patch_fingerprint,
            operations,
            patch_artifact_refs: vec![forward_ref],
            affected_finding_ids: Vec::new(),
            expected_result_fingerprint: Some(expected_after_fingerprint.clone()),
            status: PatchSetStatus::Proposed,
            applied_workspace_snapshot_id: None,
            rollback_artifact_refs: vec![reverse_ref],
        },
        forward_artifact,
        reverse_artifact,
    ))
}

fn permit(
    candidate: &RustStyleCandidate,
    approval_fingerprint: Sha256Hash,
    automatic: bool,
) -> Result<RustApplyPermit, RustStyleWorkflowError> {
    let permit_fingerprint = versioned_fingerprint(
        "star.rust-style-apply-permit",
        1,
        &serde_json::json!({
            "candidate_fingerprint":candidate.candidate_fingerprint,
            "approval_fingerprint":approval_fingerprint,
            "automatic":automatic,
        }),
    )
    .map_err(|_| RustStyleWorkflowError::Fingerprint)?;
    Ok(RustApplyPermit {
        permit_id: format!("rsp_{}", &permit_fingerprint.as_str()[7..33]),
        candidate_fingerprint: candidate.candidate_fingerprint.clone(),
        approval_fingerprint,
        automatic,
        consumed: false,
    })
}

fn path_in_any_scope(path: &ProjectPathRef, scopes: &[ProjectPathRef]) -> bool {
    scopes.iter().any(|scope| {
        path == scope
            || path
                .as_str()
                .strip_prefix(scope.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use star_contracts::rust_style::{
        ClippyAllowlistSource, ClippyFixAllowlistEntry, RustAvailabilityState,
        RustCatalogLifecycle, RustCompleteness, RustCoverageExecution, RustCoveragePhase,
        RustEditionBinding, RustExecutableBinding, RustSourceBinding, RustSourceOwnership,
        RustStyleCoverageCell, RustTargetKind, RustTargetState, RustToolchainPinState,
        RustToolchainSource, SuggestionApplicability,
    };

    use super::*;

    struct MemoryAdapter {
        files: Vec<RustFileSnapshot>,
        inject_generated_write: bool,
        sequence: u64,
    }

    impl MemoryAdapter {
        fn new(files: Vec<RustFileSnapshot>) -> Self {
            Self {
                files,
                inject_generated_write: false,
                sequence: 0,
            }
        }

        fn source_mut(&mut self) -> &mut RustFileSnapshot {
            self.files
                .iter_mut()
                .find(|file| file.path.as_str() == "src/lib.rs")
                .unwrap()
        }

        fn output(&mut self, stdout: String) -> RustToolOutput {
            self.sequence += 1;
            RustToolOutput {
                success: true,
                exit_code: Some(0),
                stdout,
                stderr: String::new(),
                command_fingerprint: Sha256Hash::digest(
                    format!("memory-command-{}", self.sequence).as_bytes(),
                ),
            }
        }
    }

    impl RustStyleAdapter for MemoryAdapter {
        fn snapshot(&self) -> Result<Vec<RustFileSnapshot>, RustStyleAdapterError> {
            Ok(self.files.clone())
        }

        fn materialize_exact(
            &mut self,
            files: &[RustFileSnapshot],
        ) -> Result<(), RustStyleAdapterError> {
            self.files = files.to_vec();
            Ok(())
        }

        fn run_rustfmt(&mut self, check: bool) -> Result<RustToolOutput, RustStyleAdapterError> {
            if !check {
                let source = self.source_mut();
                if source.bytes == b"pub fn answer()->i32{return 1;}\n" {
                    source.bytes = b"pub fn answer() -> i32 {\n    return 1;\n}\n".to_vec();
                }
                if self.inject_generated_write {
                    self.files
                        .iter_mut()
                        .find(|file| file.path.as_str() == "generated/out.rs")
                        .unwrap()
                        .bytes
                        .push(b'!');
                    self.inject_generated_write = false;
                }
            }
            Ok(self.output(String::new()))
        }

        fn run_clippy_check(&mut self) -> Result<RustToolOutput, RustStyleAdapterError> {
            let source = &self.source_mut().bytes;
            let stdout = if let Some(start) = find_bytes(source, b"return 1;") {
                let end = start + b"return 1;".len();
                serde_json::json!({
                    "reason":"compiler-message",
                    "message":{
                        "code":{"code":"clippy::needless_return"},
                        "spans":[],
                        "children":[{
                            "spans":[{
                                "file_name":"src/lib.rs",
                                "byte_start":start,
                                "byte_end":end,
                                "suggested_replacement":"1",
                                "suggestion_applicability":"MachineApplicable",
                                "expansion":null
                            }]
                        }]
                    }
                })
                .to_string()
            } else {
                String::new()
            };
            Ok(self.output(stdout))
        }

        fn run_clippy_fix(
            &mut self,
            exact_lint_ids: &[String],
        ) -> Result<RustToolOutput, RustStyleAdapterError> {
            assert_eq!(exact_lint_ids, ["clippy::needless_return"]);
            let source = self.source_mut();
            let start = find_bytes(&source.bytes, b"return 1;").unwrap();
            source
                .bytes
                .splice(start..start + b"return 1;".len(), b"1".iter().copied());
            Ok(self.output(String::new()))
        }
    }

    fn file(path: &str, bytes: &[u8], ownership: RustSourceOwnership) -> RustFileSnapshot {
        RustFileSnapshot {
            path: ProjectPathRef::parse(path).unwrap(),
            bytes: bytes.to_vec(),
            ownership,
        }
    }

    fn initial_files() -> Vec<RustFileSnapshot> {
        vec![
            file(
                "Cargo.toml",
                b"[package]\nname='fixture'\n",
                RustSourceOwnership::Handwritten,
            ),
            file(
                "generated/out.rs",
                b"pub const GENERATED: bool = true;\n",
                RustSourceOwnership::Generated,
            ),
            file(
                "src/lib.rs",
                b"pub fn answer()->i32{return 1;}\n",
                RustSourceOwnership::Handwritten,
            ),
            file(
                "vendor/lib.rs",
                b"pub fn vendored() {}\n",
                RustSourceOwnership::Vendor,
            ),
        ]
    }

    fn executable(logical_id: &str) -> RustExecutableBinding {
        RustExecutableBinding {
            logical_id: logical_id.to_owned(),
            opaque_file_identity: format!("opaque:{logical_id}"),
            version: "1.96.0".to_owned(),
            sha256: Sha256Hash::digest(logical_id.as_bytes()),
            component_state: RustAvailabilityState::Available,
        }
    }

    fn binding() -> RustToolchainBinding {
        let cargo = executable("cargo");
        let rustc = executable("rustc");
        let rustfmt = executable("rustfmt");
        let clippy_driver = executable("clippy-driver");
        RustToolchainBinding {
            schema_id: "star.rust-toolchain-binding".to_owned(),
            schema_version: 1,
            contract_version: 1,
            workspace_root_ref: "project-root".to_owned(),
            manifest_refs: vec![RustSourceBinding {
                source_ref: "Cargo.toml".to_owned(),
                content_sha256: Sha256Hash::digest(b"manifest"),
            }],
            toolchain_source: RustToolchainSource::RustToolchainToml,
            toolchain_source_ref: "rust-toolchain.toml".to_owned(),
            toolchain_pin_state: RustToolchainPinState::PinnedStable,
            channel: "1.96.0".to_owned(),
            release: Some("1.96.0".to_owned()),
            host_triple: "x86_64-pc-windows-msvc".to_owned(),
            cargo,
            rustc,
            rustfmt,
            clippy_driver,
            parsing_editions: vec![RustEditionBinding {
                subject_ref: "fixture".to_owned(),
                edition: "2024".to_owned(),
                provenance: "Cargo.toml".to_owned(),
            }],
            style_editions: vec![RustEditionBinding {
                subject_ref: "fixture".to_owned(),
                edition: "2024".to_owned(),
                provenance: "cargo_edition_inferred".to_owned(),
            }],
            msrv_bindings: vec![RustEditionBinding {
                subject_ref: "fixture".to_owned(),
                edition: "1.96".to_owned(),
                provenance: "Cargo.toml".to_owned(),
            }],
            host_target: "x86_64-pc-windows-msvc".to_owned(),
            requested_target_triples: vec![
                "x86_64-pc-windows-msvc".to_owned(),
                "aarch64-pc-windows-msvc".to_owned(),
            ],
            config_bindings: vec![],
            component_states: vec![],
            target_states: vec![
                RustTargetState {
                    target_triple: "x86_64-pc-windows-msvc".to_owned(),
                    state: RustAvailabilityState::Available,
                },
                RustTargetState {
                    target_triple: "aarch64-pc-windows-msvc".to_owned(),
                    state: RustAvailabilityState::Available,
                },
            ],
            completeness: RustCompleteness::Complete,
            limitations: vec![],
            binding_fingerprint: Sha256Hash::digest(b"binding"),
        }
    }

    fn policy(project_id: ProjectId, auto_policy: RustAutoPolicy) -> RustStylePolicySnapshot {
        let binding = binding();
        RustStylePolicySnapshot {
            schema_id: "star.rust-style-policy-snapshot".to_owned(),
            schema_version: 1,
            contract_version: 1,
            profile_ref: "rust_style_auto_fix@1".to_owned(),
            profile_definition_hash: Sha256Hash::digest(b"profile"),
            pipeline_ref: "rust_style_v1@1".to_owned(),
            fixed_adapter_definition_fingerprint: Sha256Hash::digest(b"fixed-adapter"),
            formatting_sources: vec![],
            lint_level_sources: vec![],
            clippy_parameter_sources: vec![],
            clippy_fix_allowlist: vec![ClippyFixAllowlistEntry {
                lint_id: "clippy::needless_return".to_owned(),
                entry_version: "1.0.0".to_owned(),
                source: ClippyAllowlistSource::ProjectCatalog,
                source_ref: "catalog/rust-style.toml".to_owned(),
                clippy_release: "1.96.0".to_owned(),
                clippy_executable_sha256: binding.clippy_driver.sha256,
                required_applicability: SuggestionApplicability::MachineApplicable,
                allowed_scope: vec![ProjectPathRef::parse("src").unwrap()],
                public_api_policy: "deny".to_owned(),
                required_check_families: vec!["test_correctness".to_owned()],
                corpus_ref: "specs/corpus/rust-style/multicrate".to_owned(),
                lifecycle: RustCatalogLifecycle::Active,
                definition_fingerprint: Sha256Hash::digest(b"allowlist-entry"),
            }],
            coverage_policy_ref: "rust-style-coverage-v1".to_owned(),
            scope_project_id: project_id,
            scope_packages: vec!["app".to_owned(), "macros".to_owned()],
            scope_paths: vec![ProjectPathRef::parse("src").unwrap()],
            auto_policy,
            standing_grant_ref: (auto_policy == RustAutoPolicy::PersonalAuto)
                .then(|| "user-grant".to_owned()),
            max_files: 4,
            max_hunks: 8,
            max_changed_bytes: 4096,
            forbidden_operations: vec![
                "create".to_owned(),
                "delete".to_owned(),
                "rename".to_owned(),
                "generated_write".to_owned(),
                "vendor_write".to_owned(),
                "manifest_write".to_owned(),
            ],
            policy_completeness: RustCompleteness::Complete,
            limitations: vec![],
            policy_fingerprint: Sha256Hash::digest(b"policy"),
        }
    }

    fn coverage() -> RustStyleCoverageMatrix {
        let definitions = [
            (
                "app-lib-default",
                "app",
                RustTargetKind::Lib,
                "lib",
                "default",
                RustSourceOwnership::Handwritten,
                "x86_64-pc-windows-msvc",
            ),
            (
                "app-bin-feature",
                "app",
                RustTargetKind::Bin,
                "app",
                "cli",
                RustSourceOwnership::Handwritten,
                "x86_64-pc-windows-msvc",
            ),
            (
                "app-build",
                "app",
                RustTargetKind::CustomBuild,
                "build-script-build",
                "default",
                RustSourceOwnership::Handwritten,
                "x86_64-pc-windows-msvc",
            ),
            (
                "macro-proc",
                "macros",
                RustTargetKind::ProcMacro,
                "macros",
                "default",
                RustSourceOwnership::Handwritten,
                "x86_64-pc-windows-msvc",
            ),
            (
                "arm-cfg",
                "app",
                RustTargetKind::Lib,
                "lib",
                "arm",
                RustSourceOwnership::Handwritten,
                "aarch64-pc-windows-msvc",
            ),
            (
                "generated-observed",
                "app",
                RustTargetKind::Lib,
                "generated",
                "default",
                RustSourceOwnership::Generated,
                "x86_64-pc-windows-msvc",
            ),
            (
                "vendor-observed",
                "app",
                RustTargetKind::Lib,
                "vendor",
                "default",
                RustSourceOwnership::Vendor,
                "x86_64-pc-windows-msvc",
            ),
        ];
        let cells = definitions
            .into_iter()
            .map(
                |(cell_id, package_id, target_kind, target_name, feature, ownership, triple)| {
                    RustStyleCoverageCell {
                        cell_id: cell_id.to_owned(),
                        workspace_ref: "workspace".to_owned(),
                        package_id: package_id.to_owned(),
                        manifest_sha256: Sha256Hash::digest(package_id.as_bytes()),
                        target_kind,
                        target_name: target_name.to_owned(),
                        source_root: ProjectPathRef::parse("src").unwrap(),
                        feature_set_id: feature.to_owned(),
                        default_features: feature == "default",
                        features: (feature != "default")
                            .then(|| feature.to_owned())
                            .into_iter()
                            .collect(),
                        required_features_satisfied: true,
                        host_triple: "x86_64-pc-windows-msvc".to_owned(),
                        target_triple: triple.to_owned(),
                        cfg_observation_ref: format!("cfg:{triple}"),
                        ownership,
                        phase: RustCoveragePhase::CandidateFinalCheck,
                        execution: RustCoverageExecution::Executed,
                        reason: None,
                    }
                },
            )
            .collect::<Vec<_>>();
        RustStyleCoverageMatrix {
            schema_id: "star.rust-style-coverage-matrix".to_owned(),
            schema_version: 1,
            contract_version: 1,
            policy_ref: "rust-style-coverage-v1".to_owned(),
            required_cell_ids: cells.iter().map(|cell| cell.cell_id.clone()).collect(),
            cells,
            cfg_frontier: vec![],
            conflicts: vec![],
            completeness: RustCompleteness::Complete,
            limitations: vec![],
            coverage_fingerprint: Sha256Hash::digest(b"coverage"),
        }
    }

    #[test]
    fn fixed_pipeline_builds_patch_and_second_run_is_zero_diff() {
        let project_id = ProjectId::new();
        let binding = binding();
        let policy = policy(project_id.clone(), RustAutoPolicy::SafeDefault);
        let coverage = coverage();
        let mut preview = MemoryAdapter::new(initial_files());
        let mut replay = MemoryAdapter::new(initial_files());
        let candidate = prepare_rust_style_candidate(
            RustStyleCandidateInput {
                project_id: project_id.clone(),
                base_workspace_snapshot_id: WorkspaceSnapshotId::new(),
                scope: RustStylePatchScope::Workspace,
                binding: &binding,
                policy: &policy,
                coverage: &coverage,
            },
            &mut preview,
            &mut replay,
        )
        .unwrap();
        assert_eq!(candidate.state, RustCandidateState::Prepared);
        assert_eq!(candidate.changes.len(), 1);
        assert_eq!(candidate.selected_suggestions.len(), 1);
        assert_eq!(
            candidate
                .steps
                .iter()
                .map(|step| step.ordinal)
                .collect::<Vec<_>>(),
            [5, 6, 7, 11, 12]
        );
        assert!(candidate.patch_set.is_some());
        assert!(candidate.forward_artifact.is_some());
        assert!(candidate.reverse_artifact.is_some());
        let patch_binding = star_execution::rust_style::rust_style_patch_binding(
            candidate.forward_artifact.as_ref().unwrap(),
        )
        .unwrap();
        assert_eq!(patch_binding.scope, RustStylePatchScope::Workspace);
        assert_eq!(patch_binding.auto_policy, RustAutoPolicy::SafeDefault);

        let final_files = preview.snapshot().unwrap();
        let mut second_preview = MemoryAdapter::new(final_files.clone());
        let mut second_replay = MemoryAdapter::new(final_files);
        let second = prepare_rust_style_candidate(
            RustStyleCandidateInput {
                project_id,
                base_workspace_snapshot_id: WorkspaceSnapshotId::new(),
                scope: RustStylePatchScope::Workspace,
                binding: &binding,
                policy: &policy,
                coverage: &coverage,
            },
            &mut second_preview,
            &mut second_replay,
        )
        .unwrap();
        assert_eq!(second.state, RustCandidateState::SucceededNoChange);
        assert!(second.changes.is_empty());
        assert!(second.patch_set.is_none());
    }

    #[test]
    fn generated_or_vendor_write_blocks_entire_candidate() {
        let project_id = ProjectId::new();
        let mut preview = MemoryAdapter::new(initial_files());
        preview.inject_generated_write = true;
        let mut replay = MemoryAdapter::new(initial_files());
        let binding = binding();
        let policy = policy(project_id.clone(), RustAutoPolicy::SafeDefault);
        let coverage = coverage();
        let result = prepare_rust_style_candidate(
            RustStyleCandidateInput {
                project_id,
                base_workspace_snapshot_id: WorkspaceSnapshotId::new(),
                scope: RustStylePatchScope::Workspace,
                binding: &binding,
                policy: &policy,
                coverage: &coverage,
            },
            &mut preview,
            &mut replay,
        );
        assert!(matches!(
            result,
            Err(RustStyleWorkflowError::Validation(
                RustStyleValidationError::SideEffectViolation
            ))
        ));
    }

    #[test]
    fn exact_approval_personal_auto_and_recovery_are_fail_closed() {
        let project_id = ProjectId::new();
        let binding = binding();
        let personal_policy = policy(project_id.clone(), RustAutoPolicy::PersonalAuto);
        let coverage = coverage();
        let mut preview = MemoryAdapter::new(initial_files());
        let mut replay = MemoryAdapter::new(initial_files());
        let candidate = prepare_rust_style_candidate(
            RustStyleCandidateInput {
                project_id: project_id.clone(),
                base_workspace_snapshot_id: WorkspaceSnapshotId::new(),
                scope: RustStylePatchScope::Workspace,
                binding: &binding,
                policy: &personal_policy,
                coverage: &coverage,
            },
            &mut preview,
            &mut replay,
        )
        .unwrap();
        assert!(
            authorize_exact_human(
                &candidate,
                &Sha256Hash::digest(b"wrong"),
                PreApplyGateVerdict::AutoPass
            )
            .is_err()
        );
        let mut human_permit = authorize_exact_human(
            &candidate,
            &candidate.candidate_fingerprint,
            PreApplyGateVerdict::AutoPass,
        )
        .unwrap();
        struct Port(SourceMutationObservation);
        impl RustSourceMutationPort for Port {
            fn apply_exact(
                &mut self,
                _candidate: &RustStyleCandidate,
            ) -> SourceMutationObservation {
                self.0
            }
        }
        let mut recovery_port = Port(SourceMutationObservation::Applied {
            post_gate_auto_pass: false,
            evidence_complete: true,
        });
        assert_eq!(
            apply_with_permit(&candidate, &mut human_permit, &mut recovery_port).unwrap(),
            RustApplyState::RecoveryRequired
        );
        assert!(apply_with_permit(&candidate, &mut human_permit, &mut recovery_port).is_err());

        let expires_at = (Utc::now() + Duration::hours(1)).to_rfc3339();
        let mut grant = RustAutoApplyGrant {
            project_id,
            profile_ref: personal_policy.profile_ref.clone(),
            pipeline_ref: personal_policy.pipeline_ref.clone(),
            toolchain_fingerprint: candidate.toolchain_fingerprint.clone(),
            style_policy_fingerprint: candidate.policy_fingerprint.clone(),
            coverage_fingerprint: candidate.coverage_fingerprint.clone(),
            scope_paths: personal_policy.scope_paths.clone(),
            max_files: personal_policy.max_files,
            max_changed_bytes: personal_policy.max_changed_bytes,
            expires_at,
            grant_fingerprint: Sha256Hash::digest(b"unsealed-grant"),
        };
        grant.grant_fingerprint = versioned_fingerprint(
            "star.rust-style-auto-grant",
            1,
            &serde_json::json!({
                "project_id":grant.project_id,
                "profile_ref":grant.profile_ref,
                "pipeline_ref":grant.pipeline_ref,
                "toolchain_fingerprint":grant.toolchain_fingerprint,
                "style_policy_fingerprint":grant.style_policy_fingerprint,
                "coverage_fingerprint":grant.coverage_fingerprint,
                "scope_paths":grant.scope_paths,
                "max_files":grant.max_files,
                "max_changed_bytes":grant.max_changed_bytes,
                "expires_at":grant.expires_at,
            }),
        )
        .unwrap();
        let approval_request = prepare_personal_auto_approval_request(
            &candidate,
            &personal_policy,
            &grant,
            PreApplyGateVerdict::AutoPass,
            star_contracts::ids::GateId::new(),
            1,
            Sha256Hash::digest(b"pre-gate"),
            Utc::now(),
        )
        .unwrap();
        let approval_decision =
            seal_rust_style_policy_approval_decision(RustStylePolicyApprovalDecision {
                schema_id: RUST_STYLE_POLICY_APPROVAL_DECISION_SCHEMA_ID.to_owned(),
                schema_version: 1,
                contract_version: 1,
                approval_id: star_contracts::ids::ApprovalId::new(),
                scope_hash: Sha256Hash::digest(b"approval-scope"),
                request_fingerprint: approval_request.request_fingerprint.clone(),
                decision: ApprovalDecision::Approve,
                resolved_at: Utc::now().to_rfc3339(),
                decision_fingerprint: Sha256Hash::digest(b"pending-decision"),
            })
            .unwrap();
        let permit = authorize_personal_auto(PersonalAutoAuthorization {
            candidate: &candidate,
            policy: &personal_policy,
            grant: &grant,
            approved_patch_set_id: &approval_request.patch_set_id,
            approved_patch_fingerprint: &approval_request.patch_fingerprint,
            approval_request: &approval_request,
            approval_decision: &approval_decision,
            pre_gate: PreApplyGateVerdict::AutoPass,
            now: Utc::now(),
        })
        .unwrap();
        assert!(permit.automatic);
        let mut denied = approval_decision.clone();
        denied.decision = ApprovalDecision::Deny;
        denied = seal_rust_style_policy_approval_decision(denied).unwrap();
        assert!(
            authorize_personal_auto(PersonalAutoAuthorization {
                candidate: &candidate,
                policy: &personal_policy,
                grant: &grant,
                approved_patch_set_id: &approval_request.patch_set_id,
                approved_patch_fingerprint: &approval_request.patch_fingerprint,
                approval_request: &approval_request,
                approval_decision: &denied,
                pre_gate: PreApplyGateVerdict::AutoPass,
                now: Utc::now(),
            })
            .is_err()
        );
        let safe_policy = policy(candidate.project_id.clone(), RustAutoPolicy::SafeDefault);
        assert!(
            authorize_personal_auto(PersonalAutoAuthorization {
                candidate: &candidate,
                policy: &safe_policy,
                grant: &grant,
                approved_patch_set_id: &approval_request.patch_set_id,
                approved_patch_fingerprint: &approval_request.patch_fingerprint,
                approval_request: &approval_request,
                approval_decision: &approval_decision,
                pre_gate: PreApplyGateVerdict::AutoPass,
                now: Utc::now(),
            })
            .is_err()
        );
    }

    fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }
}
