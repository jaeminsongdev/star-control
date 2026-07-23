//! Deterministic M3 CheckGraph execution and authoritative evidence writer.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use star_contracts::{
    Sha256Hash, canonical_sha256,
    evidence::{
        ActorRef, ArtifactManifest, CatalogRef, Completeness, DiagnosticConfidence,
        DiagnosticSeverity, DiagnosticStatus, DocumentRef, GateDecisionKind, GateScope,
        ObservedTool, OutputLimits, RiskRef, TerminationReason, ValidationOutcome,
    },
    evidence_v2::{
        BaselineRelationV2, BaselineV2, CheckRequirementV2, ClaimEvaluationStatusV2,
        ClaimEvaluationV2, ClaimEvidenceRefV2, ClaimGateEffectV2, CompletionAssertionV2,
        CompletionClaimKindV2, CompletionClaimSubjectV2, CompletionClaimV2,
        DIAGNOSTIC_V2_SCHEMA_ID, DecisionDocumentRefV2, DiagnosticEvaluationSubjectV2,
        DiagnosticEvaluationV2, DiagnosticGateEffectV2, DiagnosticV2, DispositionV2,
        EVIDENCE_BUNDLE_V2_SCHEMA_ID, EVIDENCE_V2_SCHEMA_VERSION, EvidenceBundleV2,
        EvidenceFreshnessV2, EvidenceSubjectBinding, EvidenceV2Error, GATE_DECISION_V2_SCHEMA_ID,
        GateDecisionV2, InvocationWorkingDirectoryV2, ProcessStartStateV2, REVIEW_PACK_SCHEMA_ID,
        REVIEW_PACK_SCHEMA_VERSION, REVIEW_PACK_SECTION_ORDER, REWORK_DIRECTIVE_SCHEMA_ID,
        REWORK_DIRECTIVE_SCHEMA_VERSION, ReviewPackItemV1, ReviewPackSectionV1, ReviewPackV1,
        ReviewQuestionV1, ReworkDirectiveV1, RunGateEffectV2, RunSatisfactionStateV2,
        RunSatisfactionV2, SuppressionStateV2, SuppressionV2, TASK_INVOCATION_V2_SCHEMA_ID,
        TaskInvocationV2, VALIDATION_RESULT_V2_SCHEMA_ID, VALIDATION_RUN_V2_SCHEMA_ID,
        ValidationResultV2, ValidationRunV2, ValidationStabilityV2, empty_fingerprint,
    },
    ids::{
        DiagnosticId, EvidenceBundleId, GateId, ReviewPackId, ReworkDirectiveId, TaskInvocationId,
        ValidationResultId, ValidationRunId,
    },
    management::{DispositionDecision, SuppressionStatus},
    planning::{
        ChangeSet, CheckPlanV2, CollectionState, FullValidationPlan, ValidationPlanV2Readiness,
    },
};
use thiserror::Error;

use crate::rules::{RuleDecisionFloorV2, RuleDiagnosticInputV2, RuleFamilyV2};

#[derive(Clone, Debug)]
pub struct ExecutableBinding {
    pub check_id: String,
    pub check_ref: CatalogRef,
    pub tool_ref: CatalogRef,
    pub logical_executable: String,
    pub executable_binding_fingerprint: Sha256Hash,
    pub cwd: InvocationWorkingDirectoryV2,
    pub permission_action: String,
    pub output_limits: OutputLimits,
    pub subject_binding: EvidenceSubjectBinding,
}

#[derive(Clone, Debug)]
pub struct RawDiagnostic {
    pub code: String,
    pub title: String,
    pub message: String,
    pub severity: DiagnosticSeverity,
    pub confidence: DiagnosticConfidence,
    pub status: DiagnosticStatus,
    pub blocking: bool,
}

#[derive(Clone, Debug)]
pub struct CheckExecutionObservation {
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub exit_code: Option<i32>,
    pub termination_reason: TerminationReason,
    pub completeness: Completeness,
    pub stability: ValidationStabilityV2,
    pub artifact_refs: Vec<star_contracts::evidence::ArtifactRef>,
    pub observed_tool: Option<ObservedTool>,
    pub diagnostics: Vec<RawDiagnostic>,
}

#[derive(Clone, Debug, Error)]
#[error("check executor failed: {code}")]
pub struct CheckExecutorError {
    pub code: String,
    pub message: String,
    pub termination_reason: TerminationReason,
}

pub trait CheckExecutor {
    fn execute(
        &mut self,
        invocation: &TaskInvocationV2,
    ) -> Result<CheckExecutionObservation, CheckExecutorError>;
}

#[derive(Clone, Debug, Error)]
#[error("validation artifact manifest finalization failed: {code}")]
pub struct ArtifactManifestFinalizationError {
    pub code: String,
}

pub trait ArtifactManifestFinalizer {
    fn finalize(
        &mut self,
        validation_plan_ref: &DocumentRef,
        runs: &[ValidationRunV2],
        diagnostics: &[DiagnosticV2],
    ) -> Result<ArtifactManifest, ArtifactManifestFinalizationError>;
}

