use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};
use star_contracts::{
    ProjectId, Sha256Hash,
    coordination_v2::{
        BundleAggregateState, BundleStepEdge, BundleStepGraph,
        CHANGE_BUNDLE_PARTICIPANT_V2_SCHEMA_ID, CHANGE_BUNDLE_RELEASE_HANDOFF_SCHEMA_ID,
        CROSS_REPO_CHANGE_BUNDLE_SCHEMA_ID, ChangeBundleParticipantV2, ChangeBundleReleaseHandoff,
        CompletionLevel, ConflictResolutionClass, CrossRepoChangeBundle, DirtyState,
        MERGE_CONFLICT_RECORD_SCHEMA_ID, MERGE_PLAN_V2_SCHEMA_ID, MERGE_QUEUE_RECORD_SCHEMA_ID,
        MULTI_PROJECT_GOAL_SCHEMA_ID, MergeConflictRecord, MergePlanV2, MergeQueueEntryState,
        MergeQueueRecord, MergeStrategyV2, MultiProjectGoal, OVERLAP_ANALYSIS_SCHEMA_ID,
        OverlapAnalysis, OverlapAxis, OverlapDisposition, OverlapItem, OverlapSubject,
        PROJECT_MERGE_RESULT_SCHEMA_ID, ParticipantState, ProjectMergeResult,
        ProjectMergeResultState, REMOTE_OPERATION_RECORD_SCHEMA_ID,
        REMOTE_STATE_SNAPSHOT_V2_SCHEMA_ID, RemoteAction, RemoteOperationRecord,
        RemoteOperationState, RemoteRefObservation, RemoteStateSnapshotV2,
        WORKTREE_RECORD_SCHEMA_ID, WorktreeRecord, WorktreeState,
    },
    development_v2::CoverageState,
};

use crate::{DevelopmentError, fingerprint, placeholder, token};

pub fn seal_step_graph(mut graph: BundleStepGraph) -> Result<BundleStepGraph, DevelopmentError> {
    if graph.steps.is_empty() || graph.steps.len() > 4_096 || graph.edges.len() > 16_384 {
        return Err(DevelopmentError::Invalid);
    }
    graph
        .steps
        .sort_by(|left, right| left.step_id.cmp(&right.step_id));
    graph.edges.sort_by(|left, right| {
        (
            &left.from_step_id,
            &left.to_step_id,
            format!("{:?}", left.edge_kind),
        )
            .cmp(&(
                &right.from_step_id,
                &right.to_step_id,
                format!("{:?}", right.edge_kind),
            ))
    });
    if graph
        .steps
        .iter()
        .any(|step| !token(&step.step_id, 192) || step.completion_condition.trim().is_empty())
        || graph
            .steps
            .windows(2)
            .any(|pair| pair[0].step_id == pair[1].step_id)
        || graph.edges.windows(2).any(|pair| pair[0] == pair[1])
    {
        return Err(DevelopmentError::Conflict);
    }
    graph.topological_order = topological_order(
        &graph
            .steps
            .iter()
            .map(|step| step.step_id.clone())
            .collect::<Vec<_>>(),
        &graph.edges,
    )?;
    graph.graph_fingerprint = fingerprint(
        "star.bundle-step-graph",
        &serde_json::json!({"steps":graph.steps,"edges":graph.edges,"topological_order":graph.topological_order}),
    )?;
    Ok(graph)
}

pub fn seal_multi_project_goal(
    mut goal: MultiProjectGoal,
) -> Result<MultiProjectGoal, DevelopmentError> {
    if goal.schema_id != MULTI_PROJECT_GOAL_SCHEMA_ID
        || goal.schema_version != 1
        || goal.revision == 0
        || !token(&goal.multi_project_goal_id, 192)
        || goal.goal_spec_ref.trim().is_empty()
        || goal.participants.len() < 2
        || goal.task_spec_refs.is_empty()
        || goal.scope_revision_refs.is_empty()
        || !valid_budget(&goal.resource_budget)
    {
        return Err(DevelopmentError::Invalid);
    }
    goal.participants
        .sort_by(|left, right| left.project_id.cmp(&right.project_id));
    if goal
        .participants
        .windows(2)
        .any(|pair| pair[0].project_id == pair[1].project_id)
        || goal
            .participants
            .iter()
            .any(|participant| participant.roles.is_empty())
    {
        return Err(DevelopmentError::Conflict);
    }
    let projects = goal
        .participants
        .iter()
        .map(|item| &item.project_id)
        .collect::<BTreeSet<_>>();
    goal.project_relations
        .sort_by(|left, right| left.relation_id.cmp(&right.relation_id));
    if goal
        .project_relations
        .windows(2)
        .any(|pair| pair[0].relation_id == pair[1].relation_id)
        || goal.project_relations.iter().any(|relation| {
            !token(&relation.relation_id, 192)
                || relation.provider_project_id == relation.consumer_project_id
                || !projects.contains(&relation.provider_project_id)
                || !projects.contains(&relation.consumer_project_id)
                || relation.contract_refs.is_empty()
        })
    {
        return Err(DevelopmentError::Conflict);
    }
    goal.step_graph = seal_step_graph(goal.step_graph)?;
    let step_ids = goal
        .step_graph
        .steps
        .iter()
        .map(|step| step.step_id.as_str())
        .collect::<BTreeSet<_>>();
    if goal.compatibility_windows.iter().any(|window| {
        !step_ids.contains(window.open_step_ref.as_str())
            || !step_ids.contains(window.close_step_ref.as_str())
            || !projects.contains(&window.provider_project_id)
            || window
                .required_consumer_project_ids
                .iter()
                .any(|id| !projects.contains(id))
    }) {
        return Err(DevelopmentError::Conflict);
    }
    normalize_strings(&mut goal.task_spec_refs);
    normalize_strings(&mut goal.scope_revision_refs);
    normalize_strings(&mut goal.source_snapshot_refs);
    normalize_strings(&mut goal.unknowns);
    normalize_strings(&mut goal.questions);
    goal.goal_fingerprint = fingerprint(
        MULTI_PROJECT_GOAL_SCHEMA_ID,
        &serde_json::json!({
            "multi_project_goal_id":goal.multi_project_goal_id,"revision":goal.revision,
            "previous_revision_ref":goal.previous_revision_ref,"goal_spec_ref":goal.goal_spec_ref,
            "task_spec_refs":goal.task_spec_refs,"scope_revision_refs":goal.scope_revision_refs,
            "participants":goal.participants,"project_relations":goal.project_relations,
            "step_graph":goal.step_graph,"compatibility_windows":goal.compatibility_windows,
            "cross_project_invariants":goal.cross_project_invariants,
            "completion_target":goal.completion_target,"resource_budget":goal.resource_budget,
            "permission_floor_ref":goal.permission_floor_ref,"source_snapshot_refs":goal.source_snapshot_refs,
            "unknowns":goal.unknowns,"questions":goal.questions,
        }),
    )?;
    Ok(goal)
}

