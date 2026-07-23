use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use star_contracts::{
    Sha256Hash,
    development_v2::CoverageState,
    migration_v2::{
        CROSS_PROJECT_MIGRATION_HANDOFF_SCHEMA_ID, CrossProjectMigrationHandoff,
        EQUIVALENCE_REPORT_SCHEMA_ID, EquivalenceDimensionState, EquivalenceReport,
        EquivalenceState, LANGUAGE_MIGRATION_PLAN_SCHEMA_ID, LanguageMigrationPlan,
        MIGRATION_ATTEMPT_SCHEMA_ID, MIGRATION_CHECKPOINT_V2_SCHEMA_ID,
        MIGRATION_PLAN_V2_SCHEMA_ID, MIGRATION_VALIDATION_REPORT_SCHEMA_ID, MetricComparison,
        MigrationAttempt, MigrationAttemptState, MigrationChain, MigrationCheckpointV2,
        MigrationEffectClass, MigrationPhase, MigrationPhasePlan, MigrationPlanV2,
        MigrationStrategy, MigrationSupportDecision, MigrationValidationReport,
        MigrationVersionEntry, MigrationVersionVector, PERFORMANCE_COMPARISON_V2_SCHEMA_ID,
        PERFORMANCE_RUN_SCHEMA_ID, PERFORMANCE_WORKLOAD_SPEC_SCHEMA_ID,
        PROJECT_MIGRATION_MANIFEST_SCHEMA_ID, PerformanceCohort, PerformanceComparisonState,
        PerformanceComparisonV2, PerformanceRun, PerformanceWorkloadSpec, ProjectMigrationManifest,
        RESTORE_VERIFICATION_RECORD_SCHEMA_ID, ResolvedMigrationStep, RestoreVerificationRecord,
        VersionObservationState,
    },
};

use crate::{DevelopmentError, fingerprint, placeholder, safe_relative_path, token};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationPlanInput {
    pub migration_plan_id: String,
    pub revision: u64,
    pub task_spec_ref: String,
    pub scope_revision_ref: String,
    pub impact_analysis_ref: String,
    pub checkout_id: String,
    pub source_subject_fingerprint: Sha256Hash,
    pub target_id: String,
    pub observed_version_vector: MigrationVersionVector,
    pub strategy: MigrationStrategy,
    pub resource_estimate: String,
    pub rollback_plan_ref: String,
    #[serde(default)]
    pub validation_plan_refs: Vec<String>,
    #[serde(default)]
    pub permission_checkpoints: Vec<String>,
    #[serde(default)]
    pub source_patch_refs: Vec<String>,
    #[serde(default)]
    pub consumer_compatibility_refs: Vec<String>,
    pub cross_project_handoff_ref: Option<String>,
}

pub fn parse_project_migration_manifest(
    bytes: &[u8],
) -> Result<ProjectMigrationManifest, DevelopmentError> {
    let text = std::str::from_utf8(bytes).map_err(|_| DevelopmentError::Invalid)?;
    let mut manifest: ProjectMigrationManifest =
        toml::from_str(text).map_err(|_| DevelopmentError::Invalid)?;
    validate_project_migration_manifest(&manifest)?;
    manifest.content_fingerprint = Sha256Hash::digest(bytes);
    Ok(manifest)
}

pub fn validate_project_migration_manifest(
    manifest: &ProjectMigrationManifest,
) -> Result<(), DevelopmentError> {
    if manifest.schema_id != PROJECT_MIGRATION_MANIFEST_SCHEMA_ID
        || manifest.schema_version != 1
        || !token(&manifest.manifest_id, 192)
        || manifest.manifest_version.trim().is_empty()
        || manifest.target_specs.is_empty()
    {
        return Err(DevelopmentError::Invalid);
    }
    let targets = manifest
        .target_specs
        .iter()
        .map(|target| target.target_id.as_str())
        .collect::<BTreeSet<_>>();
    if targets.len() != manifest.target_specs.len()
        || manifest.target_specs.iter().any(|target| {
            !token(&target.target_id, 128)
                || target.owner.trim().is_empty()
                || target.locator_class.trim().is_empty()
                || target.version_source_ref.trim().is_empty()
                || target.target_version.trim().is_empty()
        })
    {
        return Err(DevelopmentError::Invalid);
    }
    let invariants = manifest
        .invariant_specs
        .iter()
        .map(|invariant| invariant.invariant_id.as_str())
        .collect::<BTreeSet<_>>();
    if invariants.len() != manifest.invariant_specs.len()
        || manifest.invariant_specs.iter().any(|invariant| {
            !token(&invariant.invariant_id, 128)
                || invariant.before_check_ref.trim().is_empty()
                || invariant.after_check_ref.trim().is_empty()
        })
    {
        return Err(DevelopmentError::Invalid);
    }
    let mut chains = BTreeSet::new();
    for chain in &manifest.migration_chains {
        if !token(&chain.chain_id, 128)
            || !targets.contains(chain.target_id.as_str())
            || !chains.insert(chain.chain_id.as_str())
            || chain.steps.is_empty()
        {
            return Err(DevelopmentError::Invalid);
        }
        validate_chain(chain, &invariants)?;
    }
    Ok(())
}

pub fn seal_migration_version_vector(
    mut entries: Vec<MigrationVersionEntry>,
) -> Result<MigrationVersionVector, DevelopmentError> {
    entries.sort_by(|left, right| left.axis_id.cmp(&right.axis_id));
    if entries.is_empty()
        || entries
            .windows(2)
            .any(|pair| pair[0].axis_id == pair[1].axis_id)
        || entries.iter().any(|entry| {
            !token(&entry.axis_id, 128)
                || entry.owner.trim().is_empty()
                || entry.version_scheme.trim().is_empty()
                || entry.source_ref.trim().is_empty()
                || entry.observation_state == VersionObservationState::Observed
                    && entry
                        .observed_version
                        .as_ref()
                        .is_none_or(|value| value.trim().is_empty())
                || entry.observation_state != VersionObservationState::Observed
                    && entry.observed_version.is_some()
        })
    {
        return Err(DevelopmentError::Invalid);
    }
    let vector_fingerprint = fingerprint("star.migration-version-vector", &entries)?;
    Ok(MigrationVersionVector {
        entries,
        vector_fingerprint,
    })
}