#[derive(Clone, Debug)]
pub struct CheckGraphRunContext {
    pub gate_scope: GateScope,
    pub decided_by: ActorRef,
    pub artifact_manifest: ArtifactManifest,
    pub force_human_review: bool,
    pub baselines: Vec<BaselineV2>,
    pub suppressions: Vec<SuppressionV2>,
    pub dispositions: Vec<DispositionV2>,
    pub evaluation_time: DateTime<Utc>,
    pub max_attempts_per_check: u32,
    pub preflight_diagnostics: Vec<RuleDiagnosticInputV2>,
    pub completion_claims: Vec<CompletionClaimV2>,
    pub change_sets: Vec<ChangeSet>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckGraphRunResult {
    pub validation_runs: Vec<ValidationRunV2>,
    pub diagnostics: Vec<DiagnosticV2>,
    pub validation_results: Vec<ValidationResultV2>,
    pub gate_decision: GateDecisionV2,
    pub evidence_bundle: EvidenceBundleV2,
    pub review_pack: ReviewPackV1,
    pub rework_directive: Option<ReworkDirectiveV1>,
}

#[derive(Debug, Error)]
pub enum CheckGraphRunnerError {
    #[error("validation plan is not executable")]
    PlanNotReady,
    #[error("check graph is invalid or cyclic")]
    Graph,
    #[error("an executable binding is absent or conflicts with the plan")]
    Binding,
    #[error("M3 evidence contract could not be sealed")]
    Evidence(#[from] EvidenceV2Error),
    #[error("canonical fingerprint could not be calculated")]
    Fingerprint,
    #[error("attempt policy is outside the supported bounded range")]
    AttemptPolicy,
    #[error("validation artifact manifest could not be finalized")]
    ArtifactManifest(#[from] ArtifactManifestFinalizationError),
}

pub fn run_check_graph(
    plan: &FullValidationPlan,
    bindings: &[ExecutableBinding],
    context: CheckGraphRunContext,
    executor: &mut dyn CheckExecutor,
) -> Result<CheckGraphRunResult, CheckGraphRunnerError> {
    run_check_graph_inner(plan, bindings, context, executor, None)
}

pub fn run_check_graph_with_artifact_finalizer(
    plan: &FullValidationPlan,
    bindings: &[ExecutableBinding],
    context: CheckGraphRunContext,
    executor: &mut dyn CheckExecutor,
    artifact_finalizer: &mut dyn ArtifactManifestFinalizer,
) -> Result<CheckGraphRunResult, CheckGraphRunnerError> {
    run_check_graph_inner(plan, bindings, context, executor, Some(artifact_finalizer))
}

fn run_check_graph_inner(
    plan: &FullValidationPlan,
    bindings: &[ExecutableBinding],
    context: CheckGraphRunContext,
    executor: &mut dyn CheckExecutor,
    mut artifact_finalizer: Option<&mut dyn ArtifactManifestFinalizer>,
) -> Result<CheckGraphRunResult, CheckGraphRunnerError> {
    if plan.readiness != ValidationPlanV2Readiness::Ready
        || plan.required_checks.is_empty()
        || !plan.unresolved_checks.is_empty()
    {
        return Err(CheckGraphRunnerError::PlanNotReady);
    }
    if !(1..=3).contains(&context.max_attempts_per_check) {
        return Err(CheckGraphRunnerError::AttemptPolicy);
    }
    let plan_ref = star_contracts::evidence::DocumentRef {
        schema_id: star_contracts::planning::FULL_VALIDATION_PLAN_SCHEMA_ID.to_owned(),
        document_id: plan.validation_plan_id.to_string(),
        revision: plan.revision,
        sha256: document_hash(plan)?,
    };
    let binding_map = bindings
        .iter()
        .map(|binding| (binding.check_id.as_str(), binding))
        .collect::<BTreeMap<_, _>>();
    if binding_map.len() != bindings.len() {
        return Err(CheckGraphRunnerError::Binding);
    }
    let checks = plan
        .required_checks
        .iter()
        .map(|check| (check.plan_item_id.as_str(), check))
        .collect::<BTreeMap<_, _>>();
    if checks.len() != plan.required_checks.len()
        || plan
            .check_graph
            .nodes
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != checks.keys().copied().collect::<BTreeSet<_>>()
    {
        return Err(CheckGraphRunnerError::Graph);
    }
    for check in &plan.required_checks {
        let binding = binding_map
            .get(check.check_id.as_str())
            .copied()
            .ok_or(CheckGraphRunnerError::Binding)?;
        let sealed_binding = binding.subject_binding.clone().seal()?;
        if binding.logical_executable != check.invocation.logical_executable
            || sealed_binding != binding.subject_binding
            || binding.subject_binding.validation_plan_ref != plan_ref
            || binding.subject_binding.project_id != check.project_id
            || binding.subject_binding.check_descriptor_ref.as_ref() != Some(&check.descriptor_ref)
            || binding.subject_binding.effective_config_fingerprint != plan.config_fingerprint
            || binding.subject_binding.catalog_snapshot_ref != plan.catalog_snapshot_ref
            || binding.subject_binding.tool_descriptor_ref.as_ref() != Some(&binding.tool_ref)
            || binding.subject_binding.freshness != EvidenceFreshnessV2::Current
        {
            return Err(CheckGraphRunnerError::Binding);
        }
    }
    let (order, predecessors) = topological_order(plan, &checks)?;
    let mut runs = Vec::with_capacity(order.len() * context.max_attempts_per_check as usize);
    let mut diagnostics = Vec::new();
    let mut satisfied_items = BTreeSet::new();
    let mut diagnostic_sequence = 0_u64;
    for plan_item_id in order {
        let check = checks[plan_item_id];
        let binding = binding_map[check.check_id.as_str()];
        let dependencies_satisfied = predecessors
            .get(plan_item_id)
            .is_none_or(|required| required.is_subset(&satisfied_items));
        if !dependencies_satisfied {
            let run_id = ValidationRunId::new();
            let invocation = invocation_for(check, binding, &plan_ref, 1)?.seal()?;
            diagnostic_sequence += 1;
            let diagnostic = diagnostic_for(
                diagnostic_sequence,
                "CHECK_DEPENDENCY_NOT_SATISFIED",
                "Required predecessor did not satisfy its check",
                "This CheckGraph node was not run because a required predecessor failed or was not run.",
                DiagnosticSeverity::Error,
                DiagnosticConfidence::High,
                DiagnosticStatus::Confirmed,
                true,
                check,
                &run_id,
                &binding.check_ref,
                vec![],
            )?;
            let diagnostic_id = diagnostic.diagnostic_id.clone();
            diagnostics.push(diagnostic);
            runs.push(
                ValidationRunV2 {
                    schema_id: VALIDATION_RUN_V2_SCHEMA_ID.to_owned(),
                    schema_version: 2,
                    validation_run_id: run_id,
                    revision: 1,
                    validation_plan_ref: plan_ref.clone(),
                    check_ref: check.descriptor_ref.clone(),
                    subject_binding: binding.subject_binding.clone(),
                    plan_item_id: check.plan_item_id.clone(),
                    project_id: check.project_id.clone(),
                    phase: plan.phase.clone(),
                    attempt: 1,
                    invocation,
                    process_start_state: ProcessStartStateV2::NotStarted,
                    not_run_reason: Some(
                        star_contracts::evidence_v2::NotRunReasonV2::DependencyUnsatisfied,
                    ),
                    started_at: None,
                    finished_at: None,
                    outcome: ValidationOutcome::NotRun,
                    completeness: Completeness::Unverified,
                    stability: ValidationStabilityV2::NotEvaluated,
                    exit_code: None,
                    termination_reason: None,
                    diagnostic_ids: vec![diagnostic_id],
                    artifact_refs: vec![],
                    observed_tool: None,
                    result_fingerprint: empty_fingerprint(),
                }
                .seal()?,
            );
            continue;
        }
        let mut attempt_runs = Vec::new();
        for attempt in 1..=context.max_attempts_per_check {
            let run = execute_check_attempt(
                plan,
                &plan_ref,
                check,
                binding,
                attempt,
                &mut diagnostic_sequence,
                &mut diagnostics,
                executor,
            )?;
            let retry = attempt < context.max_attempts_per_check && retryable_attempt(&run);
            attempt_runs.push(run);
            if !retry {
                break;
            }
        }
        if attempt_runs.len() == 1 && attempt_runs[0].satisfies_required_check() {
            satisfied_items.insert(check.plan_item_id.clone());
        }
        runs.extend(attempt_runs);
    }
    runs.sort_by(|left, right| {
        (&left.plan_item_id, left.attempt).cmp(&(&right.plan_item_id, right.attempt))
    });
    for preflight in &context.preflight_diagnostics {
        let check = preflight_check(plan, preflight.family).ok_or(CheckGraphRunnerError::Graph)?;
        let run = runs
            .iter()
            .filter(|run| run.plan_item_id == check.plan_item_id)
            .max_by_key(|run| run.attempt)
            .or_else(|| runs.first())
            .ok_or(CheckGraphRunnerError::Graph)?;
        diagnostic_sequence += 1;
        diagnostics.push(diagnostic_for(
            diagnostic_sequence,
            &preflight.code,
            &preflight.title,
            &preflight.message,
            preflight.severity,
            preflight.confidence,
            preflight.status,
            preflight.decision_floor == RuleDecisionFloorV2::Block,
            check,
            &run.validation_run_id,
            &CatalogRef {
                catalog_id: preflight.rule_id.clone(),
                format_version: 2,
                item_version: "2.0.0".to_owned(),
                sha256: Sha256Hash::digest(preflight.rule_id.as_bytes()),
            },
            vec![],
        )?);
    }
    let mut completion_claims = context
        .completion_claims
        .into_iter()
        .map(CompletionClaimV2::seal)
        .collect::<Result<Vec<_>, _>>()?;
    completion_claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    if completion_claims
        .windows(2)
        .any(|pair| pair[0].claim_id == pair[1].claim_id)
    {
        return Err(CheckGraphRunnerError::Evidence(
            EvidenceV2Error::CompletionClaim,
        ));
    }
    let claim_evaluations = evaluate_completion_claims(
        plan,
        &completion_claims,
        &context.change_sets,
        &runs,
        &mut diagnostic_sequence,
        &mut diagnostics,
    )?;
    diagnostics.sort_by_key(|diagnostic| diagnostic.sequence);
    let diagnostic_evaluations = evaluate_diagnostics(
        &runs,
        &diagnostics,
        &context.baselines,
        &context.suppressions,
        &context.dispositions,
        context.evaluation_time,
    )?;
    let run_satisfactions =
        evaluate_run_satisfactions(plan, &runs, &diagnostics, &diagnostic_evaluations)?;
    let validation_results =
        build_validation_results(plan, &runs, &run_satisfactions, context.evaluation_time)?;
    let required_run_refs = runs
        .iter()
        .map(ValidationRunV2::reference)
        .collect::<Result<Vec<_>, _>>()?;
    let satisfied_run_refs = runs
        .iter()
        .filter(|run| run.satisfies_required_check())
        .map(ValidationRunV2::reference)
        .collect::<Result<Vec<_>, _>>()?;
    let mut blocking_diagnostic_refs = diagnostic_evaluations
        .iter()
        .filter(|evaluation| evaluation.gate_effect == DiagnosticGateEffectV2::Blocks)
        .filter_map(|evaluation| match &evaluation.evaluation_subject {
            DiagnosticEvaluationSubjectV2::CurrentDiagnostic { diagnostic_ref } => {
                Some(diagnostic_ref.clone())
            }
            DiagnosticEvaluationSubjectV2::BaselineEntry { .. } => None,
        })
        .collect::<Vec<_>>();
    blocking_diagnostic_refs.extend(
        claim_evaluations
            .iter()
            .filter(|evaluation| evaluation.gate_effect == ClaimGateEffectV2::Block)
            .flat_map(|evaluation| evaluation.diagnostic_refs.iter().cloned()),
    );
    blocking_diagnostic_refs.sort();
    blocking_diagnostic_refs.dedup();
    let effects = run_satisfactions
        .iter()
        .map(|satisfaction| satisfaction.gate_effect)
        .collect::<BTreeSet<_>>();
    let claim_effects = claim_evaluations
        .iter()
        .map(|evaluation| evaluation.gate_effect)
        .collect::<BTreeSet<_>>();
    let rule_review_required = context
        .preflight_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.decision_floor == RuleDecisionFloorV2::HumanReview);
    let decision = if effects.contains(&RunGateEffectV2::Block)
        || claim_effects.contains(&ClaimGateEffectV2::Block)
        || !blocking_diagnostic_refs.is_empty()
    {
        GateDecisionKind::Block
    } else if effects.contains(&RunGateEffectV2::HumanReview)
        || claim_effects.contains(&ClaimGateEffectV2::HumanReview)
        || context.force_human_review
        || rule_review_required
        || plan.independent_review.required
    {
        GateDecisionKind::HumanReview
    } else {
        GateDecisionKind::AutoPass
    };
    let reason_codes = match decision {
        GateDecisionKind::AutoPass => vec!["ALL_REQUIRED_CHECKS_COMPLETE_STABLE_PASS".to_owned()],
        GateDecisionKind::HumanReview
            if claim_effects.contains(&ClaimGateEffectV2::HumanReview) =>
        {
            vec!["REQUIRED_COMPLETION_CLAIM_UNVERIFIED".to_owned()]
        }
        GateDecisionKind::HumanReview if effects.contains(&RunGateEffectV2::HumanReview) => {
            vec!["RUN_REQUIRES_HUMAN_REVIEW".to_owned()]
        }
        GateDecisionKind::HumanReview if rule_review_required => {
            vec!["RULE_FAMILY_REQUIRES_HUMAN_REVIEW".to_owned()]
        }
        GateDecisionKind::HumanReview => vec!["INDEPENDENT_REVIEW_REQUIRED".to_owned()],
        GateDecisionKind::Block if claim_effects.contains(&ClaimGateEffectV2::Block) => {
            vec!["REQUIRED_COMPLETION_CLAIM_CONTRADICTED_OR_STALE".to_owned()]
        }
        GateDecisionKind::Block => vec!["REQUIRED_CHECK_NOT_SATISFIED".to_owned()],
    };
    let binding_set = validation_results
        .iter()
        .map(|result| result.subject_binding.binding_fingerprint.clone())
        .collect::<BTreeSet<_>>();
    let subject_binding_set_fingerprint = canonical_sha256(&serde_json::json!({
        "domain":"star.evidence-subject-binding-set",
        "version":2,
        "value":binding_set,
    }))
    .map_err(|_| CheckGraphRunnerError::Fingerprint)?;
    let gate = GateDecisionV2 {
        schema_id: GATE_DECISION_V2_SCHEMA_ID.to_owned(),
        schema_version: 2,
        gate_id: GateId::new(),
        revision: 1,
        validation_plan_ref: plan_ref.clone(),
        subject_binding_set_fingerprint: subject_binding_set_fingerprint.clone(),
        scope: context.gate_scope,
        decision,
        validation_result_refs: validation_results
            .iter()
            .map(ValidationResultV2::reference)
            .collect::<Result<Vec<_>, _>>()?,
        required_run_refs,
        satisfied_run_refs,
        diagnostic_evaluations: diagnostic_evaluations.clone(),
        claim_evaluations: claim_evaluations.clone(),
        run_satisfactions: run_satisfactions.clone(),
        blocking_diagnostic_refs,
        reason_codes,
        remaining_risks: risk_refs(plan),
        policy_fingerprint: canonical_sha256(&serde_json::json!({
            "config":plan.config_fingerprint,
            "gate_policy":plan.gate_policy,
        }))
        .map_err(|_| CheckGraphRunnerError::Fingerprint)?,
        decided_by: context.decided_by,
        decided_at: context.evaluation_time,
        valid_until: context
            .suppressions
            .iter()
            .filter(|suppression| suppression.status == SuppressionStatus::Active)
            .filter_map(|suppression| suppression.expires_at)
            .filter(|expires| *expires > context.evaluation_time)
            .min(),
        decision_fingerprint: empty_fingerprint(),
    }
    .seal(&runs, &diagnostics, &validation_results)?;
    let complete_execution = runs
        .iter()
        .all(|run| run.completeness == Completeness::Complete);
    let missing_reasons = if complete_execution {
        vec![]
    } else {
        vec!["VALIDATION_EVIDENCE_INCOMPLETE".to_owned()]
    };
    let artifact_manifest = if let Some(finalizer) = artifact_finalizer.as_mut() {
        finalizer.finalize(&plan_ref, &runs, &diagnostics)?
    } else {
        context.artifact_manifest
    };
    let bundle = EvidenceBundleV2 {
        schema_id: EVIDENCE_BUNDLE_V2_SCHEMA_ID.to_owned(),
        schema_version: 2,
        evidence_bundle_id: EvidenceBundleId::new(),
        revision: 1,
        task_spec_ref: plan.task_spec_ref.clone(),
        scope_revision_ref: plan.scope_revision_ref.clone(),
        impact_analysis_ref: plan.impact_analysis_ref.clone(),
        validation_plan_ref: plan_ref.clone(),
        subject_binding_set_fingerprint,
        validation_run_refs: runs
            .iter()
            .map(ValidationRunV2::reference)
            .collect::<Result<Vec<_>, _>>()?,
        validation_result_refs: validation_results
            .iter()
            .map(ValidationResultV2::reference)
            .collect::<Result<Vec<_>, _>>()?,
        diagnostic_refs: diagnostics
            .iter()
            .map(DiagnosticV2::reference)
            .collect::<Result<Vec<_>, _>>()?,
        completion_claims,
        claim_evaluations,
        gate_decision_ref: gate.reference()?,
        authoritative_gate_state: gate.authoritative_state(),
        remaining_risks: risk_refs(plan),
        artifact_manifest,
        completeness: if complete_execution {
            Completeness::Complete
        } else {
            Completeness::Partial
        },
        missing_reasons,
        created_at: Utc::now(),
        bundle_fingerprint: empty_fingerprint(),
    }
    .seal(&runs, &validation_results, &diagnostics, &gate)?;
    let rework_directive = build_rework_directive(&gate, &run_satisfactions, &diagnostics)?;
    let review_pack = build_review_pack(
        plan,
        &plan_ref,
        &runs,
        &diagnostics,
        &validation_results,
        &gate,
        &bundle,
        rework_directive.as_ref(),
        context.evaluation_time,
    )?;
    Ok(CheckGraphRunResult {
        validation_runs: runs,
        diagnostics,
        validation_results,
        gate_decision: gate,
        evidence_bundle: bundle,
        review_pack,
        rework_directive,
    })
}

#[allow(clippy::too_many_arguments)]
fn execute_check_attempt(
    plan: &FullValidationPlan,
    plan_ref: &DocumentRef,
    check: &CheckPlanV2,
    binding: &ExecutableBinding,
    attempt: u32,
    diagnostic_sequence: &mut u64,
    diagnostics: &mut Vec<DiagnosticV2>,
    executor: &mut dyn CheckExecutor,
) -> Result<ValidationRunV2, CheckGraphRunnerError> {
    let run_id = ValidationRunId::new();
    let invocation = invocation_for(check, binding, plan_ref, attempt)?.seal()?;
    let execution = executor.execute(&invocation);
    let mut diagnostic_ids = Vec::new();
    match execution {
        Ok(observation) => {
            if observation.finished_at < observation.started_at {
                return Err(CheckGraphRunnerError::Evidence(EvidenceV2Error::Run));
            }
            let expected_exit = observation
                .exit_code
                .is_some_and(|code| invocation.expected_exit_codes.contains(&code));
            let outcome = match observation.termination_reason {
                TerminationReason::Exited
                    if expected_exit
                        && observation.completeness == Completeness::Complete
                        && observation.stability == ValidationStabilityV2::Stable
                        && observation.observed_tool.is_some() =>
                {
                    ValidationOutcome::Pass
                }
                TerminationReason::Exited => ValidationOutcome::Fail,
                TerminationReason::Cancelled => ValidationOutcome::Cancelled,
                _ => ValidationOutcome::Error,
            };
            let mut raw_diagnostics = observation.diagnostics;
            if outcome != ValidationOutcome::Pass && raw_diagnostics.is_empty() {
                raw_diagnostics.push(RawDiagnostic {
                    code: match observation.termination_reason {
                        TerminationReason::Timeout => "CHECK_TIMEOUT",
                        TerminationReason::Cancelled => "CHECK_CANCELLED",
                        TerminationReason::LaunchError => "CHECK_LAUNCH_ERROR",
                        TerminationReason::OutcomeUnknown => "CHECK_OUTCOME_UNKNOWN",
                        TerminationReason::Exited => "CHECK_EXIT_CODE_FAILED",
                    }
                    .to_owned(),
                    title: "Required check did not pass".to_owned(),
                    message:
                        "The registered check did not produce a complete stable successful result."
                            .to_owned(),
                    severity: DiagnosticSeverity::Error,
                    confidence: DiagnosticConfidence::High,
                    status: DiagnosticStatus::Confirmed,
                    blocking: true,
                });
            }
            for raw in raw_diagnostics {
                *diagnostic_sequence += 1;
                let diagnostic = diagnostic_for(
                    *diagnostic_sequence,
                    &raw.code,
                    &raw.title,
                    &raw.message,
                    raw.severity,
                    raw.confidence,
                    raw.status,
                    raw.blocking,
                    check,
                    &run_id,
                    &binding.check_ref,
                    observation.artifact_refs.clone(),
                )?;
                diagnostic_ids.push(diagnostic.diagnostic_id.clone());
                diagnostics.push(diagnostic);
            }
            ValidationRunV2 {
                schema_id: VALIDATION_RUN_V2_SCHEMA_ID.to_owned(),
                schema_version: 2,
                validation_run_id: run_id,
                revision: 1,
                validation_plan_ref: plan_ref.clone(),
                check_ref: check.descriptor_ref.clone(),
                subject_binding: binding.subject_binding.clone(),
                plan_item_id: check.plan_item_id.clone(),
                project_id: check.project_id.clone(),
                phase: plan.phase.clone(),
                attempt,
                invocation,
                process_start_state: ProcessStartStateV2::Started,
                not_run_reason: None,
                started_at: Some(observation.started_at),
                finished_at: Some(observation.finished_at),
                outcome,
                completeness: observation.completeness,
                stability: observation.stability,
                exit_code: observation.exit_code,
                termination_reason: Some(observation.termination_reason),
                diagnostic_ids,
                artifact_refs: observation.artifact_refs,
                observed_tool: observation.observed_tool,
                result_fingerprint: empty_fingerprint(),
            }
            .seal()
            .map_err(CheckGraphRunnerError::from)
        }
        Err(error) => {
            *diagnostic_sequence += 1;
            let diagnostic = diagnostic_for(
                *diagnostic_sequence,
                &error.code,
                "Check executor could not produce verified evidence",
                &error.message,
                DiagnosticSeverity::Error,
                DiagnosticConfidence::High,
                DiagnosticStatus::Confirmed,
                true,
                check,
                &run_id,
                &binding.check_ref,
                vec![],
            )?;
            diagnostic_ids.push(diagnostic.diagnostic_id.clone());
            diagnostics.push(diagnostic);
            let not_started = error.termination_reason == TerminationReason::LaunchError;
            ValidationRunV2 {
                schema_id: VALIDATION_RUN_V2_SCHEMA_ID.to_owned(),
                schema_version: 2,
                validation_run_id: run_id,
                revision: 1,
                validation_plan_ref: plan_ref.clone(),
                check_ref: check.descriptor_ref.clone(),
                subject_binding: binding.subject_binding.clone(),
                plan_item_id: check.plan_item_id.clone(),
                project_id: check.project_id.clone(),
                phase: plan.phase.clone(),
                attempt,
                invocation,
                process_start_state: if not_started {
                    ProcessStartStateV2::NotStarted
                } else {
                    ProcessStartStateV2::Unknown
                },
                not_run_reason: not_started
                    .then_some(star_contracts::evidence_v2::NotRunReasonV2::LaunchError),
                started_at: None,
                finished_at: None,
                outcome: if not_started {
                    ValidationOutcome::NotRun
                } else {
                    ValidationOutcome::Error
                },
                completeness: Completeness::Unverified,
                stability: ValidationStabilityV2::NotEvaluated,
                exit_code: None,
                termination_reason: (!not_started).then_some(error.termination_reason),
                diagnostic_ids,
                artifact_refs: vec![],
                observed_tool: None,
                result_fingerprint: empty_fingerprint(),
            }
            .seal()
            .map_err(CheckGraphRunnerError::from)
        }
    }
}

fn retryable_attempt(run: &ValidationRunV2) -> bool {
    run.process_start_state == ProcessStartStateV2::Started
        && matches!(
            run.outcome,
            ValidationOutcome::Fail | ValidationOutcome::Error
        )
        && run.termination_reason != Some(TerminationReason::Cancelled)
}

fn preflight_check(plan: &FullValidationPlan, family: RuleFamilyV2) -> Option<&CheckPlanV2> {
    let preferred: &[&str] = match family {
        RuleFamilyV2::B01ChangeScopeClaim => &["project_full", "build"],
        RuleFamilyV2::B02TestTrust => &["test"],
        RuleFamilyV2::B03ValidatorSelfProtection => &["validator_guard", "contract", "test"],
        RuleFamilyV2::B04ArchitectureContractDrift => {
            &["architecture", "contract", "generation", "hardcoding"]
        }
        RuleFamilyV2::B05SecuritySupplyChain => &["security", "dependency", "project_full"],
        RuleFamilyV2::B06Regression => &["regression", "test"],
        RuleFamilyV2::B07DocsConfigEnvironment => &["docs", "config", "project_full"],
    };
    preferred
        .iter()
        .find_map(|family| {
            plan.required_checks
                .iter()
                .find(|check| check.family == *family)
        })
        .or_else(|| plan.required_checks.first())
}

fn evaluate_completion_claims(
    plan: &FullValidationPlan,
    claims: &[CompletionClaimV2],
    change_sets: &[ChangeSet],
    runs: &[ValidationRunV2],
    diagnostic_sequence: &mut u64,
    diagnostics: &mut Vec<DiagnosticV2>,
) -> Result<Vec<ClaimEvaluationV2>, CheckGraphRunnerError> {
    let mut evaluations = Vec::with_capacity(claims.len());
    for claim in claims {
        let project_id = claim.subject.project_id();
        let current_run = match &claim.subject {
            CompletionClaimSubjectV2::CheckPlan { plan_item_id, .. } => runs
                .iter()
                .filter(|run| &run.plan_item_id == plan_item_id)
                .max_by_key(|run| run.attempt),
            _ => runs
                .iter()
                .filter(|run| &run.project_id == project_id)
                .max_by_key(|run| (&run.plan_item_id, run.attempt)),
        }
        .ok_or(CheckGraphRunnerError::Binding)?;
        let check = plan
            .required_checks
            .iter()
            .find(|check| check.plan_item_id == current_run.plan_item_id)
            .ok_or(CheckGraphRunnerError::Binding)?;
        let stale = claim
            .reported_subject_binding
            .as_ref()
            .is_some_and(|reported| {
                reported.binding_fingerprint != current_run.subject_binding.binding_fingerprint
            });
        let (mut status, actual_evidence_refs, mut reason_codes) = if stale {
            (
                ClaimEvaluationStatusV2::Stale,
                vec![],
                vec!["CLAIM_REPORTED_BINDING_IS_NOT_CURRENT".to_owned()],
            )
        } else {
            evaluate_completion_claim_actual(plan, claim, change_sets, runs)?
        };
        if current_run.subject_binding.freshness != EvidenceFreshnessV2::Current {
            status = ClaimEvaluationStatusV2::Stale;
            reason_codes = vec!["CLAIM_CURRENT_BINDING_IS_STALE".to_owned()];
        }
        let gate_effect = match (claim.required, status) {
            (_, ClaimEvaluationStatusV2::Verified | ClaimEvaluationStatusV2::NotApplicable) => {
                ClaimGateEffectV2::None
            }
            (true, ClaimEvaluationStatusV2::Contradicted | ClaimEvaluationStatusV2::Stale) => {
                ClaimGateEffectV2::Block
            }
            (true, ClaimEvaluationStatusV2::Unverified) => ClaimGateEffectV2::HumanReview,
            (false, _) => ClaimGateEffectV2::None,
        };
        let mut diagnostic_refs = Vec::new();
        if !matches!(
            status,
            ClaimEvaluationStatusV2::Verified | ClaimEvaluationStatusV2::NotApplicable
        ) {
            let (code, title, message, severity) = match status {
                ClaimEvaluationStatusV2::Contradicted => (
                    "star.validation.claim.contradicted",
                    "Completion claim contradicts current evidence",
                    "The typed completion assertion does not match the current observed evidence.",
                    DiagnosticSeverity::Error,
                ),
                ClaimEvaluationStatusV2::Stale => (
                    "star.validation.claim.stale",
                    "Completion claim evidence is stale",
                    "The reported subject binding does not match the current validation subject.",
                    DiagnosticSeverity::Error,
                ),
                ClaimEvaluationStatusV2::Unverified => (
                    "star.validation.claim.unverified",
                    "Completion claim could not be verified",
                    "The current evidence is incomplete for this typed completion assertion.",
                    DiagnosticSeverity::Warning,
                ),
                ClaimEvaluationStatusV2::Verified | ClaimEvaluationStatusV2::NotApplicable => {
                    unreachable!("positive claims do not create diagnostics")
                }
            };
            *diagnostic_sequence += 1;
            let diagnostic = diagnostic_for(
                *diagnostic_sequence,
                code,
                title,
                message,
                severity,
                DiagnosticConfidence::High,
                DiagnosticStatus::Confirmed,
                gate_effect == ClaimGateEffectV2::Block,
                check,
                &current_run.validation_run_id,
                &CatalogRef {
                    catalog_id: code.to_owned(),
                    format_version: 2,
                    item_version: "2.0.0".to_owned(),
                    sha256: Sha256Hash::digest(code.as_bytes()),
                },
                vec![],
            )?;
            diagnostic_refs.push(diagnostic.reference()?);
            diagnostics.push(diagnostic);
        }
        evaluations.push(
            ClaimEvaluationV2 {
                claim_ref: claim.reference(),
                current_subject_binding: current_run.subject_binding.clone(),
                actual_evidence_refs,
                status,
                diagnostic_refs,
                gate_effect,
                reason_codes,
                evaluation_fingerprint: empty_fingerprint(),
            }
            .seal()?,
        );
    }
    evaluations.sort_by(|left, right| left.claim_ref.claim_id.cmp(&right.claim_ref.claim_id));
    Ok(evaluations)
}

fn evaluate_completion_claim_actual(
    plan: &FullValidationPlan,
    claim: &CompletionClaimV2,
    change_sets: &[ChangeSet],
    runs: &[ValidationRunV2],
) -> Result<
    (
        ClaimEvaluationStatusV2,
        Vec<ClaimEvidenceRefV2>,
        Vec<String>,
    ),
    CheckGraphRunnerError,
> {
    match (&claim.kind, &claim.subject, &claim.assertion) {
        (
            CompletionClaimKindV2::Change,
            CompletionClaimSubjectV2::Path { project_id, path },
            CompletionAssertionV2::Change {
                operation,
                after_sha256,
            },
        ) => {
            let project_change_sets = change_sets
                .iter()
                .filter(|change_set| &change_set.project_id == project_id)
                .collect::<Vec<_>>();
            if project_change_sets.is_empty()
                || project_change_sets
                    .iter()
                    .any(|change_set| change_set.collection_state != CollectionState::Complete)
            {
                return Ok((
                    ClaimEvaluationStatusV2::Unverified,
                    vec![],
                    vec!["CLAIM_CHANGE_COLLECTION_INCOMPLETE".to_owned()],
                ));
            }
            let observed = project_change_sets.iter().find_map(|change_set| {
                change_set
                    .entries
                    .iter()
                    .find(|entry| &entry.path == path)
                    .map(|entry| (*change_set, entry))
            });
            let actual_refs = observed
                .and_then(|(change_set, _)| {
                    plan.change_set_refs
                        .iter()
                        .find(|reference| {
                            reference.document_id == change_set.change_set_id.as_str()
                        })
                        .cloned()
                })
                .map(|document_ref| vec![ClaimEvidenceRefV2::Document { document_ref }])
                .unwrap_or_default();
            let verified = observed.is_some_and(|(_, entry)| {
                &entry.change_kind == operation
                    && after_sha256
                        .as_ref()
                        .is_none_or(|expected| entry.after_sha256.as_ref() == Some(expected))
            });
            Ok(if verified {
                (
                    ClaimEvaluationStatusV2::Verified,
                    actual_refs,
                    vec!["CLAIM_CHANGE_MATCHES_CURRENT_CHANGE_SET".to_owned()],
                )
            } else {
                (
                    ClaimEvaluationStatusV2::Contradicted,
                    actual_refs,
                    vec!["CLAIM_CHANGE_DOES_NOT_MATCH_CURRENT_CHANGE_SET".to_owned()],
                )
            })
        }
        (
            CompletionClaimKindV2::CheckExecuted,
            CompletionClaimSubjectV2::CheckPlan { plan_item_id, .. },
            CompletionAssertionV2::Pass,
        ) => evaluate_claim_runs(
            runs.iter().filter(|run| &run.plan_item_id == plan_item_id),
            "CLAIM_CHECK_CURRENT_COMPLETE_STABLE_PASS",
        ),
        (CompletionClaimKindV2::BugFixed, _, CompletionAssertionV2::Fixed) => {
            let regression_items = plan
                .required_checks
                .iter()
                .filter(|check| check.family == "regression")
                .map(|check| check.plan_item_id.as_str())
                .collect::<BTreeSet<_>>();
            evaluate_claim_runs(
                runs.iter()
                    .filter(|run| regression_items.contains(run.plan_item_id.as_str())),
                "CLAIM_BUG_HAS_CURRENT_REGRESSION_PASS",
            )
        }
        _ => Ok((
            ClaimEvaluationStatusV2::Unverified,
            vec![],
            vec!["CLAIM_DOMAIN_EVIDENCE_NOT_AVAILABLE".to_owned()],
        )),
    }
}

fn evaluate_claim_runs<'a>(
    runs: impl Iterator<Item = &'a ValidationRunV2>,
    verified_reason: &str,
) -> Result<
    (
        ClaimEvaluationStatusV2,
        Vec<ClaimEvidenceRefV2>,
        Vec<String>,
    ),
    CheckGraphRunnerError,
> {
    let runs = runs.collect::<Vec<_>>();
    let actual_evidence_refs = runs
        .iter()
        .map(|run| {
            Ok(ClaimEvidenceRefV2::ValidationRun {
                validation_run_ref: run.reference()?,
            })
        })
        .collect::<Result<Vec<_>, CheckGraphRunnerError>>()?;
    if !runs.is_empty() && runs.iter().all(|run| run.satisfies_required_check()) {
        Ok((
            ClaimEvaluationStatusV2::Verified,
            actual_evidence_refs,
            vec![verified_reason.to_owned()],
        ))
    } else if runs.iter().any(|run| {
        matches!(
            run.outcome,
            ValidationOutcome::Fail | ValidationOutcome::Error
        ) && run.completeness == Completeness::Complete
    }) {
        Ok((
            ClaimEvaluationStatusV2::Contradicted,
            actual_evidence_refs,
            vec!["CLAIM_CHECK_CURRENT_EVIDENCE_FAILED".to_owned()],
        ))
    } else {
        Ok((
            ClaimEvaluationStatusV2::Unverified,
            actual_evidence_refs,
            vec!["CLAIM_CHECK_CURRENT_EVIDENCE_INCOMPLETE".to_owned()],
        ))
    }
}

pub fn evaluate_diagnostics(
    runs: &[ValidationRunV2],
    diagnostics: &[DiagnosticV2],
    baselines: &[BaselineV2],
    suppressions: &[SuppressionV2],
    dispositions: &[DispositionV2],
    evaluation_time: DateTime<Utc>,
) -> Result<Vec<DiagnosticEvaluationV2>, CheckGraphRunnerError> {
    for baseline in baselines {
        if baseline.clone().seal().as_ref() != Ok(baseline) {
            return Err(CheckGraphRunnerError::Evidence(EvidenceV2Error::Baseline));
        }
    }
    for suppression in suppressions {
        if suppression.clone().seal().as_ref() != Ok(suppression) {
            return Err(CheckGraphRunnerError::Evidence(
                EvidenceV2Error::Suppression,
            ));
        }
    }
    for disposition in dispositions {
        if disposition.clone().seal().as_ref() != Ok(disposition) {
            return Err(CheckGraphRunnerError::Evidence(
                EvidenceV2Error::Disposition,
            ));
        }
    }
    let run_map = runs
        .iter()
        .map(|run| (run.validation_run_id.clone(), run))
        .collect::<BTreeMap<_, _>>();
    let mut evaluations = Vec::new();
    for diagnostic in diagnostics {
        let run = run_map
            .get(&diagnostic.validation_run_id)
            .copied()
            .ok_or(CheckGraphRunnerError::Binding)?;
        let baseline = baselines
            .iter()
            .filter(|baseline| {
                baseline.active && baseline.reviewed && baseline.project_id == diagnostic.project_id
            })
            .max_by_key(|baseline| baseline.revision);
        let exact_entry = baseline.and_then(|baseline| {
            baseline
                .entries
                .iter()
                .find(|entry| entry.diagnostic_fingerprint == diagnostic.fingerprint)
        });
        let related_entry = baseline.and_then(|baseline| {
            baseline
                .entries
                .iter()
                .find(|entry| entry.rule_ref == diagnostic.rule_ref)
        });
        let baseline_relation = if exact_entry.is_some() {
            BaselineRelationV2::ExistingUnchanged
        } else if let Some(entry) = related_entry {
            if severity_rank(diagnostic.severity) > severity_rank(entry.severity) {
                BaselineRelationV2::Worsened
            } else {
                BaselineRelationV2::Improved
            }
        } else if baseline.is_some() {
            BaselineRelationV2::New
        } else {
            BaselineRelationV2::Unbaselined
        };
        let baseline_ref = baseline.map(|baseline| DecisionDocumentRefV2 {
            document_id: baseline.baseline_id.to_string(),
            revision: baseline.revision,
            fingerprint: baseline.set_fingerprint.clone(),
        });
        let suppression = suppressions
            .iter()
            .filter(|suppression| suppression.project_id == diagnostic.project_id)
            .filter(|suppression| {
                suppression
                    .diagnostic_fingerprint
                    .as_ref()
                    .is_some_and(|value| value == &diagnostic.fingerprint)
                    || suppression
                        .rule_ref
                        .as_ref()
                        .is_some_and(|value| value == &diagnostic.rule_ref)
            })
            .max_by_key(|suppression| suppression.revision);
        let suppression_state = suppression
            .map(|suppression| {
                if suppression.status == SuppressionStatus::Revoked {
                    SuppressionStateV2::Revoked
                } else if suppression.status == SuppressionStatus::Stale
                    || suppression.subject_binding_fingerprint
                        != run.subject_binding.binding_fingerprint
                {
                    SuppressionStateV2::Stale
                } else if suppression.status == SuppressionStatus::Expired
                    || suppression
                        .expires_at
                        .is_some_and(|expires| expires <= evaluation_time)
                {
                    SuppressionStateV2::Expired
                } else if suppression.status == SuppressionStatus::Active {
                    SuppressionStateV2::Active
                } else {
                    SuppressionStateV2::Invalid
                }
            })
            .unwrap_or(SuppressionStateV2::None);
        let suppression_ref = suppression.map(|suppression| DecisionDocumentRefV2 {
            document_id: suppression.suppression_id.to_string(),
            revision: suppression.revision,
            fingerprint: suppression.content_fingerprint.clone(),
        });
        let disposition = dispositions
            .iter()
            .filter(|disposition| {
                disposition.active
                    && disposition.project_id == diagnostic.project_id
                    && disposition.diagnostic_fingerprint == diagnostic.fingerprint
                    && disposition.subject_binding_fingerprint
                        == run.subject_binding.binding_fingerprint
                    && disposition
                        .expires_at
                        .is_none_or(|expires| expires > evaluation_time)
            })
            .max_by_key(|disposition| disposition.revision);
        let disposition_ref = disposition.map(|disposition| DecisionDocumentRefV2 {
            document_id: disposition.disposition_id.to_string(),
            revision: disposition.revision,
            fingerprint: disposition.content_fingerprint.clone(),
        });
        let (mut gate_effect, mut reason_codes) = if suppression_state == SuppressionStateV2::Active
        {
            (
                DiagnosticGateEffectV2::RemainingRisk,
                vec!["ACTIVE_SUPPRESSION_PRESERVES_RAW_DIAGNOSTIC".to_owned()],
            )
        } else if disposition.is_some_and(|disposition| {
            matches!(
                disposition.decision,
                DispositionDecision::FalsePositive | DispositionDecision::AcceptedRisk
            )
        }) {
            (
                DiagnosticGateEffectV2::RequiresReview,
                vec!["DISPOSITION_REQUIRES_CURRENT_REVIEW".to_owned()],
            )
        } else if baseline_relation == BaselineRelationV2::ExistingUnchanged {
            (
                DiagnosticGateEffectV2::RemainingRisk,
                vec!["BASELINE_EXISTING_UNCHANGED".to_owned()],
            )
        } else if diagnostic.blocking {
            (
                DiagnosticGateEffectV2::Blocks,
                vec![
                    match baseline_relation {
                        BaselineRelationV2::New | BaselineRelationV2::Unbaselined => {
                            "NEW_BLOCKING_DIAGNOSTIC"
                        }
                        BaselineRelationV2::Worsened => "WORSENED_BLOCKING_DIAGNOSTIC",
                        _ => "UNSUPPRESSED_BLOCKING_DIAGNOSTIC",
                    }
                    .to_owned(),
                ],
            )
        } else {
            (
                DiagnosticGateEffectV2::RemainingRisk,
                vec!["NON_BLOCKING_DIAGNOSTIC".to_owned()],
            )
        };
        let later_clean_attempt = runs.iter().any(|candidate| {
            candidate.plan_item_id == run.plan_item_id
                && candidate.attempt > run.attempt
                && candidate.satisfies_required_check()
        });
        if later_clean_attempt && gate_effect == DiagnosticGateEffectV2::Blocks {
            gate_effect = DiagnosticGateEffectV2::RequiresReview;
            reason_codes.push("RETRY_OUTCOME_DIVERGED_FLAKY_NOT_FALSE_POSITIVE".to_owned());
        }
        if matches!(
            suppression_state,
            SuppressionStateV2::Expired | SuppressionStateV2::Stale | SuppressionStateV2::Revoked
        ) {
            reason_codes.push("SUPPRESSION_NOT_CURRENT".to_owned());
        }
        evaluations.push(
            DiagnosticEvaluationV2 {
                evaluation_subject: DiagnosticEvaluationSubjectV2::CurrentDiagnostic {
                    diagnostic_ref: diagnostic.reference()?,
                },
                subject_binding_fingerprint: run.subject_binding.binding_fingerprint.clone(),
                baseline_relation,
                baseline_ref,
                suppression_state,
                suppression_ref,
                disposition_ref,
                gate_effect,
                reason_codes,
                evaluation_fingerprint: empty_fingerprint(),
            }
            .seal()?,
        );
    }
    let observed = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.fingerprint.clone())
        .collect::<BTreeSet<_>>();
    for baseline in baselines
        .iter()
        .filter(|baseline| baseline.active && baseline.reviewed)
    {
        let Some(run) = runs
            .iter()
            .find(|run| run.project_id == baseline.project_id)
        else {
            continue;
        };
        if run.subject_binding.freshness != EvidenceFreshnessV2::Current
            || runs.iter().any(|candidate| {
                candidate.project_id == baseline.project_id
                    && candidate.completeness != Completeness::Complete
            })
        {
            continue;
        }
        for entry in baseline
            .entries
            .iter()
            .filter(|entry| !observed.contains(&entry.diagnostic_fingerprint))
        {
            evaluations.push(
                DiagnosticEvaluationV2 {
                    evaluation_subject: DiagnosticEvaluationSubjectV2::BaselineEntry {
                        baseline_id: baseline.baseline_id.clone(),
                        revision: baseline.revision,
                        entry_fingerprint: entry.entry_fingerprint.clone(),
                    },
                    subject_binding_fingerprint: run.subject_binding.binding_fingerprint.clone(),
                    baseline_relation: BaselineRelationV2::NotObserved,
                    baseline_ref: Some(DecisionDocumentRefV2 {
                        document_id: baseline.baseline_id.to_string(),
                        revision: baseline.revision,
                        fingerprint: baseline.set_fingerprint.clone(),
                    }),
                    suppression_state: SuppressionStateV2::None,
                    suppression_ref: None,
                    disposition_ref: None,
                    gate_effect: DiagnosticGateEffectV2::None,
                    reason_codes: vec!["BASELINE_ENTRY_NOT_OBSERVED".to_owned()],
                    evaluation_fingerprint: empty_fingerprint(),
                }
                .seal()?,
            );
        }
    }
    evaluations.sort_by(|left, right| {
        left.evaluation_fingerprint
            .cmp(&right.evaluation_fingerprint)
    });
    Ok(evaluations)
}