pub fn seal_participant(
    mut participant: ChangeBundleParticipantV2,
) -> Result<ChangeBundleParticipantV2, DevelopmentError> {
    if participant.schema_id != CHANGE_BUNDLE_PARTICIPANT_V2_SCHEMA_ID
        || participant.schema_version != 2
        || participant.revision == 0
        || !token(&participant.participant_id, 192)
        || participant.roles.is_empty()
        || participant.step_ids.is_empty()
        || participant.change_bundle_ref.trim().is_empty()
        || participant.recovery_plan_ref.trim().is_empty()
        || !valid_git_oid(&participant.base_commit_oid, &participant.git_object_format)
        || matches!(
            participant.dirty_state,
            DirtyState::DirtyPartial | DirtyState::Unverified
        ) && !matches!(
            participant.state,
            ParticipantState::Preparing | ParticipantState::Held | ParticipantState::Failed
        )
    {
        return Err(DevelopmentError::Invalid);
    }
    participant.roles.sort();
    participant.roles.dedup();
    for values in [
        &mut participant.step_ids,
        &mut participant.change_plan_refs,
        &mut participant.patch_set_refs,
        &mut participant.migration_plan_refs,
        &mut participant.worktree_record_refs,
        &mut participant.validation_plan_refs,
        &mut participant.gate_decision_refs,
        &mut participant.evidence_bundle_refs,
        &mut participant.remote_snapshot_refs,
        &mut participant.remote_operation_refs,
        &mut participant.compensation_refs,
    ] {
        normalize_strings(values);
    }
    participant.participant_fingerprint = fingerprint(
        CHANGE_BUNDLE_PARTICIPANT_V2_SCHEMA_ID,
        &serde_json::json!({
            "participant_id":participant.participant_id,"revision":participant.revision,
            "previous_revision_ref":participant.previous_revision_ref,"change_bundle_ref":participant.change_bundle_ref,
            "project_id":participant.project_id,"required":participant.required,"roles":participant.roles,
            "step_ids":participant.step_ids,"checkout_id":participant.checkout_id,
            "repository_fingerprint":participant.repository_fingerprint,"git_object_format":participant.git_object_format,
            "base_project_revision_ref":participant.base_project_revision_ref,"base_commit_oid":participant.base_commit_oid,
            "baseline_workspace_snapshot_ref":participant.baseline_workspace_snapshot_ref,
            "dirty_manifest_ref":participant.dirty_manifest_ref,"dirty_state":participant.dirty_state,
            "preexisting_change_set_ref":participant.preexisting_change_set_ref,
            "change_plan_refs":participant.change_plan_refs,"patch_set_refs":participant.patch_set_refs,
            "migration_plan_refs":participant.migration_plan_refs,"worktree_record_refs":participant.worktree_record_refs,
            "merge_plan_ref":participant.merge_plan_ref,"merge_queue_ref":participant.merge_queue_ref,
            "validation_plan_refs":participant.validation_plan_refs,"gate_decision_refs":participant.gate_decision_refs,
            "evidence_bundle_refs":participant.evidence_bundle_refs,"project_merge_result_ref":participant.project_merge_result_ref,
            "remote_snapshot_refs":participant.remote_snapshot_refs,"remote_operation_refs":participant.remote_operation_refs,
            "recovery_plan_ref":participant.recovery_plan_ref,"compensation_refs":participant.compensation_refs,
            "state":participant.state,"pending_action":participant.pending_action,
            "actual_subject_binding_ref":participant.actual_subject_binding_ref,
        }),
    )?;
    Ok(participant)
}

pub fn seal_cross_repo_bundle(
    goal: &MultiProjectGoal,
    participants: &[ChangeBundleParticipantV2],
    mut bundle: CrossRepoChangeBundle,
) -> Result<CrossRepoChangeBundle, DevelopmentError> {
    if bundle.schema_id != CROSS_REPO_CHANGE_BUNDLE_SCHEMA_ID
        || bundle.schema_version != 1
        || bundle.revision == 0
        || !token(&bundle.change_bundle_id, 192)
        || bundle.multi_project_goal_ref != goal.multi_project_goal_id
        || bundle.participant_refs.len() != participants.len()
        || !valid_budget(&bundle.resource_budget)
    {
        return Err(DevelopmentError::Invalid);
    }
    let sealed = participants
        .iter()
        .cloned()
        .map(seal_participant)
        .collect::<Result<Vec<_>, _>>()?;
    let expected = sealed
        .iter()
        .map(|item| item.participant_id.as_str())
        .collect::<BTreeSet<_>>();
    let actual = bundle
        .participant_refs
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if expected != actual
        || sealed
            .iter()
            .any(|participant| participant.change_bundle_ref != bundle.change_bundle_id)
    {
        return Err(DevelopmentError::Conflict);
    }
    bundle.step_graph = seal_step_graph(bundle.step_graph)?;
    if bundle.step_graph != goal.step_graph {
        return Err(DevelopmentError::Conflict);
    }
    bundle.state = aggregate_bundle_state(&sealed);
    bundle.completion_level_reached = completion_level(&sealed, bundle.remote_policy);
    if bundle.state == BundleAggregateState::Completed
        && bundle.completion_level_reached < bundle.completion_target
    {
        bundle.state = BundleAggregateState::PartiallyApplied;
        bundle
            .remaining_risks
            .push("requested_completion_level_not_reached".to_owned());
    }
    normalize_strings(&mut bundle.participant_refs);
    normalize_strings(&mut bundle.open_effect_refs);
    normalize_strings(&mut bundle.pending_approval_refs);
    normalize_strings(&mut bundle.remaining_risks);
    normalize_strings(&mut bundle.hold_reasons);
    bundle.bundle_fingerprint = fingerprint(
        CROSS_REPO_CHANGE_BUNDLE_SCHEMA_ID,
        &serde_json::json!({
            "change_bundle_id":bundle.change_bundle_id,"revision":bundle.revision,
            "previous_revision_ref":bundle.previous_revision_ref,"multi_project_goal_ref":bundle.multi_project_goal_ref,
            "task_spec_refs":bundle.task_spec_refs,"scope_revision_refs":bundle.scope_revision_refs,
            "input_handoff_refs":bundle.input_handoff_refs,"participant_refs":bundle.participant_refs,
            "step_graph":bundle.step_graph,"compatibility_window_refs":bundle.compatibility_window_refs,
            "merge_policy":bundle.merge_policy,"remote_policy":bundle.remote_policy,
            "resource_budget":bundle.resource_budget,"budget_snapshot_ref":bundle.budget_snapshot_ref,
            "permission_plan_ref":bundle.permission_plan_ref,"gate_policy_fingerprint":bundle.gate_policy_fingerprint,
            "prepare_gate_ref":bundle.prepare_gate_ref,"goal_gate_ref":bundle.goal_gate_ref,
            "state":bundle.state,"completion_target":bundle.completion_target,
            "completion_level_reached":bundle.completion_level_reached,"open_effect_refs":bundle.open_effect_refs,
            "pending_approval_refs":bundle.pending_approval_refs,"remaining_risks":bundle.remaining_risks,
            "hold_reasons":bundle.hold_reasons,"supersedes_bundle_ref":bundle.supersedes_bundle_ref,
        }),
    )?;
    Ok(bundle)
}

pub fn analyze_overlap(
    analysis_id: String,
    revision: u64,
    change_bundle_ref: String,
    mut subjects: Vec<OverlapSubject>,
    ordered_pairs: &BTreeSet<(String, String)>,
) -> Result<OverlapAnalysis, DevelopmentError> {
    if !token(&analysis_id, 192) || revision == 0 || subjects.len() < 2 {
        return Err(DevelopmentError::Invalid);
    }
    subjects.sort_by(|left, right| left.participant_ref.cmp(&right.participant_ref));
    if subjects
        .windows(2)
        .any(|pair| pair[0].participant_ref == pair[1].participant_ref)
    {
        return Err(DevelopmentError::Conflict);
    }
    let mut items = Vec::new();
    let mut limitations = Vec::new();
    for left_index in 0..subjects.len() {
        for right_index in left_index + 1..subjects.len() {
            let left = &subjects[left_index];
            let right = &subjects[right_index];
            let ordered = ordered_pairs
                .contains(&(left.participant_ref.clone(), right.participant_ref.clone()))
                || ordered_pairs
                    .contains(&(right.participant_ref.clone(), left.participant_ref.clone()));
            if left.coverage != CoverageState::Complete || right.coverage != CoverageState::Complete
            {
                items.push(overlap_item(
                    left,
                    right,
                    OverlapAxis::RepositoryPolicy,
                    "coverage",
                    OverlapDisposition::Unknown,
                    "subject coverage is not complete",
                ));
                limitations.push(format!(
                    "incomplete_overlap_subject:{}:{}",
                    left.participant_ref, right.participant_ref
                ));
                continue;
            }
            let same_repo = left.repository_fingerprint == right.repository_fingerprint;
            for (axis, left_refs, right_refs) in [
                (OverlapAxis::File, &left.file_refs, &right.file_refs),
                (OverlapAxis::Rename, &left.rename_refs, &right.rename_refs),
                (OverlapAxis::Range, &left.range_refs, &right.range_refs),
                (OverlapAxis::Symbol, &left.symbol_refs, &right.symbol_refs),
                (
                    OverlapAxis::Contract,
                    &left.contract_refs,
                    &right.contract_refs,
                ),
                (
                    OverlapAxis::Generated,
                    &left.generated_owner_refs,
                    &right.generated_owner_refs,
                ),
                (
                    OverlapAxis::Dependency,
                    &left.dependency_refs,
                    &right.dependency_refs,
                ),
                (
                    OverlapAxis::RepositoryPolicy,
                    &left.repository_policy_refs,
                    &right.repository_policy_refs,
                ),
            ] {
                let semantic_axis = matches!(
                    axis,
                    OverlapAxis::Contract | OverlapAxis::Generated | OverlapAxis::Dependency
                );
                if !same_repo && !semantic_axis {
                    continue;
                }
                for subject_ref in intersection(left_refs, right_refs) {
                    let disposition = if ordered {
                        OverlapDisposition::OrderedOverlap
                    } else {
                        OverlapDisposition::ConflictPossible
                    };
                    items.push(overlap_item(
                        left,
                        right,
                        axis,
                        &subject_ref,
                        disposition,
                        if ordered {
                            "dependency order requires reprepare"
                        } else {
                            "shared current subject requires conflict analysis"
                        },
                    ));
                }
            }
        }
    }
    items.sort_by(|left, right| {
        (
            &left.left_participant_ref,
            &left.right_participant_ref,
            left.axis,
            &left.subject_ref,
        )
            .cmp(&(
                &right.left_participant_ref,
                &right.right_participant_ref,
                right.axis,
                &right.subject_ref,
            ))
    });
    let overall = items
        .iter()
        .map(|item| item.disposition)
        .max_by_key(|state| overlap_rank(*state))
        .unwrap_or(OverlapDisposition::Disjoint);
    normalize_strings(&mut limitations);
    let mut analysis = OverlapAnalysis {
        schema_id: OVERLAP_ANALYSIS_SCHEMA_ID.to_owned(),
        schema_version: 1,
        overlap_analysis_id: analysis_id,
        revision,
        change_bundle_ref,
        subjects,
        items,
        overall,
        parallel_safe: overall == OverlapDisposition::Disjoint,
        merge_ready: matches!(
            overall,
            OverlapDisposition::Disjoint | OverlapDisposition::OrderedOverlap
        ),
        limitations,
        analysis_fingerprint: placeholder(),
    };
    analysis.analysis_fingerprint = fingerprint(
        OVERLAP_ANALYSIS_SCHEMA_ID,
        &serde_json::json!({
            "overlap_analysis_id":analysis.overlap_analysis_id,"revision":analysis.revision,
            "change_bundle_ref":analysis.change_bundle_ref,"subjects":analysis.subjects,"items":analysis.items,
            "overall":analysis.overall,"parallel_safe":analysis.parallel_safe,"merge_ready":analysis.merge_ready,
            "limitations":analysis.limitations,
        }),
    )?;
    Ok(analysis)
}