pub fn build_migration_plan(
    manifest: &ProjectMigrationManifest,
    input: MigrationPlanInput,
) -> Result<MigrationPlanV2, DevelopmentError> {
    validate_project_migration_manifest(manifest)?;
    validate_vector(&input.observed_version_vector)?;
    if !token(&input.migration_plan_id, 192)
        || input.revision == 0
        || input.task_spec_ref.trim().is_empty()
        || input.scope_revision_ref.trim().is_empty()
        || input.impact_analysis_ref.trim().is_empty()
        || input.checkout_id.trim().is_empty()
        || input.resource_estimate.trim().is_empty()
        || input.rollback_plan_ref.trim().is_empty()
    {
        return Err(DevelopmentError::Invalid);
    }
    let target = manifest
        .target_specs
        .iter()
        .find(|target| target.target_id == input.target_id)
        .ok_or(DevelopmentError::Invalid)?;
    let observed = input
        .observed_version_vector
        .entries
        .iter()
        .find(|entry| entry.axis_id == input.target_id);
    let observed_version = observed.and_then(|entry| entry.observed_version.as_deref());
    let (support_decision, ordered_steps, mut blockers) = match observed {
        None
        | Some(MigrationVersionEntry {
            observation_state: VersionObservationState::Unknown,
            ..
        }) => (
            MigrationSupportDecision::UnknownVersion,
            Vec::new(),
            vec!["MIGRATION_VERSION_UNKNOWN".to_owned()],
        ),
        Some(MigrationVersionEntry {
            observation_state: VersionObservationState::Corrupt,
            ..
        }) => (
            MigrationSupportDecision::Corrupt,
            Vec::new(),
            vec!["MIGRATION_VERSION_CORRUPT".to_owned()],
        ),
        Some(_) if observed_version == Some(target.target_version.as_str()) => (
            MigrationSupportDecision::CurrentSupported,
            Vec::new(),
            Vec::new(),
        ),
        Some(_) => resolve_chain(
            manifest,
            target.target_id.as_str(),
            observed_version.unwrap(),
            &target.target_version,
        ),
    };
    if input.strategy == MigrationStrategy::TransactionalInPlace
        && ordered_steps
            .iter()
            .any(|step| step.effect_class != MigrationEffectClass::LiveNondestructive)
    {
        blockers
            .push("transactional_in_place capability is not proven for the full chain".to_owned());
    }
    let target_entry = MigrationVersionEntry {
        axis_id: target.target_id.clone(),
        owner: target.owner.clone(),
        observed_version: Some(target.target_version.clone()),
        version_scheme: observed
            .map(|entry| entry.version_scheme.clone())
            .unwrap_or_else(|| "declared".to_owned()),
        source_ref: target.version_source_ref.clone(),
        source_fingerprint: manifest.content_fingerprint.clone(),
        coverage: CoverageState::Complete,
        observation_state: VersionObservationState::Observed,
    };
    let target_version_vector = seal_migration_version_vector(vec![target_entry])?;
    blockers.sort();
    blockers.dedup();
    let destructive = ordered_steps
        .iter()
        .any(|step| step.effect_class == MigrationEffectClass::LiveDestructive);
    let dry_run_plan = phase_plan("dry_run", !ordered_steps.is_empty(), "no live target write");
    let backup_plan = phase_plan(
        "backup",
        !ordered_steps.is_empty(),
        "immutable backup artifact",
    );
    let rehearsal_plan = phase_plan(
        "migration_rehearsal",
        !ordered_steps.is_empty(),
        "disposable subject reaches target version",
    );
    let activation_plan = phase_plan(
        "activate",
        !ordered_steps.is_empty(),
        "candidate becomes visible after post-execute validation",
    );
    let resume_plan = phase_plan(
        "resume",
        ordered_steps.len() > 1,
        "checkpoint reconciles exact durable prefix",
    );
    let mut permission_checkpoints = input.permission_checkpoints;
    if destructive {
        permission_checkpoints.push("live_destructive".to_owned());
    }
    permission_checkpoints.sort();
    permission_checkpoints.dedup();
    let mut plan = MigrationPlanV2 {
        schema_id: MIGRATION_PLAN_V2_SCHEMA_ID.to_owned(),
        schema_version: 2,
        migration_plan_id: input.migration_plan_id,
        revision: input.revision,
        task_spec_ref: input.task_spec_ref,
        scope_revision_ref: input.scope_revision_ref,
        impact_analysis_ref: input.impact_analysis_ref,
        project_id: manifest.project_id.clone(),
        checkout_id: input.checkout_id,
        source_subject_fingerprint: input.source_subject_fingerprint,
        manifest_ref: manifest.manifest_id.clone(),
        manifest_fingerprint: manifest.content_fingerprint.clone(),
        target_id: input.target_id,
        observed_version_vector: input.observed_version_vector,
        target_version_vector,
        support_decision,
        ordered_steps,
        invariant_refs: manifest
            .invariant_specs
            .iter()
            .filter(|invariant| invariant.required)
            .map(|invariant| invariant.invariant_id.clone())
            .collect(),
        strategy: input.strategy,
        resource_estimate: input.resource_estimate,
        dry_run_plan,
        backup_plan,
        rehearsal_plan,
        activation_plan,
        resume_plan,
        rollback_plan_ref: input.rollback_plan_ref,
        validation_plan_refs: input.validation_plan_refs,
        permission_checkpoints,
        source_patch_refs: input.source_patch_refs,
        consumer_compatibility_refs: input.consumer_compatibility_refs,
        cross_project_handoff_ref: input.cross_project_handoff_ref,
        blockers,
        plan_fingerprint: placeholder(),
    };
    plan.plan_fingerprint = migration_plan_fingerprint(&plan)?;
    Ok(plan)
}

