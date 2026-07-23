//! M4 safe recipe, preview, PatchSet, application, and recovery contracts.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    Sha256Hash, canonical_sha256,
    evidence::{ActorRef, ArtifactRef, Completeness, DocumentRef, GateDecisionRef},
    ids::{
        ChangePlanId, CheckoutId, PatchApplicationId, PatchSetId, ProjectId, RecipeExecutionId,
        WorkspaceSnapshotId, WorktreeDecisionId,
    },
    management::ProjectPathRef,
};

pub const CHANGE_RECIPE_V2_SCHEMA_ID: &str = "star.change-recipe";
pub const PATCH_SET_V2_SCHEMA_ID: &str = "star.patch-set";
pub const RECIPE_EXECUTION_SCHEMA_ID: &str = "star.recipe-execution";
pub const PATCH_APPLICATION_SCHEMA_ID: &str = "star.patch-application";
pub const WORKTREE_DECISION_SCHEMA_ID: &str = "star.worktree-decision";
pub const PATCH_V1_TO_V2_MIGRATION_PLAN_SCHEMA_ID: &str = "star.patch-v1-to-v2-migration-plan";
pub const PATCH_V1_TO_V2_MIGRATION_RESULT_SCHEMA_ID: &str = "star.patch-v1-to-v2-migration-result";