pub fn seal_worktree_record(
    mut record: WorktreeRecord,
) -> Result<WorktreeRecord, DevelopmentError> {
    if record.schema_id != WORKTREE_RECORD_SCHEMA_ID
        || record.schema_version != 1
        || record.revision == 0
        || !token(&record.worktree_id, 192)
        || !token(&record.participant_id, 192)
        || !token(&record.step_id, 192)
        || !valid_git_oid_any(&record.base_commit_oid)
        || record.root_binding_id.trim().is_empty()
        || record.before_manifest_ref.trim().is_empty()
        || matches!(
            record.state,
            WorktreeState::Ready
                | WorktreeState::Dirty
                | WorktreeState::Validating
                | WorktreeState::MergeReady
        ) && (record.creation_receipt_ref.is_none() || record.last_probe_ref.is_none())
        || record.state == WorktreeState::Discarded && record.evidence_hold
    {
        return Err(DevelopmentError::Invalid);
    }
    record.record_fingerprint = fingerprint(
        WORKTREE_RECORD_SCHEMA_ID,
        &serde_json::json!({
            "worktree_id":record.worktree_id,"revision":record.revision,"previous_revision_ref":record.previous_revision_ref,
            "project_id":record.project_id,"participant_id":record.participant_id,"step_id":record.step_id,
            "repository_fingerprint":record.repository_fingerprint,"base_commit_oid":record.base_commit_oid,
            "root_binding_id":record.root_binding_id,"role":record.role,"branch_ref":record.branch_ref,
            "creation_receipt_ref":record.creation_receipt_ref,"before_manifest_ref":record.before_manifest_ref,
            "current_manifest_ref":record.current_manifest_ref,"owner_token_fingerprint":record.owner_token_fingerprint,
            "state":record.state,"retention":record.retention,"evidence_hold":record.evidence_hold,
            "last_probe_ref":record.last_probe_ref,
        }),
    )?;
    Ok(record)
}

pub fn seal_merge_plan(
    mut plan: MergePlanV2,
    overlap: &OverlapAnalysis,
) -> Result<MergePlanV2, DevelopmentError> {
    if plan.schema_id != MERGE_PLAN_V2_SCHEMA_ID
        || plan.schema_version != 2
        || plan.revision == 0
        || !token(&plan.merge_plan_id, 192)
        || plan.inputs.is_empty()
        || !valid_git_oid_any(&plan.target_base_commit_oid)
        || plan.overlap_analysis_ref != overlap.overlap_analysis_id
        || !overlap.merge_ready
    {
        return Err(DevelopmentError::Invalid);
    }
    normalize_strings(&mut plan.inputs);
    normalize_strings(&mut plan.order);
    normalize_strings(&mut plan.dependency_refs);
    plan.plan_fingerprint = fingerprint(
        MERGE_PLAN_V2_SCHEMA_ID,
        &serde_json::json!({
            "merge_plan_id":plan.merge_plan_id,"revision":plan.revision,"previous_revision_ref":plan.previous_revision_ref,
            "change_bundle_ref":plan.change_bundle_ref,"participant_ref":plan.participant_ref,"project_id":plan.project_id,
            "repository_fingerprint":plan.repository_fingerprint,"integration_worktree_ref":plan.integration_worktree_ref,
            "target_ref":plan.target_ref,"target_base_commit_oid":plan.target_base_commit_oid,"inputs":plan.inputs,
            "strategy":plan.strategy,"order":plan.order,"dependency_refs":plan.dependency_refs,
            "overlap_analysis_ref":plan.overlap_analysis_ref,"conflict_policy":plan.conflict_policy,
            "validation_plan_ref":plan.validation_plan_ref,"rollback_plan_ref":plan.rollback_plan_ref,
            "permission_plan_ref":plan.permission_plan_ref,"status":plan.status,
        }),
    )?;
    Ok(plan)
}

pub fn seal_merge_queue(mut queue: MergeQueueRecord) -> Result<MergeQueueRecord, DevelopmentError> {
    if queue.schema_id != MERGE_QUEUE_RECORD_SCHEMA_ID
        || queue.schema_version != 1
        || queue.revision == 0
        || !token(&queue.merge_queue_id, 192)
        || queue.entries.is_empty()
        || !valid_git_oid_any(&queue.current_base_commit_oid)
        || queue
            .entries
            .iter()
            .filter(|entry| {
                matches!(
                    entry.state,
                    MergeQueueEntryState::Integrating | MergeQueueEntryState::Validating
                )
            })
            .count()
            > 1
    {
        return Err(DevelopmentError::Invalid);
    }
    let ids = queue
        .entries
        .iter()
        .map(|entry| entry.entry_id.as_str())
        .collect::<BTreeSet<_>>();
    if ids.len() != queue.entries.len()
        || queue.entries.iter().any(|entry| {
            !token(&entry.entry_id, 192)
                || !valid_git_oid_any(&entry.expected_predecessor_commit_oid)
                || entry
                    .dependency_entry_refs
                    .iter()
                    .any(|dependency| !ids.contains(dependency.as_str()))
        })
    {
        return Err(DevelopmentError::Conflict);
    }
    if let Some(active) = &queue.active_entry_ref
        && !ids.contains(active.as_str())
    {
        return Err(DevelopmentError::Conflict);
    }
    queue.queue_fingerprint = fingerprint(
        MERGE_QUEUE_RECORD_SCHEMA_ID,
        &serde_json::json!({
            "merge_queue_id":queue.merge_queue_id,"revision":queue.revision,"previous_revision_ref":queue.previous_revision_ref,
            "project_id":queue.project_id,"repository_fingerprint":queue.repository_fingerprint,
            "integration_target_ref":queue.integration_target_ref,"current_base_commit_oid":queue.current_base_commit_oid,
            "entries":queue.entries,"active_entry_ref":queue.active_entry_ref,"repository_lock_ref":queue.repository_lock_ref,
            "resource_reservation_ref":queue.resource_reservation_ref,
        }),
    )?;
    Ok(queue)
}

