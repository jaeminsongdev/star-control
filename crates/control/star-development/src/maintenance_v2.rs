use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use star_contracts::{
    ProjectId, Sha256Hash,
    development_v2::CoverageState,
    maintenance_v2::{
        DEPENDENCY_SNAPSHOT_SCHEMA_ID, DEPENDENCY_UPDATE_PLAN_SCHEMA_ID, DependencyRecord,
        DependencySnapshot, DependencyUpdatePlan, DependencyUpdateStatus,
        EXTERNAL_DATA_SNAPSHOT_SCHEMA_ID, ExternalDataObservation, ExternalDataSnapshot,
        ExternalDataSourceDescriptor, ExternalFreshness, FAILURE_RECORD_SCHEMA_ID,
        FailureCausalityRole, FailureInvocation, FailureKind, FailureRecord, FailureSubjectBinding,
        MAINTENANCE_RADAR_SNAPSHOT_SCHEMA_ID, MaintenanceRadarItem, MaintenanceRadarSnapshot,
        PrimarySymptom, RECOVERY_PLAN_V2_SCHEMA_ID, REGRESSION_RECORD_SCHEMA_ID,
        REPRODUCTION_PACK_V2_SCHEMA_ID, RecoveryPlanState, RecoveryPlanV2, RegressionRecord,
        RegressionState, ReproductionArtifactRef, ReproductionAttemptV2, ReproductionPackV2,
        ReproductionResult, RootCandidateRef, SUPPLY_CHAIN_SNAPSHOT_SCHEMA_ID,
        SupplyChainObservation, SupplyChainSnapshot, UpdateCandidate, VerificationState,
    },
};

