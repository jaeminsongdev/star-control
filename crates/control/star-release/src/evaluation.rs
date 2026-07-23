use std::collections::BTreeSet;

use star_contracts::{
    EvaluationRunId, Sha256Hash,
    release_v2::{
        CaseAdjudication, ComparabilityState, EVALUATION_CATALOG_ITEM_SCHEMA_ID,
        EVALUATION_RUN_V2_SCHEMA_ID, EvaluationCaseResult, EvaluationCatalogItem,
        EvaluationCatalogLifecycle, EvaluationComparability, EvaluationContext,
        EvaluationDefinition, EvaluationMetricSummary, EvaluationMode, EvaluationOutcome,
        EvaluationRecommendation, EvaluationRunV2, ProtectedMetricResult,
    },
};
use star_domain::versioned_fingerprint;

use crate::ReleaseError;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EvaluationInput {
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
    let recommendation = if protected_weakened {
        EvaluationRecommendation::Reject
    } else if not_comparable
        || metrics.sample_count < input.minimum_sample_count
        || metrics.unresolved > 0
        || safety.candidate_unknown_outcomes > 0
    {
        EvaluationRecommendation::NeedsReview
    } else if safety.candidate_false_negatives > safety.baseline_false_negatives
        || safety.candidate_false_positives > safety.baseline_false_positives
        || safety.candidate_adverse_outcomes > safety.baseline_adverse_outcomes
        || safety.candidate_rollbacks > safety.baseline_rollbacks
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
    limitations.sort();
    limitations.dedup();
    let comparison = comparison_lines(&metrics, safety);
    let mut run = EvaluationRunV2 {
        schema_id: EVALUATION_RUN_V2_SCHEMA_ID.to_owned(),
        schema_version: 2,
        evaluation_run_id: EvaluationRunId::new(),
        subject_kind: input.candidate.subject.kind,
        subject: input.candidate.subject.clone(),
        evaluation_context: input.evaluation_context,
        baseline: input.baseline,
        candidate: input.candidate,
        mode: input.mode,
        corpus_ref: input.corpus_ref,
        case_selection_fingerprint: input.case_selection_fingerprint,
        measurement_protocol_fingerprint: input.measurement_protocol_fingerprint,
        case_results: input.case_results,
        ground_truth_summary: metrics.clone(),
        finding_metrics: metrics.clone(),
        efficiency_metrics: metrics,
        usage_and_cost_refs: Vec::new(),
        comparability: input.comparability,
        protected_metric_results: input.protected_metric_results,
        limitations,
        comparison,
        recommendation,
        decision_ref: None,
        radar_item_refs: Vec::new(),
        run_fingerprint: Sha256Hash::digest(b"unsealed-evaluation-run"),
    };
    run.run_fingerprint = versioned_fingerprint(
        EVALUATION_RUN_V2_SCHEMA_ID,
        2,
        &serde_json::json!({
            "evaluation_run_id":run.evaluation_run_id,
            "subject_kind":run.subject_kind,
            "subject":run.subject,
            "evaluation_context":run.evaluation_context,
            "baseline":run.baseline,
            "candidate":run.candidate,
            "mode":run.mode,
            "corpus_ref":run.corpus_ref,
            "case_selection_fingerprint":run.case_selection_fingerprint,
            "measurement_protocol_fingerprint":run.measurement_protocol_fingerprint,
            "case_results":run.case_results,
            "ground_truth_summary":run.ground_truth_summary,
            "comparability":run.comparability,
            "protected_metric_results":run.protected_metric_results,
            "limitations":run.limitations,
            "comparison":run.comparison,
            "recommendation":run.recommendation,
        }),
    )
    .map_err(|_| ReleaseError::Fingerprint)?;
    Ok(run)
}

pub fn transition_catalog_item(
    mut item: EvaluationCatalogItem,
    next: EvaluationCatalogLifecycle,
    trial_candidate: bool,
) -> Result<EvaluationCatalogItem, ReleaseError> {
    let valid = match (item.lifecycle, next) {
        (EvaluationCatalogLifecycle::Active, EvaluationCatalogLifecycle::Deprecated) => {
            item.replacement_ref.is_some()
                && item.migration_guide_ref.is_some()
                && item.compatibility_deadline.is_some()
                && item.last_evaluation_run_ref.is_some()
        }
        (EvaluationCatalogLifecycle::Deprecated, EvaluationCatalogLifecycle::Retired) => {
            item.tombstone_ref.is_some()
                && item.migration_guide_ref.is_some()
                && item.last_evaluation_run_ref.is_some()
        }
        (EvaluationCatalogLifecycle::Active, EvaluationCatalogLifecycle::Rejected) => {
            trial_candidate
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
                || case.baseline_run_refs.is_empty()
                || case.candidate_run_refs.is_empty()
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
    }
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
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::release_v2::{
        EvaluationCatalogLifecycle, EvaluationSubject, EvaluationSubjectKind,
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

    fn case(id: &str, baseline_detected: bool, candidate_detected: bool) -> EvaluationCaseResult {
        EvaluationCaseResult {
            case_id: id.to_owned(),
            case_version: "1".to_owned(),
            corpus_ref: "evals/corpus/v1".to_owned(),
            evaluation_context: EvaluationContext::CliOnly,
            task_source_binding: Sha256Hash::digest(id.as_bytes()),
            baseline_run_refs: vec![star_contracts::ValidationRunId::new()],
            candidate_run_refs: vec![star_contracts::ValidationRunId::new()],
            adjudication: CaseAdjudication::ConfirmedDefect,
            baseline_detected,
            candidate_detected,
            baseline_duration_ms: 100,
            candidate_duration_ms: 80,
            baseline_rework_count: 1,
            candidate_rework_count: 0,
            baseline_outcome: EvaluationOutcome::Success,
            candidate_outcome: EvaluationOutcome::Success,
            candidate_flaky: false,
            limitations: vec![],
        }
    }

    fn input() -> EvaluationInput {
        EvaluationInput {
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
        }
    }

    #[test]
    fn comparable_candidate_with_more_detected_defects_and_less_rework_is_accepted() {
        let run = evaluate(input()).unwrap();
        assert_eq!(run.recommendation, EvaluationRecommendation::Accept);
        assert_eq!(run.ground_truth_summary.candidate_false_negatives, 0);
        assert!(
            run.efficiency_metrics.candidate_total_duration_ms
                < run.efficiency_metrics.baseline_total_duration_ms
        );
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
}