pub fn seal_merge_conflict(
    mut record: MergeConflictRecord,
) -> Result<MergeConflictRecord, DevelopmentError> {
    if record.schema_id != MERGE_CONFLICT_RECORD_SCHEMA_ID
        || record.schema_version != 1
        || record.revision == 0
        || !token(&record.conflict_id, 192)
        || record.conflict_items.is_empty()
        || !valid_git_oid_any(&record.base_commit_oid)
        || !valid_git_oid_any(&record.left_revision)
        || !valid_git_oid_any(&record.right_revision)
        || record.resolution_class == ConflictResolutionClass::MechanicalSafe
            && record.conflict_items.iter().any(|item| {
                matches!(
                    item.axis,
                    OverlapAxis::Contract
                        | OverlapAxis::Generated
                        | OverlapAxis::Dependency
                        | OverlapAxis::RepositoryPolicy
                        | OverlapAxis::Rename
                )
            })
    {
        return Err(DevelopmentError::Invalid);
    }
    record.conflict_fingerprint = fingerprint(
        MERGE_CONFLICT_RECORD_SCHEMA_ID,
        &serde_json::json!({
            "conflict_id":record.conflict_id,"revision":record.revision,"project_id":record.project_id,
            "merge_plan_ref":record.merge_plan_ref,"queue_entry_refs":record.queue_entry_refs,
            "base_commit_oid":record.base_commit_oid,"left_revision":record.left_revision,"right_revision":record.right_revision,
            "conflict_items":record.conflict_items,"left_intent_refs":record.left_intent_refs,
            "right_intent_refs":record.right_intent_refs,"contract_refs":record.contract_refs,
            "raw_conflict_artifact_ref":record.raw_conflict_artifact_ref,"resolution_class":record.resolution_class,
            "resolution_decision_ref":record.resolution_decision_ref,"resolution_patch_set_ref":record.resolution_patch_set_ref,
            "revalidation_refs":record.revalidation_refs,"state":record.state,
        }),
    )?;
    Ok(record)
}

pub fn seal_project_merge_result(
    mut result: ProjectMergeResult,
) -> Result<ProjectMergeResult, DevelopmentError> {
    if result.schema_id != PROJECT_MERGE_RESULT_SCHEMA_ID
        || result.schema_version != 1
        || result.revision == 0
        || !token(&result.project_merge_result_id, 192)
        || !valid_git_oid_any(&result.integration_before_commit_oid)
        || result
            .integration_after_commit_oid
            .as_ref()
            .is_some_and(|oid| !valid_git_oid_any(oid))
        || matches!(
            result.result,
            ProjectMergeResultState::LocalCommit | ProjectMergeResultState::LocalBranchUpdated
        ) && result.integration_after_commit_oid.is_none()
        || result.local_branch_updated && result.branch_update_approval_ref.is_none()
        || result.result == ProjectMergeResultState::OutcomeUnknown
            && result.integration_after_commit_oid.is_some()
    {
        return Err(DevelopmentError::Invalid);
    }
    result.result_fingerprint = fingerprint(
        PROJECT_MERGE_RESULT_SCHEMA_ID,
        &serde_json::json!({
            "project_merge_result_id":result.project_merge_result_id,"revision":result.revision,"project_id":result.project_id,
            "repository_fingerprint":result.repository_fingerprint,"merge_plan_ref":result.merge_plan_ref,
            "queue_entry_ref":result.queue_entry_ref,"integration_before_commit_oid":result.integration_before_commit_oid,
            "integration_after_commit_oid":result.integration_after_commit_oid,"working_tree_snapshot_ref":result.working_tree_snapshot_ref,
            "actual_strategy":result.actual_strategy,"commit_parent_oids":result.commit_parent_oids,
            "adapter_receipt_ref":result.adapter_receipt_ref,"preexisting_change_preservation_ref":result.preexisting_change_preservation_ref,
            "actual_change_set_ref":result.actual_change_set_ref,"scope_deviation_refs":result.scope_deviation_refs,
            "validation_plan_ref":result.validation_plan_ref,"gate_decision_ref":result.gate_decision_ref,
            "evidence_bundle_ref":result.evidence_bundle_ref,"local_branch_updated":result.local_branch_updated,
            "branch_update_approval_ref":result.branch_update_approval_ref,"rollback_capabilities":result.rollback_capabilities,
            "result":result.result,
        }),
    )?;
    Ok(result)
}

pub fn seal_remote_snapshot(
    mut snapshot: RemoteStateSnapshotV2,
) -> Result<RemoteStateSnapshotV2, DevelopmentError> {
    if snapshot.schema_id != REMOTE_STATE_SNAPSHOT_V2_SCHEMA_ID
        || snapshot.schema_version != 2
        || snapshot.revision == 0
        || !token(&snapshot.remote_snapshot_id, 192)
        || snapshot.remote_identity.trim().is_empty()
        || snapshot.adapter_descriptor_ref.trim().is_empty()
        || snapshot.query_scope.is_empty()
        || snapshot.captured_at.trim().is_empty()
        || snapshot.valid_until.trim().is_empty()
        || snapshot
            .refs
            .iter()
            .any(|item| !valid_git_oid_any(&item.object_id))
    {
        return Err(DevelopmentError::Invalid);
    }
    snapshot
        .refs
        .sort_by(|left, right| left.provider_ref.cmp(&right.provider_ref));
    snapshot.snapshot_fingerprint = fingerprint(
        REMOTE_STATE_SNAPSHOT_V2_SCHEMA_ID,
        &serde_json::json!({
            "remote_snapshot_id":snapshot.remote_snapshot_id,"revision":snapshot.revision,"project_id":snapshot.project_id,
            "remote_kind":snapshot.remote_kind,"adapter_descriptor_ref":snapshot.adapter_descriptor_ref,
            "remote_identity":snapshot.remote_identity,"local_subject_ref":snapshot.local_subject_ref,
            "query_scope":snapshot.query_scope,"refs":snapshot.refs,"pull_requests":snapshot.pull_requests,
            "checks":snapshot.checks,"releases":snapshot.releases,"capabilities":snapshot.capabilities,
            "captured_at":snapshot.captured_at,"valid_until":snapshot.valid_until,"completeness":snapshot.completeness,
            "limitations":snapshot.limitations,"raw_artifact_ref":snapshot.raw_artifact_ref,
        }),
    )?;
    Ok(snapshot)
}

pub fn seal_remote_operation(
    mut operation: RemoteOperationRecord,
) -> Result<RemoteOperationRecord, DevelopmentError> {
    if operation.schema_id != REMOTE_OPERATION_RECORD_SCHEMA_ID
        || operation.schema_version != 1
        || operation.revision == 0
        || !token(&operation.remote_operation_id, 192)
        || !valid_git_oid_any(&operation.local_source_revision)
        || operation.target.trim().is_empty()
        || operation.idempotency_key.trim().is_empty()
        || operation.action == RemoteAction::Push
            && parse_git_push_target(&operation.target).is_err()
        || matches!(
            operation.state,
            RemoteOperationState::Executing
                | RemoteOperationState::Succeeded
                | RemoteOperationState::Failed
                | RemoteOperationState::OutcomeUnknown
                | RemoteOperationState::Reconciled
        ) && operation.approval_request_ref.is_none()
        || matches!(
            operation.state,
            RemoteOperationState::Succeeded | RemoteOperationState::Reconciled
        ) && operation.after_snapshot_ref.is_none()
        || operation.state == RemoteOperationState::Succeeded
            && operation.adapter_receipt_ref.is_none()
    {
        return Err(DevelopmentError::Invalid);
    }
    operation.request_fingerprint = fingerprint(
        "star.remote-operation-request",
        &serde_json::json!({
            "remote_operation_id":operation.remote_operation_id,"project_id":operation.project_id,
            "change_bundle_ref":operation.change_bundle_ref,"participant_ref":operation.participant_ref,
            "action":operation.action,"before_snapshot_ref":operation.before_snapshot_ref,
            "local_source_revision":operation.local_source_revision,"target":operation.target,
            "expected_remote_precondition":operation.expected_remote_precondition,
            "permission_plan_ref":operation.permission_plan_ref,"idempotency_key":operation.idempotency_key,
        }),
    )?;
    operation.operation_fingerprint = fingerprint(
        REMOTE_OPERATION_RECORD_SCHEMA_ID,
        &serde_json::json!({
            "remote_operation_id":operation.remote_operation_id,"revision":operation.revision,"project_id":operation.project_id,
            "change_bundle_ref":operation.change_bundle_ref,"participant_ref":operation.participant_ref,"action":operation.action,
            "before_snapshot_ref":operation.before_snapshot_ref,"local_source_revision":operation.local_source_revision,
            "target":operation.target,"expected_remote_precondition":operation.expected_remote_precondition,
            "permission_plan_ref":operation.permission_plan_ref,"approval_request_ref":operation.approval_request_ref,
            "idempotency_key":operation.idempotency_key,"request_fingerprint":operation.request_fingerprint,
            "adapter_receipt_ref":operation.adapter_receipt_ref,"after_snapshot_ref":operation.after_snapshot_ref,
            "state":operation.state,"diagnostic_refs":operation.diagnostic_refs,
        }),
    )?;
    Ok(operation)
}

