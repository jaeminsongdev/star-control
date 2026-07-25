//! Codex capability normalization and stage routing contracts.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    CapabilitySnapshotId, GoalId, RouteDecisionId, RunId, Sha256Hash, StageId, canonical_sha256,
    evidence::{ArtifactRef, DocumentRef},
    planning::ValidationRiskLevel,
    stage::{StageExecutorKindV1, StageModeV1, StageSpecV1},
};

pub const CAPABILITY_SNAPSHOT_SCHEMA_ID: &str = "star.capability-snapshot";
pub const ROUTE_DECISION_SCHEMA_ID: &str = "star.route-decision";
pub const ROUTING_CONTRACT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySourceV1 {
    CodexAppServer,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ModelRoleV1 {
    Luna,
    Terra,
    Sol,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffortV1 {
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
    Max,
    Ultra,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionModeV1 {
    Single,
    Max,
    Ultra,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionRealizationV1 {
    Native,
    Managed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RouteDecisionKindV1 {
    Initial,
    Retry,
    Escalation,
    UserOverride,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EstimatedUsageClassV1 {
    Small,
    Standard,
    Large,
    Unknown,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RouteConfidenceV1 {
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModelCapabilityV1 {
    pub catalog_id: String,
    pub model_id: String,
    pub display_name: String,
    pub hidden: bool,
    pub is_default: bool,
    pub supported_reasoning_efforts: Vec<ReasoningEffortV1>,
    pub default_reasoning_effort: ReasoningEffortV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CodexPermissionCapabilitiesV1 {
    pub approval_policy_configurable: bool,
    pub sandbox_mode_configurable: bool,
    pub network_policy_observable: bool,
    pub paid_action_observable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilitySnapshotV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub capability_snapshot_id: CapabilitySnapshotId,
    pub source: CapabilitySourceV1,
    pub captured_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex_version: Option<String>,
    pub protocol_version: String,
    pub protocol_schema_fingerprint: Sha256Hash,
    pub models: Vec<ModelCapabilityV1>,
    pub operations: BTreeMap<String, bool>,
    pub native_execution_modes: Vec<ExecutionModeV1>,
    pub managed_execution_modes: Vec<ExecutionModeV1>,
    pub permission_capabilities: CodexPermissionCapabilitiesV1,
    pub limits: BTreeMap<String, u32>,
    pub limitations: Vec<String>,
    pub raw_artifact_ref: ArtifactRef,
    pub snapshot_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteAlternativeV1 {
    pub model_role: ModelRoleV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    pub reasoning_effort: ReasoningEffortV1,
    pub execution_mode: ExecutionModeV1,
    pub reason_not_selected: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteFallbackV1 {
    pub ordinal: u32,
    pub model_role: ModelRoleV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    pub reasoning_effort: ReasoningEffortV1,
    pub execution_mode: ExecutionModeV1,
    pub condition: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteDecisionV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub route_decision_id: RouteDecisionId,
    pub revision: u64,
    pub goal_id: GoalId,
    pub run_id: RunId,
    pub stage_id: StageId,
    pub stage_revision: u64,
    pub stage_fingerprint: Sha256Hash,
    pub stage_graph_ref: DocumentRef,
    pub decision_kind: RouteDecisionKindV1,
    pub model_role: ModelRoleV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_model: Option<String>,
    pub resolved_model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_reasoning_effort: Option<ReasoningEffortV1>,
    pub reasoning_effort: ReasoningEffortV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_reasoning_effort: Option<ReasoningEffortV1>,
    pub stage_mode: StageModeV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_execution_mode: Option<ExecutionModeV1>,
    pub execution_mode: ExecutionModeV1,
    pub execution_realization: ExecutionRealizationV1,
    pub capability_snapshot_ref: DocumentRef,
    pub config_fingerprint: Sha256Hash,
    pub risk_level: ValidationRiskLevel,
    pub parallelizable: bool,
    pub estimated_usage_class: EstimatedUsageClassV1,
    pub confidence: RouteConfidenceV1,
    pub rationale: Vec<String>,
    pub alternatives: Vec<RouteAlternativeV1>,
    pub fallback_chain: Vec<RouteFallbackV1>,
    pub permission_plan_ref: DocumentRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_snapshot_ref: Option<DocumentRef>,
    pub decided_at: DateTime<Utc>,
    pub decision_fingerprint: Sha256Hash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RoutingContractError {
    #[error("routing schema or identity is invalid")]
    Identity,
    #[error("capability snapshot is invalid or expired")]
    Capability,
    #[error("route is incompatible with its stage")]
    Stage,
    #[error("requested route is unsupported by the capability snapshot")]
    Unsupported,
    #[error("route content is invalid")]
    Content,
    #[error("routing fingerprint could not be calculated")]
    Fingerprint,
}

impl CapabilitySnapshotV1 {
    pub fn seal(mut self) -> Result<Self, RoutingContractError> {
        // `model/list` order and each model's advertised effort order are product
        // data. Preserve both exactly; silently sorting them would make a stale or
        // incorrectly normalized capability response look current.
        self.native_execution_modes.sort();
        self.native_execution_modes.dedup();
        self.managed_execution_modes.sort();
        self.managed_execution_modes.dedup();
        self.limitations.sort();
        self.limitations.dedup();
        self.validate_shape()?;
        self.snapshot_fingerprint = canonical_sha256(&serde_json::json!({
            "domain":CAPABILITY_SNAPSHOT_SCHEMA_ID,
            "version":ROUTING_CONTRACT_VERSION,
            "value":{
                "capability_snapshot_id":self.capability_snapshot_id,
                "source":self.source,
                "captured_at":self.captured_at,
                "expires_at":self.expires_at,
                "codex_version":self.codex_version,
                "protocol_version":self.protocol_version,
                "protocol_schema_fingerprint":self.protocol_schema_fingerprint,
                "models":self.models,
                "operations":self.operations,
                "native_execution_modes":self.native_execution_modes,
                "managed_execution_modes":self.managed_execution_modes,
                "permission_capabilities":self.permission_capabilities,
                "limits":self.limits,
                "limitations":self.limitations,
                "raw_artifact_ref":self.raw_artifact_ref,
            }
        }))
        .map_err(|_| RoutingContractError::Fingerprint)?;
        Ok(self)
    }

    pub fn verify(&self) -> Result<(), RoutingContractError> {
        let expected = self.clone().seal()?;
        if expected != *self {
            return Err(RoutingContractError::Fingerprint);
        }
        Ok(())
    }

    pub fn is_current_at(&self, now: DateTime<Utc>) -> bool {
        self.captured_at <= now && now < self.expires_at
    }

    pub fn supports_model_effort(&self, model_id: &str, effort: ReasoningEffortV1) -> bool {
        self.models.iter().any(|model| {
            model.model_id == model_id && model.supported_reasoning_efforts.contains(&effort)
        })
    }

    pub fn supports_execution_mode(
        &self,
        mode: ExecutionModeV1,
        realization: ExecutionRealizationV1,
    ) -> bool {
        match realization {
            ExecutionRealizationV1::Native => self.native_execution_modes.contains(&mode),
            ExecutionRealizationV1::Managed => self.managed_execution_modes.contains(&mode),
        }
    }

    fn validate_shape(&self) -> Result<(), RoutingContractError> {
        let model_ids = self
            .models
            .iter()
            .map(|model| model.model_id.as_str())
            .collect::<BTreeSet<_>>();
        let catalog_ids = self
            .models
            .iter()
            .map(|model| model.catalog_id.as_str())
            .collect::<BTreeSet<_>>();
        if self.schema_id != CAPABILITY_SNAPSHOT_SCHEMA_ID
            || self.schema_version != ROUTING_CONTRACT_VERSION
            || self.expires_at <= self.captured_at
            || self
                .codex_version
                .as_deref()
                .is_none_or(|version| !bounded_text(version, 128))
            || !bounded_token(&self.protocol_version, 96)
            || self.protocol_schema_fingerprint == Sha256Hash::digest(b"")
            || self.models.is_empty()
            || model_ids.len() != self.models.len()
            || catalog_ids.len() != self.models.len()
            || self.models.iter().filter(|model| model.is_default).count() > 1
            || self.models.iter().any(|model| {
                !bounded_token(&model.model_id, 128)
                    || !bounded_token(&model.catalog_id, 128)
                    || !bounded_text(&model.display_name, 256)
                    || model.supported_reasoning_efforts.is_empty()
                    || model
                        .supported_reasoning_efforts
                        .iter()
                        .collect::<BTreeSet<_>>()
                        .len()
                        != model.supported_reasoning_efforts.len()
                    || !model
                        .supported_reasoning_efforts
                        .contains(&model.default_reasoning_effort)
            })
            || self
                .operations
                .keys()
                .any(|operation| !bounded_token(operation, 96))
            || self
                .limits
                .iter()
                .any(|(key, value)| !bounded_token(key, 96) || *value == 0)
            || self.raw_artifact_ref.validate().is_err()
            || self.limitations.len() > 256
            || self
                .limitations
                .iter()
                .any(|limitation| !bounded_text(limitation, 1_024))
            || (self
                .native_execution_modes
                .contains(&ExecutionModeV1::Single)
                && (self.operations.get("thread_start") != Some(&true)
                    || self.operations.get("turn_start") != Some(&true)
                    || !self.permission_capabilities.approval_policy_configurable
                    || !self.permission_capabilities.sandbox_mode_configurable))
        {
            return Err(RoutingContractError::Capability);
        }
        Ok(())
    }
}

impl RouteDecisionV1 {
    pub fn seal_against(
        mut self,
        stage: &StageSpecV1,
        snapshot: &CapabilitySnapshotV1,
    ) -> Result<Self, RoutingContractError> {
        stage.verify().map_err(|_| RoutingContractError::Stage)?;
        snapshot.verify()?;
        self.rationale.sort();
        self.rationale.dedup();
        self.alternatives.sort_by(|left, right| {
            (&left.model_role, &left.model_id, &left.reasoning_effort).cmp(&(
                &right.model_role,
                &right.model_id,
                &right.reasoning_effort,
            ))
        });
        self.fallback_chain.sort_by_key(|fallback| fallback.ordinal);
        self.validate_against(stage, snapshot)?;
        self.decision_fingerprint = canonical_sha256(&serde_json::json!({
            "domain":ROUTE_DECISION_SCHEMA_ID,
            "version":ROUTING_CONTRACT_VERSION,
            "value":{
                "route_decision_id":self.route_decision_id,
                "revision":self.revision,
                "goal_id":self.goal_id,
                "run_id":self.run_id,
                "stage_id":self.stage_id,
                "stage_revision":self.stage_revision,
                "stage_fingerprint":self.stage_fingerprint,
                "stage_graph_ref":self.stage_graph_ref,
                "decision_kind":self.decision_kind,
                "model_role":self.model_role,
                "requested_model":self.requested_model,
                "resolved_model":self.resolved_model,
                "requested_reasoning_effort":self.requested_reasoning_effort,
                "reasoning_effort":self.reasoning_effort,
                "plan_reasoning_effort":self.plan_reasoning_effort,
                "stage_mode":self.stage_mode,
                "requested_execution_mode":self.requested_execution_mode,
                "execution_mode":self.execution_mode,
                "execution_realization":self.execution_realization,
                "capability_snapshot_ref":self.capability_snapshot_ref,
                "config_fingerprint":self.config_fingerprint,
                "risk_level":self.risk_level,
                "parallelizable":self.parallelizable,
                "estimated_usage_class":self.estimated_usage_class,
                "confidence":self.confidence,
                "rationale":self.rationale,
                "alternatives":self.alternatives,
                "fallback_chain":self.fallback_chain,
                "permission_plan_ref":self.permission_plan_ref,
                "budget_snapshot_ref":self.budget_snapshot_ref,
                "decided_at":self.decided_at,
            }
        }))
        .map_err(|_| RoutingContractError::Fingerprint)?;
        Ok(self)
    }

    pub fn verify_against(
        &self,
        stage: &StageSpecV1,
        snapshot: &CapabilitySnapshotV1,
    ) -> Result<(), RoutingContractError> {
        let expected = self.clone().seal_against(stage, snapshot)?;
        if expected != *self {
            return Err(RoutingContractError::Fingerprint);
        }
        Ok(())
    }

    fn validate_against(
        &self,
        stage: &StageSpecV1,
        snapshot: &CapabilitySnapshotV1,
    ) -> Result<(), RoutingContractError> {
        if self.schema_id != ROUTE_DECISION_SCHEMA_ID
            || self.schema_version != ROUTING_CONTRACT_VERSION
            || self.revision == 0
            || stage.executor_kind != StageExecutorKindV1::Codex
            || self.goal_id != stage.goal_id
            || self.stage_id != stage.stage_id
            || self.stage_revision != stage.revision
            || self.stage_fingerprint != stage.stage_fingerprint
            || self.stage_mode != stage.stage_mode
            || self.capability_snapshot_ref.schema_id != CAPABILITY_SNAPSHOT_SCHEMA_ID
            || self.capability_snapshot_ref.document_id != snapshot.capability_snapshot_id.as_str()
            || self.capability_snapshot_ref.revision != 1
            || self.capability_snapshot_ref.sha256 != snapshot.snapshot_fingerprint
            || stage.permission_plan_ref.as_ref() != Some(&self.permission_plan_ref)
        {
            return Err(RoutingContractError::Stage);
        }
        if !snapshot.is_current_at(self.decided_at)
            || !snapshot.supports_model_effort(&self.resolved_model, self.reasoning_effort)
            || !snapshot.supports_execution_mode(self.execution_mode, self.execution_realization)
        {
            return Err(RoutingContractError::Unsupported);
        }
        let ordinals = self
            .fallback_chain
            .iter()
            .map(|fallback| fallback.ordinal)
            .collect::<BTreeSet<_>>();
        if !bounded_token(&self.resolved_model, 128)
            || self
                .requested_model
                .as_deref()
                .is_some_and(|model| !bounded_token(model, 128))
            || self.config_fingerprint == Sha256Hash::digest(b"")
            || self.rationale.is_empty()
            || self.rationale.len() > 128
            || self
                .rationale
                .iter()
                .any(|reason| !bounded_text(reason, 1_024))
            || self.fallback_chain.is_empty()
            || self.fallback_chain.len() > 64
            || self.fallback_chain.iter().any(|fallback| {
                fallback.ordinal == 0
                    || !bounded_text(&fallback.condition, 512)
                    || fallback.model_id.as_deref().is_none_or(|model| {
                        !bounded_token(model, 128)
                            || !snapshot.supports_model_effort(model, fallback.reasoning_effort)
                    })
                    || (!snapshot.supports_execution_mode(
                        fallback.execution_mode,
                        ExecutionRealizationV1::Native,
                    ) && !snapshot.supports_execution_mode(
                        fallback.execution_mode,
                        ExecutionRealizationV1::Managed,
                    ))
            })
            || ordinals.len() != self.fallback_chain.len()
            || self
                .fallback_chain
                .iter()
                .enumerate()
                .any(|(index, fallback)| fallback.ordinal != u32::try_from(index + 1).unwrap_or(0))
            || self.alternatives.len() > 128
            || self.alternatives.iter().any(|alternative| {
                !bounded_text(&alternative.reason_not_selected, 512)
                    || alternative
                        .model_id
                        .as_deref()
                        .is_some_and(|model| !bounded_token(model, 128))
            })
            || (self.stage_mode != StageModeV1::Plan && self.plan_reasoning_effort.is_some())
            || (self.stage_mode == StageModeV1::Plan
                && self.plan_reasoning_effort.is_some_and(|effort| {
                    !snapshot.supports_model_effort(&self.resolved_model, effort)
                }))
            || !document_ref_is_bounded(&self.permission_plan_ref, "star.permission-plan")
            || !document_ref_is_bounded(&self.stage_graph_ref, "star.stage-graph")
            || self.budget_snapshot_ref.as_ref().is_some_and(|reference| {
                !document_ref_is_bounded(reference, "star.budget-snapshot")
            })
        {
            return Err(RoutingContractError::Content);
        }
        Ok(())
    }
}

fn document_ref_is_bounded(reference: &DocumentRef, schema_id: &str) -> bool {
    reference.schema_id == schema_id
        && bounded_token(&reference.document_id, 192)
        && reference.revision > 0
        && reference.sha256 != Sha256Hash::digest(b"")
}

fn bounded_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.contains('\0')
}

fn bounded_token(value: &str, max: usize) -> bool {
    bounded_text(value, max)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/'))
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;
    use crate::{
        ArtifactId, ProjectId,
        evidence::{ArtifactKind, ProducerRef, RedactionStatus, RetentionClass},
        stage::{StageFailurePolicyV1, StageStateV1},
    };

    fn empty_hash() -> Sha256Hash {
        Sha256Hash::digest(b"unsealed")
    }

    fn document(schema_id: &str, document_id: &str, hash: Sha256Hash) -> DocumentRef {
        DocumentRef {
            schema_id: schema_id.to_owned(),
            document_id: document_id.to_owned(),
            revision: 1,
            sha256: hash,
        }
    }

    fn artifact() -> ArtifactRef {
        ArtifactRef {
            artifact_id: ArtifactId::new(),
            kind: ArtifactKind::Log,
            project_id: None,
            relative_path: "capabilities/redacted.json".to_owned(),
            media_type: "application/json".to_owned(),
            size_bytes: 16,
            sha256: Sha256Hash::digest(b"raw"),
            created_at: Utc::now(),
            producer: ProducerRef {
                component: "star-adapter-codex".to_owned(),
                product_version: "0.1.0".to_owned(),
                build_id: "test".to_owned(),
                platform: "windows-x64".to_owned(),
            },
            redaction_status: RedactionStatus::Redacted,
            retention_class: RetentionClass::Evidence,
            source_artifact_ref: None,
        }
    }

    fn snapshot(now: DateTime<Utc>) -> CapabilitySnapshotV1 {
        CapabilitySnapshotV1 {
            schema_id: CAPABILITY_SNAPSHOT_SCHEMA_ID.to_owned(),
            schema_version: ROUTING_CONTRACT_VERSION,
            capability_snapshot_id: CapabilitySnapshotId::new(),
            source: CapabilitySourceV1::CodexAppServer,
            captured_at: now,
            expires_at: now + Duration::minutes(15),
            codex_version: Some("26.7.0".to_owned()),
            protocol_version: "app-server-v1".to_owned(),
            protocol_schema_fingerprint: Sha256Hash::digest(b"app-server-schema"),
            models: vec![ModelCapabilityV1 {
                catalog_id: "gpt-5.6-terra".to_owned(),
                model_id: "gpt-5.6-terra".to_owned(),
                display_name: "Terra".to_owned(),
                hidden: false,
                is_default: true,
                supported_reasoning_efforts: vec![
                    ReasoningEffortV1::Medium,
                    ReasoningEffortV1::High,
                ],
                default_reasoning_effort: ReasoningEffortV1::Medium,
            }],
            operations: BTreeMap::from([
                ("thread_start".to_owned(), true),
                ("turn_start".to_owned(), true),
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
            raw_artifact_ref: artifact(),
            snapshot_fingerprint: empty_hash(),
        }
        .seal()
        .unwrap()
    }

    fn codex_stage(goal_id: &GoalId) -> StageSpecV1 {
        StageSpecV1 {
            schema_id: crate::stage::STAGE_SPEC_SCHEMA_ID.to_owned(),
            schema_version: 1,
            stage_id: StageId::new(),
            revision: 1,
            goal_id: goal_id.clone(),
            task_spec_ref: None,
            scope_revision_ref: None,
            title: "implement".to_owned(),
            objective: "implement the bounded change".to_owned(),
            stage_mode: StageModeV1::Execute,
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
            completion_criteria: vec!["tests_pass".to_owned()],
            failure_policy: StageFailurePolicyV1::Replan,
            route_decision_ref: None,
            permission_plan_ref: Some(document("star.permission-plan", "permission", empty_hash())),
            validation_plan_ref: Some(document("star.validation-plan", "validation", empty_hash())),
            impact_analysis_ref: None,
            change_plan_refs: Vec::new(),
            result_ref: None,
            state: StageStateV1::Ready,
            stage_fingerprint: empty_hash(),
        }
        .seal()
        .unwrap()
    }

    fn route(
        stage: &StageSpecV1,
        snapshot: &CapabilitySnapshotV1,
        now: DateTime<Utc>,
    ) -> RouteDecisionV1 {
        RouteDecisionV1 {
            schema_id: ROUTE_DECISION_SCHEMA_ID.to_owned(),
            schema_version: ROUTING_CONTRACT_VERSION,
            route_decision_id: RouteDecisionId::new(),
            revision: 1,
            goal_id: stage.goal_id.clone(),
            run_id: RunId::new(),
            stage_id: stage.stage_id.clone(),
            stage_revision: stage.revision,
            stage_fingerprint: stage.stage_fingerprint.clone(),
            stage_graph_ref: document(
                "star.stage-graph",
                "stage-graph",
                Sha256Hash::digest(b"stage-graph"),
            ),
            decision_kind: RouteDecisionKindV1::Initial,
            model_role: ModelRoleV1::Terra,
            requested_model: None,
            resolved_model: "gpt-5.6-terra".to_owned(),
            requested_reasoning_effort: None,
            reasoning_effort: ReasoningEffortV1::High,
            plan_reasoning_effort: None,
            stage_mode: stage.stage_mode,
            requested_execution_mode: None,
            execution_mode: ExecutionModeV1::Single,
            execution_realization: ExecutionRealizationV1::Native,
            capability_snapshot_ref: document(
                CAPABILITY_SNAPSHOT_SCHEMA_ID,
                snapshot.capability_snapshot_id.as_str(),
                snapshot.snapshot_fingerprint.clone(),
            ),
            config_fingerprint: Sha256Hash::digest(b"config"),
            risk_level: ValidationRiskLevel::High,
            parallelizable: false,
            estimated_usage_class: EstimatedUsageClassV1::Standard,
            confidence: RouteConfidenceV1::High,
            rationale: vec!["multi_file_implementation".to_owned()],
            alternatives: vec![RouteAlternativeV1 {
                model_role: ModelRoleV1::Luna,
                model_id: None,
                reasoning_effort: ReasoningEffortV1::Low,
                execution_mode: ExecutionModeV1::Single,
                reason_not_selected: "insufficient_for_risk".to_owned(),
            }],
            fallback_chain: vec![RouteFallbackV1 {
                ordinal: 1,
                model_role: ModelRoleV1::Terra,
                model_id: Some("gpt-5.6-terra".to_owned()),
                reasoning_effort: ReasoningEffortV1::Medium,
                execution_mode: ExecutionModeV1::Single,
                condition: "high_effort_unavailable".to_owned(),
            }],
            permission_plan_ref: document("star.permission-plan", "permission", empty_hash()),
            budget_snapshot_ref: None,
            decided_at: now,
            decision_fingerprint: empty_hash(),
        }
    }

    #[test]
    fn route_positive_is_bound_to_current_capability_and_stage() {
        let now = Utc::now();
        let snapshot = snapshot(now);
        let stage = codex_stage(&GoalId::new());
        let route = route(&stage, &snapshot, now)
            .seal_against(&stage, &snapshot)
            .unwrap();
        route.verify_against(&stage, &snapshot).unwrap();
    }

    #[test]
    fn route_negative_unsupported_model_is_rejected() {
        let now = Utc::now();
        let snapshot = snapshot(now);
        let stage = codex_stage(&GoalId::new());
        let mut route = route(&stage, &snapshot, now);
        route.resolved_model = "unobserved-model".to_owned();
        assert_eq!(
            route.seal_against(&stage, &snapshot),
            Err(RoutingContractError::Unsupported)
        );
    }

    #[test]
    fn capability_failure_native_single_requires_permission_overrides() {
        let mut capability = snapshot(Utc::now());
        capability.permission_capabilities.sandbox_mode_configurable = false;
        assert_eq!(capability.seal(), Err(RoutingContractError::Capability));
    }

    #[test]
    fn route_negative_permission_plan_must_match_the_stage() {
        let now = Utc::now();
        let snapshot = snapshot(now);
        let stage = codex_stage(&GoalId::new());
        let mut route = route(&stage, &snapshot, now);
        route.permission_plan_ref = document(
            "star.permission-plan",
            "different-permission",
            Sha256Hash::digest(b"different-permission"),
        );
        assert_eq!(
            route.seal_against(&stage, &snapshot),
            Err(RoutingContractError::Stage)
        );
    }

    #[test]
    fn route_negative_fallback_must_be_executable_by_the_snapshot() {
        let now = Utc::now();
        let snapshot = snapshot(now);
        let stage = codex_stage(&GoalId::new());
        let mut decision = route(&stage, &snapshot, now);
        decision.fallback_chain[0].model_id = Some("unobserved-model".to_owned());
        assert_eq!(
            decision.seal_against(&stage, &snapshot),
            Err(RoutingContractError::Content)
        );

        let mut decision = route(&stage, &snapshot, now);
        decision.fallback_chain[0].execution_mode = ExecutionModeV1::Max;
        assert_eq!(
            decision.seal_against(&stage, &snapshot),
            Err(RoutingContractError::Content)
        );

        let mut decision = route(&stage, &snapshot, now);
        decision.fallback_chain[0].model_id = None;
        assert_eq!(
            decision.seal_against(&stage, &snapshot),
            Err(RoutingContractError::Content)
        );
    }

    #[test]
    fn route_failure_expired_snapshot_is_rejected() {
        let now = Utc::now();
        let snapshot = snapshot(now - Duration::hours(1));
        let stage = codex_stage(&GoalId::new());
        assert_eq!(
            route(&stage, &snapshot, now).seal_against(&stage, &snapshot),
            Err(RoutingContractError::Unsupported)
        );
    }

    #[test]
    fn route_recovery_reseals_after_new_capability_snapshot() {
        let now = Utc::now();
        let old = snapshot(now - Duration::hours(1));
        let current = snapshot(now);
        let stage = codex_stage(&GoalId::new());
        let mut retried = route(&stage, &old, now);
        assert!(retried.clone().seal_against(&stage, &old).is_err());
        retried.decision_kind = RouteDecisionKindV1::Retry;
        retried.capability_snapshot_ref = document(
            CAPABILITY_SNAPSHOT_SCHEMA_ID,
            current.capability_snapshot_id.as_str(),
            current.snapshot_fingerprint.clone(),
        );
        assert!(retried.seal_against(&stage, &current).is_ok());
    }
}