use crate::{DevelopmentError, fingerprint, placeholder, safe_relative_path, token};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FailureRecordInput {
    pub failure_record_id: String,
    pub occurrence_id: String,
    #[serde(default)]
    pub diagnostic_refs: Vec<String>,
    #[serde(default)]
    pub finding_refs: Vec<String>,
    pub subject_binding: FailureSubjectBinding,
    pub failure_kind: FailureKind,
    pub producer_code: String,
    pub raw_message: String,
    pub logical_owner: String,
    pub signature: String,
    pub causality_role: FailureCausalityRole,
    #[serde(default)]
    pub root_candidate_refs: Vec<RootCandidateRef>,
    #[serde(default)]
    pub cascade_parent_refs: Vec<String>,
    pub invocation: FailureInvocation,
    pub environment_compatibility_class: String,
    pub environment_fingerprint: Sha256Hash,
    #[serde(default)]
    pub input_refs: Vec<String>,
    pub input_fingerprint: Sha256Hash,
    pub seed: Option<String>,
    pub manifest_fingerprint: Option<Sha256Hash>,
    pub stdout_ref: Option<String>,
    pub stderr_ref: Option<String>,
    #[serde(default)]
    pub artifact_refs: Vec<String>,
    pub observed_at: String,
    pub attempt_id: String,
    pub verification_state: VerificationState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReproductionPackInput {
    pub reproduction_pack_id: String,
    pub dirty_state: String,
    #[serde(default)]
    pub manifest_refs: Vec<String>,
    pub expected_result: String,
    pub observed_result: String,
    pub attempts: Vec<ReproductionAttemptV2>,
    #[serde(default)]
    pub artifacts: Vec<ReproductionArtifactRef>,
    #[serde(default)]
    pub limitations: Vec<String>,
}

pub fn build_failure_record(input: FailureRecordInput) -> Result<FailureRecord, DevelopmentError> {
    if !token(&input.failure_record_id, 192)
        || !token(&input.occurrence_id, 192)
        || !token(&input.producer_code, 160)
        || input.raw_message.trim().is_empty()
        || input.logical_owner.trim().is_empty()
        || input.signature.trim().is_empty()
        || input.invocation.command_descriptor.trim().is_empty()
        || input.invocation.executable_identity.trim().is_empty()
        || input.invocation.logical_cwd.trim().is_empty()
        || input.invocation.timeout_ms == 0
        || input.environment_compatibility_class.trim().is_empty()
        || input.observed_at.trim().is_empty()
        || !token(&input.attempt_id, 192)
        || input.root_candidate_refs.iter().any(|candidate| {
            !candidate.confidence.is_finite()
                || !(0.0..=1.0).contains(&candidate.confidence)
                || candidate.reason.trim().is_empty()
        })
        || input
            .cascade_parent_refs
            .iter()
            .any(|parent| parent == &input.failure_record_id)
    {
        return Err(DevelopmentError::Invalid);
    }
    let message_template = normalize_failure_message(&input.raw_message);
    if message_template.is_empty() || contains_sensitive_shape(&message_template) {
        return Err(DevelopmentError::Blocked);
    }
    let primary_symptom = PrimarySymptom {
        producer_code: input.producer_code,
        message_template,
        logical_owner: input.logical_owner,
        signature: input.signature,
        normalization_version: 1,
    };
    let family_fingerprint = fingerprint(
        "star.failure-family.v2",
        &serde_json::json!({
            "contract_version": 2,
            "failure_kind": input.failure_kind,
            "producer_code": primary_symptom.producer_code,
            "message_template": primary_symptom.message_template,
            "logical_owner": primary_symptom.logical_owner,
            "signature": primary_symptom.signature,
            "command_descriptor": input.invocation.command_descriptor,
            "structured_arg_shape": input.invocation.structured_args.iter().map(|arg| redact_argument_shape(arg)).collect::<Vec<_>>(),
            "environment_compatibility_class": input.environment_compatibility_class,
            "tool_compatibility_class": input.invocation.executable_identity,
            "normalization_version": 1,
        }),
    )?;
    let occurrence_fingerprint = fingerprint(
        "star.failure-occurrence.v2",
        &serde_json::json!({
            "family_fingerprint": family_fingerprint,
            "subject_binding": input.subject_binding,
            "structured_args": input.invocation.structured_args,
            "logical_cwd": input.invocation.logical_cwd,
            "environment_fingerprint": input.environment_fingerprint,
            "executable_identity": input.invocation.executable_identity,
            "input_fingerprint": input.input_fingerprint,
            "seed": input.seed,
            "manifest_fingerprint": input.manifest_fingerprint,
        }),
    )?;
    let mut diagnostic_refs = input.diagnostic_refs;
    diagnostic_refs.sort();
    diagnostic_refs.dedup();
    let mut finding_refs = input.finding_refs;
    finding_refs.sort();
    finding_refs.dedup();
    let mut cascade_parent_refs = input.cascade_parent_refs;
    cascade_parent_refs.sort();
    cascade_parent_refs.dedup();
    let mut record = FailureRecord {
        schema_id: FAILURE_RECORD_SCHEMA_ID.to_owned(),
        schema_version: 1,
        failure_record_id: input.failure_record_id,
        occurrence_id: input.occurrence_id,
        diagnostic_refs,
        finding_refs,
        subject_binding: input.subject_binding,
        failure_kind: input.failure_kind,
        family_fingerprint,
        occurrence_fingerprint,
        primary_symptom,
        causality_role: input.causality_role,
        root_candidate_refs: input.root_candidate_refs,
        cascade_parent_refs,
        invocation: input.invocation,
        environment_compatibility_class: input.environment_compatibility_class,
        environment_fingerprint: input.environment_fingerprint,
        input_refs: input.input_refs,
        seed: input.seed,
        stdout_ref: input.stdout_ref,
        stderr_ref: input.stderr_ref,
        artifact_refs: input.artifact_refs,
        observed_at: input.observed_at,
        attempt_id: input.attempt_id,
        verification_state: input.verification_state,
        content_fingerprint: placeholder(),
    };
    record.content_fingerprint = fingerprint(
        FAILURE_RECORD_SCHEMA_ID,
        &serde_json::json!({
            "failure_record_id": record.failure_record_id,
            "occurrence_id": record.occurrence_id,
            "diagnostic_refs": record.diagnostic_refs,
            "finding_refs": record.finding_refs,
            "subject_binding": record.subject_binding,
            "failure_kind": record.failure_kind,
            "family_fingerprint": record.family_fingerprint,
            "occurrence_fingerprint": record.occurrence_fingerprint,
            "primary_symptom": record.primary_symptom,
            "causality_role": record.causality_role,
            "root_candidate_refs": record.root_candidate_refs,
            "cascade_parent_refs": record.cascade_parent_refs,
            "invocation": record.invocation,
            "environment_compatibility_class": record.environment_compatibility_class,
            "environment_fingerprint": record.environment_fingerprint,
            "input_refs": record.input_refs,
            "seed": record.seed,
            "stdout_ref": record.stdout_ref,
            "stderr_ref": record.stderr_ref,
            "artifact_refs": record.artifact_refs,
            "observed_at": record.observed_at,
            "attempt_id": record.attempt_id,
            "verification_state": record.verification_state,
        }),
    )?;
    Ok(record)
}

pub fn build_reproduction_pack_v2(
    failure: &FailureRecord,
    mut input: ReproductionPackInput,
) -> Result<ReproductionPackV2, DevelopmentError> {
    if failure.schema_id != FAILURE_RECORD_SCHEMA_ID
        || !token(&input.reproduction_pack_id, 192)
        || input.dirty_state.trim().is_empty()
        || input.expected_result.trim().is_empty()
        || input.observed_result.trim().is_empty()
    {
        return Err(DevelopmentError::Invalid);
    }
    input.attempts.sort_by_key(|attempt| attempt.attempt);
    if input.attempts.is_empty()
        || input.attempts.iter().any(|attempt| attempt.attempt == 0)
        || input
            .attempts
            .windows(2)
            .any(|pair| pair[0].attempt == pair[1].attempt)
        || input.artifacts.iter().any(|artifact| {
            matches!(
                artifact.redaction_status.as_str(),
                "quarantined" | "unknown"
            ) && artifact.safe_for_default_report
        })
    {
        return Err(DevelopmentError::Invalid);
    }
    let result = if input.attempts.iter().any(|attempt| {
        attempt.result == ReproductionResult::Reproduced
            && attempt.family_fingerprint.as_ref() == Some(&failure.family_fingerprint)
    }) {
        ReproductionResult::Reproduced
    } else if input
        .attempts
        .iter()
        .any(|attempt| attempt.result == ReproductionResult::BlockedExternal)
    {
        ReproductionResult::BlockedExternal
    } else if input
        .attempts
        .iter()
        .any(|attempt| attempt.result == ReproductionResult::DifferentFailure)
    {
        ReproductionResult::DifferentFailure
    } else if input
        .attempts
        .iter()
        .any(|attempt| attempt.result == ReproductionResult::Incomplete)
    {
        ReproductionResult::Incomplete
    } else {
        ReproductionResult::NotReproduced
    };
    let completeness = if result == ReproductionResult::Incomplete
        || result == ReproductionResult::BlockedExternal
    {
        CoverageState::Partial
    } else {
        CoverageState::Complete
    };
    if result == ReproductionResult::BlockedExternal
        && !input
            .limitations
            .iter()
            .any(|limitation| !limitation.trim().is_empty())
    {
        return Err(DevelopmentError::Invalid);
    }
    input.limitations.sort();
    input.limitations.dedup();
    let mut pack = ReproductionPackV2 {
        schema_id: REPRODUCTION_PACK_V2_SCHEMA_ID.to_owned(),
        schema_version: 2,
        reproduction_pack_id: input.reproduction_pack_id,
        failure_record_ref: failure.failure_record_id.clone(),
        family_fingerprint: failure.family_fingerprint.clone(),
        occurrence_fingerprint: failure.occurrence_fingerprint.clone(),
        subject_binding: failure.subject_binding.clone(),
        dirty_state: input.dirty_state,
        invocation: failure.invocation.clone(),
        environment_compatibility_class: failure.environment_compatibility_class.clone(),
        environment_fingerprint: failure.environment_fingerprint.clone(),
        manifest_refs: input.manifest_refs,
        input_refs: failure.input_refs.clone(),
        seed: failure.seed.clone(),
        expected_result: input.expected_result,
        observed_result: input.observed_result,
        attempts: input.attempts,
        artifacts: input.artifacts,
        result,
        completeness,
        limitations: input.limitations,
        pack_fingerprint: placeholder(),
    };
    pack.pack_fingerprint = fingerprint(
        REPRODUCTION_PACK_V2_SCHEMA_ID,
        &serde_json::json!({
            "reproduction_pack_id": pack.reproduction_pack_id,
            "failure_record_ref": pack.failure_record_ref,
            "family_fingerprint": pack.family_fingerprint,
            "occurrence_fingerprint": pack.occurrence_fingerprint,
            "subject_binding": pack.subject_binding,
            "dirty_state": pack.dirty_state,
            "invocation": pack.invocation,
            "environment_compatibility_class": pack.environment_compatibility_class,
            "environment_fingerprint": pack.environment_fingerprint,
            "manifest_refs": pack.manifest_refs,
            "input_refs": pack.input_refs,
            "seed": pack.seed,
            "expected_result": pack.expected_result,
            "observed_result": pack.observed_result,
            "attempts": pack.attempts,
            "artifacts": pack.artifacts,
            "result": pack.result,
            "completeness": pack.completeness,
            "limitations": pack.limitations,
        }),
    )?;
    Ok(pack)
}

pub fn seal_regression_record(
    mut record: RegressionRecord,
) -> Result<RegressionRecord, DevelopmentError> {
    if record.schema_id != REGRESSION_RECORD_SCHEMA_ID
        || record.schema_version != 1
        || !token(&record.regression_record_id, 192)
        || record.before_failure_ref.trim().is_empty()
        || record.after_validation_ref.trim().is_empty()
    {
        return Err(DevelopmentError::Invalid);
    }
    record.recurrence_failure_refs.sort();
    record.recurrence_failure_refs.dedup();
    record.state = if !record.recurrence_failure_refs.is_empty() {
        RegressionState::Recurring
    } else if record.verification_state == VerificationState::Verified {
        RegressionState::Fixed
    } else if record.verification_state == VerificationState::Contradicted {
        RegressionState::Contradicted
    } else {
        RegressionState::Unverified
    };
    record.record_fingerprint = fingerprint(
        REGRESSION_RECORD_SCHEMA_ID,
        &serde_json::json!({
            "regression_record_id": record.regression_record_id,
            "family_fingerprint": record.family_fingerprint,
            "before_failure_ref": record.before_failure_ref,
            "after_validation_ref": record.after_validation_ref,
            "after_subject_fingerprint": record.after_subject_fingerprint,
            "recurrence_failure_refs": record.recurrence_failure_refs,
            "state": record.state,
            "verification_state": record.verification_state,
            "evidence_refs": record.evidence_refs,
        }),
    )?;
    Ok(record)
}

pub fn seal_recovery_plan(mut plan: RecoveryPlanV2) -> Result<RecoveryPlanV2, DevelopmentError> {
    if plan.schema_id != RECOVERY_PLAN_V2_SCHEMA_ID
        || plan.schema_version != 2
        || !token(&plan.recovery_plan_id, 192)
        || plan.failure_record_ref.trim().is_empty()
        || plan.owner.trim().is_empty()
        || plan.steps.is_empty()
    {
        return Err(DevelopmentError::Invalid);
    }
    plan.steps.sort_by_key(|step| step.order);
    let step_ids = plan
        .steps
        .iter()
        .map(|step| step.step_id.as_str())
        .collect::<BTreeSet<_>>();
    if step_ids.len() != plan.steps.len()
        || plan.steps.iter().any(|step| {
            !token(&step.step_id, 128)
                || step.order == 0
                || step.action.trim().is_empty()
                || step.expected_checkpoint.trim().is_empty()
                || step.validation_check_ref.trim().is_empty()
                || step.stop_condition.trim().is_empty()
                || step.destructive_effect && !step.permission_required
                || step
                    .prerequisite_step_ids
                    .iter()
                    .any(|required| !step_ids.contains(required.as_str()))
                || step
                    .fallback_step_id
                    .as_ref()
                    .is_some_and(|fallback| !step_ids.contains(fallback.as_str()))
        })
        || recovery_graph_has_cycle(&plan.steps)
    {
        return Err(DevelopmentError::Invalid);
    }
    plan.blockers.sort();
    plan.blockers.dedup();
    plan.state = if !plan.blockers.is_empty() {
        RecoveryPlanState::Blocked
    } else if plan.steps.iter().any(|step| step.permission_required) {
        RecoveryPlanState::AwaitingPermission
    } else {
        RecoveryPlanState::Ready
    };
    plan.plan_fingerprint = fingerprint(
        RECOVERY_PLAN_V2_SCHEMA_ID,
        &serde_json::json!({
            "recovery_plan_id": plan.recovery_plan_id,
            "project_id": plan.project_id,
            "failure_record_ref": plan.failure_record_ref,
            "recovery_kind": plan.recovery_kind,
            "exact_subject_fingerprint": plan.exact_subject_fingerprint,
            "steps": plan.steps,
            "owner": plan.owner,
            "state": plan.state,
            "blockers": plan.blockers,
        }),
    )?;
    Ok(plan)
}

pub fn scan_dependency_snapshot(
    project_root: &Path,
    project_id: ProjectId,
    snapshot_id: String,
    subject_revision: String,
) -> Result<DependencySnapshot, DevelopmentError> {
    if !token(&snapshot_id, 192) || subject_revision.trim().is_empty() {
        return Err(DevelopmentError::Invalid);
    }
    let root = project_root
        .canonicalize()
        .map_err(|_| DevelopmentError::Adapter)?;
    if root.join("Cargo.toml").is_file() {
        scan_cargo_dependencies(&root, project_id, snapshot_id, subject_revision)
    } else if root.join("package.json").is_file() {
        scan_node_dependencies(&root, project_id, snapshot_id, subject_revision)
    } else if root.join("pyproject.toml").is_file() {
        scan_python_dependencies(&root, project_id, snapshot_id, subject_revision)
    } else {
        Err(DevelopmentError::Unverified)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalDataSnapshotInput {
    pub source: ExternalDataSourceDescriptor,
    pub retrieved_at: String,
    pub valid_until: String,
    pub evaluation_time: String,
    pub source_artifact_ref: String,
    pub source_sha256: Sha256Hash,
    #[serde(default)]
    pub observations: Vec<ExternalDataObservation>,
    pub available: bool,
}

pub fn build_external_data_snapshot(
    snapshot_id: String,
    input: ExternalDataSnapshotInput,
) -> Result<ExternalDataSnapshot, DevelopmentError> {
    let ExternalDataSnapshotInput {
        source,
        retrieved_at,
        valid_until,
        evaluation_time,
        source_artifact_ref,
        source_sha256,
        mut observations,
        available,
    } = input;
    if !token(&snapshot_id, 192)
        || !token(&source.source_id, 160)
        || source.source_kind.trim().is_empty()
        || source.provider.trim().is_empty()
        || source.retrieval_mode.trim().is_empty()
        || source.integrity_policy.trim().is_empty()
        || source.maximum_age_seconds == 0
        || !timestamp_shape(&retrieved_at)
        || !timestamp_shape(&valid_until)
        || !timestamp_shape(&evaluation_time)
        || source_artifact_ref.trim().is_empty()
    {
        return Err(DevelopmentError::Invalid);
    }
    observations.sort_by(|left, right| left.subject.cmp(&right.subject));
    if observations
        .windows(2)
        .any(|pair| pair[0].subject == pair[1].subject)
    {
        return Err(DevelopmentError::Conflict);
    }
    let freshness = if !available {
        ExternalFreshness::Unavailable
    } else if evaluation_time > valid_until {
        ExternalFreshness::Expired
    } else {
        ExternalFreshness::Current
    };
    let completeness = if freshness == ExternalFreshness::Current {
        CoverageState::Complete
    } else {
        CoverageState::Partial
    };
    let limitations = match freshness {
        ExternalFreshness::Current => Vec::new(),
        ExternalFreshness::Expired => vec!["external data passed valid_until".to_owned()],
        ExternalFreshness::Unavailable => vec!["external data source unavailable".to_owned()],
        _ => vec!["external data freshness is not current".to_owned()],
    };
    let mut snapshot = ExternalDataSnapshot {
        schema_id: EXTERNAL_DATA_SNAPSHOT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        snapshot_id,
        source,
        retrieved_at,
        valid_until,
        evaluation_time,
        source_artifact_ref,
        source_sha256,
        observations,
        freshness,
        completeness,
        limitations,
        content_fingerprint: placeholder(),
    };
    snapshot.content_fingerprint = fingerprint(
        EXTERNAL_DATA_SNAPSHOT_SCHEMA_ID,
        &serde_json::json!({
            "snapshot_id": snapshot.snapshot_id,
            "source": snapshot.source,
            "retrieved_at": snapshot.retrieved_at,
            "valid_until": snapshot.valid_until,
            "evaluation_time": snapshot.evaluation_time,
            "source_artifact_ref": snapshot.source_artifact_ref,
            "source_sha256": snapshot.source_sha256,
            "observations": snapshot.observations,
            "freshness": snapshot.freshness,
            "completeness": snapshot.completeness,
            "limitations": snapshot.limitations,
        }),
    )?;
    Ok(snapshot)
}

pub fn build_supply_chain_snapshot(
    snapshot_id: String,
    dependency: &DependencySnapshot,
    external: &[ExternalDataSnapshot],
    mut observations: Vec<SupplyChainObservation>,
) -> Result<SupplyChainSnapshot, DevelopmentError> {
    if !token(&snapshot_id, 192)
        || dependency.schema_id != DEPENDENCY_SNAPSHOT_SCHEMA_ID
        || external
            .iter()
            .any(|item| item.schema_id != EXTERNAL_DATA_SNAPSHOT_SCHEMA_ID)
    {
        return Err(DevelopmentError::Invalid);
    }
    observations.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    if observations
        .windows(2)
        .any(|pair| pair[0].observation_id == pair[1].observation_id)
    {
        return Err(DevelopmentError::Conflict);
    }
    let freshness = external
        .iter()
        .map(|item| item.freshness)
        .fold(ExternalFreshness::Current, worst_freshness);
    let completeness = if dependency.completeness == CoverageState::Complete
        && external
            .iter()
            .all(|item| item.completeness == CoverageState::Complete)
    {
        CoverageState::Complete
    } else {
        CoverageState::Partial
    };
    let mut limitations = dependency.limitations.clone();
    limitations.extend(
        external
            .iter()
            .flat_map(|item| item.limitations.iter().cloned()),
    );
    limitations.sort();
    limitations.dedup();
    let mut snapshot = SupplyChainSnapshot {
        schema_id: SUPPLY_CHAIN_SNAPSHOT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        snapshot_id,
        project_id: dependency.project_id.clone(),
        subject_revision: dependency.subject_revision.clone(),
        dependency_snapshot_ref: dependency.snapshot_id.clone(),
        dependency_snapshot_fingerprint: dependency.content_fingerprint.clone(),
        external_data_snapshot_refs: external
            .iter()
            .map(|item| item.snapshot_id.clone())
            .collect(),
        observations,
        freshness,
        completeness,
        limitations,
        content_fingerprint: placeholder(),
    };
    snapshot.content_fingerprint = fingerprint(
        SUPPLY_CHAIN_SNAPSHOT_SCHEMA_ID,
        &serde_json::json!({
            "snapshot_id": snapshot.snapshot_id,
            "project_id": snapshot.project_id,
            "subject_revision": snapshot.subject_revision,
            "dependency_snapshot_ref": snapshot.dependency_snapshot_ref,
            "dependency_snapshot_fingerprint": snapshot.dependency_snapshot_fingerprint,
            "external_data_snapshot_refs": snapshot.external_data_snapshot_refs,
            "observations": snapshot.observations,
            "freshness": snapshot.freshness,
            "completeness": snapshot.completeness,
            "limitations": snapshot.limitations,
        }),
    )?;
    Ok(snapshot)
}

pub fn build_dependency_update_plan(
    plan_id: String,
    snapshot: &DependencySnapshot,
    mut candidate: UpdateCandidate,
    mut expected_manifest_paths: Vec<String>,
    mut expected_lockfile_paths: Vec<String>,
) -> Result<DependencyUpdatePlan, DevelopmentError> {
    if !token(&plan_id, 192)
        || !token(&candidate.candidate_id, 192)
        || !snapshot
            .dependencies
            .iter()
            .any(|dependency| dependency.dependency_id == candidate.dependency_id)
        || candidate.proposed_constraint.trim().is_empty()
        || candidate.reason.trim().is_empty()
        || candidate.package_manager_adapter_ref.trim().is_empty()
        || candidate.source_change
            && !candidate
                .risk_markers
                .iter()
                .any(|marker| marker == "source_change")
    {
        return Err(DevelopmentError::Invalid);
    }
    candidate.affected_project_ids.sort();
    candidate.affected_project_ids.dedup();
    candidate.affected_surfaces.sort();
    candidate.affected_surfaces.dedup();
    candidate.risk_markers.sort();
    candidate.risk_markers.dedup();
    expected_manifest_paths.sort();
    expected_manifest_paths.dedup();
    expected_lockfile_paths.sort();
    expected_lockfile_paths.dedup();
    if expected_manifest_paths.is_empty()
        || expected_manifest_paths
            .iter()
            .any(|path| !safe_relative_path(path))
        || expected_lockfile_paths
            .iter()
            .any(|path| !safe_relative_path(path))
    {
        return Err(DevelopmentError::Invalid);
    }
    let (status, blockers) = match candidate.source_freshness {
        ExternalFreshness::Current => (
            DependencyUpdateStatus::AwaitingPatchPreparationApproval,
            Vec::new(),
        ),
        ExternalFreshness::Stale
        | ExternalFreshness::Expired
        | ExternalFreshness::Unknown
        | ExternalFreshness::Unavailable => (
            DependencyUpdateStatus::AwaitingRefreshApproval,
            vec!["current external source evidence is required".to_owned()],
        ),
    };
    let mut plan = DependencyUpdatePlan {
        schema_id: DEPENDENCY_UPDATE_PLAN_SCHEMA_ID.to_owned(),
        schema_version: 1,
        plan_id,
        project_id: snapshot.project_id.clone(),
        dependency_snapshot_ref: snapshot.snapshot_id.clone(),
        candidate,
        expected_manifest_paths,
        expected_lockfile_paths,
        patch_set_ref: None,
        previous_lockfile_artifact_ref: None,
        rollback_recipe_ref: None,
        status,
        blockers,
        plan_fingerprint: placeholder(),
    };
    plan.plan_fingerprint = fingerprint(
        DEPENDENCY_UPDATE_PLAN_SCHEMA_ID,
        &serde_json::json!({
            "plan_id": plan.plan_id,
            "project_id": plan.project_id,
            "dependency_snapshot_ref": plan.dependency_snapshot_ref,
            "candidate": plan.candidate,
            "expected_manifest_paths": plan.expected_manifest_paths,
            "expected_lockfile_paths": plan.expected_lockfile_paths,
            "patch_set_ref": plan.patch_set_ref,
            "previous_lockfile_artifact_ref": plan.previous_lockfile_artifact_ref,
            "rollback_recipe_ref": plan.rollback_recipe_ref,
            "status": plan.status,
            "blockers": plan.blockers,
        }),
    )?;
    Ok(plan)
}

pub fn build_maintenance_radar_snapshot(
    snapshot_id: String,
    evaluation_time: String,
    valid_until: Option<String>,
    mut items: Vec<MaintenanceRadarItem>,
) -> Result<MaintenanceRadarSnapshot, DevelopmentError> {
    if !token(&snapshot_id, 192)
        || !timestamp_shape(&evaluation_time)
        || valid_until
            .as_ref()
            .is_some_and(|value| !timestamp_shape(value))
        || items.is_empty()
        || items.iter().any(|item| {
            !token(&item.item_id, 192)
                || item.subject.trim().is_empty()
                || item.priority.blocking_rank > 3
                || item.priority.risk_rank > 4
                || item.priority.freshness_rank > 4
                || item.priority.regression_rank > 3
                || item.priority.evidence_rank > 3
        })
    {
        return Err(DevelopmentError::Invalid);
    }
    items.sort_by(|left, right| radar_key(left).cmp(&radar_key(right)));
    if items
        .windows(2)
        .any(|pair| pair[0].item_id == pair[1].item_id)
    {
        return Err(DevelopmentError::Conflict);
    }
    let completeness = if items
        .iter()
        .all(|item| item.completeness == CoverageState::Complete)
    {
        CoverageState::Complete
    } else {
        CoverageState::Partial
    };
    let mut limitations = items
        .iter()
        .filter(|item| item.freshness != ExternalFreshness::Current)
        .map(|item| format!("non-current input: {}", item.item_id))
        .collect::<Vec<_>>();
    if valid_until
        .as_ref()
        .is_some_and(|valid_until| evaluation_time > *valid_until)
    {
        limitations.push("radar snapshot passed valid_until".to_owned());
    }
    limitations.sort();
    limitations.dedup();
    let mut snapshot = MaintenanceRadarSnapshot {
        schema_id: MAINTENANCE_RADAR_SNAPSHOT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        snapshot_id,
        evaluation_time,
        items,
        valid_until,
        completeness,
        limitations,
        content_fingerprint: placeholder(),
    };
    snapshot.content_fingerprint = fingerprint(
        MAINTENANCE_RADAR_SNAPSHOT_SCHEMA_ID,
        &serde_json::json!({
            "snapshot_id": snapshot.snapshot_id,
            "evaluation_time": snapshot.evaluation_time,
            "items": snapshot.items,
            "valid_until": snapshot.valid_until,
            "completeness": snapshot.completeness,
            "limitations": snapshot.limitations,
        }),
    )?;
    Ok(snapshot)
}

fn scan_cargo_dependencies(
    root: &Path,
    project_id: ProjectId,
    snapshot_id: String,
    subject_revision: String,
) -> Result<DependencySnapshot, DevelopmentError> {
    let manifest_bytes = read_bounded(root, "Cargo.toml", 8 * 1024 * 1024)?;
    let manifest: toml::Value = std::str::from_utf8(&manifest_bytes)
        .map_err(|_| DevelopmentError::Invalid)
        .and_then(|text| toml::from_str(text).map_err(|_| DevelopmentError::Invalid))?;
    let direct = cargo_direct_dependencies(&manifest);
    let lockfile_path = root.join("Cargo.lock");
    let (lockfile_bytes, packages, completeness, limitations) = if lockfile_path.is_file() {
        let bytes = read_bounded(root, "Cargo.lock", 32 * 1024 * 1024)?;
        let lock: toml::Value = std::str::from_utf8(&bytes)
            .map_err(|_| DevelopmentError::Invalid)
            .and_then(|text| toml::from_str(text).map_err(|_| DevelopmentError::Invalid))?;
        let packages = lock
            .get("package")
            .and_then(toml::Value::as_array)
            .cloned()
            .unwrap_or_default();
        (Some(bytes), packages, CoverageState::Complete, Vec::new())
    } else {
        (
            None,
            Vec::new(),
            CoverageState::Partial,
            vec!["Cargo.lock is missing; transitive graph is unverified".to_owned()],
        )
    };
    let mut dependencies = Vec::new();
    if packages.is_empty() {
        for (name, requested) in direct {
            dependencies.push(dependency_record(
                &project_id,
                "rust",
                &name,
                Some(requested),
                None,
                "registry:unknown".to_owned(),
                None,
                true,
            ));
        }
    } else {
        for package in packages {
            let Some(table) = package.as_table() else {
                continue;
            };
            let Some(name) = table.get("name").and_then(toml::Value::as_str) else {
                continue;
            };
            let version = table
                .get("version")
                .and_then(toml::Value::as_str)
                .map(str::to_owned);
            let source = table
                .get("source")
                .and_then(toml::Value::as_str)
                .unwrap_or("workspace")
                .to_owned();
            let integrity = table
                .get("checksum")
                .and_then(toml::Value::as_str)
                .map(str::to_owned);
            dependencies.push(dependency_record(
                &project_id,
                "rust",
                name,
                direct.get(name).cloned(),
                version,
                source,
                integrity,
                direct.contains_key(name),
            ));
        }
    }
    seal_dependency_snapshot(DependencySnapshot {
        schema_id: DEPENDENCY_SNAPSHOT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        snapshot_id,
        project_id,
        subject_revision,
        package_manager_id: "cargo".to_owned(),
        package_manager_version: None,
        resolver_mode: "locked-file-read-only".to_owned(),
        manifest_path: "Cargo.toml".to_owned(),
        manifest_sha256: Sha256Hash::digest(&manifest_bytes),
        lockfile_path: lockfile_bytes.as_ref().map(|_| "Cargo.lock".to_owned()),
        lockfile_sha256: lockfile_bytes
            .as_ref()
            .map(|bytes| Sha256Hash::digest(bytes)),
        dependencies,
        completeness,
        limitations,
        content_fingerprint: placeholder(),
    })
}

fn scan_node_dependencies(
    root: &Path,
    project_id: ProjectId,
    snapshot_id: String,
    subject_revision: String,
) -> Result<DependencySnapshot, DevelopmentError> {
    let manifest_bytes = read_bounded(root, "package.json", 8 * 1024 * 1024)?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest_bytes).map_err(|_| DevelopmentError::Invalid)?;
    let mut direct = BTreeMap::new();
    for table in [
        "dependencies",
        "devDependencies",
        "optionalDependencies",
        "peerDependencies",
    ] {
        if let Some(values) = manifest.get(table).and_then(serde_json::Value::as_object) {
            for (name, value) in values {
                if let Some(version) = value.as_str() {
                    direct.insert(name.clone(), version.to_owned());
                }
            }
        }
    }
    let lock_name = ["package-lock.json", "npm-shrinkwrap.json"]
        .into_iter()
        .find(|path| root.join(path).is_file());
    let mut dependencies = Vec::new();
    let (lockfile_bytes, completeness, limitations) = if let Some(lock_name) = lock_name {
        let bytes = read_bounded(root, lock_name, 64 * 1024 * 1024)?;
        let lock: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| DevelopmentError::Invalid)?;
        if let Some(packages) = lock.get("packages").and_then(serde_json::Value::as_object) {
            for (path, value) in packages {
                if path.is_empty() {
                    continue;
                }
                let name = value
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| path.rsplit("node_modules/").next())
                    .unwrap_or(path);
                dependencies.push(dependency_record(
                    &project_id,
                    "node",
                    name,
                    direct.get(name).cloned(),
                    value
                        .get("version")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned),
                    value
                        .get("resolved")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("registry:unknown")
                        .to_owned(),
                    value
                        .get("integrity")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned),
                    direct.contains_key(name),
                ));
            }
        }
        (Some(bytes), CoverageState::Complete, Vec::new())
    } else {
        for (name, requested) in direct {
            dependencies.push(dependency_record(
                &project_id,
                "node",
                &name,
                Some(requested),
                None,
                "registry:unknown".to_owned(),
                None,
                true,
            ));
        }
        (
            None,
            CoverageState::Partial,
            vec!["supported Node lockfile is missing; transitive graph is unverified".to_owned()],
        )
    };
    seal_dependency_snapshot(DependencySnapshot {
        schema_id: DEPENDENCY_SNAPSHOT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        snapshot_id,
        project_id,
        subject_revision,
        package_manager_id: "npm".to_owned(),
        package_manager_version: None,
        resolver_mode: "lockfile-read-only".to_owned(),
        manifest_path: "package.json".to_owned(),
        manifest_sha256: Sha256Hash::digest(&manifest_bytes),
        lockfile_path: lock_name.map(str::to_owned),
        lockfile_sha256: lockfile_bytes
            .as_ref()
            .map(|bytes| Sha256Hash::digest(bytes)),
        dependencies,
        completeness,
        limitations,
        content_fingerprint: placeholder(),
    })
}