pub fn seal_migration_checkpoint(
    plan: &MigrationPlanV2,
    mut checkpoint: MigrationCheckpointV2,
) -> Result<MigrationCheckpointV2, DevelopmentError> {
    if checkpoint.schema_id != MIGRATION_CHECKPOINT_V2_SCHEMA_ID
        || checkpoint.schema_version != 2
        || !token(&checkpoint.checkpoint_id, 192)
        || checkpoint.plan_ref != plan.migration_plan_id
        || checkpoint.plan_fingerprint != plan.plan_fingerprint
    {
        return Err(DevelopmentError::Invalid);
    }
    let expected_prefix = plan
        .ordered_steps
        .iter()
        .take(checkpoint.completed_step_refs.len())
        .map(|step| step.step_id.as_str())
        .collect::<Vec<_>>();
    let actual = checkpoint
        .completed_step_refs
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if actual != expected_prefix
        || checkpoint
            .in_progress_step_ref
            .as_ref()
            .is_some_and(|step| {
                plan.ordered_steps
                    .get(checkpoint.completed_step_refs.len())
                    .is_none_or(|expected| &expected.step_id != step)
            })
        || checkpoint.in_progress_step_ref.is_some()
            && !checkpoint.replay_safe
            && !checkpoint.reconciliation_required
    {
        return Err(DevelopmentError::Conflict);
    }
    checkpoint.checkpoint_fingerprint = fingerprint(
        MIGRATION_CHECKPOINT_V2_SCHEMA_ID,
        &serde_json::json!({
            "checkpoint_id": checkpoint.checkpoint_id,
            "plan_ref": checkpoint.plan_ref,
            "plan_fingerprint": checkpoint.plan_fingerprint,
            "completed_step_refs": checkpoint.completed_step_refs,
            "in_progress_step_ref": checkpoint.in_progress_step_ref,
            "target_version": checkpoint.target_version,
            "target_state_fingerprint": checkpoint.target_state_fingerprint,
            "last_receipt_ref": checkpoint.last_receipt_ref,
            "replay_safe": checkpoint.replay_safe,
            "reconciliation_required": checkpoint.reconciliation_required,
        }),
    )?;
    Ok(checkpoint)
}

pub fn seal_migration_attempt(
    plan: &MigrationPlanV2,
    previous: &[MigrationAttempt],
    mut attempt: MigrationAttempt,
) -> Result<MigrationAttempt, DevelopmentError> {
    if attempt.schema_id != MIGRATION_ATTEMPT_SCHEMA_ID
        || attempt.schema_version != 1
        || !token(&attempt.attempt_id, 192)
        || attempt.attempt_no == 0
        || attempt.plan_ref != plan.migration_plan_id
        || attempt.plan_fingerprint != plan.plan_fingerprint
        || previous
            .iter()
            .any(|item| item.attempt_id == attempt.attempt_id)
        || previous
            .iter()
            .any(|item| item.attempt_no == attempt.attempt_no)
    {
        return Err(DevelopmentError::Invalid);
    }
    let live_effect = matches!(
        attempt.phase,
        MigrationPhase::Backup
            | MigrationPhase::Execute
            | MigrationPhase::Resume
            | MigrationPhase::Activate
            | MigrationPhase::Rollback
    );
    if live_effect
        && (attempt.permission_decision_ref.is_none() || attempt.gate_decision_ref.is_none())
    {
        return Err(DevelopmentError::Blocked);
    }
    if attempt.state == MigrationAttemptState::Succeeded
        && (attempt.subject_binding_after.is_none()
            || attempt.effect_committed.is_none()
            || attempt.receipt_refs.is_empty())
    {
        return Err(DevelopmentError::Invalid);
    }
    if attempt.state == MigrationAttemptState::OutcomeUnknown {
        attempt.effect_committed = None;
    }
    if attempt.state == MigrationAttemptState::PartiallyApplied
        && attempt.checkpoint_after_ref.is_none()
    {
        return Err(DevelopmentError::Invalid);
    }
    if !migration_phase_prerequisites_met(plan, previous, attempt.phase) {
        return Err(DevelopmentError::Blocked);
    }
    attempt.receipt_refs.sort();
    attempt.receipt_refs.dedup();
    attempt.diagnostic_refs.sort();
    attempt.diagnostic_refs.dedup();
    attempt.attempt_fingerprint = fingerprint(
        MIGRATION_ATTEMPT_SCHEMA_ID,
        &serde_json::json!({
            "attempt_id": attempt.attempt_id,
            "attempt_no": attempt.attempt_no,
            "plan_ref": attempt.plan_ref,
            "plan_fingerprint": attempt.plan_fingerprint,
            "phase": attempt.phase,
            "step_ref": attempt.step_ref,
            "subject_binding_before": attempt.subject_binding_before,
            "checkpoint_before_ref": attempt.checkpoint_before_ref,
            "permission_decision_ref": attempt.permission_decision_ref,
            "gate_decision_ref": attempt.gate_decision_ref,
            "invocation_ref": attempt.invocation_ref,
            "tool_observation_ref": attempt.tool_observation_ref,
            "receipt_refs": attempt.receipt_refs,
            "subject_binding_after": attempt.subject_binding_after,
            "checkpoint_after_ref": attempt.checkpoint_after_ref,
            "diagnostic_refs": attempt.diagnostic_refs,
            "state": attempt.state,
            "effect_committed": attempt.effect_committed,
            "loss_observed": attempt.loss_observed,
        }),
    )?;
    Ok(attempt)
}

pub fn seal_migration_validation_report(
    mut report: MigrationValidationReport,
) -> Result<MigrationValidationReport, DevelopmentError> {
    if report.schema_id != MIGRATION_VALIDATION_REPORT_SCHEMA_ID
        || report.schema_version != 1
        || !token(&report.report_id, 192)
        || report.plan_ref.trim().is_empty()
        || report.attempt_ref.trim().is_empty()
        || report.invariant_results.is_empty()
    {
        return Err(DevelopmentError::Invalid);
    }
    report
        .invariant_results
        .sort_by(|left, right| left.invariant_ref.cmp(&right.invariant_ref));
    report.state = if report
        .invariant_results
        .iter()
        .any(|result| result.loss_observed || result.state == "failed")
    {
        "failed".to_owned()
    } else if report.completeness != CoverageState::Complete
        || report
            .invariant_results
            .iter()
            .any(|result| result.state != "pass")
    {
        "unverified".to_owned()
    } else {
        "pass".to_owned()
    };
    report.report_fingerprint = fingerprint(
        MIGRATION_VALIDATION_REPORT_SCHEMA_ID,
        &serde_json::json!({
            "report_id": report.report_id,
            "plan_ref": report.plan_ref,
            "attempt_ref": report.attempt_ref,
            "invariant_results": report.invariant_results,
            "reference_validation_refs": report.reference_validation_refs,
            "gate_refs": report.gate_refs,
            "target_version_observed": report.target_version_observed,
            "state": report.state,
            "completeness": report.completeness,
        }),
    )?;
    Ok(report)
}