/// Parses the canonical target used by the built-in Git push adapter.
///
/// The target deliberately binds both the configured remote name and the full
/// Git ref so neither can be supplied out-of-band after approval.
pub fn parse_git_push_target(target: &str) -> Result<(&str, &str), DevelopmentError> {
    let (remote_name, target_ref) = target
        .strip_prefix("git:")
        .and_then(|value| value.split_once(':'))
        .ok_or(DevelopmentError::Invalid)?;
    if !token(remote_name, 128) || !valid_remote_ref(target_ref) {
        return Err(DevelopmentError::Invalid);
    }
    Ok((remote_name, target_ref))
}

pub fn seal_release_handoff(
    mut handoff: ChangeBundleReleaseHandoff,
) -> Result<ChangeBundleReleaseHandoff, DevelopmentError> {
    if handoff.schema_id != CHANGE_BUNDLE_RELEASE_HANDOFF_SCHEMA_ID
        || handoff.schema_version != 1
        || handoff.revision == 0
        || !token(&handoff.release_handoff_id, 192)
        || handoff.project_inputs.is_empty()
    {
        return Err(DevelopmentError::Invalid);
    }
    handoff
        .project_inputs
        .sort_by(|left, right| left.project_id.cmp(&right.project_id));
    if handoff
        .project_inputs
        .windows(2)
        .any(|pair| pair[0].project_id == pair[1].project_id)
        || handoff
            .project_inputs
            .iter()
            .any(|input| !valid_git_oid(&input.commit_oid, &input.git_object_format))
    {
        return Err(DevelopmentError::Conflict);
    }
    let project_ids = handoff
        .project_inputs
        .iter()
        .map(|input| &input.project_id)
        .collect::<BTreeSet<_>>();
    if handoff.dependency_order.len() != project_ids.len()
        || handoff.dependency_order.iter().collect::<BTreeSet<_>>() != project_ids
    {
        return Err(DevelopmentError::Conflict);
    }
    handoff.ready = handoff.completion_level_reached >= handoff.completion_target
        && handoff.remaining_risks.is_empty()
        && handoff.limitations.is_empty()
        && handoff
            .project_inputs
            .iter()
            .all(|input| input.unresolved_risks.is_empty())
        && (handoff.completion_target < CompletionLevel::RemoteMerged
            || handoff.project_inputs.iter().all(|input| {
                input.remote_merged_commit_oid.as_ref() == Some(&input.commit_oid)
                    && input.remote_snapshot_ref.is_some()
            }));
    handoff.handoff_fingerprint = fingerprint(
        CHANGE_BUNDLE_RELEASE_HANDOFF_SCHEMA_ID,
        &serde_json::json!({
            "release_handoff_id":handoff.release_handoff_id,"revision":handoff.revision,
            "change_bundle_ref":handoff.change_bundle_ref,"multi_project_goal_ref":handoff.multi_project_goal_ref,
            "completion_target":handoff.completion_target,"completion_level_reached":handoff.completion_level_reached,
            "project_inputs":handoff.project_inputs,"dependency_order":handoff.dependency_order,
            "compatibility_windows":handoff.compatibility_windows,"overall_gate_ref":handoff.overall_gate_ref,
            "remaining_risks":handoff.remaining_risks,"limitations":handoff.limitations,"ready":handoff.ready,
        }),
    )?;
    Ok(handoff)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitRepositoryObservation {
    pub repository_fingerprint: Sha256Hash,
    pub object_format: String,
    pub head_commit_oid: String,
    pub dirty_state: DirtyState,
    pub status_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalEffectPermit {
    pub permission_decision_ref: String,
    pub gate_decision_ref: String,
    pub exact_plan_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitEffectReceipt {
    pub operation: String,
    pub before_commit_oid: String,
    pub after_commit_oid: Option<String>,
    pub status_fingerprint: Sha256Hash,
    pub state: String,
}

pub struct GitCoordinationAdapter;

impl GitCoordinationAdapter {
    pub fn observe(repository_root: &Path) -> Result<GitRepositoryObservation, DevelopmentError> {
        let common = git_stdout(
            repository_root,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?;
        let object_format = git_stdout(repository_root, &["rev-parse", "--show-object-format"])?;
        let head = git_stdout(repository_root, &["rev-parse", "HEAD"])?;
        if !valid_git_oid(&head, &object_format) {
            return Err(DevelopmentError::Invalid);
        }
        let status = git_bytes(repository_root, &["status", "--porcelain=v2", "-z"])?;
        Ok(GitRepositoryObservation {
            repository_fingerprint: Sha256Hash::digest(common.as_bytes()),
            object_format,
            head_commit_oid: head,
            dirty_state: if status.is_empty() {
                DirtyState::Clean
            } else {
                DirtyState::DirtyComplete
            },
            status_fingerprint: Sha256Hash::digest(&status),
        })
    }

    pub fn create_owned_worktree(
        repository_root: &Path,
        protected_parent: &Path,
        worktree_leaf: &str,
        branch_ref: &str,
        base_commit_oid: &str,
        expected_repository_fingerprint: &Sha256Hash,
        permit: &LocalEffectPermit,
    ) -> Result<(PathBuf, GitEffectReceipt), DevelopmentError> {
        if !token(worktree_leaf, 128)
            || !valid_star_branch(branch_ref)
            || !valid_git_oid_any(base_commit_oid)
            || permit.permission_decision_ref.trim().is_empty()
            || permit.gate_decision_ref.trim().is_empty()
        {
            return Err(DevelopmentError::Invalid);
        }
        let before = Self::observe(repository_root)?;
        if &before.repository_fingerprint != expected_repository_fingerprint {
            return Err(DevelopmentError::Conflict);
        }
        fs::create_dir_all(protected_parent).map_err(|_| DevelopmentError::Adapter)?;
        let protected_parent =
            fs::canonicalize(protected_parent).map_err(|_| DevelopmentError::Adapter)?;
        let canonical_target = protected_parent.join(worktree_leaf);
        if canonical_target.exists()
            || canonical_target.parent() != Some(protected_parent.as_path())
        {
            return Err(DevelopmentError::Conflict);
        }
        let target = git_compatible_windows_path(&canonical_target);
        let output = Command::new("git")
            .current_dir(repository_root)
            .args([
                "worktree",
                "add",
                "-b",
                branch_ref,
                target.to_string_lossy().as_ref(),
                base_commit_oid,
            ])
            .output()
            .map_err(|_| DevelopmentError::Adapter)?;
        if !output.status.success() {
            return Err(DevelopmentError::Conflict);
        }
        let after = Self::observe(&target)?;
        if after.head_commit_oid != base_commit_oid
            || after.dirty_state != DirtyState::Clean
            || after.repository_fingerprint != before.repository_fingerprint
        {
            return Err(DevelopmentError::Conflict);
        }
        Ok((
            target,
            GitEffectReceipt {
                operation: "git_worktree_add".to_owned(),
                before_commit_oid: before.head_commit_oid,
                after_commit_oid: Some(after.head_commit_oid),
                status_fingerprint: after.status_fingerprint,
                state: "succeeded".to_owned(),
            },
        ))
    }

    pub fn merge_in_owned_worktree(
        integration_worktree: &Path,
        expected_head: &str,
        input_commit_oid: &str,
        strategy: MergeStrategyV2,
        expected_plan_fingerprint: &Sha256Hash,
        permit: &LocalEffectPermit,
    ) -> Result<GitEffectReceipt, DevelopmentError> {
        if permit.permission_decision_ref.trim().is_empty()
            || permit.gate_decision_ref.trim().is_empty()
            || &permit.exact_plan_fingerprint != expected_plan_fingerprint
            || !valid_git_oid_any(expected_head)
            || !valid_git_oid_any(input_commit_oid)
            || strategy == MergeStrategyV2::ApplyPatch
        {
            return Err(DevelopmentError::Invalid);
        }
        let before = Self::observe(integration_worktree)?;
        if before.head_commit_oid != expected_head || before.dirty_state != DirtyState::Clean {
            return Err(DevelopmentError::Conflict);
        }
        let args = match strategy {
            MergeStrategyV2::FastForwardOnly => vec!["merge", "--ff-only", input_commit_oid],
            MergeStrategyV2::MergeCommit => {
                vec!["merge", "--no-ff", "--no-commit", input_commit_oid]
            }
            MergeStrategyV2::Squash => vec!["merge", "--squash", "--no-commit", input_commit_oid],
            MergeStrategyV2::ApplyPatch => unreachable!(),
        };
        let output = Command::new("git")
            .current_dir(integration_worktree)
            .args(args)
            .output()
            .map_err(|_| DevelopmentError::Adapter)?;
        let after = Self::observe(integration_worktree)?;
        Ok(GitEffectReceipt {
            operation: "git_merge".to_owned(),
            before_commit_oid: before.head_commit_oid,
            after_commit_oid: Some(after.head_commit_oid),
            status_fingerprint: after.status_fingerprint,
            state: if output.status.success() {
                "succeeded".to_owned()
            } else {
                "conflicted_or_failed".to_owned()
            },
        })
    }

    pub fn observe_remote_refs(
        project_id: ProjectId,
        repository_root: &Path,
        remote_name: &str,
        snapshot_id: String,
        revision: u64,
        captured_at: String,
        valid_until: String,
    ) -> Result<RemoteStateSnapshotV2, DevelopmentError> {
        if !token(remote_name, 128) || !token(&snapshot_id, 192) {
            return Err(DevelopmentError::Invalid);
        }
        let url = git_stdout(repository_root, &["remote", "get-url", remote_name])?;
        let output = Command::new("git")
            .current_dir(repository_root)
            .args(["ls-remote", "--heads", "--tags", remote_name])
            .output()
            .map_err(|_| DevelopmentError::Adapter)?;
        if !output.status.success() {
            return Err(DevelopmentError::Adapter);
        }
        let text = std::str::from_utf8(&output.stdout).map_err(|_| DevelopmentError::Invalid)?;
        let mut refs = Vec::new();
        for line in text.lines() {
            let Some((oid, provider_ref)) = line.split_once('\t') else {
                return Err(DevelopmentError::Invalid);
            };
            if !valid_git_oid_any(oid) || !provider_ref.starts_with("refs/") {
                return Err(DevelopmentError::Invalid);
            }
            refs.push(RemoteRefObservation {
                provider_ref: provider_ref.to_owned(),
                object_id: oid.to_owned(),
                object_kind: "git_ref".to_owned(),
            });
        }
        let remote_identity = format!(
            "git:{}",
            Sha256Hash::digest(redact_remote_url(&url).as_bytes())
        );
        seal_remote_snapshot(RemoteStateSnapshotV2 {
            schema_id: REMOTE_STATE_SNAPSHOT_V2_SCHEMA_ID.to_owned(),
            schema_version: 2,
            remote_snapshot_id: snapshot_id,
            revision,
            project_id,
            remote_kind: "git".to_owned(),
            adapter_descriptor_ref: "builtin.git-cli.ls-remote.v1".to_owned(),
            remote_identity,
            local_subject_ref: Self::observe(repository_root)?.head_commit_oid,
            query_scope: vec!["refs".to_owned()],
            refs,
            pull_requests: vec![],
            checks: vec![],
            releases: vec![],
            capabilities: BTreeMap::from([
                ("observe_refs".to_owned(), true),
                ("push".to_owned(), true),
            ]),
            captured_at,
            valid_until,
            completeness: CoverageState::Complete,
            limitations: vec!["git_cli_does_not_observe_pr_checks_or_releases".to_owned()],
            raw_artifact_ref: None,
            snapshot_fingerprint: placeholder(),
        })
    }

    pub fn push_approved_ref(
        repository_root: &Path,
        remote_name: &str,
        source_commit_oid: &str,
        target_ref: &str,
        approved_operation: &RemoteOperationRecord,
        exact_operation_fingerprint: &Sha256Hash,
    ) -> Result<GitEffectReceipt, DevelopmentError> {
        let operation = seal_remote_operation(approved_operation.clone())?;
        let approved_target = parse_git_push_target(&operation.target)?;
        if operation.action != RemoteAction::Push
            || operation.state != RemoteOperationState::Executing
            || operation.approval_request_ref.is_none()
            || &operation.operation_fingerprint != exact_operation_fingerprint
            || operation.local_source_revision != source_commit_oid
            || !valid_remote_ref(target_ref)
            || !token(remote_name, 128)
            || approved_target != (remote_name, target_ref)
        {
            return Err(DevelopmentError::Invalid);
        }
        let commit_expression = format!("{source_commit_oid}^{{commit}}");
        let resolved_source = git_stdout(
            repository_root,
            &["rev-parse", "--verify", commit_expression.as_str()],
        )?;
        if resolved_source != source_commit_oid {
            return Err(DevelopmentError::Conflict);
        }
        let before = Self::observe(repository_root)?;
        let refspec = format!("{source_commit_oid}:{target_ref}");
        let output = Command::new("git")
            .current_dir(repository_root)
            .args(["push", "--porcelain", remote_name, &refspec])
            .output()
            .map_err(|_| DevelopmentError::Adapter)?;
        let after = Self::observe(repository_root)?;
        Ok(GitEffectReceipt {
            operation: "git_push".to_owned(),
            before_commit_oid: before.head_commit_oid,
            after_commit_oid: Some(after.head_commit_oid),
            status_fingerprint: Sha256Hash::digest(&output.stdout),
            state: if output.status.success() {
                "request_accepted_requires_remote_refresh".to_owned()
            } else {
                "failed_or_outcome_unknown".to_owned()
            },
        })
    }
}

fn aggregate_bundle_state(participants: &[ChangeBundleParticipantV2]) -> BundleAggregateState {
    let required = participants
        .iter()
        .filter(|item| item.required)
        .collect::<Vec<_>>();
    if required
        .iter()
        .any(|item| item.state == ParticipantState::OutcomeUnknown)
    {
        return BundleAggregateState::OutcomeUnknown;
    }
    if required
        .iter()
        .any(|item| item.state == ParticipantState::RollbackRequired)
    {
        return BundleAggregateState::RollbackRequired;
    }
    if required
        .iter()
        .any(|item| item.state == ParticipantState::Held)
    {
        return BundleAggregateState::Held;
    }
    if required
        .iter()
        .any(|item| item.state == ParticipantState::PartiallyApplied)
    {
        return BundleAggregateState::PartiallyApplied;
    }
    if required.iter().any(|item| {
        matches!(
            item.state,
            ParticipantState::Applying | ParticipantState::Merging
        )
    }) {
        return BundleAggregateState::Applying;
    }
    if required
        .iter()
        .any(|item| item.state == ParticipantState::Validating)
    {
        return BundleAggregateState::Validating;
    }
    if required
        .iter()
        .any(|item| item.state == ParticipantState::AwaitingValidation)
    {
        return BundleAggregateState::AwaitingValidation;
    }
    if required
        .iter()
        .any(|item| item.state == ParticipantState::AwaitingApply)
    {
        return BundleAggregateState::AwaitingApply;
    }
    if required
        .iter()
        .any(|item| item.state == ParticipantState::Preparing)
    {
        return BundleAggregateState::Preparing;
    }
    if required
        .iter()
        .any(|item| item.state == ParticipantState::Failed)
    {
        return BundleAggregateState::Failed;
    }
    if required
        .iter()
        .any(|item| item.state == ParticipantState::Cancelled)
    {
        return BundleAggregateState::Cancelled;
    }
    if required
        .iter()
        .all(|item| item.state == ParticipantState::Completed)
    {
        return BundleAggregateState::Completed;
    }
    BundleAggregateState::Prepared
}

fn completion_level(
    participants: &[ChangeBundleParticipantV2],
    _remote_policy: star_contracts::coordination_v2::RemotePolicy,
) -> CompletionLevel {
    let required = participants
        .iter()
        .filter(|item| item.required)
        .collect::<Vec<_>>();
    if required.iter().all(|item| {
        matches!(
            item.state,
            ParticipantState::LocalCompleted
                | ParticipantState::RemotePending
                | ParticipantState::Completed
        ) && item.project_merge_result_ref.is_some()
    }) {
        return CompletionLevel::LocalIntegrated;
    }
    if required.iter().all(|item| {
        matches!(
            item.state,
            ParticipantState::MergeReady
                | ParticipantState::LocalCompleted
                | ParticipantState::RemotePending
                | ParticipantState::Completed
        ) && !item.gate_decision_refs.is_empty()
    }) {
        return CompletionLevel::ValidatedParticipants;
    }
    CompletionLevel::None
}

fn topological_order(
    ids: &[String],
    edges: &[BundleStepEdge],
) -> Result<Vec<String>, DevelopmentError> {
    let set = ids.iter().map(String::as_str).collect::<BTreeSet<_>>();
    if set.len() != ids.len() {
        return Err(DevelopmentError::Conflict);
    }
    let mut indegree = set
        .iter()
        .map(|id| (*id, 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<&str, Vec<&str>>::new();
    for edge in edges {
        if edge.from_step_id == edge.to_step_id
            || !set.contains(edge.from_step_id.as_str())
            || !set.contains(edge.to_step_id.as_str())
        {
            return Err(DevelopmentError::Conflict);
        }
        *indegree
            .get_mut(edge.to_step_id.as_str())
            .ok_or(DevelopmentError::Conflict)? += 1;
        outgoing
            .entry(edge.from_step_id.as_str())
            .or_default()
            .push(edge.to_step_id.as_str());
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(id, _)| *id)
        .collect::<VecDeque<_>>();
    let mut order = Vec::new();
    while let Some(id) = ready.pop_front() {
        order.push(id.to_owned());
        let mut children = outgoing.get(id).cloned().unwrap_or_default();
        children.sort();
        children.dedup();
        for child in children {
            let count = indegree.get_mut(child).ok_or(DevelopmentError::Conflict)?;
            *count -= 1;
            if *count == 0 {
                let index = ready
                    .iter()
                    .position(|item| *item > child)
                    .unwrap_or(ready.len());
                ready.insert(index, child);
            }
        }
    }
    if order.len() != ids.len() {
        return Err(DevelopmentError::Conflict);
    }
    Ok(order)
}

fn overlap_item(
    left: &OverlapSubject,
    right: &OverlapSubject,
    axis: OverlapAxis,
    subject_ref: &str,
    disposition: OverlapDisposition,
    reason: &str,
) -> OverlapItem {
    OverlapItem {
        left_participant_ref: left.participant_ref.clone(),
        right_participant_ref: right.participant_ref.clone(),
        axis,
        subject_ref: subject_ref.to_owned(),
        disposition,
        reason: reason.to_owned(),
        evidence_refs: vec![],
    }
}

fn intersection(left: &[String], right: &[String]) -> Vec<String> {
    let left = left.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let right = right.iter().map(String::as_str).collect::<BTreeSet<_>>();
    left.intersection(&right)
        .map(|value| (*value).to_owned())
        .collect()
}

fn overlap_rank(value: OverlapDisposition) -> u8 {
    match value {
        OverlapDisposition::Disjoint => 0,
        OverlapDisposition::OrderedOverlap => 1,
        OverlapDisposition::ConflictPossible => 2,
        OverlapDisposition::ConflictConfirmed => 3,
        OverlapDisposition::Unknown => 4,
    }
}

fn valid_budget(budget: &star_contracts::coordination_v2::ResourceBudget) -> bool {
    budget.max_parallel_projects > 0
        && budget.max_active_worktrees > 0
        && budget.max_concurrent_writes > 0
        && budget.max_processes > 0
        && budget.memory_limit_bytes > 0
        && budget.worktree_disk_limit_bytes > 0
        && budget.artifact_limit_bytes > 0
        && budget.wall_time_limit_ms > 0
}

fn valid_git_oid(value: &str, object_format: &str) -> bool {
    let expected = match object_format {
        "sha1" => 40,
        "sha256" => 64,
        _ => return false,
    };
    value.len() == expected
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
fn valid_git_oid_any(value: &str) -> bool {
    valid_git_oid(value, "sha1") || valid_git_oid(value, "sha256")
}
fn valid_star_branch(value: &str) -> bool {
    value.starts_with("star/")
        && value.len() <= 240
        && !value.contains("..")
        && !value.contains('\0')
}
fn valid_remote_ref(value: &str) -> bool {
    value.starts_with("refs/heads/")
        && value.len() <= 512
        && !value.contains("..")
        && !value.contains('\0')
}
fn normalize_strings(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

fn git_stdout(root: &Path, args: &[&str]) -> Result<String, DevelopmentError> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|_| DevelopmentError::Adapter)?;
    if !output.status.success() {
        return Err(DevelopmentError::Adapter);
    }
    let value = std::str::from_utf8(&output.stdout)
        .map_err(|_| DevelopmentError::Invalid)?
        .trim();
    if value.is_empty() {
        return Err(DevelopmentError::Invalid);
    }
    Ok(value.to_owned())
}
fn git_bytes(root: &Path, args: &[&str]) -> Result<Vec<u8>, DevelopmentError> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|_| DevelopmentError::Adapter)?;
    if !output.status.success() {
        return Err(DevelopmentError::Adapter);
    }
    Ok(output.stdout)
}
fn redact_remote_url(value: &str) -> String {
    if let Some((scheme, rest)) = value.split_once("://")
        && let Some((_, host_path)) = rest.rsplit_once('@')
    {
        return format!("{scheme}://{host_path}");
    }
    value.to_owned()
}

fn git_compatible_windows_path(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::{
        CheckoutId,
        coordination_v2::{
            BundleEdgeKind, BundleStep, BundleStepKind, GoalParticipant, ParticipantRole,
            ProjectRelation, ProjectRelationKind, RelationCertainty, ResourceBudget,
        },
    };

    fn budget() -> ResourceBudget {
        ResourceBudget {
            max_parallel_projects: 2,
            max_active_worktrees: 4,
            max_concurrent_writes: 1,
            max_processes: 4,
            cpu_weight_limit: 100,
            memory_limit_bytes: 1024,
            worktree_disk_limit_bytes: 1024,
            artifact_limit_bytes: 1024,
            wall_time_limit_ms: 1_000,
        }
    }

    #[test]
    fn relation_and_step_cycles_are_rejected() {
        let project_a = ProjectId::new();
        let project_b = ProjectId::new();
        let graph = BundleStepGraph {
            steps: vec![
                BundleStep {
                    step_id: "a".into(),
                    project_id: Some(project_a.clone()),
                    stage_ref: None,
                    step_kind: BundleStepKind::ProjectPatchApply,
                    input_refs: vec![],
                    output_refs: vec![],
                    expected_effect: "source_write".into(),
                    required_gate_refs: vec!["gate-a".into()],
                    completion_condition: "applied".into(),
                },
                BundleStep {
                    step_id: "b".into(),
                    project_id: Some(project_b.clone()),
                    stage_ref: None,
                    step_kind: BundleStepKind::ConsumerTransition,
                    input_refs: vec![],
                    output_refs: vec![],
                    expected_effect: "source_write".into(),
                    required_gate_refs: vec!["gate-b".into()],
                    completion_condition: "validated".into(),
                },
            ],
            edges: vec![
                BundleStepEdge {
                    from_step_id: "a".into(),
                    to_step_id: "b".into(),
                    edge_kind: BundleEdgeKind::ProviderBeforeConsumer,
                    reason: "contract".into(),
                    evidence_refs: vec![],
                },
                BundleStepEdge {
                    from_step_id: "b".into(),
                    to_step_id: "a".into(),
                    edge_kind: BundleEdgeKind::Requires,
                    reason: "invalid".into(),
                    evidence_refs: vec![],
                },
            ],
            topological_order: vec![],
            graph_fingerprint: placeholder(),
        };
        assert!(seal_step_graph(graph).is_err());
        let _ = (
            CheckoutId::new(),
            GoalParticipant {
                project_id: project_a.clone(),
                required: true,
                roles: vec![ParticipantRole::Provider],
                source_of_truth_refs: vec!["source".into()],
            },
            ProjectRelation {
                relation_id: "provider-consumer".into(),
                provider_project_id: project_a,
                consumer_project_id: project_b,
                relation_kind: ProjectRelationKind::Api,
                contract_refs: vec!["contract".into()],
                accepted_versions: vec!["v1".into()],
                minimum_provider_version: None,
                certainty: RelationCertainty::Confirmed,
                evidence_refs: vec!["evidence".into()],
                freshness: CoverageState::Complete,
                limitations: vec![],
            },
            budget(),
        );
    }

    #[test]
    fn incomplete_overlap_blocks_parallel_dispatch() {
        let subjects = vec![
            OverlapSubject {
                participant_ref: "a".into(),
                project_id: ProjectId::new(),
                repository_fingerprint: Sha256Hash::digest(b"a"),
                file_refs: vec!["src/lib.rs".into()],
                rename_refs: vec![],
                range_refs: vec![],
                symbol_refs: vec![],
                contract_refs: vec![],
                generated_owner_refs: vec![],
                dependency_refs: vec![],
                repository_policy_refs: vec![],
                coverage: CoverageState::Complete,
            },
            OverlapSubject {
                participant_ref: "b".into(),
                project_id: ProjectId::new(),
                repository_fingerprint: Sha256Hash::digest(b"b"),
                file_refs: vec!["src/lib.rs".into()],
                rename_refs: vec![],
                range_refs: vec![],
                symbol_refs: vec![],
                contract_refs: vec![],
                generated_owner_refs: vec![],
                dependency_refs: vec![],
                repository_policy_refs: vec![],
                coverage: CoverageState::Partial,
            },
        ];
        let analysis = analyze_overlap(
            "overlap-one".into(),
            1,
            "bundle-one".into(),
            subjects,
            &BTreeSet::new(),
        )
        .unwrap();
        assert_eq!(analysis.overall, OverlapDisposition::Unknown);
        assert!(!analysis.parallel_safe);
        assert!(!analysis.merge_ready);
    }

    #[test]
    fn remote_operation_recomputes_request_identity_and_requires_approval_evidence() {
        let mut operation = RemoteOperationRecord {
            schema_id: REMOTE_OPERATION_RECORD_SCHEMA_ID.to_owned(),
            schema_version: 1,
            remote_operation_id: "remote-operation-one".to_owned(),
            revision: 1,
            project_id: ProjectId::new(),
            change_bundle_ref: "bundle-one".to_owned(),
            participant_ref: "participant-one".to_owned(),
            action: RemoteAction::Push,
            before_snapshot_ref: "remote-snapshot-one".to_owned(),
            local_source_revision: "a".repeat(40),
            target: "git:origin:refs/heads/main".to_owned(),
            expected_remote_precondition: "b".repeat(40),
            permission_plan_ref: "permission-plan-one".to_owned(),
            approval_request_ref: None,
            idempotency_key: "push-once".to_owned(),
            request_fingerprint: Sha256Hash::digest(b"caller supplied value is ignored"),
            adapter_receipt_ref: None,
            after_snapshot_ref: None,
            state: RemoteOperationState::Planned,
            diagnostic_refs: vec![],
            operation_fingerprint: placeholder(),
        };
        let supplied = operation.request_fingerprint.clone();
        let planned = seal_remote_operation(operation.clone()).unwrap();
        assert_ne!(planned.request_fingerprint, supplied);
        assert_eq!(
            parse_git_push_target(&planned.target).unwrap(),
            ("origin", "refs/heads/main")
        );

        operation.state = RemoteOperationState::Executing;
        assert!(seal_remote_operation(operation.clone()).is_err());
        operation.approval_request_ref = Some("approval-one".to_owned());
        assert!(seal_remote_operation(operation.clone()).is_ok());

        operation.state = RemoteOperationState::Succeeded;
        assert!(seal_remote_operation(operation.clone()).is_err());
        operation.adapter_receipt_ref = Some("receipt-one".to_owned());
        operation.after_snapshot_ref = Some("remote-snapshot-two".to_owned());
        assert!(seal_remote_operation(operation).is_ok());
    }

    #[test]
    fn git_adapter_creates_owned_worktree_and_fast_forwards_only_with_exact_permit() {
        let root = std::env::temp_dir().join(format!(
            "star-coordination-v2-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        fs::create_dir_all(&root).unwrap();
        run_git(&root, &["init", "-b", "main"]);
        fs::write(root.join("tracked.txt"), b"base\n").unwrap();
        run_git(&root, &["add", "tracked.txt"]);
        run_git_with_identity(&root, &["commit", "-m", "base"]);
        let base = git_stdout(&root, &["rev-parse", "HEAD"]).unwrap();
        run_git(&root, &["checkout", "-b", "feature"]);
        fs::write(root.join("tracked.txt"), b"base\nfeature\n").unwrap();
        run_git(&root, &["add", "tracked.txt"]);
        run_git_with_identity(&root, &["commit", "-m", "feature"]);
        let feature = git_stdout(&root, &["rev-parse", "HEAD"]).unwrap();
        run_git(&root, &["checkout", "main"]);
        let observation = GitCoordinationAdapter::observe(&root).unwrap();
        let parent = std::env::temp_dir().join(format!(
            "star-owned-worktrees-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        let permit = LocalEffectPermit {
            permission_decision_ref: "permission:test".to_owned(),
            gate_decision_ref: "gate:test".to_owned(),
            exact_plan_fingerprint: Sha256Hash::digest(b"plan"),
        };
        let (worktree, creation) = GitCoordinationAdapter::create_owned_worktree(
            &root,
            &parent,
            "integration-one",
            "star/bundle/integration-one",
            &base,
            &observation.repository_fingerprint,
            &permit,
        )
        .unwrap();
        assert_eq!(creation.state, "succeeded");
        let merge = GitCoordinationAdapter::merge_in_owned_worktree(
            &worktree,
            &base,
            &feature,
            MergeStrategyV2::FastForwardOnly,
            &permit.exact_plan_fingerprint,
            &permit,
        )
        .unwrap();
        assert_eq!(merge.state, "succeeded");
        assert_eq!(merge.after_commit_oid.as_deref(), Some(feature.as_str()));

        let remote = std::env::temp_dir().join(format!(
            "star-coordination-remote-{}-{}",
            std::process::id(),
            ProjectId::new()
        ));
        let remote_text = remote.to_string_lossy().to_string();
        run_git(&root, &["init", "--bare", remote_text.as_str()]);
        run_git(&root, &["remote", "add", "origin", remote_text.as_str()]);
        run_git(&root, &["push", "origin", "main:refs/heads/main"]);
        let operation = seal_remote_operation(RemoteOperationRecord {
            schema_id: REMOTE_OPERATION_RECORD_SCHEMA_ID.to_owned(),
            schema_version: 1,
            remote_operation_id: "push-feature".to_owned(),
            revision: 2,
            project_id: ProjectId::new(),
            change_bundle_ref: "bundle-one".to_owned(),
            participant_ref: "participant-one".to_owned(),
            action: RemoteAction::Push,
            before_snapshot_ref: "remote-before".to_owned(),
            local_source_revision: feature.clone(),
            target: "git:origin:refs/heads/main".to_owned(),
            expected_remote_precondition: base,
            permission_plan_ref: "permission-plan-one".to_owned(),
            approval_request_ref: Some("approval-one".to_owned()),
            idempotency_key: "push-feature-once".to_owned(),
            request_fingerprint: placeholder(),
            adapter_receipt_ref: None,
            after_snapshot_ref: None,
            state: RemoteOperationState::Executing,
            diagnostic_refs: vec![],
            operation_fingerprint: placeholder(),
        })
        .unwrap();
        assert!(
            GitCoordinationAdapter::push_approved_ref(
                &root,
                "origin",
                &feature,
                "refs/heads/main",
                &operation,
                &Sha256Hash::digest(b"stale approval"),
            )
            .is_err()
        );
        let push = GitCoordinationAdapter::push_approved_ref(
            &root,
            "origin",
            &feature,
            "refs/heads/main",
            &operation,
            &operation.operation_fingerprint,
        )
        .unwrap();
        assert_eq!(push.state, "request_accepted_requires_remote_refresh");
        let remote_refs = GitCoordinationAdapter::observe_remote_refs(
            operation.project_id,
            &root,
            "origin",
            "remote-after".to_owned(),
            1,
            "2026-07-23T00:00:00Z".to_owned(),
            "2026-07-23T00:05:00Z".to_owned(),
        )
        .unwrap();
        assert!(
            remote_refs.refs.iter().any(|item| {
                item.provider_ref == "refs/heads/main" && item.object_id == feature
            })
        );
    }

    fn run_git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(root)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    fn run_git_with_identity(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(root)
            .args([
                "-c",
                "user.name=Star Test",
                "-c",
                "user.email=star@example.invalid",
            ])
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }
}