pub fn evaluate_run_satisfactions(
    plan: &FullValidationPlan,
    runs: &[ValidationRunV2],
    diagnostics: &[DiagnosticV2],
    evaluations: &[DiagnosticEvaluationV2],
) -> Result<Vec<RunSatisfactionV2>, CheckGraphRunnerError> {
    let mut grouped = BTreeMap::<&str, Vec<&ValidationRunV2>>::new();
    for run in runs {
        grouped
            .entry(run.plan_item_id.as_str())
            .or_default()
            .push(run);
    }
    let mut result = Vec::with_capacity(plan.required_checks.len());
    for check in &plan.required_checks {
        let mut attempts = grouped
            .remove(check.plan_item_id.as_str())
            .ok_or(CheckGraphRunnerError::Graph)?;
        attempts.sort_by_key(|run| run.attempt);
        if attempts
            .iter()
            .enumerate()
            .any(|(index, run)| run.attempt != index as u32 + 1)
        {
            return Err(CheckGraphRunnerError::Graph);
        }
        let attempt_ids = attempts
            .iter()
            .map(|run| run.validation_run_id.clone())
            .collect::<BTreeSet<_>>();
        let run_diagnostic_refs = diagnostics
            .iter()
            .filter(|diagnostic| attempt_ids.contains(&diagnostic.validation_run_id))
            .map(DiagnosticV2::reference)
            .collect::<Result<BTreeSet<_>, _>>()?;
        let run_evaluations = evaluations
            .iter()
            .filter(|evaluation| match &evaluation.evaluation_subject {
                DiagnosticEvaluationSubjectV2::CurrentDiagnostic { diagnostic_ref } => {
                    run_diagnostic_refs.contains(diagnostic_ref)
                }
                DiagnosticEvaluationSubjectV2::BaselineEntry { .. } => false,
            })
            .collect::<Vec<_>>();
        let first = attempts[0];
        let first_diagnostic_fingerprints = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.validation_run_id == first.validation_run_id)
            .map(|diagnostic| diagnostic.fingerprint.clone())
            .collect::<BTreeSet<_>>();
        let retry_outcome_diverged = attempts.iter().skip(1).any(|run| {
            let fingerprints = diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.validation_run_id == run.validation_run_id)
                .map(|diagnostic| diagnostic.fingerprint.clone())
                .collect::<BTreeSet<_>>();
            run.outcome != first.outcome
                || run.completeness != first.completeness
                || run.stability != first.stability
                || run.exit_code != first.exit_code
                || fingerprints != first_diagnostic_fingerprints
        });
        let flaky = retry_outcome_diverged
            || attempts
                .iter()
                .any(|run| run.stability == ValidationStabilityV2::Flaky);
        let ratchet_accepted = !run_diagnostic_refs.is_empty()
            && ratchet_eligible_family(&check.family)
            && attempts.iter().all(|run| {
                run.completeness == Completeness::Complete
                    && run.stability == ValidationStabilityV2::Stable
                    && run.subject_binding.freshness == EvidenceFreshnessV2::Current
            })
            && run_evaluations.iter().all(|evaluation| {
                evaluation.baseline_relation == BaselineRelationV2::ExistingUnchanged
                    || evaluation.suppression_state == SuppressionStateV2::Active
            });
        let requires_review = run_evaluations
            .iter()
            .any(|evaluation| evaluation.gate_effect == DiagnosticGateEffectV2::RequiresReview);
        let (satisfaction, gate_effect, reason_code, policy_reason) =
            if attempts.len() == 1 && first.satisfies_required_check() {
                (
                    RunSatisfactionStateV2::CleanPass,
                    RunGateEffectV2::None,
                    "COMPLETE_CURRENT_STABLE_PASS",
                    "required check produced exact positive evidence",
                )
            } else if flaky {
                (
                    RunSatisfactionStateV2::Unsatisfied,
                    if plan.gate_policy.fail_on_flaky {
                        RunGateEffectV2::Block
                    } else {
                        RunGateEffectV2::HumanReview
                    },
                    "RETRY_OUTCOME_DIVERGED_FLAKY",
                    "attempt divergence is preserved as flaky evidence and is never a clean pass",
                )
            } else if ratchet_accepted {
                (
                    RunSatisfactionStateV2::RatchetSatisfied,
                    RunGateEffectV2::None,
                    "EXISTING_DEBT_RATCHET_SATISFIED",
                    "all failures are unchanged baseline debt or current suppressions",
                )
            } else if requires_review {
                (
                    RunSatisfactionStateV2::Unsatisfied,
                    RunGateEffectV2::HumanReview,
                    "REQUIRED_CHECK_REQUIRES_REVIEW",
                    "policy requires a human decision without rewriting the raw outcome",
                )
            } else {
                (
                    RunSatisfactionStateV2::Unsatisfied,
                    RunGateEffectV2::Block,
                    "REQUIRED_CHECK_UNSATISFIED",
                    "required execution is failed, incomplete, stale, flaky, or not run",
                )
            };
        result.push(
            RunSatisfactionV2 {
                plan_item_id: check.plan_item_id.clone(),
                requirement: CheckRequirementV2::Required,
                validation_run_refs: attempts
                    .iter()
                    .map(|run| run.reference())
                    .collect::<Result<Vec<_>, _>>()?,
                raw_outcomes: attempts.iter().map(|run| run.outcome).collect(),
                satisfaction,
                gate_effect,
                reason_code: reason_code.to_owned(),
                diagnostic_evaluation_fingerprints: run_evaluations
                    .iter()
                    .map(|evaluation| evaluation.evaluation_fingerprint.clone())
                    .collect(),
                policy_reason: policy_reason.to_owned(),
                content_fingerprint: empty_fingerprint(),
            }
            .seal()?,
        );
    }
    if !grouped.is_empty() {
        return Err(CheckGraphRunnerError::Graph);
    }
    result.sort_by(|left, right| left.plan_item_id.cmp(&right.plan_item_id));
    Ok(result)
}