pub fn seal_restore_verification(
    mut record: RestoreVerificationRecord,
) -> Result<RestoreVerificationRecord, DevelopmentError> {
    if record.schema_id != RESTORE_VERIFICATION_RECORD_SCHEMA_ID
        || record.schema_version != 1
        || !token(&record.record_id, 192)
        || record.plan_ref.trim().is_empty()
        || record.backup_artifact_ref.trim().is_empty()
    {
        return Err(DevelopmentError::Invalid);
    }
    record.state = if record.integrity_verified && !record.behavior_check_refs.is_empty() {
        "verified".to_owned()
    } else {
        "unverified".to_owned()
    };
    record.record_fingerprint = fingerprint(
        RESTORE_VERIFICATION_RECORD_SCHEMA_ID,
        &serde_json::json!({
            "record_id": record.record_id,
            "plan_ref": record.plan_ref,
            "backup_artifact_ref": record.backup_artifact_ref,
            "backup_fingerprint": record.backup_fingerprint,
            "restored_subject_fingerprint": record.restored_subject_fingerprint,
            "integrity_verified": record.integrity_verified,
            "behavior_check_refs": record.behavior_check_refs,
            "state": record.state,
        }),
    )?;
    Ok(record)
}

pub fn seal_performance_workload(
    mut specification: PerformanceWorkloadSpec,
) -> Result<PerformanceWorkloadSpec, DevelopmentError> {
    if specification.schema_id != PERFORMANCE_WORKLOAD_SPEC_SCHEMA_ID
        || specification.schema_version != 1
        || !token(&specification.workload_id, 192)
        || specification.task_ref.trim().is_empty()
        || specification.environment_class.trim().is_empty()
        || specification.build_mode.trim().is_empty()
        || specification.measured_count < 3
        || !specification.noise_budget_ratio.is_finite()
        || specification.noise_budget_ratio < 0.0
        || specification.metrics.is_empty()
        || specification.metrics.iter().any(|metric| {
            !token(&metric.metric_id, 128)
                || metric.unit.trim().is_empty()
                || !metric.budget_ratio.is_finite()
                || metric.budget_ratio <= 0.0
        })
    {
        return Err(DevelopmentError::Invalid);
    }
    specification
        .metrics
        .sort_by(|left, right| left.metric_id.cmp(&right.metric_id));
    specification.specification_fingerprint = fingerprint(
        PERFORMANCE_WORKLOAD_SPEC_SCHEMA_ID,
        &serde_json::json!({
            "workload_id": specification.workload_id,
            "project_id": specification.project_id,
            "task_ref": specification.task_ref,
            "input_fingerprint": specification.input_fingerprint,
            "environment_class": specification.environment_class,
            "build_mode": specification.build_mode,
            "warmup_count": specification.warmup_count,
            "measured_count": specification.measured_count,
            "outlier_policy": specification.outlier_policy,
            "noise_budget_ratio": specification.noise_budget_ratio,
            "metrics": specification.metrics,
            "correctness_check_refs": specification.correctness_check_refs,
        }),
    )?;
    Ok(specification)
}

pub fn seal_performance_run(
    workload: &PerformanceWorkloadSpec,
    mut run: PerformanceRun,
) -> Result<PerformanceRun, DevelopmentError> {
    if run.schema_id != PERFORMANCE_RUN_SCHEMA_ID
        || run.schema_version != 1
        || !token(&run.run_id, 192)
        || run.workload_ref != workload.workload_id
        || run.workload_fingerprint != workload.specification_fingerprint
        || run.attempt == 0
        || run.build_mode != workload.build_mode
        || run.measurements.is_empty()
        || run.measurements.iter().any(|measurement| {
            !measurement.value.is_finite()
                || measurement.value < 0.0
                || measurement.unit.trim().is_empty()
                || measurement.collector.trim().is_empty()
                || !workload.metrics.iter().any(|metric| {
                    metric.metric_id == measurement.metric_id && metric.unit == measurement.unit
                })
        })
    {
        return Err(DevelopmentError::Invalid);
    }
    run.measurements
        .sort_by(|left, right| left.metric_id.cmp(&right.metric_id));
    run.run_fingerprint = fingerprint(
        PERFORMANCE_RUN_SCHEMA_ID,
        &serde_json::json!({
            "run_id": run.run_id,
            "workload_ref": run.workload_ref,
            "workload_fingerprint": run.workload_fingerprint,
            "cohort": run.cohort,
            "attempt": run.attempt,
            "warmup": run.warmup,
            "subject_fingerprint": run.subject_fingerprint,
            "environment_fingerprint": run.environment_fingerprint,
            "toolchain_fingerprint": run.toolchain_fingerprint,
            "build_mode": run.build_mode,
            "measurements": run.measurements,
            "correctness_passed": run.correctness_passed,
            "evidence_refs": run.evidence_refs,
        }),
    )?;
    Ok(run)
}

