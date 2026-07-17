//! Deterministic tracked-path impact planning and cache reuse policy.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};
use star_contracts::{
    Sha256Hash, canonical_sha256,
    evidence::{
        AffectedUnit, AffectedUnitKind, Completeness, Diagnostic, DiagnosticRef,
        DiagnosticSeverity, DiagnosticStatus, EVIDENCE_CONTRACT_SCHEMA_VERSION, EVIDENCE_FLOW,
        EvidenceBundle, EvidenceBundleRef, GateDecision, GateDecisionRef,
        IndependentReviewRequirement, IndependentReviewTrigger, PlannedCheck,
        PlannedCheckDisposition, ProfileReasonCode, ReverseConsumer,
        VALIDATION_PLAN_SCHEMA_VERSION, VALIDATION_POLICY_SCHEMA_VERSION, ValidationCacheKeyInputs,
        ValidationCapabilityLevel, ValidationChangeClass, ValidationChangedFile, ValidationCommand,
        ValidationEscalation, ValidationInputFingerprintComponents, ValidationOutcome,
        ValidationPlan, ValidationPlanInvariantError, ValidationPlanReadiness,
        ValidationPlanSchemaId, ValidationProfile, ValidationProfileSelection, ValidationRun,
        ValidationRunRef, ValidationUncertainty, ValidationUncertaintyCode,
        resolve_validation_profile,
    },
    ids::ValidationPlanId,
};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitDependency {
    pub provider_unit_id: String,
    pub consumer_unit_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationCheckDefinition {
    pub profile: ValidationProfile,
    pub check_id: String,
    pub unit_id: String,
    pub command: ValidationCommand,
    pub selection_reason: String,
}

#[derive(Clone, Debug)]
pub struct ValidationCacheCandidate {
    pub check_id: String,
    pub cache_key: Sha256Hash,
    pub validation_run: ValidationRun,
    pub validation_run_ref: ValidationRunRef,
    pub stability: CacheValidationStability,
    pub suppression_applied: bool,
    pub artifacts_available: bool,
    pub policy_schema_version: u32,
    pub evidence_schema_version: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheValidationStability {
    Stable,
    Flaky,
    NotEvaluated,
}

#[derive(Clone, Debug)]
pub struct ValidationPlanningInput {
    pub project_key: String,
    pub revision: String,
    pub requested_profile: Option<ValidationProfile>,
    pub requested_unit: Option<String>,
    pub requested_unit_required_profile: Option<ValidationProfile>,
    pub workspace_unit_id: String,
    pub changed_files: Vec<ValidationChangedFile>,
    pub dependencies: Vec<UnitDependency>,
    pub checks: Vec<ValidationCheckDefinition>,
    pub cache_candidates: Vec<ValidationCacheCandidate>,
    pub fingerprints: ValidationInputFingerprintComponents,
    pub fingerprints_complete: bool,
    pub impact_complete: bool,
    pub repeated_failures: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheMissReason {
    NoCandidate,
    KeyMismatch,
    OutcomeNotPass,
    Incomplete,
    Unstable,
    ExecutionUnverified,
    EvidenceReferenceMismatch,
    Suppressed,
    ArtifactUnavailable,
    PolicySchemaChanged,
    EvidenceSchemaChanged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CacheReuseDecision {
    Reuse(ValidationRunRef),
    Execute(CacheMissReason),
}

#[derive(Debug, Error)]
pub enum ValidationPlanningError {
    #[error("validation planning input is invalid")]
    InvalidInput,
    #[error("validation planning fingerprint failed")]
    Fingerprint,
    #[error("validation plan contract failed: {0}")]
    Contract(#[from] ValidationPlanInvariantError),
}

pub fn evaluate_cache_reuse(
    expected_key: &Sha256Hash,
    candidate: Option<&ValidationCacheCandidate>,
) -> CacheReuseDecision {
    let Some(candidate) = candidate else {
        return CacheReuseDecision::Execute(CacheMissReason::NoCandidate);
    };
    if candidate.cache_key != *expected_key {
        return CacheReuseDecision::Execute(CacheMissReason::KeyMismatch);
    }
    if candidate.validation_run.cache.as_ref().is_none_or(|cache| {
        cache.cache_key != expected_key.as_str()
            || cache.hit != cache.source_validation_run_ref.is_some()
    }) {
        return CacheReuseDecision::Execute(CacheMissReason::KeyMismatch);
    }
    if candidate.validation_run.outcome != ValidationOutcome::Pass {
        return CacheReuseDecision::Execute(CacheMissReason::OutcomeNotPass);
    }
    if candidate.validation_run.completeness != Completeness::Complete {
        return CacheReuseDecision::Execute(CacheMissReason::Incomplete);
    }
    if candidate.stability != CacheValidationStability::Stable {
        return CacheReuseDecision::Execute(CacheMissReason::Unstable);
    }
    if !candidate.validation_run.satisfies_required_check() {
        return CacheReuseDecision::Execute(CacheMissReason::ExecutionUnverified);
    }
    if candidate.validation_run.validate().is_err()
        || candidate.validation_run.check_ref.catalog_id != candidate.check_id
        || !validation_run_reference_matches(
            &candidate.validation_run,
            &candidate.validation_run_ref,
        )
    {
        return CacheReuseDecision::Execute(CacheMissReason::EvidenceReferenceMismatch);
    }
    if candidate.suppression_applied {
        return CacheReuseDecision::Execute(CacheMissReason::Suppressed);
    }
    if !candidate.artifacts_available {
        return CacheReuseDecision::Execute(CacheMissReason::ArtifactUnavailable);
    }
    if candidate.policy_schema_version != VALIDATION_POLICY_SCHEMA_VERSION {
        return CacheReuseDecision::Execute(CacheMissReason::PolicySchemaChanged);
    }
    if candidate.evidence_schema_version != EVIDENCE_CONTRACT_SCHEMA_VERSION {
        return CacheReuseDecision::Execute(CacheMissReason::EvidenceSchemaChanged);
    }
    CacheReuseDecision::Reuse(candidate.validation_run_ref.clone())
}

fn validation_run_reference_matches(run: &ValidationRun, reference: &ValidationRunRef) -> bool {
    reference.validation_run_id == run.validation_run_id
        && reference.revision == run.revision
        && serde_json::to_value(run)
            .ok()
            .and_then(|value| canonical_sha256(&value).ok())
            .is_some_and(|sha256| sha256 == reference.sha256)
}

fn diagnostic_reference_matches(diagnostic: &Diagnostic, reference: &DiagnosticRef) -> bool {
    reference.diagnostic_id == diagnostic.diagnostic_id
        && reference.sequence == diagnostic.sequence
        && serde_json::to_value(diagnostic)
            .ok()
            .and_then(|value| canonical_sha256(&value).ok())
            .is_some_and(|sha256| sha256 == reference.sha256)
}

pub fn build_validation_plan(
    mut input: ValidationPlanningInput,
) -> Result<ValidationPlan, ValidationPlanningError> {
    if input.project_key.trim().is_empty()
        || input.revision.trim().is_empty()
        || input.workspace_unit_id.trim().is_empty()
        || input.fingerprints.revision != input.revision
    {
        return Err(ValidationPlanningError::InvalidInput);
    }
    normalize_changed_files(&mut input.changed_files)?;
    let mut required = if input.changed_files.is_empty() {
        input
            .requested_unit_required_profile
            .unwrap_or(ValidationProfile::Target)
    } else {
        ValidationProfile::Quick
    };
    let mut reasons = BTreeSet::new();
    let mut uncertainties = BTreeMap::new();
    let mut direct_unit_reasons: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut public_providers = BTreeSet::new();
    let mut review_triggers = BTreeSet::new();

    if input.changed_files.is_empty() {
        reasons.insert(if required == ValidationProfile::Quick {
            ProfileReasonCode::DocumentationOnly
        } else {
            ProfileReasonCode::InternalUnitChange
        });
        direct_unit_reasons
            .entry(
                input
                    .requested_unit
                    .clone()
                    .unwrap_or_else(|| input.workspace_unit_id.clone()),
            )
            .or_default()
            .insert("No change set was supplied; the selected unit profile is used.".to_owned());
    }
    for file in &input.changed_files {
        required = required.max(file.change_class.required_profile());
        match file.change_class {
            ValidationChangeClass::Documentation | ValidationChangeClass::Configuration => {
                reasons.insert(ProfileReasonCode::DocumentationOnly);
            }
            ValidationChangeClass::InternalCode => {
                reasons.insert(ProfileReasonCode::InternalUnitChange);
            }
            ValidationChangeClass::PublicContract => {
                reasons.insert(ProfileReasonCode::PublicContractChange);
                review_triggers.insert(IndependentReviewTrigger::PublicContract);
            }
            ValidationChangeClass::ValidatorPolicy => {
                reasons.insert(ProfileReasonCode::ValidatorOrPolicyChange);
            }
            ValidationChangeClass::Toolchain | ValidationChangeClass::Lockfile => {
                reasons.insert(ProfileReasonCode::ToolchainOrLockfileChange);
            }
            ValidationChangeClass::Security | ValidationChangeClass::DataMigration => {
                reasons.insert(ProfileReasonCode::SecurityOrDataChange);
                review_triggers.insert(if file.change_class == ValidationChangeClass::Security {
                    IndependentReviewTrigger::Security
                } else {
                    IndependentReviewTrigger::DataMigration
                });
            }
            ValidationChangeClass::WorkflowRelease => {
                reasons.insert(ProfileReasonCode::WorkflowOrReleaseChange);
            }
            ValidationChangeClass::Unknown => {
                reasons.insert(ProfileReasonCode::ImpactUncertain);
                uncertainties.insert(
                    ValidationUncertaintyCode::ImpactUnavailable,
                    ValidationUncertainty {
                        code: ValidationUncertaintyCode::ImpactUnavailable,
                        summary: format!("Impact could not be classified for {}.", file.path),
                        escalation: ValidationEscalation::HumanReview,
                    },
                );
            }
        }
        let unit = file
            .direct_unit
            .as_deref()
            .or(input.requested_unit.as_deref())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                required = required.max(ValidationProfile::Full);
                reasons.insert(ProfileReasonCode::MissingUnitMapping);
                uncertainties.insert(
                    ValidationUncertaintyCode::UnitMappingMissing,
                    ValidationUncertainty {
                        code: ValidationUncertaintyCode::UnitMappingMissing,
                        summary: "At least one changed path has no unit mapping.".to_owned(),
                        escalation: ValidationEscalation::ExpandToWorkspace,
                    },
                );
                input.workspace_unit_id.clone()
            });
        direct_unit_reasons
            .entry(unit.clone())
            .or_default()
            .insert(format!("Directly affected by {}.", file.path));
        if file.change_class == ValidationChangeClass::PublicContract {
            public_providers.insert(unit);
        }
    }
    if !input.impact_complete {
        required = required.max(ValidationProfile::Full);
        reasons.insert(ProfileReasonCode::ImpactUncertain);
        uncertainties.insert(
            ValidationUncertaintyCode::ReverseConsumerGraphIncomplete,
            ValidationUncertainty {
                code: ValidationUncertaintyCode::ReverseConsumerGraphIncomplete,
                summary: "The reverse-consumer graph is incomplete.".to_owned(),
                escalation: ValidationEscalation::HumanReview,
            },
        );
    }
    if !input.fingerprints_complete {
        required = required.max(ValidationProfile::Full);
        reasons.insert(ProfileReasonCode::ImpactUncertain);
        uncertainties.insert(
            ValidationUncertaintyCode::FingerprintInputUnavailable,
            ValidationUncertainty {
                code: ValidationUncertaintyCode::FingerprintInputUnavailable,
                summary: "At least one toolchain, lockfile, validator, or configuration fingerprint input is unavailable.".to_owned(),
                escalation: ValidationEscalation::HumanReview,
            },
        );
    }

    let requested = input.requested_profile;
    if let Some(profile) = requested {
        reasons.insert(match profile {
            ValidationProfile::Quick => ProfileReasonCode::ExplicitQuick,
            ValidationProfile::Target => ProfileReasonCode::ExplicitTarget,
            ValidationProfile::Full => ProfileReasonCode::ExplicitFull,
            ValidationProfile::Release => ProfileReasonCode::ExplicitRelease,
        });
    }
    let selected = resolve_validation_profile(required, requested);
    if selected == ValidationProfile::Release {
        review_triggers.insert(IndependentReviewTrigger::Release);
        uncertainties.insert(
            ValidationUncertaintyCode::ReleaseEnvironmentUnavailable,
            ValidationUncertainty {
                code: ValidationUncertaintyCode::ReleaseEnvironmentUnavailable,
                summary: "Release platform, security, recovery, and artifact gates require the release environment.".to_owned(),
                escalation: ValidationEscalation::HumanReview,
            },
        );
    }
    if input.repeated_failures {
        review_triggers.insert(IndependentReviewTrigger::RepeatedFailure);
    }

    let reverse_consumers = reverse_consumers(&public_providers, &input.dependencies);
    for consumer in &reverse_consumers {
        direct_unit_reasons
            .entry(consumer.consumer_unit_id.clone())
            .or_default()
            .insert(format!(
                "Reverse consumer of public contract provider {}.",
                consumer.provider_unit_id
            ));
    }
    if selected >= ValidationProfile::Full {
        direct_unit_reasons
            .entry(input.workspace_unit_id.clone())
            .or_default()
            .insert("FULL validation expands to the owning workspace.".to_owned());
    }

    let direct_units = direct_unit_reasons
        .into_iter()
        .map(|(unit_id, reasons)| AffectedUnit {
            kind: if unit_id == input.workspace_unit_id {
                AffectedUnitKind::Workspace
            } else {
                AffectedUnitKind::Unit
            },
            unit_id,
            reason: reasons.into_iter().collect::<Vec<_>>().join(" "),
        })
        .collect::<Vec<_>>();

    let input_fingerprint = input
        .fingerprints
        .fingerprint()
        .map_err(|_| ValidationPlanningError::Fingerprint)?;
    let candidates: BTreeMap<_, _> = if input.fingerprints_complete {
        input
            .cache_candidates
            .iter()
            .map(|candidate| (candidate.check_id.as_str(), candidate))
            .collect()
    } else {
        BTreeMap::new()
    };
    let mut checks = input
        .checks
        .into_iter()
        .filter(|definition| definition.profile == selected)
        .map(|definition| {
            let command_value = serde_json::to_value(&definition.command)
                .map_err(|_| ValidationPlanningError::Fingerprint)?;
            let command = canonical_sha256(&command_value)
                .map_err(|_| ValidationPlanningError::Fingerprint)?;
            let cache_key_inputs = ValidationCacheKeyInputs {
                inputs: input.fingerprints.clone(),
                command,
            };
            let cache_key = cache_key_inputs
                .fingerprint()
                .map_err(|_| ValidationPlanningError::Fingerprint)?;
            let reuse = evaluate_cache_reuse(
                &cache_key,
                candidates.get(definition.check_id.as_str()).copied(),
            );
            let (disposition, source_validation_run_ref, cache_reason) = match reuse {
                CacheReuseDecision::Reuse(reference) => (
                    PlannedCheckDisposition::Reuse,
                    Some(reference),
                    "Identical complete stable success is reusable.".to_owned(),
                ),
                CacheReuseDecision::Execute(reason) => (
                    PlannedCheckDisposition::Execute,
                    None,
                    format!("Execution required: {reason:?}."),
                ),
            };
            Ok(PlannedCheck {
                check_id: definition.check_id,
                unit_id: definition.unit_id,
                command: definition.command,
                disposition,
                selection_reason: format!("{} {}", definition.selection_reason, cache_reason),
                cache_key_inputs,
                cache_key,
                source_validation_run_ref,
            })
        })
        .collect::<Result<Vec<_>, ValidationPlanningError>>()?;
    checks.sort_by(|left, right| left.check_id.cmp(&right.check_id));
    if checks.is_empty() {
        return Err(ValidationPlanningError::InvalidInput);
    }

    let uncertainties = uncertainties.into_values().collect::<Vec<_>>();
    let readiness = if uncertainties
        .iter()
        .any(|item| item.escalation == ValidationEscalation::HumanReview)
    {
        ValidationPlanReadiness::HumanReview
    } else {
        ValidationPlanReadiness::Ready
    };
    let plan = ValidationPlan {
        schema_id: ValidationPlanSchemaId::ValidationPlan,
        schema_version: VALIDATION_PLAN_SCHEMA_VERSION,
        validation_plan_id: ValidationPlanId::new(),
        capability_level: ValidationCapabilityLevel::TrackedPathPrecursor,
        project_key: input.project_key,
        revision: input.revision,
        changed_files: input.changed_files,
        direct_units,
        reverse_consumers,
        profile: ValidationProfileSelection {
            requested,
            required,
            selected,
            reasons: reasons.into_iter().collect(),
        },
        checks,
        uncertainties,
        independent_review: IndependentReviewRequirement {
            required: !review_triggers.is_empty(),
            triggers: review_triggers.into_iter().collect(),
        },
        readiness,
        evidence_flow: EVIDENCE_FLOW.to_vec(),
        input_fingerprint,
        plan_fingerprint: Sha256Hash::digest(b"unsealed-validation-plan"),
    };
    Ok(plan.seal()?)
}

fn normalize_changed_files(
    changed_files: &mut [ValidationChangedFile],
) -> Result<(), ValidationPlanningError> {
    for file in changed_files.iter_mut() {
        file.path = file.path.replace('\\', "/");
        file.sources.sort();
        file.sources.dedup();
        if file.path.trim().is_empty() || file.sources.is_empty() {
            return Err(ValidationPlanningError::InvalidInput);
        }
    }
    changed_files.sort_by(|left, right| left.path.cmp(&right.path));
    if changed_files
        .windows(2)
        .any(|pair| pair[0].path == pair[1].path)
    {
        return Err(ValidationPlanningError::InvalidInput);
    }
    Ok(())
}

fn reverse_consumers(
    providers: &BTreeSet<String>,
    dependencies: &[UnitDependency],
) -> Vec<ReverseConsumer> {
    let mut reverse: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for dependency in dependencies {
        reverse
            .entry(dependency.provider_unit_id.as_str())
            .or_default()
            .push(dependency.consumer_unit_id.as_str());
    }
    for consumers in reverse.values_mut() {
        consumers.sort_unstable();
        consumers.dedup();
    }
    let mut result = BTreeMap::<(String, String), ReverseConsumer>::new();
    for provider in providers {
        let mut queue = VecDeque::from([(provider.clone(), vec![provider.clone()])]);
        let mut visited = BTreeSet::from([provider.clone()]);
        while let Some((current, path)) = queue.pop_front() {
            for consumer in reverse.get(current.as_str()).into_iter().flatten() {
                let consumer = (*consumer).to_owned();
                if !visited.insert(consumer.clone()) {
                    continue;
                }
                let mut dependency_path = path.clone();
                dependency_path.push(consumer.clone());
                result.insert(
                    (provider.clone(), consumer.clone()),
                    ReverseConsumer {
                        provider_unit_id: provider.clone(),
                        consumer_unit_id: consumer.clone(),
                        dependency_path: dependency_path.clone(),
                        reason: "Public contract change propagates to a reverse consumer."
                            .to_owned(),
                    },
                );
                queue.push_back((consumer, dependency_path));
            }
        }
    }
    result.into_values().collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationEvidenceRun {
    pub validation_run: ValidationRun,
    pub validation_run_ref: ValidationRunRef,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationEvidenceDiagnostic {
    pub diagnostic: Diagnostic,
    pub diagnostic_ref: DiagnosticRef,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiValidationRunSummary {
    pub validation_run_ref: ValidationRunRef,
    pub check_id: String,
    pub executable: String,
    pub args: Vec<String>,
    pub working_directory: String,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiEvidenceSummary {
    pub gate_state: star_contracts::evidence::AuthoritativeGateState,
    pub gate_decision_ref: GateDecisionRef,
    pub evidence_bundle_ref: EvidenceBundleRef,
    pub runs: Vec<AiValidationRunSummary>,
    pub duration_ms: Option<u64>,
    pub failure_summaries: Vec<String>,
    pub remaining_risk_count: usize,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceCompressionError {
    #[error("gate and evidence references are inconsistent")]
    ReferenceMismatch,
    #[error("evidence bundle is not complete and validated")]
    EvidenceIncomplete,
    #[error("gate decision is not valid against the immutable validation runs")]
    GateInvalid,
    #[error("run summary is not referenced by the evidence bundle")]
    RunNotInBundle,
    #[error("diagnostic summary is not referenced by the evidence bundle")]
    DiagnosticNotInBundle,
    #[error("an immutable evidence reference does not match its document")]
    EvidenceReferenceMismatch,
}

pub fn compress_evidence_for_ai(
    gate: &GateDecision,
    gate_ref: GateDecisionRef,
    bundle: &EvidenceBundle,
    bundle_ref: EvidenceBundleRef,
    runs: &[ValidationEvidenceRun],
    diagnostics: &[ValidationEvidenceDiagnostic],
) -> Result<AiEvidenceSummary, EvidenceCompressionError> {
    let gate_hash = serde_json::to_value(gate)
        .ok()
        .and_then(|value| canonical_sha256(&value).ok());
    let bundle_hash = serde_json::to_value(bundle)
        .ok()
        .and_then(|value| canonical_sha256(&value).ok());
    if gate_ref.gate_id != gate.gate_id
        || gate_ref.revision != gate.revision
        || gate_hash.as_ref() != Some(&gate_ref.sha256)
        || bundle_ref.evidence_bundle_id != bundle.evidence_bundle_id
        || bundle_ref.revision != bundle.revision
        || bundle_hash.as_ref() != Some(&bundle_ref.sha256)
        || bundle.gate_decision_ref != gate_ref
    {
        return Err(EvidenceCompressionError::ReferenceMismatch);
    }
    if bundle.completeness != Completeness::Complete || bundle.validate().is_err() {
        return Err(EvidenceCompressionError::EvidenceIncomplete);
    }
    if runs.iter().any(|item| {
        item.validation_run.validate().is_err()
            || !validation_run_reference_matches(&item.validation_run, &item.validation_run_ref)
    }) || diagnostics.iter().any(|item| {
        item.diagnostic.validate().is_err()
            || !diagnostic_reference_matches(&item.diagnostic, &item.diagnostic_ref)
    }) {
        return Err(EvidenceCompressionError::EvidenceReferenceMismatch);
    }
    let bundle_runs: BTreeSet<_> = bundle.validation_run_refs.iter().cloned().collect();
    let summary_runs: BTreeSet<_> = runs
        .iter()
        .map(|run| run.validation_run_ref.clone())
        .collect();
    if bundle_runs != summary_runs || summary_runs.len() != runs.len() {
        return Err(EvidenceCompressionError::RunNotInBundle);
    }
    let bundle_diagnostics: BTreeSet<_> = bundle.diagnostic_refs.iter().cloned().collect();
    let summary_diagnostics: BTreeSet<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.diagnostic_ref.clone())
        .collect();
    if bundle_diagnostics != summary_diagnostics || summary_diagnostics.len() != diagnostics.len() {
        return Err(EvidenceCompressionError::DiagnosticNotInBundle);
    }
    let immutable_runs = runs
        .iter()
        .map(|item| item.validation_run.clone())
        .collect::<Vec<_>>();
    if gate.validate_against(&immutable_runs).is_err()
        || gate
            .blocking_diagnostic_refs
            .iter()
            .any(|reference| !bundle_diagnostics.contains(reference))
    {
        return Err(EvidenceCompressionError::GateInvalid);
    }
    let summaries = runs
        .iter()
        .map(|item| {
            let run = &item.validation_run;
            let duration_ms =
                run.started_at
                    .zip(run.finished_at)
                    .and_then(|(started_at, finished_at)| {
                        u64::try_from((finished_at - started_at).num_milliseconds()).ok()
                    });
            AiValidationRunSummary {
                validation_run_ref: item.validation_run_ref.clone(),
                check_id: run.check_ref.catalog_id.clone(),
                executable: run.invocation.executable.clone(),
                args: run.invocation.args.clone(),
                working_directory: run.invocation.cwd.path.clone(),
                exit_code: run.exit_code,
                duration_ms,
            }
        })
        .collect::<Vec<_>>();
    let duration_ms = summaries
        .iter()
        .map(|summary| summary.duration_ms)
        .collect::<Option<Vec<_>>>()
        .map(|durations| durations.into_iter().sum());
    let blocking_diagnostics: BTreeSet<_> = gate.blocking_diagnostic_refs.iter().collect();
    let failure_summaries = diagnostics
        .iter()
        .filter(|item| {
            blocking_diagnostics.contains(&item.diagnostic_ref)
                || (matches!(
                    item.diagnostic.severity,
                    DiagnosticSeverity::Error | DiagnosticSeverity::Critical
                ) && !matches!(
                    item.diagnostic.status,
                    DiagnosticStatus::Resolved | DiagnosticStatus::Suppressed
                ))
        })
        .map(|item| {
            if item.diagnostic.title.trim().is_empty() {
                item.diagnostic.rule_id.catalog_id.clone()
            } else {
                item.diagnostic.title.clone()
            }
        })
        .collect();
    Ok(AiEvidenceSummary {
        gate_state: gate.authoritative_state(),
        gate_decision_ref: gate_ref,
        evidence_bundle_ref: bundle_ref,
        runs: summaries,
        duration_ms,
        failure_summaries,
        remaining_risk_count: bundle.remaining_risks.len(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use chrono::{DateTime, Utc};
    use star_contracts::{
        evidence::{
            ActorRef, ActorType, ArtifactKind, ArtifactManifest, ArtifactRef, CatalogRef,
            ChangeEvidenceRefs, DocumentRef, EvidenceBundleSchemaId, GateDecisionKind,
            GateDecisionSchemaId, GatePolicySnapshot, GateScope, OutputLimits, ProducerRef,
            ProjectPathKind, ProjectPathRef, RedactionStatus, RetentionClass, TaskInvocation,
            TerminationReason, VALIDATION_PLAN_SCHEMA_ID, ValidationCache, ValidationChangeSource,
            ValidationRunSchemaId,
        },
        ids::{
            ArtifactId, EvidenceBundleId, GateId, GoalId, ProjectId, RunId, TaskInvocationId,
            ValidationRunId,
        },
    };

    use super::*;

    fn hash(value: &str) -> Sha256Hash {
        Sha256Hash::digest(value.as_bytes())
    }

    fn fingerprints() -> ValidationInputFingerprintComponents {
        ValidationInputFingerprintComponents {
            revision: "0123456789abcdef".to_owned(),
            staged_diff: hash("staged"),
            unstaged_diff: hash("unstaged"),
            untracked_content: hash("untracked"),
            toolchain: hash("toolchain"),
            lockfile: hash("lockfile"),
            project_manifest: hash("project-manifest"),
            validation_scripts: hash("validation-scripts"),
            config: hash("config"),
            policy_schema_version: VALIDATION_POLICY_SCHEMA_VERSION,
            evidence_schema_version: EVIDENCE_CONTRACT_SCHEMA_VERSION,
        }
    }

    fn command(profile: ValidationProfile) -> ValidationCheckDefinition {
        let name = format!("{:?}", profile).to_ascii_lowercase();
        ValidationCheckDefinition {
            profile,
            check_id: format!("{name}-check"),
            unit_id: if profile >= ValidationProfile::Full {
                "workspace".to_owned()
            } else {
                "affected".to_owned()
            },
            command: ValidationCommand {
                executable: "validator".to_owned(),
                args: vec![name],
                working_directory: ".".to_owned(),
                expected_exit_codes: BTreeSet::from([0]),
            },
            selection_reason: "Selected by the effective profile.".to_owned(),
        }
    }

    fn changed(
        path: &str,
        change_class: ValidationChangeClass,
        unit: Option<&str>,
    ) -> ValidationChangedFile {
        ValidationChangedFile {
            path: path.to_owned(),
            sources: vec![ValidationChangeSource::Unstaged],
            change_class,
            direct_unit: unit.map(str::to_owned),
        }
    }

    fn input(files: Vec<ValidationChangedFile>) -> ValidationPlanningInput {
        ValidationPlanningInput {
            project_key: "star-control".to_owned(),
            revision: "0123456789abcdef".to_owned(),
            requested_profile: None,
            requested_unit: None,
            requested_unit_required_profile: None,
            workspace_unit_id: "workspace".to_owned(),
            changed_files: files,
            dependencies: vec![
                UnitDependency {
                    provider_unit_id: "star-contracts".to_owned(),
                    consumer_unit_id: "star-validation".to_owned(),
                },
                UnitDependency {
                    provider_unit_id: "star-validation".to_owned(),
                    consumer_unit_id: "star-controller".to_owned(),
                },
            ],
            checks: vec![
                command(ValidationProfile::Quick),
                command(ValidationProfile::Target),
                command(ValidationProfile::Full),
                command(ValidationProfile::Release),
            ],
            cache_candidates: Vec::new(),
            fingerprints: fingerprints(),
            fingerprints_complete: true,
            impact_complete: true,
            repeated_failures: false,
        }
    }

    #[test]
    fn docs_select_quick_and_internal_code_selects_target() {
        let docs = build_validation_plan(input(vec![changed(
            "docs/README.md",
            ValidationChangeClass::Documentation,
            Some("docs"),
        )]))
        .unwrap();
        assert_eq!(docs.profile.selected, ValidationProfile::Quick);
        assert_eq!(docs.checks[0].check_id, "quick-check");

        let mut explicit_target_docs = input(vec![changed(
            "docs/README.md",
            ValidationChangeClass::Documentation,
            Some("docs"),
        )]);
        explicit_target_docs.requested_profile = Some(ValidationProfile::Target);
        assert_eq!(
            build_validation_plan(explicit_target_docs)
                .unwrap()
                .profile
                .selected,
            ValidationProfile::Quick
        );

        let code = build_validation_plan(input(vec![changed(
            "crates/control/star-validation/src/lib.rs",
            ValidationChangeClass::InternalCode,
            Some("star-validation"),
        )]))
        .unwrap();
        assert_eq!(code.profile.selected, ValidationProfile::Target);
        assert_eq!(code.readiness, ValidationPlanReadiness::Ready);
    }

    #[test]
    fn clean_explicit_unit_uses_the_unit_profile_instead_of_workspace_default() {
        let mut docs = input(Vec::new());
        docs.requested_unit = Some("docs".to_owned());
        docs.requested_unit_required_profile = Some(ValidationProfile::Quick);
        let plan = build_validation_plan(docs).unwrap();
        assert_eq!(plan.profile.selected, ValidationProfile::Quick);
        assert_eq!(plan.direct_units[0].unit_id, "docs");

        let mut package = input(Vec::new());
        package.requested_unit = Some("star-validation".to_owned());
        package.requested_unit_required_profile = Some(ValidationProfile::Target);
        let plan = build_validation_plan(package).unwrap();
        assert_eq!(plan.profile.selected, ValidationProfile::Target);
        assert_eq!(plan.direct_units[0].unit_id, "star-validation");
    }

    #[test]
    fn public_contract_selects_full_and_transitive_reverse_consumers() {
        let plan = build_validation_plan(input(vec![changed(
            "crates/foundation/star-contracts/src/evidence.rs",
            ValidationChangeClass::PublicContract,
            Some("star-contracts"),
        )]))
        .unwrap();
        assert_eq!(plan.profile.selected, ValidationProfile::Full);
        assert_eq!(plan.reverse_consumers.len(), 2);
        assert!(
            plan.direct_units
                .iter()
                .any(|unit| unit.unit_id == "star-controller")
        );
        assert_eq!(
            plan.independent_review.triggers,
            vec![IndependentReviewTrigger::PublicContract]
        );
    }

    #[test]
    fn unknown_or_unmapped_impact_expands_and_requires_human_review() {
        let mut planning = input(vec![changed(
            "opaque.input",
            ValidationChangeClass::Unknown,
            None,
        )]);
        planning.impact_complete = false;
        let plan = build_validation_plan(planning).unwrap();
        assert_eq!(plan.profile.selected, ValidationProfile::Full);
        assert_eq!(plan.readiness, ValidationPlanReadiness::HumanReview);
        assert!(
            plan.uncertainties
                .iter()
                .any(|item| item.code == ValidationUncertaintyCode::UnitMappingMissing)
        );
    }

    #[test]
    fn unavailable_fingerprint_input_disables_reuse_and_requires_human_review() {
        let mut planning = input(vec![changed(
            "src/lib.rs",
            ValidationChangeClass::InternalCode,
            Some("star-validation"),
        )]);
        planning.fingerprints_complete = false;
        let plan = build_validation_plan(planning).unwrap();
        assert_eq!(plan.profile.selected, ValidationProfile::Full);
        assert_eq!(plan.readiness, ValidationPlanReadiness::HumanReview);
        assert!(plan.checks.iter().all(|check| {
            check.disposition == PlannedCheckDisposition::Execute
                && check.source_validation_run_ref.is_none()
        }));
        assert!(
            plan.uncertainties.iter().any(|item| {
                item.code == ValidationUncertaintyCode::FingerprintInputUnavailable
            })
        );
    }

    #[test]
    fn release_is_only_selected_explicitly_and_requires_independent_review() {
        let mut planning = input(vec![changed(
            "src/lib.rs",
            ValidationChangeClass::InternalCode,
            Some("star-validation"),
        )]);
        planning.requested_profile = Some(ValidationProfile::Release);
        let plan = build_validation_plan(planning).unwrap();
        assert_eq!(plan.profile.selected, ValidationProfile::Release);
        assert!(
            plan.independent_review
                .triggers
                .contains(&IndependentReviewTrigger::Release)
        );
    }

    fn at(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn passing_run() -> ValidationRun {
        ValidationRun {
            schema_id: ValidationRunSchemaId::ValidationRun,
            schema_version: 1,
            validation_run_id: ValidationRunId::new(),
            revision: 1,
            created_at: at("2026-07-16T00:00:00Z"),
            updated_at: at("2026-07-16T00:00:01Z"),
            producer: ProducerRef {
                component: "test".to_owned(),
                product_version: "1".to_owned(),
                build_id: "test".to_owned(),
                platform: "windows-x64".to_owned(),
            },
            extensions: BTreeMap::new(),
            validation_plan_ref: DocumentRef {
                schema_id: VALIDATION_PLAN_SCHEMA_ID.to_owned(),
                document_id: "plan".to_owned(),
                revision: 1,
                sha256: hash("plan"),
            },
            check_ref: CatalogRef {
                catalog_id: "target-check".to_owned(),
                format_version: 1,
                item_version: "1".to_owned(),
                sha256: hash("check"),
            },
            tool_ref: CatalogRef {
                catalog_id: "tools".to_owned(),
                format_version: 1,
                item_version: "1".to_owned(),
                sha256: hash("tool"),
            },
            attempt: 1,
            invocation: TaskInvocation {
                invocation_id: TaskInvocationId::new(),
                tool_ref: CatalogRef {
                    catalog_id: "tools".to_owned(),
                    format_version: 1,
                    item_version: "1".to_owned(),
                    sha256: hash("tool"),
                },
                executable: "validator".to_owned(),
                args: Vec::new(),
                cwd: ProjectPathRef {
                    project_id: ProjectId::new(),
                    path: ".".to_owned(),
                    path_kind: ProjectPathKind::Directory,
                },
                env_refs: BTreeMap::new(),
                stdin_ref: None,
                timeout_ms: 1_000,
                permission_action: "process_run".to_owned(),
                idempotency_key: "test".to_owned(),
                expected_exit_codes: BTreeSet::from([0]),
                output_limits: OutputLimits {
                    stdout_bytes: 1,
                    stderr_bytes: 1,
                    artifact_bytes: 1,
                },
            },
            started_at: Some(at("2026-07-16T00:00:00Z")),
            finished_at: Some(at("2026-07-16T00:00:01Z")),
            outcome: ValidationOutcome::Pass,
            completeness: Completeness::Complete,
            exit_code: Some(0),
            termination_reason: Some(TerminationReason::Exited),
            diagnostic_refs: Vec::new(),
            stdout_ref: None,
            stderr_ref: None,
            result_artifact_refs: Vec::new(),
            observed_tool: None,
            cache: None,
        }
    }

    fn run_ref(run: &ValidationRun) -> ValidationRunRef {
        ValidationRunRef {
            validation_run_id: run.validation_run_id.clone(),
            revision: run.revision,
            sha256: canonical_sha256(&serde_json::to_value(run).unwrap()).unwrap(),
        }
    }

    fn candidate(key: Sha256Hash) -> ValidationCacheCandidate {
        let mut validation_run = passing_run();
        validation_run.cache = Some(ValidationCache {
            hit: false,
            cache_key: key.as_str().to_owned(),
            source_validation_run_ref: None,
        });
        ValidationCacheCandidate {
            check_id: "target-check".to_owned(),
            cache_key: key,
            validation_run_ref: run_ref(&validation_run),
            validation_run,
            stability: CacheValidationStability::Stable,
            suppression_applied: false,
            artifacts_available: true,
            policy_schema_version: VALIDATION_POLICY_SCHEMA_VERSION,
            evidence_schema_version: EVIDENCE_CONTRACT_SCHEMA_VERSION,
        }
    }

    #[test]
    fn cache_reuses_only_identical_complete_stable_unsuppressed_evidence() {
        let key = hash("cache-key");
        let reusable = candidate(key.clone());
        assert!(matches!(
            evaluate_cache_reuse(&key, Some(&reusable)),
            CacheReuseDecision::Reuse(_)
        ));

        let mut flaky = reusable.clone();
        flaky.stability = CacheValidationStability::Flaky;
        assert_eq!(
            evaluate_cache_reuse(&key, Some(&flaky)),
            CacheReuseDecision::Execute(CacheMissReason::Unstable)
        );
        let mut suppressed = reusable.clone();
        suppressed.suppression_applied = true;
        assert_eq!(
            evaluate_cache_reuse(&key, Some(&suppressed)),
            CacheReuseDecision::Execute(CacheMissReason::Suppressed)
        );
        let mut missing_artifact = reusable.clone();
        missing_artifact.artifacts_available = false;
        assert_eq!(
            evaluate_cache_reuse(&key, Some(&missing_artifact)),
            CacheReuseDecision::Execute(CacheMissReason::ArtifactUnavailable)
        );
        let mut failed = reusable.clone();
        failed.validation_run.outcome = ValidationOutcome::Fail;
        assert_eq!(
            evaluate_cache_reuse(&key, Some(&failed)),
            CacheReuseDecision::Execute(CacheMissReason::OutcomeNotPass)
        );
        let mut partial = reusable.clone();
        partial.validation_run.completeness = Completeness::Partial;
        assert_eq!(
            evaluate_cache_reuse(&key, Some(&partial)),
            CacheReuseDecision::Execute(CacheMissReason::Incomplete)
        );
        let mut unverified = reusable.clone();
        unverified.validation_run.completeness = Completeness::Unverified;
        assert_eq!(
            evaluate_cache_reuse(&key, Some(&unverified)),
            CacheReuseDecision::Execute(CacheMissReason::Incomplete)
        );
        let mut bad_exit = reusable.clone();
        bad_exit.validation_run.exit_code = Some(1);
        assert_eq!(
            evaluate_cache_reuse(&key, Some(&bad_exit)),
            CacheReuseDecision::Execute(CacheMissReason::ExecutionUnverified)
        );
        let mut mismatched_reference = reusable.clone();
        mismatched_reference.validation_run_ref.sha256 = hash("not-the-run");
        assert_eq!(
            evaluate_cache_reuse(&key, Some(&mismatched_reference)),
            CacheReuseDecision::Execute(CacheMissReason::EvidenceReferenceMismatch)
        );
        let mut unbound_cache_key = reusable.clone();
        unbound_cache_key.validation_run.cache = None;
        assert_eq!(
            evaluate_cache_reuse(&key, Some(&unbound_cache_key)),
            CacheReuseDecision::Execute(CacheMissReason::KeyMismatch)
        );
        let mut mismatched_check = reusable.clone();
        mismatched_check.check_id = "different-check".to_owned();
        assert_eq!(
            evaluate_cache_reuse(&key, Some(&mismatched_check)),
            CacheReuseDecision::Execute(CacheMissReason::EvidenceReferenceMismatch)
        );
        let mut old_policy = reusable.clone();
        old_policy.policy_schema_version = VALIDATION_POLICY_SCHEMA_VERSION + 1;
        assert_eq!(
            evaluate_cache_reuse(&key, Some(&old_policy)),
            CacheReuseDecision::Execute(CacheMissReason::PolicySchemaChanged)
        );
        let mut old_evidence = reusable.clone();
        old_evidence.evidence_schema_version = EVIDENCE_CONTRACT_SCHEMA_VERSION + 1;
        assert_eq!(
            evaluate_cache_reuse(&key, Some(&old_evidence)),
            CacheReuseDecision::Execute(CacheMissReason::EvidenceSchemaChanged)
        );
        assert_eq!(
            evaluate_cache_reuse(&hash("different"), Some(&reusable)),
            CacheReuseDecision::Execute(CacheMissReason::KeyMismatch)
        );

        let base = ValidationCacheKeyInputs {
            inputs: fingerprints(),
            command: hash("command"),
        };
        let base_key = base.fingerprint().unwrap();
        let mut changed_inputs = Vec::new();

        let mut changed = base.clone();
        changed.inputs.revision = "b".repeat(40);
        changed_inputs.push(changed);
        let mut changed = base.clone();
        changed.inputs.staged_diff = hash("changed-staged");
        changed_inputs.push(changed);
        let mut changed = base.clone();
        changed.inputs.unstaged_diff = hash("changed-unstaged");
        changed_inputs.push(changed);
        let mut changed = base.clone();
        changed.inputs.untracked_content = hash("changed-untracked");
        changed_inputs.push(changed);
        let mut changed = base.clone();
        changed.inputs.toolchain = hash("changed-toolchain");
        changed_inputs.push(changed);
        let mut changed = base.clone();
        changed.inputs.lockfile = hash("changed-lockfile");
        changed_inputs.push(changed);
        let mut changed = base.clone();
        changed.inputs.project_manifest = hash("changed-manifest");
        changed_inputs.push(changed);
        let mut changed = base.clone();
        changed.inputs.validation_scripts = hash("changed-validator");
        changed_inputs.push(changed);
        let mut changed = base.clone();
        changed.inputs.config = hash("changed-config");
        changed_inputs.push(changed);
        let mut changed = base.clone();
        changed.inputs.policy_schema_version += 1;
        changed_inputs.push(changed);
        let mut changed = base.clone();
        changed.inputs.evidence_schema_version += 1;
        changed_inputs.push(changed);
        let mut changed = base.clone();
        changed.command = hash("changed-command");
        changed_inputs.push(changed);

        for changed in changed_inputs {
            assert_ne!(base_key, changed.fingerprint().unwrap());
        }
    }

    fn document(schema_id: &str, document_id: &str) -> DocumentRef {
        DocumentRef {
            schema_id: schema_id.to_owned(),
            document_id: document_id.to_owned(),
            revision: 1,
            sha256: hash(document_id),
        }
    }

    fn artifact(path: &str, kind: ArtifactKind) -> ArtifactRef {
        ArtifactRef {
            artifact_id: ArtifactId::new(),
            kind,
            project_id: None,
            relative_path: path.to_owned(),
            media_type: "application/json".to_owned(),
            size_bytes: 1,
            sha256: hash(path),
            created_at: at("2026-07-16T00:00:00Z"),
            producer: ProducerRef {
                component: "test".to_owned(),
                product_version: "1".to_owned(),
                build_id: "test".to_owned(),
                platform: "windows-x64".to_owned(),
            },
            redaction_status: RedactionStatus::NotNeeded,
            retention_class: RetentionClass::Evidence,
            source_artifact_ref: None,
        }
    }

    #[test]
    fn ai_summary_requires_exact_complete_gate_bundle_and_run_refs() {
        let run = passing_run();
        let validation_run_ref = run_ref(&run);
        let gate = GateDecision {
            schema_id: GateDecisionSchemaId::GateDecision,
            schema_version: 1,
            gate_id: GateId::new(),
            revision: 1,
            created_at: at("2026-07-16T00:00:02Z"),
            updated_at: at("2026-07-16T00:00:02Z"),
            producer: ProducerRef {
                component: "test".to_owned(),
                product_version: "1".to_owned(),
                build_id: "test".to_owned(),
                platform: "windows-x64".to_owned(),
            },
            extensions: BTreeMap::new(),
            scope: GateScope::Goal {
                goal_id: GoalId::new(),
                run_id: RunId::new(),
                revision: 1,
            },
            decision: GateDecisionKind::AutoPass,
            required_run_refs: vec![validation_run_ref.clone()],
            satisfied_run_refs: vec![validation_run_ref.clone()],
            blocking_diagnostic_refs: Vec::new(),
            waivers: Vec::new(),
            omissions: Vec::new(),
            remaining_risks: Vec::new(),
            policy_snapshot: GatePolicySnapshot {
                policy_ref: document("star.gate-policy", "policy"),
                policy_sha256: hash("policy"),
                thresholds: BTreeMap::new(),
            },
            decided_by: ActorRef {
                actor_type: ActorType::Controller,
                actor_id: "controller".to_owned(),
                display_name: "Controller".to_owned(),
                auth_source: "test".to_owned(),
            },
        };
        let gate_ref = GateDecisionRef {
            gate_id: gate.gate_id.clone(),
            revision: gate.revision,
            sha256: canonical_sha256(&serde_json::to_value(&gate).unwrap()).unwrap(),
        };
        let bundle = EvidenceBundle {
            schema_id: EvidenceBundleSchemaId::EvidenceBundle,
            schema_version: 1,
            evidence_bundle_id: EvidenceBundleId::new(),
            revision: 1,
            created_at: at("2026-07-16T00:00:03Z"),
            updated_at: at("2026-07-16T00:00:03Z"),
            producer: gate.producer.clone(),
            extensions: BTreeMap::new(),
            goal_spec_ref: document("star.goal-spec", "goal"),
            stage_graph_ref: document("star.stage-graph", "stage"),
            final_revision_ref: document("star.project-revision", "revision"),
            stage_evidence: Vec::new(),
            change_evidence: ChangeEvidenceRefs {
                before_fingerprint: hash("before"),
                after_fingerprint: hash("after"),
                change_set_ref: document("star.change-set", "change"),
                changed_files_ref: artifact("evidence/changed-files.json", ArtifactKind::ChangeSet),
            },
            validation_plan_refs: vec![document(VALIDATION_PLAN_SCHEMA_ID, "plan")],
            validation_run_refs: vec![validation_run_ref.clone()],
            diagnostic_refs: Vec::new(),
            gate_decision_ref: gate_ref.clone(),
            event_ranges: Vec::new(),
            cost_record_refs: Vec::new(),
            unmeasured_usage: Vec::new(),
            merge_result_ref: None,
            remaining_risks: Vec::new(),
            handoff_ref: None,
            artifact_manifest: ArtifactManifest {
                manifest_ref: artifact("evidence/manifest.json", ArtifactKind::Manifest),
                artifacts: Vec::new(),
            },
            completeness: Completeness::Complete,
            missing_reasons: Vec::new(),
        };
        let bundle_ref = EvidenceBundleRef {
            evidence_bundle_id: bundle.evidence_bundle_id.clone(),
            revision: bundle.revision,
            sha256: canonical_sha256(&serde_json::to_value(&bundle).unwrap()).unwrap(),
        };
        let mut tampered_run_ref = validation_run_ref.clone();
        tampered_run_ref.sha256 = hash("not-the-run");
        assert_eq!(
            compress_evidence_for_ai(
                &gate,
                gate_ref.clone(),
                &bundle,
                bundle_ref.clone(),
                &[ValidationEvidenceRun {
                    validation_run: run.clone(),
                    validation_run_ref: tampered_run_ref,
                }],
                &[],
            ),
            Err(EvidenceCompressionError::EvidenceReferenceMismatch)
        );
        let summary = compress_evidence_for_ai(
            &gate,
            gate_ref,
            &bundle,
            bundle_ref,
            &[ValidationEvidenceRun {
                validation_run: run,
                validation_run_ref,
            }],
            &[],
        )
        .unwrap();
        assert_eq!(summary.duration_ms, Some(1_000));
        assert_eq!(summary.runs.len(), 1);
        assert_eq!(summary.runs[0].executable, "validator");
        assert!(summary.failure_summaries.is_empty());
    }
}
