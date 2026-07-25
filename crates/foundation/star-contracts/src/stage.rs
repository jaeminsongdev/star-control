//! Stage graph contracts for durable deterministic and Codex execution plans.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    GoalId, ProjectId, Sha256Hash, StageGraphId, StageId, StageResultId, canonical_sha256,
    evidence::DocumentRef, profile::DevelopmentProfileResolutionV1,
};

pub const STAGE_SPEC_SCHEMA_ID: &str = "star.stage-spec";
pub const STAGE_GRAPH_SCHEMA_ID: &str = "star.stage-graph";
pub const STAGE_RESULT_SCHEMA_ID: &str = "star.stage-result";
pub const STAGE_CONTRACT_VERSION: u32 = 1;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum StageModeV1 {
    Plan,
    Execute,
    Review,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StageExecutorKindV1 {
    DeterministicLocal,
    Codex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StageFailurePolicyV1 {
    Retry,
    Replan,
    Block,
    Rollback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StageStateV1 {
    Draft,
    Ready,
    Running,
    WaitingApproval,
    Paused,
    Blocked,
    Failed,
    Cancelled,
    Completed,
    Superseded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StageResultOutcomeV1 {
    Completed,
    Failed,
    Blocked,
    Cancelled,
    OutcomeUnknown,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum StageEdgeRelationV1 {
    Requires,
    ProvidesContract,
    Validates,
    Merges,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StageSpecV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub stage_id: StageId,
    pub revision: u64,
    pub goal_id: GoalId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_spec_ref: Option<DocumentRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_revision_ref: Option<DocumentRef>,
    pub title: String,
    pub objective: String,
    pub stage_mode: StageModeV1,
    pub executor_kind: StageExecutorKindV1,
    pub work_profile_id: String,
    #[serde(default)]
    pub work_profile_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_profile_definition_hash: Option<Sha256Hash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_catalog_fingerprint: Option<Sha256Hash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_resolution_fingerprint: Option<Sha256Hash>,
    pub project_ids: Vec<ProjectId>,
    pub included_work: Vec<String>,
    pub excluded_work: Vec<String>,
    pub expected_change_scope: Vec<String>,
    pub dependencies: Vec<StageId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel_group: Option<String>,
    pub completion_criteria: Vec<String>,
    pub failure_policy: StageFailurePolicyV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_decision_ref: Option<DocumentRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_plan_ref: Option<DocumentRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_plan_ref: Option<DocumentRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impact_analysis_ref: Option<DocumentRef>,
    #[serde(default)]
    pub change_plan_refs: Vec<DocumentRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_ref: Option<DocumentRef>,
    pub state: StageStateV1,
    pub stage_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StageProjectEvidenceV1 {
    pub project_id: ProjectId,
    pub evidence_bundle_ref: DocumentRef,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StageResultV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub stage_result_id: StageResultId,
    pub revision: u64,
    pub goal_id: GoalId,
    pub stage_graph_ref: DocumentRef,
    pub stage_ref: DocumentRef,
    pub outcome: StageResultOutcomeV1,
    pub completed_criteria: Vec<String>,
    pub project_evidence: Vec<StageProjectEvidenceV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_record_ref: Option<DocumentRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_action: Option<String>,
    pub source_effect_may_have_started: bool,
    pub recorded_at: DateTime<Utc>,
    pub result_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StageGraphEdgeV1 {
    pub from: StageId,
    pub to: StageId,
    pub relation: StageEdgeRelationV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StageParallelGroupV1 {
    pub group_id: String,
    pub stage_ids: Vec<StageId>,
    pub max_parallel: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StageGraphV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub stage_graph_id: StageGraphId,
    pub goal_id: GoalId,
    pub plan_revision: u64,
    pub stages: Vec<StageSpecV1>,
    pub edges: Vec<StageGraphEdgeV1>,
    pub parallel_groups: Vec<StageParallelGroupV1>,
    pub critical_path: Vec<StageId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integration_stage_id: Option<StageId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_graph_fingerprint: Option<Sha256Hash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replan_reason: Option<String>,
    pub graph_fingerprint: Sha256Hash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StageContractError {
    #[error("stage schema or identity is invalid")]
    Identity,
    #[error("stage content or lifecycle is invalid")]
    Content,
    #[error("stage graph contains a missing or duplicate stage")]
    GraphMember,
    #[error("stage graph contains a dependency cycle")]
    Cycle,
    #[error("parallel stages have overlapping write scope")]
    ParallelOverlap,
    #[error("stage development Profile binding is missing or stale")]
    ProfileBinding,
    #[error("stage fingerprint could not be calculated")]
    Fingerprint,
}

impl StageSpecV1 {
    pub fn permission_scope_hash(&self) -> Result<Sha256Hash, StageContractError> {
        canonical_sha256(&serde_json::json!({
            "domain":"star.stage-permission-scope",
            "version":STAGE_CONTRACT_VERSION,
            "value":{
                "stage_id":self.stage_id,
                "revision":self.revision,
                "goal_id":self.goal_id,
                "task_spec_ref":self.task_spec_ref,
                "scope_revision_ref":self.scope_revision_ref,
                "stage_mode":self.stage_mode,
                "executor_kind":self.executor_kind,
                "work_profile_id":self.work_profile_id,
                "work_profile_version":self.work_profile_version,
                "work_profile_definition_hash":self.work_profile_definition_hash,
                "profile_catalog_fingerprint":self.profile_catalog_fingerprint,
                "profile_resolution_fingerprint":self.profile_resolution_fingerprint,
                "project_ids":self.project_ids,
                "included_work":self.included_work,
                "excluded_work":self.excluded_work,
                "expected_change_scope":self.expected_change_scope,
                "completion_criteria":self.completion_criteria,
                "failure_policy":self.failure_policy,
            }
        }))
        .map_err(|_| StageContractError::Fingerprint)
    }

    pub fn bind_profile_resolution(
        mut self,
        resolution: &DevelopmentProfileResolutionV1,
    ) -> Result<Self, StageContractError> {
        resolution
            .validate()
            .map_err(|_| StageContractError::ProfileBinding)?;
        if resolution.selected_profiles.len() != 1 {
            return Err(StageContractError::ProfileBinding);
        }
        let profile_ref = &resolution.selected_profiles[0];
        let definition_refs = resolution
            .definition_refs
            .iter()
            .filter(|reference| reference.profile_ref == *profile_ref)
            .collect::<Vec<_>>();
        if definition_refs.len() != 1 {
            return Err(StageContractError::ProfileBinding);
        }
        self.work_profile_id = profile_ref.profile_id.clone();
        self.work_profile_version = profile_ref.profile_version.clone();
        self.work_profile_definition_hash = Some(definition_refs[0].definition_hash.clone());
        self.profile_catalog_fingerprint = Some(resolution.catalog_fingerprint.clone());
        self.profile_resolution_fingerprint =
            Some(resolution.profile_resolution_fingerprint.clone());
        Ok(self)
    }

    pub fn verify_profile_binding(
        &self,
        resolution: &DevelopmentProfileResolutionV1,
    ) -> Result<(), StageContractError> {
        resolution
            .validate()
            .map_err(|_| StageContractError::ProfileBinding)?;
        if resolution.selected_profiles.len() != 1
            || resolution.selected_profiles[0].profile_id != self.work_profile_id
            || resolution.selected_profiles[0].profile_version != self.work_profile_version
            || self.profile_catalog_fingerprint.as_ref() != Some(&resolution.catalog_fingerprint)
            || self.profile_resolution_fingerprint.as_ref()
                != Some(&resolution.profile_resolution_fingerprint)
        {
            return Err(StageContractError::ProfileBinding);
        }
        let matching_definition_refs = resolution
            .definition_refs
            .iter()
            .filter(|reference| reference.profile_ref == resolution.selected_profiles[0])
            .collect::<Vec<_>>();
        if matching_definition_refs.len() != 1
            || self.work_profile_definition_hash.as_ref()
                != Some(&matching_definition_refs[0].definition_hash)
        {
            return Err(StageContractError::ProfileBinding);
        }
        Ok(())
    }

    pub fn seal(mut self) -> Result<Self, StageContractError> {
        sort_dedup_strings(&mut self.included_work)?;
        sort_dedup_strings(&mut self.excluded_work)?;
        sort_dedup_strings(&mut self.expected_change_scope)?;
        self.project_ids.sort();
        self.project_ids.dedup();
        self.dependencies.sort();
        self.dependencies.dedup();
        self.completion_criteria.sort();
        self.completion_criteria.dedup();
        self.change_plan_refs.sort();
        self.change_plan_refs.dedup();
        self.validate_shape()?;
        self.stage_fingerprint = canonical_sha256(&serde_json::json!({
            "domain": STAGE_SPEC_SCHEMA_ID,
            "version": STAGE_CONTRACT_VERSION,
            "value": {
                "stage_id":self.stage_id,
                "revision":self.revision,
                "goal_id":self.goal_id,
                "task_spec_ref":self.task_spec_ref,
                "scope_revision_ref":self.scope_revision_ref,
                "title":self.title,
                "objective":self.objective,
                "stage_mode":self.stage_mode,
                "executor_kind":self.executor_kind,
                "work_profile_id":self.work_profile_id,
                "work_profile_version":self.work_profile_version,
                "work_profile_definition_hash":self.work_profile_definition_hash,
                "profile_catalog_fingerprint":self.profile_catalog_fingerprint,
                "profile_resolution_fingerprint":self.profile_resolution_fingerprint,
                "project_ids":self.project_ids,
                "included_work":self.included_work,
                "excluded_work":self.excluded_work,
                "expected_change_scope":self.expected_change_scope,
                "dependencies":self.dependencies,
                "parallel_group":self.parallel_group,
                "completion_criteria":self.completion_criteria,
                "failure_policy":self.failure_policy,
                "route_decision_ref":self.route_decision_ref,
                "permission_plan_ref":self.permission_plan_ref,
                "validation_plan_ref":self.validation_plan_ref,
                "impact_analysis_ref":self.impact_analysis_ref,
                "change_plan_refs":self.change_plan_refs,
                "result_ref":self.result_ref,
                "state":self.state,
            }
        }))
        .map_err(|_| StageContractError::Fingerprint)?;
        Ok(self)
    }

    pub fn verify(&self) -> Result<(), StageContractError> {
        let expected = self.clone().seal()?;
        if expected != *self {
            return Err(StageContractError::Fingerprint);
        }
        Ok(())
    }

    fn validate_shape(&self) -> Result<(), StageContractError> {
        if self.schema_id != STAGE_SPEC_SCHEMA_ID
            || self.schema_version != STAGE_CONTRACT_VERSION
            || self.revision == 0
            || !bounded_text(&self.title, 256)
            || !bounded_text(&self.objective, 4_096)
            || !bounded_token(&self.work_profile_id, 96)
            || semver::Version::parse(&self.work_profile_version).is_err()
            || self.work_profile_definition_hash.is_none()
            || self.profile_catalog_fingerprint.is_none()
            || self.profile_resolution_fingerprint.is_none()
            || self.project_ids.is_empty()
            || self.project_ids.len() > 64
            || self.included_work.is_empty()
            || self.excluded_work.is_empty()
            || (self.stage_mode == StageModeV1::Execute && self.expected_change_scope.is_empty())
            || self.completion_criteria.is_empty()
            || self.dependencies.len() > 256
            || self.dependencies.contains(&self.stage_id)
            || self
                .task_spec_ref
                .as_ref()
                .is_some_and(|reference| !valid_document_ref(reference, "star.task-spec"))
            || self
                .scope_revision_ref
                .as_ref()
                .is_some_and(|reference| !valid_document_ref(reference, "star.scope-revision"))
            || self
                .route_decision_ref
                .as_ref()
                .is_some_and(|reference| !valid_document_ref(reference, "star.route-decision"))
            || self
                .permission_plan_ref
                .as_ref()
                .is_some_and(|reference| !valid_document_ref(reference, "star.permission-plan"))
            || self
                .validation_plan_ref
                .as_ref()
                .is_some_and(|reference| !valid_document_ref(reference, "star.validation-plan"))
            || self
                .impact_analysis_ref
                .as_ref()
                .is_some_and(|reference| !valid_document_ref(reference, "star.impact-analysis"))
            || self.change_plan_refs.len() > 256
            || self
                .change_plan_refs
                .iter()
                .any(|reference| !valid_document_ref(reference, "star.change-plan"))
            || self
                .parallel_group
                .as_deref()
                .is_some_and(|value| !bounded_token(value, 96))
            || self
                .result_ref
                .as_ref()
                .is_some_and(|reference| !valid_document_ref(reference, STAGE_RESULT_SCHEMA_ID))
        {
            return Err(StageContractError::Content);
        }
        if self.executor_kind == StageExecutorKindV1::DeterministicLocal
            && self.route_decision_ref.is_some()
        {
            return Err(StageContractError::Content);
        }
        if self.executor_kind == StageExecutorKindV1::Codex
            && self.state != StageStateV1::Draft
            && (self.permission_plan_ref.is_none() || self.validation_plan_ref.is_none())
        {
            return Err(StageContractError::Content);
        }
        if self.work_profile_id == "change_planning"
            && (self.executor_kind != StageExecutorKindV1::DeterministicLocal
                || self.task_spec_ref.is_none()
                || self.scope_revision_ref.is_none())
        {
            return Err(StageContractError::Content);
        }
        let terminal_result_required = matches!(
            self.state,
            StageStateV1::Blocked
                | StageStateV1::Failed
                | StageStateV1::Cancelled
                | StageStateV1::Completed
        );
        if terminal_result_required != self.result_ref.is_some() {
            return Err(StageContractError::Content);
        }
        Ok(())
    }
}

impl StageResultV1 {
    pub fn seal(mut self) -> Result<Self, StageContractError> {
        self.completed_criteria.sort();
        self.completed_criteria.dedup();
        self.project_evidence.sort();
        self.project_evidence.dedup();
        self.validate_shape()?;
        self.result_fingerprint = canonical_sha256(&serde_json::json!({
            "domain":STAGE_RESULT_SCHEMA_ID,
            "version":STAGE_CONTRACT_VERSION,
            "value":{
                "stage_result_id":self.stage_result_id,
                "revision":self.revision,
                "goal_id":self.goal_id,
                "stage_graph_ref":self.stage_graph_ref,
                "stage_ref":self.stage_ref,
                "outcome":self.outcome,
                "completed_criteria":self.completed_criteria,
                "project_evidence":self.project_evidence,
                "execution_record_ref":self.execution_record_ref,
                "failure_code":self.failure_code,
                "recovery_action":self.recovery_action,
                "source_effect_may_have_started":self.source_effect_may_have_started,
                "recorded_at":self.recorded_at,
            }
        }))
        .map_err(|_| StageContractError::Fingerprint)?;
        Ok(self)
    }

    pub fn verify(&self) -> Result<(), StageContractError> {
        let expected = self.clone().seal()?;
        if expected != *self {
            return Err(StageContractError::Fingerprint);
        }
        Ok(())
    }

    pub fn reference(&self) -> DocumentRef {
        DocumentRef {
            schema_id: STAGE_RESULT_SCHEMA_ID.to_owned(),
            document_id: self.stage_result_id.to_string(),
            revision: self.revision,
            sha256: self.result_fingerprint.clone(),
        }
    }

    pub fn terminal_state(&self) -> StageStateV1 {
        match self.outcome {
            StageResultOutcomeV1::Completed => StageStateV1::Completed,
            StageResultOutcomeV1::Failed => StageStateV1::Failed,
            StageResultOutcomeV1::Blocked | StageResultOutcomeV1::OutcomeUnknown => {
                StageStateV1::Blocked
            }
            StageResultOutcomeV1::Cancelled => StageStateV1::Cancelled,
        }
    }

    pub fn verify_against(
        &self,
        graph: &StageGraphV1,
        stage: &StageSpecV1,
    ) -> Result<(), StageContractError> {
        self.verify()?;
        graph.verify()?;
        stage.verify()?;
        let expected_graph_ref = DocumentRef {
            schema_id: STAGE_GRAPH_SCHEMA_ID.to_owned(),
            document_id: graph.stage_graph_id.to_string(),
            revision: graph.plan_revision,
            sha256: graph.graph_fingerprint.clone(),
        };
        let expected_stage_ref = DocumentRef {
            schema_id: STAGE_SPEC_SCHEMA_ID.to_owned(),
            document_id: stage.stage_id.to_string(),
            revision: stage.revision,
            sha256: stage.stage_fingerprint.clone(),
        };
        if self.goal_id != graph.goal_id
            || stage.goal_id != graph.goal_id
            || self.stage_graph_ref != expected_graph_ref
            || self.stage_ref != expected_stage_ref
            || !graph.stages.iter().any(|candidate| candidate == stage)
            || matches!(
                stage.state,
                StageStateV1::Blocked
                    | StageStateV1::Failed
                    | StageStateV1::Cancelled
                    | StageStateV1::Completed
                    | StageStateV1::Superseded
            )
        {
            return Err(StageContractError::Identity);
        }
        let project_ids = self
            .project_evidence
            .iter()
            .map(|evidence| evidence.project_id.clone())
            .collect::<Vec<_>>();
        if self.outcome == StageResultOutcomeV1::Completed {
            if self.completed_criteria != stage.completion_criteria
                || project_ids != stage.project_ids
                || !matches!(stage.state, StageStateV1::Ready | StageStateV1::Running)
            {
                return Err(StageContractError::Identity);
            }
        } else if self
            .completed_criteria
            .iter()
            .any(|criterion| !stage.completion_criteria.contains(criterion))
            || project_ids
                .iter()
                .any(|project_id| !stage.project_ids.contains(project_id))
        {
            return Err(StageContractError::Identity);
        }
        if stage.executor_kind == StageExecutorKindV1::Codex && self.execution_record_ref.is_none()
        {
            return Err(StageContractError::Content);
        }
        Ok(())
    }

    fn validate_shape(&self) -> Result<(), StageContractError> {
        if self.schema_id != STAGE_RESULT_SCHEMA_ID
            || self.schema_version != STAGE_CONTRACT_VERSION
            || self.revision == 0
            || !valid_document_ref(&self.stage_graph_ref, STAGE_GRAPH_SCHEMA_ID)
            || !valid_document_ref(&self.stage_ref, STAGE_SPEC_SCHEMA_ID)
            || self.completed_criteria.len() > 256
            || self
                .completed_criteria
                .iter()
                .any(|criterion| !bounded_text(criterion, 4_096))
            || self.project_evidence.len() > 64
            || self.project_evidence.iter().any(|evidence| {
                !bounded_token(evidence.project_id.as_str(), 192)
                    || !valid_document_ref(&evidence.evidence_bundle_ref, "star.evidence-bundle")
            })
            || self
                .project_evidence
                .windows(2)
                .any(|pair| pair[0].project_id >= pair[1].project_id)
            || self.execution_record_ref.as_ref().is_some_and(|reference| {
                !valid_document_ref(reference, "star.codex-execution-record")
            })
            || self
                .failure_code
                .as_deref()
                .is_some_and(|code| !bounded_token(code, 128))
            || self
                .recovery_action
                .as_deref()
                .is_some_and(|action| !bounded_text(action, 4_096))
        {
            return Err(StageContractError::Content);
        }
        if self.outcome == StageResultOutcomeV1::Completed {
            if self.completed_criteria.is_empty()
                || self.project_evidence.is_empty()
                || self.failure_code.is_some()
                || self.recovery_action.is_some()
            {
                return Err(StageContractError::Content);
            }
        } else if self.failure_code.is_none() {
            return Err(StageContractError::Content);
        }
        if self.outcome == StageResultOutcomeV1::OutcomeUnknown
            && (!self.source_effect_may_have_started || self.recovery_action.is_none())
        {
            return Err(StageContractError::Content);
        }
        Ok(())
    }
}

impl StageGraphV1 {
    pub fn seal(mut self) -> Result<Self, StageContractError> {
        self.stages = self
            .stages
            .into_iter()
            .map(StageSpecV1::seal)
            .collect::<Result<Vec<_>, _>>()?;
        self.stages
            .sort_by(|left, right| left.stage_id.cmp(&right.stage_id));
        self.edges.sort();
        self.edges.dedup();
        for group in &mut self.parallel_groups {
            group.stage_ids.sort();
            group.stage_ids.dedup();
        }
        self.parallel_groups
            .sort_by(|left, right| left.group_id.cmp(&right.group_id));
        if self.critical_path.iter().collect::<BTreeSet<_>>().len() != self.critical_path.len() {
            return Err(StageContractError::GraphMember);
        }
        self.validate_graph()?;
        self.graph_fingerprint = canonical_sha256(&serde_json::json!({
            "domain":STAGE_GRAPH_SCHEMA_ID,
            "version":STAGE_CONTRACT_VERSION,
            "value":{
                "stage_graph_id":self.stage_graph_id,
                "goal_id":self.goal_id,
                "plan_revision":self.plan_revision,
                "stages":self.stages,
                "edges":self.edges,
                "parallel_groups":self.parallel_groups,
                "critical_path":self.critical_path,
                "integration_stage_id":self.integration_stage_id,
                "previous_graph_fingerprint":self.previous_graph_fingerprint,
                "replan_reason":self.replan_reason,
            }
        }))
        .map_err(|_| StageContractError::Fingerprint)?;
        Ok(self)
    }

    pub fn verify(&self) -> Result<(), StageContractError> {
        let expected = self.clone().seal()?;
        if expected != *self {
            return Err(StageContractError::Fingerprint);
        }
        Ok(())
    }

    fn validate_graph(&self) -> Result<(), StageContractError> {
        if self.schema_id != STAGE_GRAPH_SCHEMA_ID
            || self.schema_version != STAGE_CONTRACT_VERSION
            || self.plan_revision == 0
            || self.stages.is_empty()
            || (self.plan_revision == 1
                && (self.previous_graph_fingerprint.is_some() || self.replan_reason.is_some()))
            || (self.plan_revision > 1
                && (self.previous_graph_fingerprint.is_none()
                    || self
                        .replan_reason
                        .as_deref()
                        .is_none_or(|reason| !bounded_text(reason, 2_048))))
        {
            return Err(StageContractError::Identity);
        }
        let by_id = self
            .stages
            .iter()
            .map(|stage| (stage.stage_id.clone(), stage))
            .collect::<BTreeMap<_, _>>();
        if by_id.len() != self.stages.len()
            || self
                .stages
                .iter()
                .any(|stage| stage.goal_id != self.goal_id)
            || self
                .integration_stage_id
                .as_ref()
                .is_some_and(|stage_id| !by_id.contains_key(stage_id))
            || self
                .critical_path
                .iter()
                .any(|stage_id| !by_id.contains_key(stage_id))
        {
            return Err(StageContractError::GraphMember);
        }
        let requires = self
            .edges
            .iter()
            .filter(|edge| edge.relation == StageEdgeRelationV1::Requires)
            .map(|edge| (edge.from.clone(), edge.to.clone()))
            .collect::<BTreeSet<_>>();
        let declared_dependencies = self
            .stages
            .iter()
            .flat_map(|stage| {
                stage
                    .dependencies
                    .iter()
                    .map(|dependency| (dependency.clone(), stage.stage_id.clone()))
            })
            .collect::<BTreeSet<_>>();
        if self.edges.iter().any(|edge| {
            edge.from == edge.to || !by_id.contains_key(&edge.from) || !by_id.contains_key(&edge.to)
        }) || requires != declared_dependencies
        {
            return Err(StageContractError::GraphMember);
        }
        let mut incoming = by_id
            .keys()
            .map(|stage_id| (stage_id.clone(), 0_usize))
            .collect::<BTreeMap<_, _>>();
        let mut outgoing = BTreeMap::<StageId, Vec<StageId>>::new();
        for (from, to) in &requires {
            *incoming
                .get_mut(to)
                .ok_or(StageContractError::GraphMember)? += 1;
            outgoing.entry(from.clone()).or_default().push(to.clone());
        }
        let mut ready = incoming
            .iter()
            .filter_map(|(stage_id, count)| (*count == 0).then_some(stage_id.clone()))
            .collect::<Vec<_>>();
        let mut visited = 0_usize;
        while let Some(stage_id) = ready.pop() {
            visited += 1;
            for successor in outgoing.get(&stage_id).into_iter().flatten() {
                let count = incoming
                    .get_mut(successor)
                    .ok_or(StageContractError::GraphMember)?;
                *count -= 1;
                if *count == 0 {
                    ready.push(successor.clone());
                }
            }
        }
        if visited != by_id.len() {
            return Err(StageContractError::Cycle);
        }
        if self
            .critical_path
            .windows(2)
            .any(|pair| !requires.contains(&(pair[0].clone(), pair[1].clone())))
        {
            return Err(StageContractError::GraphMember);
        }
        let mut group_ids = BTreeSet::new();
        let mut grouped_stage_ids = BTreeSet::new();
        for group in &self.parallel_groups {
            if !bounded_token(&group.group_id, 96)
                || group.max_parallel == 0
                || group.stage_ids.len() < 2
                || usize::try_from(group.max_parallel)
                    .ok()
                    .is_none_or(|limit| limit > group.stage_ids.len())
                || !group_ids.insert(group.group_id.as_str())
                || group
                    .stage_ids
                    .iter()
                    .any(|stage_id| !grouped_stage_ids.insert(stage_id.clone()))
            {
                return Err(StageContractError::Content);
            }
            let members = group
                .stage_ids
                .iter()
                .map(|stage_id| {
                    by_id
                        .get(stage_id)
                        .copied()
                        .ok_or(StageContractError::GraphMember)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if members
                .iter()
                .any(|stage| stage.parallel_group.as_deref() != Some(&group.group_id))
            {
                return Err(StageContractError::GraphMember);
            }
            for (index, left) in members.iter().enumerate() {
                for right in members.iter().skip(index + 1) {
                    if requires.contains(&(left.stage_id.clone(), right.stage_id.clone()))
                        || requires.contains(&(right.stage_id.clone(), left.stage_id.clone()))
                    {
                        return Err(StageContractError::GraphMember);
                    }
                    let overlap = left.expected_change_scope.iter().any(|left_scope| {
                        right
                            .expected_change_scope
                            .iter()
                            .any(|right_scope| scopes_overlap(left_scope, right_scope))
                    });
                    if overlap
                        && (left.stage_mode == StageModeV1::Execute
                            || right.stage_mode == StageModeV1::Execute)
                    {
                        return Err(StageContractError::ParallelOverlap);
                    }
                }
            }
        }
        if self.stages.iter().any(|stage| {
            stage.parallel_group.as_ref().is_some_and(|group_id| {
                !self.parallel_groups.iter().any(|group| {
                    &group.group_id == group_id && group.stage_ids.contains(&stage.stage_id)
                })
            })
        }) {
            return Err(StageContractError::GraphMember);
        }
        Ok(())
    }
}

fn scopes_overlap(left: &str, right: &str) -> bool {
    fn normalized(value: &str) -> &str {
        value.trim().trim_end_matches(['/', '\\'])
    }
    let left = normalized(left);
    let right = normalized(right);
    left == right
        || right
            .strip_prefix(left)
            .is_some_and(|suffix| suffix.starts_with(['/', '\\']))
        || left
            .strip_prefix(right)
            .is_some_and(|suffix| suffix.starts_with(['/', '\\']))
}

fn sort_dedup_strings(values: &mut Vec<String>) -> Result<(), StageContractError> {
    values.sort();
    values.dedup();
    if values.len() > 256 || values.iter().any(|value| !bounded_text(value, 4_096)) {
        return Err(StageContractError::Content);
    }
    Ok(())
}

fn bounded_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.contains('\0')
}

fn bounded_token(value: &str, max: usize) -> bool {
    bounded_text(value, max)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn valid_document_ref(reference: &DocumentRef, schema_id: &str) -> bool {
    reference.schema_id == schema_id
        && bounded_token(&reference.document_id, 192)
        && reference.revision > 0
        && reference.sha256 != Sha256Hash::digest(b"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_hash() -> Sha256Hash {
        Sha256Hash::digest(b"unsealed")
    }

    fn stage(goal_id: &GoalId, title: &str, mode: StageModeV1) -> StageSpecV1 {
        StageSpecV1 {
            schema_id: STAGE_SPEC_SCHEMA_ID.to_owned(),
            schema_version: STAGE_CONTRACT_VERSION,
            stage_id: StageId::new(),
            revision: 1,
            goal_id: goal_id.clone(),
            task_spec_ref: None,
            scope_revision_ref: None,
            title: title.to_owned(),
            objective: format!("complete {title}"),
            stage_mode: mode,
            executor_kind: StageExecutorKindV1::DeterministicLocal,
            work_profile_id: "project_understanding".to_owned(),
            work_profile_version: "1.1.0".to_owned(),
            work_profile_definition_hash: Some(Sha256Hash::digest(b"profile-definition")),
            profile_catalog_fingerprint: Some(Sha256Hash::digest(b"profile-catalog")),
            profile_resolution_fingerprint: Some(Sha256Hash::digest(b"profile-resolution")),
            project_ids: vec![ProjectId::new()],
            included_work: vec![title.to_owned()],
            excluded_work: vec!["external_publish".to_owned()],
            expected_change_scope: vec![format!("src/{title}.rs")],
            dependencies: Vec::new(),
            parallel_group: None,
            completion_criteria: vec![format!("{title}_complete")],
            failure_policy: StageFailurePolicyV1::Block,
            route_decision_ref: None,
            permission_plan_ref: None,
            validation_plan_ref: None,
            impact_analysis_ref: None,
            change_plan_refs: Vec::new(),
            result_ref: None,
            state: StageStateV1::Draft,
            stage_fingerprint: empty_hash(),
        }
    }

    #[test]
    fn stage_graph_positive_path_is_canonical_and_acyclic() {
        let goal_id = GoalId::new();
        let first = stage(&goal_id, "discover", StageModeV1::Plan)
            .seal()
            .unwrap();
        let mut second = stage(&goal_id, "validate", StageModeV1::Review);
        second.dependencies = vec![first.stage_id.clone()];
        let second = second.seal().unwrap();
        let graph = StageGraphV1 {
            schema_id: STAGE_GRAPH_SCHEMA_ID.to_owned(),
            schema_version: STAGE_CONTRACT_VERSION,
            stage_graph_id: StageGraphId::new(),
            goal_id,
            plan_revision: 1,
            stages: vec![second.clone(), first.clone()],
            edges: vec![StageGraphEdgeV1 {
                from: first.stage_id,
                to: second.stage_id,
                relation: StageEdgeRelationV1::Requires,
            }],
            parallel_groups: Vec::new(),
            critical_path: vec![],
            integration_stage_id: None,
            previous_graph_fingerprint: None,
            replan_reason: None,
            graph_fingerprint: empty_hash(),
        }
        .seal()
        .unwrap();
        graph.verify().unwrap();
    }

    #[test]
    fn stage_result_positive_binds_exact_graph_stage_projects_and_criteria() {
        let goal_id = GoalId::new();
        let mut stage = stage(&goal_id, "execute", StageModeV1::Execute);
        stage.state = StageStateV1::Ready;
        let stage = stage.seal().unwrap();
        let graph = StageGraphV1 {
            schema_id: STAGE_GRAPH_SCHEMA_ID.to_owned(),
            schema_version: STAGE_CONTRACT_VERSION,
            stage_graph_id: StageGraphId::new(),
            goal_id: goal_id.clone(),
            plan_revision: 1,
            stages: vec![stage.clone()],
            edges: vec![],
            parallel_groups: vec![],
            critical_path: vec![stage.stage_id.clone()],
            integration_stage_id: Some(stage.stage_id.clone()),
            previous_graph_fingerprint: None,
            replan_reason: None,
            graph_fingerprint: empty_hash(),
        }
        .seal()
        .unwrap();
        let result = StageResultV1 {
            schema_id: STAGE_RESULT_SCHEMA_ID.to_owned(),
            schema_version: STAGE_CONTRACT_VERSION,
            stage_result_id: StageResultId::new(),
            revision: 1,
            goal_id,
            stage_graph_ref: DocumentRef {
                schema_id: STAGE_GRAPH_SCHEMA_ID.to_owned(),
                document_id: graph.stage_graph_id.to_string(),
                revision: graph.plan_revision,
                sha256: graph.graph_fingerprint.clone(),
            },
            stage_ref: DocumentRef {
                schema_id: STAGE_SPEC_SCHEMA_ID.to_owned(),
                document_id: stage.stage_id.to_string(),
                revision: stage.revision,
                sha256: stage.stage_fingerprint.clone(),
            },
            outcome: StageResultOutcomeV1::Completed,
            completed_criteria: stage.completion_criteria.clone(),
            project_evidence: vec![StageProjectEvidenceV1 {
                project_id: stage.project_ids[0].clone(),
                evidence_bundle_ref: DocumentRef {
                    schema_id: "star.evidence-bundle".to_owned(),
                    document_id: "evb_stage_current".to_owned(),
                    revision: 1,
                    sha256: Sha256Hash::digest(b"stage-evidence"),
                },
            }],
            execution_record_ref: None,
            failure_code: None,
            recovery_action: None,
            source_effect_may_have_started: false,
            recorded_at: Utc::now(),
            result_fingerprint: empty_hash(),
        }
        .seal()
        .unwrap();
        result.verify_against(&graph, &stage).unwrap();

        let mut failed = result.clone();
        failed.stage_result_id = StageResultId::new();
        failed.outcome = StageResultOutcomeV1::Failed;
        failed.completed_criteria.clear();
        failed.project_evidence.clear();
        failed.failure_code = Some("STAGE_EXECUTION_FAILED".to_owned());
        failed.result_fingerprint = empty_hash();
        let failed = failed.seal().unwrap();
        failed.verify_against(&graph, &stage).unwrap();

        let mut false_complete = result.clone();
        false_complete.completed_criteria.clear();
        false_complete.result_fingerprint = empty_hash();
        assert_eq!(false_complete.seal(), Err(StageContractError::Content));

        let mut stale = result;
        stale.stage_ref.sha256 = Sha256Hash::digest(b"stale-stage");
        stale = stale.seal().unwrap();
        assert_eq!(
            stale.verify_against(&graph, &stage),
            Err(StageContractError::Identity)
        );
    }

    #[test]
    fn stage_result_recovery_requires_effect_boundary_and_action() {
        let result = StageResultV1 {
            schema_id: STAGE_RESULT_SCHEMA_ID.to_owned(),
            schema_version: STAGE_CONTRACT_VERSION,
            stage_result_id: StageResultId::new(),
            revision: 1,
            goal_id: GoalId::new(),
            stage_graph_ref: DocumentRef {
                schema_id: STAGE_GRAPH_SCHEMA_ID.to_owned(),
                document_id: StageGraphId::new().to_string(),
                revision: 1,
                sha256: Sha256Hash::digest(b"graph"),
            },
            stage_ref: DocumentRef {
                schema_id: STAGE_SPEC_SCHEMA_ID.to_owned(),
                document_id: StageId::new().to_string(),
                revision: 1,
                sha256: Sha256Hash::digest(b"stage"),
            },
            outcome: StageResultOutcomeV1::OutcomeUnknown,
            completed_criteria: vec!["reconcile before retry".to_owned()],
            project_evidence: vec![],
            execution_record_ref: None,
            failure_code: Some("CODEX_OPERATION_LOST".to_owned()),
            recovery_action: None,
            source_effect_may_have_started: false,
            recorded_at: Utc::now(),
            result_fingerprint: empty_hash(),
        };
        assert_eq!(result.seal(), Err(StageContractError::Content));
    }

    #[test]
    fn stage_graph_negative_cycle_is_rejected() {
        let goal_id = GoalId::new();
        let mut first = stage(&goal_id, "one", StageModeV1::Plan);
        let mut second = stage(&goal_id, "two", StageModeV1::Plan);
        first.dependencies = vec![second.stage_id.clone()];
        second.dependencies = vec![first.stage_id.clone()];
        let result = StageGraphV1 {
            schema_id: STAGE_GRAPH_SCHEMA_ID.to_owned(),
            schema_version: STAGE_CONTRACT_VERSION,
            stage_graph_id: StageGraphId::new(),
            goal_id,
            plan_revision: 1,
            stages: vec![first.clone(), second.clone()],
            edges: vec![
                StageGraphEdgeV1 {
                    from: first.stage_id.clone(),
                    to: second.stage_id.clone(),
                    relation: StageEdgeRelationV1::Requires,
                },
                StageGraphEdgeV1 {
                    from: second.stage_id,
                    to: first.stage_id,
                    relation: StageEdgeRelationV1::Requires,
                },
            ],
            parallel_groups: Vec::new(),
            critical_path: vec![],
            integration_stage_id: None,
            previous_graph_fingerprint: None,
            replan_reason: None,
            graph_fingerprint: empty_hash(),
        }
        .seal();
        assert_eq!(result, Err(StageContractError::Cycle));
    }

    #[test]
    fn stage_graph_failure_parallel_write_overlap_is_rejected() {
        let goal_id = GoalId::new();
        let mut first = stage(&goal_id, "one", StageModeV1::Execute);
        let mut second = stage(&goal_id, "two", StageModeV1::Execute);
        first.parallel_group = Some("writers".to_owned());
        second.parallel_group = Some("writers".to_owned());
        second.expected_change_scope = first.expected_change_scope.clone();
        let stage_ids = vec![first.stage_id.clone(), second.stage_id.clone()];
        let result = StageGraphV1 {
            schema_id: STAGE_GRAPH_SCHEMA_ID.to_owned(),
            schema_version: STAGE_CONTRACT_VERSION,
            stage_graph_id: StageGraphId::new(),
            goal_id,
            plan_revision: 1,
            stages: vec![first, second],
            edges: Vec::new(),
            parallel_groups: vec![StageParallelGroupV1 {
                group_id: "writers".to_owned(),
                stage_ids,
                max_parallel: 2,
            }],
            critical_path: vec![],
            integration_stage_id: None,
            previous_graph_fingerprint: None,
            replan_reason: None,
            graph_fingerprint: empty_hash(),
        }
        .seal();
        assert_eq!(result, Err(StageContractError::ParallelOverlap));
    }

    #[test]
    fn stage_graph_negative_requires_edges_must_equal_stage_dependencies() {
        let goal_id = GoalId::new();
        let first = stage(&goal_id, "one", StageModeV1::Plan);
        let second = stage(&goal_id, "two", StageModeV1::Plan);
        let graph = StageGraphV1 {
            schema_id: STAGE_GRAPH_SCHEMA_ID.to_owned(),
            schema_version: STAGE_CONTRACT_VERSION,
            stage_graph_id: StageGraphId::new(),
            goal_id,
            plan_revision: 1,
            stages: vec![first.clone(), second.clone()],
            edges: vec![StageGraphEdgeV1 {
                from: first.stage_id,
                to: second.stage_id,
                relation: StageEdgeRelationV1::Requires,
            }],
            parallel_groups: Vec::new(),
            critical_path: Vec::new(),
            integration_stage_id: None,
            previous_graph_fingerprint: None,
            replan_reason: None,
            graph_fingerprint: empty_hash(),
        };
        assert_eq!(graph.seal(), Err(StageContractError::GraphMember));
    }

    #[test]
    fn stage_negative_rejects_unsealed_typed_document_reference() {
        let goal_id = GoalId::new();
        let mut candidate = stage(&goal_id, "typed-ref", StageModeV1::Execute);
        candidate.task_spec_ref = Some(DocumentRef {
            schema_id: "star.scope-revision".to_owned(),
            document_id: "wrong-schema".to_owned(),
            revision: 1,
            sha256: Sha256Hash::digest(b"wrong-schema"),
        });
        assert_eq!(candidate.seal(), Err(StageContractError::Content));
    }

    #[test]
    fn stage_graph_negative_dependent_stages_cannot_be_parallel_members() {
        let goal_id = GoalId::new();
        let mut first = stage(&goal_id, "one", StageModeV1::Plan);
        let mut second = stage(&goal_id, "two", StageModeV1::Plan);
        first.parallel_group = Some("readers".to_owned());
        second.parallel_group = Some("readers".to_owned());
        second.dependencies = vec![first.stage_id.clone()];
        let stage_ids = vec![first.stage_id.clone(), second.stage_id.clone()];
        let graph = StageGraphV1 {
            schema_id: STAGE_GRAPH_SCHEMA_ID.to_owned(),
            schema_version: STAGE_CONTRACT_VERSION,
            stage_graph_id: StageGraphId::new(),
            goal_id,
            plan_revision: 1,
            stages: vec![first.clone(), second.clone()],
            edges: vec![StageGraphEdgeV1 {
                from: first.stage_id,
                to: second.stage_id,
                relation: StageEdgeRelationV1::Requires,
            }],
            parallel_groups: vec![StageParallelGroupV1 {
                group_id: "readers".to_owned(),
                stage_ids,
                max_parallel: 2,
            }],
            critical_path: Vec::new(),
            integration_stage_id: None,
            previous_graph_fingerprint: None,
            replan_reason: None,
            graph_fingerprint: empty_hash(),
        };
        assert_eq!(graph.seal(), Err(StageContractError::GraphMember));
    }

    #[test]
    fn stage_graph_failure_parallel_parent_child_scope_overlap_is_rejected() {
        let goal_id = GoalId::new();
        let mut first = stage(&goal_id, "one", StageModeV1::Execute);
        let mut second = stage(&goal_id, "two", StageModeV1::Execute);
        first.parallel_group = Some("writers".to_owned());
        first.expected_change_scope = vec!["src".to_owned()];
        second.parallel_group = Some("writers".to_owned());
        second.expected_change_scope = vec!["src/lib.rs".to_owned()];
        let stage_ids = vec![first.stage_id.clone(), second.stage_id.clone()];
        let graph = StageGraphV1 {
            schema_id: STAGE_GRAPH_SCHEMA_ID.to_owned(),
            schema_version: STAGE_CONTRACT_VERSION,
            stage_graph_id: StageGraphId::new(),
            goal_id,
            plan_revision: 1,
            stages: vec![first, second],
            edges: Vec::new(),
            parallel_groups: vec![StageParallelGroupV1 {
                group_id: "writers".to_owned(),
                stage_ids,
                max_parallel: 2,
            }],
            critical_path: Vec::new(),
            integration_stage_id: None,
            previous_graph_fingerprint: None,
            replan_reason: None,
            graph_fingerprint: empty_hash(),
        };
        assert_eq!(graph.seal(), Err(StageContractError::ParallelOverlap));
    }

    #[test]
    fn stage_graph_recovery_requires_reseal_after_revision_change() {
        let goal_id = GoalId::new();
        let stage = stage(&goal_id, "recover", StageModeV1::Review)
            .seal()
            .unwrap();
        let mut stale = stage.clone();
        stale.revision = 2;
        assert_eq!(stale.verify(), Err(StageContractError::Fingerprint));
        assert!(stale.seal().is_ok());
    }
}