pub fn compare_performance_runs(
    comparison_id: String,
    workload: &PerformanceWorkloadSpec,
    baseline: &[PerformanceRun],
    candidate: &[PerformanceRun],
) -> Result<PerformanceComparisonV2, DevelopmentError> {
    if !token(&comparison_id, 192)
        || baseline.is_empty()
        || candidate.is_empty()
        || baseline
            .iter()
            .any(|run| run.cohort != PerformanceCohort::Baseline || run.warmup)
        || candidate
            .iter()
            .any(|run| run.cohort != PerformanceCohort::Candidate || run.warmup)
    {
        return Err(DevelopmentError::Invalid);
    }
    let comparable = baseline.iter().chain(candidate.iter()).all(|run| {
        run.workload_fingerprint == workload.specification_fingerprint
            && run.environment_fingerprint == baseline[0].environment_fingerprint
            && run.toolchain_fingerprint == baseline[0].toolchain_fingerprint
            && run.build_mode == workload.build_mode
    });
    let correctness_verified = baseline
        .iter()
        .chain(candidate.iter())
        .all(|run| run.correctness_passed);
    let mut comparisons = Vec::new();
    for metric in &workload.metrics {
        let mut before = metric_values(baseline, &metric.metric_id);
        let mut after = metric_values(candidate, &metric.metric_id);
        if before.len() < 3 || after.len() < 3 {
            return Err(DevelopmentError::Unverified);
        }
        before.sort_by(f64::total_cmp);
        after.sort_by(f64::total_cmp);
        let baseline_median = median(&before);
        let candidate_median = median(&after);
        let baseline_p95 = percentile(&before, 0.95);
        let candidate_p95 = percentile(&after, 0.95);
        let ratio = if baseline_median == 0.0 {
            f64::INFINITY
        } else {
            candidate_median / baseline_median
        };
        let noise_ratio = coefficient_of_variation(&before).max(coefficient_of_variation(&after));
        let state = if !comparable {
            PerformanceComparisonState::Incomparable
        } else if !correctness_verified {
            PerformanceComparisonState::CorrectnessUnverified
        } else if noise_ratio > workload.noise_budget_ratio {
            PerformanceComparisonState::NoiseInconclusive
        } else if ratio > metric.budget_ratio {
            PerformanceComparisonState::Regression
        } else {
            PerformanceComparisonState::Pass
        };
        comparisons.push(MetricComparison {
            metric_id: metric.metric_id.clone(),
            unit: metric.unit.clone(),
            baseline_median,
            candidate_median,
            baseline_p95,
            candidate_p95,
            ratio,
            noise_ratio,
            budget_ratio: metric.budget_ratio,
            state,
        });
    }
    let state = aggregate_performance_state(comparisons.iter().map(|item| item.state));
    let mut limitations = Vec::new();
    if !comparable {
        limitations.push("cohort binding differs".to_owned());
    }
    if !correctness_verified {
        limitations.push("correctness check did not pass for every measured run".to_owned());
    }
    let mut output = PerformanceComparisonV2 {
        schema_id: PERFORMANCE_COMPARISON_V2_SCHEMA_ID.to_owned(),
        schema_version: 2,
        comparison_id,
        workload_ref: workload.workload_id.clone(),
        workload_fingerprint: workload.specification_fingerprint.clone(),
        baseline_run_refs: baseline.iter().map(|run| run.run_id.clone()).collect(),
        candidate_run_refs: candidate.iter().map(|run| run.run_id.clone()).collect(),
        metric_comparisons: comparisons,
        correctness_verified,
        comparable,
        state,
        limitations,
        comparison_fingerprint: placeholder(),
    };
    output.comparison_fingerprint = fingerprint(
        PERFORMANCE_COMPARISON_V2_SCHEMA_ID,
        &serde_json::json!({
            "comparison_id": output.comparison_id,
            "workload_ref": output.workload_ref,
            "workload_fingerprint": output.workload_fingerprint,
            "baseline_run_refs": output.baseline_run_refs,
            "candidate_run_refs": output.candidate_run_refs,
            "metric_comparisons": output.metric_comparisons,
            "correctness_verified": output.correctness_verified,
            "comparable": output.comparable,
            "state": output.state,
            "limitations": output.limitations,
        }),
    )?;
    Ok(output)
}

pub fn seal_language_migration_plan(
    mut plan: LanguageMigrationPlan,
) -> Result<LanguageMigrationPlan, DevelopmentError> {
    if plan.schema_id != LANGUAGE_MIGRATION_PLAN_SCHEMA_ID
        || plan.schema_version != 1
        || !token(&plan.plan_id, 192)
        || plan.revision == 0
        || plan.task_spec_ref.trim().is_empty()
        || plan.impact_analysis_ref.trim().is_empty()
        || plan.checkout_id.trim().is_empty()
        || plan.behavior_contract_refs.is_empty()
        || plan.boundary_adapter_specs.is_empty()
        || plan.coexistence_phases.is_empty()
        || plan.rollback_plan_ref.trim().is_empty()
    {
        return Err(DevelopmentError::Invalid);
    }
    plan.coexistence_phases.sort_by_key(|phase| phase.order);
    if plan
        .coexistence_phases
        .windows(2)
        .any(|pair| pair[0].order == pair[1].order)
        || plan
            .coexistence_phases
            .iter()
            .any(|phase| !token(&phase.phase_id, 128) || phase.order == 0)
    {
        return Err(DevelopmentError::Invalid);
    }
    plan.state = if plan.unknown_semantics.is_empty() {
        "planned".to_owned()
    } else {
        "human_review".to_owned()
    };
    plan.plan_fingerprint = fingerprint(
        LANGUAGE_MIGRATION_PLAN_SCHEMA_ID,
        &serde_json::json!({
            "plan_id": plan.plan_id,
            "revision": plan.revision,
            "task_spec_ref": plan.task_spec_ref,
            "impact_analysis_ref": plan.impact_analysis_ref,
            "project_id": plan.project_id,
            "checkout_id": plan.checkout_id,
            "source_stack": plan.source_stack,
            "target_stack": plan.target_stack,
            "behavior_contract_refs": plan.behavior_contract_refs,
            "boundary_adapter_specs": plan.boundary_adapter_specs,
            "coexistence_phases": plan.coexistence_phases,
            "consumer_transition_order": plan.consumer_transition_order,
            "recipe_refs": plan.recipe_refs,
            "codegen_refs": plan.codegen_refs,
            "comparison_plan_refs": plan.comparison_plan_refs,
            "compatibility_window": plan.compatibility_window,
            "cutover_plan": plan.cutover_plan,
            "rollback_plan_ref": plan.rollback_plan_ref,
            "platform_evidence_matrix": plan.platform_evidence_matrix,
            "unknown_semantics": plan.unknown_semantics,
            "state": plan.state,
        }),
    )?;
    Ok(plan)
}

pub fn seal_equivalence_report(
    plan: &LanguageMigrationPlan,
    mut report: EquivalenceReport,
) -> Result<EquivalenceReport, DevelopmentError> {
    if report.schema_id != EQUIVALENCE_REPORT_SCHEMA_ID
        || report.schema_version != 1
        || !token(&report.equivalence_report_id, 192)
        || report.plan_ref != plan.plan_id
        || report.dimension_results.is_empty()
    {
        return Err(DevelopmentError::Invalid);
    }
    report
        .dimension_results
        .sort_by(|left, right| left.dimension_id.cmp(&right.dimension_id));
    report.equivalence_state = aggregate_equivalence(&report);
    report.report_fingerprint = fingerprint(
        EQUIVALENCE_REPORT_SCHEMA_ID,
        &serde_json::json!({
            "equivalence_report_id": report.equivalence_report_id,
            "plan_ref": report.plan_ref,
            "baseline_subject": report.baseline_subject,
            "candidate_subject": report.candidate_subject,
            "dimension_results": report.dimension_results,
            "build_compile_result": report.build_compile_result,
            "test_contract_results": report.test_contract_results,
            "performance_comparison_refs": report.performance_comparison_refs,
            "platform_matrix_results": report.platform_matrix_results,
            "consumer_results": report.consumer_results,
            "unknown_semantics": report.unknown_semantics,
            "equivalence_state": report.equivalence_state,
            "gate_refs": report.gate_refs,
        }),
    )?;
    Ok(report)
}

