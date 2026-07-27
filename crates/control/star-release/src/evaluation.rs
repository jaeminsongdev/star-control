use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};

use star_contracts::{
    EvaluationRunId, ProjectId, Sha256Hash,
    release_v2::{
        BUDGET_SNAPSHOT_V1_SCHEMA_ID, BudgetDecisionV1, BudgetQuantityV1, BudgetSnapshotV1,
        COST_RECORD_V1_SCHEMA_ID, CaseAdjudication, ComparabilityState, CostRecordRefV1,
        CostRecordV1, EVALUATION_CASE_DEFINITION_V1_SCHEMA_ID, EVALUATION_CATALOG_ITEM_SCHEMA_ID,
        EVALUATION_POLICY_V1_SCHEMA_ID, EVALUATION_RUN_V2_SCHEMA_ID, EvaluationCaseDefinitionV1,
        EvaluationCaseResult, EvaluationCatalogItem, EvaluationCatalogLifecycle,
        EvaluationComparability, EvaluationContext, EvaluationDefinition, EvaluationMetricSummary,
        EvaluationMode, EvaluationOutcome, EvaluationPolicyRefV1, EvaluationPolicyV1,
        EvaluationQuantityComparisonV1, EvaluationQuantityV1, EvaluationRecommendation,
        EvaluationRunV2, EvaluationSuppressionSummary, ProtectedMetricResult,
    },
};
use star_domain::versioned_fingerprint;

use crate::ReleaseError;

