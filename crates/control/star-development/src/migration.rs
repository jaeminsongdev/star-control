use star_contracts::{
    Sha256Hash,
    development::{
        MIGRATION_RUN_SCHEMA_ID, MigrationCheckpoint, MigrationRun, MigrationRunState,
        MigrationStep, PERFORMANCE_COMPARISON_SCHEMA_ID, PerformanceComparison, PerformanceState,
        PlatformMigrationEvidence, PlatformVerificationState,
    },
};

use crate::{DevelopmentError, fingerprint, placeholder, token};

#[derive(Clone, Debug)]
pub struct MigrationStepReceipt {
    pub state_fingerprint: Sha256Hash,
}

pub trait MigrationExecutor {
    fn execute_step(
        &mut self,
        step: &MigrationStep,
        current_state: &Sha256Hash,
    ) -> Result<MigrationStepReceipt, DevelopmentError>;

    fn rollback(
        &mut self,
        source_version: u32,
        source_state: &Sha256Hash,
    ) -> Result<(), DevelopmentError>;
}

pub fn start_migration(
    migration_id: &str,
    source_version: u32,
    target_version: u32,
    source_state: Sha256Hash,
    mut steps: Vec<MigrationStep>,
) -> Result<MigrationRun, DevelopmentError> {
    steps.sort_by_key(|step| step.from_version);
    if !token(migration_id, 160)
        || source_version >= target_version
        || steps.is_empty()
        || steps
            .first()
            .is_none_or(|step| step.from_version != source_version)
        || steps
            .last()
            .is_none_or(|step| step.to_version != target_version)
        || steps
            .windows(2)
            .any(|pair| pair[0].to_version != pair[1].from_version)
        || steps
            .iter()
            .any(|step| step.from_version >= step.to_version || !token(&step.step_id, 128))
    {
        return Err(DevelopmentError::Invalid);
    }
    seal_migration(MigrationRun {
        schema_id: MIGRATION_RUN_SCHEMA_ID.to_owned(),
        schema_version: 1,
        migration_id: migration_id.to_owned(),
        source_version,
        target_version,
        steps,
        checkpoint: MigrationCheckpoint {
            completed_step_ids: vec![],
            current_version: source_version,
            state_fingerprint: source_state,
        },
        state: MigrationRunState::Planned,
        limitations: vec![],
        run_fingerprint: placeholder(),
    })
}

pub fn resume_migration(
    mut run: MigrationRun,
    executor: &mut dyn MigrationExecutor,
) -> Result<MigrationRun, DevelopmentError> {
    validate_checkpoint(&run)?;
    if run.state == MigrationRunState::Completed {
        return Ok(run);
    }
    if matches!(
        run.state,
        MigrationRunState::RolledBack | MigrationRunState::Failed
    ) {
        return Err(DevelopmentError::Blocked);
    }
    run.state = MigrationRunState::Running;
    for step in run.steps.clone() {
        if run.checkpoint.completed_step_ids.contains(&step.step_id) {
            continue;
        }
        if step.from_version != run.checkpoint.current_version {
            return Err(DevelopmentError::Conflict);
        }
        match executor.execute_step(&step, &run.checkpoint.state_fingerprint) {
            Ok(receipt) => {
                run.checkpoint.completed_step_ids.push(step.step_id);
                run.checkpoint.current_version = step.to_version;
                run.checkpoint.state_fingerprint = receipt.state_fingerprint;
            }
            Err(_) => {
                run.state = MigrationRunState::Interrupted;
                run.limitations = vec!["step_interrupted_without_promotion".to_owned()];
                return seal_migration(run);
            }
        }
    }
    run.state = if run.checkpoint.current_version == run.target_version {
        MigrationRunState::Completed
    } else {
        MigrationRunState::RollbackRequired
    };
    seal_migration(run)
}

pub fn rollback_migration(
    mut run: MigrationRun,
    source_state: Sha256Hash,
    executor: &mut dyn MigrationExecutor,
) -> Result<MigrationRun, DevelopmentError> {
    if run.state == MigrationRunState::Completed || run.state == MigrationRunState::Running {
        return Err(DevelopmentError::Blocked);
    }
    executor.rollback(run.source_version, &source_state)?;
    run.checkpoint = MigrationCheckpoint {
        completed_step_ids: vec![],
        current_version: run.source_version,
        state_fingerprint: source_state,
    };
    run.state = MigrationRunState::RolledBack;
    run.limitations.clear();
    seal_migration(run)
}