pub fn seal_cross_project_migration_handoff(
    mut handoff: CrossProjectMigrationHandoff,
) -> Result<CrossProjectMigrationHandoff, DevelopmentError> {
    if handoff.schema_id != CROSS_PROJECT_MIGRATION_HANDOFF_SCHEMA_ID
        || handoff.schema_version != 1
        || !token(&handoff.handoff_id, 192)
        || handoff.participants.is_empty()
    {
        return Err(DevelopmentError::Invalid);
    }
    handoff.participants.sort_by(|left, right| {
        (left.ordering_hint, &left.project_id).cmp(&(right.ordering_hint, &right.project_id))
    });
    handoff.blockers.sort();
    handoff.blockers.dedup();
    handoff.ready_for_change_bundle = handoff.blockers.is_empty()
        && handoff
            .participants
            .iter()
            .all(|participant| participant.state == "ready");
    handoff.content_fingerprint = fingerprint(
        CROSS_PROJECT_MIGRATION_HANDOFF_SCHEMA_ID,
        &serde_json::json!({
            "handoff_id": handoff.handoff_id,
            "participants": handoff.participants,
            "dependency_edges": handoff.dependency_edges,
            "blockers": handoff.blockers,
            "ready_for_change_bundle": handoff.ready_for_change_bundle,
        }),
    )?;
    Ok(handoff)
}

fn validate_chain(
    chain: &MigrationChain,
    invariants: &BTreeSet<&str>,
) -> Result<(), DevelopmentError> {
    let mut step_ids = BTreeSet::new();
    let mut edges = BTreeSet::new();
    for (index, step) in chain.steps.iter().enumerate() {
        if !token(&step.step_id, 128)
            || step.step_version.trim().is_empty()
            || step.from_version.trim().is_empty()
            || step.to_version.trim().is_empty()
            || step.from_version == step.to_version
            || !step_ids.insert(step.step_id.as_str())
            || !edges.insert((step.from_version.as_str(), step.to_version.as_str()))
            || step.invocation_template_ref.trim().is_empty()
            || step.expected_output.trim().is_empty()
            || step.rollback_ref.trim().is_empty()
            || step.tool_ref.trim().is_empty()
            || step.normalizer_ref.trim().is_empty()
            || step
                .write_scope
                .iter()
                .any(|path| !safe_relative_path(path))
            || step
                .invariant_refs
                .iter()
                .any(|invariant| !invariants.contains(invariant.as_str()))
            || index > 0 && chain.steps[index - 1].to_version != step.from_version
        {
            return Err(DevelopmentError::Invalid);
        }
    }
    Ok(())
}

fn validate_vector(vector: &MigrationVersionVector) -> Result<(), DevelopmentError> {
    let expected = fingerprint("star.migration-version-vector", &vector.entries)?;
    if vector.vector_fingerprint != expected {
        return Err(DevelopmentError::Conflict);
    }
    Ok(())
}