pub const CODE_HEALTH_MAINTENANCE_PROFILE_ID: &str = "code_health_maintenance";
pub const CODE_HEALTH_MAINTENANCE_PROFILE_VERSION: &str = "1.0.0";

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EvaluationInput {
    pub evaluation_policy_ref: EvaluationPolicyRefV1,
    pub evaluation_context: EvaluationContext,
    pub baseline: EvaluationDefinition,
    pub candidate: EvaluationDefinition,
    pub mode: EvaluationMode,
    pub corpus_ref: String,
    pub case_selection_fingerprint: Sha256Hash,
    pub measurement_protocol_fingerprint: Sha256Hash,
    pub case_results: Vec<EvaluationCaseResult>,
    pub comparability: Vec<EvaluationComparability>,
    pub protected_metric_results: Vec<ProtectedMetricResult>,
    pub minimum_sample_count: u32,
    #[serde(default)]
    pub radar_item_refs: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BudgetSnapshotInput {
    pub snapshot_id: String,
    pub revision: u64,
    pub project_id: ProjectId,
    pub scope_ref: String,
    pub cost_record_refs: Vec<CostRecordRefV1>,
    pub limits: Vec<BudgetQuantityV1>,
    #[serde(default)]
    pub reserved: Vec<BudgetQuantityV1>,
    #[serde(default)]
    pub unknown_measurements: Vec<String>,
    #[serde(default)]
    pub permission_approval_refs: Vec<String>,
    pub paid_action_pending: bool,
    pub evaluated_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ComparativeSafety {
    baseline_false_negatives: u32,
    candidate_false_negatives: u32,
    baseline_false_positives: u32,
    candidate_false_positives: u32,
    baseline_adverse_outcomes: u32,
    candidate_adverse_outcomes: u32,
    baseline_unknown_outcomes: u32,
    candidate_unknown_outcomes: u32,
    baseline_rollbacks: u32,
    candidate_rollbacks: u32,
}

pub fn evaluate(input: EvaluationInput) -> Result<EvaluationRunV2, ReleaseError> {
    validate_input(&input)?;
    let metrics = summarize(&input.case_results);
    let safety = comparative_safety(&input.case_results);
    let protected_weakened = input
        .protected_metric_results
        .iter()
        .any(|result| result.weakened);
    let not_comparable = input
        .comparability
        .iter()
        .any(|result| result.state == ComparabilityState::NotComparable);
    let cost_limited = input
        .case_results
        .iter()
        .any(|case| case.baseline_cost_refs.is_empty() || case.candidate_cost_refs.is_empty());
    let evidence_limited = input
        .case_results
        .iter()
        .any(|case| !case.limitations.is_empty())
        || cost_limited;
    let recommendation = if protected_weakened {
        EvaluationRecommendation::Reject
    } else if not_comparable
        || evidence_limited
        || metrics.sample_count < input.minimum_sample_count
        || metrics.unresolved > 0
        || safety.candidate_unknown_outcomes > 0
    {
        EvaluationRecommendation::NeedsReview
    } else if safety.candidate_false_negatives > safety.baseline_false_negatives
        || safety.candidate_false_positives > safety.baseline_false_positives
        || safety.candidate_adverse_outcomes > safety.baseline_adverse_outcomes
        || safety.candidate_rollbacks > safety.baseline_rollbacks
        || metrics.candidate_new_or_worsened_count > metrics.baseline_new_or_worsened_count
        || metrics.suppression_newly_added_count > 0
        || metrics.suppression_broadened_count > 0
        || metrics.candidate_flaky > 0
    {
        EvaluationRecommendation::Reject
    } else if candidate_improves(&metrics, safety) {
        EvaluationRecommendation::Accept
    } else if candidate_equal(&input.case_results, &metrics) {
        EvaluationRecommendation::Keep
    } else {
        EvaluationRecommendation::Trial
    };
    let mut limitations = input
        .case_results
        .iter()
        .flat_map(|case| case.limitations.iter().cloned())
        .collect::<Vec<_>>();
    if protected_weakened {
        limitations.push("protected_validator_corpus_or_profile_weakened".to_owned());
    }
    if not_comparable {
        limitations.push("baseline_candidate_not_comparable".to_owned());
    }
    if cost_limited {
        limitations.push("provider_usage_or_cost_measurement_unavailable".to_owned());
    }
    limitations.sort();
    limitations.dedup();
    let mut comparison = comparison_lines(&metrics, safety);
    let usage_and_cost_refs = input
        .case_results
        .iter()
        .flat_map(|case| {
            case.baseline_cost_refs
                .iter()
                .chain(&case.candidate_cost_refs)
                .cloned()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let usage_and_cost_metrics = summarize_usage_and_cost(&input.case_results)?;
    comparison.extend(usage_and_cost_metrics.iter().map(|metric| {
        format!(
            "usage_or_cost.{}:{}->{}",
            metric.unit, metric.baseline_quantity, metric.candidate_quantity
        )
    }));
    let mut radar_item_refs = input.radar_item_refs;
    radar_item_refs.sort();
    radar_item_refs.dedup();
    let mut run = EvaluationRunV2 {
        schema_id: EVALUATION_RUN_V2_SCHEMA_ID.to_owned(),
        schema_version: 2,
        evaluation_run_id: EvaluationRunId::new(),
        subject_kind: input.candidate.subject.kind,
        subject: input.candidate.subject.clone(),
        evaluation_context: input.evaluation_context,
        evaluation_policy_ref: input.evaluation_policy_ref,
        baseline: input.baseline,
        candidate: input.candidate,
        mode: input.mode,
        corpus_ref: input.corpus_ref,
        case_selection_fingerprint: input.case_selection_fingerprint,
        measurement_protocol_fingerprint: input.measurement_protocol_fingerprint,
        minimum_sample_count: input.minimum_sample_count,
        case_results: input.case_results,
        ground_truth_summary: metrics.clone(),
        finding_metrics: metrics.clone(),
        efficiency_metrics: metrics,
        usage_and_cost_refs,
        usage_and_cost_metrics,
        comparability: input.comparability,
        protected_metric_results: input.protected_metric_results,
        limitations,
        comparison,
        recommendation,
        decision_ref: None,
        radar_item_refs,
        run_fingerprint: Sha256Hash::digest(b"unsealed-evaluation-run"),
    };
    run.run_fingerprint = evaluation_run_fingerprint(&run)?;
    Ok(run)
}

pub fn verify_evaluation_run(run: &EvaluationRunV2) -> Result<(), ReleaseError> {
    if run.schema_id != EVALUATION_RUN_V2_SCHEMA_ID
        || run.schema_version != 2
        || run.subject_kind != run.candidate.subject.kind
        || run.subject != run.candidate.subject
        || run.run_fingerprint != evaluation_run_fingerprint(run)?
    {
        return Err(ReleaseError::Invalid);
    }
    let expected = evaluate(EvaluationInput {
        evaluation_context: run.evaluation_context,
        evaluation_policy_ref: run.evaluation_policy_ref.clone(),
        baseline: run.baseline.clone(),
        candidate: run.candidate.clone(),
        mode: run.mode,
        corpus_ref: run.corpus_ref.clone(),
        case_selection_fingerprint: run.case_selection_fingerprint.clone(),
        measurement_protocol_fingerprint: run.measurement_protocol_fingerprint.clone(),
        case_results: run.case_results.clone(),
        comparability: run.comparability.clone(),
        protected_metric_results: run.protected_metric_results.clone(),
        minimum_sample_count: run.minimum_sample_count,
        radar_item_refs: run.radar_item_refs.clone(),
    })?;
    if run.ground_truth_summary != expected.ground_truth_summary
        || run.finding_metrics != expected.finding_metrics
        || run.efficiency_metrics != expected.efficiency_metrics
        || run.usage_and_cost_refs != expected.usage_and_cost_refs
        || run.usage_and_cost_metrics != expected.usage_and_cost_metrics
        || run.radar_item_refs != expected.radar_item_refs
        || run.limitations != expected.limitations
        || run.comparison != expected.comparison
        || run.recommendation != expected.recommendation
    {
        return Err(ReleaseError::Invalid);
    }
    Ok(())
}

fn evaluation_run_fingerprint(run: &EvaluationRunV2) -> Result<Sha256Hash, ReleaseError> {
    versioned_fingerprint(
        EVALUATION_RUN_V2_SCHEMA_ID,
        2,
        &serde_json::json!({
            "evaluation_run_id":run.evaluation_run_id,
            "subject_kind":run.subject_kind,
            "subject":run.subject,
            "evaluation_context":run.evaluation_context,
            "evaluation_policy_ref":run.evaluation_policy_ref,
            "baseline":run.baseline,
            "candidate":run.candidate,
            "mode":run.mode,
            "corpus_ref":run.corpus_ref,
            "case_selection_fingerprint":run.case_selection_fingerprint,
            "measurement_protocol_fingerprint":run.measurement_protocol_fingerprint,
            "minimum_sample_count":run.minimum_sample_count,
            "case_results":run.case_results,
            "ground_truth_summary":run.ground_truth_summary,
            "finding_metrics":run.finding_metrics,
            "efficiency_metrics":run.efficiency_metrics,
            "usage_and_cost_refs":run.usage_and_cost_refs,
            "usage_and_cost_metrics":run.usage_and_cost_metrics,
            "comparability":run.comparability,
            "protected_metric_results":run.protected_metric_results,
            "limitations":run.limitations,
            "comparison":run.comparison,
            "recommendation":run.recommendation,
            "decision_ref":run.decision_ref,
            "radar_item_refs":run.radar_item_refs,
        }),
    )
    .map_err(|_| ReleaseError::Fingerprint)
}

pub fn transition_catalog_item(
    mut item: EvaluationCatalogItem,
    next: EvaluationCatalogLifecycle,
    trial_candidate: bool,
) -> Result<EvaluationCatalogItem, ReleaseError> {
    let valid = match (item.lifecycle, next) {
        (EvaluationCatalogLifecycle::Active, EvaluationCatalogLifecycle::Deprecated) => {
            !item.trial_candidate
                && !trial_candidate
                && item.replacement_ref.is_some()
                && item.migration_guide_ref.is_some()
                && item.compatibility_deadline.is_some()
                && item.last_evaluation_run_ref.is_some()
        }
        (EvaluationCatalogLifecycle::Deprecated, EvaluationCatalogLifecycle::Retired) => {
            !item.trial_candidate
                && !trial_candidate
                && item.tombstone_ref.is_some()
                && item.migration_guide_ref.is_some()
                && item.last_evaluation_run_ref.is_some()
        }
        (EvaluationCatalogLifecycle::Active, EvaluationCatalogLifecycle::Rejected) => {
            item.trial_candidate
                && trial_candidate
                && item.tombstone_ref.is_some()
                && item.last_evaluation_run_ref.is_some()
        }
        _ => false,
    };
    if !valid {
        return Err(ReleaseError::Blocked);
    }
    item.lifecycle = next;
    seal_catalog_item(item)
}

/// Turns a verified Code Health profile evaluation into a catalog-only result.
/// This never mutates the built-in Profile set: an accepted candidate requires
/// the separately owned product decision and its complete catalog/doc package.
pub fn code_health_profile_catalog_item(
    run: &EvaluationRunV2,
) -> Result<EvaluationCatalogItem, ReleaseError> {
    verify_evaluation_run(run)?;
    if run.subject_kind != star_contracts::release_v2::EvaluationSubjectKind::Profile
        || run.candidate.subject.item_id != CODE_HEALTH_MAINTENANCE_PROFILE_ID
        || run.candidate.subject.version != CODE_HEALTH_MAINTENANCE_PROFILE_VERSION
        || !matches!(
            run.mode,
            EvaluationMode::Offline | EvaluationMode::Replay | EvaluationMode::Shadow
        )
    {
        return Err(ReleaseError::Invalid);
    }
    let (lifecycle, tombstone_ref) = match run.recommendation {
        EvaluationRecommendation::Trial | EvaluationRecommendation::NeedsReview => {
            (EvaluationCatalogLifecycle::Active, None)
        }
        EvaluationRecommendation::Keep | EvaluationRecommendation::Reject => (
            EvaluationCatalogLifecycle::Rejected,
            Some(format!("evaluation-run:{}", run.evaluation_run_id)),
        ),
        EvaluationRecommendation::Accept => return Err(ReleaseError::Blocked),
    };
    seal_catalog_item(EvaluationCatalogItem {
        schema_id: EVALUATION_CATALOG_ITEM_SCHEMA_ID.to_owned(),
        schema_version: 1,
        item_id: CODE_HEALTH_MAINTENANCE_PROFILE_ID.to_owned(),
        item_version: CODE_HEALTH_MAINTENANCE_PROFILE_VERSION.to_owned(),
        definition_fingerprint: run.candidate.subject.definition_fingerprint.clone(),
        trial_candidate: true,
        lifecycle,
        owner: "docs/contracts/code-health-and-maintainability.md".to_owned(),
        corpus_ref: run.corpus_ref.clone(),
        replacement_ref: None,
        migration_guide_ref: None,
        compatibility_deadline: None,
        last_evaluation_run_ref: Some(run.evaluation_run_id.clone()),
        tombstone_ref,
        item_fingerprint: Sha256Hash::digest(b"unsealed-code-health-profile-catalog-item"),
    })
}

pub fn seal_catalog_item(
    mut item: EvaluationCatalogItem,
) -> Result<EvaluationCatalogItem, ReleaseError> {
    if item.schema_id != EVALUATION_CATALOG_ITEM_SCHEMA_ID
        || item.schema_version != 1
        || !catalog_token(&item.item_id, 192)
        || !catalog_token(&item.item_version, 128)
        || item.owner.trim().is_empty()
        || item.corpus_ref.trim().is_empty()
        || match item.lifecycle {
            EvaluationCatalogLifecycle::Active => false,
            EvaluationCatalogLifecycle::Deprecated => {
                item.replacement_ref.is_none()
                    || item.migration_guide_ref.is_none()
                    || item.compatibility_deadline.is_none()
                    || item.last_evaluation_run_ref.is_none()
            }
            EvaluationCatalogLifecycle::Retired => {
                item.tombstone_ref.is_none()
                    || item.migration_guide_ref.is_none()
                    || item.last_evaluation_run_ref.is_none()
            }
            EvaluationCatalogLifecycle::Rejected => {
                item.tombstone_ref.is_none() || item.last_evaluation_run_ref.is_none()
            }
        }
    {
        return Err(ReleaseError::Invalid);
    }
    item.item_fingerprint = versioned_fingerprint(
        EVALUATION_CATALOG_ITEM_SCHEMA_ID,
        1,
        &serde_json::json!({
            "item_id":item.item_id,
            "item_version":item.item_version,
            "definition_fingerprint":item.definition_fingerprint,
            "trial_candidate":item.trial_candidate,
            "lifecycle":item.lifecycle,
            "owner":item.owner,
            "corpus_ref":item.corpus_ref,
            "replacement_ref":item.replacement_ref,
            "migration_guide_ref":item.migration_guide_ref,
            "compatibility_deadline":item.compatibility_deadline,
            "last_evaluation_run_ref":item.last_evaluation_run_ref,
            "tombstone_ref":item.tombstone_ref,
        }),
    )
    .map_err(|_| ReleaseError::Fingerprint)?;
    Ok(item)
}

fn catalog_token(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn validate_input(input: &EvaluationInput) -> Result<(), ReleaseError> {
    if input.corpus_ref.trim().is_empty()
        || input.case_results.is_empty()
        || input.minimum_sample_count == 0
        || !catalog_token(&input.evaluation_policy_ref.policy_id, 192)
        || !catalog_token(&input.evaluation_policy_ref.policy_version, 128)
        || input.evaluation_policy_ref.revision == 0
        || input.baseline.subject.kind != input.candidate.subject.kind
        || input.baseline.subject.item_id != input.candidate.subject.item_id
        || input.baseline.subject.item_id.trim().is_empty()
        || input.baseline.subject.version.trim().is_empty()
        || input.candidate.subject.version.trim().is_empty()
        || input.case_results.iter().any(|case| {
            case.evaluation_context != input.evaluation_context
                || case.corpus_ref != input.corpus_ref
                || case.case_id.trim().is_empty()
                || case.case_version.trim().is_empty()
                || case.case_definition_ref.case_id != case.case_id
                || case.case_definition_ref.case_version != case.case_version
                || case.baseline_run_refs.is_empty()
                || case.candidate_run_refs.is_empty()
                || case.adjudication_evidence_refs.is_empty()
                || case
                    .adjudication_evidence_refs
                    .iter()
                    .any(|reference| reference.trim().is_empty())
                || case.baseline_new_or_worsened_count > case.baseline_finding_count
                || case.candidate_new_or_worsened_count > case.candidate_finding_count
                || case.baseline_existing_debt_count > case.baseline_finding_count
                || case.candidate_existing_debt_count > case.candidate_finding_count
                || case
                    .baseline_cost_refs
                    .iter()
                    .chain(&case.candidate_cost_refs)
                    .any(|reference| {
                        !catalog_token(&reference.cost_record_id, 192) || reference.revision == 0
                    })
                || case.baseline_cost_refs.is_empty() != case.baseline_usage_and_cost.is_empty()
                || case.candidate_cost_refs.is_empty() != case.candidate_usage_and_cost.is_empty()
                || !valid_evaluation_quantities(&case.baseline_usage_and_cost)
                || !valid_evaluation_quantities(&case.candidate_usage_and_cost)
        })
    {
        return Err(ReleaseError::Invalid);
    }
    let case_ids = input
        .case_results
        .iter()
        .map(|case| (&case.case_id, &case.case_version))
        .collect::<BTreeSet<_>>();
    if case_ids.len() != input.case_results.len() {
        return Err(ReleaseError::Conflict);
    }
    for case in &input.case_results {
        let baseline = case.baseline_run_refs.iter().collect::<BTreeSet<_>>();
        let candidate = case.candidate_run_refs.iter().collect::<BTreeSet<_>>();
        let adjudication = case
            .adjudication_evidence_refs
            .iter()
            .collect::<BTreeSet<_>>();
        let baseline_cost = case.baseline_cost_refs.iter().collect::<BTreeSet<_>>();
        let candidate_cost = case.candidate_cost_refs.iter().collect::<BTreeSet<_>>();
        if baseline.len() != case.baseline_run_refs.len()
            || candidate.len() != case.candidate_run_refs.len()
            || !baseline.is_disjoint(&candidate)
            || adjudication.len() != case.adjudication_evidence_refs.len()
            || baseline_cost.len() != case.baseline_cost_refs.len()
            || candidate_cost.len() != case.candidate_cost_refs.len()
            || !baseline_cost.is_disjoint(&candidate_cost)
        {
            return Err(ReleaseError::Conflict);
        }
    }
    let all_cost_refs = input
        .case_results
        .iter()
        .flat_map(|case| {
            case.baseline_cost_refs
                .iter()
                .chain(&case.candidate_cost_refs)
        })
        .collect::<Vec<_>>();
    if all_cost_refs.iter().collect::<BTreeSet<_>>().len() != all_cost_refs.len()
        || input
            .radar_item_refs
            .iter()
            .any(|reference| reference.trim().is_empty())
        || input.radar_item_refs.iter().collect::<BTreeSet<_>>().len()
            != input.radar_item_refs.len()
    {
        return Err(ReleaseError::Conflict);
    }
    let required_dimensions = BTreeSet::from([
        "case",
        "source",
        "config",
        "catalog",
        "tool",
        "environment",
        "protocol",
    ]);
    let dimensions = input
        .comparability
        .iter()
        .map(|item| item.dimension.as_str())
        .collect::<BTreeSet<_>>();
    if dimensions != required_dimensions
        || input.comparability.len() != required_dimensions.len()
        || input
            .comparability
            .iter()
            .any(|item| item.evidence_ref.trim().is_empty())
    {
        return Err(ReleaseError::Invalid);
    }
    let protected = input
        .protected_metric_results
        .iter()
        .map(|item| item.metric_id.as_str())
        .collect::<BTreeSet<_>>();
    if input.protected_metric_results.len() != protected.len()
        || input
            .protected_metric_results
            .iter()
            .any(|item| item.evidence_ref.trim().is_empty())
        || !["validator_guard", "corpus", "profile"]
            .iter()
            .all(|required| protected.contains(required))
    {
        return Err(ReleaseError::Invalid);
    }
    Ok(())
}

fn summarize(cases: &[EvaluationCaseResult]) -> EvaluationMetricSummary {
    EvaluationMetricSummary {
        sample_count: cases.len() as u32,
        confirmed_defects: cases
            .iter()
            .filter(|case| case.adjudication == CaseAdjudication::ConfirmedDefect)
            .count() as u32,
        candidate_false_negatives: cases
            .iter()
            .filter(|case| {
                case.adjudication == CaseAdjudication::ConfirmedDefect && !case.candidate_detected
            })
            .count() as u32,
        candidate_false_positives: cases
            .iter()
            .filter(|case| {
                case.adjudication == CaseAdjudication::FalsePositive && case.candidate_detected
            })
            .count() as u32,
        unresolved: cases
            .iter()
            .filter(|case| case.adjudication == CaseAdjudication::Unresolved)
            .count() as u32,
        candidate_flaky: cases.iter().filter(|case| case.candidate_flaky).count() as u32,
        baseline_total_duration_ms: cases.iter().map(|case| case.baseline_duration_ms).sum(),
        candidate_total_duration_ms: cases.iter().map(|case| case.candidate_duration_ms).sum(),
        baseline_rework_count: cases.iter().map(|case| case.baseline_rework_count).sum(),
        candidate_rework_count: cases.iter().map(|case| case.candidate_rework_count).sum(),
        candidate_rollbacks: cases
            .iter()
            .filter(|case| case.candidate_outcome == EvaluationOutcome::Rollback)
            .count() as u32,
        baseline_finding_count: cases
            .iter()
            .map(|case| case.baseline_finding_count)
            .fold(0, u32::saturating_add),
        candidate_finding_count: cases
            .iter()
            .map(|case| case.candidate_finding_count)
            .fold(0, u32::saturating_add),
        baseline_new_or_worsened_count: cases
            .iter()
            .map(|case| case.baseline_new_or_worsened_count)
            .fold(0, u32::saturating_add),
        candidate_new_or_worsened_count: cases
            .iter()
            .map(|case| case.candidate_new_or_worsened_count)
            .fold(0, u32::saturating_add),
        baseline_existing_debt_count: cases
            .iter()
            .map(|case| case.baseline_existing_debt_count)
            .fold(0, u32::saturating_add),
        candidate_existing_debt_count: cases
            .iter()
            .map(|case| case.candidate_existing_debt_count)
            .fold(0, u32::saturating_add),
        baseline_suppressions: summarize_suppressions(
            cases.iter().map(|case| &case.baseline_suppressions),
        ),
        candidate_suppressions: summarize_suppressions(
            cases.iter().map(|case| &case.candidate_suppressions),
        ),
        suppression_newly_added_count: cases
            .iter()
            .map(|case| case.suppression_newly_added_count)
            .fold(0, u32::saturating_add),
        suppression_broadened_count: cases
            .iter()
            .map(|case| case.suppression_broadened_count)
            .fold(0, u32::saturating_add),
        suppression_removed_count: cases
            .iter()
            .map(|case| case.suppression_removed_count)
            .fold(0, u32::saturating_add),
    }
}

fn valid_evaluation_quantities(quantities: &[EvaluationQuantityV1]) -> bool {
    quantities.is_empty()
        || quantities
            .iter()
            .all(|quantity| catalog_token(&quantity.unit, 64) && quantity.quantity > 0)
            && quantities
                .iter()
                .map(|quantity| quantity.unit.as_str())
                .collect::<BTreeSet<_>>()
                .len()
                == quantities.len()
}

fn summarize_usage_and_cost(
    cases: &[EvaluationCaseResult],
) -> Result<Vec<EvaluationQuantityComparisonV1>, ReleaseError> {
    let mut totals = BTreeMap::<String, (u64, u64)>::new();
    for case in cases {
        for quantity in &case.baseline_usage_and_cost {
            let entry = totals.entry(quantity.unit.clone()).or_default();
            entry.0 = entry
                .0
                .checked_add(quantity.quantity)
                .ok_or(ReleaseError::Invalid)?;
        }
        for quantity in &case.candidate_usage_and_cost {
            let entry = totals.entry(quantity.unit.clone()).or_default();
            entry.1 = entry
                .1
                .checked_add(quantity.quantity)
                .ok_or(ReleaseError::Invalid)?;
        }
    }
    Ok(totals
        .into_iter()
        .map(
            |(unit, (baseline_quantity, candidate_quantity))| EvaluationQuantityComparisonV1 {
                unit,
                baseline_quantity,
                candidate_quantity,
            },
        )
        .collect())
}

fn summarize_suppressions<'a>(
    summaries: impl Iterator<Item = &'a EvaluationSuppressionSummary>,
) -> EvaluationSuppressionSummary {
    summaries.fold(
        EvaluationSuppressionSummary::default(),
        |mut total, item| {
            total.active = total.active.saturating_add(item.active);
            total.expired = total.expired.saturating_add(item.expired);
            total.stale = total.stale.saturating_add(item.stale);
            total.revoked = total.revoked.saturating_add(item.revoked);
            total.invalid = total.invalid.saturating_add(item.invalid);
            total
        },
    )
}

fn comparative_safety(cases: &[EvaluationCaseResult]) -> ComparativeSafety {
    ComparativeSafety {
        baseline_false_negatives: cases
            .iter()
            .filter(|case| {
                case.adjudication == CaseAdjudication::ConfirmedDefect && !case.baseline_detected
            })
            .count() as u32,
        candidate_false_negatives: cases
            .iter()
            .filter(|case| {
                case.adjudication == CaseAdjudication::ConfirmedDefect && !case.candidate_detected
            })
            .count() as u32,
        baseline_false_positives: cases
            .iter()
            .filter(|case| {
                case.adjudication == CaseAdjudication::FalsePositive && case.baseline_detected
            })
            .count() as u32,
        candidate_false_positives: cases
            .iter()
            .filter(|case| {
                case.adjudication == CaseAdjudication::FalsePositive && case.candidate_detected
            })
            .count() as u32,
        baseline_adverse_outcomes: cases
            .iter()
            .filter(|case| adverse_outcome(case.baseline_outcome))
            .count() as u32,
        candidate_adverse_outcomes: cases
            .iter()
            .filter(|case| adverse_outcome(case.candidate_outcome))
            .count() as u32,
        baseline_unknown_outcomes: cases
            .iter()
            .filter(|case| case.baseline_outcome == EvaluationOutcome::Unknown)
            .count() as u32,
        candidate_unknown_outcomes: cases
            .iter()
            .filter(|case| case.candidate_outcome == EvaluationOutcome::Unknown)
            .count() as u32,
        baseline_rollbacks: cases
            .iter()
            .filter(|case| case.baseline_outcome == EvaluationOutcome::Rollback)
            .count() as u32,
        candidate_rollbacks: cases
            .iter()
            .filter(|case| case.candidate_outcome == EvaluationOutcome::Rollback)
            .count() as u32,
    }
}

fn adverse_outcome(outcome: EvaluationOutcome) -> bool {
    matches!(
        outcome,
        EvaluationOutcome::Failure | EvaluationOutcome::Rejected | EvaluationOutcome::Reverted
    )
}

fn candidate_improves(metrics: &EvaluationMetricSummary, safety: ComparativeSafety) -> bool {
    let safety_improved = safety.candidate_false_negatives < safety.baseline_false_negatives
        || safety.candidate_false_positives < safety.baseline_false_positives
        || safety.candidate_adverse_outcomes < safety.baseline_adverse_outcomes
        || safety.candidate_unknown_outcomes < safety.baseline_unknown_outcomes
        || safety.candidate_rollbacks < safety.baseline_rollbacks;
    let safety_equal = safety.candidate_false_negatives == safety.baseline_false_negatives
        && safety.candidate_false_positives == safety.baseline_false_positives
        && safety.candidate_adverse_outcomes == safety.baseline_adverse_outcomes
        && safety.candidate_unknown_outcomes == safety.baseline_unknown_outcomes
        && safety.candidate_rollbacks == safety.baseline_rollbacks;
    safety_improved
        || metrics.candidate_new_or_worsened_count < metrics.baseline_new_or_worsened_count
        || metrics.suppression_removed_count > 0
        || safety_equal
            && ((metrics.candidate_total_duration_ms < metrics.baseline_total_duration_ms
                && metrics.candidate_rework_count <= metrics.baseline_rework_count)
                || (metrics.candidate_rework_count < metrics.baseline_rework_count
                    && metrics.candidate_total_duration_ms <= metrics.baseline_total_duration_ms))
}

fn candidate_equal(cases: &[EvaluationCaseResult], metrics: &EvaluationMetricSummary) -> bool {
    cases.iter().all(|case| {
        case.baseline_detected == case.candidate_detected
            && case.baseline_outcome == case.candidate_outcome
            && !case.candidate_flaky
    }) && metrics.candidate_total_duration_ms == metrics.baseline_total_duration_ms
        && metrics.candidate_rework_count == metrics.baseline_rework_count
        && metrics.candidate_finding_count == metrics.baseline_finding_count
        && metrics.candidate_new_or_worsened_count == metrics.baseline_new_or_worsened_count
        && metrics.candidate_existing_debt_count == metrics.baseline_existing_debt_count
        && metrics.candidate_suppressions == metrics.baseline_suppressions
        && metrics.suppression_newly_added_count == 0
        && metrics.suppression_broadened_count == 0
        && metrics.suppression_removed_count == 0
}

fn comparison_lines(metrics: &EvaluationMetricSummary, safety: ComparativeSafety) -> Vec<String> {
    vec![
        format!(
            "confirmed_false_negative:{}->{}",
            safety.baseline_false_negatives, safety.candidate_false_negatives
        ),
        format!(
            "false_positive:{}->{}",
            safety.baseline_false_positives, safety.candidate_false_positives
        ),
        format!(
            "adverse_outcome:{}->{}",
            safety.baseline_adverse_outcomes, safety.candidate_adverse_outcomes
        ),
        format!(
            "unknown_outcome:{}->{}",
            safety.baseline_unknown_outcomes, safety.candidate_unknown_outcomes
        ),
        format!(
            "rollback:{}->{}",
            safety.baseline_rollbacks, safety.candidate_rollbacks
        ),
        format!(
            "duration_ms:{}->{}",
            metrics.baseline_total_duration_ms, metrics.candidate_total_duration_ms
        ),
        format!(
            "rework:{}->{}",
            metrics.baseline_rework_count, metrics.candidate_rework_count
        ),
        format!(
            "finding:{}->{}",
            metrics.baseline_finding_count, metrics.candidate_finding_count
        ),
        format!(
            "new_or_worsened:{}->{}",
            metrics.baseline_new_or_worsened_count, metrics.candidate_new_or_worsened_count
        ),
        format!(
            "active_suppression:{}->{}",
            metrics.baseline_suppressions.active, metrics.candidate_suppressions.active
        ),
        format!(
            "suppression_delta:new={},broadened={},removed={}",
            metrics.suppression_newly_added_count,
            metrics.suppression_broadened_count,
            metrics.suppression_removed_count
        ),
    ]
}

pub fn seal_evaluation_case_definition(
    mut definition: EvaluationCaseDefinitionV1,
) -> Result<EvaluationCaseDefinitionV1, ReleaseError> {
    definition.ground_truth_evidence_refs.sort();
    definition.ground_truth_evidence_refs.dedup();
    if definition.schema_id != EVALUATION_CASE_DEFINITION_V1_SCHEMA_ID
        || definition.schema_version != 1
        || definition.revision == 0
        || !catalog_token(&definition.case_id, 192)
        || !catalog_token(&definition.case_version, 128)
        || definition.corpus_ref.trim().is_empty()
        || definition.corpus_ref.len() > 1_024
        || definition.ground_truth_evidence_refs.is_empty()
        || definition
            .ground_truth_evidence_refs
            .iter()
            .any(|reference| !bounded_ref(reference))
        || !canonical_source_ref(&definition.source_ref)
    {
        return Err(ReleaseError::Invalid);
    }
    definition.content_fingerprint = versioned_fingerprint(
        EVALUATION_CASE_DEFINITION_V1_SCHEMA_ID,
        1,
        &serde_json::json!({
            "revision":definition.revision,
            "project_id":definition.project_id,
            "case_id":definition.case_id,
            "case_version":definition.case_version,
            "corpus_ref":definition.corpus_ref,
            "evaluation_context":definition.evaluation_context,
            "adjudication":definition.adjudication,
            "ground_truth_evidence_refs":definition.ground_truth_evidence_refs,
            "source_ref":definition.source_ref,
        }),
    )
    .map_err(|_| ReleaseError::Fingerprint)?;
    Ok(definition)
}

pub fn verify_evaluation_case_definition(
    definition: &EvaluationCaseDefinitionV1,
) -> Result<(), ReleaseError> {
    let expected = definition.content_fingerprint.clone();
    let sealed = seal_evaluation_case_definition(definition.clone())?;
    if sealed != *definition || sealed.content_fingerprint != expected {
        return Err(ReleaseError::Invalid);
    }
    Ok(())
}

pub fn seal_evaluation_policy(
    mut policy: EvaluationPolicyV1,
) -> Result<EvaluationPolicyV1, ReleaseError> {
    policy.case_refs.sort();
    policy.comparability_dimensions.sort();
    policy.comparability_dimensions.dedup();
    policy.protected_metric_ids.sort();
    policy.protected_metric_ids.dedup();
    let case_identities = policy
        .case_refs
        .iter()
        .map(|reference| (&reference.case_id, &reference.case_version))
        .collect::<BTreeSet<_>>();
    let required_dimensions = BTreeSet::from([
        "case",
        "source",
        "config",
        "catalog",
        "tool",
        "environment",
        "protocol",
    ]);
    let dimensions = policy
        .comparability_dimensions
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let protected = policy
        .protected_metric_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if policy.schema_id != EVALUATION_POLICY_V1_SCHEMA_ID
        || policy.schema_version != 1
        || policy.revision == 0
        || !catalog_token(&policy.policy_id, 192)
        || !catalog_token(&policy.policy_version, 128)
        || policy.corpus_ref.trim().is_empty()
        || policy.corpus_ref.len() > 1_024
        || policy.case_refs.is_empty()
        || case_identities.len() != policy.case_refs.len()
        || policy.case_refs.iter().any(|reference| {
            !catalog_token(&reference.case_id, 192) || !catalog_token(&reference.case_version, 128)
        })
        || policy.minimum_sample_count == 0
        || usize::try_from(policy.minimum_sample_count)
            .ok()
            .is_none_or(|minimum| minimum > policy.case_refs.len())
        || !(1..=100).contains(&policy.max_attempts_per_case)
        || dimensions != required_dimensions
        || !["validator_guard", "corpus", "profile"]
            .iter()
            .all(|required| protected.contains(required))
        || !policy.require_provider_cost
        || !canonical_source_ref(&policy.source_ref)
    {
        return Err(ReleaseError::Invalid);
    }
    policy.content_fingerprint = versioned_fingerprint(
        EVALUATION_POLICY_V1_SCHEMA_ID,
        1,
        &serde_json::json!({
            "revision":policy.revision,
            "project_id":policy.project_id,
            "policy_id":policy.policy_id,
            "policy_version":policy.policy_version,
            "subject_kind":policy.subject_kind,
            "evaluation_context":policy.evaluation_context,
            "mode":policy.mode,
            "corpus_ref":policy.corpus_ref,
            "case_refs":policy.case_refs,
            "minimum_sample_count":policy.minimum_sample_count,
            "max_attempts_per_case":policy.max_attempts_per_case,
            "comparability_dimensions":policy.comparability_dimensions,
            "protected_metric_ids":policy.protected_metric_ids,
            "require_provider_cost":policy.require_provider_cost,
            "source_ref":policy.source_ref,
        }),
    )
    .map_err(|_| ReleaseError::Fingerprint)?;
    Ok(policy)
}

pub fn verify_evaluation_policy(policy: &EvaluationPolicyV1) -> Result<(), ReleaseError> {
    let expected = policy.content_fingerprint.clone();
    let sealed = seal_evaluation_policy(policy.clone())?;
    if sealed != *policy || sealed.content_fingerprint != expected {
        return Err(ReleaseError::Invalid);
    }
    Ok(())
}

pub fn seal_cost_record(mut record: CostRecordV1) -> Result<CostRecordV1, ReleaseError> {
    record.validation_run_refs.sort();
    record.validation_run_refs.dedup();
    record
        .usage
        .sort_by(|left, right| left.unit.cmp(&right.unit));
    record.measurement_unavailable.sort();
    record.measurement_unavailable.dedup();
    record.provider_evidence_refs.sort();
    record.provider_evidence_refs.dedup();
    let usage_units = record
        .usage
        .iter()
        .map(|usage| usage.unit.as_str())
        .collect::<BTreeSet<_>>();
    let monetary_valid = record.monetary_cost.as_ref().is_none_or(|cost| {
        cost.amount_microunits > 0
            && cost.currency.len() == 3
            && cost.currency.bytes().all(|byte| byte.is_ascii_uppercase())
            && bounded_ref(&cost.price_source_ref)
            && bounded_ref(&cost.provider_statement_ref)
    });
    if record.schema_id != COST_RECORD_V1_SCHEMA_ID
        || record.schema_version != 1
        || !catalog_token(&record.cost_record_id, 192)
        || record.revision == 0
        || record.scope_ref.trim().is_empty()
        || record.scope_ref.len() > 1_024
        || !catalog_token(&record.source, 128)
        || record.estimated
        || (record.usage.is_empty() && record.monetary_cost.is_none())
        || usage_units.len() != record.usage.len()
        || record.usage.iter().any(|usage| {
            !catalog_token(&usage.unit, 64)
                || usage.quantity == 0
                || !bounded_ref(&usage.provider_evidence_ref)
        })
        || !monetary_valid
        || record.provider_evidence_refs.is_empty()
        || record
            .provider_evidence_refs
            .iter()
            .any(|reference| !bounded_ref(reference))
        || record
            .measurement_unavailable
            .iter()
            .any(|reason| reason.trim().is_empty() || reason.len() > 512)
        || matches!(
            record.scope_kind,
            star_contracts::release_v2::CostScopeKindV1::ValidationRun
                | star_contracts::release_v2::CostScopeKindV1::Evaluation
        ) && record.validation_run_refs.is_empty()
    {
        return Err(ReleaseError::Invalid);
    }
    record.content_fingerprint = versioned_fingerprint(
        COST_RECORD_V1_SCHEMA_ID,
        1,
        &serde_json::json!({
            "cost_record_id":record.cost_record_id,
            "revision":record.revision,
            "project_id":record.project_id,
            "scope_kind":record.scope_kind,
            "scope_ref":record.scope_ref,
            "validation_run_refs":record.validation_run_refs,
            "source":record.source,
            "usage":record.usage,
            "monetary_cost":record.monetary_cost,
            "estimated":record.estimated,
            "paid_action":record.paid_action,
            "measured_at":record.measured_at,
            "measurement_unavailable":record.measurement_unavailable,
            "provider_evidence_refs":record.provider_evidence_refs,
        }),
    )
    .map_err(|_| ReleaseError::Fingerprint)?;
    Ok(record)
}

pub fn verify_cost_record(record: &CostRecordV1) -> Result<(), ReleaseError> {
    let expected = record.content_fingerprint.clone();
    let sealed = seal_cost_record(record.clone())?;
    if sealed != *record || sealed.content_fingerprint != expected {
        return Err(ReleaseError::Invalid);
    }
    Ok(())
}

pub fn build_budget_snapshot(
    mut input: BudgetSnapshotInput,
    cost_records: &[CostRecordV1],
    config_fingerprint: Sha256Hash,
) -> Result<BudgetSnapshotV1, ReleaseError> {
    if !catalog_token(&input.snapshot_id, 192)
        || input.revision == 0
        || input.scope_ref.trim().is_empty()
        || input.scope_ref.len() > 1_024
        || input.limits.is_empty()
    {
        return Err(ReleaseError::Invalid);
    }
    normalize_quantities(&mut input.limits, false)?;
    normalize_quantities(&mut input.reserved, true)?;
    input.unknown_measurements.sort();
    input.unknown_measurements.dedup();
    input.permission_approval_refs.sort();
    input.permission_approval_refs.dedup();
    input.cost_record_refs.sort();
    if input
        .cost_record_refs
        .windows(2)
        .any(|pair| pair[0] == pair[1])
        || input.cost_record_refs.iter().any(|reference| {
            !catalog_token(&reference.cost_record_id, 192) || reference.revision == 0
        })
    {
        return Err(ReleaseError::Conflict);
    }
    if input
        .unknown_measurements
        .iter()
        .any(|reason| reason.trim().is_empty() || reason.len() > 512)
        || input
            .permission_approval_refs
            .iter()
            .any(|reference| !bounded_ref(reference))
    {
        return Err(ReleaseError::Invalid);
    }

    let mut observed = BTreeMap::<String, u64>::new();
    let mut refs = Vec::new();
    let mut seen = BTreeSet::new();
    for record in cost_records {
        verify_cost_record(record)?;
        if record.project_id != input.project_id
            || !seen.insert((record.cost_record_id.clone(), record.revision))
        {
            return Err(ReleaseError::Conflict);
        }
        for usage in &record.usage {
            add_quantity(&mut observed, &usage.unit, usage.quantity)?;
        }
        if let Some(cost) = &record.monetary_cost {
            add_quantity(
                &mut observed,
                &format!("money_{}_microunits", cost.currency.to_ascii_lowercase()),
                cost.amount_microunits,
            )?;
        }
        input
            .unknown_measurements
            .extend(record.measurement_unavailable.iter().cloned());
        refs.push(cost_record_ref(record));
    }
    input.unknown_measurements.sort();
    input.unknown_measurements.dedup();
    refs.sort();
    if refs != input.cost_record_refs {
        return Err(ReleaseError::Invalid);
    }

    let limits = input
        .limits
        .iter()
        .map(|quantity| (quantity.unit.clone(), quantity.quantity))
        .collect::<BTreeMap<_, _>>();
    let reserved = input
        .reserved
        .iter()
        .map(|quantity| (quantity.unit.clone(), quantity.quantity))
        .collect::<BTreeMap<_, _>>();
    for unit in observed.keys().chain(reserved.keys()) {
        if !limits.contains_key(unit) {
            input
                .unknown_measurements
                .push(format!("limit_missing:{unit}"));
        }
    }
    input.unknown_measurements.sort();
    input.unknown_measurements.dedup();

    let mut exhausted = false;
    let remaining = limits
        .iter()
        .map(|(unit, limit)| {
            let used = observed.get(unit).copied().unwrap_or_default();
            let held = reserved.get(unit).copied().unwrap_or_default();
            let committed = used.checked_add(held).ok_or(ReleaseError::Invalid)?;
            exhausted |= committed > *limit;
            Ok(BudgetQuantityV1 {
                unit: unit.clone(),
                quantity: limit.saturating_sub(committed),
            })
        })
        .collect::<Result<Vec<_>, ReleaseError>>()?;
    let decision = if exhausted {
        BudgetDecisionV1::Exhausted
    } else if !input.unknown_measurements.is_empty() {
        BudgetDecisionV1::Unknown
    } else if input.paid_action_pending && input.permission_approval_refs.is_empty() {
        BudgetDecisionV1::ApprovalRequired
    } else {
        BudgetDecisionV1::WithinBudget
    };
    let observed = observed
        .into_iter()
        .map(|(unit, quantity)| BudgetQuantityV1 { unit, quantity })
        .collect::<Vec<_>>();
    let mut snapshot = BudgetSnapshotV1 {
        schema_id: BUDGET_SNAPSHOT_V1_SCHEMA_ID.to_owned(),
        schema_version: 1,
        snapshot_id: input.snapshot_id,
        revision: input.revision,
        project_id: input.project_id,
        scope_ref: input.scope_ref,
        limits: input.limits,
        observed,
        reserved: input.reserved,
        remaining,
        unknown_measurements: input.unknown_measurements,
        decision,
        cost_record_refs: refs,
        permission_approval_refs: input.permission_approval_refs,
        paid_action_pending: input.paid_action_pending,
        config_fingerprint,
        evaluated_at: input.evaluated_at,
        content_fingerprint: Sha256Hash::digest(b"unsealed-budget-snapshot"),
    };
    snapshot.content_fingerprint = budget_snapshot_fingerprint(&snapshot)?;
    Ok(snapshot)
}

pub fn verify_budget_snapshot(
    snapshot: &BudgetSnapshotV1,
    cost_records: &[CostRecordV1],
) -> Result<(), ReleaseError> {
    if snapshot.schema_id != BUDGET_SNAPSHOT_V1_SCHEMA_ID
        || snapshot.schema_version != 1
        || snapshot.content_fingerprint != budget_snapshot_fingerprint(snapshot)?
    {
        return Err(ReleaseError::Invalid);
    }
    let expected = build_budget_snapshot(
        BudgetSnapshotInput {
            snapshot_id: snapshot.snapshot_id.clone(),
            revision: snapshot.revision,
            project_id: snapshot.project_id.clone(),
            scope_ref: snapshot.scope_ref.clone(),
            cost_record_refs: snapshot.cost_record_refs.clone(),
            limits: snapshot.limits.clone(),
            reserved: snapshot.reserved.clone(),
            unknown_measurements: snapshot
                .unknown_measurements
                .iter()
                .filter(|reason| !reason.starts_with("limit_missing:"))
                .cloned()
                .collect(),
            permission_approval_refs: snapshot.permission_approval_refs.clone(),
            paid_action_pending: snapshot.paid_action_pending,
            evaluated_at: snapshot.evaluated_at,
        },
        cost_records,
        snapshot.config_fingerprint.clone(),
    )?;
    if expected != *snapshot {
        return Err(ReleaseError::Invalid);
    }
    Ok(())
}

fn budget_snapshot_fingerprint(snapshot: &BudgetSnapshotV1) -> Result<Sha256Hash, ReleaseError> {
    versioned_fingerprint(
        BUDGET_SNAPSHOT_V1_SCHEMA_ID,
        1,
        &serde_json::json!({
            "snapshot_id":snapshot.snapshot_id,
            "revision":snapshot.revision,
            "project_id":snapshot.project_id,
            "scope_ref":snapshot.scope_ref,
            "limits":snapshot.limits,
            "observed":snapshot.observed,
            "reserved":snapshot.reserved,
            "remaining":snapshot.remaining,
            "unknown_measurements":snapshot.unknown_measurements,
            "decision":snapshot.decision,
            "cost_record_refs":snapshot.cost_record_refs,
            "permission_approval_refs":snapshot.permission_approval_refs,
            "paid_action_pending":snapshot.paid_action_pending,
            "config_fingerprint":snapshot.config_fingerprint,
            "evaluated_at":snapshot.evaluated_at,
        }),
    )
    .map_err(|_| ReleaseError::Fingerprint)
}

fn normalize_quantities(
    quantities: &mut [BudgetQuantityV1],
    allow_zero: bool,
) -> Result<(), ReleaseError> {
    quantities.sort_by(|left, right| left.unit.cmp(&right.unit));
    if quantities.iter().any(|quantity| {
        !catalog_token(&quantity.unit, 64) || (!allow_zero && quantity.quantity == 0)
    }) || quantities
        .windows(2)
        .any(|pair| pair[0].unit == pair[1].unit)
    {
        return Err(ReleaseError::Invalid);
    }
    Ok(())
}

fn add_quantity(
    totals: &mut BTreeMap<String, u64>,
    unit: &str,
    quantity: u64,
) -> Result<(), ReleaseError> {
    let current = totals.get(unit).copied().unwrap_or_default();
    totals.insert(
        unit.to_owned(),
        current.checked_add(quantity).ok_or(ReleaseError::Invalid)?,
    );
    Ok(())
}

fn cost_record_ref(record: &CostRecordV1) -> CostRecordRefV1 {
    CostRecordRefV1 {
        cost_record_id: record.cost_record_id.clone(),
        revision: record.revision,
        content_fingerprint: record.content_fingerprint.clone(),
    }
}

fn bounded_ref(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 2_048 && !value.contains('\0')
}

fn canonical_source_ref(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 1_024
        && !value.contains('\0')
        && !value.contains('\\')
        && !value.starts_with('/')
        && !value
            .split('/')
            .any(|segment| segment.is_empty() || segment == "..")
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::{
        profile::BUILTIN_DEVELOPMENT_PROFILE_IDS,
        release_v2::{EvaluationCatalogLifecycle, EvaluationSubject, EvaluationSubjectKind},
    };

    fn definition(version: &str) -> EvaluationDefinition {
        EvaluationDefinition {
            subject: EvaluationSubject {
                kind: EvaluationSubjectKind::Check,
                item_id: "star.check.release".to_owned(),
                version: version.to_owned(),
                definition_fingerprint: Sha256Hash::digest(version.as_bytes()),
            },
            resolved_closure_fingerprint: Sha256Hash::digest(
                format!("closure-{version}").as_bytes(),
            ),
            policy_fingerprint: Sha256Hash::digest(b"policy"),
        }
    }

    fn code_health_profile_definition(version: &str) -> EvaluationDefinition {
        EvaluationDefinition {
            subject: EvaluationSubject {
                kind: EvaluationSubjectKind::Profile,
                item_id: CODE_HEALTH_MAINTENANCE_PROFILE_ID.to_owned(),
                version: version.to_owned(),
                definition_fingerprint: Sha256Hash::digest(
                    format!("code-health-profile-{version}").as_bytes(),
                ),
            },
            resolved_closure_fingerprint: Sha256Hash::digest(
                format!("code-health-closure-{version}").as_bytes(),
            ),
            policy_fingerprint: Sha256Hash::digest(b"code-health-profile-policy"),
        }
    }

    fn case(id: &str, baseline_detected: bool, candidate_detected: bool) -> EvaluationCaseResult {
        EvaluationCaseResult {
            case_id: id.to_owned(),
            case_version: "1".to_owned(),
            corpus_ref: "evals/corpus/v1".to_owned(),
            evaluation_context: EvaluationContext::CliOnly,
            case_definition_ref: star_contracts::release_v2::EvaluationCaseDefinitionRefV1 {
                case_id: id.to_owned(),
                case_version: "1".to_owned(),
                content_fingerprint: Sha256Hash::digest(format!("case-definition-{id}").as_bytes()),
            },
            task_source_binding: Sha256Hash::digest(id.as_bytes()),
            baseline_run_refs: vec![star_contracts::ValidationRunId::new()],
            candidate_run_refs: vec![star_contracts::ValidationRunId::new()],
            adjudication: CaseAdjudication::ConfirmedDefect,
            adjudication_evidence_refs: vec![format!("adjudication:{id}")],
            baseline_detected,
            candidate_detected,
            baseline_duration_ms: 100,
            candidate_duration_ms: 80,
            baseline_rework_count: 1,
            candidate_rework_count: 0,
            baseline_outcome: EvaluationOutcome::Success,
            candidate_outcome: EvaluationOutcome::Success,
            candidate_flaky: false,
            baseline_finding_count: u32::from(baseline_detected),
            candidate_finding_count: u32::from(candidate_detected),
            baseline_new_or_worsened_count: 0,
            candidate_new_or_worsened_count: 0,
            baseline_existing_debt_count: 0,
            candidate_existing_debt_count: 0,
            baseline_suppressions: EvaluationSuppressionSummary::default(),
            candidate_suppressions: EvaluationSuppressionSummary::default(),
            suppression_newly_added_count: 0,
            suppression_broadened_count: 0,
            suppression_removed_count: 0,
            baseline_cost_refs: vec![CostRecordRefV1 {
                cost_record_id: format!("cost-{id}-baseline"),
                revision: 1,
                content_fingerprint: Sha256Hash::digest(format!("cost-{id}-baseline").as_bytes()),
            }],
            candidate_cost_refs: vec![CostRecordRefV1 {
                cost_record_id: format!("cost-{id}-candidate"),
                revision: 1,
                content_fingerprint: Sha256Hash::digest(format!("cost-{id}-candidate").as_bytes()),
            }],
            baseline_usage_and_cost: vec![EvaluationQuantityV1 {
                unit: "tokens".to_owned(),
                quantity: 100,
            }],
            candidate_usage_and_cost: vec![EvaluationQuantityV1 {
                unit: "tokens".to_owned(),
                quantity: 80,
            }],
            limitations: vec![],
        }
    }

    fn input() -> EvaluationInput {
        EvaluationInput {
            evaluation_policy_ref: EvaluationPolicyRefV1 {
                policy_id: "release-evaluation".to_owned(),
                policy_version: "1".to_owned(),
                revision: 1,
                content_fingerprint: Sha256Hash::digest(b"evaluation-policy"),
            },
            evaluation_context: EvaluationContext::CliOnly,
            baseline: definition("1.0.0"),
            candidate: definition("1.1.0"),
            mode: EvaluationMode::Replay,
            corpus_ref: "evals/corpus/v1".to_owned(),
            case_selection_fingerprint: Sha256Hash::digest(b"selection"),
            measurement_protocol_fingerprint: Sha256Hash::digest(b"protocol"),
            case_results: vec![
                case("case-1", false, true),
                case("case-2", true, true),
                case("case-3", true, true),
            ],
            comparability: [
                "case",
                "source",
                "config",
                "catalog",
                "tool",
                "environment",
                "protocol",
            ]
            .into_iter()
            .map(|dimension| EvaluationComparability {
                dimension: dimension.to_owned(),
                state: ComparabilityState::Compatible,
                evidence_ref: format!("{dimension}-binding"),
            })
            .collect(),
            protected_metric_results: ["validator_guard", "corpus", "profile"]
                .into_iter()
                .map(|metric_id| ProtectedMetricResult {
                    metric_id: metric_id.to_owned(),
                    weakened: false,
                    evidence_ref: format!("{metric_id}-comparison"),
                })
                .collect(),
            minimum_sample_count: 3,
            radar_item_refs: vec![],
        }
    }

    #[test]
    fn code_health_profile_trial_keeps_the_sixteen_builtin_profiles_unchanged() {
        let mut input = input();
        input.mode = EvaluationMode::Shadow;
        input.baseline = code_health_profile_definition("0.9.0");
        input.candidate = code_health_profile_definition(CODE_HEALTH_MAINTENANCE_PROFILE_VERSION);
        for case in &mut input.case_results {
            case.baseline_cost_refs.clear();
            case.candidate_cost_refs.clear();
            case.baseline_usage_and_cost.clear();
            case.candidate_usage_and_cost.clear();
            case.limitations = vec!["external_provider_cost_unavailable".to_owned()];
        }
        let run = evaluate(input).unwrap();
        assert_eq!(run.recommendation, EvaluationRecommendation::NeedsReview);
        let item = code_health_profile_catalog_item(&run).unwrap();
        assert_eq!(item.lifecycle, EvaluationCatalogLifecycle::Active);
        assert!(item.trial_candidate);
        assert_eq!(item.last_evaluation_run_ref, Some(run.evaluation_run_id));
        assert_eq!(BUILTIN_DEVELOPMENT_PROFILE_IDS.len(), 16);
        assert!(!BUILTIN_DEVELOPMENT_PROFILE_IDS.contains(&CODE_HEALTH_MAINTENANCE_PROFILE_ID));
    }

    #[test]
    fn code_health_profile_accept_requires_a_separate_product_decision() {
        let mut input = input();
        input.baseline = code_health_profile_definition("0.9.0");
        input.candidate = code_health_profile_definition(CODE_HEALTH_MAINTENANCE_PROFILE_VERSION);
        let run = evaluate(input).unwrap();
        assert_eq!(run.recommendation, EvaluationRecommendation::Accept);
        assert_eq!(
            code_health_profile_catalog_item(&run),
            Err(ReleaseError::Blocked)
        );
    }

    #[test]
    fn code_health_profile_worsened_false_positive_is_rejected() {
        let mut input = input();
        input.baseline = code_health_profile_definition("0.9.0");
        input.candidate = code_health_profile_definition(CODE_HEALTH_MAINTENANCE_PROFILE_VERSION);
        input.case_results[0].adjudication = CaseAdjudication::FalsePositive;
        let run = evaluate(input).unwrap();
        assert_eq!(run.recommendation, EvaluationRecommendation::Reject);
        let item = code_health_profile_catalog_item(&run).unwrap();
        assert_eq!(item.lifecycle, EvaluationCatalogLifecycle::Rejected);
        assert_eq!(
            item.tombstone_ref,
            Some(format!("evaluation-run:{}", run.evaluation_run_id))
        );
    }

    #[test]
    fn code_health_profile_trial_is_replay_deterministic() {
        let mut input = input();
        input.mode = EvaluationMode::Replay;
        input.baseline = code_health_profile_definition("0.9.0");
        input.candidate = code_health_profile_definition(CODE_HEALTH_MAINTENANCE_PROFILE_VERSION);
        for case in &mut input.case_results {
            case.baseline_cost_refs.clear();
            case.candidate_cost_refs.clear();
            case.baseline_usage_and_cost.clear();
            case.candidate_usage_and_cost.clear();
        }
        let run = evaluate(input).unwrap();
        let first = code_health_profile_catalog_item(&run).unwrap();
        let second = code_health_profile_catalog_item(&run).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.lifecycle, EvaluationCatalogLifecycle::Active);
        assert!(first.trial_candidate);
    }

    #[test]
    fn precommitted_case_and_policy_reject_drift_or_weakened_cost_evidence() {
        let project_id = ProjectId::new();
        let definition = seal_evaluation_case_definition(EvaluationCaseDefinitionV1 {
            schema_id: EVALUATION_CASE_DEFINITION_V1_SCHEMA_ID.to_owned(),
            schema_version: 1,
            revision: 1,
            project_id: project_id.clone(),
            case_id: "positive-case-1".to_owned(),
            case_version: "1.0.0".to_owned(),
            corpus_ref: "evals/corpus/v1".to_owned(),
            evaluation_context: EvaluationContext::CliOnly,
            adjudication: CaseAdjudication::ConfirmedDefect,
            ground_truth_evidence_refs: vec!["corpus:positive-case-1".to_owned()],
            source_ref: "evals/cases/positive-case-1.json".to_owned(),
            content_fingerprint: Sha256Hash::digest(b"unsealed-case"),
        })
        .unwrap();
        verify_evaluation_case_definition(&definition).unwrap();
        let mut drifted_definition = definition.clone();
        drifted_definition.adjudication = CaseAdjudication::FalsePositive;
        assert!(verify_evaluation_case_definition(&drifted_definition).is_err());

        let policy = seal_evaluation_policy(EvaluationPolicyV1 {
            schema_id: EVALUATION_POLICY_V1_SCHEMA_ID.to_owned(),
            schema_version: 1,
            revision: 1,
            project_id,
            policy_id: "release-evaluation".to_owned(),
            policy_version: "1.0.0".to_owned(),
            subject_kind: EvaluationSubjectKind::Check,
            evaluation_context: EvaluationContext::CliOnly,
            mode: EvaluationMode::Replay,
            corpus_ref: "evals/corpus/v1".to_owned(),
            case_refs: vec![star_contracts::release_v2::EvaluationCaseDefinitionRefV1 {
                case_id: definition.case_id.clone(),
                case_version: definition.case_version.clone(),
                content_fingerprint: definition.content_fingerprint.clone(),
            }],
            minimum_sample_count: 1,
            max_attempts_per_case: 3,
            comparability_dimensions: [
                "case",
                "source",
                "config",
                "catalog",
                "tool",
                "environment",
                "protocol",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            protected_metric_ids: ["validator_guard", "corpus", "profile"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            require_provider_cost: true,
            source_ref: "evals/policies/release-evaluation.json".to_owned(),
            content_fingerprint: Sha256Hash::digest(b"unsealed-policy"),
        })
        .unwrap();
        verify_evaluation_policy(&policy).unwrap();

        let mut drifted_policy = policy.clone();
        drifted_policy.max_attempts_per_case += 1;
        assert!(verify_evaluation_policy(&drifted_policy).is_err());
        let mut weakened_policy = policy;
        weakened_policy.require_provider_cost = false;
        assert!(seal_evaluation_policy(weakened_policy).is_err());
    }

    #[test]
    fn comparable_candidate_with_more_detected_defects_and_less_rework_is_accepted() {
        let run = evaluate(input()).unwrap();
        verify_evaluation_run(&run).unwrap();
        assert_eq!(run.recommendation, EvaluationRecommendation::Accept);
        assert_eq!(run.ground_truth_summary.candidate_false_negatives, 0);
        assert!(
            run.efficiency_metrics.candidate_total_duration_ms
                < run.efficiency_metrics.baseline_total_duration_ms
        );
    }

    #[test]
    fn stored_evaluation_run_rejects_recommendation_input_or_metric_tampering() {
        let run = evaluate(input()).unwrap();

        let mut duration_tampered = run.clone();
        duration_tampered.case_results[0].candidate_duration_ms += 1;
        assert!(verify_evaluation_run(&duration_tampered).is_err());

        let mut recommendation_tampered = run.clone();
        recommendation_tampered.recommendation = EvaluationRecommendation::Reject;
        assert!(verify_evaluation_run(&recommendation_tampered).is_err());

        let mut sample_floor_tampered = run;
        sample_floor_tampered.minimum_sample_count += 1;
        assert!(verify_evaluation_run(&sample_floor_tampered).is_err());
    }

    #[test]
    fn adjudication_without_evidence_is_invalid() {
        let mut candidate = input();
        candidate.case_results[0].adjudication_evidence_refs.clear();
        assert!(matches!(evaluate(candidate), Err(ReleaseError::Invalid)));
    }

    #[test]
    fn existing_baseline_miss_does_not_block_a_non_worsening_candidate() {
        let mut candidate = input();
        candidate.minimum_sample_count = 1;
        candidate.case_results = vec![case("existing-miss", false, false)];
        let run = evaluate(candidate).unwrap();
        assert_eq!(run.recommendation, EvaluationRecommendation::Accept);
        assert!(
            run.comparison
                .contains(&"confirmed_false_negative:1->1".to_owned())
        );
    }

    #[test]
    fn faster_candidate_with_worsened_false_positive_is_rejected() {
        let mut candidate = input();
        let mut false_positive = case("false-positive", false, true);
        false_positive.adjudication = CaseAdjudication::FalsePositive;
        candidate.case_results.push(false_positive);
        candidate.minimum_sample_count = 4;
        let run = evaluate(candidate).unwrap();
        assert_eq!(run.recommendation, EvaluationRecommendation::Reject);
        assert!(run.comparison.contains(&"false_positive:0->1".to_owned()));
    }

    #[test]
    fn worsened_failure_or_unknown_outcome_is_never_accepted() {
        let mut failed = input();
        failed.case_results[0].candidate_outcome = EvaluationOutcome::Failure;
        assert_eq!(
            evaluate(failed).unwrap().recommendation,
            EvaluationRecommendation::Reject
        );

        let mut unknown = input();
        unknown.case_results[0].candidate_outcome = EvaluationOutcome::Unknown;
        assert_eq!(
            evaluate(unknown).unwrap().recommendation,
            EvaluationRecommendation::NeedsReview
        );
    }

    #[test]
    fn duplicate_or_unbound_comparability_evidence_is_invalid() {
        let mut duplicate = input();
        duplicate
            .comparability
            .push(duplicate.comparability[0].clone());
        assert!(matches!(evaluate(duplicate), Err(ReleaseError::Invalid)));

        let mut unbound = input();
        unbound.comparability[0].evidence_ref.clear();
        assert!(matches!(evaluate(unbound), Err(ReleaseError::Invalid)));

        let mut wrong_corpus = input();
        wrong_corpus.case_results[0].corpus_ref = "evals/corpus/other".to_owned();
        assert!(matches!(evaluate(wrong_corpus), Err(ReleaseError::Invalid)));
    }

    #[test]
    fn validator_corpus_or_profile_weakening_is_release_blocking_reject() {
        for metric in ["validator_guard", "corpus", "profile"] {
            let mut candidate = input();
            candidate
                .protected_metric_results
                .iter_mut()
                .find(|item| item.metric_id == metric)
                .unwrap()
                .weakened = true;
            let run = evaluate(candidate).unwrap();
            assert_eq!(run.recommendation, EvaluationRecommendation::Reject);
            assert!(
                run.limitations
                    .contains(&"protected_validator_corpus_or_profile_weakened".to_owned())
            );
        }
    }

    #[test]
    fn not_comparable_is_never_promoted_to_accept() {
        let mut candidate = input();
        candidate.comparability[5].state = ComparabilityState::NotComparable;
        assert_eq!(
            evaluate(candidate).unwrap().recommendation,
            EvaluationRecommendation::NeedsReview
        );
    }

    #[test]
    fn catalog_lifecycle_is_closed_and_preserves_tombstone() {
        let evaluation_id = EvaluationRunId::new();
        let active = EvaluationCatalogItem {
            schema_id: EVALUATION_CATALOG_ITEM_SCHEMA_ID.to_owned(),
            schema_version: 1,
            item_id: "star.check.old".to_owned(),
            item_version: "1.0.0".to_owned(),
            definition_fingerprint: Sha256Hash::digest(b"old"),
            trial_candidate: false,
            lifecycle: EvaluationCatalogLifecycle::Active,
            owner: "star-control".to_owned(),
            corpus_ref: "evals/corpus/v1".to_owned(),
            replacement_ref: Some("star.check.new@1.0.0".to_owned()),
            migration_guide_ref: Some("docs/migration/check.md".to_owned()),
            compatibility_deadline: Some("2026-12-31".to_owned()),
            last_evaluation_run_ref: Some(evaluation_id),
            tombstone_ref: Some("catalog/tombstones/star.check.old.json".to_owned()),
            item_fingerprint: Sha256Hash::digest(b"unsealed"),
        };
        let deprecated = transition_catalog_item(
            active.clone(),
            EvaluationCatalogLifecycle::Deprecated,
            false,
        )
        .unwrap();
        let retired =
            transition_catalog_item(deprecated, EvaluationCatalogLifecycle::Retired, false)
                .unwrap();
        assert_eq!(retired.lifecycle, EvaluationCatalogLifecycle::Retired);
        assert!(retired.tombstone_ref.is_some());
        assert_eq!(
            transition_catalog_item(active, EvaluationCatalogLifecycle::Retired, false),
            Err(ReleaseError::Blocked)
        );
    }

    #[test]
    fn rejected_lifecycle_requires_a_sealed_trial_candidate_flag() {
        let evaluation_id = EvaluationRunId::new();
        let mut active = EvaluationCatalogItem {
            schema_id: EVALUATION_CATALOG_ITEM_SCHEMA_ID.to_owned(),
            schema_version: 1,
            item_id: "star.check.trial".to_owned(),
            item_version: "1.0.0".to_owned(),
            definition_fingerprint: Sha256Hash::digest(b"trial"),
            trial_candidate: false,
            lifecycle: EvaluationCatalogLifecycle::Active,
            owner: "star-control".to_owned(),
            corpus_ref: "evals/corpus/v1".to_owned(),
            replacement_ref: None,
            migration_guide_ref: None,
            compatibility_deadline: None,
            last_evaluation_run_ref: Some(evaluation_id),
            tombstone_ref: Some("catalog/tombstones/star.check.trial.json".to_owned()),
            item_fingerprint: Sha256Hash::digest(b"unsealed"),
        };
        assert_eq!(
            transition_catalog_item(active.clone(), EvaluationCatalogLifecycle::Rejected, true),
            Err(ReleaseError::Blocked)
        );
        active.trial_candidate = true;
        assert_eq!(
            transition_catalog_item(active, EvaluationCatalogLifecycle::Rejected, false),
            Err(ReleaseError::Blocked)
        );
    }

    fn cost_record(
        id: &str,
        project_id: ProjectId,
        run_id: star_contracts::ValidationRunId,
        quantity: u64,
    ) -> CostRecordV1 {
        seal_cost_record(CostRecordV1 {
            schema_id: COST_RECORD_V1_SCHEMA_ID.to_owned(),
            schema_version: 1,
            cost_record_id: id.to_owned(),
            revision: 1,
            project_id,
            scope_kind: star_contracts::release_v2::CostScopeKindV1::ValidationRun,
            scope_ref: format!("validation-run:{run_id}"),
            validation_run_refs: vec![run_id],
            source: "provider".to_owned(),
            usage: vec![star_contracts::release_v2::CostUsageV1 {
                unit: "tokens".to_owned(),
                quantity,
                provider_evidence_ref: format!("provider-statement:{id}"),
            }],
            monetary_cost: None,
            estimated: false,
            paid_action: false,
            measured_at: Utc::now(),
            measurement_unavailable: vec!["monetary_cost".to_owned()],
            provider_evidence_refs: vec![format!("provider-statement:{id}")],
            content_fingerprint: Sha256Hash::digest(b"unsealed"),
        })
        .unwrap()
    }

    #[test]
    fn cost_record_and_budget_snapshot_reject_tampering_and_derive_decision() {
        let project_id = ProjectId::new();
        let record = cost_record(
            "cost-budget-1",
            project_id.clone(),
            star_contracts::ValidationRunId::new(),
            40,
        );
        verify_cost_record(&record).unwrap();
        let mut tampered = record.clone();
        tampered.usage[0].quantity = 41;
        assert_eq!(verify_cost_record(&tampered), Err(ReleaseError::Invalid));

        let reference = cost_record_ref(&record);
        let snapshot = build_budget_snapshot(
            BudgetSnapshotInput {
                snapshot_id: "budget-1".to_owned(),
                revision: 1,
                project_id,
                scope_ref: "goal:test".to_owned(),
                cost_record_refs: vec![reference],
                limits: vec![BudgetQuantityV1 {
                    unit: "tokens".to_owned(),
                    quantity: 100,
                }],
                reserved: vec![BudgetQuantityV1 {
                    unit: "tokens".to_owned(),
                    quantity: 10,
                }],
                unknown_measurements: vec![],
                permission_approval_refs: vec![],
                paid_action_pending: false,
                evaluated_at: Utc::now(),
            },
            std::slice::from_ref(&record),
            Sha256Hash::digest(b"config"),
        )
        .unwrap();
        assert_eq!(snapshot.decision, BudgetDecisionV1::Unknown);
        assert_eq!(snapshot.observed[0].quantity, 40);
        assert_eq!(snapshot.remaining[0].quantity, 50);
        verify_budget_snapshot(&snapshot, &[record]).unwrap();
    }

    #[test]
    fn missing_cost_or_broadened_suppression_is_never_accepted() {
        let mut missing_cost = input();
        missing_cost.case_results[0].candidate_cost_refs.clear();
        missing_cost.case_results[0]
            .candidate_usage_and_cost
            .clear();
        assert_eq!(
            evaluate(missing_cost).unwrap().recommendation,
            EvaluationRecommendation::NeedsReview
        );

        let mut broadened = input();
        broadened.case_results[0].suppression_broadened_count = 1;
        assert_eq!(
            evaluate(broadened).unwrap().recommendation,
            EvaluationRecommendation::Reject
        );
    }
}