fn scan_python_dependencies(
    root: &Path,
    project_id: ProjectId,
    snapshot_id: String,
    subject_revision: String,
) -> Result<DependencySnapshot, DevelopmentError> {
    let manifest_bytes = read_bounded(root, "pyproject.toml", 8 * 1024 * 1024)?;
    let manifest: toml::Value = std::str::from_utf8(&manifest_bytes)
        .map_err(|_| DevelopmentError::Invalid)
        .and_then(|text| toml::from_str(text).map_err(|_| DevelopmentError::Invalid))?;
    let mut dependencies = Vec::new();
    if let Some(values) = manifest
        .get("project")
        .and_then(|value| value.get("dependencies"))
        .and_then(toml::Value::as_array)
    {
        for value in values.iter().filter_map(toml::Value::as_str) {
            let name = value
                .split(|character: char| {
                    !character.is_ascii_alphanumeric() && character != '-' && character != '_'
                })
                .next()
                .unwrap_or(value);
            dependencies.push(dependency_record(
                &project_id,
                "python",
                name,
                Some(value.to_owned()),
                None,
                "index:unknown".to_owned(),
                None,
                true,
            ));
        }
    }
    seal_dependency_snapshot(DependencySnapshot {
        schema_id: DEPENDENCY_SNAPSHOT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        snapshot_id,
        project_id,
        subject_revision,
        package_manager_id: "python-project".to_owned(),
        package_manager_version: None,
        resolver_mode: "manifest-only-read-only".to_owned(),
        manifest_path: "pyproject.toml".to_owned(),
        manifest_sha256: Sha256Hash::digest(&manifest_bytes),
        lockfile_path: None,
        lockfile_sha256: None,
        dependencies,
        completeness: CoverageState::Partial,
        limitations: vec!["no supported Python lockfile adapter was selected".to_owned()],
        content_fingerprint: placeholder(),
    })
}

