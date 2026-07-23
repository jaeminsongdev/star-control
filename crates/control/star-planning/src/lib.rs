//! Pure M2 scope, impact, risk-path, and affected-check planner.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use star_contracts::{
    Sha256Hash,
    evidence::{ActorRef, DocumentRef},
    ids::{
        ChangeSetId, ImpactAnalysisId, ProjectId, ScopeRevisionId, TaskSpecId, ValidationPlanId,
    },
    index::{
        CodeIndexSnapshot, IndexEdge, IndexEntity, IndexEntityKind, IndexFreshnessState,
        IndexRelation, IndexTier, ProjectCatalogSnapshot, SourceClass, SourceEntry,
    },
    managed_registry::{
        EvidenceCompleteness, ManagedRegistrySnapshot, RegistryFreshness, RegistryResolutionState,
    },
    management::{ProjectPathRef, SymbolResolution},
    planning::{
        AffectedProject, AffectedScope, BaselinePolicy, CHANGE_SET_SCHEMA_ID, ChangeEntry,
        ChangeOrigin, ChangeSet, ChangeSetKind, CheckApplicability, CheckCandidate,
        CheckDescriptor, CheckGraphV2, CheckInvocationTemplate, CheckOverride, CheckOverrideKind,
        CheckPlanV2, CheckResolutionOutcome, CollectionState, ExcludedScope,
        FULL_VALIDATION_PLAN_SCHEMA_ID, FallbackDecision, FullValidationPlan, GatePolicyV2,
        IMPACT_ANALYSIS_SCHEMA_ID, ImpactAnalysis, ImpactCertainty, ImpactConfidence,
        ImpactConfidenceSummary, ImpactEdge, ImpactKind, ImpactProjectInput, ImpactResolution,
        ImpactSeed, ImpactStatus, ImpactedNode, IntendedChange, NoResult, NoResultReason,
        ObservedChangeKind, PlanningBundle, PlanningContractError, PlanningSelector, ProjectTarget,
        ProjectTargetRole, ReviewKind, ReviewRequirementV2, RiskPathDescriptor, RiskPathFinding,
        RiskSeverityFloor, SCOPE_REVISION_SCHEMA_ID, ScopeApprovalState, ScopeAxis,
        ScopeItemSource, ScopeReasonCode, ScopeRelation, ScopeRevision, ScopeSet,
        ScopeSourceSnapshotRef, ScopeUserDecision, ScopedSelector, SeedResolution, SelectorKind,
        SuccessCriterion, TASK_SPEC_SCHEMA_ID, TaskSpec, ValidationPlanV2Readiness,
        ValidationRiskLevel, ValidationScopeLevel, document_ref, empty_fingerprint,
    },
    profile::{DevelopmentProfileResolutionV1, ProfileReviewFloorV1},
};
use star_domain::versioned_fingerprint;
use thiserror::Error;