pub fn compare_performance(
    workload_id: &str,
    reference_binding: &Sha256Hash,
    candidate_binding: &Sha256Hash,
    mut reference_samples_ms: Vec<f64>,
    mut candidate_samples_ms: Vec<f64>,
    budget_ratio: f64,
) -> Result<PerformanceComparison, DevelopmentError> {
    if !token(workload_id, 128)
        || reference_samples_ms.len() < 5
        || candidate_samples_ms.len() < 5
        || !budget_ratio.is_finite()
        || budget_ratio < 1.0
        || reference_samples_ms
            .iter()
            .chain(candidate_samples_ms.iter())
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(DevelopmentError::Invalid);
    }
    reference_samples_ms.sort_by(f64::total_cmp);
    candidate_samples_ms.sort_by(f64::total_cmp);
    let reference_p95_ms = percentile_95(&reference_samples_ms);
    let candidate_p95_ms = percentile_95(&candidate_samples_ms);
    let state = if reference_binding != candidate_binding {
        PerformanceState::Incomparable
    } else if candidate_p95_ms <= reference_p95_ms * budget_ratio {
        PerformanceState::Pass
    } else {
        PerformanceState::Regression
    };
    let mut comparison = PerformanceComparison {
        schema_id: PERFORMANCE_COMPARISON_SCHEMA_ID.to_owned(),
        schema_version: 1,
        workload_id: workload_id.to_owned(),
        binding_fingerprint: if reference_binding == candidate_binding {
            reference_binding.clone()
        } else {
            fingerprint(
                "star.incomparable-performance-bindings",
                &(reference_binding, candidate_binding),
            )?
        },
        reference_samples_ms,
        candidate_samples_ms,
        reference_p95_ms,
        candidate_p95_ms,
        budget_ratio,
        state,
        comparison_fingerprint: placeholder(),
    };
    comparison.comparison_fingerprint = fingerprint(
        PERFORMANCE_COMPARISON_SCHEMA_ID,
        &serde_json::json!({
            "workload_id":comparison.workload_id,
            "binding_fingerprint":comparison.binding_fingerprint,
            "reference_samples_ms":comparison.reference_samples_ms,
            "candidate_samples_ms":comparison.candidate_samples_ms,
            "reference_p95_ms":comparison.reference_p95_ms,
            "candidate_p95_ms":comparison.candidate_p95_ms,
            "budget_ratio":comparison.budget_ratio,
            "state":comparison.state,
        }),
    )?;
    Ok(comparison)
}

pub fn platform_migration_evidence(
    target_triple: &str,
    artifact_sha256: Sha256Hash,
    architecture: &str,
    cross_build_complete: bool,
    native_executed: bool,
    mut simulation_checks: Vec<String>,
) -> Result<PlatformMigrationEvidence, DevelopmentError> {
    simulation_checks.sort();
    simulation_checks.dedup();
    if target_triple.trim().is_empty()
        || architecture.trim().is_empty()
        || simulation_checks
            .iter()
            .any(|check| check.trim().is_empty())
    {
        return Err(DevelopmentError::Invalid);
    }
    let state = if native_executed && cross_build_complete {
        PlatformVerificationState::NativeVerified
    } else if cross_build_complete && !simulation_checks.is_empty() {
        PlatformVerificationState::NativeUnverified
    } else {
        PlatformVerificationState::Unsupported
    };
    Ok(PlatformMigrationEvidence {
        target_triple: target_triple.to_owned(),
        artifact_sha256,
        architecture: architecture.to_owned(),
        cross_build_complete,
        simulation_checks,
        state,
    })
}

fn validate_checkpoint(run: &MigrationRun) -> Result<(), DevelopmentError> {
    if run.schema_id != MIGRATION_RUN_SCHEMA_ID || run.schema_version != 1 {
        return Err(DevelopmentError::Invalid);
    }
    let expected_prefix = run
        .steps
        .iter()
        .take(run.checkpoint.completed_step_ids.len())
        .map(|step| step.step_id.as_str())
        .collect::<Vec<_>>();
    let actual = run
        .checkpoint
        .completed_step_ids
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if actual != expected_prefix
        || (!actual.is_empty()
            && run.steps[actual.len() - 1].to_version != run.checkpoint.current_version)
        || actual.is_empty() && run.checkpoint.current_version != run.source_version
    {
        return Err(DevelopmentError::Conflict);
    }
    Ok(())
}