fn build_validation_results(
    plan: &FullValidationPlan,
    runs: &[ValidationRunV2],
    satisfactions: &[RunSatisfactionV2],
    created_at: DateTime<Utc>,
) -> Result<Vec<ValidationResultV2>, CheckGraphRunnerError> {
    let mut grouped = BTreeMap::new();
    for run in runs {
        grouped
            .entry(run.project_id.clone())
            .or_insert_with(Vec::new)
            .push(run);
    }
    let mut results = Vec::new();
    for (project_id, project_runs) in grouped {
        let mut subject = project_runs[0].subject_binding.clone();
        let shared_binding_fields_match = project_runs.iter().all(|run| {
            run.subject_binding.project_id == subject.project_id
                && run.subject_binding.checkout_id == subject.checkout_id
                && run.subject_binding.project_revision_id == subject.project_revision_id
                && run.subject_binding.workspace_snapshot_id == subject.workspace_snapshot_id
                && run.subject_binding.workspace_content_fingerprint
                    == subject.workspace_content_fingerprint
                && run.subject_binding.task_spec_ref == subject.task_spec_ref
                && run.subject_binding.scope_revision_ref == subject.scope_revision_ref
                && run.subject_binding.impact_analysis_ref == subject.impact_analysis_ref
                && run.subject_binding.change_set_refs == subject.change_set_refs
                && run.subject_binding.validation_plan_ref == subject.validation_plan_ref
                && run.subject_binding.effective_config_fingerprint
                    == subject.effective_config_fingerprint
                && run.subject_binding.catalog_snapshot_ref == subject.catalog_snapshot_ref
                && run.subject_binding.freshness == subject.freshness
        });
        if !shared_binding_fields_match {
            return Err(CheckGraphRunnerError::Binding);
        }
        let environments = project_runs
            .iter()
            .map(|run| {
                run.subject_binding
                    .execution_environment_fingerprint
                    .clone()
            })
            .collect::<BTreeSet<_>>();
        subject.check_descriptor_ref = None;
        subject.rule_refs.clear();
        subject.tool_registry_snapshot_ref = None;
        subject.tool_descriptor_ref = None;
        subject.observed_tool_fingerprint = None;
        subject.invocation_fingerprint = None;
        subject.execution_environment_fingerprint = domain_hash(
            "star.execution-environment-set",
            EVIDENCE_V2_SCHEMA_VERSION,
            &environments,
        )?;
        subject.binding_fingerprint = empty_fingerprint();
        subject = subject.seal()?;
        let project_satisfactions = satisfactions
            .iter()
            .filter(|satisfaction| {
                project_runs
                    .iter()
                    .any(|run| run.plan_item_id == satisfaction.plan_item_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        let outcome = aggregate_outcome(&project_runs);
        let completeness = aggregate_completeness(&project_runs);
        let stability = aggregate_stability(&project_runs);
        let stale_reasons = project_runs
            .iter()
            .flat_map(|run| run.subject_binding.stale_reasons.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        results.push(
            ValidationResultV2 {
                schema_id: VALIDATION_RESULT_V2_SCHEMA_ID.to_owned(),
                schema_version: EVIDENCE_V2_SCHEMA_VERSION,
                validation_result_id: ValidationResultId::new(),
                revision: 1,
                validation_plan_ref: subject.validation_plan_ref.clone(),
                project_id,
                subject_binding: subject.clone(),
                validation_run_refs: project_runs
                    .iter()
                    .map(|run| run.reference())
                    .collect::<Result<Vec<_>, _>>()?,
                outcome,
                completeness,
                freshness: subject.freshness,
                stale_reasons,
                stability,
                run_satisfactions: project_satisfactions,
                normalizer_fingerprint: subject.normalizer_fingerprint.clone(),
                created_at,
                result_fingerprint: empty_fingerprint(),
            }
            .seal(runs)?,
        );
    }
    results.sort_by(|left, right| left.project_id.cmp(&right.project_id));
    let _ = plan;
    Ok(results)
}

fn build_rework_directive(
    gate: &GateDecisionV2,
    satisfactions: &[RunSatisfactionV2],
    diagnostics: &[DiagnosticV2],
) -> Result<Option<ReworkDirectiveV1>, CheckGraphRunnerError> {
    if gate.decision != GateDecisionKind::Block {
        return Ok(None);
    }
    let mut failed = satisfactions
        .iter()
        .filter(|item| {
            !matches!(
                item.satisfaction,
                RunSatisfactionStateV2::CleanPass | RunSatisfactionStateV2::RatchetSatisfied
            )
        })
        .map(|item| item.plan_item_id.clone())
        .collect::<Vec<_>>();
    let blocking_refs = gate
        .blocking_diagnostic_refs
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    failed.extend(diagnostics.iter().filter_map(|diagnostic| {
        diagnostic
            .reference()
            .ok()
            .filter(|reference| blocking_refs.contains(reference))
            .map(|_| diagnostic.plan_item_id.clone())
    }));
    Ok(Some(
        ReworkDirectiveV1 {
            schema_id: REWORK_DIRECTIVE_SCHEMA_ID.to_owned(),
            schema_version: REWORK_DIRECTIVE_SCHEMA_VERSION,
            rework_directive_id: ReworkDirectiveId::new(),
            revision: 1,
            gate_decision_ref: gate.reference()?,
            blocking_diagnostic_refs: gate.blocking_diagnostic_refs.clone(),
            failed_or_missing_plan_item_ids: failed,
            expected_actual_differences: vec![
                "required validation evidence did not match the accepted plan".to_owned(),
            ],
            safe_remediations: vec![
                "inspect the referenced Diagnostic and correct the bounded cause".to_owned(),
            ],
            required_rechecks: vec![
                "rerun the unchanged required CheckPlan after correcting the cause".to_owned(),
            ],
            replan_required: false,
            rerunnable_same_plan: true,
            created_at: gate.decided_at,
            directive_fingerprint: empty_fingerprint(),
        }
        .seal(gate)?,
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_review_pack(
    plan: &FullValidationPlan,
    plan_ref: &DocumentRef,
    runs: &[ValidationRunV2],
    diagnostics: &[DiagnosticV2],
    results: &[ValidationResultV2],
    gate: &GateDecisionV2,
    bundle: &EvidenceBundleV2,
    rework: Option<&ReworkDirectiveV1>,
    created_at: DateTime<Utc>,
) -> Result<ReviewPackV1, CheckGraphRunnerError> {
    let plan_item = ReviewPackItemV1 {
        item_kind: "task_spec".to_owned(),
        status: "accepted_plan".to_owned(),
        summary: format!(
            "TaskSpec {} and ValidationPlan revision {} are the authoritative request inputs.",
            plan.task_spec_ref.document_id, plan.revision
        ),
        evidence_refs: vec![plan.task_spec_ref.clone(), plan_ref.clone()],
    };
    let changes_item = ReviewPackItemV1 {
        item_kind: "actual_change_set".to_owned(),
        status: "bound".to_owned(),
        summary: format!(
            "{} current ChangeSet reference(s) are bound to this run.",
            plan.change_set_refs.len()
        ),
        evidence_refs: plan.change_set_refs.clone(),
    };
    let claim_items = if gate.claim_evaluations.is_empty() {
        vec![ReviewPackItemV1 {
            item_kind: "completion_claim".to_owned(),
            status: "complete_empty_set".to_owned(),
            summary: "No completion claims were submitted for this validation run.".to_owned(),
            evidence_refs: vec![plan.task_spec_ref.clone()],
        }]
    } else {
        gate.claim_evaluations
            .iter()
            .map(|evaluation| {
                let mut evidence_refs = evaluation
                    .actual_evidence_refs
                    .iter()
                    .filter_map(|reference| match reference {
                        ClaimEvidenceRefV2::Document { document_ref } => Some(document_ref.clone()),
                        ClaimEvidenceRefV2::ValidationRun { validation_run_ref } => {
                            Some(DocumentRef {
                                schema_id: VALIDATION_RUN_V2_SCHEMA_ID.to_owned(),
                                document_id: validation_run_ref.validation_run_id.to_string(),
                                revision: validation_run_ref.revision,
                                sha256: validation_run_ref.sha256.clone(),
                            })
                        }
                        ClaimEvidenceRefV2::Artifact { .. } => None,
                    })
                    .collect::<Vec<_>>();
                if evidence_refs.is_empty() {
                    evidence_refs.push(plan.task_spec_ref.clone());
                }
                ReviewPackItemV1 {
                    item_kind: "completion_claim".to_owned(),
                    status: enum_text(evaluation.status),
                    summary: format!(
                        "{}: {}",
                        evaluation.claim_ref.claim_id,
                        evaluation.reason_codes.join(",")
                    ),
                    evidence_refs,
                }
            })
            .collect()
    };
    let check_items = gate
        .run_satisfactions
        .iter()
        .map(|satisfaction| {
            let evidence_refs = runs
                .iter()
                .filter(|run| run.plan_item_id == satisfaction.plan_item_id)
                .map(validation_run_document_ref)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ReviewPackItemV1 {
                item_kind: "check_result".to_owned(),
                status: enum_text(satisfaction.satisfaction),
                summary: format!(
                    "{}: {} ({})",
                    satisfaction.plan_item_id, satisfaction.reason_code, satisfaction.policy_reason
                ),
                evidence_refs,
            })
        })
        .collect::<Result<Vec<_>, CheckGraphRunnerError>>()?;
    let diagnostic_items = gate
        .diagnostic_evaluations
        .iter()
        .map(|evaluation| {
            let evidence_refs = match &evaluation.evaluation_subject {
                DiagnosticEvaluationSubjectV2::CurrentDiagnostic { diagnostic_ref } => diagnostics
                    .iter()
                    .find(|diagnostic| {
                        diagnostic.diagnostic_id == diagnostic_ref.diagnostic_id
                            && diagnostic.sequence == diagnostic_ref.sequence
                    })
                    .map(diagnostic_document_ref)
                    .transpose()?
                    .into_iter()
                    .collect(),
                DiagnosticEvaluationSubjectV2::BaselineEntry { .. } => vec![],
            };
            Ok(ReviewPackItemV1 {
                item_kind: "diagnostic_evaluation".to_owned(),
                status: enum_text(evaluation.baseline_relation),
                summary: evaluation.reason_codes.join(","),
                evidence_refs,
            })
        })
        .collect::<Result<Vec<_>, CheckGraphRunnerError>>()?;
    let quality_items = diagnostics
        .iter()
        .map(|diagnostic| {
            Ok(ReviewPackItemV1 {
                item_kind: "quality_or_security_diagnostic".to_owned(),
                status: enum_text(diagnostic.severity),
                summary: format!("{}: {}", diagnostic.code, diagnostic.title),
                evidence_refs: vec![diagnostic_document_ref(diagnostic)?],
            })
        })
        .collect::<Result<Vec<_>, CheckGraphRunnerError>>()?;
    let gate_item = ReviewPackItemV1 {
        item_kind: "gate_decision".to_owned(),
        status: enum_text(gate.decision),
        summary: gate.reason_codes.join(","),
        evidence_refs: vec![DocumentRef {
            schema_id: GATE_DECISION_V2_SCHEMA_ID.to_owned(),
            document_id: gate.gate_id.to_string(),
            revision: gate.revision,
            sha256: gate.reference()?.sha256,
        }],
    };
    let risk_items = if gate.remaining_risks.is_empty() {
        vec![ReviewPackItemV1 {
            item_kind: "remaining_risk".to_owned(),
            status: "none_recorded".to_owned(),
            summary: "No additional remaining risk was recorded by the selected plan.".to_owned(),
            evidence_refs: vec![plan_ref.clone()],
        }]
    } else {
        gate.remaining_risks
            .iter()
            .map(|risk| ReviewPackItemV1 {
                item_kind: "remaining_risk".to_owned(),
                status: "open".to_owned(),
                summary: risk.title.clone(),
                evidence_refs: vec![plan_ref.clone()],
            })
            .collect()
    };
    let evidence_items = results
        .iter()
        .map(|result| {
            Ok(ReviewPackItemV1 {
                item_kind: "evidence_identity".to_owned(),
                status: enum_text(result.completeness),
                summary: format!(
                    "EvidenceBundle {} binds ValidationResult {}.",
                    bundle.evidence_bundle_id, result.validation_result_id
                ),
                evidence_refs: vec![bundle.reference()?, result.reference()?],
            })
        })
        .collect::<Result<Vec<_>, CheckGraphRunnerError>>()?;
    let sections = vec![
        ReviewPackSectionV1 {
            key: REVIEW_PACK_SECTION_ORDER[0].to_owned(),
            items: vec![plan_item],
        },
        ReviewPackSectionV1 {
            key: REVIEW_PACK_SECTION_ORDER[1].to_owned(),
            items: vec![changes_item],
        },
        ReviewPackSectionV1 {
            key: REVIEW_PACK_SECTION_ORDER[2].to_owned(),
            items: claim_items,
        },
        ReviewPackSectionV1 {
            key: REVIEW_PACK_SECTION_ORDER[3].to_owned(),
            items: check_items,
        },
        ReviewPackSectionV1 {
            key: REVIEW_PACK_SECTION_ORDER[4].to_owned(),
            items: diagnostic_items,
        },
        ReviewPackSectionV1 {
            key: REVIEW_PACK_SECTION_ORDER[5].to_owned(),
            items: quality_items,
        },
        ReviewPackSectionV1 {
            key: REVIEW_PACK_SECTION_ORDER[6].to_owned(),
            items: vec![gate_item],
        },
        ReviewPackSectionV1 {
            key: REVIEW_PACK_SECTION_ORDER[7].to_owned(),
            items: risk_items,
        },
        ReviewPackSectionV1 {
            key: REVIEW_PACK_SECTION_ORDER[8].to_owned(),
            items: evidence_items,
        },
    ];
    let questions = (gate.decision == GateDecisionKind::HumanReview)
        .then(|| ReviewQuestionV1 {
            question_id: "review-current-evidence".to_owned(),
            prompt: "현재 evidence와 remaining risk를 검토하고 다음 진행 여부를 결정해 주세요."
                .to_owned(),
            options: vec![
                "approve_manual_progress".to_owned(),
                "request_rework".to_owned(),
            ],
            impact: "승인은 자동 통과로 집계되지 않으며 exact evidence revision에만 적용됩니다."
                .to_owned(),
        })
        .into_iter()
        .collect();
    ReviewPackV1 {
        schema_id: REVIEW_PACK_SCHEMA_ID.to_owned(),
        schema_version: REVIEW_PACK_SCHEMA_VERSION,
        review_pack_id: ReviewPackId::new(),
        revision: 1,
        evidence_bundle_ref: bundle.reference()?,
        authoritative_gate_decision_ref: gate.reference()?,
        section_order: REVIEW_PACK_SECTION_ORDER
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        sections,
        questions,
        required_action_refs: rework
            .map(ReworkDirectiveV1::reference)
            .transpose()?
            .into_iter()
            .collect(),
        rendered_artifact_refs: vec![],
        completeness: bundle.completeness,
        missing_reasons: bundle.missing_reasons.clone(),
        created_at,
        review_pack_fingerprint: empty_fingerprint(),
    }
    .seal(bundle, gate)
    .map_err(CheckGraphRunnerError::from)
}

fn aggregate_outcome(runs: &[&ValidationRunV2]) -> ValidationOutcome {
    if runs
        .iter()
        .any(|run| run.outcome == ValidationOutcome::Error)
    {
        ValidationOutcome::Error
    } else if runs
        .iter()
        .any(|run| run.outcome == ValidationOutcome::Cancelled)
    {
        ValidationOutcome::Cancelled
    } else if runs
        .iter()
        .any(|run| run.outcome == ValidationOutcome::NotRun)
    {
        ValidationOutcome::NotRun
    } else if runs
        .iter()
        .any(|run| run.outcome == ValidationOutcome::Fail)
    {
        ValidationOutcome::Fail
    } else {
        ValidationOutcome::Pass
    }
}

fn aggregate_completeness(runs: &[&ValidationRunV2]) -> Completeness {
    if runs
        .iter()
        .any(|run| run.completeness == Completeness::Unverified)
    {
        Completeness::Unverified
    } else if runs
        .iter()
        .any(|run| run.completeness == Completeness::Partial)
    {
        Completeness::Partial
    } else {
        Completeness::Complete
    }
}

fn aggregate_stability(runs: &[&ValidationRunV2]) -> ValidationStabilityV2 {
    if runs
        .iter()
        .any(|run| run.stability == ValidationStabilityV2::Flaky)
        || runs.iter().any(|run| {
            runs.iter().any(|candidate| {
                candidate.plan_item_id == run.plan_item_id
                    && candidate.attempt != run.attempt
                    && (candidate.outcome != run.outcome
                        || candidate.completeness != run.completeness
                        || candidate.exit_code != run.exit_code)
            })
        })
    {
        ValidationStabilityV2::Flaky
    } else if runs
        .iter()
        .any(|run| run.stability == ValidationStabilityV2::NotEvaluated)
    {
        ValidationStabilityV2::NotEvaluated
    } else {
        ValidationStabilityV2::Stable
    }
}

fn ratchet_eligible_family(family: &str) -> bool {
    matches!(
        family,
        "format" | "lint" | "docs" | "config" | "contract" | "hardcoding" | "generation"
    )
}

const fn severity_rank(severity: DiagnosticSeverity) -> u8 {
    match severity {
        DiagnosticSeverity::Info => 0,
        DiagnosticSeverity::Warning => 1,
        DiagnosticSeverity::Error => 2,
        DiagnosticSeverity::Critical => 3,
    }
}

fn domain_hash<T: Serialize>(
    domain: &str,
    version: u32,
    value: &T,
) -> Result<Sha256Hash, CheckGraphRunnerError> {
    canonical_sha256(&serde_json::json!({
        "domain":domain,
        "version":version,
        "value":value,
    }))
    .map_err(|_| CheckGraphRunnerError::Fingerprint)
}

fn enum_text<T: Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned())
}

fn validation_run_document_ref(
    run: &ValidationRunV2,
) -> Result<DocumentRef, CheckGraphRunnerError> {
    Ok(DocumentRef {
        schema_id: VALIDATION_RUN_V2_SCHEMA_ID.to_owned(),
        document_id: run.validation_run_id.to_string(),
        revision: run.revision,
        sha256: run.reference()?.sha256,
    })
}

fn diagnostic_document_ref(
    diagnostic: &DiagnosticV2,
) -> Result<DocumentRef, CheckGraphRunnerError> {
    Ok(DocumentRef {
        schema_id: DIAGNOSTIC_V2_SCHEMA_ID.to_owned(),
        document_id: diagnostic.diagnostic_id.to_string(),
        revision: diagnostic.sequence,
        sha256: diagnostic.reference()?.sha256,
    })
}

type CheckOrder<'a> = Vec<&'a str>;
type CheckPredecessors<'a> = BTreeMap<&'a str, BTreeSet<String>>;

fn topological_order<'a>(
    plan: &'a FullValidationPlan,
    checks: &BTreeMap<&'a str, &'a CheckPlanV2>,
) -> Result<(CheckOrder<'a>, CheckPredecessors<'a>), CheckGraphRunnerError> {
    let mut indegree = checks
        .keys()
        .map(|key| (*key, 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<&str, Vec<&str>>::new();
    let mut predecessors = BTreeMap::<&str, BTreeSet<String>>::new();
    for edge in &plan.check_graph.edges {
        if edge.from_plan_item_id == edge.to_plan_item_id
            || !checks.contains_key(edge.from_plan_item_id.as_str())
            || !checks.contains_key(edge.to_plan_item_id.as_str())
        {
            return Err(CheckGraphRunnerError::Graph);
        }
        *indegree
            .get_mut(edge.to_plan_item_id.as_str())
            .ok_or(CheckGraphRunnerError::Graph)? += 1;
        outgoing
            .entry(edge.from_plan_item_id.as_str())
            .or_default()
            .push(edge.to_plan_item_id.as_str());
        predecessors
            .entry(edge.to_plan_item_id.as_str())
            .or_default()
            .insert(edge.from_plan_item_id.clone());
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(key, _)| *key)
        .collect::<VecDeque<_>>();
    let mut order = Vec::with_capacity(checks.len());
    while let Some(node) = ready.pop_front() {
        order.push(node);
        let mut next = outgoing.get(node).cloned().unwrap_or_default();
        next.sort();
        for child in next {
            let degree = indegree
                .get_mut(child)
                .ok_or(CheckGraphRunnerError::Graph)?;
            *degree -= 1;
            if *degree == 0 {
                let position = ready
                    .iter()
                    .position(|queued| *queued > child)
                    .unwrap_or(ready.len());
                ready.insert(position, child);
            }
        }
    }
    if order.len() != checks.len() {
        return Err(CheckGraphRunnerError::Graph);
    }
    Ok((order, predecessors))
}

fn invocation_for(
    check: &CheckPlanV2,
    binding: &ExecutableBinding,
    plan_ref: &star_contracts::evidence::DocumentRef,
    attempt: u32,
) -> Result<TaskInvocationV2, CheckGraphRunnerError> {
    let idempotency = canonical_sha256(&serde_json::json!({
        "plan":plan_ref,
        "plan_item_id":check.plan_item_id,
        "binding":binding.executable_binding_fingerprint,
        "attempt":attempt,
    }))
    .map_err(|_| CheckGraphRunnerError::Fingerprint)?;
    Ok(TaskInvocationV2 {
        schema_id: TASK_INVOCATION_V2_SCHEMA_ID.to_owned(),
        schema_version: 2,
        invocation_id: TaskInvocationId::new(),
        tool_ref: binding.tool_ref.clone(),
        executable: binding.logical_executable.clone(),
        executable_binding_fingerprint: binding.executable_binding_fingerprint.clone(),
        args: check.invocation.args.clone(),
        cwd: binding.cwd.clone(),
        env_refs: BTreeMap::new(),
        stdin_ref: None,
        timeout_ms: check.invocation.timeout_ms,
        permission_action: binding.permission_action.clone(),
        idempotency_key: idempotency.as_str().to_owned(),
        expected_exit_codes: check
            .invocation
            .expected_exit_codes
            .iter()
            .copied()
            .collect(),
        output_limits: binding.output_limits.clone(),
        input_fingerprint: empty_fingerprint(),
    })
}

#[allow(clippy::too_many_arguments)]
fn diagnostic_for(
    sequence: u64,
    code: &str,
    title: &str,
    message: &str,
    severity: DiagnosticSeverity,
    confidence: DiagnosticConfidence,
    status: DiagnosticStatus,
    blocking: bool,
    check: &CheckPlanV2,
    validation_run_id: &ValidationRunId,
    rule_ref: &CatalogRef,
    evidence_refs: Vec<star_contracts::evidence::ArtifactRef>,
) -> Result<DiagnosticV2, CheckGraphRunnerError> {
    Ok(DiagnosticV2 {
        schema_id: DIAGNOSTIC_V2_SCHEMA_ID.to_owned(),
        schema_version: 2,
        diagnostic_id: DiagnosticId::new(),
        sequence,
        code: code.to_owned(),
        rule_ref: rule_ref.clone(),
        title: title.to_owned(),
        message: message.to_owned(),
        severity,
        confidence,
        status,
        blocking,
        project_id: check.project_id.clone(),
        plan_item_id: check.plan_item_id.clone(),
        validation_run_id: validation_run_id.clone(),
        evidence_refs,
        first_seen_at: Utc::now(),
        last_seen_at: Utc::now(),
        fingerprint: empty_fingerprint(),
    }
    .seal()?)
}

fn risk_refs(plan: &FullValidationPlan) -> Vec<RiskRef> {
    if plan.manual_observations.is_empty() {
        return Vec::new();
    }
    plan.manual_observations
        .iter()
        .enumerate()
        .map(|(index, observation)| RiskRef {
            risk_id: format!("manual-{index}"),
            title: observation.clone(),
            severity: DiagnosticSeverity::Warning,
            evidence_refs: vec![],
        })
        .collect()
}

fn document_hash<T: Serialize>(value: &T) -> Result<Sha256Hash, CheckGraphRunnerError> {
    let value = serde_json::to_value(value).map_err(|_| CheckGraphRunnerError::Fingerprint)?;
    canonical_sha256(&value).map_err(|_| CheckGraphRunnerError::Fingerprint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::{
        evidence::{
            ArtifactKind, ArtifactRef, AuthoritativeGateState, ProducerRef, RedactionStatus,
            RetentionClass,
        },
        ids::{
            ArtifactId, ChangeSetId, CheckoutId, GoalId, ProjectCatalogSnapshotId, ProjectId,
            ProjectRevisionId, RunId, ValidationPlanId, WorkspaceSnapshotId,
        },
        management::ProjectPathRef,
        planning::{
            AffectedScope, ChangeSetKind, CheckApplicability, CheckCandidate, CheckGraphEdgeV2,
            CheckGraphV2, CheckInvocationTemplate, CheckResolutionOutcome, FallbackDecision,
            GatePolicyV2, ObservedChangeKind, ReviewKind, ReviewRequirementV2, ValidationRiskLevel,
            ValidationScopeLevel,
        },
    };

    #[derive(Default)]
    struct FakeExecutor {
        observations: VecDeque<Result<CheckExecutionObservation, CheckExecutorError>>,
        calls: usize,
    }

    impl CheckExecutor for FakeExecutor {
        fn execute(
            &mut self,
            _invocation: &TaskInvocationV2,
        ) -> Result<CheckExecutionObservation, CheckExecutorError> {
            self.calls += 1;
            self.observations.pop_front().expect("fixture observation")
        }
    }

    fn reference(schema_id: &str, document_id: &str) -> star_contracts::evidence::DocumentRef {
        star_contracts::evidence::DocumentRef {
            schema_id: schema_id.to_owned(),
            document_id: document_id.to_owned(),
            revision: 1,
            sha256: Sha256Hash::digest(document_id.as_bytes()),
        }
    }

    fn catalog(id: &str) -> CatalogRef {
        CatalogRef {
            catalog_id: id.to_owned(),
            format_version: 1,
            item_version: "1.0.0".to_owned(),
            sha256: Sha256Hash::digest(id.as_bytes()),
        }
    }

    fn plan(review: bool) -> FullValidationPlan {
        let project_id = ProjectId::new();
        let checks = ["format", "test"]
            .into_iter()
            .enumerate()
            .map(|(index, family)| CheckPlanV2 {
                plan_item_id: format!("item-{index}"),
                check_id: format!("check-{family}"),
                descriptor_ref: reference("star.check-descriptor", &format!("check-{family}")),
                tool_id: "fixture.validator".to_owned(),
                family: family.to_owned(),
                project_id: project_id.clone(),
                scope_level: ValidationScopeLevel::ProjectFull,
                outcome: CheckResolutionOutcome::SelectedRequired,
                reason_codes: vec!["fixture".to_owned()],
                impact_edge_ids: vec![],
                risk_path_ids: vec![],
                invocation: CheckInvocationTemplate {
                    logical_executable: "project-validator".to_owned(),
                    args: vec!["--profile".to_owned(), "target".to_owned()],
                    timeout_ms: 60_000,
                    expected_exit_codes: vec![0],
                },
                fallback_floor: ValidationScopeLevel::ProjectFull,
                evidence_kinds: vec!["validation_result".to_owned()],
            })
            .collect::<Vec<_>>();
        FullValidationPlan {
            schema_id: star_contracts::planning::FULL_VALIDATION_PLAN_SCHEMA_ID.to_owned(),
            schema_version: 2,
            validation_plan_id: ValidationPlanId::new(),
            revision: 1,
            task_spec_ref: reference("star.task-spec", "task"),
            scope_revision: 1,
            scope_revision_ref: reference("star.scope-revision", "scope"),
            phase: "patch_pre_apply".to_owned(),
            change_set_refs: vec![reference("star.change-set", "changes")],
            impact_analysis_ref: reference("star.impact-analysis", "impact"),
            risk_level: ValidationRiskLevel::Low,
            affected_scope: vec![AffectedScope {
                project_id,
                requested_level: ValidationScopeLevel::Package,
                selected_level: ValidationScopeLevel::ProjectFull,
                selectors: vec![],
                reason_codes: vec!["fixture".to_owned()],
                limitations: vec![],
            }],
            candidate_checks: checks
                .iter()
                .map(|check| CheckCandidate {
                    family: check.family.clone(),
                    check_id: Some(check.check_id.clone()),
                    applicability: CheckApplicability::Applicable,
                    outcome: CheckResolutionOutcome::SelectedRequired,
                    evidence_refs: vec![],
                    reason_code: "fixture".to_owned(),
                })
                .collect(),
            required_checks: checks.clone(),
            optional_checks: vec![],
            check_graph: CheckGraphV2 {
                nodes: checks
                    .iter()
                    .map(|check| check.plan_item_id.clone())
                    .collect(),
                edges: vec![CheckGraphEdgeV2 {
                    from_plan_item_id: "item-0".to_owned(),
                    to_plan_item_id: "item-1".to_owned(),
                    relation: "requires".to_owned(),
                }],
                max_parallel: 2,
                failure_policy: "block_dependents".to_owned(),
            },
            omitted_checks: vec![],
            unresolved_checks: vec![],
            previous_success_comparisons: vec![],
            fallback_decisions: Vec::<FallbackDecision>::new(),
            manual_observations: vec![],
            independent_review: ReviewRequirementV2 {
                required: review,
                review_kind: if review {
                    ReviewKind::HumanSemantic
                } else {
                    ReviewKind::None
                },
                reason_codes: if review {
                    vec!["fixture_review".to_owned()]
                } else {
                    vec![]
                },
                absence_behavior: "human_review".to_owned(),
            },
            gate_policy: GatePolicyV2 {
                fail_on_required_failure: true,
                fail_on_partial: true,
                fail_on_unverified: true,
                fail_on_flaky: true,
            },
            config_fingerprint: Sha256Hash::digest(b"config"),
            catalog_snapshot_ref: reference(
                "star.project-catalog-snapshot",
                ProjectCatalogSnapshotId::from_stable_bytes(b"catalog").as_str(),
            ),
            profile_resolution: None,
            selection_fingerprint: empty_fingerprint(),
            readiness: ValidationPlanV2Readiness::Ready,
        }
        .seal()
        .unwrap()
    }

    fn binding_set(plan: &FullValidationPlan) -> Vec<ExecutableBinding> {
        let plan_ref = reference(
            star_contracts::planning::FULL_VALIDATION_PLAN_SCHEMA_ID,
            plan.validation_plan_id.as_str(),
        );
        plan.required_checks
            .iter()
            .map(|check| {
                let tool_ref = catalog("fixture.validator");
                let subject_binding = EvidenceSubjectBinding {
                    project_id: check.project_id.clone(),
                    checkout_id: CheckoutId::from_stable_bytes(b"fixture-checkout"),
                    project_revision_id: ProjectRevisionId::from_stable_bytes(b"fixture-revision"),
                    workspace_snapshot_id: WorkspaceSnapshotId::from_stable_bytes(
                        b"fixture-workspace",
                    ),
                    workspace_content_fingerprint: Sha256Hash::digest(b"fixture-workspace"),
                    task_spec_ref: plan.task_spec_ref.clone(),
                    scope_revision_ref: plan.scope_revision_ref.clone(),
                    impact_analysis_ref: plan.impact_analysis_ref.clone(),
                    change_set_refs: plan.change_set_refs.clone(),
                    change_plan_refs: vec![],
                    patch_set_ref: None,
                    validation_plan_ref: star_contracts::evidence::DocumentRef {
                        sha256: document_hash(plan).unwrap(),
                        ..plan_ref.clone()
                    },
                    gate_phase: star_contracts::evidence_v2::GatePhaseV2::PatchPreApply,
                    profile_resolution_fingerprint: plan.selection_fingerprint.clone(),
                    effective_config_fingerprint: plan.config_fingerprint.clone(),
                    gate_policy_fingerprint: Sha256Hash::digest(b"fixture-gate-policy"),
                    catalog_snapshot_ref: plan.catalog_snapshot_ref.clone(),
                    validator_registry_fingerprint: Sha256Hash::digest(
                        b"fixture-validator-registry",
                    ),
                    check_descriptor_ref: Some(check.descriptor_ref.clone()),
                    rule_refs: vec![catalog(&check.check_id)],
                    tool_registry_snapshot_ref: None,
                    tool_descriptor_ref: Some(tool_ref.clone()),
                    observed_tool_fingerprint: None,
                    invocation_fingerprint: None,
                    execution_environment_fingerprint: Sha256Hash::digest(b"fixture-environment"),
                    normalizer_fingerprint: Sha256Hash::digest(b"fixture-normalizer"),
                    freshness: EvidenceFreshnessV2::Current,
                    stale_reasons: vec![],
                    binding_fingerprint: empty_fingerprint(),
                    probed_at: Utc::now(),
                }
                .seal()
                .unwrap();
                ExecutableBinding {
                    check_id: check.check_id.clone(),
                    check_ref: catalog(&check.check_id),
                    tool_ref,
                    logical_executable: "project-validator".to_owned(),
                    executable_binding_fingerprint: Sha256Hash::digest(b"fixture-binding"),
                    cwd: InvocationWorkingDirectoryV2::ProjectRoot,
                    permission_action: "local_write".to_owned(),
                    output_limits: OutputLimits {
                        stdout_bytes: 1024,
                        stderr_bytes: 1024,
                        artifact_bytes: 4096,
                    },
                    subject_binding,
                }
            })
            .collect()
    }

    fn manifest_ref() -> ArtifactRef {
        ArtifactRef {
            artifact_id: ArtifactId::new(),
            kind: ArtifactKind::Manifest,
            project_id: None,
            relative_path: ".ai-runs/star-control/fixture/manifest.json".to_owned(),
            media_type: "application/json".to_owned(),
            size_bytes: 2,
            sha256: Sha256Hash::digest(b"{}"),
            created_at: Utc::now(),
            producer: ProducerRef {
                component: "fixture".to_owned(),
                product_version: "0.1.0".to_owned(),
                build_id: "fixture".to_owned(),
                platform: "windows-x64".to_owned(),
            },
            redaction_status: RedactionStatus::NotNeeded,
            retention_class: RetentionClass::Evidence,
            source_artifact_ref: None,
        }
    }

    fn context() -> CheckGraphRunContext {
        CheckGraphRunContext {
            gate_scope: GateScope::Goal {
                goal_id: GoalId::new(),
                run_id: RunId::new(),
                revision: 1,
            },
            decided_by: ActorRef {
                actor_type: star_contracts::evidence::ActorType::Controller,
                actor_id: "fixture-controller".to_owned(),
                display_name: "Fixture Controller".to_owned(),
                auth_source: "fixture".to_owned(),
            },
            artifact_manifest: ArtifactManifest {
                manifest_ref: manifest_ref(),
                artifacts: vec![],
            },
            force_human_review: false,
            baselines: vec![],
            suppressions: vec![],
            dispositions: vec![],
            evaluation_time: Utc::now(),
            max_attempts_per_check: 1,
            preflight_diagnostics: vec![],
            completion_claims: vec![],
            change_sets: vec![],
        }
    }

    fn observation(
        termination_reason: TerminationReason,
        exit_code: Option<i32>,
        completeness: Completeness,
        stability: ValidationStabilityV2,
    ) -> CheckExecutionObservation {
        let now = Utc::now();
        CheckExecutionObservation {
            started_at: now,
            finished_at: now,
            exit_code,
            termination_reason,
            completeness,
            stability,
            artifact_refs: vec![],
            observed_tool: Some(ObservedTool {
                executable_path: "registered://fixture-validator".to_owned(),
                version: "1.0.0".to_owned(),
                sha256: Sha256Hash::digest(b"fixture-validator"),
            }),
            diagnostics: vec![],
        }
    }

    fn claim_actor() -> ActorRef {
        ActorRef {
            actor_type: star_contracts::evidence::ActorType::User,
            actor_id: "fixture-user".to_owned(),
            display_name: "Fixture User".to_owned(),
            auth_source: "fixture".to_owned(),
        }
    }

    fn passing_observations() -> VecDeque<Result<CheckExecutionObservation, CheckExecutorError>> {
        VecDeque::from([
            Ok(observation(
                TerminationReason::Exited,
                Some(0),
                Completeness::Complete,
                ValidationStabilityV2::Stable,
            )),
            Ok(observation(
                TerminationReason::Exited,
                Some(0),
                Completeness::Complete,
                ValidationStabilityV2::Stable,
            )),
        ])
    }

    #[test]
    fn completion_claims_are_compared_with_current_runs_and_change_sets() {
        let plan = plan(false);
        let project_id = plan.required_checks[0].project_id.clone();
        let mut verified_context = context();
        verified_context.completion_claims = vec![CompletionClaimV2 {
            claim_id: "check-format-passed".to_owned(),
            kind: CompletionClaimKindV2::CheckExecuted,
            subject: CompletionClaimSubjectV2::CheckPlan {
                project_id: project_id.clone(),
                plan_item_id: plan.required_checks[0].plan_item_id.clone(),
                descriptor_ref: plan.required_checks[0].descriptor_ref.clone(),
            },
            assertion: CompletionAssertionV2::Pass,
            required: true,
            reported_evidence_refs: vec![],
            reported_subject_binding: None,
            source_actor: claim_actor(),
            created_at: Utc::now(),
            claim_fingerprint: empty_fingerprint(),
        }];
        let mut executor = FakeExecutor {
            observations: passing_observations(),
            calls: 0,
        };
        let result =
            run_check_graph(&plan, &binding_set(&plan), verified_context, &mut executor).unwrap();
        assert_eq!(result.gate_decision.decision, GateDecisionKind::AutoPass);
        assert_eq!(
            result.gate_decision.claim_evaluations[0].status,
            ClaimEvaluationStatusV2::Verified
        );

        let mut contradicted_context = context();
        contradicted_context.completion_claims = vec![CompletionClaimV2 {
            claim_id: "source-modified".to_owned(),
            kind: CompletionClaimKindV2::Change,
            subject: CompletionClaimSubjectV2::Path {
                project_id: project_id.clone(),
                path: ProjectPathRef::parse("src/lib.rs").unwrap(),
            },
            assertion: CompletionAssertionV2::Change {
                operation: ObservedChangeKind::Modify,
                after_sha256: Some(Sha256Hash::digest(b"expected-after")),
            },
            required: true,
            reported_evidence_refs: vec![],
            reported_subject_binding: None,
            source_actor: claim_actor(),
            created_at: Utc::now(),
            claim_fingerprint: empty_fingerprint(),
        }];
        contradicted_context.change_sets = vec![ChangeSet {
            schema_id: star_contracts::planning::CHANGE_SET_SCHEMA_ID.to_owned(),
            schema_version: 2,
            change_set_id: ChangeSetId::new(),
            task_spec_ref: plan.task_spec_ref.clone(),
            scope_revision_ref: plan.scope_revision_ref.clone(),
            project_id,
            checkout_id: CheckoutId::from_stable_bytes(b"fixture-checkout"),
            change_set_kind: ChangeSetKind::ObservedAfterChange,
            base_revision_id: ProjectRevisionId::from_stable_bytes(b"fixture-revision"),
            observed_workspace_snapshot_id: WorkspaceSnapshotId::from_stable_bytes(
                b"fixture-workspace",
            ),
            comparison_scope: vec![],
            entries: vec![],
            collection_limits: vec![],
            collection_state: CollectionState::Complete,
            change_set_fingerprint: Sha256Hash::digest(b"empty-current-change-set"),
            captured_at: Utc::now(),
        }];
        let mut executor = FakeExecutor {
            observations: passing_observations(),
            calls: 0,
        };
        let result = run_check_graph(
            &plan,
            &binding_set(&plan),
            contradicted_context,
            &mut executor,
        )
        .unwrap();
        assert_eq!(result.gate_decision.decision, GateDecisionKind::Block);
        assert_eq!(
            result.gate_decision.claim_evaluations[0].status,
            ClaimEvaluationStatusV2::Contradicted
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "star.validation.claim.contradicted")
        );
    }

    struct FixtureArtifactFinalizer {
        called: bool,
        final_manifest_ref: ArtifactRef,
    }

    impl ArtifactManifestFinalizer for FixtureArtifactFinalizer {
        fn finalize(
            &mut self,
            _validation_plan_ref: &DocumentRef,
            _runs: &[ValidationRunV2],
            _diagnostics: &[DiagnosticV2],
        ) -> Result<ArtifactManifest, ArtifactManifestFinalizationError> {
            self.called = true;
            Ok(ArtifactManifest {
                manifest_ref: self.final_manifest_ref.clone(),
                artifacts: vec![],
            })
        }
    }

    #[test]
    fn final_artifact_manifest_is_bound_before_the_evidence_bundle_is_sealed() {
        let plan = plan(false);
        let mut executor = FakeExecutor {
            observations: VecDeque::from([
                Ok(observation(
                    TerminationReason::Exited,
                    Some(0),
                    Completeness::Complete,
                    ValidationStabilityV2::Stable,
                )),
                Ok(observation(
                    TerminationReason::Exited,
                    Some(0),
                    Completeness::Complete,
                    ValidationStabilityV2::Stable,
                )),
            ]),
            calls: 0,
        };
        let mut final_manifest_ref = manifest_ref();
        final_manifest_ref.relative_path =
            ".ai-runs/star-control/fixture/final-manifest.json".to_owned();
        final_manifest_ref.artifact_id = ArtifactId::new();
        let mut finalizer = FixtureArtifactFinalizer {
            called: false,
            final_manifest_ref: final_manifest_ref.clone(),
        };
        let result = run_check_graph_with_artifact_finalizer(
            &plan,
            &binding_set(&plan),
            context(),
            &mut executor,
            &mut finalizer,
        )
        .unwrap();
        assert!(finalizer.called);
        assert_eq!(
            result.evidence_bundle.artifact_manifest.manifest_ref,
            final_manifest_ref
        );
        assert_eq!(result.gate_decision.decision, GateDecisionKind::AutoPass);
    }

    #[test]
    fn complete_stable_graph_is_the_only_auto_pass_path() {
        let plan = plan(false);
        let mut executor = FakeExecutor {
            observations: VecDeque::from([
                Ok(observation(
                    TerminationReason::Exited,
                    Some(0),
                    Completeness::Complete,
                    ValidationStabilityV2::Stable,
                )),
                Ok(observation(
                    TerminationReason::Exited,
                    Some(0),
                    Completeness::Complete,
                    ValidationStabilityV2::Stable,
                )),
            ]),
            calls: 0,
        };
        let result = run_check_graph(&plan, &binding_set(&plan), context(), &mut executor).unwrap();
        assert_eq!(executor.calls, 2);
        assert_eq!(result.gate_decision.decision, GateDecisionKind::AutoPass);
        assert_eq!(
            result.evidence_bundle.authoritative_gate_state,
            AuthoritativeGateState::Passed
        );
        assert_eq!(result.evidence_bundle.completeness, Completeness::Complete);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn timeout_and_flaky_evidence_block_and_never_run_dependents() {
        let plan_value = plan(false);
        let mut executor = FakeExecutor {
            observations: VecDeque::from([Ok(observation(
                TerminationReason::Timeout,
                None,
                Completeness::Partial,
                ValidationStabilityV2::NotEvaluated,
            ))]),
            calls: 0,
        };
        let result = run_check_graph(
            &plan_value,
            &binding_set(&plan_value),
            context(),
            &mut executor,
        )
        .unwrap();
        assert_eq!(executor.calls, 1);
        assert_eq!(result.gate_decision.decision, GateDecisionKind::Block);
        assert_eq!(result.validation_runs[1].outcome, ValidationOutcome::NotRun);
        assert_eq!(result.evidence_bundle.completeness, Completeness::Partial);
        assert!(
            result
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.blocking)
        );

        let single = plan(false);
        let mut flaky = FakeExecutor {
            observations: VecDeque::from([Ok(observation(
                TerminationReason::Exited,
                Some(0),
                Completeness::Complete,
                ValidationStabilityV2::Flaky,
            ))]),
            calls: 0,
        };
        let result =
            run_check_graph(&single, &binding_set(&single), context(), &mut flaky).unwrap();
        assert_eq!(result.gate_decision.decision, GateDecisionKind::Block);
        assert_ne!(result.validation_runs[0].outcome, ValidationOutcome::Pass);
    }

    #[test]
    fn cyclic_graph_is_rejected_before_executor_side_effects() {
        let mut plan = plan(false);
        plan.check_graph.edges.push(CheckGraphEdgeV2 {
            from_plan_item_id: "item-1".to_owned(),
            to_plan_item_id: "item-0".to_owned(),
            relation: "requires".to_owned(),
        });
        let mut executor = FakeExecutor::default();
        assert!(matches!(
            run_check_graph(&plan, &binding_set(&plan), context(), &mut executor),
            Err(CheckGraphRunnerError::Graph)
        ));
        assert_eq!(executor.calls, 0);
    }

    #[test]
    fn successful_checks_with_required_review_are_not_auto_passed() {
        let plan = plan(true);
        let mut executor = FakeExecutor {
            observations: VecDeque::from([
                Ok(observation(
                    TerminationReason::Exited,
                    Some(0),
                    Completeness::Complete,
                    ValidationStabilityV2::Stable,
                )),
                Ok(observation(
                    TerminationReason::Exited,
                    Some(0),
                    Completeness::Complete,
                    ValidationStabilityV2::Stable,
                )),
            ]),
            calls: 0,
        };
        let result = run_check_graph(&plan, &binding_set(&plan), context(), &mut executor).unwrap();
        assert_eq!(result.gate_decision.decision, GateDecisionKind::HumanReview);
        assert_eq!(
            result.evidence_bundle.authoritative_gate_state,
            AuthoritativeGateState::AwaitingHumanReview
        );
    }

    #[test]
    fn divergent_attempts_are_flaky_and_never_clean_pass() {
        let mut plan = plan(false);
        plan.required_checks.truncate(1);
        plan.candidate_checks.truncate(1);
        plan.check_graph.nodes.truncate(1);
        plan.check_graph.edges.clear();
        plan.gate_policy.fail_on_flaky = false;
        let bindings = binding_set(&plan);
        let mut context = context();
        context.max_attempts_per_check = 2;
        let mut executor = FakeExecutor {
            observations: VecDeque::from([
                Ok(observation(
                    TerminationReason::Exited,
                    Some(1),
                    Completeness::Complete,
                    ValidationStabilityV2::Stable,
                )),
                Ok(observation(
                    TerminationReason::Exited,
                    Some(0),
                    Completeness::Complete,
                    ValidationStabilityV2::Stable,
                )),
            ]),
            calls: 0,
        };
        let result = run_check_graph(&plan, &bindings, context, &mut executor).unwrap();
        assert_eq!(executor.calls, 2);
        assert_eq!(result.validation_runs.len(), 2);
        assert_eq!(result.validation_runs[0].attempt, 1);
        assert_eq!(result.validation_runs[1].attempt, 2);
        assert_ne!(
            result.validation_runs[0].invocation.invocation_id,
            result.validation_runs[1].invocation.invocation_id
        );
        assert_eq!(result.gate_decision.decision, GateDecisionKind::HumanReview);
        assert_eq!(
            result.validation_results[0].stability,
            ValidationStabilityV2::Flaky
        );
        assert!(result.gate_decision.run_satisfactions.iter().all(|item| {
            item.satisfaction == RunSatisfactionStateV2::Unsatisfied
                && item.reason_code == "RETRY_OUTCOME_DIVERGED_FLAKY"
        }));
    }

    #[test]
    fn blocking_preflight_rule_is_included_in_the_authoritative_gate() {
        let plan = plan(false);
        let mut context = context();
        context.preflight_diagnostics = vec![RuleDiagnosticInputV2 {
            family: RuleFamilyV2::B01ChangeScopeClaim,
            rule_id: "star.validation.change.out-of-scope".to_owned(),
            code: "ACTUAL_CHANGE_OUT_OF_SCOPE".to_owned(),
            title: "Out of scope".to_owned(),
            message: "A redacted project path is outside the accepted scope.".to_owned(),
            severity: DiagnosticSeverity::Error,
            confidence: DiagnosticConfidence::High,
            status: DiagnosticStatus::Confirmed,
            decision_floor: RuleDecisionFloorV2::Block,
        }];
        let mut executor = FakeExecutor {
            observations: VecDeque::from([
                Ok(observation(
                    TerminationReason::Exited,
                    Some(0),
                    Completeness::Complete,
                    ValidationStabilityV2::Stable,
                )),
                Ok(observation(
                    TerminationReason::Exited,
                    Some(0),
                    Completeness::Complete,
                    ValidationStabilityV2::Stable,
                )),
            ]),
            calls: 0,
        };
        let result = run_check_graph(&plan, &binding_set(&plan), context, &mut executor).unwrap();
        assert_eq!(result.gate_decision.decision, GateDecisionKind::Block);
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.rule_ref.catalog_id == "star.validation.change.out-of-scope"
                && diagnostic.blocking
        }));
    }
}
