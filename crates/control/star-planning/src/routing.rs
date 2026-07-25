//! Deterministic Codex route selection from a current CapabilitySnapshot.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use star_contracts::{
    RouteDecisionId, RunId, Sha256Hash,
    evidence::DocumentRef,
    planning::ValidationRiskLevel,
    routing::{
        CAPABILITY_SNAPSHOT_SCHEMA_ID, CapabilitySnapshotV1, EstimatedUsageClassV1,
        ExecutionModeV1, ExecutionRealizationV1, ModelRoleV1, ROUTE_DECISION_SCHEMA_ID,
        ROUTING_CONTRACT_VERSION, ReasoningEffortV1, RouteAlternativeV1, RouteConfidenceV1,
        RouteDecisionKindV1, RouteDecisionV1, RouteFallbackV1, RoutingContractError,
    },
    stage::{StageExecutorKindV1, StageModeV1, StageSpecV1},
};
use thiserror::Error;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingPolicyV1 {
    pub role_models: BTreeMap<ModelRoleV1, Vec<String>>,
    pub max_parallel_codex: u32,
    pub allow_managed_ultra: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteRequestV1 {
    pub run_id: RunId,
    pub decision_kind: RouteDecisionKindV1,
    pub stage_graph_ref: DocumentRef,
    pub requested_model: Option<String>,
    pub requested_reasoning_effort: Option<ReasoningEffortV1>,
    pub requested_execution_mode: Option<ExecutionModeV1>,
    pub risk_level: ValidationRiskLevel,
    pub parallelizable: bool,
    pub estimated_usage_class: EstimatedUsageClassV1,
    pub permission_plan_ref: DocumentRef,
    pub budget_snapshot_ref: Option<DocumentRef>,
    pub config_fingerprint: Sha256Hash,
    pub decided_at: DateTime<Utc>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RoutePlanningError {
    #[error("route requires a Codex stage")]
    DeterministicStage,
    #[error("Codex capability snapshot is stale or lacks required lifecycle operations")]
    CapabilityUnavailable,
    #[error("no allowed model supports the required effort")]
    NoModel,
    #[error("requested execution mode cannot be realized safely")]
    ExecutionMode,
    #[error("route contract failed: {0}")]
    Contract(#[from] RoutingContractError),
}

pub fn route_codex_stage(
    stage: &StageSpecV1,
    snapshot: &CapabilitySnapshotV1,
    policy: &RoutingPolicyV1,
    request: RouteRequestV1,
) -> Result<RouteDecisionV1, RoutePlanningError> {
    if stage.executor_kind != StageExecutorKindV1::Codex {
        return Err(RoutePlanningError::DeterministicStage);
    }
    snapshot.verify()?;
    if !snapshot.is_current_at(request.decided_at)
        || !["thread_start", "turn_start"]
            .iter()
            .all(|operation| snapshot.operations.get(*operation) == Some(&true))
    {
        return Err(RoutePlanningError::CapabilityUnavailable);
    }
    let role = role_for(stage.stage_mode, request.risk_level);
    let desired_effort = request
        .requested_reasoning_effort
        .unwrap_or_else(|| effort_for(stage.stage_mode, request.risk_level));
    let candidates = if let Some(requested) = request.requested_model.as_ref() {
        vec![requested.clone()]
    } else {
        policy.role_models.get(&role).cloned().unwrap_or_default()
    };
    let (resolved_model, reasoning_effort, model_downgraded) = candidates
        .iter()
        .find_map(|model_id| {
            snapshot
                .models
                .iter()
                .find(|model| &model.model_id == model_id)
                .and_then(|model| {
                    supported_effort(desired_effort, &model.supported_reasoning_efforts)
                        .map(|effort| (model.model_id.clone(), effort, effort != desired_effort))
                })
        })
        .or_else(|| {
            snapshot.models.iter().find_map(|model| {
                supported_effort(desired_effort, &model.supported_reasoning_efforts)
                    .map(|effort| (model.model_id.clone(), effort, true))
            })
        })
        .ok_or(RoutePlanningError::NoModel)?;
    let requested_mode = request
        .requested_execution_mode
        .unwrap_or(ExecutionModeV1::Single);
    let (execution_mode, execution_realization, mode_changed) =
        realize_mode(requested_mode, request.parallelizable, snapshot, policy)?;
    let mut rationale = vec![format!("stage_mode:{:?}", stage.stage_mode).to_ascii_lowercase()];
    rationale.push(format!("risk:{:?}", request.risk_level).to_ascii_lowercase());
    if model_downgraded {
        rationale.push("requested_or_preferred_model_capability_fallback".to_owned());
    }
    if mode_changed {
        rationale.push("requested_execution_mode_safe_fallback".to_owned());
    }
    let alternatives = candidates
        .iter()
        .filter(|model| *model != &resolved_model)
        .map(|model| RouteAlternativeV1 {
            model_role: role,
            model_id: Some(model.clone()),
            reasoning_effort: desired_effort,
            execution_mode: requested_mode,
            reason_not_selected: "capability_or_policy_constraint".to_owned(),
        })
        .collect::<Vec<_>>();
    let fallback_chain = vec![RouteFallbackV1 {
        ordinal: 1,
        model_role: role,
        model_id: Some(resolved_model.clone()),
        reasoning_effort: snapshot
            .models
            .iter()
            .find(|model| model.model_id == resolved_model)
            .map(|model| model.default_reasoning_effort)
            .unwrap_or(reasoning_effort),
        execution_mode: ExecutionModeV1::Single,
        condition: "selected_route_rejected_before_effect".to_owned(),
    }];
    let capability_snapshot_ref = DocumentRef {
        schema_id: CAPABILITY_SNAPSHOT_SCHEMA_ID.to_owned(),
        document_id: snapshot.capability_snapshot_id.as_str().to_owned(),
        revision: 1,
        sha256: snapshot.snapshot_fingerprint.clone(),
    };
    RouteDecisionV1 {
        schema_id: ROUTE_DECISION_SCHEMA_ID.to_owned(),
        schema_version: ROUTING_CONTRACT_VERSION,
        route_decision_id: RouteDecisionId::new(),
        revision: 1,
        goal_id: stage.goal_id.clone(),
        run_id: request.run_id,
        stage_id: stage.stage_id.clone(),
        stage_revision: stage.revision,
        stage_fingerprint: stage.stage_fingerprint.clone(),
        stage_graph_ref: request.stage_graph_ref,
        decision_kind: request.decision_kind,
        model_role: role,
        requested_model: request.requested_model,
        resolved_model,
        requested_reasoning_effort: request.requested_reasoning_effort,
        reasoning_effort,
        plan_reasoning_effort: (stage.stage_mode == StageModeV1::Plan).then_some(reasoning_effort),
        stage_mode: stage.stage_mode,
        requested_execution_mode: request.requested_execution_mode,
        execution_mode,
        execution_realization,
        capability_snapshot_ref,
        config_fingerprint: request.config_fingerprint,
        risk_level: request.risk_level,
        parallelizable: request.parallelizable,
        estimated_usage_class: request.estimated_usage_class,
        confidence: if model_downgraded || mode_changed {
            RouteConfidenceV1::Medium
        } else {
            RouteConfidenceV1::High
        },
        rationale,
        alternatives,
        fallback_chain,
        permission_plan_ref: request.permission_plan_ref,
        budget_snapshot_ref: request.budget_snapshot_ref,
        decided_at: request.decided_at,
        decision_fingerprint: Sha256Hash::digest(b"unsealed-route"),
    }
    .seal_against(stage, snapshot)
    .map_err(RoutePlanningError::from)
}

fn role_for(mode: StageModeV1, risk: ValidationRiskLevel) -> ModelRoleV1 {
    match (mode, risk) {
        (StageModeV1::Review, _) | (_, ValidationRiskLevel::Critical) => ModelRoleV1::Sol,
        (StageModeV1::Plan, ValidationRiskLevel::High) => ModelRoleV1::Sol,
        (_, ValidationRiskLevel::Low) => ModelRoleV1::Luna,
        _ => ModelRoleV1::Terra,
    }
}

fn effort_for(mode: StageModeV1, risk: ValidationRiskLevel) -> ReasoningEffortV1 {
    match (mode, risk) {
        (StageModeV1::Review, ValidationRiskLevel::Critical)
        | (StageModeV1::Plan, ValidationRiskLevel::Critical) => ReasoningEffortV1::Xhigh,
        (StageModeV1::Review, _) | (_, ValidationRiskLevel::High) => ReasoningEffortV1::High,
        (_, ValidationRiskLevel::Low) => ReasoningEffortV1::Low,
        _ => ReasoningEffortV1::Medium,
    }
}

fn supported_effort(
    desired: ReasoningEffortV1,
    supported: &[ReasoningEffortV1],
) -> Option<ReasoningEffortV1> {
    supported
        .iter()
        .copied()
        .filter(|effort| *effort <= desired)
        .max()
        .or_else(|| supported.iter().copied().min())
}

fn realize_mode(
    requested: ExecutionModeV1,
    parallelizable: bool,
    snapshot: &CapabilitySnapshotV1,
    policy: &RoutingPolicyV1,
) -> Result<(ExecutionModeV1, ExecutionRealizationV1, bool), RoutePlanningError> {
    if snapshot.native_execution_modes.contains(&requested) {
        return Ok((requested, ExecutionRealizationV1::Native, false));
    }
    if requested == ExecutionModeV1::Ultra
        && parallelizable
        && policy.allow_managed_ultra
        && policy.max_parallel_codex >= 2
        && snapshot
            .managed_execution_modes
            .contains(&ExecutionModeV1::Ultra)
    {
        return Ok((
            ExecutionModeV1::Ultra,
            ExecutionRealizationV1::Managed,
            false,
        ));
    }
    if snapshot
        .native_execution_modes
        .contains(&ExecutionModeV1::Single)
    {
        return Ok((
            ExecutionModeV1::Single,
            ExecutionRealizationV1::Native,
            true,
        ));
    }
    if snapshot
        .managed_execution_modes
        .contains(&ExecutionModeV1::Single)
    {
        return Ok((
            ExecutionModeV1::Single,
            ExecutionRealizationV1::Managed,
            true,
        ));
    }
    Err(RoutePlanningError::ExecutionMode)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use chrono::Duration;
    use star_contracts::{
        ArtifactId, CapabilitySnapshotId, GoalId, ProjectId, StageId,
        evidence::{ArtifactKind, ArtifactRef, ProducerRef, RedactionStatus, RetentionClass},
        routing::{CapabilitySourceV1, CodexPermissionCapabilitiesV1, ModelCapabilityV1},
        stage::{STAGE_CONTRACT_VERSION, STAGE_SPEC_SCHEMA_ID, StageFailurePolicyV1, StageStateV1},
    };

    use super::*;

    fn document(schema: &str, id: &str) -> DocumentRef {
        DocumentRef {
            schema_id: schema.to_owned(),
            document_id: id.to_owned(),
            revision: 1,
            sha256: Sha256Hash::digest(id.as_bytes()),
        }
    }

    fn stage(goal_id: &GoalId, mode: StageModeV1) -> StageSpecV1 {
        StageSpecV1 {
            schema_id: STAGE_SPEC_SCHEMA_ID.to_owned(),
            schema_version: STAGE_CONTRACT_VERSION,
            stage_id: StageId::new(),
            revision: 1,
            goal_id: goal_id.clone(),
            task_spec_ref: None,
            scope_revision_ref: None,
            title: "implement".to_owned(),
            objective: "implement bounded source changes".to_owned(),
            stage_mode: mode,
            executor_kind: StageExecutorKindV1::Codex,
            work_profile_id: "ai_development_validation".to_owned(),
            work_profile_version: "1.1.0".to_owned(),
            work_profile_definition_hash: Some(Sha256Hash::digest(b"profile-definition")),
            profile_catalog_fingerprint: Some(Sha256Hash::digest(b"profile-catalog")),
            profile_resolution_fingerprint: Some(Sha256Hash::digest(b"profile-resolution")),
            project_ids: vec![ProjectId::new()],
            included_work: vec!["implementation".to_owned()],
            excluded_work: vec!["publish".to_owned()],
            expected_change_scope: vec!["src".to_owned()],
            dependencies: Vec::new(),
            parallel_group: None,
            completion_criteria: vec!["target_pass".to_owned()],
            failure_policy: StageFailurePolicyV1::Replan,
            route_decision_ref: None,
            permission_plan_ref: Some(document("star.permission-plan", "permission")),
            validation_plan_ref: None,
            impact_analysis_ref: None,
            change_plan_refs: Vec::new(),
            result_ref: None,
            state: StageStateV1::Draft,
            stage_fingerprint: Sha256Hash::digest(b"unsealed"),
        }
        .seal()
        .unwrap()
    }

    fn snapshot(now: DateTime<Utc>) -> CapabilitySnapshotV1 {
        CapabilitySnapshotV1 {
            schema_id: CAPABILITY_SNAPSHOT_SCHEMA_ID.to_owned(),
            schema_version: 1,
            capability_snapshot_id: CapabilitySnapshotId::new(),
            source: CapabilitySourceV1::CodexAppServer,
            captured_at: now,
            expires_at: now + Duration::minutes(15),
            codex_version: Some("26.7".to_owned()),
            protocol_version: "app-server-v1".to_owned(),
            protocol_schema_fingerprint: Sha256Hash::digest(b"app-server-schema"),
            models: vec![
                ModelCapabilityV1 {
                    catalog_id: "gpt-terra".to_owned(),
                    model_id: "gpt-terra".to_owned(),
                    display_name: "Terra".to_owned(),
                    hidden: false,
                    is_default: true,
                    supported_reasoning_efforts: vec![
                        ReasoningEffortV1::Medium,
                        ReasoningEffortV1::High,
                    ],
                    default_reasoning_effort: ReasoningEffortV1::Medium,
                },
                ModelCapabilityV1 {
                    catalog_id: "gpt-sol".to_owned(),
                    model_id: "gpt-sol".to_owned(),
                    display_name: "Sol".to_owned(),
                    hidden: false,
                    is_default: false,
                    supported_reasoning_efforts: vec![
                        ReasoningEffortV1::High,
                        ReasoningEffortV1::Xhigh,
                    ],
                    default_reasoning_effort: ReasoningEffortV1::High,
                },
            ],
            operations: BTreeMap::from([
                ("thread_start".to_owned(), true),
                ("turn_start".to_owned(), true),
                ("turn_interrupt".to_owned(), true),
            ]),
            native_execution_modes: vec![ExecutionModeV1::Single],
            managed_execution_modes: vec![ExecutionModeV1::Single, ExecutionModeV1::Ultra],
            permission_capabilities: CodexPermissionCapabilitiesV1 {
                approval_policy_configurable: true,
                sandbox_mode_configurable: true,
                network_policy_observable: true,
                paid_action_observable: false,
            },
            limits: BTreeMap::from([("max_parallel".to_owned(), 3)]),
            limitations: vec!["paid_cost_unavailable".to_owned()],
            raw_artifact_ref: ArtifactRef {
                artifact_id: ArtifactId::new(),
                kind: ArtifactKind::Log,
                project_id: None,
                relative_path: "codex/capability.json".to_owned(),
                media_type: "application/json".to_owned(),
                size_bytes: 1,
                sha256: Sha256Hash::digest(b"raw"),
                created_at: now,
                producer: ProducerRef {
                    component: "test".to_owned(),
                    product_version: "0.1.0".to_owned(),
                    build_id: "test".to_owned(),
                    platform: "windows-x64".to_owned(),
                },
                redaction_status: RedactionStatus::Redacted,
                retention_class: RetentionClass::Evidence,
                source_artifact_ref: None,
            },
            snapshot_fingerprint: Sha256Hash::digest(b"unsealed"),
        }
        .seal()
        .unwrap()
    }

    fn policy() -> RoutingPolicyV1 {
        RoutingPolicyV1 {
            role_models: BTreeMap::from([
                (ModelRoleV1::Luna, vec!["gpt-terra".to_owned()]),
                (ModelRoleV1::Terra, vec!["gpt-terra".to_owned()]),
                (ModelRoleV1::Sol, vec!["gpt-sol".to_owned()]),
            ]),
            max_parallel_codex: 3,
            allow_managed_ultra: true,
        }
    }

    fn request(now: DateTime<Utc>) -> RouteRequestV1 {
        RouteRequestV1 {
            run_id: RunId::new(),
            decision_kind: RouteDecisionKindV1::Initial,
            stage_graph_ref: document("star.stage-graph", "stage-graph"),
            requested_model: None,
            requested_reasoning_effort: None,
            requested_execution_mode: None,
            risk_level: ValidationRiskLevel::High,
            parallelizable: false,
            estimated_usage_class: EstimatedUsageClassV1::Standard,
            permission_plan_ref: document("star.permission-plan", "permission"),
            budget_snapshot_ref: None,
            config_fingerprint: Sha256Hash::digest(b"config"),
            decided_at: now,
        }
    }

    #[test]
    fn routing_positive_selects_observed_model_and_effort() {
        let now = Utc::now();
        let route = route_codex_stage(
            &stage(&GoalId::new(), StageModeV1::Execute),
            &snapshot(now),
            &policy(),
            request(now),
        )
        .unwrap();
        assert_eq!(route.resolved_model, "gpt-terra");
        assert_eq!(route.reasoning_effort, ReasoningEffortV1::High);
    }

    #[test]
    fn routing_negative_rejects_deterministic_local_stage() {
        let now = Utc::now();
        let mut local = stage(&GoalId::new(), StageModeV1::Plan);
        local.executor_kind = StageExecutorKindV1::DeterministicLocal;
        local = local.seal().unwrap();
        assert_eq!(
            route_codex_stage(&local, &snapshot(now), &policy(), request(now)),
            Err(RoutePlanningError::DeterministicStage)
        );
    }

    #[test]
    fn routing_failure_rejects_stale_capability() {
        let now = Utc::now();
        let stale = snapshot(now - Duration::hours(1));
        assert_eq!(
            route_codex_stage(
                &stage(&GoalId::new(), StageModeV1::Execute),
                &stale,
                &policy(),
                request(now),
            ),
            Err(RoutePlanningError::CapabilityUnavailable)
        );
    }

    #[test]
    fn routing_recovery_falls_back_from_unavailable_max_to_single() {
        let now = Utc::now();
        let mut requested = request(now);
        requested.requested_execution_mode = Some(ExecutionModeV1::Max);
        let route = route_codex_stage(
            &stage(&GoalId::new(), StageModeV1::Execute),
            &snapshot(now),
            &policy(),
            requested,
        )
        .unwrap();
        assert_eq!(route.execution_mode, ExecutionModeV1::Single);
        assert!(
            route
                .rationale
                .iter()
                .any(|reason| reason == "requested_execution_mode_safe_fallback")
        );
    }

    #[test]
    fn routing_negative_rejects_permission_plan_mismatch() {
        let now = Utc::now();
        let mut requested = request(now);
        requested.permission_plan_ref = document("star.permission-plan", "other-permission");
        assert_eq!(
            route_codex_stage(
                &stage(&GoalId::new(), StageModeV1::Execute),
                &snapshot(now),
                &policy(),
                requested,
            ),
            Err(RoutePlanningError::Contract(RoutingContractError::Stage))
        );
    }
}