pub const PATCH_V2_SCHEMA_VERSION: u32 = 2;
pub const PATCH_LIFECYCLE_SCHEMA_VERSION: u32 = 1;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RewriteAssuranceV2 {
    TextExact,
    SyntaxAware,
    SymbolAware,
    CodegenAssured,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TargetSelectorKindV2 {
    Path,
    Symbol,
    Finding,
    ManagedDeclaration,
    Contract,
    GeneratorInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TargetSelector {
    Path {
        project_id: ProjectId,
        paths: Vec<ProjectPathRef>,
        expected_content_fingerprints: BTreeMap<ProjectPathRef, Sha256Hash>,
    },
    Symbol {
        project_id: ProjectId,
        symbol_ids: Vec<String>,
        expected_definition_fingerprints: BTreeMap<String, Sha256Hash>,
    },
    Finding {
        project_id: ProjectId,
        finding_ids: Vec<String>,
        expected_finding_fingerprints: BTreeMap<String, Sha256Hash>,
    },
    ManagedDeclaration {
        project_id: ProjectId,
        declaration_ids: Vec<String>,
        expected_declaration_fingerprints: BTreeMap<String, Sha256Hash>,
    },
    Contract {
        project_id: ProjectId,
        contract_ids: Vec<String>,
        expected_surface_fingerprints: BTreeMap<String, Sha256Hash>,
    },
    GeneratorInput {
        project_id: ProjectId,
        input_paths: Vec<ProjectPathRef>,
        expected_input_fingerprints: BTreeMap<ProjectPathRef, Sha256Hash>,
    },
}

impl TargetSelector {
    pub fn kind(&self) -> TargetSelectorKindV2 {
        match self {
            Self::Path { .. } => TargetSelectorKindV2::Path,
            Self::Symbol { .. } => TargetSelectorKindV2::Symbol,
            Self::Finding { .. } => TargetSelectorKindV2::Finding,
            Self::ManagedDeclaration { .. } => TargetSelectorKindV2::ManagedDeclaration,
            Self::Contract { .. } => TargetSelectorKindV2::Contract,
            Self::GeneratorInput { .. } => TargetSelectorKindV2::GeneratorInput,
        }
    }

    pub fn project_id(&self) -> &ProjectId {
        match self {
            Self::Path { project_id, .. }
            | Self::Symbol { project_id, .. }
            | Self::Finding { project_id, .. }
            | Self::ManagedDeclaration { project_id, .. }
            | Self::Contract { project_id, .. }
            | Self::GeneratorInput { project_id, .. } => project_id,
        }
    }

    pub fn validate(&self) -> Result<(), PatchV2Error> {
        let valid_string_set = |values: &[String]| {
            !values.is_empty()
                && values.iter().all(|value| {
                    !value.trim().is_empty() && value.len() <= 512 && !value.contains('\0')
                })
                && values.windows(2).all(|pair| pair[0] < pair[1])
        };
        let valid_path_set = |values: &[ProjectPathRef]| {
            !values.is_empty() && values.windows(2).all(|pair| pair[0] < pair[1])
        };
        let valid = match self {
            Self::Path {
                paths,
                expected_content_fingerprints,
                ..
            } => {
                valid_path_set(paths)
                    && paths
                        .iter()
                        .all(|path| expected_content_fingerprints.contains_key(path))
                    && paths.len() == expected_content_fingerprints.len()
            }
            Self::Symbol {
                symbol_ids,
                expected_definition_fingerprints,
                ..
            } => {
                valid_string_set(symbol_ids)
                    && symbol_ids
                        .iter()
                        .all(|id| expected_definition_fingerprints.contains_key(id))
                    && symbol_ids.len() == expected_definition_fingerprints.len()
            }
            Self::Finding {
                finding_ids,
                expected_finding_fingerprints,
                ..
            } => {
                valid_string_set(finding_ids)
                    && finding_ids
                        .iter()
                        .all(|id| expected_finding_fingerprints.contains_key(id))
                    && finding_ids.len() == expected_finding_fingerprints.len()
            }
            Self::ManagedDeclaration {
                declaration_ids,
                expected_declaration_fingerprints,
                ..
            } => {
                valid_string_set(declaration_ids)
                    && declaration_ids
                        .iter()
                        .all(|id| expected_declaration_fingerprints.contains_key(id))
                    && declaration_ids.len() == expected_declaration_fingerprints.len()
            }
            Self::Contract {
                contract_ids,
                expected_surface_fingerprints,
                ..
            } => {
                valid_string_set(contract_ids)
                    && contract_ids
                        .iter()
                        .all(|id| expected_surface_fingerprints.contains_key(id))
                    && contract_ids.len() == expected_surface_fingerprints.len()
            }
            Self::GeneratorInput {
                input_paths,
                expected_input_fingerprints,
                ..
            } => {
                valid_path_set(input_paths)
                    && input_paths
                        .iter()
                        .all(|path| expected_input_fingerprints.contains_key(path))
                    && input_paths.len() == expected_input_fingerprints.len()
            }
        };
        valid.then_some(()).ok_or(PatchV2Error::Selector)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeRecipeV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub recipe_id: String,
    pub recipe_version: String,
    pub display_name: String,
    pub language: Option<String>,
    pub selector_kinds: Vec<TargetSelectorKindV2>,
    pub rewrite_assurance: RewriteAssuranceV2,
    pub parameter_schema: serde_json::Value,
    pub transformer_ref: String,
    pub allowed_path_patterns: Vec<String>,
    pub intended_postconditions: Vec<String>,
    pub validation_families: Vec<String>,
    pub permission_actions: Vec<String>,
    pub idempotence_contract: String,
    pub rollback_contract: String,
    pub definition_fingerprint: Sha256Hash,
}

impl ChangeRecipeV2 {
    pub fn seal(mut self) -> Result<Self, PatchV2Error> {
        self.selector_kinds.sort();
        self.selector_kinds.dedup();
        sort_dedup_strings(&mut self.allowed_path_patterns);
        sort_dedup_strings(&mut self.intended_postconditions);
        sort_dedup_strings(&mut self.validation_families);
        sort_dedup_strings(&mut self.permission_actions);
        if self.schema_id != CHANGE_RECIPE_V2_SCHEMA_ID
            || self.schema_version != PATCH_V2_SCHEMA_VERSION
            || !valid_stable_name(&self.recipe_id)
            || Version::parse(&self.recipe_version).is_err()
            || self.display_name.trim().is_empty()
            || self.selector_kinds.is_empty()
            || !self.parameter_schema.is_object()
            || self.transformer_ref.trim().is_empty()
            || self.allowed_path_patterns.is_empty()
            || self.intended_postconditions.is_empty()
            || self.validation_families.is_empty()
            || self.permission_actions.is_empty()
            || self.idempotence_contract.trim().is_empty()
            || self.rollback_contract.trim().is_empty()
        {
            return Err(PatchV2Error::Recipe);
        }
        self.definition_fingerprint = patch_fingerprint(
            "star.change-recipe",
            PATCH_V2_SCHEMA_VERSION,
            &serde_json::json!({
                "recipe_id":self.recipe_id,
                "recipe_version":self.recipe_version,
                "display_name":self.display_name,
                "language":self.language,
                "selector_kinds":self.selector_kinds,
                "rewrite_assurance":self.rewrite_assurance,
                "parameter_schema":self.parameter_schema,
                "transformer_ref":self.transformer_ref,
                "allowed_path_patterns":self.allowed_path_patterns,
                "intended_postconditions":self.intended_postconditions,
                "validation_families":self.validation_families,
                "permission_actions":self.permission_actions,
                "idempotence_contract":self.idempotence_contract,
                "rollback_contract":self.rollback_contract,
            }),
        )?;
        Ok(self)
    }

    pub fn reference(&self) -> RecipeRefV2 {
        RecipeRefV2 {
            recipe_id: self.recipe_id.clone(),
            recipe_version: self.recipe_version.clone(),
            definition_fingerprint: self.definition_fingerprint.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecipeRefV2 {
    pub recipe_id: String,
    pub recipe_version: String,
    pub definition_fingerprint: Sha256Hash,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PatchOperationKindV2 {
    Add,
    Modify,
    Delete,
    Rename,
    GeneratorInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PatchOperation {
    pub operation_id: String,
    pub kind: PatchOperationKindV2,
    pub path: ProjectPathRef,
    pub rename_from: Option<ProjectPathRef>,
    pub before_sha256: Option<Sha256Hash>,
    pub after_sha256: Option<Sha256Hash>,
    pub before_mode: Option<u32>,
    pub after_mode: Option<u32>,
    pub forward_artifact_ref: ArtifactRef,
    pub reverse_artifact_ref: ArtifactRef,
    pub operation_fingerprint: Sha256Hash,
}

impl PatchOperation {
    pub fn seal(mut self) -> Result<Self, PatchV2Error> {
        let shape_valid = match self.kind {
            PatchOperationKindV2::Add => {
                self.rename_from.is_none()
                    && self.before_sha256.is_none()
                    && self.after_sha256.is_some()
            }
            PatchOperationKindV2::Modify | PatchOperationKindV2::GeneratorInput => {
                self.rename_from.is_none()
                    && self.before_sha256.is_some()
                    && self.after_sha256.is_some()
                    && self.before_sha256 != self.after_sha256
            }
            PatchOperationKindV2::Delete => {
                self.rename_from.is_none()
                    && self.before_sha256.is_some()
                    && self.after_sha256.is_none()
            }
            PatchOperationKindV2::Rename => {
                self.rename_from
                    .as_ref()
                    .is_some_and(|from| from != &self.path)
                    && self.before_sha256.is_some()
                    && self.after_sha256.is_some()
            }
        };
        if !valid_stable_name(&self.operation_id)
            || !shape_valid
            || self.forward_artifact_ref.validate().is_err()
            || self.reverse_artifact_ref.validate().is_err()
            || self.forward_artifact_ref.sha256 == self.reverse_artifact_ref.sha256
        {
            return Err(PatchV2Error::Operation);
        }
        self.operation_fingerprint = patch_fingerprint(
            "star.patch-operation",
            PATCH_V2_SCHEMA_VERSION,
            &serde_json::json!({
                "operation_id":self.operation_id,
                "kind":self.kind,
                "path":self.path,
                "rename_from":self.rename_from,
                "before_sha256":self.before_sha256,
                "after_sha256":self.after_sha256,
                "before_mode":self.before_mode,
                "after_mode":self.after_mode,
                "forward_artifact_ref":self.forward_artifact_ref,
                "reverse_artifact_ref":self.reverse_artifact_ref,
            }),
        )?;
        Ok(self)
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PatchSetStateV2 {
    Previewed,
    ReplanRequired,
    Ready,
    Applying,
    Applied,
    PartiallyApplied,
    OutcomeUnknown,
    Reverted,
    RecoveryBlocked,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PatchSetV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub patch_set_id: PatchSetId,
    pub revision: u64,
    pub recipe_ref: RecipeRefV2,
    pub recipe_execution_ref: DocumentRef,
    pub project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub change_plan_id: ChangePlanId,
    pub change_plan_revision: u64,
    pub base_workspace_snapshot_id: WorkspaceSnapshotId,
    pub target_selector_fingerprint: Sha256Hash,
    pub parameter_fingerprint: Sha256Hash,
    pub operations: Vec<PatchOperation>,
    pub preview_change_set_ref: DocumentRef,
    pub preview_impact_analysis_ref: DocumentRef,
    pub preview_validation_plan_ref: DocumentRef,
    pub expected_operation_set_fingerprint: Sha256Hash,
    pub completeness: Completeness,
    pub limitations: Vec<String>,
    pub state: PatchSetStateV2,
    pub created_at: DateTime<Utc>,
    pub patch_fingerprint: Sha256Hash,
}

impl PatchSetV2 {
    pub fn seal(mut self) -> Result<Self, PatchV2Error> {
        self.operations = self
            .operations
            .into_iter()
            .map(PatchOperation::seal)
            .collect::<Result<Vec<_>, _>>()?;
        self.operations.sort_by(|left, right| {
            (&left.path, &left.operation_id).cmp(&(&right.path, &right.operation_id))
        });
        sort_dedup_strings(&mut self.limitations);
        let operation_ids = self
            .operations
            .iter()
            .map(|operation| operation.operation_id.as_str())
            .collect::<BTreeSet<_>>();
        let operation_paths = self
            .operations
            .iter()
            .map(|operation| &operation.path)
            .collect::<BTreeSet<_>>();
        if self.schema_id != PATCH_SET_V2_SCHEMA_ID
            || self.schema_version != PATCH_V2_SCHEMA_VERSION
            || self.revision == 0
            || self.change_plan_revision == 0
            || self.recipe_execution_ref.revision == 0
            || self.preview_change_set_ref.revision == 0
            || self.preview_impact_analysis_ref.revision == 0
            || self.preview_validation_plan_ref.revision == 0
            || self.operations.is_empty()
            || operation_ids.len() != self.operations.len()
            || operation_paths.len() != self.operations.len()
            || (self.completeness == Completeness::Complete && !self.limitations.is_empty())
            || (self.completeness != Completeness::Complete && self.limitations.is_empty())
            || (self.state == PatchSetStateV2::Ready && self.completeness != Completeness::Complete)
        {
            return Err(PatchV2Error::PatchSet);
        }
        self.expected_operation_set_fingerprint = patch_fingerprint(
            "star.patch-operation-set",
            PATCH_V2_SCHEMA_VERSION,
            &self
                .operations
                .iter()
                .map(|operation| &operation.operation_fingerprint)
                .collect::<Vec<_>>(),
        )?;
        self.patch_fingerprint = patch_fingerprint(
            "star.patch-set",
            PATCH_V2_SCHEMA_VERSION,
            &serde_json::json!({
                "patch_set_id":self.patch_set_id,
                "revision":self.revision,
                "recipe_ref":self.recipe_ref,
                "recipe_execution_ref":self.recipe_execution_ref,
                "project_id":self.project_id,
                "checkout_id":self.checkout_id,
                "change_plan_id":self.change_plan_id,
                "change_plan_revision":self.change_plan_revision,
                "base_workspace_snapshot_id":self.base_workspace_snapshot_id,
                "target_selector_fingerprint":self.target_selector_fingerprint,
                "parameter_fingerprint":self.parameter_fingerprint,
                "operations":self.operations,
                "preview_change_set_ref":self.preview_change_set_ref,
                "preview_impact_analysis_ref":self.preview_impact_analysis_ref,
                "preview_validation_plan_ref":self.preview_validation_plan_ref,
                "expected_operation_set_fingerprint":self.expected_operation_set_fingerprint,
                "completeness":self.completeness,
                "limitations":self.limitations,
                "state":self.state,
                "created_at":self.created_at,
            }),
        )?;
        Ok(self)
    }

    pub fn reference(&self) -> Result<DocumentRef, PatchV2Error> {
        document_reference(
            PATCH_SET_V2_SCHEMA_ID,
            self.patch_set_id.as_str(),
            self.revision,
            self,
        )
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RecipeExecutionStateV1 {
    Started,
    Previewed,
    AlreadySatisfied,
    ReplanRequired,
    Failed,
    OutcomeUnknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecipeExecution {
    pub schema_id: String,
    pub schema_version: u32,
    pub recipe_execution_id: RecipeExecutionId,
    pub revision: u64,
    pub recipe_ref: RecipeRefV2,
    pub project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub base_workspace_snapshot_id: WorkspaceSnapshotId,
    pub target_selector: TargetSelector,
    pub target_selector_fingerprint: Sha256Hash,
    pub parameters: serde_json::Value,
    pub parameter_fingerprint: Sha256Hash,
    pub worktree_decision_ref: DocumentRef,
    pub first_preview_artifact_refs: Vec<ArtifactRef>,
    pub replay_preview_artifact_refs: Vec<ArtifactRef>,
    pub preview_change_set_ref: Option<DocumentRef>,
    pub replan_bundle_ref: Option<DocumentRef>,
    pub idempotence_proved: bool,
    pub completeness: Completeness,
    pub limitations: Vec<String>,
    pub state: RecipeExecutionStateV1,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub execution_fingerprint: Sha256Hash,
}

impl RecipeExecution {
    pub fn seal(mut self) -> Result<Self, PatchV2Error> {
        self.target_selector.validate()?;
        self.first_preview_artifact_refs.sort_by(artifact_order);
        self.first_preview_artifact_refs.dedup();
        self.replay_preview_artifact_refs.sort_by(artifact_order);
        self.replay_preview_artifact_refs.dedup();
        sort_dedup_strings(&mut self.limitations);
        self.target_selector_fingerprint = patch_fingerprint(
            "star.target-selector",
            PATCH_LIFECYCLE_SCHEMA_VERSION,
            &self.target_selector,
        )?;
        self.parameter_fingerprint = patch_fingerprint(
            "star.recipe-parameters",
            PATCH_LIFECYCLE_SCHEMA_VERSION,
            &self.parameters,
        )?;
        let terminal = self.state != RecipeExecutionStateV1::Started;
        if self.schema_id != RECIPE_EXECUTION_SCHEMA_ID
            || self.schema_version != PATCH_LIFECYCLE_SCHEMA_VERSION
            || self.revision == 0
            || self.target_selector.project_id() != &self.project_id
            || !self.parameters.is_object()
            || self.worktree_decision_ref.revision == 0
            || (terminal != self.finished_at.is_some())
            || (self.completeness == Completeness::Complete && !self.limitations.is_empty())
            || (self.completeness != Completeness::Complete && self.limitations.is_empty())
            || (matches!(
                self.state,
                RecipeExecutionStateV1::Previewed | RecipeExecutionStateV1::AlreadySatisfied
            ) && (!self.idempotence_proved || self.first_preview_artifact_refs.is_empty()))
            || (self.state == RecipeExecutionStateV1::Previewed
                && self.preview_change_set_ref.is_none())
        {
            return Err(PatchV2Error::RecipeExecution);
        }
        self.execution_fingerprint = patch_fingerprint(
            "star.recipe-execution",
            PATCH_LIFECYCLE_SCHEMA_VERSION,
            &serde_json::json!({
                "recipe_execution_id":self.recipe_execution_id,
                "revision":self.revision,
                "recipe_ref":self.recipe_ref,
                "project_id":self.project_id,
                "checkout_id":self.checkout_id,
                "base_workspace_snapshot_id":self.base_workspace_snapshot_id,
                "target_selector":self.target_selector,
                "target_selector_fingerprint":self.target_selector_fingerprint,
                "parameters":self.parameters,
                "parameter_fingerprint":self.parameter_fingerprint,
                "worktree_decision_ref":self.worktree_decision_ref,
                "first_preview_artifact_refs":self.first_preview_artifact_refs,
                "replay_preview_artifact_refs":self.replay_preview_artifact_refs,
                "preview_change_set_ref":self.preview_change_set_ref,
                "replan_bundle_ref":self.replan_bundle_ref,
                "idempotence_proved":self.idempotence_proved,
                "completeness":self.completeness,
                "limitations":self.limitations,
                "state":self.state,
                "started_at":self.started_at,
                "finished_at":self.finished_at,
            }),
        )?;
        Ok(self)
    }

    pub fn reference(&self) -> Result<DocumentRef, PatchV2Error> {
        document_reference(
            RECIPE_EXECUTION_SCHEMA_ID,
            self.recipe_execution_id.as_str(),
            self.revision,
            self,
        )
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeStrategyV1 {
    Current,
    Isolated,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeDecisionStateV1 {
    Selected,
    Materialized,
    RetainedForRecovery,
    Discarded,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorktreeDecision {
    pub schema_id: String,
    pub schema_version: u32,
    pub worktree_decision_id: WorktreeDecisionId,
    pub revision: u64,
    pub project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub base_workspace_snapshot_id: WorkspaceSnapshotId,
    pub strategy: WorktreeStrategyV1,
    pub reason_codes: Vec<String>,
    pub isolated_locator_fingerprint: Option<Sha256Hash>,
    pub materialization_artifact_refs: Vec<ArtifactRef>,
    pub state: WorktreeDecisionStateV1,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub decision_fingerprint: Sha256Hash,
}

impl WorktreeDecision {
    pub fn seal(mut self) -> Result<Self, PatchV2Error> {
        sort_dedup_strings(&mut self.reason_codes);
        self.materialization_artifact_refs.sort_by(artifact_order);
        self.materialization_artifact_refs.dedup();
        if self.schema_id != WORKTREE_DECISION_SCHEMA_ID
            || self.schema_version != PATCH_LIFECYCLE_SCHEMA_VERSION
            || self.revision == 0
            || self.reason_codes.is_empty()
            || self.updated_at < self.created_at
            || (self.strategy == WorktreeStrategyV1::Current
                && (self.isolated_locator_fingerprint.is_some()
                    || !self.materialization_artifact_refs.is_empty()))
            || (self.strategy == WorktreeStrategyV1::Isolated
                && matches!(
                    self.state,
                    WorktreeDecisionStateV1::Materialized
                        | WorktreeDecisionStateV1::RetainedForRecovery
                )
                && (self.isolated_locator_fingerprint.is_none()
                    || self.materialization_artifact_refs.is_empty()))
        {
            return Err(PatchV2Error::WorktreeDecision);
        }
        self.decision_fingerprint = patch_fingerprint(
            "star.worktree-decision",
            PATCH_LIFECYCLE_SCHEMA_VERSION,
            &serde_json::json!({
                "worktree_decision_id":self.worktree_decision_id,
                "revision":self.revision,
                "project_id":self.project_id,
                "checkout_id":self.checkout_id,
                "base_workspace_snapshot_id":self.base_workspace_snapshot_id,
                "strategy":self.strategy,
                "reason_codes":self.reason_codes,
                "isolated_locator_fingerprint":self.isolated_locator_fingerprint,
                "materialization_artifact_refs":self.materialization_artifact_refs,
                "state":self.state,
                "created_at":self.created_at,
                "updated_at":self.updated_at,
            }),
        )?;
        Ok(self)
    }

    pub fn reference(&self) -> Result<DocumentRef, PatchV2Error> {
        document_reference(
            WORKTREE_DECISION_SCHEMA_ID,
            self.worktree_decision_id.as_str(),
            self.revision,
            self,
        )
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PatchOperationReceiptStateV1 {
    NotStarted,
    AppliedExact,
    FailedBeforeEffect,
    FailedAfterEffect,
    OutcomeUnknown,
    RevertedExact,
    RecoveryBlocked,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PatchOperationReceiptV1 {
    pub operation_id: String,
    pub operation_fingerprint: Sha256Hash,
    pub state: PatchOperationReceiptStateV1,
    pub observed_before_sha256: Option<Sha256Hash>,
    pub observed_after_sha256: Option<Sha256Hash>,
    pub effect_receipt_ref: Option<ArtifactRef>,
    pub reason_code: Option<String>,
    pub recorded_at: DateTime<Utc>,
    pub receipt_fingerprint: Sha256Hash,
}

impl PatchOperationReceiptV1 {
    pub fn seal(mut self) -> Result<Self, PatchV2Error> {
        let affected = matches!(
            self.state,
            PatchOperationReceiptStateV1::AppliedExact
                | PatchOperationReceiptStateV1::FailedAfterEffect
                | PatchOperationReceiptStateV1::OutcomeUnknown
                | PatchOperationReceiptStateV1::RevertedExact
                | PatchOperationReceiptStateV1::RecoveryBlocked
        );
        if !valid_stable_name(&self.operation_id)
            || (affected && self.effect_receipt_ref.is_none())
            || self
                .effect_receipt_ref
                .as_ref()
                .is_some_and(|reference| reference.validate().is_err())
            || (matches!(
                self.state,
                PatchOperationReceiptStateV1::FailedBeforeEffect
                    | PatchOperationReceiptStateV1::FailedAfterEffect
                    | PatchOperationReceiptStateV1::OutcomeUnknown
                    | PatchOperationReceiptStateV1::RecoveryBlocked
            ) && self.reason_code.as_deref().is_none_or(str::is_empty))
        {
            return Err(PatchV2Error::Receipt);
        }
        self.receipt_fingerprint = patch_fingerprint(
            "star.patch-operation-receipt",
            PATCH_LIFECYCLE_SCHEMA_VERSION,
            &serde_json::json!({
                "operation_id":self.operation_id,
                "operation_fingerprint":self.operation_fingerprint,
                "state":self.state,
                "observed_before_sha256":self.observed_before_sha256,
                "observed_after_sha256":self.observed_after_sha256,
                "effect_receipt_ref":self.effect_receipt_ref,
                "reason_code":self.reason_code,
                "recorded_at":self.recorded_at,
            }),
        )?;
        Ok(self)
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PatchPermitKindRecordV1 {
    Automatic,
    ManualApproved,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PatchApplicationStateV1 {
    Requested,
    Preflighted,
    Applying,
    Applied,
    PartiallyApplied,
    OutcomeUnknown,
    AwaitingHumanReview,
    RecoveryRequired,
    Reverted,
    IsolatedDiscarded,
    RecoveryBlocked,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PatchApplication {
    pub schema_id: String,
    pub schema_version: u32,
    pub patch_application_id: PatchApplicationId,
    pub revision: u64,
    pub patch_set_ref: DocumentRef,
    pub project_id: ProjectId,
    pub checkout_id: CheckoutId,
    pub worktree_decision_ref: DocumentRef,
    pub requested_patch_fingerprint: Sha256Hash,
    pub permission_fingerprint: Sha256Hash,
    pub pre_gate_decision_ref: Option<GateDecisionRef>,
    pub permit_kind: Option<PatchPermitKindRecordV1>,
    pub operation_receipts: Vec<PatchOperationReceiptV1>,
    pub actual_operation_set_fingerprint: Option<Sha256Hash>,
    pub observed_after_change_set_ref: Option<DocumentRef>,
    pub post_gate_decision_ref: Option<GateDecisionRef>,
    pub reverse_patch_set_ref: Option<DocumentRef>,
    pub recovery_strategy: Option<PatchRecoveryStrategyV1>,
    pub state: PatchApplicationStateV1,
    pub reason_codes: Vec<String>,
    pub requested_by: ActorRef,
    pub requested_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub application_fingerprint: Sha256Hash,
}

impl PatchApplication {
    pub fn seal(mut self) -> Result<Self, PatchV2Error> {
        self.operation_receipts = self
            .operation_receipts
            .into_iter()
            .map(PatchOperationReceiptV1::seal)
            .collect::<Result<Vec<_>, _>>()?;
        self.operation_receipts
            .sort_by(|left, right| left.operation_id.cmp(&right.operation_id));
        sort_dedup_strings(&mut self.reason_codes);
        let receipt_ids = self
            .operation_receipts
            .iter()
            .map(|receipt| receipt.operation_id.as_str())
            .collect::<BTreeSet<_>>();
        let effect_started = !matches!(
            self.state,
            PatchApplicationStateV1::Requested
                | PatchApplicationStateV1::Preflighted
                | PatchApplicationStateV1::IsolatedDiscarded
        );
        let complete = matches!(
            self.state,
            PatchApplicationStateV1::Applied
                | PatchApplicationStateV1::Reverted
                | PatchApplicationStateV1::IsolatedDiscarded
        );
        if self.schema_id != PATCH_APPLICATION_SCHEMA_ID
            || self.schema_version != PATCH_LIFECYCLE_SCHEMA_VERSION
            || self.revision == 0
            || self.patch_set_ref.revision == 0
            || self.worktree_decision_ref.revision == 0
            || self.updated_at < self.requested_at
            || self.requested_by.actor_id.trim().is_empty()
            || self.requested_by.auth_source.trim().is_empty()
            || receipt_ids.len() != self.operation_receipts.len()
            || (effect_started
                && (self.pre_gate_decision_ref.is_none() || self.permit_kind.is_none()))
            || (complete && self.reason_codes.iter().any(String::is_empty))
            || (matches!(
                self.state,
                PatchApplicationStateV1::PartiallyApplied
                    | PatchApplicationStateV1::OutcomeUnknown
                    | PatchApplicationStateV1::RecoveryRequired
                    | PatchApplicationStateV1::RecoveryBlocked
            ) && self.reason_codes.is_empty())
            || (self.state == PatchApplicationStateV1::Applied
                && (self.actual_operation_set_fingerprint.is_none()
                    || self.observed_after_change_set_ref.is_none()
                    || self.post_gate_decision_ref.is_none()))
            || (self.state == PatchApplicationStateV1::IsolatedDiscarded
                && (!self.operation_receipts.is_empty()
                    || self.actual_operation_set_fingerprint.is_some()
                    || self.observed_after_change_set_ref.is_some()
                    || self.post_gate_decision_ref.is_some()))
        {
            return Err(PatchV2Error::PatchApplication);
        }
        self.application_fingerprint = patch_fingerprint(
            "star.patch-application",
            PATCH_LIFECYCLE_SCHEMA_VERSION,
            &serde_json::json!({
                "patch_application_id":self.patch_application_id,
                "revision":self.revision,
                "patch_set_ref":self.patch_set_ref,
                "project_id":self.project_id,
                "checkout_id":self.checkout_id,
                "worktree_decision_ref":self.worktree_decision_ref,
                "requested_patch_fingerprint":self.requested_patch_fingerprint,
                "permission_fingerprint":self.permission_fingerprint,
                "pre_gate_decision_ref":self.pre_gate_decision_ref,
                "permit_kind":self.permit_kind,
                "operation_receipts":self.operation_receipts,
                "actual_operation_set_fingerprint":self.actual_operation_set_fingerprint,
                "observed_after_change_set_ref":self.observed_after_change_set_ref,
                "post_gate_decision_ref":self.post_gate_decision_ref,
                "reverse_patch_set_ref":self.reverse_patch_set_ref,
                "recovery_strategy":self.recovery_strategy,
                "state":self.state,
                "reason_codes":self.reason_codes,
                "requested_by":self.requested_by,
                "requested_at":self.requested_at,
                "updated_at":self.updated_at,
            }),
        )?;
        Ok(self)
    }

    pub fn reference(&self) -> Result<DocumentRef, PatchV2Error> {
        document_reference(
            PATCH_APPLICATION_SCHEMA_ID,
            self.patch_application_id.as_str(),
            self.revision,
            self,
        )
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PatchRecoveryStrategyV1 {
    ReversePatch,
    DiscardIsolated,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PatchV1ToV2MigrationEntry {
    pub legacy_patch_set_ref: DocumentRef,
    pub projected_patch_set_ref: DocumentRef,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PatchV1ToV2MigrationPlan {
    pub schema_id: String,
    pub schema_version: u32,
    pub project_id: ProjectId,
    pub entries: Vec<PatchV1ToV2MigrationEntry>,
    pub dry_run: bool,
    pub backup_required: bool,
    pub rollback_supported: bool,
    pub plan_fingerprint: Sha256Hash,
}

impl PatchV1ToV2MigrationPlan {
    pub fn seal(mut self) -> Result<Self, PatchV2Error> {
        self.entries.sort_by(|left, right| {
            left.legacy_patch_set_ref
                .document_id
                .cmp(&right.legacy_patch_set_ref.document_id)
        });
        for entry in &mut self.entries {
            sort_dedup_strings(&mut entry.limitations);
        }
        if self.schema_id != PATCH_V1_TO_V2_MIGRATION_PLAN_SCHEMA_ID
            || self.schema_version != PATCH_LIFECYCLE_SCHEMA_VERSION
            || self.entries.is_empty()
            || !self.dry_run
            || !self.backup_required
            || !self.rollback_supported
            || self.entries.windows(2).any(|pair| {
                pair[0].legacy_patch_set_ref.document_id == pair[1].legacy_patch_set_ref.document_id
            })
        {
            return Err(PatchV2Error::Migration);
        }
        self.plan_fingerprint = patch_fingerprint(
            "star.patch-v1-to-v2-migration-plan",
            PATCH_LIFECYCLE_SCHEMA_VERSION,
            &serde_json::json!({
                "project_id":self.project_id,
                "entries":self.entries,
                "dry_run":self.dry_run,
                "backup_required":self.backup_required,
                "rollback_supported":self.rollback_supported,
            }),
        )?;
        Ok(self)
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PatchMigrationOutcomeV1 {
    Applied,
    RolledBack,
    Incompatible,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PatchV1ToV2MigrationResult {
    pub schema_id: String,
    pub schema_version: u32,
    pub project_id: ProjectId,
    pub plan_fingerprint: Sha256Hash,
    pub backup_manifest_ref: Option<ArtifactRef>,
    pub migrated_patch_set_refs: Vec<DocumentRef>,
    pub outcome: PatchMigrationOutcomeV1,
    pub reason_codes: Vec<String>,
    pub completed_at: DateTime<Utc>,
    pub result_fingerprint: Sha256Hash,
}

impl PatchV1ToV2MigrationResult {
    pub fn seal(mut self) -> Result<Self, PatchV2Error> {
        self.migrated_patch_set_refs.sort_by(document_ref_order);
        self.migrated_patch_set_refs.dedup();
        sort_dedup_strings(&mut self.reason_codes);
        if self.schema_id != PATCH_V1_TO_V2_MIGRATION_RESULT_SCHEMA_ID
            || self.schema_version != PATCH_LIFECYCLE_SCHEMA_VERSION
            || self
                .backup_manifest_ref
                .as_ref()
                .is_some_and(|reference| reference.validate().is_err())
            || (self.outcome == PatchMigrationOutcomeV1::Applied
                && (self.backup_manifest_ref.is_none()
                    || self.migrated_patch_set_refs.is_empty()
                    || !self.reason_codes.is_empty()))
            || (self.outcome != PatchMigrationOutcomeV1::Applied && self.reason_codes.is_empty())
        {
            return Err(PatchV2Error::Migration);
        }
        self.result_fingerprint = patch_fingerprint(
            "star.patch-v1-to-v2-migration-result",
            PATCH_LIFECYCLE_SCHEMA_VERSION,
            &serde_json::json!({
                "project_id":self.project_id,
                "plan_fingerprint":self.plan_fingerprint,
                "backup_manifest_ref":self.backup_manifest_ref,
                "migrated_patch_set_refs":self.migrated_patch_set_refs,
                "outcome":self.outcome,
                "reason_codes":self.reason_codes,
                "completed_at":self.completed_at,
            }),
        )?;
        Ok(self)
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PatchV2Error {
    #[error("target selector is invalid or crosses its exact project scope")]
    Selector,
    #[error("change recipe v2 is invalid")]
    Recipe,
    #[error("patch operation is invalid")]
    Operation,
    #[error("patch set v2 is invalid")]
    PatchSet,
    #[error("recipe execution is invalid")]
    RecipeExecution,
    #[error("worktree decision is invalid")]
    WorktreeDecision,
    #[error("patch operation receipt is invalid")]
    Receipt,
    #[error("patch application is invalid")]
    PatchApplication,
    #[error("patch migration contract is invalid")]
    Migration,
    #[error("patch contract fingerprint could not be calculated")]
    Fingerprint,
}

fn valid_stable_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'/')
        })
}

fn sort_dedup_strings(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

fn artifact_order(left: &ArtifactRef, right: &ArtifactRef) -> std::cmp::Ordering {
    (
        left.artifact_id.as_str(),
        left.relative_path.as_str(),
        left.sha256.as_str(),
    )
        .cmp(&(
            right.artifact_id.as_str(),
            right.relative_path.as_str(),
            right.sha256.as_str(),
        ))
}

fn document_ref_order(left: &DocumentRef, right: &DocumentRef) -> std::cmp::Ordering {
    (
        left.schema_id.as_str(),
        left.document_id.as_str(),
        left.revision,
        left.sha256.as_str(),
    )
        .cmp(&(
            right.schema_id.as_str(),
            right.document_id.as_str(),
            right.revision,
            right.sha256.as_str(),
        ))
}

fn patch_fingerprint<T: Serialize>(
    domain: &str,
    version: u32,
    value: &T,
) -> Result<Sha256Hash, PatchV2Error> {
    canonical_sha256(&serde_json::json!({
        "domain":domain,
        "version":version,
        "value":value,
    }))
    .map_err(|_| PatchV2Error::Fingerprint)
}

fn document_reference<T: Serialize>(
    schema_id: &str,
    document_id: &str,
    revision: u64,
    value: &T,
) -> Result<DocumentRef, PatchV2Error> {
    let value = serde_json::to_value(value).map_err(|_| PatchV2Error::Fingerprint)?;
    Ok(DocumentRef {
        schema_id: schema_id.to_owned(),
        document_id: document_id.to_owned(),
        revision,
        sha256: canonical_sha256(&value).map_err(|_| PatchV2Error::Fingerprint)?,
    })
}
