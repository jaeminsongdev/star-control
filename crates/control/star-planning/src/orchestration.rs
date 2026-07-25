//! Pure StageGraph construction and immutable replanning.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use star_contracts::{
    GoalId, Sha256Hash, StageGraphId,
    stage::{
        STAGE_CONTRACT_VERSION, STAGE_GRAPH_SCHEMA_ID, StageContractError, StageGraphEdgeV1,
        StageGraphV1, StageParallelGroupV1, StageSpecV1, StageStateV1,
    },
};
use thiserror::Error;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageGraphPlanInput {
    pub goal_id: GoalId,
    pub plan_revision: u64,
    pub stages: Vec<StageSpecV1>,
    pub edges: Vec<StageGraphEdgeV1>,
    pub parallel_groups: Vec<StageParallelGroupV1>,
    pub critical_path: Vec<star_contracts::StageId>,
    pub integration_stage_id: Option<star_contracts::StageId>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StagePlanningError {
    #[error("stage plan is invalid")]
    Invalid,
    #[error("stage plan revision is stale")]
    StaleRevision,
    #[error("a completed stage was modified during replanning")]
    CompletedStageChanged,
    #[error("stage graph contract failed: {0}")]
    Contract(#[from] StageContractError),
}

pub fn build_stage_graph(input: StageGraphPlanInput) -> Result<StageGraphV1, StagePlanningError> {
    if input.plan_revision != 1
        || input.stages.is_empty()
        || input
            .stages
            .iter()
            .any(|stage| !matches!(stage.state, StageStateV1::Draft | StageStateV1::Ready))
    {
        return Err(StagePlanningError::Invalid);
    }
    assemble_stage_graph(input, None, None)
}

fn assemble_stage_graph(
    input: StageGraphPlanInput,
    previous_graph_fingerprint: Option<Sha256Hash>,
    replan_reason: Option<String>,
) -> Result<StageGraphV1, StagePlanningError> {
    let graph_id = StageGraphId::from_stable_bytes(
        format!("{}:stage-graph", input.goal_id.as_str()).as_bytes(),
    );
    StageGraphV1 {
        schema_id: STAGE_GRAPH_SCHEMA_ID.to_owned(),
        schema_version: STAGE_CONTRACT_VERSION,
        stage_graph_id: graph_id,
        goal_id: input.goal_id,
        plan_revision: input.plan_revision,
        stages: input.stages,
        edges: input.edges,
        parallel_groups: input.parallel_groups,
        critical_path: input.critical_path,
        integration_stage_id: input.integration_stage_id,
        previous_graph_fingerprint,
        replan_reason,
        graph_fingerprint: Sha256Hash::digest(b"unsealed-stage-graph"),
    }
    .seal()
    .map_err(StagePlanningError::from)
}

pub fn replan_stage_graph(
    previous: &StageGraphV1,
    input: StageGraphPlanInput,
    reason: &str,
) -> Result<StageGraphV1, StagePlanningError> {
    previous.verify()?;
    if input.goal_id != previous.goal_id
        || input.plan_revision != previous.plan_revision.saturating_add(1)
        || reason.trim().is_empty()
        || reason.len() > 2_048
    {
        return Err(StagePlanningError::StaleRevision);
    }
    let candidate = assemble_stage_graph(
        input,
        Some(previous.graph_fingerprint.clone()),
        Some(reason.trim().to_owned()),
    )?;
    if candidate.stage_graph_id != previous.stage_graph_id {
        return Err(StagePlanningError::Invalid);
    }
    let completed = previous
        .stages
        .iter()
        .filter(|stage| stage.state == StageStateV1::Completed)
        .map(|stage| (stage.stage_id.clone(), stage.stage_fingerprint.clone()))
        .collect::<BTreeSet<_>>();
    let preserved = candidate
        .stages
        .iter()
        .filter(|stage| stage.state == StageStateV1::Completed)
        .map(|stage| (stage.stage_id.clone(), stage.stage_fingerprint.clone()))
        .collect::<BTreeSet<_>>();
    if completed != preserved {
        return Err(StagePlanningError::CompletedStageChanged);
    }
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use star_contracts::{
        ProjectId, StageId,
        evidence::DocumentRef,
        stage::{StageEdgeRelationV1, StageExecutorKindV1, StageFailurePolicyV1, StageModeV1},
    };

    use super::*;

    fn stage(goal_id: &GoalId, label: &str, state: StageStateV1) -> StageSpecV1 {
        StageSpecV1 {
            schema_id: star_contracts::stage::STAGE_SPEC_SCHEMA_ID.to_owned(),
            schema_version: STAGE_CONTRACT_VERSION,
            stage_id: StageId::from_stable_bytes(
                format!("{}:{label}", goal_id.as_str()).as_bytes(),
            ),
            revision: 1,
            goal_id: goal_id.clone(),
            task_spec_ref: None,
            scope_revision_ref: None,
            title: label.to_owned(),
            objective: format!("complete {label}"),
            stage_mode: StageModeV1::Plan,
            executor_kind: StageExecutorKindV1::DeterministicLocal,
            work_profile_id: "project_understanding".to_owned(),
            work_profile_version: "1.1.0".to_owned(),
            work_profile_definition_hash: Some(Sha256Hash::digest(b"profile-definition")),
            profile_catalog_fingerprint: Some(Sha256Hash::digest(b"profile-catalog")),
            profile_resolution_fingerprint: Some(Sha256Hash::digest(b"profile-resolution")),
            project_ids: vec![ProjectId::from_stable_bytes(goal_id.as_str().as_bytes())],
            included_work: vec![label.to_owned()],
            excluded_work: vec!["publish".to_owned()],
            expected_change_scope: Vec::new(),
            dependencies: Vec::new(),
            parallel_group: None,
            completion_criteria: vec![format!("{label}_complete")],
            failure_policy: StageFailurePolicyV1::Replan,
            route_decision_ref: None,
            permission_plan_ref: None,
            validation_plan_ref: None,
            impact_analysis_ref: None,
            change_plan_refs: Vec::new(),
            result_ref: None,
            state,
            stage_fingerprint: Sha256Hash::digest(b"unsealed"),
        }
    }

    fn input(goal_id: &GoalId, revision: u64) -> StageGraphPlanInput {
        let first = stage(goal_id, "understand", StageStateV1::Ready);
        let mut second = stage(goal_id, "plan", StageStateV1::Ready);
        second.dependencies = vec![first.stage_id.clone()];
        StageGraphPlanInput {
            goal_id: goal_id.clone(),
            plan_revision: revision,
            stages: vec![first.clone(), second.clone()],
            edges: vec![StageGraphEdgeV1 {
                from: first.stage_id,
                to: second.stage_id,
                relation: StageEdgeRelationV1::Requires,
            }],
            parallel_groups: Vec::new(),
            critical_path: Vec::new(),
            integration_stage_id: None,
        }
    }

    fn result_ref(label: &str) -> DocumentRef {
        DocumentRef {
            schema_id: star_contracts::stage::STAGE_RESULT_SCHEMA_ID.to_owned(),
            document_id: format!("srs_{label}"),
            revision: 1,
            sha256: Sha256Hash::digest(label.as_bytes()),
        }
    }

    fn stage_id(goal_id: &GoalId, label: &str) -> StageId {
        StageId::from_stable_bytes(format!("{}:{label}", goal_id.as_str()).as_bytes())
    }

    #[test]
    fn planning_positive_builds_stable_graph_identity() {
        let goal_id = GoalId::new();
        let first = build_stage_graph(input(&goal_id, 1)).unwrap();
        let second = build_stage_graph(input(&goal_id, 1)).unwrap();
        assert_eq!(first.stage_graph_id, second.stage_graph_id);
        assert_eq!(first.graph_fingerprint, second.graph_fingerprint);
    }

    #[test]
    fn planning_negative_rejects_stale_revision() {
        let goal_id = GoalId::new();
        let current = build_stage_graph(input(&goal_id, 1)).unwrap();
        assert_eq!(
            replan_stage_graph(&current, input(&goal_id, 1), "new impact"),
            Err(StagePlanningError::StaleRevision)
        );
    }

    #[test]
    fn planning_negative_rejects_initial_terminal_state_without_evidence() {
        let goal_id = GoalId::new();
        let mut candidate = input(&goal_id, 1);
        candidate.stages[0].state = StageStateV1::Completed;
        assert_eq!(
            build_stage_graph(candidate),
            Err(StagePlanningError::Invalid)
        );
    }

    #[test]
    fn planning_failure_rejects_completed_stage_rewrite() {
        let goal_id = GoalId::new();
        let mut current = build_stage_graph(input(&goal_id, 1)).unwrap();
        let completed_id = stage_id(&goal_id, "understand");
        let current_stage = current
            .stages
            .iter_mut()
            .find(|stage| stage.stage_id == completed_id)
            .unwrap();
        current_stage.state = StageStateV1::Completed;
        current_stage.result_ref = Some(result_ref("completed-rewrite"));
        *current_stage = current_stage.clone().seal().unwrap();
        current = current.seal().unwrap();
        let mut next = input(&goal_id, 2);
        let completed = current
            .stages
            .iter()
            .find(|stage| stage.stage_id == completed_id)
            .unwrap()
            .clone();
        let next_stage = next
            .stages
            .iter_mut()
            .find(|stage| stage.stage_id == completed_id)
            .unwrap();
        *next_stage = completed;
        next_stage.objective = "silently changed".to_owned();
        assert_eq!(
            replan_stage_graph(&current, next, "unexpected impact"),
            Err(StagePlanningError::CompletedStageChanged)
        );
    }

    #[test]
    fn planning_recovery_accepts_new_revision_with_completed_stage_preserved() {
        let goal_id = GoalId::new();
        let mut current = build_stage_graph(input(&goal_id, 1)).unwrap();
        let completed_id = stage_id(&goal_id, "understand");
        let current_stage = current
            .stages
            .iter_mut()
            .find(|stage| stage.stage_id == completed_id)
            .unwrap();
        current_stage.state = StageStateV1::Completed;
        current_stage.result_ref = Some(result_ref("completed-preserved"));
        *current_stage = current_stage.clone().seal().unwrap();
        current = current.seal().unwrap();
        let mut next = input(&goal_id, 2);
        let completed = current
            .stages
            .iter()
            .find(|stage| stage.stage_id == completed_id)
            .unwrap()
            .clone();
        *next
            .stages
            .iter_mut()
            .find(|stage| stage.stage_id == completed_id)
            .unwrap() = completed;
        let replanned_id = stage_id(&goal_id, "plan");
        let replanned = next
            .stages
            .iter_mut()
            .find(|stage| stage.stage_id == replanned_id)
            .unwrap();
        replanned.revision = 2;
        replanned.objective = "replanned bounded result".to_owned();
        let next = replan_stage_graph(&current, next, "new risk").unwrap();
        assert_eq!(next.plan_revision, 2);
        assert_eq!(
            next.previous_graph_fingerprint,
            Some(current.graph_fingerprint)
        );
        assert_eq!(next.replan_reason.as_deref(), Some("new risk"));
    }
}