const CATALOG_SCHEMA_ID: &str = "star.project-catalog-snapshot";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSpecDraft {
    pub title: String,
    pub objective: String,
    pub project_targets: Vec<ProjectTarget>,
    pub included_scope: Vec<PlanningSelector>,
    #[serde(default)]
    pub excluded_scope: Vec<ExcludedScope>,
    pub intended_changes: Vec<IntendedChange>,
    pub success_criteria: Vec<SuccessCriterion>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub forbidden_actions: Vec<String>,
    #[serde(default)]
    pub profile_ids: Vec<String>,
    pub baseline_policy: BaselinePolicy,
    #[serde(default)]
    pub requested_checks: Vec<String>,
    #[serde(default)]
    pub check_overrides: Vec<CheckOverride>,
    #[serde(default)]
    pub assumptions: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct PlanningProjectIndex {
    pub snapshot: CodeIndexSnapshot,
    pub source_entries: Vec<SourceEntry>,
    pub entities: Vec<IndexEntity>,
    pub edges: Vec<IndexEdge>,
    pub managed_registry_snapshot: Option<ManagedRegistrySnapshot>,
    pub observed_changes: Vec<ObservedWorkspaceChange>,
    pub collection_state: CollectionState,
    pub collection_limits: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ObservedWorkspaceChange {
    pub path: ProjectPathRef,
    pub rename_from: Option<ProjectPathRef>,
    pub change_kind: ObservedChangeKind,
    pub before_sha256: Option<Sha256Hash>,
    pub after_sha256: Option<Sha256Hash>,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub binary: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlanningPolicy {
    pub max_depth: u32,
    pub max_nodes: usize,
    pub max_edges: usize,
    pub max_check_candidates: usize,
    pub max_parallel_checks: u32,
}

impl Default for PlanningPolicy {
    fn default() -> Self {
        Self {
            max_depth: 16,
            max_nodes: 50_000,
            max_edges: 200_000,
            max_check_candidates: 2_000,
            max_parallel_checks: 4,
        }
    }
}

pub struct PlanningRequest {
    pub task: TaskSpecDraft,
    pub actor: ActorRef,
    pub catalog: ProjectCatalogSnapshot,
    pub projects: Vec<PlanningProjectIndex>,
    pub risk_descriptors: Vec<RiskPathDescriptor>,
    pub check_descriptors: Vec<CheckDescriptor>,
    pub previous_success_evidence: Vec<PreviousSuccessEvidence>,
    pub profile_resolution: Option<DevelopmentProfileResolutionV1>,
    pub policy: PlanningPolicy,
}

#[derive(Clone, Debug)]
pub struct PreviousSuccessEvidence {
    pub project_id: ProjectId,
    pub evidence_bundle_id: String,
    pub bundle_fingerprint: Sha256Hash,
    pub validation_plan: FullValidationPlan,
    pub source_snapshot_refs: Vec<ScopeSourceSnapshotRef>,
}

pub struct PlanningRevisionRequest {
    pub previous: PlanningBundle,
    pub request: PlanningRequest,
    pub reason_code: ScopeReasonCode,
    pub reason: String,
    pub user_decisions: Vec<ScopeUserDecision>,
}

#[derive(Debug, Error)]
pub enum PlanningError {
    #[error("task input is incomplete or invalid")]
    TaskInput,
    #[error("requested and excluded scope conflict")]
    ScopeConflict,
    #[error("a required project snapshot is absent or stale")]
    SnapshotUnavailable,
    #[error("planning graph resource limit was reached")]
    ResourceLimit,
    #[error("planning contract could not be sealed")]
    Contract(#[from] PlanningContractError),
    #[error("planning fingerprint failed")]
    Fingerprint,
}

pub fn build_planning_bundle(request: PlanningRequest) -> Result<PlanningBundle, PlanningError> {
    build_planning_bundle_for_phase(request, "during_stage")
}

pub fn build_planning_bundle_for_phase(
    request: PlanningRequest,
    validation_phase: &str,
) -> Result<PlanningBundle, PlanningError> {
    if !matches!(
        validation_phase,
        "during_stage" | "goal_exit" | "patch_pre_apply" | "patch_post_apply"
    ) {
        return Err(PlanningError::TaskInput);
    }
    validate_policy(&request.policy)?;
    let profile_resolution = request.profile_resolution.clone();
    let task_spec = build_task_spec(request.task, request.actor.clone())?;
    if profile_resolution.as_ref().is_some_and(|resolution| {
        resolution
            .selected_profiles
            .iter()
            .map(|profile| profile.profile_id.clone())
            .collect::<Vec<_>>()
            != task_spec.profile_ids
    }) || (profile_resolution.is_none() && !task_spec.profile_ids.is_empty())
    {
        return Err(PlanningError::TaskInput);
    }
    let project_map = request
        .projects
        .iter()
        .map(|project| (project.snapshot.project_id.clone(), project))
        .collect::<BTreeMap<_, _>>();
    let scope_revision =
        build_scope_revision(&task_spec, &request.catalog, &project_map, request.actor)?;
    let task_ref = task_ref(&task_spec);
    let scope_ref = scope_ref(&scope_revision);
    let mut change_sets = Vec::new();
    for target in &task_spec.project_targets {
        if target.role == ProjectTargetRole::ReadOnlyImpact {
            continue;
        }
        let project = project_map
            .get(&target.project_id)
            .copied()
            .ok_or(PlanningError::SnapshotUnavailable)?;
        change_sets.push(build_change_set(
            &task_spec,
            &task_ref,
            &scope_revision,
            &scope_ref,
            project,
        )?);
    }
    change_sets.sort_by(|left, right| left.project_id.cmp(&right.project_id));
    let impact_analysis = analyze_impact(
        &task_spec,
        &task_ref,
        &scope_revision,
        &scope_ref,
        &request.catalog,
        &request.projects,
        &change_sets,
        &request.risk_descriptors,
        &request.policy,
    )?;
    let validation_plan = select_validation_plan(
        &task_spec,
        &task_ref,
        &scope_revision,
        &scope_ref,
        &request.catalog,
        &change_sets,
        &impact_analysis,
        &request.check_descriptors,
        &request.previous_success_evidence,
        profile_resolution.as_ref(),
        &request.policy,
        validation_phase,
    )?;
    PlanningBundle {
        schema_id: "star.planning-bundle".to_owned(),
        schema_version: 1,
        task_spec,
        scope_revision,
        change_sets,
        impact_analysis,
        validation_plan,
        bundle_fingerprint: empty_fingerprint(),
    }
    .seal()
    .map_err(PlanningError::from)
}

pub fn planning_bundle_revision(bundle: &PlanningBundle) -> u64 {
    bundle
        .task_spec
        .revision
        .max(bundle.scope_revision.revision)
        .max(bundle.impact_analysis.revision)
        .max(bundle.validation_plan.revision)
}

pub fn task_spec_to_draft(task: &TaskSpec) -> TaskSpecDraft {
    TaskSpecDraft {
        title: task.title.clone(),
        objective: task.objective.clone(),
        project_targets: task.project_targets.clone(),
        included_scope: task.included_scope.clone(),
        excluded_scope: task.excluded_scope.clone(),
        intended_changes: task.intended_changes.clone(),
        success_criteria: task.success_criteria.clone(),
        constraints: task.constraints.clone(),
        forbidden_actions: task.forbidden_actions.clone(),
        profile_ids: task.profile_ids.clone(),
        baseline_policy: task.baseline_policy.clone(),
        requested_checks: task.requested_checks.clone(),
        check_overrides: task.check_overrides.clone(),
        assumptions: task.assumptions.clone(),
    }
}

pub fn revise_planning_bundle(
    revision: PlanningRevisionRequest,
) -> Result<PlanningBundle, PlanningError> {
    if revision.reason.trim().is_empty() {
        return Err(PlanningError::TaskInput);
    }
    let previous = revision.previous;
    let mut next = build_planning_bundle(revision.request)?;

    next.task_spec.task_spec_id = previous.task_spec.task_spec_id.clone();
    next.task_spec.revision = previous
        .task_spec
        .revision
        .checked_add(1)
        .ok_or(PlanningError::ResourceLimit)?;
    next.task_spec = next.task_spec.seal()?;
    let next_task_ref = task_ref(&next.task_spec);

    let previous_scope_ref = scope_ref(&previous.scope_revision);
    let changed_fields = planning_changed_fields(&previous, &next);
    next.scope_revision.scope_revision_id = previous.scope_revision.scope_revision_id.clone();
    next.scope_revision.revision = previous
        .scope_revision
        .revision
        .checked_add(1)
        .ok_or(PlanningError::ResourceLimit)?;
    next.scope_revision.task_spec_ref = next_task_ref.clone();
    next.scope_revision.previous_scope_revision_ref = Some(previous_scope_ref);
    next.scope_revision.reason_code = revision.reason_code;
    next.scope_revision.reason = revision.reason;
    next.scope_revision.user_decisions = revision.user_decisions;
    next.scope_revision.changed_fields = changed_fields;
    next.scope_revision = next.scope_revision.seal()?;
    let next_scope_ref = scope_ref(&next.scope_revision);

    for change_set in &mut next.change_sets {
        change_set.task_spec_ref = next_task_ref.clone();
        change_set.scope_revision_ref = next_scope_ref.clone();
        *change_set = change_set.clone().seal()?;
    }
    let change_set_refs = planning_change_set_refs(&next.change_sets);

    next.impact_analysis.impact_analysis_id = previous.impact_analysis.impact_analysis_id.clone();
    next.impact_analysis.revision = previous
        .impact_analysis
        .revision
        .checked_add(1)
        .ok_or(PlanningError::ResourceLimit)?;
    next.impact_analysis.task_spec_ref = next_task_ref.clone();
    next.impact_analysis.scope_revision_ref = next_scope_ref.clone();
    next.impact_analysis.change_set_refs = change_set_refs.clone();
    next.impact_analysis = next.impact_analysis.seal()?;

    next.validation_plan.validation_plan_id = previous.validation_plan.validation_plan_id.clone();
    next.validation_plan.revision = previous
        .validation_plan
        .revision
        .checked_add(1)
        .ok_or(PlanningError::ResourceLimit)?;
    next.validation_plan.task_spec_ref = next_task_ref;
    next.validation_plan.scope_revision = next.scope_revision.revision;
    next.validation_plan.scope_revision_ref = next_scope_ref;
    next.validation_plan.change_set_refs = change_set_refs;
    next.validation_plan.impact_analysis_ref = impact_ref(&next.impact_analysis);
    next.validation_plan = next.validation_plan.seal()?;
    next.seal().map_err(PlanningError::from)
}

pub fn invalidate_planning_bundle(
    mut bundle: PlanningBundle,
    reason: &str,
) -> Result<PlanningBundle, PlanningError> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(PlanningError::TaskInput);
    }
    bundle.impact_analysis.revision = bundle
        .impact_analysis
        .revision
        .checked_add(1)
        .ok_or(PlanningError::ResourceLimit)?;
    bundle.impact_analysis.status = ImpactStatus::Invalidated;
    bundle
        .impact_analysis
        .limitations
        .push("PLAN_INVALIDATED".to_owned());
    normalize_strings(&mut bundle.impact_analysis.limitations);
    bundle.impact_analysis = bundle.impact_analysis.seal()?;

    bundle.validation_plan.revision = bundle
        .validation_plan
        .revision
        .checked_add(1)
        .ok_or(PlanningError::ResourceLimit)?;
    bundle.validation_plan.impact_analysis_ref = impact_ref(&bundle.impact_analysis);
    bundle.validation_plan.readiness = ValidationPlanV2Readiness::Invalidated;
    bundle
        .validation_plan
        .manual_observations
        .push(format!("PLAN_INVALIDATED:{reason}"));
    normalize_strings(&mut bundle.validation_plan.manual_observations);
    bundle.validation_plan = bundle.validation_plan.seal()?;
    bundle.seal().map_err(PlanningError::from)
}

fn planning_changed_fields(previous: &PlanningBundle, next: &PlanningBundle) -> Vec<String> {
    let mut changed = Vec::new();
    let before = &previous.task_spec;
    let after = &next.task_spec;
    if before.title != after.title || before.objective != after.objective {
        changed.push("objective".to_owned());
    }
    if before.project_targets != after.project_targets {
        changed.push("project_targets".to_owned());
    }
    if before.included_scope != after.included_scope
        || before.excluded_scope != after.excluded_scope
    {
        changed.push("requested_scope".to_owned());
    }
    if before.intended_changes != after.intended_changes {
        changed.push("intended_changes".to_owned());
    }
    if before.success_criteria != after.success_criteria {
        changed.push("success_criteria".to_owned());
    }
    if before.constraints != after.constraints
        || before.forbidden_actions != after.forbidden_actions
        || before.assumptions != after.assumptions
    {
        changed.push("policy".to_owned());
    }
    if before.requested_checks != after.requested_checks
        || before.check_overrides != after.check_overrides
    {
        changed.push("validation_selection".to_owned());
    }
    if previous.scope_revision.source_snapshot_refs != next.scope_revision.source_snapshot_refs {
        changed.push("source_snapshot_refs".to_owned());
    }
    if changed.is_empty() {
        changed.push("replan".to_owned());
    }
    changed
}

fn planning_change_set_refs(change_sets: &[ChangeSet]) -> Vec<DocumentRef> {
    change_sets
        .iter()
        .map(|changes| {
            document_ref(
                CHANGE_SET_SCHEMA_ID,
                changes.change_set_id.as_str(),
                1,
                &changes.change_set_fingerprint,
            )
        })
        .collect()
}

fn impact_ref(impact: &ImpactAnalysis) -> DocumentRef {
    document_ref(
        IMPACT_ANALYSIS_SCHEMA_ID,
        impact.impact_analysis_id.as_str(),
        impact.revision,
        &impact.calculation_fingerprint,
    )
}

fn validate_policy(policy: &PlanningPolicy) -> Result<(), PlanningError> {
    if policy.max_depth == 0
        || policy.max_nodes == 0
        || policy.max_edges == 0
        || policy.max_check_candidates == 0
        || policy.max_parallel_checks == 0
    {
        return Err(PlanningError::ResourceLimit);
    }
    Ok(())
}

fn build_task_spec(mut draft: TaskSpecDraft, actor: ActorRef) -> Result<TaskSpec, PlanningError> {
    normalize_selectors(&mut draft.included_scope)?;
    draft.project_targets.sort_by(|left, right| {
        (&left.project_id, &left.checkout_id, left.role).cmp(&(
            &right.project_id,
            &right.checkout_id,
            right.role,
        ))
    });
    draft.requested_checks.sort();
    draft.requested_checks.dedup();
    draft.check_overrides.sort_by(|left, right| {
        (&left.family, left.kind, &left.reason).cmp(&(&right.family, right.kind, &right.reason))
    });
    if draft.check_overrides.iter().any(|override_item| {
        override_item.family.trim().is_empty() || override_item.reason.trim().is_empty()
    }) || draft
        .check_overrides
        .windows(2)
        .any(|pair| pair[0].family == pair[1].family)
    {
        return Err(PlanningError::TaskInput);
    }
    normalize_strings(&mut draft.constraints);
    normalize_strings(&mut draft.forbidden_actions);
    normalize_strings(&mut draft.profile_ids);
    normalize_strings(&mut draft.assumptions);
    draft
        .intended_changes
        .sort_by(|left, right| left.change_id.cmp(&right.change_id));
    draft
        .success_criteria
        .sort_by(|left, right| left.criterion_id.cmp(&right.criterion_id));
    if draft
        .excluded_scope
        .iter()
        .any(|excluded| draft.included_scope.contains(&excluded.selector))
    {
        return Err(PlanningError::ScopeConflict);
    }
    for excluded in &draft.excluded_scope {
        validate_selector(&excluded.selector)?;
    }
    for intended in &draft.intended_changes {
        validate_selector(&intended.selector)?;
    }
    TaskSpec {
        schema_id: TASK_SPEC_SCHEMA_ID.to_owned(),
        schema_version: 1,
        task_spec_id: TaskSpecId::new(),
        revision: 1,
        title: draft.title,
        objective: draft.objective,
        project_targets: draft.project_targets,
        included_scope: draft.included_scope,
        excluded_scope: draft.excluded_scope,
        intended_changes: draft.intended_changes,
        success_criteria: draft.success_criteria,
        constraints: draft.constraints,
        forbidden_actions: draft.forbidden_actions,
        profile_ids: draft.profile_ids,
        baseline_policy: draft.baseline_policy,
        requested_checks: draft.requested_checks,
        check_overrides: draft.check_overrides,
        assumptions: draft.assumptions,
        created_by: actor,
        created_at: Utc::now(),
        content_fingerprint: empty_fingerprint(),
    }
    .seal()
    .map_err(PlanningError::from)
}

fn normalize_selectors(selectors: &mut Vec<PlanningSelector>) -> Result<(), PlanningError> {
    for selector in selectors.iter() {
        validate_selector(selector)?;
    }
    selectors.sort();
    selectors.dedup();
    Ok(())
}

fn validate_selector(selector: &PlanningSelector) -> Result<(), PlanningError> {
    if selector.value.trim().is_empty()
        || selector.value.contains('\0')
        || (selector.kind == SelectorKind::Path
            && ProjectPathRef::parse(selector.value.clone()).is_err())
    {
        return Err(PlanningError::TaskInput);
    }
    Ok(())
}

fn normalize_strings(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

fn task_ref(task: &TaskSpec) -> DocumentRef {
    document_ref(
        TASK_SPEC_SCHEMA_ID,
        task.task_spec_id.as_str(),
        task.revision,
        &task.content_fingerprint,
    )
}

fn scope_ref(scope: &ScopeRevision) -> DocumentRef {
    document_ref(
        SCOPE_REVISION_SCHEMA_ID,
        scope.scope_revision_id.as_str(),
        scope.revision,
        &scope.scope_hash,
    )
}

fn catalog_ref(catalog: &ProjectCatalogSnapshot) -> DocumentRef {
    document_ref(
        CATALOG_SCHEMA_ID,
        catalog.project_catalog_snapshot_id.as_str(),
        1,
        &catalog.content_fingerprint,
    )
}

fn build_scope_revision(
    task: &TaskSpec,
    catalog: &ProjectCatalogSnapshot,
    projects: &BTreeMap<star_contracts::ProjectId, &PlanningProjectIndex>,
    actor: ActorRef,
) -> Result<ScopeRevision, PlanningError> {
    if catalog.completeness != star_contracts::management::Completeness::Complete {
        return Err(PlanningError::SnapshotUnavailable);
    }
    let project_ids = task
        .project_targets
        .iter()
        .map(|target| target.project_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let user_selectors = task
        .included_scope
        .iter()
        .cloned()
        .map(|selector| ScopedSelector {
            selector,
            source: ScopeItemSource::User,
            reason_code: "TASK_EXPLICIT_INCLUDE".to_owned(),
            evidence_refs: vec![task.task_spec_id.to_string()],
        })
        .collect::<Vec<_>>();
    let scope = |exclusions: Vec<ExcludedScope>| ScopeSet {
        project_ids: project_ids.clone(),
        selectors: user_selectors.clone(),
        exclusions,
    };
    let exclusions_for = |axis: ScopeAxis| {
        task.excluded_scope
            .iter()
            .filter(|excluded| excluded.applies_to == axis || excluded.applies_to == ScopeAxis::All)
            .cloned()
            .collect::<Vec<_>>()
    };
    let source_snapshot_refs = task
        .project_targets
        .iter()
        .map(|target| {
            let project = projects
                .get(&target.project_id)
                .copied()
                .ok_or(PlanningError::SnapshotUnavailable)?;
            if project.snapshot.checkout_id != target.checkout_id
                || project.snapshot.project_catalog_snapshot_id
                    != catalog.project_catalog_snapshot_id
            {
                return Err(PlanningError::SnapshotUnavailable);
            }
            let freshness = projection_freshness(&project.snapshot);
            if freshness != IndexFreshnessState::Current {
                return Err(PlanningError::SnapshotUnavailable);
            }
            Ok(ScopeSourceSnapshotRef {
                project_id: target.project_id.clone(),
                checkout_id: target.checkout_id.clone(),
                project_catalog_snapshot_id: catalog.project_catalog_snapshot_id.clone(),
                project_revision_id: project.snapshot.project_revision_id.clone(),
                workspace_snapshot_id: project.snapshot.workspace_snapshot_id.clone(),
                code_index_snapshot_id: project.snapshot.code_index_snapshot_id.clone(),
                freshness,
            })
        })
        .collect::<Result<Vec<_>, PlanningError>>()?;
    ScopeRevision {
        schema_id: SCOPE_REVISION_SCHEMA_ID.to_owned(),
        schema_version: 1,
        scope_revision_id: ScopeRevisionId::new(),
        revision: 1,
        task_spec_ref: task_ref(task),
        previous_scope_revision_ref: None,
        reason_code: ScopeReasonCode::Initial,
        reason: "initial_user_scope".to_owned(),
        requested_scope: scope(task.excluded_scope.clone()),
        analysis_scope: scope(exclusions_for(ScopeAxis::Analysis)),
        planned_change_scope: scope(exclusions_for(ScopeAxis::PlannedChange)),
        validation_scope: scope(exclusions_for(ScopeAxis::Validation)),
        source_snapshot_refs,
        derived_additions: vec![],
        user_decisions: vec![],
        changed_fields: vec!["initial".to_owned()],
        approval_state: ScopeApprovalState::Accepted,
        scope_hash: empty_fingerprint(),
        created_by: actor,
        created_at: Utc::now(),
    }
    .seal()
    .map_err(PlanningError::from)
}

fn projection_freshness(snapshot: &CodeIndexSnapshot) -> IndexFreshnessState {
    if snapshot
        .partitions
        .iter()
        .filter(|partition| partition.required)
        .all(|partition| {
            matches!(
                partition.state,
                star_contracts::index::IndexPartitionState::Succeeded
                    | star_contracts::index::IndexPartitionState::Reused
            ) && snapshot.freshness.iter().any(|proof| {
                proof.partition_key == partition.partition_key
                    && proof.state == IndexFreshnessState::Current
            })
        })
    {
        IndexFreshnessState::Current
    } else {
        IndexFreshnessState::Partial
    }
}

fn build_change_set(
    task: &TaskSpec,
    task_ref: &DocumentRef,
    scope: &ScopeRevision,
    scope_ref: &DocumentRef,
    project: &PlanningProjectIndex,
) -> Result<ChangeSet, PlanningError> {
    let included_paths = task
        .included_scope
        .iter()
        .filter(|selector| selector.kind == SelectorKind::Path)
        .map(|selector| selector.value.as_str())
        .collect::<BTreeSet<_>>();
    let intended_paths = task
        .intended_changes
        .iter()
        .filter(|change| change.selector.kind == SelectorKind::Path)
        .map(|change| change.selector.value.as_str())
        .collect::<BTreeSet<_>>();
    let classes = project
        .source_entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry.source_class))
        .collect::<BTreeMap<_, _>>();
    let entries = project
        .observed_changes
        .iter()
        .map(|observed| {
            let path = observed.path.as_str();
            let scope_relation = if intended_paths.contains(path) || included_paths.contains(path) {
                ScopeRelation::Planned
            } else {
                ScopeRelation::Unknown
            };
            let entry_fingerprint = versioned_fingerprint(
                "star.change-entry",
                1,
                &serde_json::json!({
                    "project_id":project.snapshot.project_id,
                    "path":observed.path,
                    "rename_from":observed.rename_from,
                    "kind":observed.change_kind,
                    "before":observed.before_sha256,
                    "after":observed.after_sha256,
                }),
            )
            .map_err(|_| PlanningError::Fingerprint)?;
            Ok(ChangeEntry {
                entry_id: format!("che_{}", &entry_fingerprint.as_str()[7..39]),
                path: observed.path.clone(),
                rename_from: observed.rename_from.clone(),
                change_kind: observed.change_kind,
                before_sha256: observed.before_sha256.clone(),
                after_sha256: observed.after_sha256.clone(),
                staged: observed.staged,
                unstaged: observed.unstaged,
                untracked: observed.untracked,
                binary: observed.binary,
                source_class: classes.get(path).copied().unwrap_or(SourceClass::Unknown),
                origin: ChangeOrigin::Preexisting,
                scope_relation,
            })
        })
        .collect::<Result<Vec<_>, PlanningError>>()?;
    ChangeSet {
        schema_id: CHANGE_SET_SCHEMA_ID.to_owned(),
        schema_version: 1,
        change_set_id: ChangeSetId::new(),
        task_spec_ref: task_ref.clone(),
        scope_revision_ref: scope_ref.clone(),
        project_id: project.snapshot.project_id.clone(),
        checkout_id: project.snapshot.checkout_id.clone(),
        change_set_kind: ChangeSetKind::PlanningBaseline,
        base_revision_id: project.snapshot.project_revision_id.clone(),
        observed_workspace_snapshot_id: project.snapshot.workspace_snapshot_id.clone(),
        comparison_scope: scope
            .analysis_scope
            .selectors
            .iter()
            .map(|selector| selector.selector.clone())
            .collect(),
        entries,
        collection_limits: project.collection_limits.clone(),
        collection_state: project.collection_state,
        change_set_fingerprint: empty_fingerprint(),
        captured_at: Utc::now(),
    }
    .seal()
    .map_err(PlanningError::from)
}

#[allow(clippy::too_many_arguments)]
fn analyze_impact(
    task: &TaskSpec,
    task_ref: &DocumentRef,
    scope: &ScopeRevision,
    scope_ref: &DocumentRef,
    catalog: &ProjectCatalogSnapshot,
    projects: &[PlanningProjectIndex],
    change_sets: &[ChangeSet],
    risk_descriptors: &[RiskPathDescriptor],
    policy: &PlanningPolicy,
) -> Result<ImpactAnalysis, PlanningError> {
    let project_map = projects
        .iter()
        .map(|project| (project.snapshot.project_id.clone(), project))
        .collect::<BTreeMap<_, _>>();
    let mut seeds = Vec::new();
    for target in &task.project_targets {
        let project = project_map
            .get(&target.project_id)
            .copied()
            .ok_or(PlanningError::SnapshotUnavailable)?;
        for selector in task
            .included_scope
            .iter()
            .chain(task.intended_changes.iter().map(|change| &change.selector))
        {
            seeds.extend(map_selector_to_seeds(selector, project)?);
        }
        if let Some(changes) = change_sets
            .iter()
            .find(|changes| changes.project_id == target.project_id)
        {
            for entry in &changes.entries {
                if entry.scope_relation != ScopeRelation::Unrelated {
                    seeds.extend(map_selector_to_seeds(
                        &PlanningSelector {
                            kind: SelectorKind::Path,
                            value: entry.path.as_str().to_owned(),
                        },
                        project,
                    )?);
                }
            }
        }
    }
    seeds.sort_by(|left, right| left.seed_id.cmp(&right.seed_id));
    seeds.dedup_by(|left, right| left.seed_id == right.seed_id);
    if seeds.is_empty() {
        return Err(PlanningError::TaskInput);
    }

    let mut impacted_nodes = Vec::new();
    let mut impact_edges = Vec::new();
    let mut limitations = Vec::new();
    for target in &task.project_targets {
        let project = project_map[&target.project_id];
        traverse_project(
            project,
            seeds
                .iter()
                .filter(|seed| seed.project_id == target.project_id),
            policy,
            &mut impacted_nodes,
            &mut impact_edges,
            &mut limitations,
        )?;
    }
    let risk_paths = evaluate_risk_paths(
        &seeds,
        &impacted_nodes,
        &impact_edges,
        change_sets,
        risk_descriptors,
    )?;
    let no_results = seeds
        .iter()
        .filter(|seed| seed.resolution != SeedResolution::Resolved)
        .map(|seed| NoResult {
            query_kind: format!("{:?}", seed.selector.kind).to_ascii_lowercase(),
            reason: match seed.resolution {
                SeedResolution::Ambiguous => NoResultReason::Ambiguous,
                SeedResolution::Stale => NoResultReason::Stale,
                SeedResolution::Excluded => NoResultReason::ExcludedByPolicy,
                SeedResolution::Unresolved => NoResultReason::NoSeedMapping,
                SeedResolution::Resolved => NoResultReason::ConfirmedEmpty,
            },
            searched_scope: vec![seed.selector.clone()],
            required_tier: Some(IndexTier::Text),
            limitations: vec!["IMPACT_NO_SEED_MAPPING".to_owned()],
        })
        .collect::<Vec<_>>();
    if !no_results.is_empty() {
        limitations.push("IMPACT_NO_SEED_MAPPING".to_owned());
    }
    normalize_strings(&mut limitations);
    let confidence_summary = ImpactConfidenceSummary {
        confirmed: impacted_nodes
            .iter()
            .filter(|node| node.certainty == ImpactCertainty::Confirmed)
            .count() as u64,
        possible: impacted_nodes
            .iter()
            .filter(|node| node.certainty == ImpactCertainty::Possible)
            .count() as u64,
        high: impacted_nodes
            .iter()
            .filter(|node| node.confidence == ImpactConfidence::High)
            .count() as u64,
        medium: impacted_nodes
            .iter()
            .filter(|node| node.confidence == ImpactConfidence::Medium)
            .count() as u64,
        low: impacted_nodes
            .iter()
            .filter(|node| node.confidence == ImpactConfidence::Low)
            .count() as u64,
    };
    let affected_projects = task
        .project_targets
        .iter()
        .map(|target| {
            let nodes = impacted_nodes
                .iter()
                .filter(|node| node.project_id == target.project_id)
                .collect::<Vec<_>>();
            AffectedProject {
                project_id: target.project_id.clone(),
                certainty: if nodes
                    .iter()
                    .all(|node| node.certainty == ImpactCertainty::Confirmed)
                {
                    ImpactCertainty::Confirmed
                } else {
                    ImpactCertainty::Possible
                },
                closure_complete: !limitations
                    .iter()
                    .any(|limitation| limitation == "IMPACT_GRAPH_LIMIT"),
                impacted_node_count: nodes.len() as u64,
            }
        })
        .collect();
    let config_fingerprint = versioned_fingerprint("star.planning-policy", 1, policy)
        .map_err(|_| PlanningError::Fingerprint)?;
    let change_set_refs = change_sets
        .iter()
        .map(|changes| {
            document_ref(
                CHANGE_SET_SCHEMA_ID,
                changes.change_set_id.as_str(),
                1,
                &changes.change_set_fingerprint,
            )
        })
        .collect();
    let impact_status = if seeds
        .iter()
        .all(|seed| seed.resolution != SeedResolution::Resolved)
    {
        ImpactStatus::Blocked
    } else if !limitations.is_empty()
        || seeds
            .iter()
            .any(|seed| seed.resolution != SeedResolution::Resolved)
        || change_sets
            .iter()
            .any(|changes| changes.collection_state != CollectionState::Complete)
    {
        ImpactStatus::Partial
    } else {
        ImpactStatus::Complete
    };
    ImpactAnalysis {
        schema_id: IMPACT_ANALYSIS_SCHEMA_ID.to_owned(),
        schema_version: 1,
        impact_analysis_id: ImpactAnalysisId::new(),
        revision: 1,
        task_spec_ref: task_ref.clone(),
        scope_revision_ref: scope_ref.clone(),
        project_inputs: scope
            .source_snapshot_refs
            .iter()
            .map(|input| ImpactProjectInput {
                project_id: input.project_id.clone(),
                checkout_id: input.checkout_id.clone(),
                project_catalog_snapshot_id: input.project_catalog_snapshot_id.clone(),
                code_index_snapshot_id: input.code_index_snapshot_id.clone(),
                project_revision_id: input.project_revision_id.clone(),
                workspace_snapshot_id: input.workspace_snapshot_id.clone(),
                freshness: input.freshness,
            })
            .collect(),
        change_set_refs,
        catalog_snapshot_ref: catalog_ref(catalog),
        effective_config_fingerprint: config_fingerprint,
        seeds,
        impacted_nodes,
        impact_edges,
        risk_paths,
        affected_projects,
        no_results,
        limitations: limitations.clone(),
        confidence_summary,
        calculation_fingerprint: empty_fingerprint(),
        status: impact_status,
        generated_at: Utc::now(),
    }
    .seal()
    .map_err(PlanningError::from)
}

fn map_selector_to_seeds(
    selector: &PlanningSelector,
    project: &PlanningProjectIndex,
) -> Result<Vec<ImpactSeed>, PlanningError> {
    if selector.kind == SelectorKind::ManagedDeclaration {
        let Some(registry) = project.managed_registry_snapshot.as_ref() else {
            return Ok(vec![seed(
                project,
                selector,
                None,
                SeedResolution::Unresolved,
            )?]);
        };
        let declaration = registry
            .declarations
            .iter()
            .find(|declaration| declaration.managed_declaration_id.as_str() == selector.value);
        let owner_pin_current = registry.owner_project_id == project.snapshot.project_id
            && registry.project_revision_id == project.snapshot.project_revision_id
            && registry.workspace_snapshot_id == project.snapshot.workspace_snapshot_id
            && registry.code_index_snapshot_id.as_ref()
                == Some(&project.snapshot.code_index_snapshot_id);
        let consumer_pin_current = declaration.is_some_and(|declaration| {
            declaration
                .consumer_contracts
                .iter()
                .any(|contract| contract.project_id == project.snapshot.project_id)
                && registry
                    .consumers
                    .iter()
                    .filter(|consumer| {
                        consumer.declaration_id == declaration.managed_declaration_id
                            && consumer.project_id == project.snapshot.project_id
                    })
                    .all(|consumer| {
                        project.source_entries.iter().any(|source| {
                            source.path == consumer.path
                                && source.content_sha256 == consumer.source_sha256
                        })
                    })
        });
        if (!owner_pin_current && !consumer_pin_current)
            || registry.freshness != RegistryFreshness::Current
            || registry.completeness != EvidenceCompleteness::Complete
            || registry.resolution_state != RegistryResolutionState::Valid
        {
            return Ok(vec![seed(project, selector, None, SeedResolution::Stale)?]);
        }
    }
    let matched =
        match selector.kind {
            SelectorKind::Path => {
                let sources = project
                    .source_entries
                    .iter()
                    .filter(|source| path_selector_matches(&selector.value, source.path.as_str()))
                    .map(|source| source.canonical_source_id.as_str().to_owned())
                    .collect::<BTreeSet<_>>();
                project
                    .entities
                    .iter()
                    .filter(|entity| entity.kind == IndexEntityKind::Source)
                    .filter(|entity| {
                        entity
                            .canonical_source_id
                            .as_ref()
                            .is_some_and(|source| sources.contains(source.as_str()))
                    })
                    .map(|entity| entity.entity_key.clone())
                    .collect::<Vec<_>>()
            }
            SelectorKind::SourceClass => {
                let class = parse_source_class(&selector.value);
                let sources = project
                    .source_entries
                    .iter()
                    .filter(|source| Some(source.source_class) == class)
                    .map(|source| source.canonical_source_id.as_str().to_owned())
                    .collect::<BTreeSet<_>>();
                project
                    .entities
                    .iter()
                    .filter(|entity| {
                        entity
                            .canonical_source_id
                            .as_ref()
                            .is_some_and(|source| sources.contains(source.as_str()))
                    })
                    .map(|entity| entity.entity_key.clone())
                    .collect::<Vec<_>>()
            }
            SelectorKind::ManagedDeclaration => {
                let registry = project
                    .managed_registry_snapshot
                    .as_ref()
                    .expect("managed registry was checked above");
                let Some(declaration) = registry.declarations.iter().find(|declaration| {
                    declaration.managed_declaration_id.as_str() == selector.value
                }) else {
                    return Ok(vec![seed(
                        project,
                        selector,
                        None,
                        SeedResolution::Unresolved,
                    )?]);
                };
                let paths = if registry.owner_project_id == project.snapshot.project_id {
                    std::iter::once(&declaration.source_path)
                        .chain(
                            declaration
                                .binding_specs
                                .iter()
                                .map(|binding| &binding.path),
                        )
                        .chain(
                            registry
                                .consumers
                                .iter()
                                .filter(|consumer| {
                                    consumer.declaration_id == declaration.managed_declaration_id
                                        && consumer.project_id == project.snapshot.project_id
                                })
                                .map(|consumer| &consumer.path),
                        )
                        .collect::<BTreeSet<_>>()
                } else {
                    registry
                        .consumers
                        .iter()
                        .filter(|consumer| {
                            consumer.declaration_id == declaration.managed_declaration_id
                                && consumer.project_id == project.snapshot.project_id
                        })
                        .map(|consumer| &consumer.path)
                        .collect::<BTreeSet<_>>()
                };
                let source_ids = project
                    .source_entries
                    .iter()
                    .filter(|source| paths.contains(&source.path))
                    .map(|source| source.canonical_source_id.as_str())
                    .collect::<BTreeSet<_>>();
                let values = declaration
                    .primary_value
                    .iter()
                    .chain(declaration.aliases.iter().map(|alias| &alias.value))
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>();
                project
                    .entities
                    .iter()
                    .filter(|entity| {
                        entity
                            .canonical_source_id
                            .as_ref()
                            .is_some_and(|source| source_ids.contains(source.as_str()))
                            || values.contains(entity.qualified_name.as_str())
                    })
                    .map(|entity| entity.entity_key.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
            }
            _ => project
                .entities
                .iter()
                .filter(|entity| selector_entity_kind_matches(selector.kind, entity.kind))
                .filter(|entity| {
                    entity.entity_key == selector.value || entity.qualified_name == selector.value
                })
                .map(|entity| entity.entity_key.clone())
                .collect::<Vec<_>>(),
        };
    let resolution = if selector.kind == SelectorKind::ManagedDeclaration && !matched.is_empty() {
        SeedResolution::Resolved
    } else {
        match matched.len() {
            0 => SeedResolution::Unresolved,
            1 => SeedResolution::Resolved,
            _ => SeedResolution::Ambiguous,
        }
    };
    if matched.is_empty() {
        return Ok(vec![seed(project, selector, None, resolution)?]);
    }
    matched
        .into_iter()
        .map(|entity_key| seed(project, selector, Some(entity_key), resolution))
        .collect()
}

fn seed(
    project: &PlanningProjectIndex,
    selector: &PlanningSelector,
    entity_key: Option<String>,
    resolution: SeedResolution,
) -> Result<ImpactSeed, PlanningError> {
    let fingerprint = versioned_fingerprint(
        "star.impact-seed",
        1,
        &serde_json::json!({
            "project_id":project.snapshot.project_id,
            "selector":selector,
            "entity_key":entity_key,
            "resolution":resolution,
        }),
    )
    .map_err(|_| PlanningError::Fingerprint)?;
    Ok(ImpactSeed {
        seed_id: format!("isd_{}", &fingerprint.as_str()[7..39]),
        project_id: project.snapshot.project_id.clone(),
        selector: selector.clone(),
        entity_key,
        resolution,
        evidence_refs: vec![project.snapshot.code_index_snapshot_id.to_string()],
    })
}

fn path_selector_matches(selector: &str, path: &str) -> bool {
    selector == path
        || selector
            .strip_suffix("/**")
            .is_some_and(|prefix| path == prefix || path.starts_with(&format!("{prefix}/")))
}

fn parse_source_class(value: &str) -> Option<SourceClass> {
    match value {
        "source" => Some(SourceClass::Source),
        "test" => Some(SourceClass::Test),
        "docs" => Some(SourceClass::Docs),
        "config" => Some(SourceClass::Config),
        "schema" => Some(SourceClass::Schema),
        "migration" => Some(SourceClass::Migration),
        "generated" => Some(SourceClass::Generated),
        "vendor" => Some(SourceClass::Vendor),
        "cache" => Some(SourceClass::Cache),
        "output" => Some(SourceClass::Output),
        "unknown" => Some(SourceClass::Unknown),
        _ => None,
    }
}

fn selector_entity_kind_matches(selector: SelectorKind, kind: IndexEntityKind) -> bool {
    matches!(
        (selector, kind),
        (SelectorKind::Package, IndexEntityKind::Package)
            | (SelectorKind::Workspace, IndexEntityKind::Workspace)
            | (SelectorKind::Symbol, IndexEntityKind::Symbol)
            | (SelectorKind::Contract, IndexEntityKind::Contract)
            | (SelectorKind::ConfigKey, IndexEntityKind::ConfigKey)
            | (SelectorKind::Schema, IndexEntityKind::SchemaId)
    )
}

fn traverse_project<'a>(
    project: &PlanningProjectIndex,
    seeds: impl Iterator<Item = &'a ImpactSeed>,
    policy: &PlanningPolicy,
    impacted_nodes: &mut Vec<ImpactedNode>,
    impact_edges: &mut Vec<ImpactEdge>,
    limitations: &mut Vec<String>,
) -> Result<(), PlanningError> {
    let entities = project
        .entities
        .iter()
        .map(|entity| (entity.entity_key.as_str(), entity))
        .collect::<BTreeMap<_, _>>();
    let mut adjacency: BTreeMap<&str, Vec<(&IndexEdge, &str)>> = BTreeMap::new();
    for edge in &project.edges {
        let Some(target) = edge.to_entity_key.as_deref() else {
            continue;
        };
        match edge.relation {
            IndexRelation::References
            | IndexRelation::Imports
            | IndexRelation::DependsOn
            | IndexRelation::Tests
            | IndexRelation::Documents => adjacency
                .entry(target)
                .or_default()
                .push((edge, edge.from_entity_key.as_str())),
            _ => {
                adjacency
                    .entry(edge.from_entity_key.as_str())
                    .or_default()
                    .push((edge, target));
                adjacency
                    .entry(target)
                    .or_default()
                    .push((edge, edge.from_entity_key.as_str()));
            }
        }
    }
    for edges in adjacency.values_mut() {
        edges.sort_by(|left, right| {
            (left.0.edge_key.as_str(), left.1).cmp(&(right.0.edge_key.as_str(), right.1))
        });
    }
    for seed in seeds.filter(|seed| seed.resolution == SeedResolution::Resolved) {
        let Some(start) = seed.entity_key.as_deref() else {
            continue;
        };
        let mut queue = VecDeque::from([(start, 0_u32, Vec::<String>::new())]);
        let mut visited = BTreeSet::new();
        while let Some((node_key, distance, path)) = queue.pop_front() {
            if !visited.insert(node_key.to_owned()) {
                continue;
            }
            if visited.len() > policy.max_nodes || impact_edges.len() > policy.max_edges {
                limitations.push("IMPACT_GRAPH_LIMIT".to_owned());
                break;
            }
            let Some(entity) = entities.get(node_key).copied() else {
                continue;
            };
            let certainty = if distance == 0 || entity.tier >= IndexTier::Syntax {
                ImpactCertainty::Confirmed
            } else {
                ImpactCertainty::Possible
            };
            impacted_nodes.push(ImpactedNode {
                project_id: project.snapshot.project_id.clone(),
                entity_key: node_key.to_owned(),
                kind: format!("{:?}", entity.kind).to_ascii_lowercase(),
                impact_kind: if distance <= 1 {
                    ImpactKind::Direct
                } else {
                    ImpactKind::Transitive
                },
                certainty,
                confidence: if certainty == ImpactCertainty::Confirmed {
                    ImpactConfidence::High
                } else {
                    ImpactConfidence::Low
                },
                minimum_distance: distance,
            });
            if distance >= policy.max_depth {
                if adjacency.contains_key(node_key) {
                    limitations.push("IMPACT_GRAPH_LIMIT".to_owned());
                }
                continue;
            }
            for (edge, next) in adjacency.get(node_key).into_iter().flatten() {
                let edge_result = impact_edge(project, edge, node_key, next, distance + 1, &path)?;
                let mut next_path = path.clone();
                next_path.push(edge_result.edge_id.clone());
                impact_edges.push(edge_result);
                if !visited.contains(*next) {
                    queue.push_back((next, distance + 1, next_path));
                }
            }
        }
    }
    impacted_nodes.sort_by(|left, right| {
        (&left.project_id, &left.entity_key, left.minimum_distance).cmp(&(
            &right.project_id,
            &right.entity_key,
            right.minimum_distance,
        ))
    });
    impacted_nodes.dedup_by(|left, right| {
        left.project_id == right.project_id && left.entity_key == right.entity_key
    });
    impact_edges.sort_by(|left, right| left.edge_id.cmp(&right.edge_id));
    impact_edges.dedup_by(|left, right| left.edge_id == right.edge_id);
    Ok(())
}

fn impact_edge(
    project: &PlanningProjectIndex,
    edge: &IndexEdge,
    from: &str,
    to: &str,
    distance: u32,
    path: &[String],
) -> Result<ImpactEdge, PlanningError> {
    let current = projection_freshness(&project.snapshot) == IndexFreshnessState::Current;
    let resolved = edge.resolution == SymbolResolution::Resolved;
    let certainty = if current && resolved && edge.tier >= IndexTier::Syntax {
        ImpactCertainty::Confirmed
    } else {
        ImpactCertainty::Possible
    };
    let confidence = if certainty == ImpactCertainty::Confirmed
        && edge.confidence.eq_ignore_ascii_case("high")
    {
        ImpactConfidence::High
    } else if certainty == ImpactCertainty::Confirmed {
        ImpactConfidence::Medium
    } else {
        ImpactConfidence::Low
    };
    let content_fingerprint = versioned_fingerprint(
        "star.impact-edge",
        1,
        &serde_json::json!({
            "project_id":project.snapshot.project_id,
            "from":from,
            "to":to,
            "relation":edge.relation,
            "distance":distance,
            "certainty":certainty,
            "source_edge":edge.content_fingerprint,
        }),
    )
    .map_err(|_| PlanningError::Fingerprint)?;
    Ok(ImpactEdge {
        edge_id: format!("ied_{}", &content_fingerprint.as_str()[7..39]),
        project_id: project.snapshot.project_id.clone(),
        from_entity_key: from.to_owned(),
        to_entity_key: to.to_owned(),
        relation: format!("{:?}", edge.relation).to_ascii_lowercase(),
        impact_kind: if distance <= 1 {
            ImpactKind::Direct
        } else {
            ImpactKind::Transitive
        },
        distance,
        certainty,
        confidence,
        resolution: match edge.resolution {
            SymbolResolution::Resolved => ImpactResolution::Resolved,
            SymbolResolution::Ambiguous => ImpactResolution::Ambiguous,
            SymbolResolution::Unresolved => ImpactResolution::Unresolved,
            SymbolResolution::External => ImpactResolution::External,
        },
        tier: edge.tier,
        freshness: projection_freshness(&project.snapshot),
        evidence_refs: vec![edge.edge_key.clone()],
        path_edge_ids: path.to_vec(),
        limitations: if certainty == ImpactCertainty::Possible {
            vec!["IMPACT_TIER_FALLBACK".to_owned()]
        } else {
            vec![]
        },
        content_fingerprint,
    })
}

fn evaluate_risk_paths(
    seeds: &[ImpactSeed],
    nodes: &[ImpactedNode],
    edges: &[ImpactEdge],
    change_sets: &[ChangeSet],
    descriptors: &[RiskPathDescriptor],
) -> Result<Vec<RiskPathFinding>, PlanningError> {
    let changed_classes = change_sets
        .iter()
        .flat_map(|changes| changes.entries.iter().map(|entry| entry.source_class))
        .collect::<BTreeSet<_>>();
    let node_kinds = nodes
        .iter()
        .map(|node| node.kind.as_str())
        .collect::<BTreeSet<_>>();
    let mut findings = Vec::new();
    for descriptor in descriptors {
        let matched_seeds = seeds
            .iter()
            .filter(|seed| descriptor.selector_kinds.contains(&seed.selector.kind))
            .map(|seed| seed.seed_id.clone())
            .collect::<Vec<_>>();
        let matched = !matched_seeds.is_empty()
            || descriptor
                .source_classes
                .iter()
                .any(|class| changed_classes.contains(class))
            || descriptor
                .entity_kinds
                .iter()
                .any(|kind| node_kinds.contains(kind.as_str()));
        if !matched {
            continue;
        }
        let complete_collection = change_sets
            .iter()
            .all(|changes| changes.collection_state == CollectionState::Complete);
        let certainty = if complete_collection
            && matched_seeds.iter().all(|seed_id| {
                seeds.iter().any(|seed| {
                    &seed.seed_id == seed_id && seed.resolution == SeedResolution::Resolved
                })
            }) {
            ImpactCertainty::Confirmed
        } else {
            ImpactCertainty::Possible
        };
        let fingerprint = versioned_fingerprint(
            "star.risk-path-finding",
            1,
            &serde_json::json!({
                "risk_id":descriptor.risk_id,
                "version":descriptor.version,
                "seeds":matched_seeds,
                "certainty":certainty,
            }),
        )
        .map_err(|_| PlanningError::Fingerprint)?;
        let project_id = seeds
            .iter()
            .find(|seed| matched_seeds.contains(&seed.seed_id))
            .or_else(|| seeds.first())
            .ok_or(PlanningError::TaskInput)?
            .project_id
            .clone();
        findings.push(RiskPathFinding {
            finding_id: format!("rpf_{}", &fingerprint.as_str()[7..39]),
            risk_id: descriptor.risk_id.clone(),
            risk_version: descriptor.version.clone(),
            project_id: project_id.clone(),
            seed_ids: matched_seeds,
            impact_edge_ids: edges
                .iter()
                .filter(|edge| edge.project_id == project_id)
                .map(|edge| edge.edge_id.clone())
                .collect(),
            certainty,
            severity_floor: descriptor.severity_floor,
            required_check_families: descriptor.required_check_families.clone(),
            fallback_floor: descriptor.fallback_floor,
            limitations: if certainty == ImpactCertainty::Possible {
                vec!["RISK_PATH_POSSIBLE".to_owned()]
            } else {
                vec![]
            },
        });
    }
    findings.sort_by(|left, right| left.finding_id.cmp(&right.finding_id));
    Ok(findings)
}

#[allow(clippy::too_many_arguments)]
fn select_validation_plan(
    task: &TaskSpec,
    task_ref: &DocumentRef,
    scope: &ScopeRevision,
    scope_ref: &DocumentRef,
    catalog: &ProjectCatalogSnapshot,
    change_sets: &[ChangeSet],
    impact: &ImpactAnalysis,
    descriptors: &[CheckDescriptor],
    previous_success_evidence: &[PreviousSuccessEvidence],
    profile_resolution: Option<&DevelopmentProfileResolutionV1>,
    policy: &PlanningPolicy,
    validation_phase: &str,
) -> Result<FullValidationPlan, PlanningError> {
    let patch_pre_apply = validation_phase == "patch_pre_apply";
    let mut required_families = task
        .requested_checks
        .iter()
        .filter(|family| !patch_pre_apply || patch_pre_apply_check_family(family))
        .cloned()
        .collect::<BTreeSet<_>>();
    if let Some(resolution) = profile_resolution {
        required_families.extend(resolution.required_check_families.iter().cloned());
    }
    for changes in change_sets {
        for entry in &changes.entries {
            if !patch_pre_apply {
                required_families.extend(families_for_source_class(entry.source_class));
            }
            if validation_protected_path(entry.path.as_str()) {
                required_families.insert("validator_guard".to_owned());
            }
            if !patch_pre_apply && environment_sensitive_path(entry.path.as_str()) {
                required_families.insert("project_full".to_owned());
            }
        }
    }
    if !patch_pre_apply && task_requests_bug_fix(task) {
        required_families.insert("regression".to_owned());
        required_families.insert("test".to_owned());
    }
    for risk in &impact.risk_paths {
        required_families.extend(
            risk.required_check_families
                .iter()
                .filter(|family| !patch_pre_apply || patch_pre_apply_check_family(family))
                .cloned(),
        );
    }
    if required_families.is_empty() {
        let target_projects = task
            .project_targets
            .iter()
            .filter(|target| target.role != ProjectTargetRole::ReadOnlyImpact)
            .map(|target| &target.project_id)
            .collect::<BTreeSet<_>>();
        required_families.extend(
            descriptors
                .iter()
                .filter(|descriptor| {
                    !descriptor.project_ids.is_empty()
                        && descriptor
                            .project_ids
                            .iter()
                            .any(|project_id| target_projects.contains(project_id))
                })
                .map(|descriptor| descriptor.family.clone()),
        );
    }
    if required_families.is_empty() {
        required_families.insert("project_full".to_owned());
    }
    let mut optional_families = BTreeSet::new();
    if let Some(resolution) = profile_resolution {
        optional_families.extend(resolution.optional_check_families.iter().cloned());
    }
    let mut waived_families = BTreeMap::new();
    for override_item in &task.check_overrides {
        match override_item.kind {
            CheckOverrideKind::Add => {
                optional_families.insert(override_item.family.clone());
            }
            CheckOverrideKind::Promote => {
                required_families.insert(override_item.family.clone());
            }
            CheckOverrideKind::Omit => {
                waived_families.insert(override_item.family.clone(), override_item.reason.clone());
            }
        }
    }
    optional_families.retain(|family| !required_families.contains(family));
    let mut families = required_families
        .union(&optional_families)
        .cloned()
        .collect::<BTreeSet<_>>();
    families.extend(waived_families.keys().cloned());
    if families.len() > policy.max_check_candidates {
        return Err(PlanningError::ResourceLimit);
    }
    let mut candidate_checks = Vec::new();
    let mut required_checks = Vec::new();
    let mut optional_checks = Vec::new();
    let mut omitted_checks = Vec::new();
    let mut unresolved_checks = Vec::new();
    let mut affected_scope = Vec::new();
    let mut fallback_decisions = Vec::new();
    for target in task
        .project_targets
        .iter()
        .filter(|target| target.role != ProjectTargetRole::ReadOnlyImpact)
    {
        let (selected_level, floor_reason) =
            validation_scope_for_project(target, impact, change_sets);
        affected_scope.push(AffectedScope {
            project_id: target.project_id.clone(),
            requested_level: ValidationScopeLevel::Package,
            selected_level,
            selectors: task.included_scope.clone(),
            reason_codes: floor_reason.clone(),
            limitations: impact.limitations.clone(),
        });
        if selected_level != ValidationScopeLevel::Package {
            fallback_decisions.push(FallbackDecision {
                project_id: target.project_id.clone(),
                from_level: ValidationScopeLevel::Package,
                to_level: selected_level,
                trigger: floor_reason.join("+"),
                evidence_refs: vec![impact.impact_analysis_id.to_string()],
            });
        }
        let changed_classes = change_sets
            .iter()
            .filter(|changes| changes.project_id == target.project_id)
            .flat_map(|changes| changes.entries.iter().map(|entry| entry.source_class))
            .collect::<BTreeSet<_>>();
        let risk_required_families = impact
            .risk_paths
            .iter()
            .filter(|risk| risk.project_id == target.project_id)
            .flat_map(|risk| risk.required_check_families.iter().cloned())
            .collect::<BTreeSet<_>>();
        for family in &families {
            let required = required_families.contains(family);
            let descriptor = descriptors
                .iter()
                .filter(|descriptor| descriptor.family == *family)
                .find(|descriptor| descriptor.project_ids.contains(&target.project_id))
                .or_else(|| {
                    descriptors.iter().find(|descriptor| {
                        descriptor.family == *family && descriptor.project_ids.is_empty()
                    })
                });
            if let Some(reason) = waived_families.get(family) {
                candidate_checks.push(CheckCandidate {
                    family: family.clone(),
                    check_id: descriptor.map(|value| value.check_id.clone()),
                    applicability: if descriptor.is_some() {
                        CheckApplicability::Applicable
                    } else {
                        CheckApplicability::Unknown
                    },
                    outcome: CheckResolutionOutcome::UserWaived,
                    evidence_refs: descriptor
                        .map(|value| vec![value.content_fingerprint.to_string()])
                        .unwrap_or_default(),
                    reason_code: "USER_CHECK_WAIVED".to_owned(),
                });
                omitted_checks.push(format!(
                    "{}:{family}:user_waived:{reason}",
                    target.project_id
                ));
                continue;
            }
            let Some(descriptor) = descriptor else {
                candidate_checks.push(CheckCandidate {
                    family: family.clone(),
                    check_id: None,
                    applicability: CheckApplicability::Unknown,
                    outcome: CheckResolutionOutcome::UnresolvedNotFound,
                    evidence_refs: vec![],
                    reason_code: "AFFECTED_CHECK_NOT_FOUND".to_owned(),
                });
                if required {
                    unresolved_checks.push(star_contracts::planning::UnresolvedCheck {
                        family: family.clone(),
                        reason: "descriptor_not_found".to_owned(),
                        searched_catalog_scope: catalog.project_catalog_snapshot_id.to_string(),
                        required_coverage: format!("{:?}", selected_level).to_ascii_lowercase(),
                        readiness_impact: ValidationPlanV2Readiness::Blocked,
                    });
                }
                continue;
            };
            let forced = task.requested_checks.contains(family)
                || family == "project_full"
                || descriptor.project_ids.contains(&target.project_id)
                || task.check_overrides.iter().any(|override_item| {
                    override_item.family == *family
                        && override_item.kind == CheckOverrideKind::Promote
                })
                || risk_required_families.contains(family);
            let applicable = descriptor.applicable_source_classes.is_empty()
                || descriptor
                    .applicable_source_classes
                    .iter()
                    .any(|class| changed_classes.contains(class));
            if !applicable && !forced {
                candidate_checks.push(CheckCandidate {
                    family: family.clone(),
                    check_id: Some(descriptor.check_id.clone()),
                    applicability: CheckApplicability::NotApplicable,
                    outcome: CheckResolutionOutcome::OmittedNotApplicable,
                    evidence_refs: vec![descriptor.content_fingerprint.to_string()],
                    reason_code: "CHECK_NOT_APPLICABLE_TO_AFFECTED_SOURCE".to_owned(),
                });
                omitted_checks.push(format!("{}:{family}:not_applicable", target.project_id));
                continue;
            }
            if !descriptor.trusted || !descriptor.available {
                candidate_checks.push(CheckCandidate {
                    family: family.clone(),
                    check_id: Some(descriptor.check_id.clone()),
                    applicability: CheckApplicability::Applicable,
                    outcome: CheckResolutionOutcome::BlockedUnavailable,
                    evidence_refs: vec![descriptor.content_fingerprint.to_string()],
                    reason_code: "AFFECTED_CHECK_UNAVAILABLE".to_owned(),
                });
                if required {
                    unresolved_checks.push(star_contracts::planning::UnresolvedCheck {
                        family: family.clone(),
                        reason: if descriptor.trusted {
                            "tool_unavailable".to_owned()
                        } else {
                            "untrusted".to_owned()
                        },
                        searched_catalog_scope: catalog.project_catalog_snapshot_id.to_string(),
                        required_coverage: format!("{:?}", selected_level).to_ascii_lowercase(),
                        readiness_impact: ValidationPlanV2Readiness::Blocked,
                    });
                }
                continue;
            }
            let bound_level = if descriptor.supported_scope_levels.contains(&selected_level) {
                selected_level
            } else if descriptor
                .supported_scope_levels
                .contains(&ValidationScopeLevel::ProjectFull)
            {
                ValidationScopeLevel::ProjectFull
            } else {
                candidate_checks.push(CheckCandidate {
                    family: family.clone(),
                    check_id: Some(descriptor.check_id.clone()),
                    applicability: CheckApplicability::Applicable,
                    outcome: CheckResolutionOutcome::BlockedUnavailable,
                    evidence_refs: vec![descriptor.content_fingerprint.to_string()],
                    reason_code: "AFFECTED_SCOPE_UNBINDABLE".to_owned(),
                });
                if required {
                    unresolved_checks.push(star_contracts::planning::UnresolvedCheck {
                        family: family.clone(),
                        reason: "scope_unbindable".to_owned(),
                        searched_catalog_scope: catalog.project_catalog_snapshot_id.to_string(),
                        required_coverage: format!("{:?}", selected_level).to_ascii_lowercase(),
                        readiness_impact: ValidationPlanV2Readiness::Blocked,
                    });
                }
                continue;
            };
            let outcome = if required {
                CheckResolutionOutcome::SelectedRequired
            } else {
                CheckResolutionOutcome::SelectedOptional
            };
            candidate_checks.push(CheckCandidate {
                family: family.clone(),
                check_id: Some(descriptor.check_id.clone()),
                applicability: CheckApplicability::Applicable,
                outcome,
                evidence_refs: vec![descriptor.content_fingerprint.to_string()],
                reason_code: "AFFECTED_CHECK_SELECTED".to_owned(),
            });
            let item_fingerprint = versioned_fingerprint(
                "star.check-plan-item",
                1,
                &serde_json::json!({
                    "project_id":target.project_id,
                    "check_id":descriptor.check_id,
                    "scope":bound_level,
                }),
            )
            .map_err(|_| PlanningError::Fingerprint)?;
            let planned_check = CheckPlanV2 {
                plan_item_id: format!("cpi_{}", &item_fingerprint.as_str()[7..39]),
                check_id: descriptor.check_id.clone(),
                descriptor_ref: document_ref(
                    "star.check-descriptor",
                    &descriptor.check_id,
                    1,
                    &descriptor.content_fingerprint,
                ),
                tool_id: descriptor.tool_id.clone(),
                family: family.clone(),
                project_id: target.project_id.clone(),
                scope_level: bound_level,
                outcome,
                reason_codes: vec![if required {
                    "AFFECTED_CHECK_SELECTED".to_owned()
                } else {
                    "USER_OPTIONAL_CHECK_SELECTED".to_owned()
                }],
                impact_edge_ids: impact
                    .impact_edges
                    .iter()
                    .filter(|edge| edge.project_id == target.project_id)
                    .map(|edge| edge.edge_id.clone())
                    .collect(),
                risk_path_ids: impact
                    .risk_paths
                    .iter()
                    .filter(|risk| risk.project_id == target.project_id)
                    .map(|risk| risk.finding_id.clone())
                    .collect(),
                invocation: CheckInvocationTemplate {
                    logical_executable: descriptor.logical_executable.clone(),
                    args: bind_scope_arguments(&descriptor.argument_template, bound_level),
                    timeout_ms: 3_600_000,
                    expected_exit_codes: vec![0],
                },
                fallback_floor: selected_level,
                evidence_kinds: descriptor.required_evidence.clone(),
            };
            if required {
                required_checks.push(planned_check);
            } else {
                optional_checks.push(planned_check);
            }
        }
    }
    let readiness = if unresolved_checks.is_empty()
        && !required_checks.is_empty()
        && impact.status == ImpactStatus::Complete
        && change_sets
            .iter()
            .all(|changes| changes.collection_state == CollectionState::Complete)
    {
        ValidationPlanV2Readiness::Ready
    } else {
        ValidationPlanV2Readiness::Blocked
    };
    let nodes = required_checks
        .iter()
        .map(|check| check.plan_item_id.clone())
        .collect::<Vec<_>>();
    let risk_level = impact
        .risk_paths
        .iter()
        .map(|risk| risk.severity_floor)
        .max()
        .map(risk_level)
        .unwrap_or(ValidationRiskLevel::Low);
    let config_fingerprint = versioned_fingerprint("star.planning-policy", 1, policy)
        .map_err(|_| PlanningError::Fingerprint)?;
    let impact_ref = document_ref(
        IMPACT_ANALYSIS_SCHEMA_ID,
        impact.impact_analysis_id.as_str(),
        impact.revision,
        &impact.calculation_fingerprint,
    );
    let change_set_refs = change_sets
        .iter()
        .map(|changes| {
            document_ref(
                CHANGE_SET_SCHEMA_ID,
                changes.change_set_id.as_str(),
                1,
                &changes.change_set_fingerprint,
            )
        })
        .collect();
    let mut plan = FullValidationPlan {
        schema_id: FULL_VALIDATION_PLAN_SCHEMA_ID.to_owned(),
        schema_version: 2,
        validation_plan_id: ValidationPlanId::new(),
        revision: 1,
        task_spec_ref: task_ref.clone(),
        scope_revision: scope.revision,
        scope_revision_ref: scope_ref.clone(),
        phase: validation_phase.to_owned(),
        change_set_refs,
        impact_analysis_ref: impact_ref,
        risk_level,
        affected_scope,
        candidate_checks,
        required_checks,
        optional_checks,
        check_graph: CheckGraphV2 {
            nodes,
            edges: vec![],
            max_parallel: policy.max_parallel_checks,
            failure_policy: "stop_dependents_continue_independent".to_owned(),
        },
        omitted_checks,
        unresolved_checks,
        previous_success_comparisons: vec![],
        fallback_decisions,
        manual_observations: task
            .check_overrides
            .iter()
            .map(|override_item| {
                format!(
                    "check_override:{}:{:?}:{}",
                    override_item.family, override_item.kind, override_item.reason
                )
            })
            .collect(),
        independent_review: ReviewRequirementV2 {
            required: task.check_overrides.iter().any(|override_item| {
                override_item.kind == star_contracts::planning::CheckOverrideKind::Omit
            }) || profile_resolution.is_some_and(|resolution| {
                resolution.review_policy.cli_only >= ProfileReviewFloorV1::HumanSemantic
            }),
            review_kind: if task.check_overrides.iter().any(|override_item| {
                override_item.kind == star_contracts::planning::CheckOverrideKind::Omit
            }) || profile_resolution.is_some_and(|resolution| {
                resolution.review_policy.cli_only >= ProfileReviewFloorV1::HumanSemantic
            }) {
                ReviewKind::HumanSemantic
            } else {
                ReviewKind::None
            },
            reason_codes: task
                .check_overrides
                .iter()
                .filter(|override_item| {
                    override_item.kind == star_contracts::planning::CheckOverrideKind::Omit
                })
                .map(|_| "USER_WAIVER_REVIEW".to_owned())
                .chain(
                    profile_resolution
                        .filter(|resolution| {
                            resolution.review_policy.cli_only >= ProfileReviewFloorV1::HumanSemantic
                        })
                        .map(|_| "PROFILE_REVIEW_FLOOR".to_owned()),
                )
                .collect(),
            absence_behavior: "human_review".to_owned(),
        },
        gate_policy: GatePolicyV2 {
            fail_on_required_failure: true,
            fail_on_partial: true,
            fail_on_unverified: true,
            fail_on_flaky: true,
        },
        config_fingerprint,
        catalog_snapshot_ref: catalog_ref(catalog),
        profile_resolution: profile_resolution.cloned(),
        selection_fingerprint: empty_fingerprint(),
        readiness,
    }
    .seal()?;
    plan.previous_success_comparisons =
        evaluate_previous_success_reuse(&plan, scope, previous_success_evidence)?;
    plan.seal().map_err(PlanningError::from)
}

fn evaluate_previous_success_reuse(
    current: &FullValidationPlan,
    scope: &ScopeRevision,
    candidates: &[PreviousSuccessEvidence],
) -> Result<Vec<String>, PlanningError> {
    let mut comparisons = Vec::new();
    for candidate in candidates {
        let current_source = scope
            .source_snapshot_refs
            .iter()
            .find(|source| source.project_id == candidate.project_id);
        let previous_source = candidate
            .source_snapshot_refs
            .iter()
            .find(|source| source.project_id == candidate.project_id);
        let source_matches = current_source.is_some() && current_source == previous_source;
        let previous_ready =
            candidate.validation_plan.readiness == ValidationPlanV2Readiness::Ready;
        let current_signature = validation_reuse_signature(current, &candidate.project_id)?;
        let previous_signature =
            validation_reuse_signature(&candidate.validation_plan, &candidate.project_id)?;
        let disposition =
            if source_matches && previous_ready && current_signature == previous_signature {
                "reusable"
            } else if !source_matches {
                "not_reusable_source_snapshot_changed"
            } else if !previous_ready {
                "not_reusable_previous_plan_not_ready"
            } else {
                "not_reusable_selection_changed"
            };
        comparisons.push(format!(
            "{disposition}:{}:{}",
            candidate.evidence_bundle_id, candidate.bundle_fingerprint
        ));
    }
    comparisons.sort();
    comparisons.dedup();
    Ok(comparisons)
}

fn validation_reuse_signature(
    plan: &FullValidationPlan,
    project_id: &ProjectId,
) -> Result<Sha256Hash, PlanningError> {
    let required_checks = plan
        .required_checks
        .iter()
        .filter(|check| &check.project_id == project_id)
        .map(|check| {
            serde_json::json!({
                "check_id":check.check_id,
                "descriptor_ref":check.descriptor_ref,
                "tool_id":check.tool_id,
                "family":check.family,
                "scope_level":check.scope_level,
                "invocation":check.invocation,
                "fallback_floor":check.fallback_floor,
                "evidence_kinds":check.evidence_kinds,
            })
        })
        .collect::<Vec<_>>();
    let affected_scope = plan
        .affected_scope
        .iter()
        .filter(|scope| &scope.project_id == project_id)
        .collect::<Vec<_>>();
    let omitted_prefix = format!("{project_id}:");
    let omitted_checks = plan
        .omitted_checks
        .iter()
        .filter(|check| check.starts_with(&omitted_prefix))
        .collect::<Vec<_>>();
    versioned_fingerprint(
        "star.previous-success-reuse",
        1,
        &serde_json::json!({
            "project_id":project_id,
            "required_checks":required_checks,
            "affected_scope":affected_scope,
            "candidate_checks":plan.candidate_checks,
            "omitted_checks":omitted_checks,
            "independent_review":plan.independent_review,
            "config_fingerprint":plan.config_fingerprint,
            "gate_policy":plan.gate_policy,
        }),
    )
    .map_err(|_| PlanningError::Fingerprint)
}

fn families_for_source_class(class: SourceClass) -> BTreeSet<String> {
    let families: &[&str] = match class {
        SourceClass::Source => &[
            "format",
            "lint",
            "build",
            "test",
            "architecture",
            "hardcoding",
            "security",
        ],
        SourceClass::Test => &["test", "validator_guard", "security"],
        SourceClass::Docs => &["docs", "security"],
        SourceClass::Config => &["config", "test", "dependency", "security"],
        SourceClass::Schema => &["contract", "test", "architecture", "validator_guard"],
        SourceClass::Migration => &["migration", "test", "regression", "security"],
        SourceClass::Generated => &["generation", "build", "test", "architecture"],
        SourceClass::Vendor | SourceClass::Cache | SourceClass::Output => &[],
        SourceClass::Unknown => &["project_full"],
    };
    families.iter().map(|family| (*family).to_owned()).collect()
}

fn patch_pre_apply_check_family(family: &str) -> bool {
    matches!(
        family,
        "architecture" | "hardcoding" | "security" | "validator_guard"
    )
}

fn task_requests_bug_fix(task: &TaskSpec) -> bool {
    let text = format!("{} {}", task.title, task.objective).to_ascii_lowercase();
    let words = text
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .collect::<BTreeSet<_>>();
    ["bug", "fix", "bugfix", "regression", "defect"]
        .iter()
        .any(|token| words.contains(token))
        || ["버그", "오류", "회귀", "결함"]
            .iter()
            .any(|token| text.contains(token))
}

fn validation_protected_path(path: &str) -> bool {
    path == "scripts/validate.ps1"
        || path.starts_with("scripts/validation/")
        || path.starts_with("crates/control/star-validation/")
        || path.starts_with("crates/foundation/star-contracts/src/evidence")
        || path.starts_with("specs/schemas/v1/validation-")
        || path.starts_with("specs/schemas/v1/gate-")
        || path.starts_with("specs/fixtures/management/v1/validation-")
        || path.starts_with("specs/fixtures/management/v1/gate-")
}

fn environment_sensitive_path(path: &str) -> bool {
    path.starts_with(".github/")
        || path.starts_with("packaging/")
        || path.starts_with("scripts/install")
        || path.starts_with("scripts/release/")
        || path.starts_with("rust-toolchain")
}

fn validation_scope_for_project(
    target: &ProjectTarget,
    impact: &ImpactAnalysis,
    change_sets: &[ChangeSet],
) -> (ValidationScopeLevel, Vec<String>) {
    let risk_floor = impact
        .risk_paths
        .iter()
        .filter(|risk| risk.project_id == target.project_id)
        .map(|risk| risk.fallback_floor)
        .max();
    let root_sensitive = change_sets
        .iter()
        .filter(|changes| changes.project_id == target.project_id)
        .flat_map(|changes| &changes.entries)
        .any(|entry| {
            matches!(
                entry.source_class,
                SourceClass::Schema | SourceClass::Migration | SourceClass::Unknown
            ) || matches!(entry.path.as_str(), "Cargo.lock" | "Cargo.toml")
                || entry.path.as_str().starts_with(".github/")
                || entry.path.as_str().starts_with("packaging/")
        });
    if root_sensitive || risk_floor == Some(ValidationScopeLevel::ProjectFull) {
        (
            ValidationScopeLevel::ProjectFull,
            vec!["AFFECTED_SCOPE_PROMOTED_PROJECT_FULL".to_owned()],
        )
    } else if impact.status != ImpactStatus::Complete
        || risk_floor == Some(ValidationScopeLevel::Workspace)
        || !impact.limitations.is_empty()
    {
        (
            ValidationScopeLevel::Workspace,
            vec!["AFFECTED_SCOPE_PROMOTED_WORKSPACE".to_owned()],
        )
    } else {
        (
            ValidationScopeLevel::Package,
            vec!["AFFECTED_SCOPE_PACKAGE_PROVEN".to_owned()],
        )
    }
}

fn bind_scope_arguments(template: &[String], scope: ValidationScopeLevel) -> Vec<String> {
    let scope_value = match scope {
        ValidationScopeLevel::Package => "package",
        ValidationScopeLevel::Workspace => "workspace",
        ValidationScopeLevel::ProjectFull => "project_full",
    };
    template
        .iter()
        .map(|argument| argument.replace("{scope}", scope_value))
        .collect()
}

fn risk_level(floor: RiskSeverityFloor) -> ValidationRiskLevel {
    match floor {
        RiskSeverityFloor::Info => ValidationRiskLevel::Low,
        RiskSeverityFloor::Warning => ValidationRiskLevel::Medium,
        RiskSeverityFloor::Error => ValidationRiskLevel::High,
        RiskSeverityFloor::Critical => ValidationRiskLevel::Critical,
    }
}

pub fn builtin_risk_descriptors() -> Result<Vec<RiskPathDescriptor>, PlanningError> {
    let descriptors = vec![
        risk_descriptor(
            "star.risk.public-contract",
            vec![SelectorKind::Contract, SelectorKind::Schema],
            vec![SourceClass::Schema],
            vec!["contract", "test", "architecture", "validator_guard"],
            RiskSeverityFloor::Error,
            ValidationScopeLevel::ProjectFull,
        ),
        risk_descriptor(
            "star.risk.managed-registry",
            vec![SelectorKind::ManagedDeclaration],
            vec![],
            vec![
                "managed_registry_contract",
                "consumer_compatibility",
                "generated_consistency",
                "docs_contract_drift",
                "contract",
                "test",
                "architecture",
                "hardcoding",
                "validator_guard",
            ],
            RiskSeverityFloor::Error,
            ValidationScopeLevel::ProjectFull,
        ),
        risk_descriptor(
            "star.risk.dependency-lockfile",
            vec![],
            vec![SourceClass::Config],
            vec!["dependency", "security", "build", "test"],
            RiskSeverityFloor::Error,
            ValidationScopeLevel::Workspace,
        ),
        risk_descriptor(
            "star.risk.migration",
            vec![],
            vec![SourceClass::Migration],
            vec!["migration", "test", "regression", "security"],
            RiskSeverityFloor::Critical,
            ValidationScopeLevel::ProjectFull,
        ),
        risk_descriptor(
            "star.risk.generated-source",
            vec![],
            vec![SourceClass::Generated],
            vec!["generation", "build", "test", "architecture"],
            RiskSeverityFloor::Warning,
            ValidationScopeLevel::Workspace,
        ),
    ];
    descriptors
        .into_iter()
        .map(|descriptor| descriptor.seal().map_err(PlanningError::from))
        .collect()
}

fn risk_descriptor(
    risk_id: &str,
    selector_kinds: Vec<SelectorKind>,
    source_classes: Vec<SourceClass>,
    families: Vec<&str>,
    severity_floor: RiskSeverityFloor,
    fallback_floor: ValidationScopeLevel,
) -> RiskPathDescriptor {
    RiskPathDescriptor {
        schema_id: star_contracts::planning::RISK_PATH_DESCRIPTOR_SCHEMA_ID.to_owned(),
        schema_version: 1,
        risk_id: risk_id.to_owned(),
        version: "1.0.0".to_owned(),
        selector_kinds,
        source_classes,
        entity_kinds: vec![],
        required_check_families: families.into_iter().map(str::to_owned).collect(),
        severity_floor,
        fallback_floor,
        content_fingerprint: empty_fingerprint(),
    }
}

pub fn descriptor(
    check_id: &str,
    family: &str,
    supported_scope_levels: Vec<ValidationScopeLevel>,
    applicable_source_classes: Vec<SourceClass>,
    args: Vec<String>,
) -> Result<CheckDescriptor, PlanningError> {
    process_descriptor(
        check_id,
        family,
        "star.project.validator",
        "project-validator",
        supported_scope_levels,
        applicable_source_classes,
        args,
    )
}

pub fn process_descriptor(
    check_id: &str,
    family: &str,
    tool_id: &str,
    logical_executable: &str,
    supported_scope_levels: Vec<ValidationScopeLevel>,
    applicable_source_classes: Vec<SourceClass>,
    args: Vec<String>,
) -> Result<CheckDescriptor, PlanningError> {
    let content_fingerprint = versioned_fingerprint(
        "star.check-descriptor",
        1,
        &serde_json::json!({
            "check_id":check_id,
            "family":family,
            "tool_id":tool_id,
            "logical_executable":logical_executable,
            "args":args,
            "scope":supported_scope_levels,
            "classes":applicable_source_classes,
        }),
    )
    .map_err(|_| PlanningError::Fingerprint)?;
    Ok(CheckDescriptor {
        check_id: check_id.to_owned(),
        family: family.to_owned(),
        project_ids: vec![],
        tool_id: tool_id.to_owned(),
        logical_executable: logical_executable.to_owned(),
        argument_template: args,
        supported_scope_levels,
        applicable_source_classes,
        trusted: true,
        available: true,
        required_evidence: vec!["validation_result".to_owned()],
        content_fingerprint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use star_contracts::{
        evidence::{ActorType, ArtifactRef, CatalogRef},
        ids::{
            CheckoutId, CodeIndexSnapshotId, GenerationId, ProjectCatalogSnapshotId, ProjectId,
            ProjectRevisionId, ScanRunId, WorkspaceSnapshotId,
        },
        index::{
            CodeIndexCounts, FreshnessProof, IndexCoverage, IndexLimitation, IndexPartition,
            IndexPartitionKind, IndexPartitionState, ProjectCatalogCounts,
            ProjectCatalogProjectRef,
        },
        managed_registry::{
            EvidenceCompleteness, ManagedDeclaration, ManagedDeclarationId, ManagedDeclarationKind,
            ManagedLifecycle, ManagedLifecycleRecord, ManagedOwnerRef, ManagedRegistrySnapshot,
            ManagedValueRole, RegistryFreshness, RegistryResolutionState, RegistrySourceRef,
        },
        management::Completeness,
        planning::{BaselinePolicyKind, IntendedChangeKind},
    };

    fn actor() -> ActorRef {
        ActorRef {
            actor_type: ActorType::User,
            actor_id: "fixture-user".to_owned(),
            display_name: "Fixture User".to_owned(),
            auth_source: "fixture".to_owned(),
        }
    }

    fn fixture() -> (ProjectCatalogSnapshot, PlanningProjectIndex, TaskSpecDraft) {
        let project_id = ProjectId::new();
        let checkout_id = CheckoutId::new();
        let catalog_id = ProjectCatalogSnapshotId::from_stable_bytes(b"catalog");
        let revision_id = ProjectRevisionId::from_stable_bytes(b"revision");
        let workspace_id = WorkspaceSnapshotId::from_stable_bytes(b"workspace");
        let source_id = star_contracts::ids::CanonicalSourceId::from_stable_bytes(b"source");
        let source_fingerprint = Sha256Hash::digest(b"source-content");
        let catalog_fingerprint = Sha256Hash::digest(b"catalog-content");
        let catalog = ProjectCatalogSnapshot {
            schema_id: CATALOG_SCHEMA_ID.to_owned(),
            schema_version: 1,
            project_catalog_snapshot_id: catalog_id.clone(),
            discovery_scope_fingerprint: Sha256Hash::digest(b"scope"),
            discovery_config_fingerprint: Sha256Hash::digest(b"config"),
            project_refs: vec![ProjectCatalogProjectRef {
                project_id: project_id.clone(),
                content_fingerprint: Sha256Hash::digest(b"project"),
            }],
            checkout_refs: vec![],
            workspace_nodes: vec![],
            project_edges: vec![],
            counts: ProjectCatalogCounts {
                roots: 1,
                projects: 1,
                checkouts: 1,
                workspaces: 1,
                excluded: 0,
                errors: 0,
            },
            completeness: Completeness::Complete,
            limitations: vec![],
            captured_at: Utc::now(),
            content_fingerprint: catalog_fingerprint,
        };
        let partition_fingerprint = Sha256Hash::digest(b"partition");
        let snapshot = CodeIndexSnapshot {
            schema_id: "star.code-index-snapshot".to_owned(),
            schema_version: 1,
            code_index_snapshot_id: CodeIndexSnapshotId::from_stable_bytes(b"index"),
            project_id: project_id.clone(),
            checkout_id: checkout_id.clone(),
            project_catalog_snapshot_id: catalog_id,
            checkout_observation_fingerprint: Sha256Hash::digest(b"checkout"),
            project_revision_id: revision_id,
            workspace_snapshot_id: workspace_id,
            scan_run_id: ScanRunId::new(),
            generation_id: GenerationId::new(),
            analysis_input_fingerprint: Sha256Hash::digest(b"analysis"),
            scan_config_fingerprint: Sha256Hash::digest(b"scan-config"),
            index_config_fingerprint: Sha256Hash::digest(b"index-config"),
            scan_mode: star_contracts::index::IndexScanMode::Incremental,
            required_tier: IndexTier::Text,
            max_tier: IndexTier::Semantic,
            adapter_set_fingerprint: Sha256Hash::digest(b"adapters"),
            classification_fingerprint: Sha256Hash::digest(b"classification"),
            partitions: vec![IndexPartition {
                partition_key: "inventory".to_owned(),
                kind: IndexPartitionKind::Inventory,
                required: true,
                requested_tier: IndexTier::Text,
                used_tier: Some(IndexTier::Text),
                state: IndexPartitionState::Succeeded,
                input_fingerprint: partition_fingerprint.clone(),
                output_fingerprint: Some(partition_fingerprint.clone()),
                target_count: 1,
                indexed_count: 1,
                failed_count: 0,
                excluded_count: 0,
                cache_hit: false,
                limitations: vec![],
            }],
            coverage: Vec::<IndexCoverage>::new(),
            counts: CodeIndexCounts {
                sources: 1,
                packages: 1,
                modules: 1,
                symbols: 2,
                definitions: 1,
                references: 1,
                graph_edges: 1,
                findings: 0,
            },
            freshness: vec![FreshnessProof {
                partition_key: "inventory".to_owned(),
                state: IndexFreshnessState::Current,
                indexed_catalog_fingerprint: Sha256Hash::digest(b"catalog"),
                indexed_source_fingerprint: source_fingerprint.clone(),
                indexed_config_fingerprint: Sha256Hash::digest(b"config"),
                indexed_adapter_fingerprint: Sha256Hash::digest(b"adapter"),
                observed_source_fingerprint: Some(source_fingerprint.clone()),
                probe_method: "fixture".to_owned(),
                probed_at: Utc::now(),
                stale_reason_codes: vec![],
                unverified_scope_count: 0,
            }],
            toolchains: Vec::new(),
            guidance: Vec::new(),
            hardcoding_candidates: Vec::new(),
            limitations: Vec::<IndexLimitation>::new(),
            artifact_refs: Vec::<ArtifactRef>::new(),
            content_fingerprint: Sha256Hash::digest(b"index-content"),
        };
        let source = SourceEntry {
            canonical_source_id: source_id.clone(),
            path: ProjectPathRef::parse("src/lib.rs").unwrap(),
            content_sha256: source_fingerprint.clone(),
            size_bytes: 16,
            source_class: SourceClass::Source,
            facets: vec![],
            language_id: "rust".to_owned(),
            encoding: "utf-8".to_owned(),
            owner_project_id: project_id.clone(),
            owner_checkout_id: checkout_id.clone(),
            analysis_eligible: true,
            content_fingerprint: source_fingerprint.clone(),
        };
        let source_entity = IndexEntity {
            entity_key: format!("source:{}", source.canonical_source_id),
            kind: IndexEntityKind::Source,
            canonical_source_id: Some(source_id.clone()),
            symbol_id: None,
            qualified_name: "src/lib.rs".to_owned(),
            source_range: None,
            tier: IndexTier::Text,
            confidence: "high".to_owned(),
            content_fingerprint: source_fingerprint.clone(),
        };
        let provider = IndexEntity {
            entity_key: "symbol:provider".to_owned(),
            kind: IndexEntityKind::Symbol,
            canonical_source_id: Some(source_id.clone()),
            symbol_id: None,
            qualified_name: "provider".to_owned(),
            source_range: None,
            tier: IndexTier::Semantic,
            confidence: "high".to_owned(),
            content_fingerprint: Sha256Hash::digest(b"provider"),
        };
        let consumer = IndexEntity {
            entity_key: "symbol:consumer".to_owned(),
            kind: IndexEntityKind::Symbol,
            canonical_source_id: Some(source_id),
            symbol_id: None,
            qualified_name: "consumer".to_owned(),
            source_range: None,
            tier: IndexTier::Semantic,
            confidence: "high".to_owned(),
            content_fingerprint: Sha256Hash::digest(b"consumer"),
        };
        let edge = IndexEdge {
            edge_key: "edge:consumer-provider".to_owned(),
            from_entity_key: consumer.entity_key.clone(),
            to_entity_key: Some(provider.entity_key.clone()),
            unresolved_target: None,
            relation: IndexRelation::References,
            evidence_source_id: source.canonical_source_id.clone(),
            evidence_range: None,
            tier: IndexTier::Semantic,
            resolution: SymbolResolution::Resolved,
            confidence: "high".to_owned(),
            content_fingerprint: Sha256Hash::digest(b"edge"),
        };
        let index = PlanningProjectIndex {
            snapshot,
            source_entries: vec![source],
            entities: vec![source_entity, provider, consumer],
            edges: vec![edge],
            managed_registry_snapshot: None,
            observed_changes: vec![ObservedWorkspaceChange {
                path: ProjectPathRef::parse("src/lib.rs").unwrap(),
                rename_from: None,
                change_kind: ObservedChangeKind::Modify,
                before_sha256: Some(Sha256Hash::digest(b"before")),
                after_sha256: Some(source_fingerprint),
                staged: true,
                unstaged: true,
                untracked: false,
                binary: false,
            }],
            collection_state: CollectionState::Complete,
            collection_limits: vec![],
        };
        let draft = TaskSpecDraft {
            title: "Change provider".to_owned(),
            objective: "Update provider without breaking consumers".to_owned(),
            project_targets: vec![ProjectTarget {
                project_id,
                checkout_id,
                role: ProjectTargetRole::PlannedChange,
                reason: "user target".to_owned(),
            }],
            included_scope: vec![PlanningSelector {
                kind: SelectorKind::Symbol,
                value: "provider".to_owned(),
            }],
            excluded_scope: vec![],
            intended_changes: vec![IntendedChange {
                change_id: "change-provider".to_owned(),
                selector: PlanningSelector {
                    kind: SelectorKind::Symbol,
                    value: "provider".to_owned(),
                },
                change_kind: IntendedChangeKind::Modify,
                intended_postcondition: "consumer remains valid".to_owned(),
            }],
            success_criteria: vec![SuccessCriterion {
                criterion_id: "tests-pass".to_owned(),
                description: "Tests pass".to_owned(),
                verification: "validation evidence".to_owned(),
                required: true,
            }],
            constraints: vec![],
            forbidden_actions: vec!["remote_write".to_owned()],
            profile_ids: vec![],
            baseline_policy: BaselinePolicy {
                kind: BaselinePolicyKind::CurrentWorkspace,
                reference: None,
            },
            requested_checks: vec!["test".to_owned()],
            check_overrides: vec![],
            assumptions: vec![],
        };
        (catalog, index, draft)
    }

    #[test]
    fn managed_declaration_selector_resolves_only_against_an_exact_current_registry_pin() {
        let (_, mut index, _) = fixture();
        let declaration_id = ManagedDeclarationId::parse("star.error.fixture").unwrap();
        let source = index.source_entries[0].clone();
        let registry = ManagedRegistrySnapshot {
            schema_id: star_contracts::managed_registry::MANAGED_REGISTRY_SNAPSHOT_SCHEMA_ID
                .to_owned(),
            schema_version: 2,
            managed_registry_snapshot_id: star_contracts::ids::ManagedRegistrySnapshotId::new(),
            registry_id: "star.fixture".to_owned(),
            registry_version: "1.0.0".to_owned(),
            owner_project_id: index.snapshot.project_id.clone(),
            checkout_id: index.snapshot.checkout_id.clone(),
            project_revision_id: index.snapshot.project_revision_id.clone(),
            workspace_snapshot_id: index.snapshot.workspace_snapshot_id.clone(),
            git_revision: "0".repeat(40),
            manifest_sha256: Sha256Hash::digest(b"registry-manifest"),
            manifest_source_refs: vec![RegistrySourceRef {
                path: source.path.clone(),
                source_sha256: source.content_sha256.clone(),
            }],
            namespace_claims: vec![],
            declarations: vec![ManagedDeclaration {
                managed_declaration_id: declaration_id.clone(),
                item_version: "1.0.0".to_owned(),
                namespace: "star.error".to_owned(),
                semantic_key: "fixture".to_owned(),
                kind: ManagedDeclarationKind::ErrorCode,
                owner: ManagedOwnerRef {
                    project_id: index.snapshot.project_id.clone(),
                    contract_id: Some("star.errors".to_owned()),
                    module_key: Some("fixture".to_owned()),
                    approval_policy_ref: CatalogRef {
                        catalog_id: "star.policy.registry".to_owned(),
                        format_version: 1,
                        item_version: "1.0.0".to_owned(),
                        sha256: Sha256Hash::digest(b"registry-policy"),
                    },
                    display_owner: None,
                },
                value_type: "string".to_owned(),
                value_role: ManagedValueRole::StableIdentifier,
                primary_value: Some("FIXTURE_ERROR".to_owned()),
                description: "Fixture error".to_owned(),
                status: ManagedLifecycle::Active,
                lifecycle: ManagedLifecycleRecord {
                    introduced_in_registry_version: "1.0.0".to_owned(),
                    deprecated_in_registry_version: None,
                    removed_in_registry_version: None,
                    replacement_id: None,
                    migration_record_ref: None,
                },
                aliases: vec![],
                binding_specs: vec![],
                consumer_contracts: vec![],
                uniqueness_scope: "star.error".to_owned(),
                source_path: source.path,
                source_sha256: source.content_sha256,
                definition_fingerprint: Sha256Hash::digest(b"fixture-declaration"),
            }],
            binding_observations: vec![],
            consumers: vec![],
            candidates: vec![],
            local_constants: vec![],
            code_index_snapshot_id: Some(index.snapshot.code_index_snapshot_id.clone()),
            tombstones: vec![],
            tombstone_set_fingerprint: Sha256Hash::digest(b"empty-tombstones"),
            resolution_state: RegistryResolutionState::Valid,
            freshness: RegistryFreshness::Current,
            completeness: EvidenceCompleteness::Complete,
            limitations: vec![],
            diagnostic_refs: vec![],
            content_fingerprint: Sha256Hash::digest(b"unsealed"),
        }
        .seal()
        .unwrap();
        index.managed_registry_snapshot = Some(registry.clone());
        let selector = PlanningSelector {
            kind: SelectorKind::ManagedDeclaration,
            value: declaration_id.to_string(),
        };
        let resolved = map_selector_to_seeds(&selector, &index).unwrap();
        assert!(
            resolved
                .iter()
                .all(|seed| seed.resolution == SeedResolution::Resolved)
        );
        assert!(resolved.iter().any(|seed| seed.entity_key.is_some()));

        let mut stale = registry;
        stale.workspace_snapshot_id = WorkspaceSnapshotId::from_stable_bytes(b"stale-workspace");
        index.managed_registry_snapshot = Some(stale);
        assert!(
            map_selector_to_seeds(&selector, &index)
                .unwrap()
                .iter()
                .all(|seed| seed.resolution == SeedResolution::Stale)
        );
    }

    #[test]
    fn task_to_impact_to_ready_validation_plan_is_deterministic_and_typed() {
        let (catalog, index, task) = fixture();
        let descriptors = [
            "format",
            "lint",
            "build",
            "test",
            "architecture",
            "hardcoding",
            "security",
        ]
        .into_iter()
        .map(|family| {
            descriptor(
                &format!("star.check.{family}"),
                family,
                vec![
                    ValidationScopeLevel::Package,
                    ValidationScopeLevel::Workspace,
                    ValidationScopeLevel::ProjectFull,
                ],
                vec![SourceClass::Source, SourceClass::Test],
                vec!["--scope".to_owned(), "{scope}".to_owned()],
            )
            .unwrap()
        })
        .collect();
        let bundle = build_planning_bundle(PlanningRequest {
            task,
            actor: actor(),
            catalog,
            projects: vec![index],
            risk_descriptors: vec![],
            check_descriptors: descriptors,
            previous_success_evidence: vec![],
            profile_resolution: None,
            policy: PlanningPolicy::default(),
        })
        .unwrap();
        assert_eq!(
            bundle.validation_plan.readiness,
            ValidationPlanV2Readiness::Ready
        );
        assert!(
            bundle
                .impact_analysis
                .impacted_nodes
                .iter()
                .any(|node| node.entity_key == "symbol:consumer" && node.minimum_distance == 1)
        );
        assert!(
            bundle
                .impact_analysis
                .impact_edges
                .iter()
                .all(|edge| edge.certainty == ImpactCertainty::Confirmed)
        );
        assert_eq!(
            bundle.change_sets[0].entries[0].origin,
            ChangeOrigin::Preexisting
        );
    }

    #[test]
    fn missing_required_check_is_blocked_not_not_applicable() {
        let (catalog, index, mut task) = fixture();
        task.requested_checks = vec!["contract".to_owned()];
        let bundle = build_planning_bundle(PlanningRequest {
            task,
            actor: actor(),
            catalog,
            projects: vec![index],
            risk_descriptors: vec![],
            check_descriptors: vec![],
            previous_success_evidence: vec![],
            profile_resolution: None,
            policy: PlanningPolicy::default(),
        })
        .unwrap();
        assert_eq!(
            bundle.validation_plan.readiness,
            ValidationPlanV2Readiness::Blocked
        );
        assert!(
            bundle
                .validation_plan
                .unresolved_checks
                .iter()
                .any(|check| check.family == "contract" && check.reason == "descriptor_not_found")
        );
    }

    #[test]
    fn graph_limit_never_becomes_complete_or_confirmed_empty() {
        let (catalog, index, task) = fixture();
        let check = descriptor(
            "star.check.test",
            "test",
            vec![ValidationScopeLevel::ProjectFull],
            vec![SourceClass::Source],
            vec!["--scope".to_owned(), "{scope}".to_owned()],
        )
        .unwrap();
        let result = build_planning_bundle(PlanningRequest {
            task,
            actor: actor(),
            catalog,
            projects: vec![index],
            risk_descriptors: vec![],
            check_descriptors: vec![check],
            previous_success_evidence: vec![],
            profile_resolution: None,
            policy: PlanningPolicy {
                max_nodes: 1,
                ..PlanningPolicy::default()
            },
        })
        .unwrap();
        assert_eq!(result.impact_analysis.status, ImpactStatus::Partial);
        assert_eq!(
            result.validation_plan.readiness,
            ValidationPlanV2Readiness::Blocked
        );
        assert!(
            result
                .impact_analysis
                .limitations
                .contains(&"IMPACT_GRAPH_LIMIT".to_owned())
        );
        assert!(
            result
                .impact_analysis
                .no_results
                .iter()
                .all(|result| result.reason != NoResultReason::ConfirmedEmpty)
        );
    }

    #[test]
    fn scope_revision_replan_preserves_document_identity_and_records_lineage() {
        let (catalog, index, task) = fixture();
        let check = descriptor(
            "star.check.test",
            "test",
            vec![ValidationScopeLevel::ProjectFull],
            vec![SourceClass::Source],
            vec!["--scope".to_owned(), "{scope}".to_owned()],
        )
        .unwrap();
        let first = build_planning_bundle(PlanningRequest {
            task: task.clone(),
            actor: actor(),
            catalog: catalog.clone(),
            projects: vec![index.clone()],
            risk_descriptors: vec![],
            check_descriptors: vec![check.clone()],
            previous_success_evidence: vec![],
            profile_resolution: None,
            policy: PlanningPolicy::default(),
        })
        .unwrap();
        let previous_scope_ref = scope_ref(&first.scope_revision);
        let mut revised_task = task;
        revised_task.objective = "Update provider and preserve the public contract".to_owned();
        let revised = revise_planning_bundle(PlanningRevisionRequest {
            previous: first.clone(),
            request: PlanningRequest {
                task: revised_task,
                actor: actor(),
                catalog,
                projects: vec![index],
                risk_descriptors: vec![],
                check_descriptors: vec![check],
                previous_success_evidence: vec![],
                profile_resolution: None,
                policy: PlanningPolicy::default(),
            },
            reason_code: ScopeReasonCode::UserEdit,
            reason: "objective clarified".to_owned(),
            user_decisions: vec![],
        })
        .unwrap();

        assert_eq!(revised.task_spec.task_spec_id, first.task_spec.task_spec_id);
        assert_eq!(
            revised.scope_revision.scope_revision_id,
            first.scope_revision.scope_revision_id
        );
        assert_eq!(
            revised.impact_analysis.impact_analysis_id,
            first.impact_analysis.impact_analysis_id
        );
        assert_eq!(
            revised.validation_plan.validation_plan_id,
            first.validation_plan.validation_plan_id
        );
        assert_eq!(revised.task_spec.revision, 2);
        assert_eq!(revised.scope_revision.revision, 2);
        assert_eq!(
            revised.scope_revision.previous_scope_revision_ref,
            Some(previous_scope_ref)
        );
        assert!(
            revised
                .scope_revision
                .changed_fields
                .contains(&"objective".to_owned())
        );
        assert_eq!(planning_bundle_revision(&revised), 2);
        assert_eq!(revised.clone().seal().unwrap(), revised);

        let invalidated = invalidate_planning_bundle(revised, "source changed").unwrap();
        assert_eq!(planning_bundle_revision(&invalidated), 3);
        assert_eq!(
            invalidated.impact_analysis.status,
            ImpactStatus::Invalidated
        );
        assert_eq!(
            invalidated.validation_plan.readiness,
            ValidationPlanV2Readiness::Invalidated
        );
        assert_eq!(invalidated.clone().seal().unwrap(), invalidated);
    }

    #[test]
    fn affected_check_outcomes_distinguish_optional_not_applicable_and_waived() {
        let (catalog, index, mut task) = fixture();
        task.check_overrides = vec![
            CheckOverride {
                family: "audit".to_owned(),
                kind: CheckOverrideKind::Add,
                reason: "extra signal".to_owned(),
            },
            CheckOverride {
                family: "docs".to_owned(),
                kind: CheckOverrideKind::Add,
                reason: "consider documentation".to_owned(),
            },
            CheckOverride {
                family: "lint".to_owned(),
                kind: CheckOverrideKind::Omit,
                reason: "temporary approved waiver".to_owned(),
            },
        ];
        let descriptors = ["format", "lint", "build", "test", "audit"]
            .into_iter()
            .map(|family| {
                descriptor(
                    &format!("star.check.{family}"),
                    family,
                    vec![ValidationScopeLevel::ProjectFull],
                    vec![SourceClass::Source],
                    vec!["--scope".to_owned(), "{scope}".to_owned()],
                )
                .unwrap()
            })
            .chain(std::iter::once(
                descriptor(
                    "star.check.docs",
                    "docs",
                    vec![ValidationScopeLevel::ProjectFull],
                    vec![SourceClass::Docs],
                    vec!["--scope".to_owned(), "{scope}".to_owned()],
                )
                .unwrap(),
            ))
            .collect();
        let bundle = build_planning_bundle(PlanningRequest {
            task,
            actor: actor(),
            catalog,
            projects: vec![index],
            risk_descriptors: vec![],
            check_descriptors: descriptors,
            previous_success_evidence: vec![],
            profile_resolution: None,
            policy: PlanningPolicy::default(),
        })
        .unwrap();
        assert!(bundle.validation_plan.candidate_checks.iter().any(|check| {
            check.family == "audit" && check.outcome == CheckResolutionOutcome::SelectedOptional
        }));
        assert!(bundle.validation_plan.candidate_checks.iter().any(|check| {
            check.family == "docs" && check.outcome == CheckResolutionOutcome::OmittedNotApplicable
        }));
        assert!(bundle.validation_plan.candidate_checks.iter().any(|check| {
            check.family == "lint" && check.outcome == CheckResolutionOutcome::UserWaived
        }));
        assert_eq!(bundle.validation_plan.optional_checks.len(), 1);
        assert!(bundle.validation_plan.independent_review.required);
    }
}
