//! Deterministic M3 CheckGraph execution and authoritative evidence writer.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use star_contracts::{
    Sha256Hash, canonical_sha256,
    evidence::{
        ActorRef, ArtifactManifest, CatalogRef, Completeness, DiagnosticConfidence,
        DiagnosticSeverity, DiagnosticStatus, GateDecisionKind, GateScope, ObservedTool,
        OutputLimits, RiskRef, TerminationReason, ValidationOutcome,
    },
    evidence_v2::{
        DIAGNOSTIC_V2_SCHEMA_ID, DiagnosticV2, EVIDENCE_BUNDLE_V2_SCHEMA_ID, EvidenceBundleV2,
        EvidenceV2Error, GATE_DECISION_V2_SCHEMA_ID, GateDecisionV2, TASK_INVOCATION_V2_SCHEMA_ID,
        TaskInvocationV2, VALIDATION_RUN_V2_SCHEMA_ID, ValidationRunV2, ValidationStabilityV2,
        empty_fingerprint,
    },
    ids::{DiagnosticId, EvidenceBundleId, GateId, TaskInvocationId, ValidationRunId},
    management::ProjectPathRef,
    planning::{CheckPlanV2, FullValidationPlan, ValidationPlanV2Readiness},
};
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct ExecutableBinding {
    pub check_id: String,
    pub check_ref: CatalogRef,
    pub tool_ref: CatalogRef,
    pub logical_executable: String,
    pub executable_binding_fingerprint: Sha256Hash,
    pub cwd: ProjectPathRef,
    pub permission_action: String,
    pub output_limits: OutputLimits,
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

#[derive(Clone, Debug)]
pub struct CheckGraphRunContext {
    pub gate_scope: GateScope,
    pub decided_by: ActorRef,
    pub artifact_manifest: ArtifactManifest,
    pub force_human_review: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckGraphRunResult {
    pub validation_runs: Vec<ValidationRunV2>,
    pub diagnostics: Vec<DiagnosticV2>,
    pub gate_decision: GateDecisionV2,
    pub evidence_bundle: EvidenceBundleV2,
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
}

pub fn run_check_graph(
    plan: &FullValidationPlan,
    bindings: &[ExecutableBinding],
    context: CheckGraphRunContext,
    executor: &mut dyn CheckExecutor,
) -> Result<CheckGraphRunResult, CheckGraphRunnerError> {
    if plan.readiness != ValidationPlanV2Readiness::Ready
        || plan.required_checks.is_empty()
        || !plan.unresolved_checks.is_empty()
    {
        return Err(CheckGraphRunnerError::PlanNotReady);
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
        if binding.logical_executable != check.invocation.logical_executable {
            return Err(CheckGraphRunnerError::Binding);
        }
    }
    let (order, predecessors) = topological_order(plan, &checks)?;
    let mut runs = Vec::with_capacity(order.len());
    let mut diagnostics = Vec::new();
    let mut satisfied_items = BTreeSet::new();
    let mut diagnostic_sequence = 0_u64;
    for plan_item_id in order {
        let check = checks[plan_item_id];
        let binding = binding_map[check.check_id.as_str()];
        let dependencies_satisfied = predecessors
            .get(plan_item_id)
            .is_none_or(|required| required.is_subset(&satisfied_items));
        let run_id = ValidationRunId::new();
        let invocation = invocation_for(check, binding, &plan_ref)?.seal()?;
        if !dependencies_satisfied {
            diagnostic_sequence += 1;
            let diagnostic = diagnostic_for(
                diagnostic_sequence,
                "CHECK_DEPENDENCY_NOT_SATISFIED",
                "Required predecessor did not satisfy its check",
                "This CheckGraph node was not run because a required predecessor failed or was not run.",
                DiagnosticSeverity::Error,
                DiagnosticConfidence::High,
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
                    plan_item_id: check.plan_item_id.clone(),
                    project_id: check.project_id.clone(),
                    phase: plan.phase.clone(),
                    attempt: 1,
                    invocation,
                    started_at: None,
                    finished_at: None,
                    outcome: ValidationOutcome::NotRun,
                    completeness: Completeness::Complete,
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
        let execution = executor.execute(&invocation);
        let mut diagnostic_ids = Vec::new();
        let run = match execution {
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
                        message: "The registered check did not produce a complete stable successful result."
                            .to_owned(),
                        severity: DiagnosticSeverity::Error,
                        confidence: DiagnosticConfidence::High,
                        status: DiagnosticStatus::Confirmed,
                        blocking: true,
                    });
                }
                for raw in raw_diagnostics {
                    diagnostic_sequence += 1;
                    let diagnostic = diagnostic_for(
                        diagnostic_sequence,
                        &raw.code,
                        &raw.title,
                        &raw.message,
                        raw.severity,
                        raw.confidence,
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
                    plan_item_id: check.plan_item_id.clone(),
                    project_id: check.project_id.clone(),
                    phase: plan.phase.clone(),
                    attempt: 1,
                    invocation,
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
                .seal()?
            }
            Err(error) => {
                diagnostic_sequence += 1;
                let diagnostic = diagnostic_for(
                    diagnostic_sequence,
                    &error.code,
                    "Check executor could not produce verified evidence",
                    &error.message,
                    DiagnosticSeverity::Error,
                    DiagnosticConfidence::High,
                    true,
                    check,
                    &run_id,
                    &binding.check_ref,
                    vec![],
                )?;
                diagnostic_ids.push(diagnostic.diagnostic_id.clone());
                diagnostics.push(diagnostic);
                ValidationRunV2 {
                    schema_id: VALIDATION_RUN_V2_SCHEMA_ID.to_owned(),
                    schema_version: 2,
                    validation_run_id: run_id,
                    revision: 1,
                    validation_plan_ref: plan_ref.clone(),
                    plan_item_id: check.plan_item_id.clone(),
                    project_id: check.project_id.clone(),
                    phase: plan.phase.clone(),
                    attempt: 1,
                    invocation,
                    started_at: None,
                    finished_at: None,
                    outcome: ValidationOutcome::Error,
                    completeness: Completeness::Unverified,
                    stability: ValidationStabilityV2::NotEvaluated,
                    exit_code: None,
                    termination_reason: Some(error.termination_reason),
                    diagnostic_ids,
                    artifact_refs: vec![],
                    observed_tool: None,
                    result_fingerprint: empty_fingerprint(),
                }
                .seal()?
            }
        };
        if run.satisfies_required_check() {
            satisfied_items.insert(check.plan_item_id.clone());
        }
        runs.push(run);
    }
    runs.sort_by(|left, right| left.plan_item_id.cmp(&right.plan_item_id));
    diagnostics.sort_by_key(|diagnostic| diagnostic.sequence);
    let required_run_refs = runs
        .iter()
        .map(ValidationRunV2::reference)
        .collect::<Result<Vec<_>, _>>()?;
    let satisfied_run_refs = runs
        .iter()
        .filter(|run| run.satisfies_required_check())
        .map(ValidationRunV2::reference)
        .collect::<Result<Vec<_>, _>>()?;
    let blocking_diagnostic_refs = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.blocking)
        .map(DiagnosticV2::reference)
        .collect::<Result<Vec<_>, _>>()?;
    let all_satisfied = required_run_refs.len() == satisfied_run_refs.len();
    let decision = if !all_satisfied || !blocking_diagnostic_refs.is_empty() {
        GateDecisionKind::Block
    } else if context.force_human_review || plan.independent_review.required {
        GateDecisionKind::HumanReview
    } else {
        GateDecisionKind::AutoPass
    };
    let reason_codes = match decision {
        GateDecisionKind::AutoPass => vec!["ALL_REQUIRED_CHECKS_COMPLETE_STABLE_PASS".to_owned()],
        GateDecisionKind::HumanReview => vec!["INDEPENDENT_REVIEW_REQUIRED".to_owned()],
        GateDecisionKind::Block => vec!["REQUIRED_CHECK_NOT_SATISFIED".to_owned()],
    };
    let gate = GateDecisionV2 {
        schema_id: GATE_DECISION_V2_SCHEMA_ID.to_owned(),
        schema_version: 2,
        gate_id: GateId::new(),
        revision: 1,
        validation_plan_ref: plan_ref.clone(),
        scope: context.gate_scope,
        decision,
        required_run_refs,
        satisfied_run_refs,
        blocking_diagnostic_refs,
        reason_codes,
        remaining_risks: risk_refs(plan),
        policy_fingerprint: canonical_sha256(&serde_json::json!({
            "config":plan.config_fingerprint,
            "gate_policy":plan.gate_policy,
        }))
        .map_err(|_| CheckGraphRunnerError::Fingerprint)?,
        decided_by: context.decided_by,
        decided_at: Utc::now(),
        decision_fingerprint: empty_fingerprint(),
    }
    .seal(&runs, &diagnostics)?;
    let complete_execution = runs
        .iter()
        .all(|run| run.completeness == Completeness::Complete);
    let missing_reasons = if complete_execution {
        vec![]
    } else {
        vec!["VALIDATION_EVIDENCE_INCOMPLETE".to_owned()]
    };
    let bundle = EvidenceBundleV2 {
        schema_id: EVIDENCE_BUNDLE_V2_SCHEMA_ID.to_owned(),
        schema_version: 2,
        evidence_bundle_id: EvidenceBundleId::new(),
        revision: 1,
        task_spec_ref: plan.task_spec_ref.clone(),
        scope_revision_ref: plan.scope_revision_ref.clone(),
        impact_analysis_ref: plan.impact_analysis_ref.clone(),
        validation_plan_ref: plan_ref,
        validation_run_refs: runs
            .iter()
            .map(ValidationRunV2::reference)
            .collect::<Result<Vec<_>, _>>()?,
        diagnostic_refs: diagnostics
            .iter()
            .map(DiagnosticV2::reference)
            .collect::<Result<Vec<_>, _>>()?,
        gate_decision_ref: gate.reference()?,
        authoritative_gate_state: gate.authoritative_state(),
        remaining_risks: risk_refs(plan),
        artifact_manifest: context.artifact_manifest,
        completeness: if complete_execution {
            Completeness::Complete
        } else {
            Completeness::Partial
        },
        missing_reasons,
        created_at: Utc::now(),
        bundle_fingerprint: empty_fingerprint(),
    }
    .seal(&runs, &diagnostics, &gate)?;
    Ok(CheckGraphRunResult {
        validation_runs: runs,
        diagnostics,
        gate_decision: gate,
        evidence_bundle: bundle,
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
) -> Result<TaskInvocationV2, CheckGraphRunnerError> {
    let idempotency = canonical_sha256(&serde_json::json!({
        "plan":plan_ref,
        "plan_item_id":check.plan_item_id,
        "binding":binding.executable_binding_fingerprint,
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
        status: DiagnosticStatus::Confirmed,
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
        ids::{ArtifactId, GoalId, ProjectCatalogSnapshotId, ProjectId, RunId, ValidationPlanId},
        planning::{
            AffectedScope, CheckApplicability, CheckCandidate, CheckGraphEdgeV2, CheckGraphV2,
            CheckInvocationTemplate, CheckResolutionOutcome, FallbackDecision, GatePolicyV2,
            ReviewKind, ReviewRequirementV2, ValidationRiskLevel, ValidationScopeLevel,
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
            phase: "pre_apply".to_owned(),
            change_set_refs: vec![],
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
            selection_fingerprint: empty_fingerprint(),
            readiness: ValidationPlanV2Readiness::Ready,
        }
        .seal()
        .unwrap()
    }

    fn binding_set(plan: &FullValidationPlan) -> Vec<ExecutableBinding> {
        plan.required_checks
            .iter()
            .map(|check| ExecutableBinding {
                check_id: check.check_id.clone(),
                check_ref: catalog(&check.check_id),
                tool_ref: catalog("fixture.validator"),
                logical_executable: "project-validator".to_owned(),
                executable_binding_fingerprint: Sha256Hash::digest(b"fixture-binding"),
                cwd: ProjectPathRef::parse("src").unwrap(),
                permission_action: "local_write".to_owned(),
                output_limits: OutputLimits {
                    stdout_bytes: 1024,
                    stderr_bytes: 1024,
                    artifact_bytes: 4096,
                },
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
}