fn seal_dependency_snapshot(
    mut snapshot: DependencySnapshot,
) -> Result<DependencySnapshot, DevelopmentError> {
    snapshot.dependencies.sort_by(|left, right| {
        (&left.package_identity, &left.resolved_version, &left.source).cmp(&(
            &right.package_identity,
            &right.resolved_version,
            &right.source,
        ))
    });
    let mut seen = BTreeSet::new();
    if snapshot.dependencies.iter().any(|dependency| {
        dependency.package_identity.trim().is_empty()
            || !seen.insert(dependency.dependency_id.as_str())
    }) {
        return Err(DevelopmentError::Conflict);
    }
    snapshot.content_fingerprint = fingerprint(
        DEPENDENCY_SNAPSHOT_SCHEMA_ID,
        &serde_json::json!({
            "snapshot_id": snapshot.snapshot_id,
            "project_id": snapshot.project_id,
            "subject_revision": snapshot.subject_revision,
            "package_manager_id": snapshot.package_manager_id,
            "package_manager_version": snapshot.package_manager_version,
            "resolver_mode": snapshot.resolver_mode,
            "manifest_path": snapshot.manifest_path,
            "manifest_sha256": snapshot.manifest_sha256,
            "lockfile_path": snapshot.lockfile_path,
            "lockfile_sha256": snapshot.lockfile_sha256,
            "dependencies": snapshot.dependencies,
            "completeness": snapshot.completeness,
            "limitations": snapshot.limitations,
        }),
    )?;
    Ok(snapshot)
}