fn resolve_chain(
    manifest: &ProjectMigrationManifest,
    target_id: &str,
    from: &str,
    to: &str,
) -> (
    MigrationSupportDecision,
    Vec<ResolvedMigrationStep>,
    Vec<String>,
) {
    let matches = manifest
        .migration_chains
        .iter()
        .filter(|chain| chain.target_id == target_id)
        .filter_map(|chain| {
            let start = chain
                .steps
                .iter()
                .position(|step| step.from_version == from)?;
            let end = chain.steps.iter().position(|step| step.to_version == to)?;
            (start <= end).then(|| &chain.steps[start..=end])
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => (
            MigrationSupportDecision::ChainGap,
            Vec::new(),
            vec!["MIGRATION_CHAIN_GAP".to_owned()],
        ),
        [_first, _second, ..] => (
            MigrationSupportDecision::AmbiguousChain,
            Vec::new(),
            vec!["MIGRATION_CHAIN_AMBIGUOUS".to_owned()],
        ),
        [steps] => (
            MigrationSupportDecision::Migratable,
            steps
                .iter()
                .enumerate()
                .map(|(index, step)| ResolvedMigrationStep {
                    order: u32::try_from(index + 1).unwrap_or(u32::MAX),
                    step_id: step.step_id.clone(),
                    step_version: step.step_version.clone(),
                    definition_fingerprint: step.definition_fingerprint.clone(),
                    from_version: step.from_version.clone(),
                    to_version: step.to_version.clone(),
                    effect_class: step.effect_class,
                    idempotency_contract: step.idempotency_contract,
                    invocation_template_ref: step.invocation_template_ref.clone(),
                    rollback_ref: step.rollback_ref.clone(),
                })
                .collect(),
            Vec::new(),
        ),
    }
}

fn phase_plan(phase: &str, required: bool, expected_output: &str) -> MigrationPhasePlan {
    MigrationPhasePlan {
        phase: phase.to_owned(),
        required,
        input_refs: Vec::new(),
        expected_output: expected_output.to_owned(),
        stop_condition: "diagnostic block, stale subject, or unknown outcome".to_owned(),
    }
}

fn migration_plan_fingerprint(plan: &MigrationPlanV2) -> Result<Sha256Hash, DevelopmentError> {
    fingerprint(
        MIGRATION_PLAN_V2_SCHEMA_ID,
        &serde_json::json!({
            "migration_plan_id": plan.migration_plan_id,
            "revision": plan.revision,
            "task_spec_ref": plan.task_spec_ref,
            "scope_revision_ref": plan.scope_revision_ref,
            "impact_analysis_ref": plan.impact_analysis_ref,
            "project_id": plan.project_id,
            "checkout_id": plan.checkout_id,
            "source_subject_fingerprint": plan.source_subject_fingerprint,
            "manifest_ref": plan.manifest_ref,
            "manifest_fingerprint": plan.manifest_fingerprint,
            "target_id": plan.target_id,
            "observed_version_vector": plan.observed_version_vector,
            "target_version_vector": plan.target_version_vector,
            "support_decision": plan.support_decision,
            "ordered_steps": plan.ordered_steps,
            "invariant_refs": plan.invariant_refs,
            "strategy": plan.strategy,
            "resource_estimate": plan.resource_estimate,
            "dry_run_plan": plan.dry_run_plan,
            "backup_plan": plan.backup_plan,
            "rehearsal_plan": plan.rehearsal_plan,
            "activation_plan": plan.activation_plan,
            "resume_plan": plan.resume_plan,
            "rollback_plan_ref": plan.rollback_plan_ref,
            "validation_plan_refs": plan.validation_plan_refs,
            "permission_checkpoints": plan.permission_checkpoints,
            "source_patch_refs": plan.source_patch_refs,
            "consumer_compatibility_refs": plan.consumer_compatibility_refs,
            "cross_project_handoff_ref": plan.cross_project_handoff_ref,
            "blockers": plan.blockers,
        }),
    )
}

fn migration_phase_prerequisites_met(
    plan: &MigrationPlanV2,
    previous: &[MigrationAttempt],
    phase: MigrationPhase,
) -> bool {
    let succeeded = |required: MigrationPhase| {
        previous.iter().any(|attempt| {
            attempt.phase == required && attempt.state == MigrationAttemptState::Succeeded
        })
    };
    match phase {
        MigrationPhase::DryRun => true,
        MigrationPhase::Backup => succeeded(MigrationPhase::DryRun),
        MigrationPhase::BackupVerify => succeeded(MigrationPhase::Backup),
        MigrationPhase::RestoreRehearsal => succeeded(MigrationPhase::BackupVerify),
        MigrationPhase::MigrationRehearsal => succeeded(MigrationPhase::RestoreRehearsal),
        MigrationPhase::PreExecuteGate => succeeded(MigrationPhase::MigrationRehearsal),
        MigrationPhase::Execute => {
            plan.blockers.is_empty() && succeeded(MigrationPhase::PreExecuteGate)
        }
        MigrationPhase::Resume | MigrationPhase::Reconcile => previous.iter().any(|attempt| {
            matches!(
                attempt.state,
                MigrationAttemptState::PartiallyApplied | MigrationAttemptState::OutcomeUnknown
            )
        }),
        MigrationPhase::Validate => {
            succeeded(MigrationPhase::Execute) || succeeded(MigrationPhase::Resume)
        }
        MigrationPhase::Activate => succeeded(MigrationPhase::Validate),
        MigrationPhase::ConsumerValidate => succeeded(MigrationPhase::Activate),
        MigrationPhase::Rollback => previous.iter().any(|attempt| {
            matches!(
                attempt.phase,
                MigrationPhase::Execute | MigrationPhase::Resume | MigrationPhase::Activate
            )
        }),
        MigrationPhase::PostRollbackValidate => succeeded(MigrationPhase::Rollback),
    }
}

fn metric_values(runs: &[PerformanceRun], metric_id: &str) -> Vec<f64> {
    runs.iter()
        .filter_map(|run| {
            run.measurements
                .iter()
                .find(|measurement| measurement.metric_id == metric_id)
                .map(|measurement| measurement.value)
        })
        .collect()
}

fn median(values: &[f64]) -> f64 {
    if values.len().is_multiple_of(2) {
        (values[values.len() / 2 - 1] + values[values.len() / 2]) / 2.0
    } else {
        values[values.len() / 2]
    }
}

fn percentile(values: &[f64], percentile: f64) -> f64 {
    let rank = ((values.len() as f64) * percentile).ceil() as usize;
    values[rank.saturating_sub(1).min(values.len() - 1)]
}

fn coefficient_of_variation(values: &[f64]) -> f64 {
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    if mean == 0.0 {
        return 0.0;
    }
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / values.len() as f64;
    variance.sqrt() / mean
}

fn aggregate_performance_state(
    values: impl Iterator<Item = PerformanceComparisonState>,
) -> PerformanceComparisonState {
    values.fold(PerformanceComparisonState::Pass, |state, item| {
        let rank = |value| match value {
            PerformanceComparisonState::Pass => 0,
            PerformanceComparisonState::HumanReview => 1,
            PerformanceComparisonState::NoiseInconclusive => 2,
            PerformanceComparisonState::CorrectnessUnverified => 3,
            PerformanceComparisonState::Incomparable => 4,
            PerformanceComparisonState::Regression => 5,
        };
        if rank(item) > rank(state) {
            item
        } else {
            state
        }
    })
}

fn aggregate_equivalence(report: &EquivalenceReport) -> EquivalenceState {
    let required = report
        .dimension_results
        .iter()
        .filter(|dimension| dimension.required)
        .collect::<Vec<_>>();
    if required.is_empty() {
        return EquivalenceState::NotEvaluated;
    }
    if required
        .iter()
        .any(|dimension| dimension.state == EquivalenceDimensionState::NotEquivalent)
    {
        EquivalenceState::NotEquivalent
    } else if required
        .iter()
        .any(|dimension| dimension.state == EquivalenceDimensionState::Unverified)
    {
        EquivalenceState::Unverified
    } else if !report.unknown_semantics.is_empty()
        || required
            .iter()
            .any(|dimension| dimension.state == EquivalenceDimensionState::HumanReview)
    {
        EquivalenceState::HumanReview
    } else if required.iter().all(|dimension| {
        matches!(
            dimension.state,
            EquivalenceDimensionState::Equivalent | EquivalenceDimensionState::NotRequired
        )
    }) && report.build_compile_result == "pass"
    {
        EquivalenceState::Equivalent
    } else {
        EquivalenceState::Partial
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::{
        ProjectId,
        migration_v2::{IdempotencyContract, PerformanceMeasurement, PerformanceMetricSpec},
    };

    fn workload(project_id: ProjectId) -> PerformanceWorkloadSpec {
        seal_performance_workload(PerformanceWorkloadSpec {
            schema_id: PERFORMANCE_WORKLOAD_SPEC_SCHEMA_ID.to_owned(),
            schema_version: 1,
            workload_id: "scan-hot-path".to_owned(),
            project_id,
            task_ref: "task:scan".to_owned(),
            input_fingerprint: Sha256Hash::digest(b"input"),
            environment_class: "windows-x64".to_owned(),
            build_mode: "release".to_owned(),
            warmup_count: 1,
            measured_count: 3,
            outlier_policy: "none".to_owned(),
            noise_budget_ratio: 0.2,
            metrics: vec![PerformanceMetricSpec {
                metric_id: "duration_ms".to_owned(),
                unit: "ms".to_owned(),
                direction: "lower_is_better".to_owned(),
                budget_ratio: 1.2,
                required: true,
            }],
            correctness_check_refs: vec!["check:scan".to_owned()],
            specification_fingerprint: placeholder(),
        })
        .unwrap()
    }

    fn run(
        workload: &PerformanceWorkloadSpec,
        cohort: PerformanceCohort,
        attempt: u32,
        value: f64,
    ) -> PerformanceRun {
        seal_performance_run(
            workload,
            PerformanceRun {
                schema_id: PERFORMANCE_RUN_SCHEMA_ID.to_owned(),
                schema_version: 1,
                run_id: format!("run-{cohort:?}-{attempt}"),
                workload_ref: workload.workload_id.clone(),
                workload_fingerprint: workload.specification_fingerprint.clone(),
                cohort,
                attempt,
                warmup: false,
                subject_fingerprint: Sha256Hash::digest(format!("{cohort:?}").as_bytes()),
                environment_fingerprint: Sha256Hash::digest(b"environment"),
                toolchain_fingerprint: Sha256Hash::digest(b"toolchain"),
                build_mode: "release".to_owned(),
                measurements: vec![PerformanceMeasurement {
                    metric_id: "duration_ms".to_owned(),
                    value,
                    unit: "ms".to_owned(),
                    collector: "registered-clock".to_owned(),
                }],
                correctness_passed: true,
                evidence_refs: Vec::new(),
                run_fingerprint: placeholder(),
            },
        )
        .unwrap()
    }

    #[test]
    fn performance_comparison_requires_same_binding_and_numeric_budget() {
        let workload = workload(ProjectId::new());
        let baseline = (1..=3)
            .map(|attempt| run(&workload, PerformanceCohort::Baseline, attempt, 10.0))
            .collect::<Vec<_>>();
        let candidate = (1..=3)
            .map(|attempt| run(&workload, PerformanceCohort::Candidate, attempt, 13.0))
            .collect::<Vec<_>>();
        let comparison = compare_performance_runs(
            "comparison-one".to_owned(),
            &workload,
            &baseline,
            &candidate,
        )
        .unwrap();
        assert_eq!(comparison.state, PerformanceComparisonState::Regression);
    }

    #[test]
    fn not_replay_safe_checkpoint_requires_reconciliation() {
        let plan = MigrationPlanV2 {
            schema_id: MIGRATION_PLAN_V2_SCHEMA_ID.to_owned(),
            schema_version: 2,
            migration_plan_id: "migration-one".to_owned(),
            revision: 1,
            task_spec_ref: "task:one".to_owned(),
            scope_revision_ref: "scope:one".to_owned(),
            impact_analysis_ref: "impact:one".to_owned(),
            project_id: ProjectId::new(),
            checkout_id: "checkout:one".to_owned(),
            source_subject_fingerprint: Sha256Hash::digest(b"source"),
            manifest_ref: "manifest:one".to_owned(),
            manifest_fingerprint: Sha256Hash::digest(b"manifest"),
            target_id: "database".to_owned(),
            observed_version_vector: seal_migration_version_vector(vec![MigrationVersionEntry {
                axis_id: "database".to_owned(),
                owner: "db".to_owned(),
                observed_version: Some("1".to_owned()),
                version_scheme: "integer".to_owned(),
                source_ref: "probe:db".to_owned(),
                source_fingerprint: Sha256Hash::digest(b"probe"),
                coverage: CoverageState::Complete,
                observation_state: VersionObservationState::Observed,
            }])
            .unwrap(),
            target_version_vector: seal_migration_version_vector(vec![MigrationVersionEntry {
                axis_id: "database".to_owned(),
                owner: "db".to_owned(),
                observed_version: Some("2".to_owned()),
                version_scheme: "integer".to_owned(),
                source_ref: "probe:db".to_owned(),
                source_fingerprint: Sha256Hash::digest(b"probe"),
                coverage: CoverageState::Complete,
                observation_state: VersionObservationState::Observed,
            }])
            .unwrap(),
            support_decision: MigrationSupportDecision::Migratable,
            ordered_steps: vec![ResolvedMigrationStep {
                order: 1,
                step_id: "one-two".to_owned(),
                step_version: "1.0.0".to_owned(),
                definition_fingerprint: Sha256Hash::digest(b"step"),
                from_version: "1".to_owned(),
                to_version: "2".to_owned(),
                effect_class: MigrationEffectClass::LiveDestructive,
                idempotency_contract: IdempotencyContract::NotReplaySafe,
                invocation_template_ref: "tool:migrate".to_owned(),
                rollback_ref: "recovery:one".to_owned(),
            }],
            invariant_refs: Vec::new(),
            strategy: MigrationStrategy::SideBySide,
            resource_estimate: "unknown".to_owned(),
            dry_run_plan: phase_plan("dry_run", true, "preview"),
            backup_plan: phase_plan("backup", true, "backup"),
            rehearsal_plan: phase_plan("rehearsal", true, "rehearsal"),
            activation_plan: phase_plan("activate", true, "active"),
            resume_plan: phase_plan("resume", true, "resume"),
            rollback_plan_ref: "recovery:one".to_owned(),
            validation_plan_refs: Vec::new(),
            permission_checkpoints: vec!["live_destructive".to_owned()],
            source_patch_refs: Vec::new(),
            consumer_compatibility_refs: Vec::new(),
            cross_project_handoff_ref: None,
            blockers: Vec::new(),
            plan_fingerprint: Sha256Hash::digest(b"plan"),
        };
        let checkpoint = MigrationCheckpointV2 {
            schema_id: MIGRATION_CHECKPOINT_V2_SCHEMA_ID.to_owned(),
            schema_version: 2,
            checkpoint_id: "checkpoint-one".to_owned(),
            plan_ref: plan.migration_plan_id.clone(),
            plan_fingerprint: plan.plan_fingerprint.clone(),
            completed_step_refs: Vec::new(),
            in_progress_step_ref: Some("one-two".to_owned()),
            target_version: "1".to_owned(),
            target_state_fingerprint: Sha256Hash::digest(b"state"),
            last_receipt_ref: None,
            replay_safe: false,
            reconciliation_required: false,
            checkpoint_fingerprint: placeholder(),
        };
        assert_eq!(
            seal_migration_checkpoint(&plan, checkpoint).unwrap_err(),
            DevelopmentError::Conflict
        );
    }
}