fn seal_migration(mut run: MigrationRun) -> Result<MigrationRun, DevelopmentError> {
    run.run_fingerprint = fingerprint(
        MIGRATION_RUN_SCHEMA_ID,
        &serde_json::json!({
            "migration_id":run.migration_id,
            "source_version":run.source_version,
            "target_version":run.target_version,
            "steps":run.steps,
            "checkpoint":run.checkpoint,
            "state":run.state,
            "limitations":run.limitations,
        }),
    )?;
    Ok(run)
}

fn percentile_95(values: &[f64]) -> f64 {
    let rank = ((values.len() as f64) * 0.95).ceil() as usize;
    values[rank.saturating_sub(1).min(values.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeMigration {
        calls: usize,
        fail_at: Option<usize>,
        rollbacks: usize,
    }

    impl MigrationExecutor for FakeMigration {
        fn execute_step(
            &mut self,
            step: &MigrationStep,
            _current_state: &Sha256Hash,
        ) -> Result<MigrationStepReceipt, DevelopmentError> {
            self.calls += 1;
            if self.fail_at == Some(self.calls) {
                return Err(DevelopmentError::Adapter);
            }
            Ok(MigrationStepReceipt {
                state_fingerprint: Sha256Hash::digest(step.step_id.as_bytes()),
            })
        }

        fn rollback(
            &mut self,
            _source_version: u32,
            _source_state: &Sha256Hash,
        ) -> Result<(), DevelopmentError> {
            self.rollbacks += 1;
            Ok(())
        }
    }

    fn steps() -> Vec<MigrationStep> {
        vec![
            MigrationStep {
                step_id: "v1-v2".to_owned(),
                from_version: 1,
                to_version: 2,
                input_fingerprint: Sha256Hash::digest(b"one"),
            },
            MigrationStep {
                step_id: "v2-v3".to_owned(),
                from_version: 2,
                to_version: 3,
                input_fingerprint: Sha256Hash::digest(b"two"),
            },
        ]
    }

    #[test]
    fn interrupted_migration_resumes_from_exact_prefix_and_can_rollback() {
        let source = Sha256Hash::digest(b"source");
        let run = start_migration("fixture", 1, 3, source.clone(), steps()).unwrap();
        let mut failing = FakeMigration {
            fail_at: Some(2),
            ..Default::default()
        };
        let interrupted = resume_migration(run, &mut failing).unwrap();
        assert_eq!(interrupted.state, MigrationRunState::Interrupted);
        assert_eq!(interrupted.checkpoint.completed_step_ids, ["v1-v2"]);
        let mut resumed = FakeMigration::default();
        let completed = resume_migration(interrupted.clone(), &mut resumed).unwrap();
        assert_eq!(completed.state, MigrationRunState::Completed);
        assert_eq!(resumed.calls, 1);
        let mut rollback = FakeMigration::default();
        let rolled_back = rollback_migration(interrupted, source, &mut rollback).unwrap();
        assert_eq!(rolled_back.state, MigrationRunState::RolledBack);
        assert_eq!(rollback.rollbacks, 1);
    }

    #[test]
    fn performance_requires_comparable_binding_and_arm64_simulation_is_unverified() {
        let binding = Sha256Hash::digest(b"binding");
        let pass = compare_performance(
            "scan",
            &binding,
            &binding,
            vec![10.0; 5],
            vec![11.9; 5],
            1.2,
        )
        .unwrap();
        assert_eq!(pass.state, PerformanceState::Pass);
        let other = Sha256Hash::digest(b"other");
        let incomparable =
            compare_performance("scan", &binding, &other, vec![10.0; 5], vec![1.0; 5], 1.2)
                .unwrap();
        assert_eq!(incomparable.state, PerformanceState::Incomparable);
        let arm64 = platform_migration_evidence(
            "aarch64-pc-windows-msvc",
            Sha256Hash::digest(b"arm64"),
            "arm64",
            true,
            false,
            vec!["pe_machine_arm64".to_owned(), "fake_lifecycle".to_owned()],
        )
        .unwrap();
        assert_eq!(arm64.state, PlatformVerificationState::NativeUnverified);
    }
}