fn cargo_direct_dependencies(manifest: &toml::Value) -> BTreeMap<String, String> {
    let mut output = BTreeMap::new();
    for key in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(table) = manifest.get(key).and_then(toml::Value::as_table) {
            for (name, value) in table {
                let requested = value
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| {
                        value
                            .as_table()
                            .and_then(|table| table.get("version"))
                            .and_then(toml::Value::as_str)
                            .map(str::to_owned)
                    })
                    .unwrap_or_else(|| "source-bound".to_owned());
                output.insert(name.clone(), requested);
            }
        }
    }
    output
}

#[allow(clippy::too_many_arguments)]
fn dependency_record(
    project_id: &ProjectId,
    ecosystem: &str,
    name: &str,
    requested_version: Option<String>,
    resolved_version: Option<String>,
    source: String,
    integrity: Option<String>,
    direct: bool,
) -> DependencyRecord {
    let identity_seed = serde_json::json!({
        "ecosystem": ecosystem,
        "name": name,
        "version": resolved_version,
        "source": source,
    });
    let identity = star_contracts::canonical_sha256(&identity_seed)
        .unwrap_or_else(|_| Sha256Hash::digest(name.as_bytes()));
    DependencyRecord {
        dependency_id: format!("dep_{}", identity.as_str().trim_start_matches("sha256:")),
        purpose: if direct { "declared" } else { "transitive" }.to_owned(),
        ecosystem: ecosystem.to_owned(),
        package_identity: name.to_owned(),
        requested_version,
        resolved_version,
        source,
        integrity,
        license_refs: Vec::new(),
        advisory_refs: Vec::new(),
        direct,
        affected_project_ids: vec![project_id.clone()],
    }
}

fn read_bounded(root: &Path, path: &str, max: u64) -> Result<Vec<u8>, DevelopmentError> {
    let path = confined_path(root, path)?;
    let metadata = path.metadata().map_err(|_| DevelopmentError::Adapter)?;
    if !metadata.is_file() || metadata.len() > max {
        return Err(DevelopmentError::Blocked);
    }
    std::fs::read(path).map_err(|_| DevelopmentError::Adapter)
}

fn confined_path(root: &Path, logical: &str) -> Result<PathBuf, DevelopmentError> {
    if !safe_relative_path(logical) {
        return Err(DevelopmentError::Invalid);
    }
    let path = root
        .join(logical)
        .canonicalize()
        .map_err(|_| DevelopmentError::Adapter)?;
    if !path.starts_with(root) {
        return Err(DevelopmentError::Blocked);
    }
    Ok(path)
}

fn recovery_graph_has_cycle(steps: &[star_contracts::maintenance_v2::RecoveryStepV2]) -> bool {
    fn visit<'a>(
        id: &'a str,
        by_id: &BTreeMap<&'a str, &'a star_contracts::maintenance_v2::RecoveryStepV2>,
        visiting: &mut BTreeSet<&'a str>,
        visited: &mut BTreeSet<&'a str>,
    ) -> bool {
        if visited.contains(id) {
            return false;
        }
        if !visiting.insert(id) {
            return true;
        }
        let cyclic = by_id.get(id).is_some_and(|step| {
            step.prerequisite_step_ids
                .iter()
                .any(|required| visit(required, by_id, visiting, visited))
        });
        visiting.remove(id);
        visited.insert(id);
        cyclic
    }
    let by_id = steps
        .iter()
        .map(|step| (step.step_id.as_str(), step))
        .collect::<BTreeMap<_, _>>();
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    steps
        .iter()
        .any(|step| visit(&step.step_id, &by_id, &mut visiting, &mut visited))
}

fn normalize_failure_message(message: &str) -> String {
    let mut output = Vec::new();
    for raw in message.split_whitespace() {
        let lower = raw.to_ascii_lowercase();
        let looks_like_path = raw.contains('\\')
            || raw.starts_with('/')
            || raw.get(1..2) == Some(":")
            || lower.contains("appdata")
            || lower.contains("users/")
            || lower.contains("users\\");
        if looks_like_path {
            output.push("<path>".to_owned());
            continue;
        }
        let mut normalized = String::new();
        let mut digits = false;
        for character in raw.chars() {
            if character.is_ascii_digit() {
                if !digits {
                    normalized.push_str("<n>");
                    digits = true;
                }
            } else {
                normalized.push(character.to_ascii_lowercase());
                digits = false;
            }
        }
        output.push(normalized);
    }
    output.join(" ")
}

fn redact_argument_shape(argument: &str) -> String {
    if argument.contains('=') {
        let key = argument.split('=').next().unwrap_or("arg");
        format!("{key}=<value>")
    } else if argument.contains('\\') || argument.starts_with('/') {
        "<path>".to_owned()
    } else {
        argument.to_owned()
    }
}

fn contains_sensitive_shape(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("bearer ")
        || lower.contains("api_key=")
        || lower.contains("apikey=")
        || lower.contains("password=")
        || lower.contains("token=")
}

fn timestamp_shape(value: &str) -> bool {
    value.len() >= 20
        && value.as_bytes().get(4) == Some(&b'-')
        && value.as_bytes().get(7) == Some(&b'-')
        && value.contains('T')
        && (value.ends_with('Z') || value.contains('+'))
}

fn worst_freshness(left: ExternalFreshness, right: ExternalFreshness) -> ExternalFreshness {
    let rank = |state| match state {
        ExternalFreshness::Current => 0,
        ExternalFreshness::Stale => 1,
        ExternalFreshness::Unknown => 2,
        ExternalFreshness::Unavailable => 3,
        ExternalFreshness::Expired => 4,
    };
    if rank(right) > rank(left) {
        right
    } else {
        left
    }
}

fn radar_key(item: &MaintenanceRadarItem) -> (u8, u8, u8, u8, u8, &str, &str) {
    (
        item.priority.blocking_rank,
        item.priority.risk_rank,
        item.priority.freshness_rank,
        item.priority.regression_rank,
        item.priority.evidence_rank,
        item.priority.time_rank.as_str(),
        item.priority.stable_identity.as_str(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::maintenance_v2::{FailureCausalityRole, FailureSubjectBinding};

    fn failure_input(project_id: ProjectId, message: &str) -> FailureRecordInput {
        FailureRecordInput {
            failure_record_id: "failure-one".to_owned(),
            occurrence_id: "occurrence-one".to_owned(),
            diagnostic_refs: vec!["diagnostic:one".to_owned()],
            finding_refs: Vec::new(),
            subject_binding: FailureSubjectBinding {
                project_id,
                checkout_ref: "checkout:one".to_owned(),
                workspace_snapshot_ref: "workspace:one".to_owned(),
                project_revision_ref: "revision:one".to_owned(),
                change_set_ref: None,
                validation_run_ref: "validation:one".to_owned(),
            },
            failure_kind: FailureKind::Test,
            producer_code: "RUST_TEST_FAILED".to_owned(),
            raw_message: message.to_owned(),
            logical_owner: "crate::tests::case".to_owned(),
            signature: "case".to_owned(),
            causality_role: FailureCausalityRole::Independent,
            root_candidate_refs: Vec::new(),
            cascade_parent_refs: Vec::new(),
            invocation: FailureInvocation {
                command_descriptor: "cargo.test.v1".to_owned(),
                executable_identity: "cargo-compatible".to_owned(),
                structured_args: vec!["test".to_owned()],
                logical_cwd: "project-root".to_owned(),
                timeout_ms: 10_000,
            },
            environment_compatibility_class: "windows-x64".to_owned(),
            environment_fingerprint: Sha256Hash::digest(b"environment"),
            input_refs: Vec::new(),
            input_fingerprint: Sha256Hash::digest(b"input"),
            seed: None,
            manifest_fingerprint: None,
            stdout_ref: None,
            stderr_ref: None,
            artifact_refs: Vec::new(),
            observed_at: "2026-07-23T00:00:00Z".to_owned(),
            attempt_id: "attempt-one".to_owned(),
            verification_state: VerificationState::Verified,
        }
    }

    #[test]
    fn failure_family_redacts_path_pid_and_timestamp_but_occurrence_stays_exact() {
        let project_id = ProjectId::new();
        let first = build_failure_record(failure_input(
            project_id.clone(),
            "C:\\tmp\\a.rs pid 123 failed at 2026",
        ))
        .unwrap();
        let mut second_input = failure_input(project_id, "/tmp/b.rs pid 999 failed at 2030");
        second_input.failure_record_id = "failure-two".to_owned();
        second_input.occurrence_id = "occurrence-two".to_owned();
        second_input.environment_fingerprint = Sha256Hash::digest(b"different-environment");
        let second = build_failure_record(second_input).unwrap();
        assert_eq!(first.family_fingerprint, second.family_fingerprint);
        assert_ne!(first.occurrence_fingerprint, second.occurrence_fingerprint);
        assert!(!first.primary_symptom.message_template.contains("tmp"));
    }

    #[test]
    fn cargo_dependency_scan_is_source_only_and_marks_missing_lock_partial() {
        let root = std::env::temp_dir().join(format!(
            "star-dependency-scan-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            b"[package]\nname='sample'\nversion='0.1.0'\n[dependencies]\nserde='1'\n",
        )
        .unwrap();
        let snapshot = scan_dependency_snapshot(
            &root,
            ProjectId::new(),
            "dependency-one".to_owned(),
            "revision:one".to_owned(),
        )
        .unwrap();
        assert_eq!(snapshot.completeness, CoverageState::Partial);
        assert_eq!(snapshot.dependencies.len(), 1);
        assert_eq!(snapshot.dependencies[0].package_identity, "serde");
    }
}
